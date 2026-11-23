// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! A template from a finished video: how it was cut is read from its pictures
//! (`concat_vision::shots`), laid out as a project of its own shots, and other
//! pictures then go in their places, moving as theirs did. All on the phone.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use concat_core::{FrameRate, Rational};
use concat_media::{DecodeOptions, Decoder, FrameSource};
use concat_project::Command;
use concat_project::commands::TrimEdge;
use concat_vision::shots::{Join, Reader, Reading, WIDTH};
use serde_json::{Value, json};

use crate::session::{MobileSession, TimelineView};
use crate::speech::SpeechListener;
use crate::{MobileError, lanes, micros};

/// Where the timeline keeps what was read: how each slot's picture moved, and
/// where there were words.
const REEL: &str = "capcraftReel";

/// Longer than this is not a reel, and would take long to read.
const LONGEST: f64 = 300.0;

static STOP: AtomicBool = AtomicBool::new(false);

/// Asks the reading in hand to stop.
#[uniffi::export]
pub fn cancel_reel() {
    STOP.store(true, Ordering::Relaxed);
}

/// A place a picture goes in a template.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Slot {
    /// Its clip.
    pub clip_id: String,
    /// When it comes on, microseconds.
    pub start_us: i64,
    /// How long it stays.
    pub duration_us: i64,
    /// Whether its picture moves in the reel.
    pub moves: bool,
    /// Whether the user has put a picture of their own there.
    pub filled: bool,
    /// The file shown there now.
    pub path: String,
    /// Where in the file it starts, microseconds.
    pub source_us: i64,
}

/// The transition a join is laid as.
fn transition(join: Join) -> Option<&'static str> {
    match join {
        Join::Cut => None,
        Join::Dissolve => Some("concat.dissolve"),
        Join::Black => Some("capcraft.fade-black"),
        Join::White => Some("capcraft.fade-white"),
        Join::Whip => Some("concat.whip-pan"),
        Join::Zoom => Some("concat.zoom-blur-transition"),
        Join::Blur => Some("concat.blur-dissolve"),
        Join::Push => Some("concat.push"),
        Join::Wipe => Some("concat.linear-wipe"),
    }
}

/// Reads `path`, telling `step` how far it is.
fn read(path: &str, step: &dyn Fn(f32) -> bool) -> Result<(Reading, f64), String> {
    let info = concat_media::probe(path).map_err(|error| error.to_string())?;
    let video = info
        .video
        .as_ref()
        .ok_or_else(|| "That file has no picture.".to_owned())?;
    let seconds = info.duration.map_or(0.0, |d| d.as_f64());
    if seconds > LONGEST {
        return Err("That video is longer than five minutes.".to_owned());
    }
    if seconds < 0.5 {
        return Err("That video is too short.".to_owned());
    }
    let own = video.frame_rate.fps().as_f64();
    // every picture of a short one; a long one's a little apart
    let fps = own
        .clamp(10.0, if seconds > 90.0 { 15.0 } else { 30.0 })
        .round();
    let tall = (WIDTH as f64 * f64::from(video.height) / f64::from(video.width.max(1))) as usize;
    let tall = (tall / 2 * 2).clamp(24, 240);
    let options = DecodeOptions::default()
        .scaled_to(WIDTH as u32, tall as u32)
        .at_rate(FrameRate::new(Rational::new(fps as i64, 1)))
        .eight_bit(true)
        .threaded(4);
    let mut decoder = Decoder::open(path, &options).map_err(|error| error.to_string())?;
    let mut reader = Reader::new(WIDTH, tall, fps);
    let expected = (seconds * fps).max(1.0);
    while let Some(frame) = decoder.next_frame().map_err(|error| error.to_string())? {
        if frame.width() as usize != WIDTH || frame.height() as usize != tall {
            return Err("The pictures could not be read small.".to_owned());
        }
        reader.push(frame.pixels(), frame.bytes_per_pixel());
        if reader.len() % 10 == 0 && !step((reader.len() as f64 / expected * 0.7).min(0.7) as f32) {
            return Err("Cancelled.".to_owned());
        }
    }
    if reader.len() < 5 {
        return Err("That video has no pictures to read.".to_owned());
    }
    Ok((reader.finish(), seconds))
}

#[uniffi::export]
impl MobileSession {
    /// Lays the video in `path` (in this project's media) out as a template,
    /// on a thread of its own: its shots on the main track with the
    /// transitions between them, its flashes, and its sound on a row of its
    /// own.
    pub fn make_from_reel(
        self: Arc<Self>,
        path: String,
        listener: Arc<dyn SpeechListener>,
    ) -> Result<(), MobileError> {
        STOP.store(false, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("capcraft-reel".to_owned())
            .spawn(move || {
                let step = |fraction: f32| {
                    listener.on_progress(fraction, "work".to_owned());
                    !STOP.load(Ordering::Relaxed)
                };
                let laid = read(&path, &step).map_err(MobileError::Failed).and_then(
                    |(reading, seconds)| {
                        listener.on_progress(0.9, "work".to_owned());
                        self.lay_reel(&path, &reading, seconds)
                    },
                );
                match laid {
                    Ok(view) => listener.on_done(Some(view)),
                    Err(error) => listener.on_failed(error.to_string()),
                }
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
    }

    /// The places on the main track a picture can go, in order.
    pub fn slots(&self) -> Vec<Slot> {
        let kept = self.reel();
        self.with_session(|session| {
            let project = session.project();
            let timeline = project.active();
            let Some(main) = timeline.tracks.first() else {
                return Ok(Vec::new());
            };
            let mut clips: Vec<_> = timeline
                .clips
                .iter()
                .filter(|clip| clip.track_id == main.id)
                .collect();
            clips.sort_by(|a, b| a.start.total_cmp(&b.start));
            Ok(clips
                .into_iter()
                .map(|clip| {
                    let slot = kept.get("slots").and_then(|slots| slots.get(&clip.id));
                    Slot {
                        clip_id: clip.id.clone(),
                        start_us: micros(clip.start),
                        duration_us: micros(clip.duration),
                        moves: slot
                            .and_then(|slot| slot.get("moves"))
                            .and_then(Value::as_array)
                            .is_some_and(|moves| !moves.is_empty()),
                        filled: slot
                            .and_then(|slot| slot.get("filled"))
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        path: project
                            .media
                            .iter()
                            .find(|item| item.id == clip.media_id)
                            .map(|item| item.path.clone())
                            .unwrap_or_default(),
                        source_us: micros(clip.source_start),
                    }
                })
                .collect())
        })
        .unwrap_or_default()
    }

    /// Puts the picture or video in `path` (in the project's media) in slot
    /// `clip_id`, filling the frame for as long as the slot lasts; with
    /// `moves`, moving as the reel's picture did there.
    pub fn fill_slot(
        &self,
        clip_id: String,
        path: String,
        moves: bool,
    ) -> Result<TimelineView, MobileError> {
        let summary = concat_host::media::probe(&path)?;
        let item = summary.to_new_media();
        let still = item.kind == concat_project::model::MediaKind::Image;
        let length = item.duration.unwrap_or(0.0);
        let size = (item.width.unwrap_or(0), item.height.unwrap_or(0));
        let (wanted, frame) = self.with_session(|session| {
            let timeline = session.project().active();
            let clip = timeline
                .clips
                .iter()
                .find(|clip| clip.id == clip_id)
                .ok_or_else(|| MobileError::Failed("That place is gone.".to_owned()))?;
            let settings = session.settings();
            Ok((clip.duration, (settings.width, settings.height)))
        })?;
        let replaced = self.replace_media(clip_id.clone(), path)?;
        let here = if replaced.created_id.is_empty() {
            clip_id.clone()
        } else {
            replaced.created_id
        };

        // a video shorter than its place is slowed to fill it
        let (now, speed) = self.with_session(|session| {
            session
                .project()
                .active()
                .clips
                .iter()
                .find(|clip| clip.id == here)
                .map(|clip| (clip.duration, clip.speed))
                .ok_or_else(|| MobileError::Failed("That place is gone.".to_owned()))
        })?;
        let mut commands = Vec::new();
        if !still && length > 0.05 {
            let covers = now.min(length);
            if (speed - 1.0).abs() > 1e-6 {
                commands.push(Command::SetClipSpeed {
                    clip_id: here.clone(),
                    speed: 1.0,
                    ripple: true,
                });
            }
            if covers < now - 1e-3 {
                commands.push(Command::TrimClip {
                    clip_id: here.clone(),
                    edge: TrimEdge::End,
                    delta: covers - now,
                    ripple: true,
                });
            }
            if covers < wanted - 1e-3 {
                commands.push(Command::SetClipSpeed {
                    clip_id: here.clone(),
                    speed: (covers / wanted).max(0.1),
                    ripple: true,
                });
            }
        }

        // the frame filled, whatever the picture's shape
        let cover = if size.0 > 0 && size.1 > 0 && frame.0 > 0 && frame.1 > 0 {
            let (a, b) = (
                f64::from(frame.0) / f64::from(size.0),
                f64::from(frame.1) / f64::from(size.1),
            );
            (a.max(b) / a.min(b)).min(8.0)
        } else {
            1.0
        };
        let mut kept = self.reel();
        let slot = kept
            .get("slots")
            .and_then(|slots| slots.get(&clip_id))
            .cloned()
            .unwrap_or_else(|| json!({}));
        for property in ["scale", "offsetX", "offsetY"] {
            commands.extend(command(json!({
                "op": "clearClipKeys", "clipId": here, "property": property,
            })));
        }
        commands.push(Command::SetClipTransform {
            clip_id: here.clone(),
            scale: Some(cover),
            offset_x: Some(0.0),
            offset_y: Some(0.0),
            rotation: None,
            stretch_x: None,
            stretch_y: None,
        });
        // a video has moves of its own: only those an editor made are put on it
        let edited = slot.get("edited").and_then(Value::as_bool).unwrap_or(false);
        let path_of = slot
            .get("moves")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if moves && (still || edited) {
            for place in &path_of {
                let of = |i: usize| place.get(i).and_then(Value::as_f64).unwrap_or(0.0);
                for (property, value) in [
                    ("scale", of(1).max(1.0) * cover),
                    ("offsetX", of(2)),
                    ("offsetY", of(3)),
                ] {
                    commands.extend(command(json!({
                        "op": "setClipKey", "clipId": here, "property": property,
                        "at": of(0).clamp(0.0, 1.0), "value": value,
                    })));
                }
            }
        }
        if moves && slot.get("shakes").and_then(Value::as_bool).unwrap_or(false) {
            commands.extend(command(json!({
                "op": "updateClip", "clipId": here,
                "patch": {"videoEffects": [{"id": "capcraft.shake", "params": {"amount": 14}}]},
            })));
        }
        // its own sound stays out of the reel's
        if !still {
            commands.extend(command(json!({
                "op": "updateClip", "clipId": here, "patch": {"muted": true},
            })));
        }

        // a picture in a video's place is another clip: what was read goes with the place
        if let Some(slots) = kept.get_mut("slots").and_then(Value::as_object_mut) {
            let mut slot = slot;
            slot["filled"] = json!(true);
            slots.remove(&clip_id);
            slots.insert(here, slot);
        }
        commands.push(Command::SetTimelineExtra {
            key: REEL.to_owned(),
            value: Some(kept),
        });
        self.run(commands)
    }

    /// Whether the reel had words or a mark over its cuts that titles have
    /// not been put for.
    pub fn reel_words(&self) -> u32 {
        let kept = self.reel();
        if kept.get("worded").and_then(Value::as_bool).unwrap_or(false) {
            return 0;
        }
        kept.get("held")
            .and_then(Value::as_array)
            .map_or(0, |held| held.len() as u32)
    }

    /// Puts a title wherever, and whenever, the reel had words or a mark
    /// over its cuts.
    pub fn add_reel_words(&self) -> Result<TimelineView, MobileError> {
        let mut kept = self.reel();
        let held = kept
            .get("held")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut view = self.view();
        for one in &held {
            let of = |i: usize| one.get(i).and_then(Value::as_f64).unwrap_or(0.0);
            let (start, end, left, top, right, bottom) = (of(0), of(1), of(2), of(3), of(4), of(5));
            let style = json!({
                "content": "Your words",
                "fontSize": ((bottom - top) * 0.5).clamp(0.025, 0.09),
                "fontWeight": 700.0,
                "color": "#ffffff",
            });
            view = self.add_text(micros(start), micros(end - start), style.to_string())?;
            let made = view.created_id.clone();
            if made.is_empty() {
                continue;
            }
            view = self.run(vec![Command::SetClipTransform {
                clip_id: made,
                scale: None,
                offset_x: Some((left + right) / 2.0 - 0.5),
                offset_y: Some((top + bottom) / 2.0 - 0.5),
                rotation: None,
                stretch_x: None,
                stretch_y: None,
            }])?;
        }
        if kept.is_object() {
            kept["worded"] = json!(true);
            view = self.run(vec![Command::SetTimelineExtra {
                key: REEL.to_owned(),
                value: Some(kept),
            }])?;
        }
        Ok(view)
    }
}

fn command(value: Value) -> Option<Command> {
    serde_json::from_value(value).ok()
}

impl MobileSession {
    /// What was read of the reel this timeline was made from, or an empty
    /// object.
    fn reel(&self) -> Value {
        serde_json::from_str::<Value>(&self.extra(REEL.to_owned()))
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}))
    }

    /// Lays `reading` of the video in `path` out on this timeline.
    fn lay_reel(
        &self,
        path: &str,
        reading: &Reading,
        seconds: f64,
    ) -> Result<TimelineView, MobileError> {
        let media = self.import(path.to_owned())?;
        self.add_to_main(vec![media.id.clone()], 0)?;
        // the shots, and the cuts between them no nearer than the engine splits
        let starts: Vec<f64> = reading
            .shots
            .iter()
            .skip(1)
            .map(|shot| shot.start)
            .filter(|start| *start > 0.06 && *start < seconds - 0.06)
            .collect();
        for start in &starts {
            let clip = self.main_at(*start)?;
            self.run(vec![Command::SplitClips {
                clip_ids: vec![clip],
                time: *start,
            }])?;
        }

        let mut slots = serde_json::Map::new();
        let mut commands = Vec::new();
        for shot in &reading.shots {
            let Ok(clip) = self.main_at((shot.start + shot.end) / 2.0) else {
                continue;
            };
            if let Some(id) = transition(shot.join)
                && shot.start > 0.06
            {
                // no longer than the shorter of the shots it is between bears
                let most = (shot.end - shot.start).min(
                    reading
                        .shots
                        .iter()
                        .find(|before| (before.end - shot.start).abs() < 1e-6)
                        .map_or(1.0, |before| before.end - before.start),
                ) * 0.9;
                commands.extend(command(json!({
                    "op": "updateClip", "clipId": clip,
                    "patch": {"transitionIn": {"id": id, "duration": shot.join_seconds.min(most).max(0.1)}},
                })));
            }
            let places: Vec<Value> = shot
                .moves
                .iter()
                .map(|place| json!([place.at, place.scale, place.x, place.y]))
                .collect();
            slots.insert(
                clip,
                json!({"moves": places, "edited": shot.edited, "shakes": shot.shakes, "filled": false}),
            );
        }
        let held: Vec<Value> = reading
            .held
            .iter()
            .map(|one| json!([one.start, one.end, one.left, one.top, one.right, one.bottom]))
            .collect();
        commands.push(Command::SetTimelineExtra {
            key: REEL.to_owned(),
            value: Some(json!({"slots": slots, "held": held, "worded": false})),
        });

        // the sound on a row of its own, under the whole, and the shots silent
        let sound = concat_host::media::probe(path)
            .map(|summary| summary.to_new_media().has_audio)
            .unwrap_or(false);
        if sound {
            commands.extend(reading.shots.iter().filter_map(|shot| {
                let clip = self.main_at((shot.start + shot.end) / 2.0).ok()?;
                command(json!({"op": "updateClip", "clipId": clip, "patch": {"muted": true}}))
            }));
        }
        self.run(commands)?;
        if sound {
            let music = self.import_sound(path.to_owned())?;
            self.add_audio(music.id, 0)?;
        }
        for at in &reading.flashes {
            self.add_layer(
                "capcraft.flash".to_owned(),
                "Flash".to_owned(),
                micros(*at),
                300_000,
            )?;
        }
        Ok(self.view())
    }

    /// The main track's clip at `time`.
    fn main_at(&self, time: f64) -> Result<String, MobileError> {
        self.with_session(|session| {
            let timeline = session.project().active();
            timeline
                .clips
                .iter()
                .find(|clip| {
                    lanes::on_main(timeline, &clip.id)
                        && time >= clip.start
                        && time < clip.start + clip.duration
                })
                .map(|clip| clip.id.clone())
                .ok_or_else(|| MobileError::Failed("There is no shot there.".to_owned()))
        })
    }
}
