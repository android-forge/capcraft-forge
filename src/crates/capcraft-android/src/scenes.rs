// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Split at scene cuts: a video clip cut wherever its shot changes, read from
//! its pictures on the phone by the same shot reader Reel to template uses
//! (`concat_vision::shots`). For a long take, a screen recording, or a video
//! someone else cut, made into pieces that can be moved, trimmed or dropped.

use concat_core::{FrameRate, Rational};
use concat_media::{DecodeOptions, Decoder, FrameSource};
use concat_project::Command;
use concat_vision::shots::{Reader, WIDTH};

use crate::session::{MobileSession, TimelineView};
use crate::MobileError;

/// Pictures a second the shots are read at: a cut is found to a fifteenth of a
/// second, and a long clip is read in seconds.
const RATE: f64 = 15.0;
/// No piece shorter than this is cut off: a flash or a blink is not a scene.
const SHORTEST: f64 = 0.4;

fn failed(message: impl Into<String>) -> MobileError {
    MobileError::Failed(message.into())
}

/// The times, in the file, where the shots of `path` change between `from` and
/// `to` seconds.
fn cuts_in(path: &str, from: f64, to: f64) -> Result<Vec<f64>, MobileError> {
    let info = concat_media::probe(path).map_err(|e| failed(e.to_string()))?;
    let video = info.video.ok_or_else(|| failed("That clip has no picture."))?;
    let tall = (WIDTH as f64 * f64::from(video.height) / f64::from(video.width.max(1))) as usize;
    let tall = (tall / 2 * 2).clamp(24, 240);
    let options = DecodeOptions::default()
        .starting_at(Rational::approximate(from).unwrap_or(Rational::from_int(0)))
        .scaled_to(WIDTH as u32, tall as u32)
        .at_rate(FrameRate::new(Rational::new(RATE as i64, 1)))
        .eight_bit(true)
        .threaded(4);
    let mut decoder = Decoder::open(path, &options).map_err(|e| failed(e.to_string()))?;
    let mut reader = Reader::new(WIDTH, tall, RATE);
    let frames = ((to - from) * RATE).ceil() as usize;
    while reader.len() < frames {
        let Some(frame) = decoder.next_frame().map_err(|e| failed(e.to_string()))? else {
            break;
        };
        if frame.width() as usize != WIDTH || frame.height() as usize != tall {
            return Err(failed("The pictures could not be read small."));
        }
        reader.push(frame.pixels(), frame.bytes_per_pixel());
    }
    if reader.len() < 3 {
        return Ok(Vec::new());
    }
    // A shot's start, past the first, is a cut; the reading counts from `from`.
    Ok(reader
        .finish()
        .shots
        .iter()
        .skip(1)
        .map(|shot| from + shot.start)
        .collect())
}

#[uniffi::export]
impl MobileSession {
    /// Splits the video clip `clip_id` wherever its shot changes, in one undo
    /// step. A clip with no cut in it is left as it was. Not for a clip on a
    /// speed curve, whose file time does not follow the timeline in a line.
    pub fn split_at_scenes(&self, clip_id: String) -> Result<TimelineView, MobileError> {
        let (path, start, duration, source_start, speed) = self.with_session(|session| {
            let project = session.project();
            let clip = project
                .active()
                .clips
                .iter()
                .find(|c| c.id == clip_id)
                .ok_or_else(|| failed("That clip is gone."))?;
            if clip.speed_curve.is_some() {
                return Err(failed("Take the speed curve off first: its cuts would land in the wrong places."));
            }
            let media = project
                .media
                .iter()
                .find(|m| m.id == clip.media_id)
                .filter(|m| m.kind == concat_project::model::MediaKind::Video)
                .ok_or_else(|| failed("Only a video clip has scenes to split."))?;
            Ok((media.path.clone(), clip.start, clip.duration, clip.source_start, clip.speed.max(0.01)))
        })?;
        let from = source_start;
        let to = source_start + duration * speed;
        let cuts = cuts_in(&path, from, to)?;
        // Each cut in the file, where it falls on the timeline; none too near
        // the clip's ends or the cut before it.
        let mut times: Vec<f64> = Vec::new();
        for cut in cuts {
            let at = start + (cut - source_start) / speed;
            let after_last = times.last().map_or(start, |t| *t);
            if at - after_last >= SHORTEST && start + duration - at >= SHORTEST {
                times.push(at);
            }
        }
        if times.is_empty() {
            return Err(failed("No scene changes were found in that clip."));
        }
        // Later cuts first: each split leaves the earlier part with the clip's
        // own id, so every split after it still names the part it cuts.
        let commands: Vec<Command> = times
            .iter()
            .rev()
            .map(|time| Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: *time,
            })
            .collect();
        log::info!("split at {} scene cut(s)", times.len());
        self.run(commands)
    }
}
