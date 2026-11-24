// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Remix to fit: a song made the length of the video by taking a stretch out
//! of its middle, or playing one again, with both ends of the join on a beat -
//! so it still starts and ends as it was written and the join lands in time.
//!
//! The beats are the ones the timeline marks (`crate::beats`). The song becomes
//! two clips on its own row, the first up to a beat, the second from another
//! beat to the song's end, touching, as one undo step.

use concat_project::Command;
use concat_project::commands::{ClipMove, ClipPatch, TrimEdge};
use concat_project::model::MediaKind;

use crate::MobileError;
use crate::session::{MobileSession, TimelineView};

/// The join is looked for in this middle part of the song, as fractions of it:
/// never in the opening or the ending, which are what make a song sound whole.
const MIDDLE: (f64, f64) = (0.2, 0.8);
/// A song within this of the length asked for is only trimmed.
const CLOSE: f64 = 0.05;
/// How long each side of the join fades, seconds: enough that the cut does
/// not click, too short to hear as a dip.
const JOIN_FADE: f64 = 0.03;

fn failed(message: impl Into<String>) -> MobileError {
    MobileError::Failed(message.into())
}

/// The least of the song each part of a join keeps, seconds of the file.
const EDGE: f64 = 1.0;

/// Where in the song (file seconds, from `source` to `total`) a join that
/// skips `skip` seconds may start: in its middle where it can be, else
/// anywhere both parts keep at least [`EDGE`] of the song. `None` where no
/// join that long fits.
fn window(source: f64, total: f64, skip: f64) -> Option<(f64, f64)> {
    let song = total - source;
    // `at` and `at + skip` both inside the song, an edge from either end.
    let valid = (
        (source + EDGE).max(source + EDGE - skip),
        (total - EDGE).min(total - EDGE - skip),
    );
    if valid.0 >= valid.1 {
        return None;
    }
    let middle = (source + song * MIDDLE.0, source + song * MIDDLE.1);
    let preferred = (valid.0.max(middle.0), valid.1.min(middle.1));
    Some(if preferred.0 < preferred.1 { preferred } else { valid })
}

/// Where to join `beats` (file seconds, sorted) so the part up to `at` jumps
/// to `to`, with `to - at` as near `skip` as the beats allow (negative plays a
/// stretch again): `at` a beat in `window`, `to` a beat inside the song
/// (`bounds`). With no such pair the join falls in the middle of the window,
/// off the beat, which [`window`] has made a place that fits.
fn join(beats: &[f64], skip: f64, window: (f64, f64), bounds: (f64, f64)) -> (f64, f64) {
    let ends: Vec<f64> = beats
        .iter()
        .copied()
        .filter(|b| *b >= bounds.0 && *b <= bounds.1)
        .collect();
    let mut best: Option<(f64, f64, f64)> = None;
    for &at in beats.iter().filter(|b| **b >= window.0 && **b <= window.1) {
        let want = at + skip;
        let Some(&to) = ends.iter().min_by(|a, b| (*a - want).abs().total_cmp(&(*b - want).abs())) else {
            continue;
        };
        if (to - at).signum() != skip.signum() {
            continue;
        }
        let error = (to - at - skip).abs();
        if best.is_none_or(|(e, ..)| error < e) {
            best = Some((error, at, to));
        }
    }
    match best {
        // on the beat, unless the nearest beats are far from the length asked
        Some((error, at, to)) if error <= skip.abs() * 0.1 + 0.5 => (at, to),
        _ => {
            let at = (window.0 + window.1) / 2.0;
            (at, at + skip)
        }
    }
}

#[uniffi::export]
impl MobileSession {
    /// Makes the sound clip `clip_id` last `length_us` by joining its song on
    /// the beat: a stretch of the middle taken out to shorten it, or played
    /// again to lengthen it. One undo step.
    pub fn remix_to_fit(&self, clip_id: String, length_us: i64) -> Result<TimelineView, MobileError> {
        let target = crate::seconds(length_us);
        if target < 1.0 {
            return Err(failed("Make the video at least a second long first."));
        }
        let (path, project, start, duration, source, speed, total, track) = self.with_session(|session| {
            let project = session.project();
            let clip = project
                .active()
                .clips
                .iter()
                .find(|c| c.id == clip_id)
                .ok_or_else(|| failed("That clip is gone."))?;
            if clip.speed_curve.is_some() {
                return Err(failed("Take the speed curve off first."));
            }
            let media = project
                .media
                .iter()
                .find(|m| m.id == clip.media_id)
                .filter(|m| m.has_audio && m.kind != MediaKind::Image)
                .ok_or_else(|| failed("Only a clip with sound can be remixed."))?;
            let total = media.duration.ok_or_else(|| failed("That song's length is not known."))?;
            Ok((
                media.path.clone(),
                session.path().to_owned(),
                clip.start,
                clip.duration,
                clip.source_start,
                clip.speed.max(0.01),
                total,
                clip.track_id.clone(),
            ))
        })?;
        // The song from where the clip starts in it to its end, in timeline seconds.
        let available = (total - source) / speed;
        if available < 2.0 {
            return Err(failed("That song is too short to remix."));
        }
        // Everything is worked out before anything changes: a song that cannot
        // be joined is left as it was.
        let plan = if (available - target).abs() <= CLOSE {
            None
        } else {
            // File seconds to cut out (or, negative, to play again).
            let skip = (available - target) * speed;
            let Some(window) = window(source, total, skip) else {
                return Err(failed(if skip > 0.0 {
                    "The video is too short for any of this song to be kept on both sides of a join."
                } else {
                    "The song is less than half as long as the video: put it on twice first."
                }));
            };
            let beats: Vec<f64> = match crate::beats::detect_beats(
                path.clone(),
                project.clone(),
                crate::micros(source),
                crate::micros(total),
                false,
            ) {
                Ok(beats) => beats.into_iter().map(crate::seconds).collect(),
                Err(error) => {
                    log::warn!("remix: no beats read ({error:?}); joining off the beat");
                    Vec::new()
                }
            };
            let (at, to) = join(&beats, skip, window, (source + EDGE, total - EDGE));
            log::info!(
                "remix: {:.1} s of song to {:.1} s, joining {at:.2} s to {to:.2} s ({} beats)",
                available,
                target,
                beats.len()
            );
            Some((at, to))
        };
        let gesture = self.gesture();
        let mut state = self.lock();
        let result = (|| -> Result<Option<String>, MobileError> {
            // All of the song, to its own ending.
            Self::apply_all(
                &mut state,
                vec![Command::TrimClip {
                    clip_id: clip_id.clone(),
                    edge: TrimEdge::End,
                    delta: available - duration,
                    ripple: false,
                }],
                Some(&gesture),
            )?;
            let Some((at, to)) = plan else {
                // within a moment of the length already: trimmed to it exactly
                Self::apply_all(
                    &mut state,
                    vec![Command::TrimClip {
                        clip_id: clip_id.clone(),
                        edge: TrimEdge::End,
                        delta: target - available,
                        ripple: false,
                    }],
                    Some(&gesture),
                )?;
                return Ok(None);
            };
            let cut = start + (at - source) / speed;
            let tail = Self::apply_all(
                &mut state,
                vec![Command::SplitClips {
                    clip_ids: vec![clip_id.clone()],
                    time: cut,
                }],
                Some(&gesture),
            )?
            .ok_or_else(|| failed("The song could not be cut."))?;
            // The second part's in-point moves from `at` to `to`; then it is
            // put back against the first.
            let length = (total - to) / speed;
            Self::apply_all(
                &mut state,
                vec![
                    Command::TrimClip {
                        clip_id: tail.clone(),
                        edge: TrimEdge::Start,
                        delta: (to - at) / speed,
                        ripple: false,
                    },
                    Command::MoveClips {
                        moves: vec![ClipMove {
                            clip_id: tail.clone(),
                            start: cut,
                            track_id: track.clone(),
                        }],
                    },
                ],
                Some(&gesture),
            )?;
            // The beats rarely give the length to the frame: the ending is
            // trimmed to it, or the song ends a moment early.
            let end = cut + length;
            if end > start + target {
                Self::apply_all(
                    &mut state,
                    vec![Command::TrimClip {
                        clip_id: tail.clone(),
                        edge: TrimEdge::End,
                        delta: start + target - end,
                        ripple: false,
                    }],
                    Some(&gesture),
                )?;
            }
            Self::apply_all(
                &mut state,
                vec![
                    Command::UpdateClip {
                        clip_id: clip_id.clone(),
                        patch: ClipPatch {
                            fade_out: Some(JOIN_FADE),
                            ..ClipPatch::default()
                        },
                    },
                    Command::UpdateClip {
                        clip_id: tail.clone(),
                        patch: ClipPatch {
                            fade_in: Some(JOIN_FADE),
                            ..ClipPatch::default()
                        },
                    },
                ],
                Some(&gesture),
            )?;
            log::info!("remixed: joined {at:.2} s to {to:.2} s of the song");
            Ok(Some(tail))
        })();
        state.session.end_gesture();
        let created = result?;
        Ok(Self::changed(&mut state, created))
    }
}

#[cfg(test)]
mod tests {
    use super::{join, window};

    #[test]
    fn a_join_lands_on_beats_the_skip_apart() {
        let beats: Vec<f64> = (0..100).map(|i| f64::from(i) * 0.5).collect();
        let (at, to) = join(&beats, 10.2, (10.0, 30.0), (1.0, 48.0));
        assert!((to - at - 10.0).abs() < 1e-9 || (to - at - 10.5).abs() < 1e-9);
        assert!(beats.contains(&at) && beats.contains(&to));
    }

    #[test]
    fn a_join_that_must_cut_most_of_the_song_still_fits_inside_it() {
        // 248 s of song for a 78 s video: 170 s out, so the join can only start
        // in the first 77 s.
        let w = window(0.0, 248.0, 170.0).expect("a window");
        assert!(w.0 >= 1.0 && w.1 <= 77.0 && w.0 < w.1, "{w:?}");
        let beats: Vec<f64> = (0..496).map(|i| f64::from(i) * 0.5).collect();
        let (at, to) = join(&beats, 170.0, w, (1.0, 247.0));
        assert!(to < 247.0 && at >= 1.0, "{at} {to}");
        assert!((to - at - 170.0).abs() < 0.6);
        // and with no beats at all, off the beat but inside the song
        let (at, to) = join(&[], 170.0, w, (1.0, 247.0));
        assert!(to < 247.0 && at >= 1.0, "{at} {to}");
    }

    #[test]
    fn a_video_more_than_twice_the_song_is_refused() {
        assert!(window(0.0, 30.0, -40.0).is_none());
    }

    #[test]
    fn a_negative_skip_plays_a_stretch_again() {
        let beats: Vec<f64> = (0..100).map(|i| f64::from(i) * 0.5).collect();
        let (at, to) = join(&beats, -6.0, (10.0, 40.0), (1.0, 48.0));
        assert!(to < at);
        assert!((at - to - 6.0).abs() < 1e-9);
    }
}
