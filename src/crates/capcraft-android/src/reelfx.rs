// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Forge's reel effects on the phone: the list the panel shows, and the
//! tracking they draw around - the face, found by YuNet, and the person,
//! found by Remove BG's person model - for a clip's stretches with effects.

use std::sync::{Arc, OnceLock};

use concat_host::cutout::Progress;
use concat_host::{Cutouts, Faces};

use crate::MobileError;
use crate::cutout::cutouts;
use crate::session::MobileSession;
use crate::speech::SpeechListener;

/// One reel effect as the panel lists it.
#[derive(uniffi::Record)]
pub struct ReelEffectInfo {
    /// The name a clip's `reelFx` carries.
    pub name: String,
    pub title: String,
    pub desc: String,
    /// The settings it reads, `name: what` joined by `; `.
    pub options: String,
    /// It lays out pictures: the clip's own frames, or the user's.
    pub cards: bool,
}

/// Every reel effect, in order.
#[uniffi::export]
pub fn reel_effects() -> Vec<ReelEffectInfo> {
    capcraft_reelfx::effects()
        .iter()
        .map(|e| ReelEffectInfo {
            name: e.name.to_owned(),
            title: e.title.to_owned(),
            desc: e.desc.to_owned(),
            options: e.options.to_owned(),
            cards: e.cards,
        })
        .collect()
}

/// One setting a reel effect reads, as the panel lays it out.
#[derive(uniffi::Record)]
pub struct ReelSetting {
    pub key: String,
    /// "text", "number" or "color".
    pub kind: String,
    /// What the effect uses when it is not set.
    pub default_value: String,
}

/// The settings effect `name` reads, in the order it reads them.
#[uniffi::export]
pub fn reel_settings(name: String) -> Vec<ReelSetting> {
    capcraft_reelfx::settings(&name)
        .into_iter()
        .map(|s| ReelSetting {
            key: s.key,
            kind: match s.kind {
                capcraft_reelfx::Kind::Text => "text",
                capcraft_reelfx::Kind::Number => "number",
                capcraft_reelfx::Kind::Color => "color",
            }
            .to_owned(),
            default_value: s.default,
        })
        .collect()
}

/// The fonts a reel effect's `font` can name: the app's own, by full name.
#[uniffi::export]
pub fn reel_fonts() -> Vec<String> {
    capcraft_reelfx::canvas::base_names().iter().filter(|n| !n.is_empty()).cloned().collect()
}

fn faces() -> &'static Faces {
    static ONE: OnceLock<Faces> = OnceLock::new();
    ONE.get_or_init(Faces::new)
}

impl MobileSession {
    fn reel_requests(&self, clip_id: &str) -> Result<Vec<(concat_host::FaceRequest, concat_host::AnalyseRequest)>, MobileError> {
        self.with_session(|session| {
            let project = session.project();
            let timeline = project.active();
            // a reel layer's: the pictures beneath it it draws on
            let media: Vec<String> = match timeline.clips.iter().find(|c| c.id == clip_id) {
                Some(c) if c.kind == concat_project::model::ClipKind::Layer => timeline
                    .clips
                    .iter()
                    .filter(|v| v.start < c.start + c.duration && c.start < v.start + v.duration)
                    .map(|v| v.media_id.clone())
                    .collect(),
                Some(c) => vec![c.media_id.clone()],
                None => Vec::new(),
            };
            Ok(Faces::requests(project, std::path::Path::new(session.path()))
                .into_iter()
                .filter(|(id, _, _)| media.contains(id))
                .map(|(_, f, m)| (f, m))
                .collect())
        })
    }
}

#[uniffi::export]
impl MobileSession {
    /// Lays reel effect `fx` on the effect row at `at_us` for `duration_us`,
    /// as an item of its own over the pictures beneath it: moved, trimmed
    /// and keyed (its opacity the effect's strength) as any layer is.
    pub fn add_reel_layer(&self, fx: String, name: String, at_us: i64, duration_us: i64) -> Result<crate::session::TimelineView, MobileError> {
        if capcraft_reelfx::find(&fx).is_none() {
            return Err(MobileError::from(format!("no reel effect {fx}")));
        }
        let start = (at_us as f64 / 1e6).max(0.0);
        let duration = (duration_us as f64 / 1e6).max(0.1);
        let before: std::collections::HashSet<String> = self.timeline().clips.iter().map(|c| c.id.clone()).collect();
        self.place_many(
            crate::lanes::RowKind::Effect,
            start,
            duration,
            |track_id| {
                vec![concat_project::commands::Command::AddLayerClip {
                    track_id: Some(track_id),
                    start,
                    duration: Some(duration),
                    effect_id: String::new(),
                    name,
                    sound: false,
                }]
            },
            |after| {
                after
                    .clips
                    .iter()
                    .filter(|c| !before.contains(&c.id))
                    .map(|c| concat_project::commands::Command::UpdateClip {
                        clip_id: c.id.clone(),
                        patch: concat_project::commands::ClipPatch {
                            reel_fx: Some(vec![concat_project::model::ReelEffect {
                                fx: fx.clone(),
                                start: 0.0,
                                end: c.duration,
                                options: Default::default(),
                                strength: Vec::new(),
                            }]),
                            ..Default::default()
                        },
                    })
                    .collect()
            },
        )
    }

    /// Whether clip `clip_id`'s reel effects still have a face or a person
    /// to find.
    pub fn reel_tracking_missing(&self, clip_id: String) -> Result<bool, MobileError> {
        Ok(self
            .reel_requests(&clip_id)?
            .iter()
            .any(|(f, m)| Faces::outstanding(f) > 0 || Cutouts::outstanding(m) > 0))
    }

    /// Finds the face and the person for clip `clip_id`'s reel effects on a
    /// thread of its own, reporting through `listener` (the face first,
    /// then the person), and draws again when they are in.
    pub fn track_reel(self: Arc<Self>, clip_id: String, listener: Arc<dyn SpeechListener>) -> Result<(), MobileError> {
        let requests = self.reel_requests(&clip_id)?;
        if requests.is_empty() {
            listener.on_done(None);
            return Ok(());
        }
        let cutouts = cutouts()?;
        std::thread::Builder::new()
            .name("capcraft-reel-track".to_owned())
            .spawn(move || {
                for (face, person) in &requests {
                    let mut report = |half: f32, p: Progress| match p {
                        Progress::Fetching { received, total } => {
                            listener.on_progress(if total > 0 { received as f32 / total as f32 } else { 0.0 }, "download".to_owned())
                        }
                        Progress::Analysing(done) => listener.on_progress(half + done * 0.5, "work".to_owned()),
                    };
                    if let Err(message) = faces().analyse(face, &mut |p| report(0.0, p)) {
                        listener.on_failed(message);
                        return;
                    }
                    if let Err(message) = cutouts.analyse(person, &mut |p| report(0.5, p)) {
                        listener.on_failed(message);
                        return;
                    }
                }
                listener.on_done(Some(self.refreshed()));
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
    }

    /// Stops the tracking in progress.
    pub fn cancel_reel_tracking(&self) {
        faces().cancel();
        if let Ok(c) = cutouts() {
            c.cancel();
        }
    }
}
