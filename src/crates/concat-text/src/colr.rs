// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! A `COLR` version 1 glyph painted into a picture: its fills and gradients
//! over clips, transforms and layers, which newer colour emoji faces (Noto
//! Color Emoji as phones carry it) are made of.

use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder,
    Pixmap, PixmapPaint, Point, RadialGradient, Rect, Shader, SpreadMode, SweepGradient, Transform,
};
use ttf_parser::{GlyphId, RgbaColor, colr};

/// A glyph's picture, and where its left and top edges sit from the pen, in
/// pixels, y down.
pub(crate) struct Painted {
    pub pixmap: Pixmap,
    pub x: f32,
    pub y: f32,
}

/// The em's room on the left and above the baseline, and the picture's side,
/// in ems: an emoji is wider than its em and drops below the baseline.
const LEFT: f32 = 0.25;
const UP: f32 = 1.25;
const SIDE: f32 = 1.9;
/// The largest picture made, in pixels a side.
const LARGEST: f32 = 2048.0;

/// Paints `glyph` at `em` pixels, or `None` when it paints nothing.
pub(crate) fn paint(
    face: &ttf_parser::Face<'_>,
    glyph: GlyphId,
    em: f32,
    foreground: RgbaColor,
) -> Option<Painted> {
    let side = (em * SIDE).ceil();
    if !(1.0..=LARGEST).contains(&side) {
        return None;
    }
    let side = side as u32;
    let scale = em / f32::from(face.units_per_em());
    let mut painter = Painter {
        face,
        side,
        transforms: vec![Transform::from_row(
            scale,
            0.0,
            0.0,
            -scale,
            em * LEFT,
            em * UP,
        )],
        clips: vec![None],
        outline: None,
        layers: vec![(Pixmap::new(side, side)?, BlendMode::SourceOver)],
    };
    face.paint_color_glyph(glyph, 0, foreground, &mut painter)?;
    let (pixmap, _) = painter.layers.into_iter().next()?;
    pixmap
        .pixels()
        .iter()
        .any(|pixel| pixel.alpha() > 0)
        .then_some(Painted {
            pixmap,
            x: -em * LEFT,
            y: -em * UP,
        })
}

struct Painter<'f, 'a> {
    face: &'f ttf_parser::Face<'a>,
    side: u32,
    /// The font's units to pixels, innermost last.
    transforms: Vec<Transform>,
    /// What is drawn to, innermost last; `None` is everywhere.
    clips: Vec<Option<Mask>>,
    /// The outline last named, in pixels, waiting to be filled or clipped to.
    outline: Option<Path>,
    /// The picture and the layers over it, each with how it will be laid on
    /// the one under it.
    layers: Vec<(Pixmap, BlendMode)>,
}

impl Painter<'_, '_> {
    fn transform(&self) -> Transform {
        self.transforms.last().copied().unwrap_or_default()
    }

    fn clip_to(&mut self, path: Option<Path>) {
        let mut mask = self.clips.last().cloned().flatten();
        match (path, &mut mask) {
            (Some(path), Some(mask)) => {
                mask.intersect_path(&path, FillRule::Winding, true, Transform::identity());
            }
            (Some(path), None) => {
                mask = Mask::new(self.side, self.side);
                if let Some(mask) = &mut mask {
                    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
                }
            }
            // an outline with nothing in it lets nothing through
            (None, _) => mask = Mask::new(self.side, self.side),
        }
        self.clips.push(mask);
    }
}

fn spread(extend: colr::GradientExtend) -> SpreadMode {
    match extend {
        colr::GradientExtend::Pad => SpreadMode::Pad,
        colr::GradientExtend::Repeat => SpreadMode::Repeat,
        colr::GradientExtend::Reflect => SpreadMode::Reflect,
    }
}

fn color(c: RgbaColor) -> Color {
    Color::from_rgba8(c.red, c.green, c.blue, c.alpha)
}

fn stops(list: impl Iterator<Item = colr::ColorStop>) -> Vec<GradientStop> {
    let mut list: Vec<colr::ColorStop> = list.collect();
    list.sort_by(|a, b| a.stop_offset.total_cmp(&b.stop_offset));
    list.into_iter()
        .map(|stop| GradientStop::new(stop.stop_offset.clamp(0.0, 1.0), color(stop.color)))
        .collect()
}

impl<'a> colr::Painter<'a> for Painter<'_, 'a> {
    fn outline_glyph(&mut self, glyph_id: GlyphId) {
        struct Outline(PathBuilder);
        impl ttf_parser::OutlineBuilder for Outline {
            fn move_to(&mut self, x: f32, y: f32) {
                self.0.move_to(x, y);
            }
            fn line_to(&mut self, x: f32, y: f32) {
                self.0.line_to(x, y);
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                self.0.quad_to(x1, y1, x, y);
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                self.0.cubic_to(x1, y1, x2, y2, x, y);
            }
            fn close(&mut self) {
                self.0.close();
            }
        }
        let mut outline = Outline(PathBuilder::new());
        self.face.outline_glyph(glyph_id, &mut outline);
        let transform = self.transform();
        self.outline = outline.0.finish().and_then(|path| path.transform(transform));
    }

    fn paint(&mut self, paint: colr::Paint<'a>) {
        let transform = self.transform();
        let coords = self.face.variation_coordinates();
        let shader = match paint {
            colr::Paint::Solid(c) => Some(Shader::SolidColor(color(c))),
            colr::Paint::LinearGradient(g) => {
                // the third point turns the gradient: its end is the second
                // point's shadow on the line square to the first and third
                let (dx, dy) = (g.x2 - g.x0, g.y2 - g.y0);
                let (px, py) = (dy, -dx);
                let length = px * px + py * py;
                let (x1, y1) = if length > 0.0 {
                    let k = ((g.x1 - g.x0) * px + (g.y1 - g.y0) * py) / length;
                    (g.x0 + px * k, g.y0 + py * k)
                } else {
                    (g.x1, g.y1)
                };
                LinearGradient::new(
                    Point::from_xy(g.x0, g.y0),
                    Point::from_xy(x1, y1),
                    stops(g.stops(0, coords)),
                    spread(g.extend),
                    transform,
                )
            }
            colr::Paint::RadialGradient(g) => RadialGradient::new(
                Point::from_xy(g.x0, g.y0),
                g.r0,
                Point::from_xy(g.x1, g.y1),
                g.r1,
                stops(g.stops(0, coords)),
                spread(g.extend),
                transform,
            ),
            colr::Paint::SweepGradient(g) => SweepGradient::new(
                Point::from_xy(g.center_x, g.center_y),
                g.start_angle * 180.0,
                g.end_angle * 180.0,
                stops(g.stops(0, coords)),
                spread(g.extend),
                transform,
            ),
        };
        let Some(shader) = shader else { return };
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        let mask = self.clips.last().and_then(Option::as_ref);
        let Some((canvas, _)) = self.layers.last_mut() else {
            return;
        };
        match self.outline.take() {
            // an outline and its paint, with no clip between them
            Some(path) => {
                canvas.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), mask);
            }
            None => {
                if let Some(all) = Rect::from_xywh(0.0, 0.0, self.side as f32, self.side as f32) {
                    canvas.fill_rect(all, &paint, Transform::identity(), mask);
                }
            }
        }
    }

    fn push_clip(&mut self) {
        let path = self.outline.take();
        self.clip_to(path);
    }

    fn push_clip_box(&mut self, clipbox: colr::ClipBox) {
        let transform = self.transform();
        let path = Rect::from_ltrb(clipbox.x_min, clipbox.y_min, clipbox.x_max, clipbox.y_max)
            .and_then(|rect| PathBuilder::from_rect(rect).transform(transform));
        self.clip_to(path);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn push_layer(&mut self, mode: colr::CompositeMode) {
        use colr::CompositeMode as Mode;
        let mode = match mode {
            Mode::Clear => BlendMode::Clear,
            Mode::Source => BlendMode::Source,
            Mode::Destination => BlendMode::Destination,
            Mode::SourceOver => BlendMode::SourceOver,
            Mode::DestinationOver => BlendMode::DestinationOver,
            Mode::SourceIn => BlendMode::SourceIn,
            Mode::DestinationIn => BlendMode::DestinationIn,
            Mode::SourceOut => BlendMode::SourceOut,
            Mode::DestinationOut => BlendMode::DestinationOut,
            Mode::SourceAtop => BlendMode::SourceAtop,
            Mode::DestinationAtop => BlendMode::DestinationAtop,
            Mode::Xor => BlendMode::Xor,
            Mode::Plus => BlendMode::Plus,
            Mode::Screen => BlendMode::Screen,
            Mode::Overlay => BlendMode::Overlay,
            Mode::Darken => BlendMode::Darken,
            Mode::Lighten => BlendMode::Lighten,
            Mode::ColorDodge => BlendMode::ColorDodge,
            Mode::ColorBurn => BlendMode::ColorBurn,
            Mode::HardLight => BlendMode::HardLight,
            Mode::SoftLight => BlendMode::SoftLight,
            Mode::Difference => BlendMode::Difference,
            Mode::Exclusion => BlendMode::Exclusion,
            Mode::Multiply => BlendMode::Multiply,
            Mode::Hue => BlendMode::Hue,
            Mode::Saturation => BlendMode::Saturation,
            Mode::Color => BlendMode::Color,
            Mode::Luminosity => BlendMode::Luminosity,
        };
        // a side checked when the picture was made
        if let Some(layer) = Pixmap::new(self.side, self.side) {
            self.layers.push((layer, mode));
        }
    }

    fn pop_layer(&mut self) {
        if self.layers.len() < 2 {
            return;
        }
        let Some((layer, mode)) = self.layers.pop() else {
            return;
        };
        if let Some((below, _)) = self.layers.last_mut() {
            below.draw_pixmap(
                0,
                0,
                layer.as_ref(),
                &PixmapPaint {
                    blend_mode: mode,
                    ..PixmapPaint::default()
                },
                Transform::identity(),
                None,
            );
        }
    }

    fn push_transform(&mut self, t: ttf_parser::Transform) {
        let next = self
            .transform()
            .pre_concat(Transform::from_row(t.a, t.b, t.c, t.d, t.e, t.f));
        self.transforms.push(next);
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paints an emoji of a version 1 face to look at: `CONCAT_COLR_FACE=
    /// NotoColorEmoji.ttf CONCAT_COLR_OUT=out.png cargo test -p concat-text
    /// -- --ignored a_version_1_glyph`.
    #[test]
    #[ignore = "needs a COLR version 1 face"]
    fn a_version_1_glyph_is_painted_in_its_colours() {
        let data = std::fs::read(std::env::var("CONCAT_COLR_FACE").expect("CONCAT_COLR_FACE"))
            .expect("reads");
        let face = ttf_parser::Face::parse(&data, 0).expect("parses");
        let mut sheet = Pixmap::new(1200, 240).expect("a sheet");
        for (index, ch) in ['😂', '🔥', '🎉', '💰', '🌸'].into_iter().enumerate() {
            let glyph = face.glyph_index(ch).expect("has it");
            let painted = paint(&face, glyph, 120.0, RgbaColor::new(0, 0, 0, 255)).expect("paints");
            let coloured = painted
                .pixmap
                .pixels()
                .iter()
                .filter(|p| p.alpha() > 128 && p.red().abs_diff(p.blue()) > 40)
                .count();
            assert!(coloured > 500, "{ch}: {coloured} coloured pixels");
            sheet.draw_pixmap(
                index as i32 * 235,
                0,
                painted.pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        if let Ok(out) = std::env::var("CONCAT_COLR_OUT") {
            sheet.save_png(out).expect("writes");
        }
    }
}
