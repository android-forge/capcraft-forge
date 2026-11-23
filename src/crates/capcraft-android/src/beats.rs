// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Beats: the moments a piece of sound hits, found from its waveform, for the
//! timeline's beat marks and for edges to snap to.
//!
//! The waveform peaks the timeline already draws (and caches beside the
//! project) are enough: a hit is a sudden rise in loudness that stands out
//! from the bars around it. Nothing is decoded twice.

use concat_media::peaks::Peaks;
use concat_project::Command;
use concat_project::commands::TrimEdge;

use crate::session::{MobileSession, TimelineView};
use crate::{MobileError, micros, seconds};

/// The shortest a clip is cut to by the beat.
const SHORTEST: f64 = 0.3;

/// The step the loudness is read at: a hundredth of a second.
const FRAME: f64 = 0.01;

/// Where the sound of `path` hits between `from_us` and `to_us` (times in
/// the file, not the timeline). `dense` keeps closer and weaker hits; the
/// sparse reading keeps the strong ones a beat apart. `project` is the
/// project folder, where the waveform is cached.
#[uniffi::export]
pub fn detect_beats(
    path: String,
    project: String,
    from_us: i64,
    to_us: i64,
    dense: bool,
) -> Result<Vec<i64>, MobileError> {
    let peaks = concat_host::media::peaks(
        &path,
        None,
        (!project.is_empty()).then_some(project.as_str()),
    )?;
    let from = seconds(from_us).max(0.0);
    let to = seconds(to_us).max(from);
    Ok(onsets(&envelope(&peaks, from, to), from, dense)
        .into_iter()
        .map(micros)
        .collect())
}

/// The loudest sample of each frame from `from` to `to`, in 0..1.
fn envelope(peaks: &Peaks, from: f64, to: f64) -> Vec<f32> {
    let frames = ((to - from) / FRAME).floor().max(0.0) as usize;
    (0..frames)
        .map(|index| {
            let start = (from + index as f64 * FRAME) as f32;
            let (low, high) = peaks.extremes(start, start + FRAME as f32);
            high.max(-low)
        })
        .collect()
}

/// The times (from `from`, a frame apart) where `level` jumps: each frame's
/// rise over the loudest of the few before it, kept where it is the largest
/// rise nearby and clearly above the rises around it.
pub(crate) fn onsets(level: &[f32], from: f64, dense: bool) -> Vec<f64> {
    // a log scale, so a hit counts the same in a quiet bar as in a loud one
    let loudness: Vec<f32> = level.iter().map(|value| (value + 1e-3).ln()).collect();
    let rise: Vec<f32> = (0..loudness.len())
        .map(|index| {
            let past = loudness[index.saturating_sub(3)..index]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            if past.is_finite() {
                (loudness[index] - past).max(0.0)
            } else {
                0.0
            }
        })
        .collect();
    // frames either side the average rise is taken over, the shortest gap
    // between hits in seconds, and how far above that average a hit stands
    let (window, gap, lift) = if dense {
        (25, 0.18, 1.3)
    } else {
        (50, 0.35, 1.8)
    };
    let mut hits = Vec::new();
    let mut last = f64::NEG_INFINITY;
    for index in 0..rise.len() {
        let around = &rise[index.saturating_sub(window)..(index + window + 1).min(rise.len())];
        let mean = around.iter().sum::<f32>() / around.len() as f32;
        let nearby = &rise[index.saturating_sub(5)..(index + 6).min(rise.len())];
        let peak = nearby.iter().all(|other| *other <= rise[index]);
        let at = from + index as f64 * FRAME;
        if peak && rise[index] > mean * lift + 0.15 && at - last >= gap {
            hits.push(at);
            last = at;
        }
    }
    hits
}

/// What a main-track clip is: where it starts, how long it is, and the
/// most it can be (a video runs out; a picture never does).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Piece {
    length: f64,
    most: f64,
}

/// The lengths that put each cut of `pieces`, laid end to end from `start`,
/// on a beat of `beats` (timeline seconds, sorted): each piece runs to the
/// first beat at least [`SHORTEST`] on, or to its own end when that comes
/// first; past the last beat the pieces are left as they are.
fn beat_lengths(pieces: &[Piece], start: f64, beats: &[f64]) -> Vec<f64> {
    let mut at = start;
    pieces
        .iter()
        .map(|piece| {
            let length = beats
                .iter()
                .find(|&&b| b >= at + SHORTEST - 1e-6)
                .map_or(piece.length, |b| (b - at).min(piece.most));
            at += length;
            length
        })
        .collect()
}

#[uniffi::export]
impl MobileSession {
    /// Cuts the clips on the main track so each change lands on a beat:
    /// `beats_us` are timeline times. One undo step.
    pub fn fit_main_to_beats(&self, beats_us: Vec<i64>) -> Result<TimelineView, MobileError> {
        let mut beats: Vec<f64> = beats_us.into_iter().map(seconds).collect();
        beats.sort_by(f64::total_cmp);
        let (ids, pieces, start) = self.main_pieces()?;
        let lengths = beat_lengths(&pieces, start, &beats);
        self.run(trims(&ids, &pieces, &lengths))
    }

    /// Gives every clip on the main track the same length, `seconds_each`
    /// (a video shorter than that keeps all it has). One undo step.
    pub fn set_main_lengths(&self, seconds_each: f64) -> Result<TimelineView, MobileError> {
        if !(seconds_each.is_finite() && seconds_each >= 0.1) {
            return Err(MobileError::Failed("That length is too short.".to_owned()));
        }
        let (ids, pieces, _) = self.main_pieces()?;
        let lengths: Vec<f64> = pieces.iter().map(|p| seconds_each.min(p.most)).collect();
        self.run(trims(&ids, &pieces, &lengths))
    }
}

impl MobileSession {
    /// The main track's clips in order: their ids, lengths and the most each
    /// can be, and where the first starts.
    fn main_pieces(&self) -> Result<(Vec<String>, Vec<Piece>, f64), MobileError> {
        self.with_session(|session| {
            let project = session.project();
            let timeline = project.active();
            let main = timeline.tracks.first().ok_or_else(|| MobileError::Failed("There is nothing on the timeline.".to_owned()))?;
            let mut clips: Vec<_> = timeline.clips.iter().filter(|c| c.track_id == main.id).collect();
            clips.sort_by(|a, b| a.start.total_cmp(&b.start));
            let start = clips.first().map_or(0.0, |c| c.start);
            let pieces = clips
                .iter()
                .map(|clip| {
                    let media = project.media.iter().find(|m| m.id == clip.media_id);
                    let still = media.is_some_and(|m| m.kind == concat_project::model::MediaKind::Image);
                    let most = match media.and_then(|m| m.duration) {
                        Some(total) if !still && clip.speed > 0.0 => ((total - clip.source_start) / clip.speed).max(0.1),
                        _ => f64::INFINITY,
                    };
                    Piece { length: clip.duration, most }
                })
                .collect();
            Ok((clips.iter().map(|c| c.id.clone()).collect(), pieces, start))
        })
    }
}

/// The trims, in order and rippling, that give `ids` their new `lengths`.
fn trims(ids: &[String], pieces: &[Piece], lengths: &[f64]) -> Vec<Command> {
    ids.iter()
        .zip(pieces.iter().zip(lengths))
        .filter(|(_, (piece, length))| (piece.length - **length).abs() > 1e-4)
        .map(|(id, (piece, length))| Command::TrimClip { clip_id: id.clone(), edge: TrimEdge::End, delta: length - piece.length, ripple: true })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::onsets;

    /// A quiet bed with a sharp hit every half second, each dying away.
    fn clicks(seconds: f64, every: f64) -> Vec<f32> {
        let frames = (seconds / super::FRAME) as usize;
        (0..frames)
            .map(|index| {
                let t = index as f64 * super::FRAME;
                let since = t % every;
                (0.05 + 0.9 * (-since * 20.0).exp()) as f32
            })
            .collect()
    }

    #[test]
    fn finds_regular_hits() {
        let found = onsets(&clicks(4.0, 0.5), 10.0, false);
        // the first frame has no past to rise over; every later hit is found
        assert_eq!(found.len(), 7, "{found:?}");
        for (index, at) in found.iter().enumerate() {
            let wanted = 10.5 + index as f64 * 0.5;
            assert!((at - wanted).abs() < 0.02, "{at} for {wanted}");
        }
    }

    #[test]
    fn steady_sound_has_no_hits() {
        assert!(onsets(&[0.4; 300], 0.0, true).is_empty());
    }

    #[test]
    fn clips_are_cut_on_the_beats_they_reach() {
        use super::{Piece, beat_lengths};
        let photo = |length| Piece { length, most: f64::INFINITY };
        let beats = [0.5, 0.6, 1.0, 1.5, 2.0];
        // the first runs to 0.5; the next skips 0.6 (too soon) for 1.0; a short video stops at its end
        let pieces = [photo(2.0), photo(2.0), Piece { length: 2.0, most: 0.4 }, photo(3.0), photo(3.0)];
        let got = beat_lengths(&pieces, 0.0, &beats);
        assert!(got.iter().zip([0.5, 0.5, 0.4, 0.6, 3.0]).all(|(a, b)| (a - b).abs() < 1e-9), "{got:?}");
    }
}
