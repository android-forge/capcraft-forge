// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Atmosphere: a picture of smoke, light or dust laid over a clip, placed and
//! blended in one undo step.

use concat_project::Command;
use concat_project::commands::TrimEdge;
use serde_json::json;

use crate::MobileError;
use crate::lanes::RowKind;
use crate::session::{MobileSession, TimelineView, copy_look};

/// Where the timeline keeps the shadow a light cast: the person's copy's id
/// to its shadow's, so the next light takes the last one's shadow away.
pub const SHADOWS: &str = "capcraftShadows";

/// How a picture is laid over a clip.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Laid {
    /// The blend mode, as the engine names it ("screen").
    pub blend: String,
    /// How strong it is, 0 to 1.
    pub opacity: f64,
    /// Its size; 1 fits the frame.
    pub scale: f64,
    /// How far right of the middle it is, as a fraction of the frame.
    pub x: f64,
    /// How far below the middle.
    pub y: f64,
    /// Clockwise degrees.
    pub rotation: f64,
    /// Mirrored across.
    pub flip: bool,
    /// The colour adjustments it has (`concat.adjust`'s parameters, JSON), or
    /// empty.
    pub adjust: String,
}

#[uniffi::export]
impl MobileSession {
    /// Lays the picture in `path` (in the project's media) over clip
    /// `clip_id`, for as long as it.
    pub fn lay_over(
        &self,
        clip_id: String,
        path: String,
        laid: Laid,
    ) -> Result<TimelineView, MobileError> {
        let (start, duration) = self.with_session(|session| {
            session
                .project()
                .active()
                .clips
                .iter()
                .find(|clip| clip.id == clip_id)
                .map(|clip| (clip.start, clip.duration))
                .ok_or_else(|| MobileError::Failed("That clip is gone.".to_owned()))
        })?;
        let media = self.import(path)?;
        let before: std::collections::HashSet<String> = self.with_session(|session| {
            Ok(session
                .project()
                .active()
                .clips
                .iter()
                .map(|clip| clip.id.clone())
                .collect())
        })?;
        let adjust: Option<serde_json::Value> = (!laid.adjust.is_empty())
            .then(|| serde_json::from_str(&laid.adjust).ok())
            .flatten();
        self.place_many(
            RowKind::Overlay,
            start,
            duration,
            |track| {
                vec![Command::AddClip {
                    media_id: media.id.clone(),
                    track_id: track,
                    start,
                    ripple: false,
                }]
            },
            |timeline| {
                let Some(new) = timeline
                    .clips
                    .iter()
                    .find(|clip| clip.media_id == media.id && !before.contains(&clip.id))
                else {
                    return Vec::new();
                };
                let mut patch = json!({
                    "blend": laid.blend,
                    "opacity": laid.opacity,
                    "flipH": laid.flip,
                });
                if let Some(params) = &adjust {
                    patch["videoEffects"] = json!([{"id": "concat.adjust", "params": params}]);
                }
                let mut commands = vec![
                    Command::TrimClip {
                        clip_id: new.id.clone(),
                        edge: TrimEdge::End,
                        delta: duration - new.duration,
                        ripple: false,
                    },
                    Command::SetClipTransform {
                        clip_id: new.id.clone(),
                        scale: Some(laid.scale),
                        offset_x: Some(laid.x),
                        offset_y: Some(laid.y),
                        rotation: Some(laid.rotation),
                        stretch_x: None,
                        stretch_y: None,
                    },
                ];
                commands.extend(
                    serde_json::from_value(json!({
                        "op": "updateClip", "clipId": new.id, "patch": patch,
                    }))
                    .ok(),
                );
                commands
            },
        )
    }

    /// A light from (`light_x`, `light_y`) - where it is brightest, as
    /// fractions of the frame from its top left - throws the shadow of the
    /// person cut out in `person_id` (a body effect's copy) on what is behind
    /// them: a dark, soft copy of them, pushed away from the light and
    /// multiplied in, `strength` (0 to 1) strong. A shadow the person had
    /// from an earlier light goes. Returns the view with the shadow made.
    pub fn cast_shadow(&self, person_id: String, light_x: f64, light_y: f64, strength: f64) -> Result<TimelineView, MobileError> {
        let person = self
            .timeline()
            .clips
            .iter()
            .find(|clip| clip.id == person_id)
            .cloned()
            .ok_or_else(|| MobileError::Failed("The person's copy is gone.".to_owned()))?;
        // the last shadow off
        let mut shadows: serde_json::Value = serde_json::from_str(&self.extra(SHADOWS.to_owned())).unwrap_or_else(|_| json!({}));
        if !shadows.is_object() {
            shadows = json!({});
        }
        if let Some(old) = shadows.get(&person_id).and_then(serde_json::Value::as_str).map(str::to_owned)
            && self.timeline().clips.iter().any(|clip| clip.id == old)
        {
            self.delete(vec![old])?;
        }
        // away from the light: the person's middle is the frame's, near enough
        let (dx, dy) = (0.5 - light_x.clamp(0.0, 1.0), 0.5 - light_y.clamp(0.0, 1.0));
        let reach = (dx * dx + dy * dy).sqrt().max(1e-3);
        let push = 0.035 + 0.05 * reach.min(0.7);
        let (sx, sy) = (dx / reach * push, dy / reach * push * 0.6 + 0.01);
        let made = self
            .place_many(
                RowKind::Overlay,
                person.start,
                person.duration,
                |track| vec![Command::AddClip { media_id: person.media_id.clone(), track_id: track, start: person.start, ripple: false }],
                |_| Vec::new(),
            )?
            .created_id;
        if made.is_empty() {
            return Err(MobileError::Failed("No room for the shadow.".to_owned()));
        }
        {
            let mut state = self.lock();
            let gesture = self.gesture();
            copy_look(&mut state, &person, &made, &gesture)?;
            state.session.end_gesture();
        }
        let now = self.timeline().clips.iter().find(|clip| clip.id == made).map_or(person.duration, |clip| clip.duration);
        shadows[person_id.as_str()] = json!(made);
        let commands: Vec<Command> = [
            json!({"op": "trimClip", "clipId": made, "edge": "end", "delta": person.duration - now, "ripple": false}),
            json!({"op": "updateClip", "clipId": made, "patch": {
                "videoEffects": [
                    {"id": "concat.adjust", "params": {"exposure": -3.0, "saturation": -100}},
                    {"id": "concat.gaussian-blur", "params": {"radius": 14}},
                ],
                "opacity": (0.6 * strength).clamp(0.05, 0.8),
                "blend": "multiply",
                "volume": 0.0,
            }}),
            json!({"op": "setClipTransform", "clipId": made, "offsetX": person.offset_x + sx, "offsetY": person.offset_y + sy}),
            json!({"op": "setTimelineExtra", "key": SHADOWS, "value": shadows}),
        ]
        .into_iter()
        .filter_map(|c| serde_json::from_value(c).ok())
        .collect();
        let mut view = self.run(commands)?;
        // the person in front of their shadow
        view = self.restack(person_id, 99)?;
        view.created_id = made;
        Ok(view)
    }
}
