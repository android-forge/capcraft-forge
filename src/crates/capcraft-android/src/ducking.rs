// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Auto ducking: a music clip turned down wherever the other sound on the
//! timeline speaks - a voice-over, a narration, the people in the videos - and
//! back up when it stops, with volume keys that fade in and out.
//!
//! Who speaks is read from the waveforms the timeline already draws and caches
//! (`concat_host::media::peaks`): a stretch of the other clips loud against
//! their own usual level, held through the short pauses between words. It
//! hears any sound as loud as a voice, which on a reel is what the music
//! should step back for.

use concat_project::Command;
use concat_project::model::{ClipKind, KeyEase, KeyProperty, MediaKind};

use crate::MobileError;
use crate::session::{MobileSession, TimelineView};

/// The step the loudness is read at, seconds.
const FRAME: f64 = 0.02;
/// A pause between words shorter than this keeps the music down.
const GAP: f64 = 0.45;
/// A sound shorter than this is a click, not a voice.
const SHORTEST: f64 = 0.25;
/// How long the music takes to go down before a voice, seconds.
const DOWN: f64 = 0.25;
/// How long it takes to come back up after one.
const UP: f64 = 0.5;

fn failed(message: impl Into<String>) -> MobileError {
    MobileError::Failed(message.into())
}

/// One clip whose sound the music ducks under.
struct Voice {
    path: String,
    start: f64,
    duration: f64,
    source_start: f64,
    speed: f64,
    volume: f64,
}

/// The stretches of the timeline where `voice` sounds, from its waveform.
fn spoken(voice: &Voice, project: &str) -> Vec<(f64, f64)> {
    let Ok(peaks) = concat_host::media::peaks(&voice.path, None, Some(project)) else {
        return Vec::new();
    };
    let frames = (voice.duration / FRAME).floor().max(0.0) as usize;
    let levels: Vec<f32> = (0..frames)
        .map(|i| {
            let at = voice.source_start + i as f64 * FRAME * voice.speed;
            let (low, high) = peaks.extremes(at as f32, (at + FRAME * voice.speed) as f32);
            high.max(-low) * voice.volume as f32
        })
        .collect();
    if levels.is_empty() {
        return Vec::new();
    }
    // Loud against the clip's own loudest stretches: a quiet narration and a
    // loud one are both heard, a room's hiss under them is not.
    let mut sorted = levels.clone();
    sorted.sort_by(f32::total_cmp);
    let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
    let threshold = (p95 * 0.3).max(0.02);
    let mut runs: Vec<(f64, f64)> = Vec::new();
    for (i, level) in levels.iter().enumerate() {
        if *level < threshold {
            continue;
        }
        let at = voice.start + i as f64 * FRAME;
        match runs.last_mut() {
            Some(run) if at - run.1 <= GAP => run.1 = at + FRAME,
            _ => runs.push((at, at + FRAME)),
        }
    }
    runs.retain(|(a, b)| b - a >= SHORTEST);
    runs
}

/// `runs` from every voice as one sorted list, overlaps and near misses joined.
fn joined(mut runs: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    runs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (a, b) in runs {
        match out.last_mut() {
            // close enough that the music would only bob up between them
            Some(last) if a - last.1 <= DOWN + UP => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

#[uniffi::export]
impl MobileSession {
    /// Turns the clip `clip_id` - music, as a rule - down by `depth_db`
    /// wherever another clip's sound speaks over it, and back up after, as
    /// volume keys in one undo step. Its earlier volume keys are replaced.
    pub fn auto_duck(&self, clip_id: String, depth_db: f64) -> Result<TimelineView, MobileError> {
        let (target, voices, keys, project) = self.with_session(|session| {
            let project = session.project();
            let timeline = project.active();
            let target = timeline
                .clips
                .iter()
                .find(|c| c.id == clip_id)
                .ok_or_else(|| failed("That clip is gone."))?
                .clone();
            if !matches!(target.kind, ClipKind::Audio | ClipKind::Video) {
                return Err(failed("Choose a clip with sound to turn down."));
            }
            let muted = |track: &str| timeline.tracks.iter().any(|t| t.id == track && t.muted);
            let voices: Vec<Voice> = timeline
                .clips
                .iter()
                .filter(|c| c.id != clip_id && matches!(c.kind, ClipKind::Audio | ClipKind::Video))
                .filter(|c| c.volume > 0.0 && !muted(&c.track_id) && c.muted != Some(true))
                .filter(|c| c.start < target.start + target.duration && c.start + c.duration > target.start)
                .filter_map(|c| {
                    let media = project.media.iter().find(|m| m.id == c.media_id)?;
                    (media.has_audio && media.kind != MediaKind::Image).then(|| Voice {
                        path: media.path.clone(),
                        start: c.start,
                        duration: c.duration,
                        source_start: c.source_start,
                        speed: c.speed.max(0.01),
                        volume: c.volume,
                    })
                })
                .collect();
            let keys: Vec<f64> = target
                .keys
                .iter()
                .filter(|k| k.property == KeyProperty::Volume)
                .map(|k| k.at)
                .collect();
            Ok((target, voices, keys, session.path().to_owned()))
        })?;
        if voices.is_empty() {
            return Err(failed("There is no other sound under this clip to make room for."));
        }
        let spoken = joined(voices.iter().flat_map(|v| spoken(v, &project)).collect());
        let (start, end) = (target.start, target.start + target.duration);
        let spoken: Vec<(f64, f64)> = spoken
            .into_iter()
            .filter(|(a, b)| *b > start && *a < end)
            .collect();
        if spoken.is_empty() {
            return Err(failed("No voice was heard under this clip."));
        }
        let full = target.volume;
        let low = full * 10f64.powf(-depth_db.abs() / 20.0);
        let at = |t: f64| ((t - start) / target.duration.max(1e-6)).clamp(0.0, 1.0);
        let mut commands: Vec<Command> = keys
            .into_iter()
            .map(|key| Command::ClearClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::Volume,
                at: key,
            })
            .collect();
        let key = |t: f64, value: f64| Command::SetClipKey {
            clip_id: clip_id.clone(),
            property: KeyProperty::Volume,
            at: at(t),
            value,
            ease: KeyEase::LINEAR,
        };
        for (a, b) in &spoken {
            if *a - DOWN > start {
                commands.push(key(*a - DOWN, full));
            }
            commands.push(key(a.max(start), low));
            commands.push(key(b.min(end), low));
            if *b + UP < end {
                commands.push(key(*b + UP, full));
            }
        }
        log::info!("ducked under {} stretch(es) of voice", spoken.len());
        self.run(commands)
    }
}
