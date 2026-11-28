// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! A clip's grade as a `.cube` LUT: its colour packages - Adjust, the looks,
//! curves, wheels, white balance - drawn by the compositor over every colour of
//! a 33-step lattice and written out as the table other editors and cameras
//! read. A LUT holds one colour for each colour and nothing else, so what looks
//! at a pixel's neighbours (sharpen, vignette, clarity, grain) or moves the
//! picture is left out, and named.

use std::io::Write as _;
use std::sync::Arc;

use concat_core::frame::Frame;
use concat_effects::{Catalogue, manifest::Kind};
use concat_project::model::AppliedFilter;
use concat_render::compositor::Compositor as _;
use concat_render::gpu::WgpuCompositor;
use concat_render::plan::{FramePlan, PlannedLayer, detached_clip};

use crate::MobileError;
use crate::session::MobileSession;

/// Steps a side of the lattice: the size most tools read and write.
const SIZE: u32 = 33;
/// Knobs of a colour package that look past the pixel itself.
const AROUND: [(&str, &str); 4] = [
    ("sharpen", "Sharpen"),
    ("vignette", "Vignette"),
    ("clarity", "Clarity"),
    ("grain", "Grain"),
];

/// What a LUT export wrote and what it could not hold.
#[derive(Debug, Clone, uniffi::Record)]
pub struct LutExport {
    /// The `.cube` file written.
    pub path: String,
    /// The grade's parts the table holds.
    pub kept: Vec<String>,
    /// The parts it leaves out: they need more than a pixel's own colour.
    pub left_out: Vec<String>,
}

fn failed(message: impl Into<String>) -> MobileError {
    MobileError::Failed(message.into())
}

/// Every colour of the lattice once: red along a row within each blue block,
/// green down the rows, blue block after block.
fn lattice() -> Frame {
    let side = SIZE;
    let mut frame = Frame::transparent(side * side, side);
    let step = |i: u32| ((i * 255 + (side - 1) / 2) / (side - 1)) as u8;
    for b in 0..side {
        for g in 0..side {
            for r in 0..side {
                frame.set_pixel(r + b * side, g, [step(r), step(g), step(b), 255]);
            }
        }
    }
    frame
}

/// The colour parts of `effects`, with their neighbourhood knobs at rest, and
/// the names of what was left out.
fn colour_only(effects: &[AppliedFilter]) -> (Vec<AppliedFilter>, Vec<String>, Vec<String>) {
    let catalogue = Catalogue::builtin();
    let (mut kept, mut names, mut left_out) = (Vec::new(), Vec::new(), Vec::new());
    for effect in effects.iter().filter(|e| e.enabled) {
        let Some(package) = catalogue.get(&effect.id) else { continue };
        let manifest = &package.manifest;
        let colour = manifest.effect.kind == Kind::Filter
            || matches!(manifest.effect.category.as_str(), "Adjust" | "Color" | "Colour");
        if !colour {
            left_out.push(manifest.effect.name.clone());
            continue;
        }
        let mut effect = effect.clone();
        for (key, label) in AROUND {
            if effect.params.get(key).is_some_and(|v| v.abs() > 1e-6) {
                effect.params.insert(key.to_owned(), 0.0);
                left_out.push(label.to_owned());
            }
        }
        names.push(manifest.effect.name.clone());
        kept.push(effect);
    }
    (kept, names, left_out)
}

#[uniffi::export]
impl MobileSession {
    /// Writes the grade of clip `clip_id` to `path` as a 33-step `.cube`
    /// titled `title`. Drawn on the GPU by the compositor that draws the clip,
    /// so the table is the grade the preview shows.
    pub fn export_lut(&self, clip_id: String, path: String, title: String) -> Result<LutExport, MobileError> {
        let effects = self.with_session(|session| {
            session
                .project()
                .active()
                .clips
                .iter()
                .find(|c| c.id == clip_id)
                .map(|c| c.video_effects.clone())
                .ok_or_else(|| failed("That clip is gone."))
        })?;
        let (kept, names, left_out) = colour_only(&effects);
        if kept.is_empty() {
            return Err(failed("That clip has no colour adjustments to save."));
        }
        let source = lattice();
        let (width, height) = (source.width(), source.height());
        let mut layer = PlannedLayer::picture(detached_clip(), Arc::new(source));
        let kept: Vec<_> = kept.iter().map(concat_export::chains::engine_filter).collect();
        layer.effects = Catalogue::builtin().shader_passes_at(&kept, 0.0, None);
        let plan = FramePlan {
            layers: vec![layer],
            ..FramePlan::empty(width, height)
        };
        let mut compositor = WgpuCompositor::new().ok_or_else(|| failed("No GPU to draw the grade on."))?;
        let graded = compositor.render(&plan);
        if graded.width() != width || graded.height() != height || graded.pixels().is_empty() {
            return Err(failed("The grade could not be drawn."));
        }
        let mut out = String::with_capacity(36 * (SIZE * SIZE * SIZE) as usize);
        let name = title.replace(['"', '\n', '\r'], " ");
        out.push_str(&format!("TITLE \"{}\"\n", name.trim()));
        out.push_str("# Made by CapCraft Forge from a clip's grade\n");
        if !left_out.is_empty() {
            out.push_str(&format!("# Left out (not colour alone): {}\n", left_out.join(", ")));
        }
        out.push_str(&format!("LUT_3D_SIZE {SIZE}\nDOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n"));
        // Red changes fastest, then green, then blue, as the format reads.
        for b in 0..SIZE {
            for g in 0..SIZE {
                for r in 0..SIZE {
                    let [pr, pg, pb, _] = graded.pixel(r + b * SIZE, g).unwrap_or([0, 0, 0, 255]);
                    out.push_str(&format!(
                        "{:.6} {:.6} {:.6}\n",
                        f64::from(pr) / 255.0,
                        f64::from(pg) / 255.0,
                        f64::from(pb) / 255.0
                    ));
                }
            }
        }
        if let Some(folder) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(folder).map_err(|e| failed(e.to_string()))?;
        }
        std::fs::File::create(&path)
            .and_then(|mut file| file.write_all(out.as_bytes()))
            .map_err(|e| failed(format!("{path}: {e}")))?;
        Ok(LutExport {
            path,
            kept: names,
            left_out,
        })
    }
}
