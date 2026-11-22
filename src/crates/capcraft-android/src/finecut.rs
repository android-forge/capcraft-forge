// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Fine cutout: branches, wires and the like taken from what is behind them in
//! a photo (see `concat_vision::fine`), tried small as it is set and then cut
//! at the photo's own size.

use std::sync::{Arc, Mutex, OnceLock};

use concat_core::frame::Frame;
use concat_project::Command;
use concat_vision::fine::{self, Background, Options};
use concat_vision::window::{self, Rect};

use crate::objects::{keep, photo, shown};
use crate::session::{MobileSession, TimelineView};
use crate::{MobileError, Picture};

/// How a fine cutout is made.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FineCut {
    /// How faint a thing is kept, 0 to 1.
    pub detail: f32,
    /// How much noise and loose specks are cleared, 0 to 1.
    pub clean: f32,
    /// One colour for what is kept, `0xRRGGBB`, or less than 0 for its own.
    pub ink: i32,
    /// A place in the background, in fractions of the picture, or less than 0
    /// for the bright sky behind.
    pub back_x: f32,
    /// How far down it is.
    pub back_y: f32,
}

fn options(cut: &FineCut) -> Options {
    Options {
        detail: cut.detail.clamp(0.0, 1.0),
        clean: cut.clean.clamp(0.0, 1.0),
        ink: (cut.ink >= 0).then(|| [16, 8, 0].map(|by| (cut.ink >> by) as u8)),
        back: (cut.back_x >= 0.0 && cut.back_y >= 0.0)
            .then(|| [cut.back_x.min(1.0), cut.back_y.min(1.0)]),
    }
}

/// The photo tried last, small, and its background: moving a slider does not
/// work it out again.
struct Tried {
    path: String,
    back: Option<[f32; 2]>,
    small: Frame,
    background: Arc<Background>,
}

fn tried() -> &'static Mutex<Option<Tried>> {
    static ONE: OnceLock<Mutex<Option<Tried>>> = OnceLock::new();
    ONE.get_or_init(Mutex::default)
}

/// Forgets the photo tried, for when the panel closes.
#[uniffi::export]
pub fn release_fine() {
    if let Ok(mut held) = tried().lock() {
        *held = None;
    }
}

#[uniffi::export]
impl MobileSession {
    /// What `cut` would leave of photo `clip_id`, `side` pixels along its
    /// longer side.
    pub fn fine_preview(
        &self,
        clip_id: String,
        cut: FineCut,
        side: u32,
    ) -> Result<Picture, MobileError> {
        let shown = shown(self, &clip_id)?;
        let options = options(&cut);
        let mut held = tried()
            .lock()
            .map_err(|_| MobileError::Failed("the cutout is unwell".to_owned()))?;
        let same = held
            .as_ref()
            .is_some_and(|t| t.path == shown.path && t.back == options.back);
        if !same {
            let whole = photo(&shown.path).map_err(MobileError::Failed)?;
            let (w, h) = (whole.width() as f32, whole.height() as f32);
            let scale = (side.clamp(64, 2048) as f32 / w.max(h)).min(1.0);
            let small = window::frame(
                &whole,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: w,
                    height: h,
                },
                ((w * scale) as u32).max(2),
                ((h * scale) as u32).max(2),
            );
            let background = Arc::new(fine::background(&small, &options));
            *held = Some(Tried {
                path: shown.path.clone(),
                back: options.back,
                small,
                background,
            });
        }
        let tried = held
            .as_ref()
            .ok_or_else(|| MobileError::Failed("no photo".to_owned()))?;
        Ok(Picture::of(fine::cut(
            &tried.small,
            &tried.background,
            &options,
        )))
    }

    /// Cuts photo `clip_id` at its own size, as one undo step: a new picture
    /// in the project, shown in its place.
    pub fn fine_cut(&self, clip_id: String, cut: FineCut) -> Result<TimelineView, MobileError> {
        let shown = shown(self, &clip_id)?;
        let options = options(&cut);
        let whole = photo(&shown.path).map_err(MobileError::Failed)?;
        let background = fine::background(&whole, &options);
        let cut = fine::cut(&whole, &background, &options);
        let path = keep(&shown, "fine", cut).map_err(MobileError::Failed)?;
        let item = concat_host::media::probe(&path)?.to_new_media();
        self.run(vec![Command::ReplaceClipMedia {
            clip_id,
            item,
            source_start: Some(0.0),
            enhanced: false,
            offset: 0.0,
        }])
    }
}
