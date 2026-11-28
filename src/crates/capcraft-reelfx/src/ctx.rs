// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! One frame as the effects see it: its pixels as `0..=1` RGB, how much of
//! each pixel is the person, where the face is, and how far into the effect
//! this frame falls. The operations here are `fx.py`'s frame-wide ones:
//! laying a picture over the frame in front of or behind the person, moving
//! the camera, grading and the glass panels made from what is behind them.

use tiny_skia::Pixmap;

use crate::canvas::{self, Rgb, blur_f32};

/// The frame an effect draws on.
pub struct Ctx {
    pub w: usize,
    pub h: usize,
    /// `w × h × 3` values, 0 to 1.
    pub img: Vec<f32>,
    /// `w × h` values: how much each pixel is the person.
    pub m: Vec<f32>,
    /// The face's centre and width, in pixels of this frame.
    pub fx: f32,
    pub fy: f32,
    pub fw: f32,
    /// Seconds into the effect, and into the clip.
    pub lt: f32,
    pub t: f32,
}

pub fn clamp(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}
pub fn ease_out(x: f32) -> f32 {
    let x = clamp(x);
    1.0 - (1.0 - x).powi(3)
}
pub fn ease_inout(x: f32) -> f32 {
    let x = clamp(x);
    3.0 * x * x - 2.0 * x * x * x
}
pub fn ease_back(x: f32) -> f32 {
    let x = clamp(x) - 1.0;
    let s = 1.7;
    x * x * ((s + 1.0) * x + s) + 1.0
}

/// The grades `fx.py` names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grade {
    Moody,
    Bw,
    Warm,
    Cool,
    Pop,
}

impl Grade {
    pub fn named(name: &str) -> Grade {
        match name {
            "bw" => Grade::Bw,
            "warm" => Grade::Warm,
            "cool" => Grade::Cool,
            "pop" => Grade::Pop,
            _ => Grade::Moody,
        }
    }
}

/// Runs `f` over `buf` cut into bands of whole rows (`unit` values a row,
/// `rows` rows), each band on a core of its own: the first row of the band
/// and the band. A frame at export size is millions of pixels, and one
/// core at a time made an effect hundreds of milliseconds a frame.
pub fn par_rows<T: Send>(buf: &mut [T], unit: usize, rows: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 8);
    let per = rows.div_ceil(cores).max(1);
    if cores == 1 || rows < 64 {
        f(0, buf);
        return;
    }
    std::thread::scope(|scope| {
        for (i, band) in buf.chunks_mut(per * unit).enumerate() {
            let f = &f;
            scope.spawn(move || f(i * per, band));
        }
    });
}

/// `x` to the power `p` for `x` in `0..=1`, from a table: `powf` on every
/// channel of every pixel was most of a grade's time.
fn pow_table(x: f32, table: &[f32; 4097]) -> f32 {
    let at = x.clamp(0.0, 1.0) * 4096.0;
    let i = at as usize;
    if i >= 4096 {
        return table[4096];
    }
    table[i] + (table[i + 1] - table[i]) * (at - i as f32)
}

/// `x^1.3` on `0..=1`, as the moody grade lifts its shadows.
fn pow13(x: f32) -> f32 {
    static T: std::sync::OnceLock<Box<[f32; 4097]>> = std::sync::OnceLock::new();
    pow_table(x, T.get_or_init(|| Box::new(std::array::from_fn(|i| (i as f32 / 4096.0).powf(1.3)))))
}

/// `x^1.6` on `0..=1`, as a spotlight's pool falls off.
pub fn pow16(x: f32) -> f32 {
    static T: std::sync::OnceLock<Box<[f32; 4097]>> = std::sync::OnceLock::new();
    pow_table(x, T.get_or_init(|| Box::new(std::array::from_fn(|i| (i as f32 / 4096.0).powf(1.6)))))
}

/// `rgb` graded `kind`, as `fx.py`'s `grade`.
pub fn grade_px(c: [f32; 3], kind: Grade) -> [f32; 3] {
    let lum = c[0] * 0.299 + c[1] * 0.587 + c[2] * 0.114;
    let cl = |v: f32| v.clamp(0.0, 1.0);
    match kind {
        Grade::Bw => {
            let g = cl((lum - 0.5) * 1.25 + 0.5);
            [g, g, g]
        }
        Grade::Warm => [cl(c[0] * 1.08 + 0.02), cl(c[1] + 0.02), cl(c[2] * 0.86 + 0.02)],
        Grade::Cool => [cl(c[0] * 0.9), cl(c[1]), cl(c[2] * 1.1)],
        Grade::Pop => {
            let mut o = [0.0; 3];
            for i in 0..3 {
                o[i] = cl(((lum + (c[i] - lum) * 1.35) - 0.5) * 1.12 + 0.5);
            }
            o
        }
        Grade::Moody => {
            let shade = [-0.03, 0.035, 0.065];
            let light = [0.05, 0.015, -0.04];
            let mut o = [0.0; 3];
            for i in 0..3 {
                let v = pow13(cl(lum + (c[i] - lum) * 0.6)) * 0.88;
                let v = v + (1.0 - lum) * shade[i] + lum * light[i];
                o[i] = cl((v - 0.42) * 1.12 + 0.42);
            }
            o
        }
    }
}

/// Bilinear sample of a `w × h × ch` buffer at `(x, y)` pixels, clamped
/// to the edge.
fn sample(buf: &[f32], w: usize, h: usize, ch: usize, x: f32, y: f32, out: &mut [f32]) {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    for c in 0..ch {
        let a = buf[(y0 * w + x0) * ch + c];
        let b = buf[(y0 * w + x1) * ch + c];
        let d = buf[(y1 * w + x0) * ch + c];
        let e = buf[(y1 * w + x1) * ch + c];
        out[c] = (a * (1.0 - fx) + b * fx) * (1.0 - fy) + (d * (1.0 - fx) + e * fx) * fy;
    }
}

/// Reflects `v` back into `0..n`, as OpenCV's `BORDER_REFLECT`.
fn reflect(v: f32, n: usize) -> f32 {
    let n = n as f32;
    let mut v = v;
    if v < 0.0 {
        v = -v - 1.0;
    }
    if v >= n {
        let period = 2.0 * n;
        v %= period;
        if v >= n {
            v = period - v - 1.0;
        }
    }
    v.clamp(0.0, n - 1.0)
}

impl Ctx {
    /// The frame's pixel `i` as RGB.
    pub fn px(&self, i: usize) -> [f32; 3] {
        [self.img[i * 3], self.img[i * 3 + 1], self.img[i * 3 + 2]]
    }

    /// A copy of the frame graded `kind`.
    pub fn graded(&self, kind: Grade) -> Vec<f32> {
        let mut out = vec![0f32; self.img.len()];
        let (w, img) = (self.w, &self.img);
        par_rows(&mut out, w * 3, self.h, |y0, band| {
            let start = y0 * w * 3;
            for (o, i) in band.chunks_exact_mut(3).zip(img[start..].chunks_exact(3)) {
                o.copy_from_slice(&grade_px([i[0], i[1], i[2]], kind));
            }
        });
        out
    }

    /// The frame mixed `k` of the way to `other`, pixel by pixel.
    pub fn mix_with(&mut self, other: &[f32], k: f32) {
        if k <= 0.0 {
            return;
        }
        for (v, o) in self.img.iter_mut().zip(other) {
            *v = *v * (1.0 - k) + o * k;
        }
    }

    /// The edges darkened by `strength`.
    pub fn vignette(&mut self, strength: f32) {
        if strength <= 0.0 {
            return;
        }
        let (w, h) = (self.w as f32, self.h as f32);
        for y in 0..self.h {
            let dy = ((y as f32 - h / 2.0) / (h * 0.7)).powi(2);
            for x in 0..self.w {
                let d = (((x as f32 - w / 2.0) / (w * 0.75)).powi(2) + dy).clamp(0.0, 1.0);
                let f = 1.0 - strength * d;
                let i = (y * self.w + x) * 3;
                self.img[i] *= f;
                self.img[i + 1] *= f;
                self.img[i + 2] *= f;
            }
        }
    }

    /// Lays `layer` over the frame with its top-left at `(x, y)`. `behind`
    /// hides it wherever the person is; `guard` only where it would cover
    /// the face (`fx.py`'s `comp`).
    pub fn comp(&mut self, layer: &Pixmap, x: f32, y: f32, alpha: f32, behind: bool, guard: bool) {
        if alpha <= 0.0 {
            return;
        }
        let (lw, lh) = (layer.width() as i64, layer.height() as i64);
        let (x, y) = (x.round() as i64, y.round() as i64);
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + lw).min(self.w as i64), (y + lh).min(self.h as i64));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let data = layer.data();
        for py in y0..y1 {
            for px in x0..x1 {
                let li = (((py - y) * lw + (px - x)) * 4) as usize;
                let la = f32::from(data[li + 3]) / 255.0;
                if la <= 0.0 {
                    continue;
                }
                let di = py as usize * self.w + px as usize;
                let k = alpha * self.cover(di, px as f32, py as f32, behind, guard);
                if k <= 0.0 {
                    continue;
                }
                let a = la * k;
                for c in 0..3 {
                    let s = f32::from(data[li + c]) / 255.0 * k;
                    let d = &mut self.img[di * 3 + c];
                    *d = *d * (1.0 - a) + s;
                }
            }
        }
    }

    /// How much of a layer shows at frame pixel `i`, `(x, y)`.
    fn cover(&self, i: usize, x: f32, y: f32, behind: bool, guard: bool) -> f32 {
        let m = self.m[i];
        if behind {
            1.0 - m
        } else if guard {
            let e = ((x - self.fx) / (self.fw * 0.95)).powi(2) + ((y - self.fy) / (self.fw * 1.35)).powi(2);
            1.0 - ((1.2 - e) / 0.3).clamp(0.0, 1.0) * m
        } else {
            1.0
        }
    }

    /// Draws `im` centred on `(cx, cy)`, scaled and turned `rot` degrees
    /// counter-clockwise (`fx.py`'s `place`).
    #[allow(clippy::too_many_arguments)]
    pub fn place(&mut self, im: &Pixmap, cx: f32, cy: f32, scale: f32, rot: f32, alpha: f32, behind: bool, guard: bool) {
        if scale <= 0.01 || alpha <= 0.0 {
            return;
        }
        if (scale - 1.0).abs() <= 0.005 && rot.abs() <= 0.05 {
            self.comp(im, cx - im.width() as f32 / 2.0, cy - im.height() as f32 / 2.0, alpha, behind, guard);
            return;
        }
        let (iw, ih) = (im.width() as f32, im.height() as f32);
        let a = rot.to_radians();
        let (cs, sn) = (a.cos(), a.sin());
        let (hw, hh) = (iw * scale / 2.0, ih * scale / 2.0);
        let ex = hw * cs.abs() + hh * sn.abs();
        let ey = hw * sn.abs() + hh * cs.abs();
        let x0 = ((cx - ex).floor() as i64).max(0);
        let y0 = ((cy - ey).floor() as i64).max(0);
        let x1 = ((cx + ex).ceil() as i64).min(self.w as i64);
        let y1 = ((cy + ey).ceil() as i64).min(self.h as i64);
        let data = im.data();
        let (wi, hi) = (im.width() as usize, im.height() as usize);
        let at = |x: i64, y: i64, c: usize| -> f32 {
            if x < 0 || y < 0 || x >= wi as i64 || y >= hi as i64 {
                0.0
            } else {
                f32::from(data[(y as usize * wi + x as usize) * 4 + c]) / 255.0
            }
        };
        for py in y0..y1 {
            for px in x0..x1 {
                // the frame pixel back into the picture: undo the turn, then the scale
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let sx = (dx * cs - dy * sn) / scale + iw / 2.0 - 0.5;
                let sy = (dx * sn + dy * cs) / scale + ih / 2.0 - 0.5;
                if sx < -1.0 || sy < -1.0 || sx > iw || sy > ih {
                    continue;
                }
                let (fx0, fy0) = (sx.floor() as i64, sy.floor() as i64);
                let (tx, ty) = (sx - fx0 as f32, sy - fy0 as f32);
                let mut v = [0f32; 4];
                for (c, out) in v.iter_mut().enumerate() {
                    *out = (at(fx0, fy0, c) * (1.0 - tx) + at(fx0 + 1, fy0, c) * tx) * (1.0 - ty)
                        + (at(fx0, fy0 + 1, c) * (1.0 - tx) + at(fx0 + 1, fy0 + 1, c) * tx) * ty;
                }
                if v[3] <= 0.0 {
                    continue;
                }
                let di = py as usize * self.w + px as usize;
                let k = alpha * self.cover(di, px as f32, py as f32, behind, guard);
                if k <= 0.0 {
                    continue;
                }
                let a = v[3] * k;
                for c in 0..3 {
                    let d = &mut self.img[di * 3 + c];
                    *d = *d * (1.0 - a) + v[c] * k;
                }
            }
        }
    }

    /// [`Ctx::place`] at scale 1, unturned.
    pub fn put(&mut self, im: &Pixmap, cx: f32, cy: f32, alpha: f32, behind: bool, guard: bool) {
        self.place(im, cx, cy, 1.0, 0.0, alpha, behind, guard);
    }

    /// The frame and the person zoomed `zoom` times towards `(cx, cy)`,
    /// never past twice (`fx.py`'s `camera`).
    pub fn camera(&mut self, zoom: f32, cx: f32, cy: f32) {
        let zoom = zoom.min(2.0);
        if zoom <= 1.002 {
            return;
        }
        let (w, h) = (self.w as f32, self.h as f32);
        let (cw, ch) = (w / zoom, h / zoom);
        let x0 = (cx - cw / 2.0).clamp(0.0, w - cw);
        let y0 = (cy - ch / 2.0).clamp(0.0, h - ch);
        self.warp(|x, y| (x / zoom + x0, y / zoom + y0), true);
        self.fx = (self.fx - x0) * zoom;
        self.fy = (self.fy - y0) * zoom;
        self.fw *= zoom;
    }

    /// The frame and mask resampled: each output pixel `(x, y)` reads the
    /// source at `src(x, y)`, reflected past the edges when `reflect`, and
    /// carried out otherwise.
    pub fn warp(&mut self, src: impl Fn(f32, f32) -> (f32, f32) + Sync, mirror: bool) {
        let (w, h) = (self.w, self.h);
        let mut img = vec![0f32; self.img.len()];
        let mut m = vec![0f32; self.m.len()];
        let at = |x: usize, y: usize| {
            let (mut sx, mut sy) = src(x as f32, y as f32);
            if mirror {
                sx = reflect(sx, w);
                sy = reflect(sy, h);
            }
            (sx, sy)
        };
        let (from_img, from_m) = (&self.img, &self.m);
        par_rows(&mut img, w * 3, h, |y0, band| {
            let mut rgb = [0f32; 3];
            for (k, px) in band.chunks_exact_mut(3).enumerate() {
                let (sx, sy) = at(k % w, y0 + k / w);
                sample(from_img, w, h, 3, sx, sy, &mut rgb);
                px.copy_from_slice(&rgb);
            }
        });
        par_rows(&mut m, w, h, |y0, band| {
            let mut one = [0f32; 1];
            for (k, v) in band.iter_mut().enumerate() {
                let (sx, sy) = at(k % w, y0 + k / w);
                sample(from_m, w, h, 1, sx, sy, &mut one);
                *v = one[0];
            }
        });
        self.img = img;
        self.m = m;
    }

    /// Zooms on the face, keeping it `head_at` of the way down the frame.
    pub fn face_cam(&mut self, zoom: f32, head_at: f32) {
        let z = zoom.min(2.0);
        let (fx, fy) = (self.fx, self.fy);
        self.camera(z, fx, fy + (self.h as f32 / z) * (0.5 - head_at));
    }

    /// The whole picture, and the person, slid down `dy` pixels.
    pub fn shift(&mut self, dy: f32) {
        if dy.abs() < 0.5 {
            return;
        }
        self.warp(|x, y| (x, y - dy), false);
        self.fy += dy;
    }

    /// A blurred copy of the frame's region `(x0, y0)` `w × h`, `pad` past
    /// it each side, clamped to the frame: `(w + 2 pad) × (h + 2 pad) × 3`.
    pub fn blurred_region(&self, x0: i64, y0: i64, w: usize, h: usize, pad: usize, sigma: f32) -> Vec<f32> {
        let (rw, rh) = (w + 2 * pad, h + 2 * pad);
        let mut reg = vec![0f32; rw * rh * 3];
        for y in 0..rh {
            let sy = (y0 - pad as i64 + y as i64).clamp(0, self.h as i64 - 1) as usize;
            for x in 0..rw {
                let sx = (x0 - pad as i64 + x as i64).clamp(0, self.w as i64 - 1) as usize;
                let s = (sy * self.w + sx) * 3;
                reg[(y * rw + x) * 3..(y * rw + x) * 3 + 3].copy_from_slice(&self.img[s..s + 3]);
            }
        }
        blur_f32(&mut reg, rw, rh, 3, sigma);
        reg
    }

    /// A panel of glass made from whatever sits behind it (`fx.py`'s
    /// `glass`): with `liquid`, a light blur bent at the rim, a red and blue
    /// fringe and a lit edge; otherwise a frosted card with a thin rim.
    #[allow(clippy::too_many_arguments)]
    pub fn glass(&self, w: f32, h: f32, r: f32, cx: f32, cy: f32, amt: f32, tint: Rgb, blur: Option<f32>, alpha: f32, dark: f32, liquid: bool) -> Pixmap {
        let (w, h) = (w.max(2.0) as usize, h.max(2.0) as usize);
        let blur = blur.unwrap_or((self.w as f32 / 40.0).max(8.0));
        let (x0, y0) = ((cx - w as f32 / 2.0) as i64, (cy - h as f32 / 2.0) as i64);
        let tint = [f32::from(tint.0) / 255.0, f32::from(tint.1) / 255.0, f32::from(tint.2) / 255.0];
        let mut out = vec![0f32; w * h * 3];
        let mut alpha_mask = vec![0u8; w * h];
        if !liquid {
            let pad = (blur * 2.0) as usize;
            let reg = self.blurred_region(x0, y0, w, h, pad, blur);
            let rw = w + 2 * pad;
            for y in 0..h {
                for x in 0..w {
                    for c in 0..3 {
                        let v = reg[((y + pad) * rw + x + pad) * 3 + c];
                        out[(y * w + x) * 3 + c] = (v * (1.0 - dark) * (1.0 - amt) + tint[c] * amt).clamp(0.0, 1.0);
                    }
                }
            }
            let mut m = canvas::blank(w as f32, h as f32);
            canvas::rounded(&mut m, 0.0, 0.0, w as f32 - 1.0, h as f32 - 1.0, r, Some((255, 255, 255, alpha as u8)), None);
            for (i, px) in m.data().chunks_exact(4).enumerate() {
                alpha_mask[i] = px[3];
            }
            let mut im = to_pixmap(&out, w, h, &alpha_mask);
            canvas::rounded(&mut im, 1.0, 1.0, w as f32 - 2.0, h as f32 - 2.0, r, None, Some(((255, 255, 255, 120), (self.w as f32 / 540.0).max(2.0))));
            return im;
        }
        let u = self.w as f32 / 2160.0;
        let pad = (140.0 * u) as usize + 4;
        let tex = self.blurred_region(x0, y0, w, h, pad, (blur * 0.3).max(3.0));
        let (tw, th) = (w + 2 * pad, h + 2 * pad);
        let r = r.max(2.0);
        let mut px = [0f32; 3];
        for y in 0..h {
            for x in 0..w {
                let (xf, yf) = (x as f32, y as f32);
                let qx = (xf - w as f32 / 2.0).abs() - (w as f32 / 2.0 - r);
                let qy = (yf - h as f32 / 2.0).abs() - (h as f32 / 2.0 - r);
                let sdf = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
                let d = (-sdf).max(0.0) / u;
                let (mut nx, mut ny) = (xf - w as f32 / 2.0, yf - h as f32 / 2.0);
                let nl = (nx * nx + ny * ny).sqrt() + 1e-3;
                nx /= nl;
                ny /= nl;
                let k = (70.0 * (-d / 14.0).exp() + 34.0 * (-d / 70.0).exp() + 10.0 * (-d / 260.0).exp()) * u;
                for (c, fr) in [1.10f32, 1.0, 0.90].iter().enumerate() {
                    let sx = reflect(xf + pad as f32 + nx * k * fr, tw);
                    let sy = reflect(yf + pad as f32 + ny * k * fr, th);
                    sample(&tex, tw, th, 3, sx, sy, &mut px);
                    out[(y * w + x) * 3 + c] = px[c];
                }
                let spec = (-d / 5.0).exp() * (0.55 - 0.45 * (nx * 0.6 + ny * 0.8)).clamp(0.0, 1.0);
                let lift = spec * 0.85 + (-d / 30.0).exp() * 0.10;
                for c in 0..3 {
                    let v = out[(y * w + x) * 3 + c] * (1.0 - dark * 0.3) * 0.9 + tint[c] * amt * 0.25 + 0.05 * (1.0 - 0.30 * (yf / h as f32)) - 0.04;
                    out[(y * w + x) * 3 + c] = (v + lift).clamp(0.0, 1.0);
                }
                alpha_mask[y * w + x] = ((0.5 - sdf).clamp(0.0, 1.0) * alpha) as u8;
            }
        }
        to_pixmap(&out, w, h, &alpha_mask)
    }

    /// The frame's region around the face as a `w × h` picture: `fx.py`'s
    /// `np_thumb`.
    pub fn thumb(&self, w: f32, h: f32) -> Pixmap {
        let ar = w / h;
        let ch = self.h as f32 * 0.62;
        let cw = ch * ar;
        let x0 = (self.fx - cw / 2.0).clamp(0.0, (self.w as f32 - cw).max(0.0));
        let y0 = (self.fy - ch * 0.4).clamp(0.0, (self.h as f32 - ch).max(0.0));
        self.region(x0, y0, cw, ch, w, h)
    }

    /// The frame's `(x0, y0)` `cw × ch` rectangle, resized to `w × h`.
    pub fn region(&self, x0: f32, y0: f32, cw: f32, ch: f32, w: f32, h: f32) -> Pixmap {
        let (w, h) = (w.max(1.0) as usize, h.max(1.0) as usize);
        let mut rgb = vec![0f32; w * h * 3];
        let mut px = [0f32; 3];
        for y in 0..h {
            for x in 0..w {
                let sx = x0 + (x as f32 + 0.5) * cw / w as f32 - 0.5;
                let sy = y0 + (y as f32 + 0.5) * ch / h as f32 - 0.5;
                sample(&self.img, self.w, self.h, 3, sx, sy, &mut px);
                rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&px);
            }
        }
        to_pixmap(&rgb, w, h, &vec![255u8; w * h])
    }

    /// The whole frame as a picture, `w × h`.
    pub fn whole(&self, w: f32, h: f32) -> Pixmap {
        self.region(0.0, 0.0, self.w as f32, self.h as f32, w, h)
    }

    /// Area-averaged copy of the frame at `sw × sh` (OpenCV's `INTER_AREA`).
    pub fn shrunk(&self, sw: usize, sh: usize) -> Vec<f32> {
        shrink(&self.img, self.w, self.h, 3, sw, sh)
    }
}

/// `buf` (`w × h × ch`) area-averaged down to `sw × sh`.
pub fn shrink(buf: &[f32], w: usize, h: usize, ch: usize, sw: usize, sh: usize) -> Vec<f32> {
    let (sw, sh) = (sw.max(1), sh.max(1));
    let mut out = vec![0f32; sw * sh * ch];
    for y in 0..sh {
        let ya = y * h / sh;
        let yb = ((y + 1) * h / sh).max(ya + 1).min(h);
        for x in 0..sw {
            let xa = x * w / sw;
            let xb = ((x + 1) * w / sw).max(xa + 1).min(w);
            let n = ((yb - ya) * (xb - xa)) as f32;
            for c in 0..ch {
                let mut s = 0.0;
                for yy in ya..yb {
                    for xx in xa..xb {
                        s += buf[(yy * w + xx) * ch + c];
                    }
                }
                out[(y * sw + x) * ch + c] = s / n;
            }
        }
    }
    out
}

/// `0..=1` RGB with an alpha byte per pixel, as a premultiplied pixmap.
pub fn to_pixmap(rgb: &[f32], w: usize, h: usize, alpha: &[u8]) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1) as u32, h.max(1) as u32).expect("a pixmap");
    for (i, px) in pm.data_mut().chunks_exact_mut(4).enumerate() {
        let a = f32::from(alpha[i]) / 255.0;
        for c in 0..3 {
            px[c] = (rgb[i * 3 + c].clamp(0.0, 1.0) * a * 255.0 + 0.5) as u8;
        }
        px[3] = alpha[i];
    }
    pm
}
