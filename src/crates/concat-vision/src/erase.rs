// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Taking things out of a picture: what was behind them made up from what is
//! round them, by MI-GAN (MIT, Picsart AI Research). Each gap is filled from
//! a close-up of its own, so a small one is as sharp as the picture.

use std::path::Path;

use concat_core::frame::{BYTES_PER_PIXEL, Frame};

use crate::outline::Outline;
use crate::runtime::{Input, Model};
use crate::window::{self, Rect};

/// The square the model fills at.
const SIDE: usize = 512;
/// How far a gap is widened past a thing's outline, of its longer side: its
/// edge is mixed with what is behind it, and would show.
const WIDEN: f32 = 0.03;
/// And at least and at most so many pixels.
const WIDEN_LEAST: usize = 4;
const WIDEN_MOST: usize = 48;

/// Which pixels of a picture are to go.
#[derive(Clone, PartialEq, Debug)]
pub struct Gaps {
    width: usize,
    height: usize,
    on: Vec<bool>,
}

impl Gaps {
    /// The gaps `outlines` leave in a picture `width` by `height`.
    pub fn of(outlines: &[Outline], width: u32, height: u32) -> Gaps {
        let (w, h) = (width as usize, height as usize);
        let mut on = vec![false; w * h];
        for outline in outlines {
            let x0 = (outline.x * w as f32).floor().max(0.0) as usize;
            let y0 = (outline.y * h as f32).floor().max(0.0) as usize;
            let x1 = (((outline.x + outline.width) * w as f32).ceil() as usize).min(w);
            let y1 = (((outline.y + outline.height) * h as f32).ceil() as usize).min(h);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            // the thing in its part of the picture, and the box it fills
            let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
            let mut held = vec![false; (x1 - x0) * (y1 - y0)];
            for y in y0..y1 {
                let v = ((y as f32 + 0.5) / h as f32 - outline.y) / outline.height;
                for x in x0..x1 {
                    let u = ((x as f32 + 0.5) / w as f32 - outline.x) / outline.width;
                    if outline.mask.sample(u, v) >= 0.4 {
                        held[(y - y0) * (x1 - x0) + x - x0] = true;
                        (left, top) = (left.min(x), top.min(y));
                        (right, bottom) = (right.max(x + 1), bottom.max(y + 1));
                    }
                }
            }
            if right <= left {
                continue;
            }
            let long = (right - left).max(bottom - top);
            let reach = ((long as f32 * WIDEN) as usize).clamp(WIDEN_LEAST, WIDEN_MOST);
            widen(&held, (x0, y0, x1, y1), reach, &mut on, w, h);
        }
        Gaps {
            width: w,
            height: h,
            on,
        }
    }

    /// Whether there is nothing to fill.
    pub fn is_empty(&self) -> bool {
        !self.on.contains(&true)
    }

    /// `frame` with the gaps see-through: what taking something out of a
    /// cut-out leaves, nothing being behind it.
    pub fn cleared(&self, mut frame: Frame) -> Frame {
        if (frame.width() as usize, frame.height() as usize) == (self.width, self.height) {
            for (pixel, on) in frame
                .pixels_mut()
                .chunks_exact_mut(BYTES_PER_PIXEL)
                .zip(&self.on)
            {
                if *on {
                    pixel[3] = 0;
                }
            }
        }
        frame
    }

    /// The gaps apart from each other: the pixels of each.
    fn apart(&self) -> Vec<Vec<usize>> {
        let (w, h) = (self.width, self.height);
        let mut seen: Vec<bool> = self.on.iter().map(|on| !on).collect();
        let mut gaps = Vec::new();
        for start in 0..w * h {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let mut gap = vec![start];
            let mut next = 0;
            while next < gap.len() {
                let (x, y) = (gap[next] % w, gap[next] / w);
                next += 1;
                for (nx, ny) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if nx < w && ny < h && !seen[ny * w + nx] {
                        seen[ny * w + nx] = true;
                        gap.push(ny * w + nx);
                    }
                }
            }
            gaps.push(gap);
        }
        // the small ones first: they are the large ones' surroundings
        gaps.sort_by_key(Vec::len);
        gaps
    }
}

/// Sets every pixel of `on` (a picture `w` by `h`) within `reach` of one held
/// in `part` of it.
fn widen(
    held: &[bool],
    part: (usize, usize, usize, usize),
    reach: usize,
    on: &mut [bool],
    w: usize,
    h: usize,
) {
    let (x0, y0, x1, y1) = part;
    let pw = x1 - x0;
    // a disc's half width at each distance down, so corners come out round
    let half: Vec<usize> = (0..=reach)
        .map(|d| ((reach * reach - d * d) as f32).sqrt() as usize)
        .collect();
    for y in y0..y1 {
        let row = &held[(y - y0) * pw..(y - y0 + 1) * pw];
        let mut x = 0;
        while x < pw {
            if !row[x] {
                x += 1;
                continue;
            }
            let from = x;
            while x < pw && row[x] {
                x += 1;
            }
            // a run of held pixels, widened on every row the disc reaches
            for oy in y.saturating_sub(reach)..(y + reach + 1).min(h) {
                let reach = half[oy.abs_diff(y)];
                let left = (x0 + from).saturating_sub(reach);
                let right = (x0 + x + reach).min(w);
                on[oy * w + left..oy * w + right].fill(true);
            }
        }
    }
}

/// The eraser, loaded.
pub struct Eraser {
    model: Model,
}

impl Eraser {
    /// Loads the model from its downloaded file.
    pub fn load(file: &Path) -> Result<Eraser, String> {
        Ok(Eraser {
            model: Model::from_file(file)?,
        })
    }

    /// `frame` with `gaps` filled. `progress` is told how far along it is, 0
    /// to 1, and stops it by answering false.
    pub fn erase(
        &mut self,
        mut frame: Frame,
        gaps: &Gaps,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<Frame, String> {
        let (w, h) = (frame.width() as usize, frame.height() as usize);
        if (w, h) != (gaps.width, gaps.height) || frame.bytes_per_pixel() != BYTES_PER_PIXEL {
            return Err("erase: the picture is not the gaps'".to_owned());
        }
        let mut left = gaps.on.clone();
        let apart = gaps.apart();
        let count = apart.len().max(1) as f32;
        for (index, gap) in apart.iter().enumerate() {
            if !progress(index as f32 / count) {
                return Err("cancelled".to_owned());
            }
            let rect = close_up(gap, w, h);
            let filled = self.fill(&frame, &left, rect)?;
            paste(&mut frame, &filled, rect, gap, w, h);
            for at in gap {
                left[*at] = false;
            }
        }
        progress(1.0);
        Ok(frame)
    }

    /// What the model makes of `rect` of `frame`, the pixels `left` unknown.
    fn fill(&mut self, frame: &Frame, left: &[bool], rect: Rect) -> Result<Vec<u8>, String> {
        let w = frame.width() as usize;
        let h = frame.height() as usize;
        let small = window::rgb(frame, rect, SIDE, SIDE);
        let mut image = vec![0u8; 3 * SIDE * SIDE];
        let mut known = vec![255u8; SIDE * SIDE];
        for y in 0..SIDE {
            for x in 0..SIDE {
                for c in 0..3 {
                    image[c * SIDE * SIDE + y * SIDE + x] = small[(y * SIDE + x) * 3 + c];
                }
            }
        }
        // a pixel of the square is unknown where any of the picture under it is
        let (sx, sy) = (rect.width / SIDE as f32, rect.height / SIDE as f32);
        let span = |i: usize, start: f32, step: f32, end: usize| {
            let from = (start + i as f32 * step).floor().max(0.0) as usize;
            let to = ((start + (i + 1) as f32 * step).ceil() as usize).min(end);
            (from.min(end.saturating_sub(1)), to.max(from + 1).min(end))
        };
        for y in 0..SIDE {
            let (py0, py1) = span(y, rect.y, sy, h);
            for x in 0..SIDE {
                let (px0, px1) = span(x, rect.x, sx, w);
                if (py0..py1).any(|py| left[py * w + px0..py * w + px1].contains(&true)) {
                    known[y * SIDE + x] = 0;
                }
            }
        }
        let answer = self.model.run_bytes(
            vec![
                Input {
                    name: "image",
                    dims: vec![1, 3, SIDE, SIDE],
                    data: image.into(),
                },
                Input {
                    name: "mask",
                    dims: vec![1, 1, SIDE, SIDE],
                    data: known.into(),
                },
            ],
            "result",
        )?;
        if answer.len() != 3 * SIDE * SIDE {
            return Err("erase: the model's answer is the wrong size".to_owned());
        }
        Ok(answer)
    }
}

/// The part of a picture `w` by `h` a gap is filled from: round it, twice as
/// wide as it, square where there is room.
fn close_up(gap: &[usize], w: usize, h: usize) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for at in gap {
        let (x, y) = (at % w, at / w);
        (x0, y0) = (x0.min(x), y0.min(y));
        (x1, y1) = (x1.max(x + 1), y1.max(y + 1));
    }
    let side = ((x1 - x0).max(y1 - y0) * 2).max(SIDE);
    let (rw, rh) = (side.min(w), side.min(h));
    let place = |from: usize, to: usize, size: usize, end: usize| {
        ((from + to) / 2).saturating_sub(size / 2).min(end - size)
    };
    Rect {
        x: place(x0, x1, rw, w) as f32,
        y: place(y0, y1, rh, h) as f32,
        width: rw as f32,
        height: rh as f32,
    }
}

/// Lays `filled` (the square of `rect`) over the pixels of `gap`, faded into
/// what is round it over a couple of pixels.
fn paste(frame: &mut Frame, filled: &[u8], rect: Rect, gap: &[usize], w: usize, h: usize) {
    let read = |u: f32, v: f32, c: usize| {
        let (u, v) = (
            u.clamp(0.0, (SIDE - 1) as f32),
            v.clamp(0.0, (SIDE - 1) as f32),
        );
        let (x0, y0) = (u as usize, v as usize);
        let (x1, y1) = ((x0 + 1).min(SIDE - 1), (y0 + 1).min(SIDE - 1));
        let (tx, ty) = (u - x0 as f32, v - y0 as f32);
        let at = |x: usize, y: usize| f32::from(filled[c * SIDE * SIDE + y * SIDE + x]);
        let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
        let low = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
        top * (1.0 - ty) + low * ty
    };
    let (x0, y0) = (rect.x as usize, rect.y as usize);
    let (rw, rh) = (rect.width as usize, rect.height as usize);
    let mut inside = vec![0f32; rw * rh];
    for at in gap {
        let (x, y) = (at % w, at / w);
        if x >= x0 && y >= y0 && x - x0 < rw && y - y0 < rh {
            inside[(y - y0) * rw + x - x0] = 1.0;
        }
    }
    // how much of the fill each pixel takes: all in the gap, less just past it
    const FADE: usize = 2;
    let mut soft = inside.clone();
    for at in gap {
        let (x, y) = (at % w, at / w);
        if x < x0 || y < y0 {
            continue;
        }
        let (x, y) = (x - x0, y - y0);
        for ny in y.saturating_sub(FADE)..(y + FADE + 1).min(rh) {
            for nx in x.saturating_sub(FADE)..(x + FADE + 1).min(rw) {
                let d = nx.abs_diff(x).max(ny.abs_diff(y));
                let share = 1.0 - d as f32 / (FADE + 1) as f32;
                let held = &mut soft[ny * rw + nx];
                *held = held.max(share);
            }
        }
    }
    let pixels = frame.pixels_mut();
    for y in 0..rh {
        for x in 0..rw {
            let share = soft[y * rw + x];
            if share <= 0.0 || y0 + y >= h || x0 + x >= w {
                continue;
            }
            let u = (x as f32 + 0.5) / rect.width * SIDE as f32 - 0.5;
            let v = (y as f32 + 0.5) / rect.height * SIDE as f32 - 0.5;
            let at = ((y0 + y) * w + x0 + x) * BYTES_PER_PIXEL;
            for c in 0..3 {
                let was = f32::from(pixels[at + c]);
                let now = read(u, v, c);
                pixels[at + c] = (was + (now - was) * share + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mask;

    fn outline(x: f32, y: f32, width: f32, height: f32) -> Outline {
        Outline {
            x,
            y,
            width,
            height,
            mask: Mask::filled(16, 16, 255),
        }
    }

    #[test]
    fn a_gap_is_a_little_wider_than_its_thing() {
        let gaps = Gaps::of(&[outline(0.4, 0.4, 0.2, 0.2)], 100, 100);
        let on = |x: usize, y: usize| gaps.on[y * 100 + x];
        assert!(on(50, 50) && on(37, 50) && on(50, 62));
        assert!(!on(30, 50) && !on(50, 70));
        // round at the corners
        assert!(!on(36, 36));
    }

    #[test]
    fn things_apart_are_gaps_apart_each_with_a_close_up_of_its_own() {
        let gaps = Gaps::of(
            &[outline(0.05, 0.4, 0.05, 0.1), outline(0.8, 0.4, 0.1, 0.3)],
            2000,
            1000,
        );
        let apart = gaps.apart();
        assert_eq!(apart.len(), 2);
        let first = close_up(&apart[0], 2000, 1000);
        assert_eq!((first.x, first.width, first.height), (0.0, 512.0, 512.0));
        let second = close_up(&apart[1], 2000, 1000);
        assert!(second.x + second.width <= 2000.0 && second.height <= 1000.0);
        assert!(second.x < 1600.0 && second.x + second.width >= 1800.0);
    }

    #[test]
    fn a_cut_out_is_cleared_where_something_is_taken_from_it() {
        let gaps = Gaps::of(&[outline(0.4, 0.4, 0.2, 0.2)], 100, 100);
        let mut frame = Frame::black(100, 100);
        frame.fill([9, 9, 9, 255]);
        let frame = gaps.cleared(frame);
        assert_eq!(frame.pixel(50, 50), Some([9, 9, 9, 0]));
        assert_eq!(frame.pixel(10, 10), Some([9, 9, 9, 255]));
    }

    #[test]
    fn only_the_gap_and_its_edge_change() {
        let mut frame = Frame::black(1000, 1000);
        let gaps = Gaps::of(&[outline(0.45, 0.45, 0.1, 0.1)], 1000, 1000);
        let gap = gaps.apart().remove(0);
        let rect = close_up(&gap, 1000, 1000);
        paste(&mut frame, &vec![200u8; 3 * SIDE * SIDE], rect, &gap, 1000, 1000);
        assert_eq!(frame.pixel(500, 500).map(|p| p[0]), Some(200));
        assert_eq!(frame.pixel(400, 500).map(|p| p[0]), Some(0));
    }

    /// Finds the people in a picture and takes them out, to look at:
    /// `CONCAT_ERASE_MODELS=folder CONCAT_ERASE_PHOTO=in.png
    /// CONCAT_ERASE_OUT=out.png cargo test -p concat-vision --features infer
    /// -- --ignored --nocapture people_are_found`. The folder holds the models
    /// by the names in [`crate::models`].
    #[test]
    #[ignore = "needs the downloaded models and a picture"]
    fn people_are_found_and_taken_out() {
        use crate::models::ModelId;
        let var = |name: &str| std::env::var(name).expect(name);
        let model = |id: ModelId| Path::new(&var("CONCAT_ERASE_MODELS")).join(id.spec().file);
        let file = std::fs::File::open(var("CONCAT_ERASE_PHOTO")).expect("opens");
        let mut reader = png::Decoder::new(std::io::BufReader::new(file))
            .read_info()
            .expect("a png");
        let mut data = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut data).expect("reads");
        let n = info.color_type.samples();
        let rgba: Vec<u8> = data[..info.buffer_size()]
            .chunks_exact(n)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        let frame = Frame::from_rgba(info.width, info.height, rgba).expect("a frame");
        let started = std::time::Instant::now();
        let found = crate::detect::Detector::load(&model(ModelId::Detect))
            .and_then(|mut d| d.find(&frame))
            .expect("finds");
        println!("found {:?} in {:?}", found.iter().map(|f| f.kind).collect::<Vec<_>>(), started.elapsed());
        let brush = crate::Brush::load(&model(ModelId::BrushEncoder), &model(ModelId::BrushDecoder))
            .expect("loads");
        let outlines: Vec<Outline> = found
            .iter()
            .filter(|f| f.kind == "person")
            .filter_map(|f| crate::outline::of(&brush, &frame, f).ok())
            .collect();
        println!("{} outlines by {:?}", outlines.len(), started.elapsed());
        let gaps = Gaps::of(&outlines, frame.width(), frame.height());
        let done = Eraser::load(&model(ModelId::Erase))
            .and_then(|mut e| e.erase(frame, &gaps, &mut |_| true))
            .expect("erases");
        println!("done by {:?}", started.elapsed());
        let out = std::fs::File::create(var("CONCAT_ERASE_OUT")).expect("creates");
        let mut encoder = png::Encoder::new(out, done.width(), done.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(done.pixels()))
            .expect("writes");
    }
}
