// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Compound clips: neighbouring video clips of one row made into one, as
//! they look and sound, so the one can be trimmed, sped up or moved whole.
//!
//! The clips are rendered to a file of the project's and the file put in
//! their place (the engine's `ConsolidateClips`), as one undo step: undo
//! brings the clips back.

use std::path::Path;
use std::sync::Arc;

use concat_host::export::{Progress, RateMode};
use concat_host::consolidate::Consolidation;

use crate::export::fraction_of;
use crate::session::{MobileSession, TimelineView};
use crate::{MobileError, context};

/// What making a compound clip reports.
#[uniffi::export(with_foreign)]
pub trait CompoundListener: Send + Sync {
    /// How far along it is, 0 to 1.
    fn on_progress(&self, fraction: f32);
    /// It finished: the timeline with the clips made one.
    fn on_done(&self, view: TimelineView);
    /// It failed or was cancelled (`cancelled` true).
    fn on_failed(&self, message: String, cancelled: bool);
}

#[uniffi::export]
impl MobileSession {
    /// Makes `clip_ids`, video clips of one row with nothing between them,
    /// one clip. Runs on a thread of its own, as an export does, and stops
    /// as one does.
    pub fn compound(
        self: Arc<Self>,
        clip_ids: Vec<String>,
        listener: Arc<dyn CompoundListener>,
    ) -> Result<(), MobileError> {
        let context = context()?;
        let job = context
            .exporter
            .begin()
            .map_err(|_| MobileError::Busy("An export is already running.".to_owned()))?;
        let (plan, request) = self.with_session(|session| {
            let plan = Consolidation::new(session.project(), &clip_ids)?;
            let folder = Path::new(session.path()).join("media");
            std::fs::create_dir_all(&folder)
                .map_err(|error| format!("could not create {}: {error}", folder.display()))?;
            let output = (1..)
                .map(|n| folder.join(format!("Compound {n}.mp4")))
                .find(|path| !path.exists())
                .unwrap_or_else(|| folder.join("Compound.mp4"));
            let mut request =
                plan.request(session.project(), Path::new(session.path()), &output);
            // the phone's encoder is told a bitrate, not a quality
            request.rate_mode = RateMode::Cbr;
            request.bitrate_kbps = crate::export::suggested_bitrate_kbps(
                request.width,
                request.height,
                (request.rate_num as f64 / request.rate_den.max(1) as f64).round() as u32,
            )
            .saturating_mul(2);
            Ok((plan, request))
        })?;
        std::thread::Builder::new()
            .name("capcraft-compound".to_owned())
            .spawn(move || {
                let cancel = job.cancel_handle();
                let mut last = -1.0f32;
                let made = concat_host::export::run(&request, &cancel, |progress: Progress| {
                    let fraction = fraction_of(progress);
                    if fraction - last >= 0.01 || fraction >= 1.0 {
                        last = fraction;
                        listener.on_progress(fraction);
                    }
                })
                .map_err(MobileError::Failed)
                .and_then(|path| {
                    let item = concat_host::media::probe(&path)?.to_new_media();
                    let command = self
                        .with_session(|session| Ok(plan.replacement(session.project(), item)?))?;
                    self.run(vec![command])
                });
                match made {
                    Ok(view) => listener.on_done(view),
                    Err(error) => {
                        let _ = std::fs::remove_file(&request.output);
                        listener.on_failed(error.to_string(), job.cancelled());
                    }
                }
                drop(job);
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
    }
}
