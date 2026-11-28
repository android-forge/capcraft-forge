// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! Pictures the effects draw and lay over a frame: words, rounded cards,
//! lines and their shadows, as premultiplied RGBA pixmaps.
//!
//! The helpers keep the names and the sense of the ones in
//! claude-video-effects' `fx.py` (PIL's `ImageDraw`, `ImageFilter`), so an
//! effect reads the same here as there: sizes are pixels of the frame,
//! colours are straight `(r, g, b, a)` bytes, and text is set from the top
//! of its ascender unless an anchor says otherwise.

use std::sync::OnceLock;

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect,
    Stroke, Transform,
};

/// A straight-alpha colour.
pub type Rgba = (u8, u8, u8, u8);
/// A colour with no alpha of its own.
pub type Rgb = (u8, u8, u8);

/// Opaque `rgb`.
pub fn op(rgb: Rgb) -> Rgba {
    (rgb.0, rgb.1, rgb.2, 255)
}

/// `rgb` at alpha `a`.
pub fn with_a(rgb: Rgb, a: u8) -> Rgba {
    (rgb.0, rgb.1, rgb.2, a)
}

fn paint(color: Rgba) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(color.0, color.1, color.2, color.3));
    paint.anti_alias = true;
    paint
}

/// A transparent pixmap, at least one pixel a side.
pub fn blank(w: f32, h: f32) -> Pixmap {
    Pixmap::new(w.max(1.0) as u32, h.max(1.0) as u32).expect("a pixmap")
}

// ── fonts ───────────────────────────────────────────────────────────────

/// The faces the effects name, as `fx.py` does: `bold` Archivo Black, `serif`
/// Instrument Serif Italic, `mono` Space Mono Bold, `hand` Permanent Marker,
/// `ui`/`uim`/`uib` Inter at three weights, `anton`, the Montserrats and the
/// Playfair Display "didone" faces.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Bold,
    Serif,
    Mono,
    Hand,
    Ui,
    Uim,
    Uib,
    Anton,
    Mont,
    Montb,
    Didone,
    Didoneb,
    Didonei,
    /// One of the app's own fonts ([`concat_text::BASE_FONTS`]), by its place there.
    Base(u16),
}

impl Kind {
    /// The kind `fx.py` names `name`; Archivo Black for anything unknown.
    pub fn named(name: &str) -> Kind {
        match name {
            "serif" => Kind::Serif,
            "mono" => Kind::Mono,
            "hand" => Kind::Hand,
            "ui" => Kind::Ui,
            "uim" => Kind::Uim,
            "uib" => Kind::Uib,
            "anton" => Kind::Anton,
            "mont" | "monts" => Kind::Mont,
            "montb" => Kind::Montb,
            "didone" => Kind::Didone,
            "didoneb" => Kind::Didoneb,
            "didonei" => Kind::Didonei,
            _ => base_names().iter().position(|n| n.eq_ignore_ascii_case(name)).map_or(Kind::Bold, |i| Kind::Base(i as u16)),
        }
    }
}

static SPACE_MONO: &[u8] = include_bytes!("../fonts/SpaceMono-Bold.ttf");
static PLAYFAIR: &[u8] = include_bytes!("../fonts/PlayfairDisplay.ttf");
static PLAYFAIR_ITALIC: &[u8] = include_bytes!("../fonts/PlayfairDisplay-Italic.ttf");

/// One face, ready to draw: parsed once, its variable weight set.
pub struct Face {
    face: ttf_parser::Face<'static>,
    units: f32,
    ascent: f32,
    descent: f32,
}

fn load(bytes: &'static [u8], weight: Option<f32>) -> Face {
    let mut face = ttf_parser::Face::parse(bytes, 0).expect("a bundled font parses");
    if let Some(weight) = weight {
        let _ = face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), weight);
    }
    let units = f32::from(face.units_per_em());
    Face {
        ascent: f32::from(face.ascender()) / units,
        descent: -f32::from(face.descender()) / units,
        units,
        face,
    }
}

/// The app's own fonts by their full names, in [`concat_text::BASE_FONTS`]'s order.
pub fn base_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        concat_text::BASE_FONTS
            .iter()
            .map(|bytes| {
                ttf_parser::Face::parse(bytes, 0)
                    .ok()
                    .and_then(|f| {
                        f.names()
                            .into_iter()
                            .find(|n| n.name_id == ttf_parser::name_id::FULL_NAME && n.is_unicode())
                            .and_then(|n| n.to_string())
                    })
                    .unwrap_or_default()
            })
            .collect()
    })
}

/// The face of `kind`.
pub fn face(kind: Kind) -> &'static Face {
    if let Kind::Base(i) = kind {
        static BASE: OnceLock<Vec<Face>> = OnceLock::new();
        let base = BASE.get_or_init(|| concat_text::BASE_FONTS.iter().map(|b| load(b, None)).collect());
        return base.get(usize::from(i)).unwrap_or_else(|| face(Kind::Bold));
    }
    static FACES: OnceLock<Vec<Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| {
        vec![
            load(concat_text::ARCHIVO_BLACK, None),
            load(concat_text::INSTRUMENT_SERIF_ITALIC, None),
            load(SPACE_MONO, None),
            load(concat_text::PERMANENT_MARKER, None),
            load(concat_text::INTER_SEMIBOLD, None),
            load(concat_text::INTER_REGULAR, None),
            load(concat_text::INTER_BOLD, None),
            load(concat_text::ANTON, None),
            load(concat_text::MONTSERRAT_BOLD, None),
            load(concat_text::MONTSERRAT_EXTRABOLD, None),
            load(PLAYFAIR, Some(500.0)),
            load(PLAYFAIR, Some(800.0)),
            load(PLAYFAIR_ITALIC, Some(500.0)),
        ]
    });
    let i = match kind {
        Kind::Bold => 0,
        Kind::Serif => 1,
        Kind::Mono => 2,
        Kind::Hand => 3,
        Kind::Ui => 4,
        Kind::Uim => 5,
        Kind::Uib => 6,
        Kind::Anton => 7,
        Kind::Mont => 8,
        Kind::Montb => 9,
        Kind::Didone => 10,
        Kind::Didoneb => 11,
        Kind::Didonei | Kind::Base(_) => 12,
    };
    &faces[i]
}

struct Outliner<'a> {
    builder: &'a mut PathBuilder,
    scale: f32,
    x: f32,
    y: f32,
}

impl ttf_parser::OutlineBuilder for Outliner<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(self.x + x * self.scale, self.y - y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(self.x + x * self.scale, self.y - y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quad_to(
            self.x + x1 * self.scale,
            self.y - y1 * self.scale,
            self.x + x * self.scale,
            self.y - y * self.scale,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_to(
            self.x + x1 * self.scale,
            self.y - y1 * self.scale,
            self.x + x2 * self.scale,
            self.y - y2 * self.scale,
            self.x + x * self.scale,
            self.y - y * self.scale,
        );
    }
    fn close(&mut self) {
        self.builder.close();
    }
}

impl Face {
    fn advance(&self, ch: char, size: f32) -> f32 {
        let glyph = self.face.glyph_index(ch).or_else(|| self.face.glyph_index('?'));
        glyph
            .and_then(|g| self.face.glyph_hor_advance(g))
            .map_or(size * 0.5, |a| f32::from(a) * size / self.units)
    }

    /// How wide `s` is at `size` pixels to the em (PIL's `getlength`).
    pub fn length(&self, s: &str, size: f32) -> f32 {
        s.chars().map(|ch| self.advance(ch, size)).sum()
    }

    /// The ascender and descender at `size`, both positive.
    pub fn metrics(&self, size: f32) -> (f32, f32) {
        (self.ascent * size, self.descent * size)
    }

    /// `s` as one path with its baseline's left end at `(x, y)`.
    pub fn path(&self, s: &str, size: f32, x: f32, y: f32) -> Option<Path> {
        let mut builder = PathBuilder::new();
        let scale = size / self.units;
        let mut pen = x;
        for ch in s.chars() {
            if let Some(glyph) = self.face.glyph_index(ch).or_else(|| self.face.glyph_index('?')) {
                self.face.outline_glyph(
                    glyph,
                    &mut Outliner {
                        builder: &mut builder,
                        scale,
                        x: pen,
                        y,
                    },
                );
            }
            pen += self.advance(ch, size);
        }
        builder.finish()
    }
}

/// How wide `s` is in `kind` at `size`.
pub fn length(kind: Kind, size: f32, s: &str) -> f32 {
    face(kind).length(s, size)
}

/// Where a string's anchor point is, as PIL names them: the first letter
/// across (`l` left, `m` middle, `r` right), the second down (`a` top of the
/// ascender, `m` between ascender and descender, `s` baseline, `d` foot of
/// the descender).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Anchor(pub u8, pub u8);

impl Anchor {
    pub const LA: Anchor = Anchor(b'l', b'a');
    pub const LM: Anchor = Anchor(b'l', b'm');
    pub const MM: Anchor = Anchor(b'm', b'm');
    pub const RM: Anchor = Anchor(b'r', b'm');
    pub const RA: Anchor = Anchor(b'r', b'a');
    pub const MA: Anchor = Anchor(b'm', b'a');
}

/// Draws `s` onto `pm` at `(x, y)` by `anchor`, in `fill`, with an outline
/// of `stroke` pixels each side of the letters in `stroke_fill`.
#[allow(clippy::too_many_arguments)]
pub fn text_at(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    s: &str,
    kind: Kind,
    size: f32,
    fill: Rgba,
    anchor: Anchor,
    stroke: f32,
    stroke_fill: Rgba,
) {
    let f = face(kind);
    let (ascent, descent) = f.metrics(size);
    let w = f.length(s, size);
    let x0 = match anchor.0 {
        b'm' => x - w / 2.0,
        b'r' => x - w,
        _ => x,
    };
    let baseline = match anchor.1 {
        b'm' => y + (ascent - descent) / 2.0,
        b's' => y,
        b'd' => y - descent,
        _ => y + ascent,
    };
    let Some(path) = f.path(s, size, x0, baseline) else {
        return;
    };
    if stroke > 0.0 {
        let outline = Stroke {
            width: stroke * 2.0,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint(stroke_fill), &outline, Transform::identity(), None);
    }
    pm.fill_path(&path, &paint(fill), FillRule::Winding, Transform::identity(), None);
}

/// [`text_at`] with no outline.
pub fn text(pm: &mut Pixmap, x: f32, y: f32, s: &str, kind: Kind, size: f32, fill: Rgba, anchor: Anchor) {
    text_at(pm, x, y, s, kind, size, fill, anchor, 0.0, (0, 0, 0, 0));
}

/// A word as a picture of its own, as `fx.py`'s `text_img`: padded all
/// round, with an optional outline, soft shadow and glow.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub stroke: f32,
    pub stroke_fill: Rgb,
    pub shadow: f32,
    pub glow: Option<(Rgb, f32)>,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            stroke: 0.0,
            stroke_fill: crate::INK,
            shadow: 0.0,
            glow: None,
        }
    }
}

/// `s` at `size` in `kind` and `fill`, drawn `look`.
pub fn text_img(s: &str, size: f32, kind: Kind, fill: Rgb, look: Look) -> Pixmap {
    let f = face(kind);
    let (ascent, descent) = f.metrics(size);
    let pad = (size * 0.2 + look.shadow * 2.5 + look.glow.map_or(0.0, |g| g.1 * 2.5)) as i32 as f32 + 4.0;
    // the ink's own box, as PIL's getbbox gives it, from the top of the ascender
    let (bx0, by0, bx1, by1) = match f.path(s, size, 0.0, ascent).map(|p| p.bounds()) {
        Some(b) => (b.left() - look.stroke, b.top() - look.stroke, b.right() + look.stroke, b.bottom() + look.stroke),
        None => (0.0, 0.0, f.length(s, size), ascent + descent),
    };
    let w = (bx1 - bx0).max(1.0) + 2.0 * pad;
    let h = (by1 - by0).max(1.0) + 2.0 * pad;
    let (x, y) = (pad - bx0, pad - by0);
    let mut im = blank(w, h);
    if look.shadow > 0.0 {
        let mut sh = blank(w, h);
        text_at(&mut sh, x, y + look.shadow * 0.5, s, kind, size, (0, 0, 0, 160), Anchor::LA, look.stroke, (0, 0, 0, 160));
        blur(&mut sh, look.shadow);
        over(&mut im, &sh, 0, 0);
    }
    if let Some((color, radius)) = look.glow {
        let mut g = blank(w, h);
        text_at(&mut g, x, y, s, kind, size, op(color), Anchor::LA, size * 0.04, op(color));
        blur(&mut g, radius);
        over(&mut im, &g, 0, 0);
        over(&mut im, &g, 0, 0);
    }
    text_at(&mut im, x, y, s, kind, size, op(fill), Anchor::LA, look.stroke, op(look.stroke_fill));
    im
}

/// The largest size up from `start`, shrinking by 8 % a step, at which `s`
/// is no wider than `max_w`, and the word drawn at it (`fx.py`'s `fit_text`).
pub fn fit_text(s: &str, max_w: f32, kind: Kind, start: f32, fill: Rgb, look: Look) -> Pixmap {
    let mut size = start;
    while size > 10.0 && length(kind, size, s) > max_w {
        size = (size * 0.92).floor();
    }
    text_img(s, size, kind, fill, look)
}

/// The size, shrinking by 6 % a step from `start`, at which the widest of
/// `lines` fits `max_w`.
pub fn lines_fit(lines: &[&str], max_w: f32, kind: Kind, start: f32) -> f32 {
    let mut size = start;
    while size > 10.0 && lines.iter().map(|l| length(kind, size, l)).fold(0.0, f32::max) > max_w {
        size = (size * 0.94).floor();
    }
    size
}

// ── shapes ──────────────────────────────────────────────────────────────

/// A rounded rectangle path from `(x0, y0)` to `(x1, y1)` with corner `r`.
pub fn rrect_path(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Option<Path> {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    let r = r.max(0.0).min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    if x1 - x0 < 0.5 || y1 - y0 < 0.5 {
        return None;
    }
    let k = 0.552_284_8 * r;
    let mut b = PathBuilder::new();
    b.move_to(x0 + r, y0);
    b.line_to(x1 - r, y0);
    b.cubic_to(x1 - r + k, y0, x1, y0 + r - k, x1, y0 + r);
    b.line_to(x1, y1 - r);
    b.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    b.line_to(x0 + r, y1);
    b.cubic_to(x0 + r - k, y1, x0, y1 - r + k, x0, y1 - r);
    b.line_to(x0, y0 + r);
    b.cubic_to(x0, y0 + r - k, x0 + r - k, y0, x0 + r, y0);
    b.close();
    b.finish()
}

/// Fills a rounded rectangle on `pm`, and outlines it when `outline` says.
pub fn rounded(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, fill: Option<Rgba>, outline: Option<(Rgba, f32)>) {
    let Some(path) = rrect_path(x0, y0, x1, y1, r) else {
        return;
    };
    if let Some(fill) = fill {
        pm.fill_path(&path, &paint(fill), FillRule::Winding, Transform::identity(), None);
    }
    if let Some((color, width)) = outline {
        let inset = rrect_path(x0 + width / 2.0, y0 + width / 2.0, x1 - width / 2.0, y1 - width / 2.0, (r - width / 2.0).max(0.0));
        if let Some(inset) = inset {
            let stroke = Stroke {
                width,
                ..Stroke::default()
            };
            pm.stroke_path(&inset, &paint(color), &stroke, Transform::identity(), None);
        }
    }
}

/// A `w` × `h` rounded rectangle on its own (`fx.py`'s `rrect`).
pub fn rrect(w: f32, h: f32, r: f32, fill: Rgba, outline: Option<(Rgba, f32)>) -> Pixmap {
    let mut im = blank(w, h);
    rounded(&mut im, 0.0, 0.0, w.floor(), h.floor(), r, Some(fill), outline);
    im
}

/// A filled rectangle.
pub fn rect(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, fill: Rgba) {
    if let Some(r) = Rect::from_ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)) {
        pm.fill_rect(r, &paint(fill), Transform::identity(), None);
    }
}

/// An ellipse in the box `(x0, y0)`–`(x1, y1)`, filled and/or outlined.
pub fn ellipse(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, fill: Option<Rgba>, outline: Option<(Rgba, f32)>) {
    let Some(r) = Rect::from_ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)) else {
        return;
    };
    let Some(path) = PathBuilder::from_oval(r) else {
        return;
    };
    if let Some(fill) = fill {
        pm.fill_path(&path, &paint(fill), FillRule::Winding, Transform::identity(), None);
    }
    if let Some((color, width)) = outline {
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
    }
}

/// A line through `pts`, round where it turns and at its ends.
pub fn line(pm: &mut Pixmap, pts: &[(f32, f32)], color: Rgba, width: f32) {
    if pts.len() < 2 {
        return;
    }
    let mut b = PathBuilder::new();
    b.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        b.line_to(p.0, p.1);
    }
    let Some(path) = b.finish() else {
        return;
    };
    let stroke = Stroke {
        width: width.max(1.0),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    pm.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
}

/// [`line`] with square ends, as PIL draws a plain `line`.
pub fn line_flat(pm: &mut Pixmap, pts: &[(f32, f32)], color: Rgba, width: f32) {
    if pts.len() < 2 {
        return;
    }
    let mut b = PathBuilder::new();
    b.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        b.line_to(p.0, p.1);
    }
    let Some(path) = b.finish() else {
        return;
    };
    let stroke = Stroke {
        width: width.max(1.0),
        ..Stroke::default()
    };
    pm.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
}

/// A closed shape through `pts`, filled and/or outlined.
pub fn polygon(pm: &mut Pixmap, pts: &[(f32, f32)], fill: Option<Rgba>, outline: Option<(Rgba, f32)>) {
    if pts.len() < 3 {
        return;
    }
    let mut b = PathBuilder::new();
    b.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        b.line_to(p.0, p.1);
    }
    b.close();
    let Some(path) = b.finish() else {
        return;
    };
    if let Some(fill) = fill {
        pm.fill_path(&path, &paint(fill), FillRule::Winding, Transform::identity(), None);
    }
    if let Some((color, width)) = outline {
        let stroke = Stroke {
            width,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
    }
}

/// An arc of the ellipse in the box, from `a0` to `a1` degrees clockwise
/// from three o'clock, as PIL's `arc`.
#[allow(clippy::too_many_arguments)]
pub fn arc(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, a0: f32, a1: f32, color: Rgba, width: f32) {
    let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let n = ((a1 - a0).abs() / 4.0).ceil().max(2.0) as usize;
    let pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect();
    line_flat(pm, &pts, color, width);
}

/// Lays `top` over `pm` with its top-left at `(x, y)`.
pub fn over(pm: &mut Pixmap, top: &Pixmap, x: i32, y: i32) {
    pm.draw_pixmap(x, y, top.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
}

/// [`over`] at `alpha`.
pub fn over_a(pm: &mut Pixmap, top: &Pixmap, x: i32, y: i32, alpha: f32) {
    let p = PixmapPaint {
        opacity: alpha.clamp(0.0, 1.0),
        ..PixmapPaint::default()
    };
    pm.draw_pixmap(x, y, top.as_ref(), &p, Transform::identity(), None);
}

/// `top` scaled to `w` × `h` over `pm` at `(x, y)`, smoothly.
pub fn over_scaled(pm: &mut Pixmap, top: &Pixmap, x: f32, y: f32, w: f32, h: f32) {
    let sx = w / top.width() as f32;
    let sy = h / top.height() as f32;
    let p = PixmapPaint {
        quality: tiny_skia::FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    pm.draw_pixmap(0, 0, top.as_ref(), &p, Transform::from_row(sx, 0.0, 0.0, sy, x, y), None);
}

/// A copy of `pm` resized to `w` × `h`.
pub fn resized(pm: &Pixmap, w: f32, h: f32) -> Pixmap {
    let mut out = blank(w, h);
    over_scaled(&mut out, pm, 0.0, 0.0, w.max(1.0).floor(), h.max(1.0).floor());
    out
}

/// The part of `pm` from `(x0, y0)`, `w` × `h` (transparent past its edges).
pub fn crop(pm: &Pixmap, x0: f32, y0: f32, w: f32, h: f32) -> Pixmap {
    let mut out = blank(w, h);
    over(&mut out, pm, -(x0.round() as i32), -(y0.round() as i32));
    out
}

/// Every pixel's alpha scaled by `k`.
pub fn fade(pm: &mut Pixmap, k: f32) {
    let k = k.clamp(0.0, 1.0);
    for px in pm.data_mut() {
        *px = (f32::from(*px) * k + 0.5) as u8;
    }
}

/// Every pixel's alpha multiplied by the matching byte of `mask` (0–255).
pub fn mask_alpha(pm: &mut Pixmap, mask: &[u8]) {
    for (px, &m) in pm.data_mut().chunks_exact_mut(4).zip(mask) {
        for c in px.iter_mut() {
            *c = ((u32::from(*c) * u32::from(m) + 127) / 255) as u8;
        }
    }
}

/// A Gaussian blur of `sigma` pixels, as three box blurs.
pub fn blur(pm: &mut Pixmap, sigma: f32) {
    if sigma < 0.5 {
        return;
    }
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let mut buf: Vec<f32> = pm.data().iter().map(|&v| f32::from(v)).collect();
    blur_f32(&mut buf, w, h, 4, sigma);
    for (px, v) in pm.data_mut().iter_mut().zip(&buf) {
        *px = v.clamp(0.0, 255.0).round() as u8;
    }
    // premultiplied: no channel may pass its alpha after rounding
    for px in pm.data_mut().chunks_exact_mut(4) {
        let a = px[3];
        for c in &mut px[..3] {
            *c = (*c).min(a);
        }
    }
}

/// The three box radii whose passes make a Gaussian of `sigma`.
fn boxes(sigma: f32) -> [usize; 3] {
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut wl = ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - 3.0 * (wl * wl) as f32 - 12.0 * wl as f32 - 9.0) / (-4.0 * wl as f32 - 4.0)).round() as i32;
    let mut out = [0usize; 3];
    for (i, r) in out.iter_mut().enumerate() {
        let size = if (i as i32) < m { wl } else { wu };
        *r = ((size.max(1) - 1) / 2) as usize;
    }
    out
}

/// A Gaussian blur of `sigma` over `buf`, `channels` values a pixel, with
/// the edge pixels carried out past the border.
pub fn blur_f32(buf: &mut [f32], w: usize, h: usize, channels: usize, sigma: f32) {
    if sigma < 0.5 || w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0f32; buf.len()];
    for r in boxes(sigma) {
        if r == 0 {
            continue;
        }
        box_pass(buf, &mut tmp, w, h, channels, r, true);
        box_pass(&tmp, buf, w, h, channels, r, false);
    }
}

fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, ch: usize, r: usize, across: bool) {
    let (lines, len) = if across { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| if across { (line * w + i) * ch } else { (i * w + line) * ch };
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut acc = vec![0f32; ch];
    for line in 0..lines {
        acc.iter_mut().for_each(|a| *a = 0.0);
        for k in 0..=2 * r {
            let i = (k as isize - r as isize).clamp(0, len as isize - 1) as usize;
            for c in 0..ch {
                acc[c] += src[at(line, i) + c];
            }
        }
        for i in 0..len {
            for c in 0..ch {
                dst[at(line, i) + c] = acc[c] * norm;
            }
            let out = (i as isize - r as isize).clamp(0, len as isize - 1) as usize;
            let inn = (i + r + 1).min(len - 1);
            for c in 0..ch {
                acc[c] += src[at(line, inn) + c] - src[at(line, out) + c];
            }
        }
    }
}

/// `im` over a soft black shadow `dy` pixels lower, `blur` wide and
/// `alpha` of 255 dark, on a canvas grown to hold it (`fx.py`'s `shadowed`).
pub fn shadowed(im: &Pixmap, blur_px: f32, dy: f32, alpha: u8) -> Pixmap {
    let pad = (blur_px * 2.0 + dy.abs()).ceil();
    let mut out = blank(im.width() as f32 + 2.0 * pad, im.height() as f32 + 2.0 * pad);
    let mut sh = im.clone();
    for px in sh.data_mut().chunks_exact_mut(4) {
        let a = (u32::from(px[3]) * u32::from(alpha) / 255) as u8;
        px[0] = 0;
        px[1] = 0;
        px[2] = 0;
        px[3] = a;
    }
    over(&mut out, &sh, pad as i32, (pad + dy) as i32);
    blur(&mut out, blur_px);
    over(&mut out, im, pad as i32, pad as i32);
    out
}

/// Straight RGB bytes of `pm`, alpha dropped (its colour under no alpha).
pub fn straight(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.width() as usize * pm.height() as usize * 3);
    for px in pm.data().chunks_exact(4) {
        let a = u32::from(px[3]).max(1);
        for c in &px[..3] {
            out.push(((u32::from(*c) * 255 + a / 2) / a).min(255) as u8);
        }
    }
    out
}

/// A pixmap of straight RGB bytes, opaque.
pub fn from_rgb(w: u32, h: u32, rgb: &[u8]) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("a pixmap");
    for (px, c) in pm.data_mut().chunks_exact_mut(4).zip(rgb.chunks_exact(3)) {
        px[0] = c[0];
        px[1] = c[1];
        px[2] = c[2];
        px[3] = 255;
    }
    pm
}

/// The heart `fx.py` draws, `size` pixels square.
pub fn heart(size: f32, color: Rgb) -> Pixmap {
    let s = size.max(2.0);
    let mut im = blank(s, s);
    let pts: Vec<(f32, f32)> = (0..60)
        .map(|i| {
            let t = i as f32 / 59.0 * std::f32::consts::TAU;
            (
                s / 2.0 + s * 0.48 * (16.0 * t.sin().powi(3)) / 16.0,
                s * 0.46 - s * 0.46 * (13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos()) / 16.0,
            )
        })
        .collect();
    polygon(&mut im, &pts, Some(op(color)), None);
    im
}

/// A pointer, `size` tall-ish, white with a black edge, over its shadow.
pub fn cursor(size: f32) -> Pixmap {
    let s = size;
    let mut im = blank(s * 0.95, s * 1.45);
    let pts = [(3.0, 3.0), (3.0, s * 1.12), (s * 0.3, s * 0.87), (s * 0.5, s * 1.33), (s * 0.66, s * 1.26), (s * 0.46, s * 0.8), (s * 0.82, s * 0.8)];
    polygon(&mut im, &pts, Some((255, 255, 255, 255)), Some(((0, 0, 0, 255), (s / 14.0).max(2.0))));
    shadowed(&im, 6.0, 3.0, 110)
}

/// A tick mark from `(x, y)` across `s`.
pub fn tick(pm: &mut Pixmap, x: f32, y: f32, s: f32, color: Rgba, width: Option<f32>) {
    line(pm, &[(x, y + s * 0.5), (x + s * 0.36, y + s * 0.85), (x + s, y + s * 0.1)], color, width.unwrap_or((s / 5.0).max(3.0)));
}

/// The points of a bent arrow from `(x0, y0)` to `(x1, y1)`.
pub fn arrow_pts(x0: f32, y0: f32, x1: f32, y1: f32, bend: f32, n: usize) -> Vec<(f32, f32)> {
    let mx = (x0 + x1) / 2.0 - (y1 - y0) * bend;
    let my = (y0 + y1) / 2.0 + (x1 - x0) * bend;
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            (
                (1.0 - t).powi(2) * x0 + 2.0 * (1.0 - t) * t * mx + t * t * x1,
                (1.0 - t).powi(2) * y0 + 2.0 * (1.0 - t) * t * my + t * t * y1,
            )
        })
        .collect()
}

/// The first `frac` of the path through `pts`, drawn as a marker line;
/// the point it reached.
pub fn hand_path(pm: &mut Pixmap, pts: &[(f32, f32)], color: Rgba, width: f32, frac: f32) -> (f32, f32) {
    let n = ((pts.len() as f32 * frac.clamp(0.0, 1.0)) as usize).max(2).min(pts.len());
    if frac > 0.0 {
        line(pm, &pts[..n], color, width);
    }
    pts[n - 1]
}

/// The two strokes of an arrow's head at the end of `pts`.
pub fn arrow_head(pm: &mut Pixmap, pts: &[(f32, f32)], color: Rgba, width: f32, size: f32) {
    if pts.len() < 4 {
        return;
    }
    let (xa, ya) = pts[pts.len() - 4];
    let (xb, yb) = pts[pts.len() - 1];
    let a = (yb - ya).atan2(xb - xa);
    for s in [2.6f32, -2.6] {
        line(pm, &[(xb, yb), (xb + size * (a + s).cos(), yb + size * (a + s).sin())], color, width);
    }
}
