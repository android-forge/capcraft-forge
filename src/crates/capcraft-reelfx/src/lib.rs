// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! Forge's reel effects: 51 talking-head effects drawn on a clip's own
//! frames, around the person and the face tracked in them.
//!
//! Ported from Vibha Ramprakash's [claude-video-effects] (MIT; see
//! `LICENSE-claude-video-effects.txt`), whose `fx.py` draws each effect onto
//! a frame with PIL and NumPy. The effects keep its names, timing and look;
//! the drawing is Rust ([`canvas`], [`ctx`]) and runs inside the engine, so
//! the preview and the export draw the same frames. What imitated another
//! company's interface or name (a social app's comment bar and grid, a
//! phone's messages and switches, an assistant's name in the defaults) is
//! drawn as a plain card with neutral words instead.
//!
//! [claude-video-effects]: https://github.com/Vibha-Ramprakash/claude-video-effects

pub mod canvas;
pub mod ctx;
mod effects;

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub use ctx::Ctx;
use tiny_skia::Pixmap;

use canvas::Rgb;

/// `fx.py`'s palette.
pub const CREAM: Rgb = (244, 233, 216);
pub const INK: Rgb = (22, 22, 24);
pub const PEACH: Rgb = (255, 183, 165);
pub const YELLOW: Rgb = (255, 214, 10);
pub const GREEN: Rgb = (34, 197, 94);
pub const BLUE: Rgb = (10, 132, 255);
pub const PURPLE: Rgb = (124, 92, 255);
pub const ORANGE: Rgb = (217, 119, 87);
pub const RED: Rgb = (238, 34, 34);

/// One effect: its name, what it does, the settings it reads, and how it
/// draws a frame.
pub struct Effect {
    /// The name a plan and a clip carry, as `fx.py` names it.
    pub name: &'static str,
    /// What the effect browser calls it.
    pub title: &'static str,
    /// One line on what it does.
    pub desc: &'static str,
    /// The settings it reads, `name: what` joined by `; `.
    pub options: &'static str,
    /// It draws pictures from the clip (or a folder of the user's own).
    pub cards: bool,
    /// Draws the effect at `run`'s moment onto `c`.
    pub draw: fn(&mut Ctx, &mut Run),
}

/// Every effect, in `fx.py`'s order.
pub fn effects() -> &'static [Effect] {
    effects::ALL
}

/// Other names the effects answer to, as `fx.py`'s `ALIASES`.
const ALIASES: &[(&str, &str)] = &[
    ("the-noise", "noise"), ("the-lineup", "lineup"), ("bill", "the-bill"), ("to-do", "todo"), ("the-todo", "todo"),
    ("the-to-do", "todo"), ("the-tap", "tap"), ("follow", "tap"), ("comment", "comment-box"), ("pause", "pause-point"),
    ("pause-and-point", "pause-point"), ("backdrop", "backdrop-word"), ("stamp", "reject-stamp"), ("countdown", "countdown-flip"),
    ("cover", "cover-star"), ("rain", "shortcut-rain"), ("glass", "glass-widgets"), ("widgets", "glass-widgets"),
    ("collage", "red-collage"), ("split", "split-headline"), ("pastel", "pastel-word"), ("pills", "check-pills"),
    ("auto-edit", "timeline"), ("headline", "headline-box"), ("wall", "profile-wall"), ("tiers", "tier-list"),
    ("tier", "tier-list"), ("broll", "broll-split"), ("b-roll", "broll-split"), ("fan", "phone-fan"), ("dots", "dot-reveal"),
    ("headlines", "headline-wall"), ("tasks", "checklist"), ("liquid-glass", "liquid-glass-graph"),
    ("glass-graph", "liquid-glass-graph"), ("carousel", "carousel-spin"), ("match", "match-cut"),
    ("toggles", "apple-toggles"), ("switches", "apple-toggles"), ("led", "led-sign"), ("push", "slow-push"),
    ("push-in", "slow-push"), ("ots", "over-the-shoulder-zoom"), ("shoulder-zoom", "over-the-shoulder-zoom"),
    ("corner-zoom", "over-the-shoulder-zoom"), ("tape", "tape-label"), ("select", "select-resize"), ("resize", "select-resize"),
    ("trim", "wave-trim"), ("strike", "huge-strike"), ("crayon", "crayon-write"), ("deal", "fan-build"),
    ("notifications", "messages"), ("texts", "messages"), ("grid", "place-in-grid"), ("profile-grid", "place-in-grid"),
];

/// Named looks of another effect, as `fx.py`'s `LOOK_NAMES`.
const LOOKS: &[(&str, &str, &str)] = &[("glitter-spotlight", "spotlight", "sparkle"), ("glitter", "spotlight", "sparkle")];

/// The effect `name` means, through the aliases and the named looks, and
/// the look it brings with it.
pub fn find(name: &str) -> Option<(&'static Effect, Option<&'static str>)> {
    let key = name.trim().trim_start_matches('/').to_ascii_lowercase();
    if let Some((_, base, look)) = LOOKS.iter().find(|(n, _, _)| *n == key) {
        return effects().iter().find(|e| e.name == *base).map(|e| (e, Some(*look)));
    }
    let key = ALIASES.iter().find(|(a, _)| *a == key).map_or(key.as_str(), |(_, n)| n);
    effects().iter().find(|e| e.name == key).map(|e| (e, None))
}

/// One effect over a stretch of a clip, with what it keeps from frame to
/// frame: `fx.py`'s `Fx`.
pub struct Run {
    pub effect: &'static Effect,
    /// When it begins and ends, in seconds of the clip.
    pub start: f32,
    pub end: f32,
    /// How long it lasts.
    pub d: f32,
    /// Its settings.
    pub o: BTreeMap<String, String>,
    /// Pictures the card effects lay out: the user's own, or the clip's.
    /// How hard it is drawn, as `(seconds, 0..=1)` points to join; empty
    /// is full strength.
    pub strength: Vec<(f32, f32)>,
    pub cards: Arc<Vec<Pixmap>>,
    state: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl Run {
    /// The run of `name` from `start` to `end` with `options`; `None` for an
    /// effect this build does not have.
    pub fn new(name: &str, start: f32, end: f32, options: BTreeMap<String, String>) -> Option<Run> {
        let (effect, look) = find(name)?;
        let mut o = options;
        if let Some(look) = look {
            o.entry("look".to_owned()).or_insert_with(|| look.to_owned());
        }
        Some(Run {
            effect,
            start,
            end,
            d: (end - start).max(0.05),
            o,
            cards: Arc::new(Vec::new()),
            strength: Vec::new(),
            state: HashMap::new(),
        })
    }

    /// Whether the run is on at `t` seconds of the clip.
    pub fn covers(&self, t: f32) -> bool {
        t >= self.start && t < self.end
    }

    /// The setting `key`, or `default`.
    pub fn s(&self, key: &str, default: &str) -> String {
        self.o.get(key).cloned().unwrap_or_else(|| default.to_owned())
    }

    /// How hard it is drawn at `t`.
    pub fn strength_at(&self, t: f32) -> f32 {
        let p = &self.strength;
        let Some(first) = p.first() else { return 1.0 };
        if t <= first.0 {
            return first.1;
        }
        for w in p.windows(2) {
            let ((a, x), (b, y)) = (w[0], w[1]);
            if t <= b {
                return if b > a { x + (y - x) * (t - a) / (b - a) } else { y };
            }
        }
        p[p.len() - 1].1
    }

    /// The setting `key` as a number, or `default`.
    pub fn f(&self, key: &str, default: f32) -> f32 {
        self.o.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(default)
    }

    /// Whether `key` is set.
    pub fn has(&self, key: &str) -> bool {
        self.o.get(key).is_some_and(|v| !v.is_empty())
    }

    /// The setting `key` as a `#rrggbb` colour, or `default`.
    pub fn color(&self, key: &str, default: Rgb) -> Rgb {
        self.o.get(key).and_then(|v| hex(v)).unwrap_or(default)
    }

    /// The value kept under `key`, made by `make` the first time.
    pub fn cache<T: Any + Send + Sync>(&mut self, key: &str, make: impl FnOnce() -> T) -> Arc<T> {
        if let Some(found) = self.state.get(key).and_then(|v| Arc::clone(v).downcast::<T>().ok()) {
            return found;
        }
        let made = Arc::new(make());
        self.state.insert(key.to_owned(), Arc::clone(&made) as Arc<dyn Any + Send + Sync>);
        made
    }

    /// [`Run::cache`] with a maker that reads the run itself (its cards,
    /// its settings).
    pub fn cache_with<T: Any + Send + Sync>(&mut self, key: &str, make: impl FnOnce(&Run) -> T) -> Arc<T> {
        if let Some(found) = self.state.get(key).and_then(|v| Arc::clone(v).downcast::<T>().ok()) {
            return found;
        }
        let made = Arc::new(make(self));
        self.state.insert(key.to_owned(), Arc::clone(&made) as Arc<dyn Any + Send + Sync>);
        made
    }

    /// Forgets what was kept: the next frame builds it again.
    pub fn reset(&mut self) {
        self.state.clear();
    }

    /// Fading out over the last `t` seconds.
    pub fn fade(&self, c: &Ctx, t: f32) -> f32 {
        ctx::clamp((self.d - c.lt) / t)
    }

    /// `fx.py`'s `ramp`: in over `t_in`, out over `t_out` (0 for no out).
    pub fn ramp(&self, c: &Ctx, t_in: f32, t_out: f32) -> f32 {
        let i = ctx::clamp(c.lt / t_in);
        if t_out > 0.0 { i * ctx::clamp((self.d - c.lt) / t_out) } else { i }
    }
}

/// A pop from `t0` over `dur` (`fx.py`'s `pop`).
pub fn pop(c: &Ctx, t0: f32, dur: f32) -> f32 {
    if c.lt > t0 { ctx::ease_back((c.lt - t0) / dur) } else { 0.0 }
}

/// `#rrggbb` (or `rrggbb`) as a colour.
pub fn hex(v: &str) -> Option<Rgb> {
    let v = v.trim().trim_start_matches('#');
    if v.len() < 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&v[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

/// Where the face is, as fractions of the frame: its centre across and
/// down, and its width across.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Face {
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

/// Draws the runs on at `t` (seconds of the clip) onto `rgba` — a `w × h`
/// frame, four bytes a pixel, its alpha left alone — with the person mask
/// `mask` (`w × h`, 0 to 1; `None` is nobody) and the face where `face`
/// says (`None`: a third of the way down the middle). Returns whether
/// anything was drawn.
pub fn draw(rgba: &mut [u8], w: usize, h: usize, mask: Option<&[f32]>, face: Option<Face>, runs: &mut [Run], t: f32) -> bool {
    if w == 0 || h == 0 || !runs.iter().any(|r| r.covers(t)) {
        return false;
    }
    let mut img = Vec::with_capacity(w * h * 3);
    for px in rgba.chunks_exact(4) {
        img.extend(px[..3].iter().map(|&v| f32::from(v) / 255.0));
    }
    let face = face.unwrap_or(Face { x: 0.5, y: 0.33, w: 0.15 });
    let mut c = Ctx {
        w,
        h,
        img,
        m: mask.map_or_else(|| vec![0.0; w * h], <[f32]>::to_vec),
        fx: face.x * w as f32,
        fy: face.y * h as f32,
        fw: (face.w * w as f32).max(w as f32 * 0.05),
        lt: 0.0,
        t,
    };
    for run in runs.iter_mut().filter(|r| r.covers(t)) {
        c.lt = t - run.start;
        let k = run.strength_at(t).clamp(0.0, 1.0);
        if k <= 0.0 {
            continue;
        }
        // drawn in full, then laid over what was there by its strength
        let before = (k < 0.999).then(|| c.img.clone());
        (run.effect.draw)(&mut c, run);
        if let Some(before) = before {
            for (v, o) in c.img.iter_mut().zip(&before) {
                *v = o + (*v - o) * k;
            }
        }
    }
    for (px, v) in rgba.chunks_exact_mut(4).zip(c.img.chunks_exact(3)) {
        for i in 0..3 {
            px[i] = (v[i].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    true
}
