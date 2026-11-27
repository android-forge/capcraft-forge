// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Auto highlight: the liveliest moments of the main track kept, the rest
//! cut, so a long video becomes a short one.
//!
//! A moment is lively where its sound is louder than the talk around it and
//! changes more - laughter, a goal, a raised voice, a hit of music - heard
//! second by second. The best moments are taken with a little before and
//! after each, until the length asked for is filled, and left in their
//! order.

use concat_media::{AudioDecoder, AudioOptions, SampleFormat};
use concat_project::model::{ClipKind, MediaKind};

use crate::session::{MobileSession, TimelineView};
use crate::{MobileError, seconds};

/// The rate the sound is read at for this.
const RATE: u32 = 8_000;
/// How long each measured moment is, seconds.
const STEP: f64 = 0.5;
/// How much is kept before a lively moment and after it, seconds.
const BEFORE: f64 = 1.5;
const AFTER: f64 = 2.0;

/// How lively each [`STEP`] of `samples` (mono, at [`RATE`]) is: how far over
/// the whole's middle loudness, and how much its loudness moves.
fn liveliness(samples: &[f32]) -> Vec<f64> {
    let step = (f64::from(RATE) * STEP) as usize;
    let slice = step / 10;
    let db: Vec<Vec<f64>> = samples
        .chunks(step)
        .map(|chunk| {
            chunk
                .chunks(slice.max(1))
                .map(|part| {
                    let power = part.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>()
                        / part.len().max(1) as f64;
                    10.0 * power.max(1e-9).log10()
                })
                .collect()
        })
        .collect();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let loud: Vec<f64> = db.iter().map(|v| mean(v)).collect();
    let mut sorted = loud.clone();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    loud.iter()
        .zip(&db)
        .map(|(level, parts)| {
            let spread = (parts.iter().map(|p| (p - level).powi(2)).sum::<f64>()
                / parts.len().max(1) as f64)
                .sqrt();
            // silence moves as much as speech when a sound begins in it
            let heard = ((level + 60.0) / 30.0).clamp(0.0, 1.0);
            (level - middle) + 0.35 * spread.min(12.0) * heard
        })
        .collect()
}

/// The stretches of `0..length` to keep for about `wanted` seconds, by how
/// lively each [`STEP`] is: the liveliest first, each with a little around
/// it, those that touch joined, in time order.
fn pick(scores: &[f64], length: f64, wanted: f64) -> Vec<(f64, f64)> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|a, b| scores[*b].total_cmp(&scores[*a]).then(a.cmp(b)));
    let mut kept: Vec<(f64, f64)> = Vec::new();
    let total = |kept: &[(f64, f64)]| kept.iter().map(|(a, b)| b - a).sum::<f64>();
    for index in order {
        if total(&kept) >= wanted - 0.25 {
            break;
        }
        let at = index as f64 * STEP;
        let (from, to) = (
            (at - BEFORE).max(0.0),
            (at + STEP + AFTER).min(length),
        );
        if to - from < 0.5 {
            continue;
        }
        let mut next = kept.clone();
        next.push((from, to));
        next.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut joined: Vec<(f64, f64)> = Vec::new();
        for (a, b) in next {
            match joined.last_mut() {
                // a breath between two moments is kept rather than jumped
                Some(last) if a <= last.1 + 0.6 => last.1 = last.1.max(b),
                _ => joined.push((a, b)),
            }
        }
        // a moment that would run far over is left for a shorter one
        if total(&joined) <= wanted + 1.0 || kept.is_empty() {
            kept = joined;
        }
    }
    kept
}

#[uniffi::export]
impl MobileSession {
    /// Keeps about `wanted_us` of the main track's liveliest moments and
    /// cuts the rest, as one undo step. Errs when the main track has no
    /// sound to go by, or is no longer than that already.
    pub fn auto_highlight(&self, wanted_us: i64) -> Result<TimelineView, MobileError> {
        let wanted = seconds(wanted_us).max(3.0);
        let (length, clips) = self.with_session(|session| {
            let project = session.project();
            let timeline = project.active();
            let Some(main) = timeline.tracks.first() else {
                return Ok((0.0, Vec::new()));
            };
            let mine = || timeline.clips.iter().filter(|clip| clip.track_id == main.id);
            let length = mine().map(|clip| clip.start + clip.duration).fold(0.0, f64::max);
            let clips = mine()
                .filter(|clip| clip.kind == ClipKind::Video)
                .filter_map(|clip| {
                    let media = project.media.iter().find(|item| item.id == clip.media_id)?;
                    (media.has_audio && !matches!(media.kind, MediaKind::Image)).then(|| {
                        (
                            clip.start,
                            clip.duration,
                            clip.speed.max(1e-6),
                            media.path.clone(),
                            clip.source_start,
                            clip.audio_stream.map(|index| index as usize),
                        )
                    })
                })
                .collect::<Vec<_>>();
            Ok((length, clips))
        })?;
        if clips.is_empty() {
            return Err(MobileError::Failed(
                "The video has no sound to find its best moments by.".to_owned(),
            ));
        }
        if length <= wanted + 0.5 {
            return Err(MobileError::Failed(
                "The video is no longer than that already.".to_owned(),
            ));
        }
        // every clip's sound where it plays on the timeline, quiet where there is none
        let mut sound = vec![0.0f32; (length * f64::from(RATE)).ceil() as usize + 1];
        for (start, duration, speed, path, from, stream) in clips {
            let failed = |error: &dyn std::fmt::Display| MobileError::Failed(error.to_string());
            let samples = AudioDecoder::open(
                &path,
                &AudioOptions {
                    start: Some(from),
                    duration: Some((duration * speed).max(0.05)),
                    filters: Vec::new(),
                    rate: RATE,
                    channels: 1,
                    format: SampleFormat::F32,
                    stream,
                    stretch: None,
                },
            )
            .map_err(|error| failed(&error))?
            .collect_f32()
            .map_err(|error| failed(&error))?;
            let first = (start * f64::from(RATE)) as usize;
            let count = (duration * f64::from(RATE)) as usize;
            for i in 0..count {
                let Some(sample) = samples.get((i as f64 * speed) as usize) else {
                    break;
                };
                if let Some(place) = sound.get_mut(first + i) {
                    *place = *sample;
                }
            }
        }
        let kept = pick(&liveliness(&sound), length, wanted);
        let mut cuts = Vec::new();
        let mut at = 0.0;
        for (from, to) in &kept {
            if *from > at + 0.05 {
                cuts.push((at, *from));
            }
            at = *to;
        }
        if length > at + 0.05 {
            cuts.push((at, length));
        }
        if kept.is_empty() || cuts.is_empty() {
            return Err(MobileError::Failed(
                "No moments stood out enough to keep.".to_owned(),
            ));
        }
        self.cut_ranges(&cuts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shout_in_a_quiet_talk_is_the_liveliest() {
        let mut samples: Vec<f32> = (0..RATE as usize * 20)
            .map(|i| 0.02 * (i as f32 * 0.4).sin())
            .collect();
        for sample in &mut samples[RATE as usize * 12..RATE as usize * 13] {
            *sample *= 25.0;
        }
        let scores = liveliness(&samples);
        let best = (0..scores.len())
            .max_by(|a, b| scores[*a].total_cmp(&scores[*b]))
            .expect("some");
        assert!((24..26).contains(&best), "{best}");
        let kept = pick(&scores, 20.0, 5.0);
        assert!(kept.iter().any(|(a, b)| *a < 12.0 && *b > 13.0), "{kept:?}");
    }

    #[test]
    fn about_as_much_is_kept_as_asked_for_in_order() {
        let scores: Vec<f64> = (0..240).map(|i| ((i * 37) % 101) as f64).collect();
        let kept = pick(&scores, 120.0, 30.0);
        let total: f64 = kept.iter().map(|(a, b)| b - a).sum();
        assert!((24.0..=31.0).contains(&total), "{total}");
        assert!(kept.windows(2).all(|pair| pair[0].1 < pair[1].0));
    }
}
