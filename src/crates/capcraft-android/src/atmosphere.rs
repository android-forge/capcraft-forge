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

/// The effect that lights the person from the light's side.
const RELIGHT: &str = "capcraft.relight";

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
        // the picture is placed at a still's length before it is cut to the clip's,
        // so its lane must be free for that long
        self.place_many(
            RowKind::Overlay,
            start,
            duration.max(crate::session::STILL_SECONDS),
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
    /// `strength` is the light's, 0 to 1: how dark its shadows are and how
    /// much it lights the person. The person is lit from the light too: a key light of `key_colour`
    /// (red, green, blue, alpha) on the side facing it, the far side left to
    /// fall darker, as a rim of sun lights someone standing in it.
    pub fn cast_shadow(&self, person_id: String, light_x: f64, light_y: f64, strength: f64, key_colour: u32) -> Result<TimelineView, MobileError> {
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
        // the person's own light: their effects, any relight before replaced, one from the light's side
        // (round from the left, clockwise: from the left 0, from above 90, from the right 180)
        let angle = (0.5 - light_y).atan2(0.5 - light_x).to_degrees();
        let mut lit: Vec<serde_json::Value> = serde_json::to_value(&person.video_effects)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|link| link.get("id").and_then(serde_json::Value::as_str) != Some(RELIGHT))
            .collect();
        lit.push(json!({"id": RELIGHT, "params": {
            "key": key_colour, "angle": angle.round(), "strength": (55.0 * strength).round().clamp(10.0, 70.0), "softness": 60, "spot": 0,
        }}));
        // a picture is placed at a still's length before it is cut to the person's
        let still = |id: &str| {
            self.with_session(|session| Ok(session.project().media.iter().find(|item| item.id == id).map(|item| (item.kind == concat_project::model::MediaKind::Image, item.duration, item.path.clone()))))
        };
        let (is_still, length, path) = still(&person.media_id)?.ok_or_else(|| MobileError::Failed("The person's picture is gone.".to_owned()))?;
        let whole = if is_still { crate::session::STILL_SECONDS } else { length.unwrap_or(person.duration) }.max(person.duration);

        // the shadows the scene's own shapes throw, from its depth; a soft copy of the person if that cannot be had
        let traced = self.traced_shadows(&path, person.source_start, light_x, light_y, strength, key_colour);
        let (media_id, patch, transform, whole) = match &traced {
            Ok(file) => {
                let media = self.import(file.clone())?;
                (
                    media.id,
                    json!({"blend": "multiply", "opacity": 1.0, "volume": 0.0, "flipH": person.flip_h, "flipV": person.flip_v}),
                    json!({"op": "setClipTransform", "clipId": "", "scale": person.scale, "offsetX": person.offset_x, "offsetY": person.offset_y, "rotation": person.rotation}),
                    whole.max(crate::session::STILL_SECONDS),
                )
            }
            Err(_) => {
                // away from the light: the person's middle is the frame's, near enough
                let (dx, dy) = (0.5 - light_x.clamp(0.0, 1.0), 0.5 - light_y.clamp(0.0, 1.0));
                let reach = (dx * dx + dy * dy).sqrt().max(1e-3);
                let push = 0.05 + 0.07 * reach.min(0.7);
                let (sx, sy) = (dx / reach * push, dy / reach * push * 0.6 + 0.015);
                (
                    person.media_id.clone(),
                    json!({
                        "videoEffects": [
                            {"id": "concat.adjust", "params": {"exposure": -3.0, "saturation": -100}},
                            {"id": "concat.gaussian-blur", "params": {"radius": 12}},
                        ],
                        "opacity": (0.55 * strength).clamp(0.1, 0.6),
                        "blend": "multiply",
                        "volume": 0.0,
                    }),
                    json!({"op": "setClipTransform", "clipId": "", "offsetX": person.offset_x + sx, "offsetY": person.offset_y + sy}),
                    whole,
                )
            }
        };
        let made = self
            .place_many(
                RowKind::Overlay,
                person.start,
                whole,
                |track| vec![Command::AddClip { media_id: media_id.clone(), track_id: track, start: person.start, ripple: false }],
                |_| Vec::new(),
            )?
            .created_id;
        if made.is_empty() {
            return Err(MobileError::Failed("No room for the shadow.".to_owned()));
        }
        if traced.is_err() {
            // the person's copy, cut out the same, made dark
            let mut state = self.lock();
            let gesture = self.gesture();
            copy_look(&mut state, &person, &made, &gesture)?;
            state.session.end_gesture();
        }
        let now = self.timeline().clips.iter().find(|clip| clip.id == made).map_or(person.duration, |clip| clip.duration);
        shadows[person_id.as_str()] = json!(made);
        let mut transform = transform;
        transform["clipId"] = json!(made);
        let commands: Vec<Command> = [
            json!({"op": "updateClip", "clipId": person_id, "patch": {"videoEffects": lit}}),
            json!({"op": "trimClip", "clipId": made, "edge": "end", "delta": person.duration - now, "ripple": false}),
            json!({"op": "updateClip", "clipId": made, "patch": patch}),
            transform,
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

    /// Takes away what [`MobileSession::cast_shadow`] gave the person in
    /// `person_id`: their shadow and the light on them, as light laid flat
    /// (2D) has neither. One undo step.
    pub fn unlight_person(&self, person_id: String) -> Result<TimelineView, MobileError> {
        let timeline = self.timeline();
        let mut shadows: serde_json::Value = serde_json::from_str(&self.extra(SHADOWS.to_owned())).unwrap_or_else(|_| json!({}));
        let mut commands = Vec::new();
        if let Some(old) = shadows.get(&person_id).and_then(serde_json::Value::as_str)
            && timeline.clips.iter().any(|clip| clip.id == old)
        {
            commands.push(json!({"op": "removeClips", "clipIds": [old], "ripple": false}));
        }
        if let Some(all) = shadows.as_object_mut() {
            all.remove(&person_id);
        }
        commands.push(json!({"op": "setTimelineExtra", "key": SHADOWS, "value": shadows}));
        if let Some(person) = timeline.clips.iter().find(|clip| clip.id == person_id) {
            let kept: Vec<serde_json::Value> = serde_json::to_value(&person.video_effects)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .filter(|link| link.get("id").and_then(serde_json::Value::as_str) != Some(RELIGHT))
                .collect();
            commands.push(json!({"op": "updateClip", "clipId": person_id, "patch": {"videoEffects": kept}}));
        }
        self.run(commands.into_iter().filter_map(|c| serde_json::from_value(c).ok()).collect())
    }
}

impl MobileSession {
    /// The shadows a light at (`light_x`, `light_y`) throws in the picture
    /// of `path` at `at` seconds, traced through its depth, written as a
    /// picture to multiply it by; the file's path. The depth model is
    /// fetched the first time.
    pub(crate) fn traced_shadows(&self, path: &str, at: f64, light_x: f64, light_y: f64, strength: f64, key: u32) -> Result<String, String> {
        let frame = concat_host::media::still_at(path, at.max(0.0), 720).map_err(|error| error.to_string())?;
        let picture = crate::Picture::of(frame);
        let model = crate::depthlight::model(&mut |_| {})?;
        let depth = crate::depthlight::depth_of(&model, &picture)?;
        let (w, h, shade) = crate::depthlight::shadows(&depth, light_x as f32, light_y as f32);
        // none cast across what the picture's own lights already light
        let shade = crate::depthlight::filled_by_other_lights(&shade, w, h, &picture);
        // as dark as the light is strong, in the colour of the light still reaching in
        let colour = crate::depthlight::shadow_colour(&picture, key);
        let out = crate::depthlight::shade_picture(w, h, &shade, (0.8 * strength) as f32, colour);
        let dir = std::path::Path::new(&self.with_session(|session| Ok(session.path().to_owned())).map_err(|e| e.to_string())?).join("media");
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let file = dir.join(format!("Shadows {}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())));
        crate::session::write_png(&file, &out).map_err(|error| error.to_string())?;
        Ok(file.to_string_lossy().into_owned())
    }
}
