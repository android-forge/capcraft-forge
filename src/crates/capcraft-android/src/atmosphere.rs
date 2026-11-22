// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Atmosphere: a picture of smoke, light or dust laid over a clip, placed and
//! blended in one undo step.

use concat_project::Command;
use concat_project::commands::TrimEdge;
use serde_json::json;

use crate::MobileError;
use crate::lanes::RowKind;
use crate::session::{MobileSession, TimelineView};

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
}
