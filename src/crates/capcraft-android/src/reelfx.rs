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

fn faces() -> &'static Faces {
    static ONE: OnceLock<Faces> = OnceLock::new();
    ONE.get_or_init(Faces::new)
}

impl MobileSession {
    fn reel_requests(&self, clip_id: &str) -> Result<Vec<(concat_host::FaceRequest, concat_host::AnalyseRequest)>, MobileError> {
        self.with_session(|session| {
            let project = session.project();
            let media = project.active().clips.iter().find(|c| c.id == clip_id).map(|c| c.media_id.clone()).unwrap_or_default();
            Ok(Faces::requests(project, std::path::Path::new(session.path()))
                .into_iter()
                .filter(|(id, _, _)| *id == media)
                .map(|(_, f, m)| (f, m))
                .collect())
        })
    }
}

#[uniffi::export]
impl MobileSession {
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
