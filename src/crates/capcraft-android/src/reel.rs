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
use concat_vision::models::ModelId;
use concat_vision::shots::{Framing, Join, Reader, Reading, WIDTH};
use concat_vision::words::{Words, colours};
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
            let registered = kept.get("slots").and_then(Value::as_object);
            let mut clips: Vec<_> = timeline
                .clips
                .iter()
                .filter(|clip| clip.track_id == main.id || registered.is_some_and(|slots| slots.contains_key(&clip.id)))
                // a copy of another place is filled with it, not on its own
                .filter(|clip| registered.and_then(|slots| slots.get(&clip.id)).and_then(|slot| slot.get("copyOf")).is_none())
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
    /// `clip_id`, filling the frame (or the band the reel's pictures sat in)
    /// for as long as the slot lasts; with `moves`, moving as the reel's
    /// picture did there, and with `match_colours`, in its colours.
    pub fn fill_slot(
        &self,
        clip_id: String,
        path: String,
        moves: bool,
        match_colours: bool,
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
        // a cut-out copy of the person in the old picture goes with it; one is made again from the new
        if let Ok(mut bodies) = serde_json::from_str::<Value>(&self.extra(crate::body::BODY.to_owned()))
            && let Some(copy) = bodies.get(&clip_id).and_then(|b| b.get("copy")).and_then(Value::as_str).map(str::to_owned)
        {
            if let Some(all) = bodies.as_object_mut() {
                all.remove(&clip_id);
            }
            let gone: Vec<Command> = [
                json!({"op": "removeClips", "clipIds": [copy], "ripple": false}),
                json!({"op": "setTimelineExtra", "key": crate::body::BODY, "value": bodies}),
            ]
            .into_iter()
            .filter_map(|c| serde_json::from_value(c).ok())
            .collect();
            self.run(gone)?;
        }
        let replaced = self.replace_media(clip_id.clone(), path.clone())?;
        // a picture of another kind is a new clip; the same kind keeps its own
        let here = Some(replaced.created_id)
            .filter(|id| !id.is_empty() && replaced.rows.iter().any(|row| row.clips.iter().any(|c| &c.id == id)))
            .unwrap_or_else(|| clip_id.clone());

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
        let mut kept = self.reel();
        // the band the reel's pictures sat in, over bars or a blur: all of the frame unless so
        let band = kept
            .get("band")
            .and_then(Value::as_array)
            .and_then(|band| Some((band.first()?.as_f64()?, band.get(1)?.as_f64()?)))
            .filter(|(top, bottom)| bottom - top > 0.1)
            .unwrap_or((0.0, 1.0));
        let cover = if size.0 > 0 && size.1 > 0 && frame.0 > 0 && frame.1 > 0 {
            let (a, b) = (
                f64::from(frame.0) / f64::from(size.0),
                f64::from(frame.1) / f64::from(size.1),
            );
            // fitted is how the engine first shows it; covered fills the band
            (a.max(b * (band.1 - band.0)) / a.min(b)).min(8.0)
        } else {
            1.0
        };
        let middle = (band.0 + band.1) / 2.0 - 0.5;
        let slot = kept
            .get("slots")
            .and_then(|slots| slots.get(&clip_id))
            .cloned()
            .unwrap_or_else(|| json!({}));
        for property in ["scale", "offsetX", "offsetY", "rotation", "opacity"] {
            commands.extend(command(json!({
                "op": "clearClipKeys", "clipId": here, "property": property,
            })));
        }
        // a built-in template's place: its framing, keys and effects
        let base = slot.get("base").cloned();
        let of = |v: &Option<Value>, k: &str, d: f64| v.as_ref().and_then(|b| b.get(k)).and_then(Value::as_f64).unwrap_or(d);
        let sized = if base.as_ref().and_then(|b| b.get("fit")).and_then(Value::as_str) == Some("contain") {
            of(&base, "k", 1.0)
        } else {
            cover * of(&base, "k", 1.0)
        };
        commands.push(Command::SetClipTransform {
            clip_id: here.clone(),
            scale: Some(sized),
            offset_x: Some(of(&base, "x", 0.0)),
            offset_y: Some(if base.is_some() { of(&base, "y", 0.0) } else { middle }),
            rotation: Some(of(&base, "rotation", 0.0)),
            stretch_x: None,
            stretch_y: None,
        });
        if moves {
            for k in slot.get("keys").and_then(Value::as_array).cloned().unwrap_or_default() {
                let (property, at, value) = (
                    k.get(0).and_then(Value::as_str).unwrap_or_default().to_owned(),
                    k.get(1).and_then(Value::as_f64).unwrap_or(0.0),
                    k.get(2).and_then(Value::as_f64).unwrap_or(0.0),
                );
                let value = if property == "scale" { value * sized } else { value };
                commands.extend(command(json!({
                    "op": "setClipKey", "clipId": here, "property": property,
                    "at": at.clamp(0.0, 1.0), "value": value, "ease": [0.16, 1.0, 0.3, 1.0],
                })));
            }
        }
        // a video has moves of its own: only those an editor made are put on it
        let edited = slot.get("edited").and_then(Value::as_bool).unwrap_or(false);
        let path_of = slot
            .get("moves")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if moves && (still || edited) && band == (0.0, 1.0) {
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
        if let Some(blend) = slot.get("blend").and_then(Value::as_str).filter(|b| !b.is_empty()) {
            commands.extend(command(json!({"op": "updateClip", "clipId": here, "patch": {"blend": blend}})));
        }
        if slot.get("flip").and_then(Value::as_bool).unwrap_or(false) {
            commands.extend(command(json!({"op": "updateClip", "clipId": here, "patch": {"flipH": true}})));
        }
        // fast, slow and fast again, about the speed it fills its place at, so it keeps its length
        if moves && !still && length > 0.05 && slot.get("ramp").and_then(Value::as_bool).unwrap_or(false) {
            let mean = (now.min(length) / wanted).clamp(0.1, 1.0);
            commands.push(Command::SetClipSpeedCurve {
                clip_id: here.clone(),
                curve: Some(
                    [(0.0, 1.6), (0.5, 0.4), (1.0, 1.6)]
                        .iter()
                        .map(|(at, k)| concat_project::model::SpeedPoint { at: *at, speed: k * mean, slope: None })
                        .collect(),
                ),
                ripple: false,
            });
        }
        let mut looks: Vec<Value> = slot.get("fx").and_then(Value::as_array).cloned().unwrap_or_default();
        if moves && slot.get("shakes").and_then(Value::as_bool).unwrap_or(false) {
            looks.push(json!({"id": "capcraft.shake", "params": {"amount": 14}}));
        }
        // the colours of the reel's own picture there
        if match_colours
            && let Some(params) = slot.get("ref").and_then(|r| matched(r, &path_now(self, &here)?, still).ok())
        {
            looks.push(json!({"id": "capcraft.color-match", "params": params}));
        }
        commands.extend(command(json!({
            "op": "updateClip", "clipId": here, "patch": {"videoEffects": looks},
        })));
        // its own sound stays out of the reel's
        if !still {
            commands.extend(command(json!({
                "op": "updateClip", "clipId": here, "patch": {"muted": true},
            })));
        }

        // a picture in a video's place is another clip: what was read goes with the place
        let mut copies = Vec::new();
        if let Some(slots) = kept.get_mut("slots").and_then(Value::as_object_mut) {
            let mut slot = slot;
            slot["filled"] = json!(true);
            slots.remove(&clip_id);
            slots.insert(here.clone(), slot);
            // its copies follow it, by its new name when it has one
            for (id, other) in slots.iter_mut() {
                if other.get("copyOf").and_then(Value::as_str) == Some(clip_id.as_str()) {
                    other["copyOf"] = json!(here);
                    copies.push(id.clone());
                }
            }
        }
        let backwards = !still && moves && slot_reverse(&kept, &here);
        commands.push(Command::SetTimelineExtra {
            key: REEL.to_owned(),
            value: Some(kept),
        });
        let mut view = self.run(commands)?;
        // a place that plays backwards, as the template's beat wants; the
        // place is filled all the same when the copy cannot be made
        if backwards {
            view = self.reverse(here).unwrap_or(view);
        }
        // and the places that copy it, each with its own look
        for copy in copies {
            view = self.fill_slot(copy, path.clone(), moves, match_colours)?;
        }
        Ok(view)
    }

    /// Reads the words the reel shows, on a thread of its own (the finder
    /// and reader fetched first, about ten megabytes), and puts each line as
    /// a title where and when it was, in its colour, size and outline.
    pub fn read_reel_words(self: Arc<Self>, listener: Arc<dyn SpeechListener>) -> Result<(), MobileError> {
        let kept = self.reel();
        let path = kept
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MobileError::Failed("This project was not made from a video.".to_owned()))?;
        STOP.store(false, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("capcraft-reel-words".to_owned())
            .spawn(move || {
                let done = (|| {
                    let mut words = {
                        let mut tell = |f: f32| listener.on_progress(f * 0.2, "download".to_owned());
                        let find = crate::objects::fetched(ModelId::WordsFind, &mut tell)?;
                        let read = crate::objects::fetched(ModelId::WordsRead, &mut tell)?;
                        Words::load(&find, &read)?
                    };
                    let cuts: Vec<f64> = kept
                        .get("cuts")
                        .and_then(Value::as_array)
                        .map(|cuts| cuts.iter().filter_map(Value::as_f64).collect())
                        .unwrap_or_default();
                    let titles = read_titles(&path, &cuts, &mut words, &|f| {
                        listener.on_progress(0.2 + f * 0.75, "work".to_owned());
                        !STOP.load(Ordering::Relaxed)
                    })?;
                    Ok::<_, String>(titles)
                })();
                let placed = done.map_err(MobileError::Failed).and_then(|titles| {
                    if titles.is_empty() {
                        return Err(MobileError::Failed("No words were found in the video.".to_owned()));
                    }
                    self.place_titles(&titles)
                });
                match placed {
                    Ok(view) => listener.on_done(Some(view)),
                    Err(error) => listener.on_failed(error.to_string()),
                }
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
    }

    /// Puts the reel's sound on two rows, on a thread of its own: its voice
    /// (as Isolate voice finds it) and the rest, its music, with the voice
    /// taken out of it.
    pub fn separate_reel_sound(self: Arc<Self>, listener: Arc<dyn SpeechListener>) -> Result<(), MobileError> {
        let kept = self.reel();
        let path = kept
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MobileError::Failed("This project was not made from a video.".to_owned()))?;
        let sound = self
            .with_session(|session| {
                let project = session.project();
                Ok(project
                    .active()
                    .clips
                    .iter()
                    .filter(|clip| clip.kind == concat_project::model::ClipKind::Audio)
                    .find(|clip| project.media.iter().any(|item| item.id == clip.media_id && item.path == path))
                    .map(|clip| (clip.id.clone(), clip.start)))
            })?
            .ok_or_else(|| MobileError::Failed("The video's sound is no longer on the timeline.".to_owned()))?;
        let folder = self.with_session(|session| Ok(std::path::PathBuf::from(session.path()).join("media")))?;
        STOP.store(false, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("capcraft-reel-sound".to_owned())
            .spawn(move || {
                let made = (|| {
                    let model = crate::voice::model(&mut |f| listener.on_progress(f * 0.2, "download".to_owned()))?;
                    separate(&path, &model, &folder, &|f| {
                        listener.on_progress(0.2 + f * 0.75, "work".to_owned());
                        !STOP.load(Ordering::Relaxed)
                    })
                })();
                let placed = made.map_err(MobileError::Failed).and_then(|(music, voice)| {
                    self.replace_media(sound.0.clone(), music.to_string_lossy().into_owned())?;
                    let speech = self.import_sound(voice.to_string_lossy().into_owned())?;
                    self.add_audio(speech.id, micros(sound.1))?;
                    let mut kept = self.reel();
                    kept["separated"] = json!(true);
                    self.run(vec![Command::SetTimelineExtra { key: REEL.to_owned(), value: Some(kept) }])
                });
                match placed {
                    Ok(view) => listener.on_done(Some(view)),
                    Err(error) => listener.on_failed(error.to_string()),
                }
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
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

/// The file clip `id` shows.
fn path_now(session: &MobileSession, id: &str) -> Option<String> {
    session
        .with_session(|s| {
            let project = s.project();
            Ok(project
                .active()
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .and_then(|clip| project.media.iter().find(|item| item.id == clip.media_id))
                .map(|item| item.path.clone()))
        })
        .ok()
        .flatten()
}

/// The colour match that gives the picture in `path` the colours of the
/// reel's picture `reference` (`[file, seconds]`), with its strength.
fn matched(reference: &Value, path: &str, still: bool) -> Result<Value, MobileError> {
    let (file, at) = (
        reference.get(0).and_then(Value::as_str).unwrap_or_default(),
        reference.get(1).and_then(Value::as_f64).unwrap_or(0.0),
    );
    let to = concat_host::media::still_at(file, at, 160)?;
    let own = if still {
        0.0
    } else {
        concat_host::media::probe(path)?.to_new_media().duration.unwrap_or(0.0) / 2.0
    };
    let from = concat_host::media::still_at(path, own, 160)?;
    let params = crate::colormatch::color_match(crate::Picture::of(to), crate::Picture::of(from))?;
    let mut params: Value = serde_json::from_str(&params).map_err(|error| MobileError::Failed(error.to_string()))?;
    params["strength"] = json!(80);
    Ok(params)
}

/// A line of words the reel showed: what, from when to when, where
/// (fractions of the frame: left, top, right, bottom), its colour and outline.
struct Title {
    text: String,
    start: f64,
    end: f64,
    rect: [f64; 4],
    ink: String,
    outline: Option<String>,
}

/// The lines of words the video in `path` shows, read twice a second and
/// followed while they stay.
///
/// Words that are part of what was filmed - a sign, a shirt - stay with their
/// shot; words put over a video stay as the shots change under them. With
/// cuts in `cuts`, only those that stay over one are kept.
fn read_titles(path: &str, cuts: &[f64], words: &mut Words, step: &dyn Fn(f32) -> bool) -> Result<Vec<Title>, String> {
    const EVERY: f64 = 0.5;
    let info = concat_media::probe(path).map_err(|error| error.to_string())?;
    let video = info.video.as_ref().ok_or("That file has no picture.")?;
    let seconds = info.duration.map_or(0.0, |d| d.as_f64());
    let (w, h) = (f64::from(video.width.max(1)), f64::from(video.height.max(1)));
    let k = (720.0 / w.max(h)).min(1.0);
    let options = DecodeOptions::default()
        .scaled_to(((w * k) as u32 / 2 * 2).max(2), ((h * k) as u32 / 2 * 2).max(2))
        .at_rate(FrameRate::new(Rational::new(2, 1)))
        .eight_bit(true)
        .threaded(4);
    let mut decoder = Decoder::open(path, &options).map_err(|error| error.to_string())?;
    // each title as it is followed: its box, when, and every way it was read
    struct Track {
        rect: [f64; 4],
        start: f64,
        end: f64,
        last: f64,
        read: Vec<(String, u32)>,
        ink: String,
        outline: Option<String>,
    }
    let mut tracks: Vec<Track> = Vec::new();
    let mut at = 0.0;
    let wanted = (seconds / EVERY).max(1.0);
    let mut seen = 0.0;
    while let Some(frame) = decoder.next_frame().map_err(|error| error.to_string())? {
        let (fw, fh) = (f64::from(frame.width()), f64::from(frame.height()));
        for line in words.lines(&frame)? {
            if line.sure < 0.75 || !line.text.chars().any(char::is_alphanumeric) {
                continue;
            }
            let r = line.rect;
            let rect = [
                f64::from(r.x) / fw,
                f64::from(r.y) / fh,
                f64::from(r.x + r.width) / fw,
                f64::from(r.y + r.height) / fh,
            ];
            // the same title read again, a letter or two otherwise at times, after a gap of a second at most
            let same = tracks.iter_mut().find(|track| {
                at - track.last <= 1.01
                    && at > track.last
                    && overlap(&track.rect, &rect) > 0.4
                    && track.read.iter().any(|(said, _)| alike(said, &line.text))
            });
            match same {
                Some(track) => {
                    track.end = at + EVERY / 2.0;
                    track.last = at;
                    match track.read.iter_mut().find(|(said, _)| *said == line.text) {
                        Some((_, count)) => *count += 1,
                        None => track.read.push((line.text, 1)),
                    }
                }
                None => {
                    let (ink, outline) = colours(&frame, r);
                    tracks.push(Track {
                        rect,
                        start: (at - EVERY / 2.0).max(0.0),
                        end: at + EVERY / 2.0,
                        last: at,
                        read: vec![(line.text, 1)],
                        ink,
                        outline,
                    });
                }
            }
        }
        at += EVERY;
        seen += 1.0;
        if !step((seen / wanted).min(1.0) as f32) {
            return Err("Cancelled.".to_owned());
        }
    }
    // readings of the same words in the same place, broken off for a while, are one title
    tracks.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut joined: Vec<Track> = Vec::new();
    for track in tracks {
        let words = track.read.iter().max_by_key(|(_, count)| *count).map(|(said, _)| said.clone()).unwrap_or_default();
        let earlier = joined.iter_mut().find(|other| {
            track.start - other.end <= 1.6
                && overlap(&other.rect, &track.rect) > 0.4
                && other.read.iter().any(|(said, _)| alike(said, &words))
        });
        match earlier {
            Some(other) => {
                other.end = other.end.max(track.end);
                other.last = other.last.max(track.last);
                for (said, count) in track.read {
                    match other.read.iter_mut().find(|(have, _)| *have == said) {
                        Some((_, have)) => *have += count,
                        None => other.read.push((said, count)),
                    }
                }
            }
            None => joined.push(track),
        }
    }
    Ok(joined
        .into_iter()
        .filter(|t| cuts.is_empty() || cuts.iter().any(|cut| t.start < cut - 0.2 && t.end > cut + 0.2))
        .filter(|t| t.end - t.start >= 0.5)
        .map(|t| Title {
            // the way it was read most often
            text: t.read.iter().max_by_key(|(_, count)| *count).map(|(said, _)| said.clone()).unwrap_or_default(),
            start: t.start,
            end: t.end,
            rect: t.rect,
            ink: t.ink,
            outline: t.outline,
        })
        .collect())
}

/// Whether two readings are of the same words: alike but for a letter in
/// four.
fn alike(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut last = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let here = row[j];
            row[j] = (row[j] + 1).min(row[j - 1] + 1).min(last + usize::from(a[i - 1] != b[j - 1]));
            last = here;
        }
    }
    row[b.len()] * 4 <= a.len().max(b.len())
}

fn overlap(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let area = |r: &[f64; 4]| (r[2] - r[0]) * (r[3] - r[1]);
    w * h / area(a).min(area(b)).max(1e-9)
}

/// The sound of `path` as its voice and the rest, written to `folder`: the
/// rest first.
fn separate(path: &str, model: &std::path::Path, folder: &std::path::Path, step: &dyn Fn(f32) -> bool) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    use concat_media::{AudioDecoder, AudioOptions, SampleFormat};
    use sherpa_onnx::{OfflineSpeechDenoiser, OfflineSpeechDenoiserConfig, OfflineSpeechDenoiserGtcrnModelConfig, OfflineSpeechDenoiserModelConfig};
    const RATE: u32 = 44_100;
    let denoiser = OfflineSpeechDenoiser::create(&OfflineSpeechDenoiserConfig {
        model: OfflineSpeechDenoiserModelConfig {
            gtcrn: OfflineSpeechDenoiserGtcrnModelConfig { model: Some(model.to_string_lossy().into_owned()) },
            num_threads: 2,
            ..Default::default()
        },
    })
    .ok_or("The voice model would not load.")?;
    let read = |rate: u32, channels: u16| -> Result<Vec<f32>, String> {
        AudioDecoder::open(
            path,
            &AudioOptions { start: None, duration: None, filters: Vec::new(), rate, channels, format: SampleFormat::F32, stream: None, stretch: None },
        )
        .and_then(|mut d| d.collect_f32())
        .map_err(|error| error.to_string())
    };
    let low = denoiser.sample_rate().max(8_000) as u32;
    let heard = read(low, 1)?;
    if heard.is_empty() {
        return Err("The video has no sound.".to_owned());
    }
    let mut voice = Vec::with_capacity(heard.len());
    let chunk = low as usize * 30;
    for (i, part) in heard.chunks(chunk).enumerate() {
        voice.extend_from_slice(&denoiser.run(part, low as i32).samples);
        if !step(((i + 1) * chunk).min(heard.len()) as f32 / heard.len() as f32 * 0.7) {
            return Err("Cancelled.".to_owned());
        }
    }
    let whole = read(RATE, 2)?;
    let frames = whole.len() / 2;
    // the voice at the music's rate
    let ratio = f64::from(low) / f64::from(RATE);
    let at = |i: usize| -> f32 {
        let t = i as f64 * ratio;
        let k = t as usize;
        let f = (t - k as f64) as f32;
        let a = voice.get(k).copied().unwrap_or(0.0);
        let b = voice.get(k + 1).copied().unwrap_or(a);
        a + (b - a) * f
    };
    let voiced: Vec<f32> = (0..frames).map(at).collect();
    // how much of the voice each stretch holds, taken out of both sides
    let mut music = whole.clone();
    let block = 2048;
    let mut gain = 0f32;
    for start in (0..frames).step_by(block) {
        let end = (start + block).min(frames);
        let (mut mv, mut vv) = (0f32, 0f32);
        for i in start..end {
            let mono = (whole[i * 2] + whole[i * 2 + 1]) / 2.0;
            mv += mono * voiced[i];
            vv += voiced[i] * voiced[i];
        }
        let wanted = if vv > 1e-6 { (mv / vv).clamp(0.0, 1.2) } else { 0.0 };
        gain = gain * 0.6 + wanted * 0.4;
        for i in start..end {
            music[i * 2] -= gain * voiced[i];
            music[i * 2 + 1] -= gain * voiced[i];
        }
    }
    if !step(0.9) {
        return Err("Cancelled.".to_owned());
    }
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let music_file = folder.join(format!("reel-music-{stamp}.wav"));
    let voice_file = folder.join(format!("reel-voice-{stamp}.wav"));
    wav(&music_file, &music, RATE, 2)?;
    wav(&voice_file, &voiced, RATE, 1)?;
    step(1.0);
    Ok((music_file, voice_file))
}

/// `samples` (-1 to 1, the channels taking turns) as a sixteen-bit WAV file.
fn wav(path: &std::path::Path, samples: &[f32], rate: u32, channels: u16) -> Result<(), String> {
    use std::io::Write;
    let data = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2 * u32::from(channels)).to_le_bytes());
    out.extend_from_slice(&(2 * channels).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    std::fs::File::create(path)
        .and_then(|mut file| file.write_all(&out))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn command(value: Value) -> Option<Command> {
    serde_json::from_value(value).ok()
}

impl MobileSession {
    /// Puts each of `titles` as a title, and keeps that the reel's words have
    /// been put.
    fn place_titles(&self, titles: &[Title]) -> Result<TimelineView, MobileError> {
        let mut view = self.view();
        for title in titles {
            let [left, top, right, bottom] = title.rect;
            let mut style = json!({
                "content": title.text,
                "fontSize": ((bottom - top) * 0.62).clamp(0.015, 0.12),
                "fontWeight": 800.0,
                "color": title.ink,
                "shadow": false,
            });
            if let Some(outline) = &title.outline {
                style["strokeColor"] = json!(outline);
                style["strokeWidth"] = json!(((bottom - top) * 0.06).clamp(0.001, 0.008));
            }
            view = self.add_text(micros(title.start), micros((title.end - title.start).max(0.3)), style.to_string())?;
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
        let mut kept = self.reel();
        if kept.is_object() {
            kept["worded"] = json!(true);
            view = self.run(vec![Command::SetTimelineExtra { key: REEL.to_owned(), value: Some(kept) }])?;
        }
        Ok(view)
    }

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
        // its sound taken out to a row of its own, under the whole, before the shots are cut
        let sound = concat_host::media::probe(path)
            .map(|summary| summary.to_new_media().has_audio)
            .unwrap_or(false);
        if sound {
            let whole = self.main_at(0.01)?;
            self.extract_audio(whole)?;
        }
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
                json!({
                    "moves": places, "edited": shot.edited, "shakes": shot.shakes, "filled": false,
                    "ref": [path, (shot.start + shot.end) / 2.0],
                }),
            );
        }
        let held: Vec<Value> = reading
            .held
            .iter()
            .map(|one| json!([one.start, one.end, one.left, one.top, one.right, one.bottom]))
            .collect();
        let band = match reading.framing {
            Framing::Full => Value::Null,
            Framing::Bars { top, bottom } | Framing::Blurred { top, bottom } => json!([top, bottom]),
        };
        commands.push(Command::SetTimelineExtra {
            key: REEL.to_owned(),
            value: Some(json!({
                "slots": slots, "held": held, "worded": false, "band": band, "path": path,
                "cuts": reading.shots.iter().skip(1).map(|shot| shot.start).collect::<Vec<_>>(),
            })),
        });

        self.run(commands)?;
        // its pictures over a blur of themselves: so will the ones put in
        if matches!(reading.framing, Framing::Blurred { .. }) {
            self.set_background(json!({"kind": "blur", "level": 3}).to_string())?;
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

/// Whether the place `clip` of a kept reel plays its video backwards.
fn slot_reverse(kept: &Value, clip: &str) -> bool {
    kept.get("slots").and_then(|slots| slots.get(clip)).and_then(|slot| slot.get("reverse")).and_then(Value::as_bool).unwrap_or(false)
}
