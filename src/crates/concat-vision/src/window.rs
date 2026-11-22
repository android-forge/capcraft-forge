// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! A part of a picture at the size a model wants it: averaged where that is
//! smaller, so nothing small is skipped, and read between pixels where it is
//! larger.

use concat_core::frame::{BYTES_PER_PIXEL, Frame};

/// A part of a picture, in its pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    /// Its left edge.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
}

/// The red, green and blue of `rect` of `frame`, `width` by `height`.
pub fn rgb(frame: &Frame, rect: Rect, width: usize, height: usize) -> Vec<u8> {
    let (fw, fh) = (frame.width() as usize, frame.height() as usize);
    let mut out = vec![0u8; width * height * 3];
    if fw == 0 || fh == 0 || width == 0 || height == 0 {
        return out;
    }
    let pixels = frame.pixels();
    let (sx, sy) = (rect.width / width as f32, rect.height / height as f32);
    // the samples a side of each output pixel is averaged over
    let (nx, ny) = (
        (sx.ceil() as usize).clamp(1, 16),
        (sy.ceil() as usize).clamp(1, 16),
    );
    let at = |x: i64, y: i64, c: usize| {
        let x = x.clamp(0, fw as i64 - 1) as usize;
        let y = y.clamp(0, fh as i64 - 1) as usize;
        f32::from(pixels[(y * fw + x) * BYTES_PER_PIXEL + c])
    };
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0f32; 3];
            for j in 0..ny {
                let py = rect.y + (y as f32 + (j as f32 + 0.5) / ny as f32) * sy - 0.5;
                let (y0, ty) = (py.floor(), py - py.floor());
                for i in 0..nx {
                    let px = rect.x + (x as f32 + (i as f32 + 0.5) / nx as f32) * sx - 0.5;
                    let (x0, tx) = (px.floor(), px - px.floor());
                    let (x0, y0) = (x0 as i64, y0 as i64);
                    for (c, sum) in sum.iter_mut().enumerate() {
                        let top = at(x0, y0, c) * (1.0 - tx) + at(x0 + 1, y0, c) * tx;
                        let low = at(x0, y0 + 1, c) * (1.0 - tx) + at(x0 + 1, y0 + 1, c) * tx;
                        *sum += top * (1.0 - ty) + low * ty;
                    }
                }
            }
            let k = 1.0 / (nx * ny) as f32;
            for c in 0..3 {
                out[(y * width + x) * 3 + c] = (sum[c] * k + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// [`rgb`] as a picture of its own.
pub fn frame(frame: &Frame, rect: Rect, width: u32, height: u32) -> Frame {
    let rgb = rgb(frame, rect, width as usize, height as usize);
    let rgba = rgb
        .chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    Frame::from_rgba(width, height, rgba).unwrap_or_else(|| Frame::black(width, height))
}
