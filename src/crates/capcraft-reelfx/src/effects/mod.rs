// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! The 51 effects, in `fx.py`'s order, and the pieces several share.

mod pack1;
mod pack2;
mod pack3;

use tiny_skia::Pixmap;

use crate::Effect;
use crate::canvas::{self, blank, over, shadowed};
use crate::ctx::{Grade, grade_px};

pub(crate) static ALL: &[Effect] = &[
    pack1::NOISE, pack1::LINEUP, pack1::LOCKED_ON, pack1::ORBIT, pack1::LEAN_IN, pack1::BACKDROP_WORD,
    pack1::SPOTLIGHT, pack1::THE_BILL, pack1::MOODY, pack1::SWIPE, pack1::TODO, pack1::COMMENT_BOX, pack1::TAP,
    pack1::PAUSE_POINT, pack1::UNINSTALL, pack1::REEL_FRAME, pack1::SHORTCUT_RAIN, pack1::REJECT_STAMP,
    pack1::COUNTDOWN_FLIP, pack1::RABBIT_HOLE, pack1::COVER_STAR, pack1::SCRIBBLE,
    pack2::GLASS_WIDGETS, pack2::RED_COLLAGE, pack2::SPLIT_HEADLINE, pack2::PASTEL_WORD, pack2::CHECK_PILLS,
    pack2::TIMELINE, pack2::HEADLINE_BOX, pack2::PROFILE_WALL, pack2::TIER_LIST, pack2::BROLL_SPLIT,
    pack2::PHONE_FAN, pack2::DOT_REVEAL, pack2::HEADLINE_WALL, pack2::CHECKLIST, pack2::LIQUID_GLASS_GRAPH,
    pack3::CAROUSEL_SPIN, pack3::MATCH_CUT, pack3::APPLE_TOGGLES, pack3::LED_SIGN, pack3::SLOW_PUSH,
    pack3::OVER_THE_SHOULDER_ZOOM, pack3::MESSAGES, pack3::PLACE_IN_GRID, pack3::TAPE_LABEL, pack3::SELECT_RESIZE,
    pack3::WAVE_TRIM, pack3::HUGE_STRIKE, pack3::CRAYON_WRITE, pack3::FAN_BUILD,
];

/// `im` with each pixel graded `kind`.
pub(crate) fn graded(im: &Pixmap, kind: Grade) -> Pixmap {
    let mut out = im.clone();
    for px in out.data_mut().chunks_exact_mut(4) {
        let a = f32::from(px[3]).max(1.0);
        let c = [f32::from(px[0]) / a, f32::from(px[1]) / a, f32::from(px[2]) / a];
        let g = grade_px(c, kind);
        for i in 0..3 {
            px[i] = (g[i] * a + 0.5) as u8;
        }
    }
    out
}

/// Each card its own look, so a pile of frames reads as different reels
/// (`fx.py`'s `vary`).
pub(crate) fn vary(im: &Pixmap, i: usize) -> Pixmap {
    graded(im, [Grade::Pop, Grade::Bw, Grade::Warm, Grade::Cool, Grade::Moody][i % 5])
}

/// `src` cropped to cover a `w × h` box, `top` of the spare height above.
pub(crate) fn cover(src: &Pixmap, w: f32, h: f32, top: f32) -> Pixmap {
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let s = (w / sw).max(h / sh);
    let (rw, rh) = ((sw * s).max(w), (sh * s).max(h));
    let mut out = blank(w, h);
    canvas::over_scaled(&mut out, src, -(rw - w) / 2.0, -(rh - h) * top, rw, rh);
    out
}

/// `im` with its corners rounded to `r`.
pub(crate) fn round_corners(im: &mut Pixmap, r: f32) {
    let mut m = blank(im.width() as f32, im.height() as f32);
    canvas::rounded(&mut m, 0.0, 0.0, im.width() as f32 - 1.0, im.height() as f32 - 1.0, r, Some((255, 255, 255, 255)), None);
    let alpha: Vec<u8> = m.data().chunks_exact(4).map(|p| p[3]).collect();
    canvas::mask_alpha(im, &alpha);
}

/// A reel-shaped (9:16) rounded card with a soft shadow (`fx.py`'s `card`).
pub(crate) fn card(im: &Pixmap, w: f32) -> Pixmap {
    let w = w.max(4.0).floor();
    let h = (w * 16.0 / 9.0).floor();
    let mut src = cover(im, w, h, 0.5);
    round_corners(&mut src, w * 0.07);
    canvas::rounded(&mut src, 0.0, 0.0, w - 1.0, h - 1.0, w * 0.07, None, Some(((0, 0, 0, 70), 2.0)));
    shadowed(&src, (w / 16.0).max(6.0), w / 22.0, 150)
}

/// The cards of a run: the user's pictures as they are, or the clip's own
/// frames each given a look, at `w` wide.
pub(crate) fn cards(e: &crate::Run, w: f32) -> Vec<Pixmap> {
    let own = e.has("images");
    e.cards
        .iter()
        .enumerate()
        .map(|(i, im)| card(&if own { im.clone() } else { vary(im, i) }, w))
        .collect()
}

/// A grid of the cards, `cols × rows` of `cell`-wide 4:5 tiles
/// (`fx.py`'s `thumb_grid`).
pub(crate) fn thumb_grid(cards: &[Pixmap], cols: usize, rows: usize, cell: f32, gap: f32, i0: usize, look: bool) -> Pixmap {
    let ch = (cell * 1.25).floor();
    let mut im = blank(cols as f32 * cell + (cols - 1) as f32 * gap, rows as f32 * ch + (rows - 1) as f32 * gap);
    if cards.is_empty() {
        return im;
    }
    for k in 0..cols * rows {
        let src = &cards[(i0 + k) % cards.len()];
        let src = if look { vary(src, i0 + k) } else { src.clone() };
        let mut t = cover(&src, cell, ch, 0.3);
        round_corners(&mut t, (cell / 12.0).max(4.0));
        over(&mut im, &t, ((k % cols) as f32 * (cell + gap)) as i32, ((k / cols) as f32 * (ch + gap)) as i32);
    }
    im
}

/// A small deterministic random stream, so an effect lays out the same way
/// every frame and every export (Python's `random.Random(seed)` in spirit).
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    pub(crate) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    pub(crate) fn uniform(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// `s` split on `|`.
pub(crate) fn parts(s: &str) -> Vec<String> {
    s.split('|').map(str::to_owned).collect()
}
