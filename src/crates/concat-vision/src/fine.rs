// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Fine things cut from what is behind them - bare branches from the sky, a
//! fence, wires - down to a twig thinner than a pixel. No model finds those;
//! their colour against the background's does.
//!
//! A smooth background is fitted to the picture first, the bright end of each
//! part of it, so a wide trunk is not taken for sky; then the background is
//! read between the branches and filled in behind them. How much of a pixel is
//! the branch is how far its colour lies from the background's towards the
//! branch's own, and the background's share of its colour is taken off it.

use concat_core::frame::{BYTES_PER_PIXEL, Frame};

use crate::window::{self, Rect};

/// The longer side the background is worked out at.
const GUIDE: f32 = 1200.0;
/// The parts across and down the smooth background is fitted to.
const PARTS: usize = 14;
/// How bright a colour looks, red, green and blue.
const BRIGHT: [f32; 3] = [0.299, 0.587, 0.114];

/// How the cut is made.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Options {
    /// How faint a thing is kept, 0 to 1.
    pub detail: f32,
    /// How much noise and loose specks are cleared, 0 to 1.
    pub clean: f32,
    /// One colour for all that is kept, in place of its own.
    pub ink: Option<[u8; 3]>,
    /// A place in the background, in fractions of the picture, where it is not
    /// the bright sky behind.
    pub back: Option<[f32; 2]>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            detail: 0.5,
            clean: 0.2,
            ink: None,
            back: None,
        }
    }
}

type Rgb = [f32; 3];

/// A picture's background, worked out small: its colour everywhere, behind
/// what is in front of it too.
pub struct Background {
    width: usize,
    height: usize,
    colour: Vec<Rgb>,
    /// The smooth background's brightness.
    smooth: Vec<f32>,
    /// How far background pixels stray from it.
    noise: f32,
    /// The background was pointed at: it need not be the bright side.
    pointed: bool,
}

fn bright(c: Rgb) -> f32 {
    c[0] * BRIGHT[0] + c[1] * BRIGHT[1] + c[2] * BRIGHT[2]
}

fn apart(a: Rgb, b: Rgb) -> f32 {
    let d = |i: usize| a[i] - b[i];
    (0.9 * d(0) * d(0) + 1.2 * d(1) * d(1) + 0.9 * d(2) * d(2)).sqrt()
}

fn read(frame: &Frame, width: usize, height: usize) -> Vec<Rgb> {
    let whole = Rect {
        x: 0.0,
        y: 0.0,
        width: frame.width() as f32,
        height: frame.height() as f32,
    };
    window::rgb(frame, whole, width, height)
        .chunks_exact(3)
        .map(|p| [0, 1, 2].map(|i| f32::from(p[i]) / 255.0))
        .collect()
}

/// Works `frame`'s background out.
pub fn background(frame: &Frame, options: &Options) -> Background {
    let (fw, fh) = (frame.width().max(1) as f32, frame.height().max(1) as f32);
    let scale = (GUIDE / fw.max(fh)).min(1.0);
    let w = ((fw * scale) as usize).max(16);
    let h = ((fh * scale) as usize).max(16);
    let picture = read(frame, w, h);
    let light: Vec<f32> = picture.iter().map(|c| bright(*c)).collect();
    let smooth = match options.back {
        None => surface(&picture, &light, w, h),
        Some([u, v]) => {
            let x = ((u * w as f32) as usize).min(w - 1);
            let y = ((v * h as f32) as usize).min(h - 1);
            vec![average(&picture, w, h, x, y, 3); w * h]
        }
    };
    let level: Vec<f32> = smooth.iter().map(|c| bright(*c)).collect();
    // what may be background at all
    let may: Vec<bool> = (0..w * h)
        .map(|i| match options.back {
            None => light[i] > level[i] * 0.72 - 0.02,
            Some(_) => apart(picture[i], smooth[i]) < 0.22,
        })
        .collect();
    let reach = ((w.min(h) as f32 * 0.02) as usize).max(9);
    let top = local_high(&light, w, h, reach);
    let mut weight: Vec<f32> = (0..w * h)
        .map(|i| {
            let near = match options.back {
                None => light[i] > top[i] - 0.06,
                Some(_) => apart(picture[i], smooth[i]) < 0.1,
            };
            f32::from(u8::from(may[i] && near))
        })
        .collect();
    let mut colour = smooth.clone();
    let mut noise = 0.01;
    for _ in 0..3 {
        colour = filled(&picture, &weight, w, h, &smooth);
        colour = softened(&colour, w, h, (reach / 3).max(2));
        let mut strays: Vec<f32> = (0..w * h)
            .filter(|&i| weight[i] > 0.0)
            .map(|i| apart(picture[i], colour[i]))
            .collect();
        noise = middle(&mut strays).map_or(0.02, |m| (m * 1.48 + 0.004).min(0.03));
        let within = (3.0 * noise).max(0.03);
        for i in 0..w * h {
            weight[i] = f32::from(u8::from(may[i] && apart(picture[i], colour[i]) < within));
        }
    }
    Background {
        width: w,
        height: h,
        colour,
        smooth: level,
        noise: noise.max(0.006),
        pointed: options.back.is_some(),
    }
}

/// `frame` with its background see-through.
pub fn cut(frame: &Frame, back: &Background, options: &Options) -> Frame {
    let (w, h) = (frame.width() as usize, frame.height() as usize);
    let low = 2.5 * back.noise + (1.0 - options.detail) * 0.08 + options.clean * 0.03;
    let high = low + 0.18;
    let fore = foreground(frame, back, high.max(0.25));
    let pixels = frame.pixels();
    let mut out = vec![0u8; w * h * BYTES_PER_PIXEL];
    let mut shares = vec![0u8; w * h];
    // how far each pixel's colour is towards the thing's, for taking the
    // background's share of it off
    let mut alongs = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * BYTES_PER_PIXEL;
            let c: Rgb = [0, 1, 2].map(|i| f32::from(pixels[at + i]) / 255.0);
            let (u, v) = (
                (x as f32 + 0.5) / w as f32 * back.width as f32 - 0.5,
                (y as f32 + 0.5) / h as f32 * back.height as f32 - 0.5,
            );
            let b = between(&back.colour, back.width, back.height, u, v);
            let d = apart(c, b);
            if d <= low * 0.6 {
                continue;
            }
            let level = between_one(&back.smooth, back.width, back.height, u, v);
            // as bright as the sky there, or brighter, is sky
            if !back.pointed && bright(c) > (level * 0.96).max(bright(b) - 0.03) {
                continue;
            }
            let f = between(&fore, back.width, back.height, u, v);
            let toward = [0, 1, 2].map(|i| f[i] - b[i]);
            let length = toward.iter().map(|t| t * t).sum::<f32>().max(1e-3);
            let along = (0..3).map(|i| (c[i] - b[i]) * toward[i]).sum::<f32>() / length;
            let least = low / length.sqrt();
            let mut share = ((along.clamp(0.0, 1.0) - least) / (0.88 - least).max(0.05)).clamp(0.0, 1.0);
            // far from the background whatever the colour: bark in the light
            let t = ((d - 0.2) / 0.14).clamp(0.0, 1.0);
            if back.pointed || bright(c) < level * 0.85 {
                share = share.max(t * t * (3.0 - 2.0 * t));
            }
            if share <= 0.004 {
                continue;
            }
            let alpha = share * f32::from(pixels[at + 3]);
            shares[y * w + x] = (alpha + 0.5) as u8;
            alongs[y * w + x] = (along.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    specks(&mut shares, w, h, options.clean);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if shares[i] == 0 {
                continue;
            }
            let at = i * BYTES_PER_PIXEL;
            out[at + 3] = shares[i];
            if let Some(ink) = options.ink {
                out[at..at + 3].copy_from_slice(&ink);
                continue;
            }
            // inside a thing its colour is its own; at an edge some of it is the
            // background's, which would show as a rim on any other
            let inside = y > 0
                && x > 0
                && y + 1 < h
                && x + 1 < w
                && [i - 1, i + 1, i - w, i + w]
                    .iter()
                    .all(|n| shares[*n] > 230);
            if inside && shares[i] > 230 {
                out[at..at + 3].copy_from_slice(&pixels[at..at + 3]);
                continue;
            }
            let (u, v) = (
                (x as f32 + 0.5) / w as f32 * back.width as f32 - 0.5,
                (y as f32 + 0.5) / h as f32 * back.height as f32 - 0.5,
            );
            let b = between(&back.colour, back.width, back.height, u, v);
            let f = between(&fore, back.width, back.height, u, v);
            let along = (f32::from(alongs[i]) / 255.0).max(0.2);
            for c in 0..3 {
                let held = f32::from(pixels[at + c]) / 255.0;
                let clear = ((held - (1.0 - along) * b[c]) / along).clamp(0.0, 1.0);
                // half its own with the sky taken off, half the thing's round it
                out[at + c] = ((clear * 0.5 + f[c] * 0.5) * 255.0 + 0.5) as u8;
            }
        }
    }
    Frame::from_rgba(frame.width(), frame.height(), out).unwrap_or_else(|| frame.clone())
}

/// What is in front, its colour everywhere: that of pixels far from the
/// background, spread to those that are only part of it.
fn foreground(frame: &Frame, back: &Background, sure: f32) -> Vec<Rgb> {
    let (w, h) = (back.width, back.height);
    let picture = read(frame, w, h);
    let weight: Vec<f32> = (0..w * h)
        .map(|i| f32::from(u8::from(apart(picture[i], back.colour[i]) > sure)))
        .collect();
    let black = vec![[0.05; 3]; w * h];
    if weight.iter().all(|w| *w == 0.0) {
        return black;
    }
    filled(&picture, &weight, w, h, &black)
}

/// Takes off what is kept in pieces too small to be anything.
fn specks(shares: &mut [u8], w: usize, h: usize, clean: f32) {
    let scale = (w.max(h) as f32 / GUIDE).max(0.5);
    let least = (clean.clamp(0.0, 1.0) * 60.0 * scale * scale) as usize;
    if least < 2 {
        return;
    }
    let mut seen: Vec<bool> = shares.iter().map(|s| *s < 24).collect();
    let mut piece = Vec::new();
    for start in 0..w * h {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        piece.clear();
        piece.push(start);
        let mut next = 0;
        while next < piece.len() {
            let (x, y) = (piece[next] % w, piece[next] / w);
            next += 1;
            for ny in y.saturating_sub(1)..(y + 2).min(h) {
                for nx in x.saturating_sub(1)..(x + 2).min(w) {
                    let n = ny * w + nx;
                    if !seen[n] {
                        seen[n] = true;
                        piece.push(n);
                    }
                }
            }
        }
        if piece.len() < least {
            for at in &piece {
                shares[*at] = 0;
            }
        }
    }
}

/// A colour that changes smoothly over the picture, a cubic of the place,
/// fitted to the bright end of each part, parts that are dark all over thrown
/// out as the fit finds them.
fn surface(picture: &[Rgb], light: &[f32], w: usize, h: usize) -> Vec<Rgb> {
    let mut places: Vec<(f32, f32, Rgb)> = Vec::new();
    for j in 0..PARTS {
        for i in 0..PARTS {
            let (x0, x1) = (i * w / PARTS, (i + 1) * w / PARTS);
            let (y0, y1) = (j * h / PARTS, (j + 1) * h / PARTS);
            let mut part: Vec<(f32, usize)> = (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| y * w + x))
                .map(|at| (light[at], at))
                .collect();
            if part.is_empty() {
                continue;
            }
            part.sort_by(|a, b| b.0.total_cmp(&a.0));
            let most = (part.len() * 12 / 100).max(1);
            let mut sum = [0f32; 3];
            for (_, at) in &part[..most] {
                for c in 0..3 {
                    sum[c] += picture[*at][c];
                }
            }
            let across = (i as f32 + 0.5) / PARTS as f32 * 2.0 - 1.0;
            let down = (j as f32 + 0.5) / PARTS as f32 * 2.0 - 1.0;
            places.push((across, down, sum.map(|s| s / most as f32)));
        }
    }
    let mut lights: Vec<f32> = places.iter().map(|p| bright(p.2)).collect();
    let usual = middle(&mut lights.clone()).unwrap_or(0.5);
    let mut trust: Vec<f32> = places
        .iter()
        .map(|p| f32::from(u8::from(bright(p.2) > usual * 0.6)))
        .collect();
    lights.clear();
    let mut fit = [[0f32; 10]; 3];
    for _ in 0..8 {
        for (c, fit) in fit.iter_mut().enumerate() {
            *fit = solve(&places, &trust, c);
        }
        for (place, trust) in places.iter().zip(&mut trust) {
            let t = terms(place.0, place.1);
            let at = [0, 1, 2].map(|c| (0..10).map(|k| t[k] * fit[c][k]).sum::<f32>());
            let off = bright(place.2) - bright(at);
            let near = 1.0 / (1.0 + (off / 0.08).powi(2));
            *trust = if off < -0.12 {
                0.02
            } else if off > 0.0 {
                near.max(0.6)
            } else {
                near
            };
        }
    }
    (0..w * h)
        .map(|i| {
            let across = (i % w) as f32 / (w - 1).max(1) as f32 * 2.0 - 1.0;
            let down = (i / w) as f32 / (h - 1).max(1) as f32 * 2.0 - 1.0;
            let t = terms(across, down);
            [0, 1, 2].map(|c| {
                (0..10)
                    .map(|k| t[k] * fit[c][k])
                    .sum::<f32>()
                    .clamp(0.02, 1.2)
            })
        })
        .collect()
}

fn terms(x: f32, y: f32) -> [f32; 10] {
    [
        1.0,
        x,
        y,
        x * x,
        x * y,
        y * y,
        x * x * x,
        x * x * y,
        x * y * y,
        y * y * y,
    ]
}

/// Weighted least squares for colour `c`, the higher terms held back.
fn solve(places: &[(f32, f32, Rgb)], trust: &[f32], c: usize) -> [f32; 10] {
    const HOLD: [f64; 10] = [1e-6, 0.02, 0.02, 0.2, 0.2, 0.2, 1.0, 1.0, 1.0, 1.0];
    let mut m = [[0f64; 11]; 10];
    for (row, hold) in HOLD.iter().enumerate() {
        m[row][row] = *hold;
    }
    for (place, trust) in places.iter().zip(trust) {
        let t = terms(place.0, place.1);
        for (r, row) in m.iter_mut().enumerate() {
            let k = f64::from(t[r] * trust);
            for (col, value) in row.iter_mut().take(10).enumerate() {
                *value += k * f64::from(t[col]);
            }
            row[10] += k * f64::from(place.2[c]);
        }
    }
    for col in 0..10 {
        let pivot = (col..10)
            .max_by(|a, b| m[*a][col].abs().total_cmp(&m[*b][col].abs()))
            .unwrap_or(col);
        m.swap(col, pivot);
        let by = m[col][col];
        if by.abs() < 1e-12 {
            continue;
        }
        for row in 0..10 {
            if row != col {
                let k = m[row][col] / by;
                for i in col..11 {
                    let from = m[col][i];
                    m[row][i] -= k * from;
                }
            }
        }
    }
    let mut out = [0f32; 10];
    for (i, value) in out.iter_mut().enumerate() {
        if m[i][i].abs() > 1e-12 {
            *value = (m[i][10] / m[i][i]) as f32;
        }
    }
    out
}

fn middle(values: &mut [f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    let at = values.len() / 2;
    Some(*values.select_nth_unstable_by(at, f32::total_cmp).1)
}

fn average(picture: &[Rgb], w: usize, h: usize, x: usize, y: usize, reach: usize) -> Rgb {
    let mut sum = [0f32; 3];
    let mut count = 0.0;
    for ny in y.saturating_sub(reach)..(y + reach + 1).min(h) {
        for nx in x.saturating_sub(reach)..(x + reach + 1).min(w) {
            for c in 0..3 {
                sum[c] += picture[ny * w + nx][c];
            }
            count += 1.0;
        }
    }
    sum.map(|s| s / count)
}

/// The brightness a tenth of the pixels round each are over: worked out on
/// blocks and read between them.
fn local_high(light: &[f32], w: usize, h: usize, reach: usize) -> Vec<f32> {
    let side = reach.max(4);
    let (bw, bh) = (w.div_ceil(side), h.div_ceil(side));
    let mut blocks = vec![0f32; bw * bh];
    let mut held = Vec::new();
    for by in 0..bh {
        for bx in 0..bw {
            held.clear();
            let half = side / 2;
            for y in (by * side).saturating_sub(half)..((by + 1) * side + half).min(h) {
                for x in (bx * side).saturating_sub(half)..((bx + 1) * side + half).min(w) {
                    held.push(light[y * w + x]);
                }
            }
            let at = held.len() * 9 / 10;
            blocks[by * bw + bx] = *held.select_nth_unstable_by(at, f32::total_cmp).1;
        }
    }
    (0..w * h)
        .map(|i| {
            let u = ((i % w) as f32 + 0.5) / side as f32 - 0.5;
            let v = ((i / w) as f32 + 0.5) / side as f32 - 0.5;
            between_one(&blocks, bw, bh, u, v)
        })
        .collect()
}

/// `picture` where it is weighed, and what is round it where it is not: pulled
/// down a pyramid and pushed back up. `far` where nothing is weighed at all.
fn filled(picture: &[Rgb], weight: &[f32], w: usize, h: usize, far: &[Rgb]) -> Vec<Rgb> {
    if w <= 2 || h <= 2 {
        let total: f32 = weight.iter().sum();
        if total <= 0.0 {
            return far.to_vec();
        }
        let mut sum = [0f32; 3];
        for (c, k) in picture.iter().zip(weight) {
            for i in 0..3 {
                sum[i] += c[i] * k;
            }
        }
        return vec![sum.map(|s| s / total); w * h];
    }
    let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
    let mut sums = vec![[0f32; 3]; hw * hh];
    let mut held = vec![0f32; hw * hh];
    let mut distant = vec![[0f32; 3]; hw * hh];
    for y in 0..h {
        for x in 0..w {
            let (at, to) = (y * w + x, y / 2 * hw + x / 2);
            for c in 0..3 {
                sums[to][c] += picture[at][c] * weight[at];
            }
            held[to] += weight[at];
            distant[to] = far[at];
        }
    }
    for (sum, held) in sums.iter_mut().zip(&held) {
        if *held > 0.0 {
            *sum = sum.map(|s| s / held);
        }
    }
    let some: Vec<f32> = held.iter().map(|k| k.min(1.0)).collect();
    let under = filled(&sums, &some, hw, hh, &distant);
    (0..w * h)
        .map(|at| {
            let k = weight[at];
            if k >= 1.0 {
                return picture[at];
            }
            let u = ((at % w) as f32 + 0.5) / 2.0 - 0.5;
            let v = ((at / w) as f32 + 0.5) / 2.0 - 0.5;
            let b = between(&under, hw, hh, u, v);
            [0, 1, 2].map(|c| picture[at][c] * k + b[c] * (1.0 - k))
        })
        .collect()
}

/// `picture` averaged over squares `reach` either way, twice: a soft blur.
fn softened(picture: &[Rgb], w: usize, h: usize, reach: usize) -> Vec<Rgb> {
    let mut now = picture.to_vec();
    for _ in 0..2 {
        for across in [true, false] {
            let (lines, length) = if across { (h, w) } else { (w, h) };
            let at = |line: usize, i: usize| {
                if across {
                    line * w + i
                } else {
                    i * w + line
                }
            };
            let mut next = now.clone();
            for line in 0..lines {
                let mut sum = [0f64; 3];
                let mut count = 0.0;
                let add = |sum: &mut [f64; 3], c: Rgb, k: f64| {
                    for i in 0..3 {
                        sum[i] += f64::from(c[i]) * k;
                    }
                };
                for i in 0..reach.min(length) {
                    add(&mut sum, now[at(line, i)], 1.0);
                    count += 1.0;
                }
                for i in 0..length {
                    if i + reach < length {
                        add(&mut sum, now[at(line, i + reach)], 1.0);
                        count += 1.0;
                    }
                    if i > reach {
                        add(&mut sum, now[at(line, i - reach - 1)], -1.0);
                        count -= 1.0;
                    }
                    next[at(line, i)] = sum.map(|s| (s / count) as f32);
                }
            }
            now = next;
        }
    }
    now
}

fn corners(w: usize, h: usize, u: f32, v: f32) -> ([usize; 4], f32, f32) {
    let u = u.clamp(0.0, (w - 1) as f32);
    let v = v.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (u as usize, v as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    (
        [y0 * w + x0, y0 * w + x1, y1 * w + x0, y1 * w + x1],
        u - x0 as f32,
        v - y0 as f32,
    )
}

fn between(values: &[Rgb], w: usize, h: usize, u: f32, v: f32) -> Rgb {
    let (at, tx, ty) = corners(w, h, u, v);
    [0, 1, 2].map(|c| {
        let top = values[at[0]][c] * (1.0 - tx) + values[at[1]][c] * tx;
        let low = values[at[2]][c] * (1.0 - tx) + values[at[3]][c] * tx;
        top * (1.0 - ty) + low * ty
    })
}

fn between_one(values: &[f32], w: usize, h: usize, u: f32, v: f32) -> f32 {
    let (at, tx, ty) = corners(w, h, u, v);
    let top = values[at[0]] * (1.0 - tx) + values[at[1]] * tx;
    let low = values[at[2]] * (1.0 - tx) + values[at[3]] * tx;
    top * (1.0 - ty) + low * ty
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sky that lightens downwards, with a wide trunk and a twig over it.
    fn tree() -> Frame {
        let (w, h) = (400u32, 300u32);
        let mut frame = Frame::black(w, h);
        for y in 0..h {
            for x in 0..w {
                let sky = [
                    (90 + y / 3) as u8,
                    (130 + y / 3) as u8,
                    (190 + y / 6) as u8,
                    255,
                ];
                let trunk = (150..220).contains(&x);
                let twig = y == 40 || y == 41;
                frame.set_pixel(x, y, if trunk || twig { [40, 30, 25, 255] } else { sky });
            }
        }
        frame
    }

    #[test]
    fn a_trunk_and_a_twig_stay_and_the_sky_goes() {
        let frame = tree();
        let options = Options::default();
        let cut = cut(&frame, &background(&frame, &options), &options);
        let alpha = |x: u32, y: u32| cut.pixel(x, y).map_or(0, |p| p[3]);
        assert!(alpha(185, 150) > 240, "trunk {}", alpha(185, 150));
        assert!(alpha(60, 40) > 200, "twig {}", alpha(60, 40));
        assert_eq!(alpha(60, 150), 0);
        assert_eq!(alpha(350, 280), 0);
        assert_eq!(alpha(350, 10), 0);
    }

    #[test]
    fn what_is_kept_can_be_one_ink() {
        let frame = tree();
        let options = Options {
            ink: Some([255, 0, 0]),
            ..Options::default()
        };
        let cut = cut(&frame, &background(&frame, &options), &options);
        assert_eq!(cut.pixel(185, 150).map(|p| [p[0], p[1], p[2]]), Some([255, 0, 0]));
    }

    #[test]
    fn a_background_pointed_at_need_not_be_bright() {
        let mut frame = Frame::black(300, 200);
        frame.fill([20, 60, 30, 255]);
        for y in 0..200 {
            for x in 140..150 {
                frame.set_pixel(x, y, [230, 220, 210, 255]);
            }
        }
        let options = Options {
            back: Some([0.1, 0.5]),
            ..Options::default()
        };
        let cut = cut(&frame, &background(&frame, &options), &options);
        let alpha = |x: u32| cut.pixel(x, 100).map_or(0, |p| p[3]);
        assert!(alpha(145) > 240 && alpha(40) == 0, "{} {}", alpha(145), alpha(40));
    }

    #[test]
    fn a_loose_speck_is_cleared_and_a_line_is_not() {
        let mut shares = vec![0u8; 100 * 100];
        shares[5050] = 255;
        for x in 0..100 {
            shares[2000 + x] = 255;
        }
        specks(&mut shares, 100, 100, 1.0);
        assert_eq!(shares[5050], 0);
        assert_eq!(shares[2050], 255);
    }
}
