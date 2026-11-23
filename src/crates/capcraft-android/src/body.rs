// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Body effects: a look that treats the person in a clip apart from what
//! is behind them. A copy of the clip goes on an overlay lane right above
//! it with its background taken off by the segmentation model; the clip
//! under it gets the background's look (blurred, grey, darkened), the copy
//! the person's (an outline). The timeline keeps which copy belongs to
//! which clip and what was added (`capcraftBody`), so the effect can be
//! changed or taken off again, each in one undo step.

use std::sync::Arc;

use concat_project::Command;
use concat_project::model::{Cutout, CutoutMode, Subject};
use serde_json::{Value, json};

use crate::MobileError;
use crate::lanes::{self, RowKind};
use crate::session::{MobileSession, State, copy_look};
use crate::speech::SpeechListener;

/// Where the timeline keeps each clip's body effect.
pub const BODY: &str = "capcraftBody";

/// A body effect's links: those the clip under the person gets, and those
/// the person's copy gets.
fn looks(kind: &str) -> Option<(Vec<Value>, Vec<Value>)> {
    // the people lit: how many stops, how far their shadows are lifted and
    // their highlights eased
    if let Some(asked) = kind.strip_prefix("light:") {
        let mut asked = asked.split(':').map(|n| n.parse::<f64>().unwrap_or(0.0));
        let mut next = || asked.next().unwrap_or(0.0);
        let (exposure, shadows, highlights) = (next(), next(), next());
        let mut person = vec![json!({"id": "concat.adjust", "params": {
            "exposure": exposure.clamp(-3.0, 3.0),
            "shadows": shadows.clamp(-100.0, 100.0),
            "highlights": highlights.clamp(-100.0, 100.0),
        }})];
        // and a light on the face, where it is in shadow
        let (x, y, size, stops) = (next(), next(), next(), next());
        if stops > 0.0 {
            person.push(json!({"id": "capcraft.face-light", "params": {
                "x": (x * 100.0).round().clamp(0.0, 100.0),
                "y": (y * 100.0).round().clamp(0.0, 100.0),
                "size": (size * 100.0).round().clamp(5.0, 80.0),
                "stops": stops.clamp(0.0, 2.5),
            }}));
        }
        return Some((Vec::new(), person));
    }
    // the people washed in one colour, as red for anger: its colour (red, green,
    // blue, alpha in one number) and how much, keyed later to make it grow
    if let Some(asked) = kind.strip_prefix("tint:") {
        let mut asked = asked.split(':');
        let colour = asked.next().and_then(|c| c.parse::<u32>().ok()).unwrap_or(0xFF2A_1AFF);
        let amount = asked.next().and_then(|a| a.parse::<f64>().ok()).unwrap_or(70.0).clamp(0.0, 100.0);
        return Some((Vec::new(), vec![json!({"id": "capcraft.tint", "params": {"color": colour, "amount": amount}})]));
    }
    Some(match kind {
        // the people alone, in front of whatever is laid over the clip
        "front" => (Vec::new(), Vec::new()),
        // the people in black and white, the rest in colour
        "grey" => (
            Vec::new(),
            vec![json!({"id": "concat.saturation", "params": {"amount": -100}})],
        ),
        "blur" => (
            vec![json!({"id": "concat.gaussian-blur", "params": {"radius": 18}})],
            Vec::new(),
        ),
        "pop" => (
            vec![json!({"id": "concat.silver", "params": {}})],
            Vec::new(),
        ),
        "spot" => (
            vec![json!({"id": "concat.adjust", "params": {"exposure": -1.6, "saturation": -40}})],
            Vec::new(),
        ),
        "neon" => (
            vec![json!({"id": "concat.adjust", "params": {"exposure": -0.9}})],
            vec![
                json!({"id": "capcraft.outline", "params": {"width": 8, "strength": 90, "hue": 190}}),
            ],
        ),
        // a wide, soft golden glow round the person
        "aura" => (
            vec![json!({"id": "concat.adjust", "params": {"exposure": -0.5}})],
            vec![
                json!({"id": "capcraft.outline", "params": {"width": 26, "strength": 65, "hue": 45}}),
            ],
        ),
        // a thin bright line traced round the person on a dimmed picture
        "lines" => (
            vec![json!({"id": "concat.adjust", "params": {"exposure": -0.7}})],
            vec![
                json!({"id": "capcraft.outline", "params": {"width": 3, "strength": 100, "hue": 300}}),
            ],
        ),
        // the background soft and glowing, the person sharp
        "dream" => (
            vec![
                json!({"id": "concat.gaussian-blur", "params": {"radius": 6}}),
                json!({"id": "concat.glow", "params": {"amount": 70}}),
            ],
            Vec::new(),
        ),
        // the background running through the colours, the person as they are
        "trippy" => (
            vec![json!({"id": "capcraft.rainbow", "params": {"amount": 70, "speed": 0.8}})],
            Vec::new(),
        ),
        // the background breaking up, the person whole
        "glitch" => (
            vec![json!({"id": "capcraft.glitch", "params": {"amount": 70}})],
            Vec::new(),
        ),
        // the person inked as a comic, the background printed on paper
        "comic" => (
            vec![
                json!({"id": "capcraft.comic-ink", "params": {"amount": 50, "levels": 5}}),
                json!({"id": "capcraft.paper", "params": {"amount": 80}}),
            ],
            vec![json!({"id": "capcraft.comic-ink", "params": {"amount": 90, "levels": 4}})],
        ),
        _ => return None,
    })
}

/// A command from its JSON, or the reason it is not one.
fn command(value: Value) -> Result<Command, MobileError> {
    serde_json::from_value(value).map_err(|error| MobileError::Failed(error.to_string()))
}

/// `chain` without the last link of each id in `added`.
fn without(chain: &[Value], added: &[Value]) -> Vec<Value> {
    let mut chain = chain.to_vec();
    for id in added.iter().filter_map(Value::as_str) {
        if let Some(at) = chain
            .iter()
            .rposition(|link| link.get("id").and_then(Value::as_str) == Some(id))
        {
            chain.remove(at);
        }
    }
    chain
}

#[uniffi::export]
impl MobileSession {
    /// Puts body effect `kind` ("blur", "pop", "spot", "neon", "aura",
    /// "lines", "dream", "trippy", "glitch", "comic", "grey", or
    /// "tint:colour:amount") on
    /// `clip_id`, in place of any it had; an empty `kind` takes it off. The
    /// person's masks are then found on a thread of its own, reported
    /// through `listener`.
    pub fn body_effect(
        self: Arc<Self>,
        clip_id: String,
        kind: String,
        listener: Arc<dyn SpeechListener>,
    ) -> Result<(), MobileError> {
        let media = {
            let mut state = self.lock();
            let gesture = self.gesture();
            let made = Self::body_commands(&mut state, &clip_id, &kind, &gesture);
            state.session.end_gesture();
            match made {
                Err(error) => return Err(error),
                Ok(None) => {
                    let view = Self::changed(&mut state, None);
                    drop(state);
                    listener.on_done(Some(view));
                    return Ok(());
                }
                Ok(Some(media)) => {
                    // drawn at once without the masks; again when they are in
                    Self::changed(&mut state, None);
                    media
                }
            }
        };
        self.find_masks(media, listener)
    }

    /// Makes the people's tint on `clip_id` grow: none at the clip's start,
    /// `amount` (0 to 100) at `peak_us` on the timeline, held after it, as
    /// anger building. A peak at the start takes the growing off and leaves it
    /// at `amount` throughout. One undo step.
    pub fn body_tint_rise(&self, clip_id: String, peak_us: i64, amount: f64) -> Result<crate::session::TimelineView, MobileError> {
        let bodies: Value = serde_json::from_str(&self.extra(BODY.to_owned())).unwrap_or(Value::Null);
        let copy = bodies
            .get(&clip_id)
            .and_then(|b| b.get("copy"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MobileError::Failed("Colour the people first.".to_owned()))?;
        let (entry, start, length) = self.with_session(|session| {
            let clip = session
                .project()
                .active()
                .clips
                .iter()
                .find(|c| c.id == copy)
                .ok_or_else(|| MobileError::Failed("The people's copy is gone.".to_owned()))?;
            let entry = clip
                .video_effects
                .iter()
                .position(|link| link.id == "capcraft.tint")
                .ok_or_else(|| MobileError::Failed("The people have no colour to grow.".to_owned()))?;
            Ok((entry, clip.start, clip.duration))
        })?;
        let amount = amount.clamp(0.0, 100.0);
        let at = if length > 0.0 { ((crate::seconds(peak_us) - start) / length).clamp(0.0, 1.0) } else { 0.0 };
        let mut commands = vec![command(json!({"op": "clearEffectKeys", "clipId": copy, "entry": entry, "key": "amount"}))?];
        if at > 0.01 {
            commands.push(command(json!({"op": "setEffectKey", "clipId": copy, "entry": entry, "key": "amount", "at": 0.0, "value": 0.0}))?);
            commands.push(command(json!({"op": "setEffectKey", "clipId": copy, "entry": entry, "key": "amount", "at": at, "value": amount}))?);
        }
        self.run(commands)
    }
}

impl MobileSession {
    /// The edits for [`MobileSession::body_effect`] under one gesture: the
    /// old effect off, the new one on. The bin item whose masks the copy
    /// needs, or None when there is no new effect.
    fn body_commands(
        state: &mut State,
        clip_id: &str,
        kind: &str,
        gesture: &str,
    ) -> Result<Option<String>, MobileError> {
        let timeline = state.session.project().active().clone();
        if !timeline.clips.iter().any(|clip| clip.id == clip_id) {
            return Err(MobileError::Failed("That clip is gone.".to_owned()));
        }
        let mut kept = match timeline.extra.get(BODY) {
            Some(Value::Object(map)) => map.clone(),
            _ => serde_json::Map::new(),
        };
        // the effect it has comes off first: the copy, and what the clip was given
        if let Some(old) = kept.remove(clip_id) {
            let added: Vec<Value> = old
                .get("added")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut commands = Vec::new();
            if let Some(copy) = old.get("copy").and_then(Value::as_str)
                && timeline.clips.iter().any(|clip| clip.id == copy)
            {
                commands.push(Command::RemoveClips {
                    clip_ids: vec![copy.to_owned()],
                    ripple: false,
                });
            }
            if let Some(clip) = timeline.clips.iter().find(|clip| clip.id == clip_id) {
                let chain =
                    serde_json::to_value(&clip.video_effects).unwrap_or(Value::Array(Vec::new()));
                let chain = chain.as_array().cloned().unwrap_or_default();
                commands.push(command(json!({
                    "op": "updateClip",
                    "clipId": clip_id,
                    "patch": {"videoEffects": without(&chain, &added)},
                }))?);
            }
            Self::apply_all(state, commands, Some(gesture))?;
        }
        let Some((under, over)) = looks(kind) else {
            Self::apply_all(
                state,
                vec![Command::SetTimelineExtra {
                    key: BODY.to_owned(),
                    value: (!kept.is_empty()).then_some(Value::Object(kept)),
                }],
                Some(gesture),
            )?;
            return Ok(None);
        };
        let timeline = state.session.project().active().clone();
        let clip = timeline
            .clips
            .iter()
            .find(|clip| clip.id == clip_id)
            .cloned()
            .ok_or_else(|| MobileError::Failed("That clip is gone.".to_owned()))?;
        // the copy on a lane of its own over the clip; it is placed at the file's
        // whole length before it is cut to the clip's, so the lane must be free for that
        let whole = state
            .session
            .project()
            .media
            .iter()
            .find(|item| item.id == clip.media_id)
            // a picture is placed at a still's length
            .map(|item| if item.kind == concat_project::model::MediaKind::Image { crate::session::STILL_SECONDS } else { item.duration.unwrap_or(clip.duration) })
            .unwrap_or(crate::session::STILL_SECONDS)
            .max(clip.duration);
        let track = match lanes::free_lane(&timeline, RowKind::Overlay, clip.start, whole) {
            Some(track) => track,
            None => {
                let index = lanes::insert_index(&timeline, RowKind::Overlay);
                Self::apply_all(state, vec![Command::InsertTrack { index }], Some(gesture))?
                    .ok_or_else(|| MobileError::Failed("The new lane has no id.".to_owned()))?
            }
        };
        let copy = Self::apply_all(
            state,
            vec![Command::AddClip {
                media_id: clip.media_id.clone(),
                track_id: track,
                start: clip.start,
                ripple: false,
            }],
            Some(gesture),
        )?
        .ok_or_else(|| MobileError::Failed("The copy was not made.".to_owned()))?;
        copy_look(state, &clip, &copy, gesture)?;
        let mine = serde_json::to_value(&clip.video_effects).unwrap_or(Value::Array(Vec::new()));
        let mine = mine.as_array().cloned().unwrap_or_default();
        let mut person = mine.clone();
        person.extend(over);
        let mut background = mine;
        background.extend(under.iter().cloned());
        let added: Vec<Value> = under
            .iter()
            .filter_map(|link| link.get("id").cloned())
            .collect();
        kept.insert(
            clip_id.to_owned(),
            json!({"copy": copy, "kind": kind, "added": added}),
        );
        Self::apply_all(
            state,
            vec![
                command(json!({
                    "op": "updateClip",
                    "clipId": copy,
                    // what is behind the picture stays behind the original
                    "patch": {"videoEffects": person, "volume": 0.0, "background": null},
                }))?,
                Command::SetClipCutout {
                    clip_id: copy.clone(),
                    cutout: Some(Cutout {
                        mode: CutoutMode::Auto,
                        subject: Subject::Auto,
                        // light fades off at their edge rather than stopping
                        feather: if kind.starts_with("light:") { 0.006 } else { 0.002 },
                        strokes: Vec::new(),
                        invert: false,
                    }),
                },
                command(json!({
                    "op": "updateClip",
                    "clipId": clip_id,
                    "patch": {"videoEffects": background},
                }))?,
                Command::SetTimelineExtra {
                    key: BODY.to_owned(),
                    value: Some(Value::Object(kept)),
                },
            ],
            Some(gesture),
        )?;
        Ok(Some(clip.media_id.clone()))
    }
}

#[uniffi::export]
impl MobileSession {
    /// Puts the lane of `copy_id` (a person's copy) right over the lane of
    /// `under_id` (a title or a sticker), so that one shows behind the person
    /// and whatever is over it stays in front. With `under_id` empty, puts the
    /// copy back with the overlays.
    pub fn place_person_over(&self, copy_id: String, under_id: String) -> Result<crate::session::TimelineView, MobileError> {
        let timeline = self.with_session(|session| Ok(session.project().active().clone()))?;
        let lane = |id: &str| {
            let track = &timeline.clips.iter().find(|clip| clip.id == id)?.track_id;
            timeline.tracks.iter().position(|t| &t.id == track)
        };
        let from = lane(&copy_id).ok_or_else(|| MobileError::Failed("The person's copy is gone.".to_owned()))?;
        let to = if under_id.is_empty() {
            // the highest overlay lane, or just over the main track
            (1..timeline.tracks.len())
                .filter(|i| *i != from && lanes::role(&timeline, *i) == Some(RowKind::Overlay))
                .max()
                .map_or(1, |i| if i < from { i + 1 } else { i })
        } else {
            let under = lane(&under_id).ok_or_else(|| MobileError::Failed("That clip is gone.".to_owned()))?;
            if under < from { under + 1 } else { under }
        };
        if to == from {
            return Ok(self.view());
        }
        self.run(vec![Command::MoveTrack {
            track_id: timeline.tracks[from].id.clone(),
            index: to,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_has_a_look_for_the_background() {
        for kind in [
            "blur", "pop", "spot", "neon", "aura", "lines", "dream", "trippy", "glitch", "comic",
        ] {
            let (under, _) = looks(kind).expect("a look");
            assert!(!under.is_empty(), "{kind}");
        }
        assert!(looks("").is_none());
    }

    #[test]
    fn taking_an_effect_off_takes_only_what_it_added() {
        let chain = vec![
            json!({"id": "concat.gaussian-blur", "params": {"radius": 3}}),
            json!({"id": "concat.vhs"}),
            json!({"id": "concat.gaussian-blur", "params": {"radius": 18}}),
        ];
        let left = without(&chain, &[json!("concat.gaussian-blur")]);
        assert_eq!(left, chain[..2].to_vec());
    }
}
