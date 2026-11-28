// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! `fx.py`'s packs three to five: showcase effects, their named looks, the
//! over-the-shoulder zoom and messages, and the phrase pops.

use std::f32::consts::PI;

use tiny_skia::Pixmap;

use super::{Rng, card, cover, parts, round_corners, vary};
use crate::canvas::{self, Anchor, Kind, Look, Rgb, blank, blur_f32, fit_text, op, over, rrect, shadowed, text, text_img, with_a};
use crate::ctx::{Ctx, clamp, ease_back, ease_inout, ease_out};
use crate::effects::pack2::{graph_line, num_parts};
use crate::{BLUE, Effect, RED, Run, pop};

const WHITE: Rgb = (255, 255, 255);
const INKY: Rgb = (23, 23, 27);
const TAPE: Rgb = (255, 225, 77);
const CRAYON: Rgb = (200, 40, 56);
const CREAMY: Rgb = (253, 246, 200);
const GLOW_GREEN: Rgb = (52, 211, 102);

/// Letter-spaced text, `fx.py`'s `tracked`.
fn tracked(s: &str, size: f32, kind: Kind, fill: Rgb, spacing: f32) -> Pixmap {
    let n = s.chars().count();
    let w: f32 = s.chars().map(|ch| canvas::length(kind, size, &ch.to_string())).sum::<f32>() + spacing * n.saturating_sub(1) as f32;
    let (ascent, descent) = canvas::face(kind).metrics(size);
    let mut im = blank(w + 8.0, ascent + descent + size * 0.4);
    let mut x = 4.0;
    for ch in s.chars() {
        let t = ch.to_string();
        text(&mut im, x, size * 0.2, &t, kind, size, op(fill), Anchor::LA);
        x += canvas::length(kind, size, &t) + spacing;
    }
    im
}

/// Text filled top to bottom from `top` to `bot`, with an optional glow
/// (`fx.py`'s `gradient_text`).
fn gradient_text(s: &str, size: f32, kind: Kind, top: Rgb, bot: Rgb, glow: Option<(Rgb, f32)>) -> Pixmap {
    let pad = size * 0.25 + glow.map_or(0.0, |g| g.1 * 2.5);
    let (ascent, descent) = canvas::face(kind).metrics(size);
    let (w, h) = (canvas::length(kind, size, s) + 2.0 * pad, ascent + descent + 2.0 * pad);
    let mut m = blank(w, h);
    text(&mut m, pad, pad, s, kind, size, (255, 255, 255, 255), Anchor::LA);
    let mut im = blank(w, h);
    let hh = h.max(2.0);
    let iw = im.width() as usize;
    for (i, (px, mp)) in im.data_mut().chunks_exact_mut(4).zip(m.data().chunks_exact(4)).enumerate() {
        let t = (i / iw) as f32 / (hh - 1.0);
        let a = f32::from(mp[3]) / 255.0;
        let col = [f32::from(top.0) * (1.0 - t) + f32::from(bot.0) * t, f32::from(top.1) * (1.0 - t) + f32::from(bot.1) * t, f32::from(top.2) * (1.0 - t) + f32::from(bot.2) * t];
        for c in 0..3 {
            px[c] = (col[c] * a) as u8;
        }
        px[3] = mp[3];
    }
    if let Some((gc, gr)) = glow {
        let mut gl = m.clone();
        for px in gl.data_mut().chunks_exact_mut(4) {
            let a = f32::from(px[3]) / 255.0;
            px[0] = (f32::from(gc.0) * a) as u8;
            px[1] = (f32::from(gc.1) * a) as u8;
            px[2] = (f32::from(gc.2) * a) as u8;
        }
        canvas::blur(&mut gl, gr);
        let mut out = gl.clone();
        over(&mut out, &gl, 0, 0);
        over(&mut out, &im, 0, 0);
        return out;
    }
    im
}

/// A four-point sparkle with a soft glow (`fx.py`'s `star`).
fn star(size: f32) -> Pixmap {
    let s = size.max(8.0).floor();
    let h = s / 2.0;
    let t = s * 0.075;
    let mut im = blank(s, s);
    canvas::polygon(&mut im, &[(h, s * 0.02), (h + t, h - t), (s * 0.98, h), (h + t, h + t), (h, s * 0.98), (h - t, h + t), (s * 0.02, h), (h - t, h - t)], Some((255, 244, 222, 255)), None);
    let mut g = im.clone();
    canvas::blur(&mut g, s * 0.07);
    let mut out = g.clone();
    over(&mut out, &g, 0, 0);
    over(&mut out, &im, 0, 0);
    let r = s * 0.07;
    canvas::ellipse(&mut out, h - r, h - r, h + r, h + r, Some(op(WHITE)), None);
    out
}

pub(crate) fn spotlight_sparkle(c: &mut Ctx, e: &mut Run) {
    let k = e.ramp(c, 0.25, 0.2);
    let (w, h) = (c.w as f32, c.h as f32);
    c.face_cam(1.0 + (e.f("push", 1.12) - 1.0) * ease_inout(c.lt / e.d), 0.46);
    let gold = e.color("color", (255, 190, 128));
    let gf = [f32::from(gold.0) / 255.0, f32::from(gold.1) / 255.0, f32::from(gold.2) / 255.0];
    let (ox, oy) = (c.fx, c.fy - c.fw * 1.1);
    // the halo round the person: their mask blurred, past its own edge
    let mut halo = c.m.clone();
    blur_f32(&mut halo, c.w, c.h, 1, w * 0.012);
    for y in 0..c.h {
        for x in 0..c.w {
            let i = y * c.w + x;
            let (xf, yf) = (x as f32, y as f32);
            let d = ((xf - c.fx) / (w * 0.55)).powi(2) + ((yf - (c.fy + h * 0.15)) / (h * 0.5)).powi(2);
            let pool = (1.0 - d).clamp(0.0, 1.0).powf(1.5);
            let m = c.m[i];
            let f = (1.0 - k * (0.92 - 0.3 * pool)) * (1.0 - m) + m * (1.0 + 0.06 * k);
            let ang = (yf - oy).atan2(xf - ox);
            let rad = ((xf - ox).powi(2) + (yf - oy).powi(2)).sqrt();
            let rays = (0.5 + 0.5 * (16.0 * ang + c.lt * 0.9).cos()).powi(5) * (-rad / (h * 0.42)).exp() + 0.9 * (-(rad / (h * 0.2)).powi(2)).exp();
            let hl = (halo[i] * 1.6 - m).clamp(0.0, 1.0);
            let add = (rays * 0.32 + hl * 0.7) * k * (1.0 - m);
            for ch in 0..3 {
                c.img[i * 3 + ch] = c.img[i * 3 + ch] * f + add * gf[ch];
            }
        }
    }
    let word = e.s("text", "STAR");
    let wim = e.cache("w", || {
        let mut size = (h * 0.2).floor();
        while size > 20.0 && canvas::length(Kind::Didonei, size, &word) > w * 0.86 {
            size = (size * 0.94).floor();
        }
        let im = gradient_text(&word, size, Kind::Didonei, (255, 255, 250), (255, 200, 130), Some((gold, size * 0.06)));
        shadowed(&im, (size * 0.05).floor(), 0.0, 200)
    });
    // a shine across the word
    let mut w2 = (*wim).clone();
    let (ww, wh) = (w2.width() as f32, w2.height() as f32);
    let centre = (-0.3 + 1.6 * clamp((c.lt - 0.25) / 0.9)) * ww * 1.3;
    let iw = w2.width() as usize;
    for (i, px) in w2.data_mut().chunks_exact_mut(4).enumerate() {
        let (x, y) = ((i % iw) as f32, (i / iw) as f32);
        let band = (-(((x + y * 0.5) - centre) / (ww * 0.07)).powi(2)).exp();
        let a = px[3];
        for c in &mut px[..3] {
            *c = (f32::from(*c) + band * 0.9 * f32::from(a)).min(f32::from(a)) as u8;
        }
    }
    let s = 1.08 - 0.08 * ease_out(c.lt / 0.5);
    c.place(&w2, w / 2.0, (wh * 0.5 + h * 0.06).max(c.fy - c.fw * 1.05 - wh * 0.28), s, 0.0, k, true, false);
    let spr = e.cache("star", || star(w * 0.07));
    let spots = e.cache("sp", || {
        let mut r = Rng::new(9);
        (0..48).map(|_| (r.uniform(0.04, 0.96), r.uniform(0.07, 0.62), r.uniform(0.025, 0.07), r.uniform(0.0, 6.3), r.uniform(5.0, 9.0), r.next() < 0.35)).collect::<Vec<_>>()
    });
    for &(x, y, sz, ph, sp, front) in spots.iter() {
        let a = (ph + c.lt * sp).sin().max(0.0).powi(2) * k;
        if a < 0.03 {
            continue;
        }
        let sc = sz / 0.07 * (0.45 + 0.75 * a);
        c.place(&spr, x * w, y * h, sc, c.lt * 40.0 + ph * 30.0, a, !front, front);
    }
}

pub(crate) fn cover_editorial(c: &mut Ctx, e: &mut Run) {
    let k = ease_out(c.lt / 0.35);
    let al = e.fade(c, 0.2);
    let (w, h) = (c.w as f32, c.h as f32);
    // soft film: a little desaturated and cream, with a bloom of the highlights
    let mut bloom = vec![0f32; c.img.len()];
    for (b, v) in bloom.iter_mut().zip(&c.img) {
        *b = (v - 0.62).clamp(0.0, 1.0);
    }
    blur_f32(&mut bloom, c.w, c.h, 3, w * 0.012);
    let mut target = c.img.clone();
    for i in 0..c.w * c.h {
        let p = c.px(i);
        let lum = p[0] * 0.299 + p[1] * 0.587 + p[2] * 0.114;
        let tint = [252.0 / 255.0, 238.0 / 255.0, 228.0 / 255.0];
        for ch in 0..3 {
            let soft = (p[ch] * 0.82 + lum * 0.18) * 0.9 + tint[ch] * 0.1;
            target[i * 3 + ch] = (soft + bloom[i * 3 + ch] * 0.55).clamp(0.0, 1.0);
        }
    }
    c.mix_with(&target, k);
    let ink = e.color("color", (26, 20, 22));
    let rose = e.color("accent", (168, 70, 94));
    let cream = (252, 244, 236);
    let title = e.s("text", "EFFECTS").to_uppercase();
    let m = e.cache("m", || {
        let mut size = (h * 0.16).floor();
        while size > 20.0 && canvas::length(Kind::Didoneb, size, &title) + size * 0.04 * title.chars().count() as f32 > w * 0.92 {
            size = (size * 0.95).floor();
        }
        tracked(&title, size, Kind::Didoneb, ink, size * 0.04)
    });
    let mh = m.height() as f32;
    let s = 1.05 - 0.05 * ease_out(c.lt / 0.5);
    c.place(&m, w / 2.0, h * 0.085 + mh / 2.0, s, 0.0, al * k, true, false);
    let issue = e.s("issue", "THE ONE-CLIP ISSUE  ·  NO. 42  ·  2026");
    let iss = e.cache("i", || tracked(&issue, (w * 0.022).floor(), Kind::Ui, ink, w * 0.006));
    c.place(&iss, w / 2.0, h * 0.075, 1.0, 0.0, al * k, true, false);
    let rule = e.cache("r", || rrect(w * 0.86, (w / 600.0).floor().max(3.0), 1.0, op(ink), None));
    c.place(&rule, w / 2.0, h * 0.087 + mh, 1.0, 0.0, al * k * 0.8, true, false);
    let lines = parts(&e.s("lines", "One clip,|42 looks.|NO EDITOR  ·  NO APPS"));
    let styles = [(Kind::Didonei, 0.085, ink), (Kind::Didoneb, 0.105, rose), (Kind::Ui, 0.03, ink)];
    let mut y = h * e.f("lines_y", 0.575);
    for (j, ln) in lines.iter().enumerate() {
        let (kind, sz, col) = styles[j.min(2)];
        let im = e.cache(&format!("l{j}"), || {
            if kind == Kind::Ui { tracked(ln, (w * sz).floor(), kind, col, w * 0.006) } else { text_img(ln, w * sz, kind, col, Look { glow: Some((cream, w * 0.012)), ..Look::default() }) }
        });
        let kj = ease_out((c.lt - 0.2 - 0.1 * j as f32) / 0.35);
        c.comp(&im, w * 0.05 - (1.0 - kj) * w * 0.03, y, al * kj, false, false);
        y += im.height() as f32 * if kind == Kind::Ui { 1.6 } else { 0.82 };
    }
    let right = parts(&e.s("right", "the art of the|effortless edit"));
    let mut y = h * e.f("right_y", 0.715);
    for (j, ln) in right.iter().enumerate() {
        let im = e.cache(&format!("r{j}"), || text_img(ln, w * 0.058, Kind::Didonei, ink, Look { glow: Some((cream, w * 0.01)), ..Look::default() }));
        let kj = ease_out((c.lt - 0.45 - 0.1 * j as f32) / 0.35);
        c.comp(&im, w * 0.95 - im.width() as f32 + (1.0 - kj) * w * 0.03, y, al * kj, false, false);
        y += im.height() as f32 * 0.78;
    }
}

/// A ring chart with its value and label inside (`fx.py`'s `donut`).
fn donut(size: f32, pct: f32, col: Rgb, label: &str, value: &str) -> Pixmap {
    let s = (size * 2.0).floor();
    let mut im = blank(s, s);
    let wd = (s * 0.1).floor();
    let pad = (s * 0.08).floor() + wd / 2.0;
    canvas::ellipse(&mut im, pad, pad, s - pad, s - pad, None, Some(((255, 255, 255, 46), wd)));
    if pct > 0.002 {
        canvas::arc(&mut im, pad, pad, s - pad, s - pad, -90.0, -90.0 + 360.0 * pct, op(col), wd);
    }
    text(&mut im, s / 2.0, s / 2.0 - s * 0.02, value, Kind::Uib, s * 0.2, op(WHITE), Anchor::MM);
    text(&mut im, s / 2.0, s / 2.0 + s * 0.15, &label.to_uppercase(), Kind::Mono, s * 0.065, (255, 255, 255, 170), Anchor::MM);
    canvas::resized(&im, size, size)
}

pub(crate) fn glass_bento(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let al = e.fade(c, 0.2) * clamp(c.lt / 0.12);
    if al <= 0.0 {
        return;
    }
    let green = e.color("color", (52, 211, 110));
    let peach = (255, 160, 122);
    let q = ease_out(c.lt / 0.4);
    let (pw, ph) = ((w * 0.9).floor(), (h * 0.3).floor());
    let (pcx, pcy) = (w / 2.0, h * 0.08 + ph / 2.0 + (1.0 - q) * h * 0.02);
    let mut pan = c.glass(pw, ph, w * 0.04, pcx, pcy, 0.06, WHITE, Some((w / 30.0).floor().max(10.0)), 240.0, 0.62, true);
    let mut lay = blank(pw, ph);
    let pad = pw * 0.05;
    let g = ease_inout((c.lt - 0.2) / (e.d * 0.75).max(0.3));
    text(&mut lay, pad, pad * 0.8, &e.s("text", "VIEWS").to_uppercase(), Kind::Mono, pw * 0.026, (255, 255, 255, 180), Anchor::LA);
    let (a, _, _) = num_parts(&e.s("from", "0"));
    let (b, suf, dp) = num_parts(&e.s("to", "197K"));
    let s = format!("{:.dp$}{suf}", a + (b - a) * g);
    text(&mut lay, pw - pad - canvas::length(Kind::Uib, pw * 0.085, &s), pad * 0.45, &s, Kind::Uib, pw * 0.085, op(WHITE), Anchor::LA);
    let (x0, x1, y0, y1) = (pad, pw - pad, ph - pad * 0.8, ph * 0.3);
    for j in 1..4 {
        let yy = y0 - (y0 - y1) * j as f32 / 3.3;
        canvas::line_flat(&mut lay, &[(x0, yy), (x1, yy)], (255, 255, 255, 34), (w / 1000.0).max(1.0));
    }
    let n = 80;
    let pts: Vec<(f32, f32)> = (0..=(n as f32 * g) as usize)
        .map(|j| {
            let t = j as f32 / n as f32;
            (x0 + (x1 - x0) * t, y0 - (y0 - y1) * t.powf(1.7) + (t * 17.0).sin() * ph * 0.02 * (1.0 - t))
        })
        .collect();
    if pts.len() > 1 {
        graph_line(&mut lay, &pts, y0, x0, green, (pw * 0.006).floor().max(4.0));
    }
    over(&mut pan, &lay, 0, 0);
    c.put(&pan, pcx, pcy, al * q, true, false);
    let tile = |c: &mut Ctx, i: usize, cx: f32, cy: f32, tw: f32, th: f32, draw: &dyn Fn(&mut Pixmap, &Ctx, f32, f32)| {
        let k = pop(c, 0.15 + 0.12 * i as f32, 0.4);
        if k <= 0.0 {
            return;
        }
        let mut t = c.glass(tw, th, w * 0.035, cx, cy, 0.08, WHITE, Some((w / 34.0).floor().max(10.0)), 240.0, 0.5, true);
        draw(&mut t, c, tw, th);
        c.place(&shadowed(&t, (w / 80.0).floor().max(8.0), (w / 120.0).floor(), 90), cx, cy, 0.7 + 0.3 * k, 0.0, al * clamp(k * 2.0), false, false);
    };
    let dn = e.s("donut", "29%::ERASE share");
    let (dv, dl) = dn.split_once("::").map_or((dn.clone(), String::new()), |(a, b)| (a.to_owned(), b.to_owned()));
    let pct = num_parts(&dv).0 / 100.0;
    let kp = e.s("kpi", "672K::views · top 7 reels");
    let (kv, kl) = kp.split_once("::").map_or((kp.clone(), String::new()), |(a, b)| (a.to_owned(), b.to_owned()));
    let bs = e.s("bars", "197|140|124|83|50::top reels · K views");
    let (bv, bl) = bs.split_once("::").map_or((bs.clone(), String::new()), |(a, b)| (a.to_owned(), b.to_owned()));
    let vals: Vec<f32> = bv.split('|').map(|v| num_parts(v).0).collect();
    let tw = w * 0.25;
    let gap = w * 0.02;
    let ty = (h * 0.5).max(c.fy + c.fw * 1.1);
    tile(c, 0, w * 0.15, ty + tw * 0.5, tw, tw, &|t, c, tw, th| {
        let p = ease_out((c.lt - 0.35) / 0.8) * pct;
        let d = donut(tw * 0.86, p, peach, &dl, &format!("{}%", (p * 100.0).round() as i32));
        over(t, &d, ((tw - d.width() as f32) / 2.0) as i32, ((th - d.height() as f32) / 2.0) as i32);
    });
    tile(c, 1, w * 0.15, ty + tw + gap + tw * 0.28, tw, tw * 0.56, &|t, c, tw, th| {
        let (b, suf, dp) = num_parts(&kv);
        let v = b * ease_out((c.lt - 0.45) / 0.8);
        text(t, tw * 0.1, th * 0.18, &format!("{v:.dp$}{suf}"), Kind::Uib, th * 0.36, op(WHITE), Anchor::LA);
        text(t, tw * 0.1, th * 0.7, &kl.to_uppercase(), Kind::Mono, th * 0.1, (255, 255, 255, 170), Anchor::LA);
    });
    tile(c, 2, w * 0.85, ty + tw * 0.65, tw, tw * 1.3, &|t, c, tw, th| {
        text(t, tw * 0.1, th * 0.06, &bl.to_uppercase(), Kind::Mono, tw * 0.055, (255, 255, 255, 170), Anchor::LA);
        let n = vals.len().max(1);
        let bw = tw * 0.8 / n as f32;
        let (top, base) = (th * 0.2, th * 0.88);
        let mx = vals.iter().copied().fold(1e-6, f32::max);
        for (j, v) in vals.iter().enumerate() {
            let kk = ease_out((c.lt - 0.4 - 0.08 * j as f32) / 0.5);
            let hgt = (base - top) * v / mx * kk;
            let x = tw * 0.1 + j as f32 * bw;
            canvas::rounded(t, x + bw * 0.14, base - hgt, x + bw * 0.86, base, bw * 0.18, Some(if j == 0 { op(green) } else { (255, 255, 255, 150) }), None);
            if kk > 0.6 {
                text(t, x + bw / 2.0, base - hgt - tw * 0.02, &format!("{v}"), Kind::Uib, tw * 0.06, (255, 255, 255, 230), Anchor(b'm', b'd'));
            }
        }
    });
}

pub(crate) const CAROUSEL_SPIN: Effect = Effect {
    name: "carousel-spin",
    title: "Carousel Spin",
    desc: "Your reels spin around you in a 3D carousel, then slow to a stop",
    options: "images: your own pictures; count; size (0.3); turns (1.1)",
    cards: true,
    draw: carousel_spin,
};
fn carousel_spin(c: &mut Ctx, e: &mut Run) {
    let kk = e.f("count", e.cards.len().clamp(6, 12) as f32).max(1.0) as usize;
    let cw = (c.w as f32 * e.f("size", 0.3)).floor();
    let cs = e.cache_with("cards", |e| {
        let own = e.has("images");
        e
            .cards
            .iter()
            .enumerate()
            .map(|(i, im)| {
                let f = card(&if own { im.clone() } else { vary(im, i) }, cw);
                let mut dk = f.clone();
                for px in dk.data_mut().chunks_exact_mut(4) {
                    for v in &mut px[..3] {
                        *v = (f32::from(*v) * 0.55) as u8;
                    }
                }
                (f, dk)
            })
            .collect::<Vec<(Pixmap, Pixmap)>>()
    });
    if cs.is_empty() {
        return;
    }
    let k = e.ramp(c, 0.3, 0.25);
    let (w, h) = (c.w as f32, c.h as f32);
    for y in 0..c.h {
        for x in 0..c.w {
            let i = y * c.w + x;
            let d = ((x as f32 - c.fx) / (w * 0.55)).powi(2) + ((y as f32 - (c.fy + h * 0.15)) / (h * 0.5)).powi(2);
            let dim = 1.0 - k * 0.5 * d.clamp(0.0, 1.0) - k * 0.12;
            let m = c.m[i];
            let f = dim * (1.0 - m) + m;
            for ch in 0..3 {
                c.img[i * 3 + ch] *= f;
            }
        }
    }
    let p = clamp(c.lt / e.d);
    let spin = e.f("turns", 1.1) * 2.0 * PI * (1.0 - (1.0 - p).powf(2.4));
    let (intro, out) = (ease_out(c.lt / 0.45), e.fade(c, 0.25));
    let (rad, ry) = (w * 0.56 * intro, h * 0.07);
    let cy = c.fy + c.fw * 1.8;
    let mut items: Vec<(f32, f32, usize)> = (0..kk)
        .map(|i| {
            let th = i as f32 * 2.0 * PI / kk as f32 + spin;
            (th.cos(), th, i)
        })
        .collect();
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = cs.len();
    let fx = c.fx;
    for (depth, th, i) in items {
        let (f, dk) = &cs[(i * if n % 3 != 0 { 3 } else { 2 }) % n];
        let src = if depth >= 0.0 { f } else { dk };
        let sx = th.cos().abs().powf(0.6).max(0.06);
        let s = (0.72 + 0.34 * (depth + 1.0) / 2.0) * (0.5 + 0.5 * intro);
        let squeezed = canvas::resized(src, (src.width() as f32 * s * sx).max(2.0), (src.height() as f32 * s).max(2.0));
        // the near panels cross in front of you whole, the far ones pass behind
        c.place(&squeezed, fx + rad * th.sin(), cy + ry * depth, 1.0, 0.0, out * clamp(c.lt / 0.12) * (0.85 + 0.15 * depth), depth < 0.0, false);
    }
}

const MATCH_TXT: &str = "one clip went in and every cut came back before lunch while editors once spent whole weeks rebuilding the same timeline by hand early users called it unsettling then impossible to ignore the dead air was gone the captions matched and the colour finally looked like the room it was shot in nobody opened an editing app all week";

/// One article page, the key word in a red box locked at `(wx, wy)`.
fn match_page(w: usize, h: usize, word: &str, red: Rgb, j: usize, wx: f32, wy: f32) -> Vec<f32> {
    let (wf, hf) = (w as f32, h as f32);
    let mut rnd = Rng::new(31 + j as u64);
    let paper: Rgb = [(246, 243, 236), (236, 229, 212), (251, 251, 250), (241, 236, 228), (232, 232, 230), (248, 240, 228)][j % 6];
    let kind = [Kind::Serif, Kind::Mont, Kind::Ui, Kind::Serif, Kind::Anton, Kind::Mont][j % 6];
    let size = wf * [0.085, 0.07, 0.072, 0.095, 0.08, 0.068][j % 6];
    let lh = size * 1.22;
    let words: Vec<&str> = MATCH_TXT.split(' ').collect();
    let ink = (40, 38, 36, 255);
    let mut im = rrect(wf, hf, 0.0, op(paper), None);
    let (asc, _) = canvas::face(kind).metrics(size);
    let ww = canvas::length(kind, size, word);
    let bx0 = wx - ww / 2.0;
    let run = |im: &mut Pixmap, rnd: &mut Rng, mut x: f32, y: f32, dir: f32| {
        while if dir > 0.0 { x < wf + size } else { x > -size * 6.0 } {
            let wd = format!("{} ", words[(rnd.next() * words.len() as f32) as usize % words.len()]);
            let l = canvas::length(kind, size, &wd);
            if dir < 0.0 {
                x -= l;
            }
            text(im, x, y, &wd, kind, size, ink, Anchor::LA);
            if dir > 0.0 {
                x += l;
            }
        }
    };
    let off = rnd.uniform(0.0, size * 3.0);
    let ky = wy - asc;
    let mut y = ky - lh * ((ky / lh) + 1.0).ceil();
    while y < hf + lh {
        if (y - ky).abs() >= 1.0 {
            let start = -off - rnd.uniform(0.0, size * 4.0);
            run(&mut im, &mut rnd, start, y, 1.0);
        }
        y += lh;
    }
    canvas::rect(&mut im, 0.0, ky - lh * 0.1, wf, ky + lh * 0.95, op(paper));
    run(&mut im, &mut rnd, bx0 - size * 0.45, ky, -1.0);
    run(&mut im, &mut rnd, bx0 + ww + size * 0.45, ky, 1.0);
    let mut a: Vec<f32> = im.data().chunks_exact(4).flat_map(|p| [f32::from(p[0]) / 255.0, f32::from(p[1]) / 255.0, f32::from(p[2]) / 255.0]).collect();
    let mut bl = a.clone();
    blur_f32(&mut bl, w, h, 3, wf * 0.006);
    for yy in 0..h {
        for xx in 0..w {
            let dd = (((xx as f32 - wx) / (wf * 0.62)).powi(2) + ((yy as f32 - wy) / (hf * 0.32)).powi(2) - 0.15).clamp(0.0, 1.0);
            let o = (yy * w + xx) * 3;
            for ch in 0..3 {
                a[o + ch] = (a[o + ch] * (1.0 - dd) + bl[o + ch] * dd) * (1.0 - 0.18 * (dd * 1.4).clamp(0.0, 1.0));
            }
        }
    }
    // the word in its red box, sharp
    let (padx, pady) = (size * 0.22, size * 0.12);
    let (ascent, descent) = canvas::face(kind).metrics(size);
    let mut bx = rrect(ww + 2.0 * padx, ascent + descent * 0.4 + 2.0 * pady, size * 0.08, op(red), None);
    text(&mut bx, padx, pady, word, kind, size, op(WHITE), Anchor::LA);
    let (x0, y0) = ((bx0 - padx) as i64, (ky - pady) as i64);
    let (bw, bh) = (bx.width() as i64, bx.height() as i64);
    let data = bx.data();
    for yy in 0..bh {
        for xx in 0..bw {
            let (px, py) = (x0 + xx, y0 + yy);
            if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                continue;
            }
            let s = ((yy * bw + xx) * 4) as usize;
            let al = f32::from(data[s + 3]) / 255.0;
            let o = (py as usize * w + px as usize) * 3;
            for ch in 0..3 {
                a[o + ch] = a[o + ch] * (1.0 - al) + f32::from(data[s + ch]) / 255.0;
            }
        }
    }
    a
}

pub(crate) const MATCH_CUT: Effect = Effect {
    name: "match-cut",
    title: "Match Cut",
    desc: "News pages cut on one word, locked in a red box behind your head",
    options: "text: the word (THIS); color; pages: how many cuts (8); images: your own pictures as the pages",
    cards: false,
    draw: match_cut,
};
/// One of your own pictures as a page: filling the frame, the word in its
/// red box at the same place on every one.
fn photo_page(pic: &Pixmap, w: usize, h: usize, word: &str, red: Rgb, wx: f32, wy: f32) -> Vec<f32> {
    let (wf, hf) = (w as f32, h as f32);
    let (pw, ph) = (pic.width() as f32, pic.height() as f32);
    let s = (wf / pw).max(hf / ph);
    let mut im = blank(wf, hf);
    canvas::over_scaled(&mut im, pic, (wf - pw * s) / 2.0, (hf - ph * s) / 2.0, pw * s, ph * s);
    let size = wf * 0.09;
    let t = text_img(word, size, Kind::Anton, WHITE, Look::default());
    let (tw, th) = (t.width() as f32, t.height() as f32);
    let mut bx = rrect(tw + size * 0.44, th + size * 0.2, size * 0.08, op(red), None);
    over(&mut bx, &t, (size * 0.22) as i32, (size * 0.1) as i32);
    over(&mut im, &bx, (wx - bx.width() as f32 / 2.0) as i32, (wy - bx.height() as f32 / 2.0) as i32);
    im.data().chunks_exact(4).flat_map(|p| [f32::from(p[0]) / 255.0, f32::from(p[1]) / 255.0, f32::from(p[2]) / 255.0]).collect()
}

fn match_cut(c: &mut Ctx, e: &mut Run) {
    let k = clamp(c.lt / 0.06) * e.fade(c, 0.12);
    c.face_cam(1.0 + 0.06 * ease_out(c.lt / e.d), 0.44);
    let (wf, hf) = (c.w as f32, c.h as f32);
    let (fy, fw) = (c.fy, c.fw);
    let anchor = e.cache("anchor", || (wf / 2.0, (hf * 0.1).max(fy - fw * 1.55)));
    // the cuts quicken at first and slow to the last page
    let n = (e.f("pages", 8.0).round() as usize).clamp(3, 16);
    let durs: Vec<f32> = (0..n).map(|j| if j + 1 == n { 0.45 } else { 0.15 + 0.15 * (j as f32 / (n - 1) as f32).powi(2) }).collect();
    let (mut t, mut j) = (c.lt, 0);
    while j < durs.len() - 1 && t > durs[j] {
        t -= durs[j];
        j += 1;
    }
    let word = e.s("text", "THIS");
    let red = e.color("color", (214, 40, 40));
    let (w, h) = (c.w, c.h);
    let own = e.has("images") && !e.cards.is_empty();
    let pg = e.cache_with(&format!("pg{j}"), |e| {
        if own { photo_page(&e.cards[j % e.cards.len()], w, h, &word, red, anchor.0, anchor.1) } else { match_page(w, h, &word, red, j, anchor.0, anchor.1) }
    });
    for i in 0..w * h {
        let m = c.m[i];
        for ch in 0..3 {
            let v = c.img[i * 3 + ch];
            c.img[i * 3 + ch] = v * (1.0 - k) + (pg[i * 3 + ch] * (1.0 - m) + v * m) * k;
        }
    }
}

pub(crate) const APPLE_TOGGLES: Effect = Effect {
    name: "apple-toggles",
    title: "Toggles",
    desc: "A settings card flips its switches on, one line at a time",
    options: "text: rows joined with |; ticks: seconds into the effect, joined with |; color; pos: below for under the chin",
    cards: false,
    draw: toggles,
};
fn toggles(c: &mut Ctx, e: &mut Run) {
    let rows = parts(&e.s("text", "One clip|Pick an effect|Zero editing apps"));
    let below = e.s("pos", "above") == "below";
    if !below {
        c.face_cam(1.0 + (e.f("zoom", 1.12) - 1.0) * ease_out(c.lt / 0.5), 0.56);
    }
    let w = c.w as f32;
    let cw = (w * 0.72).floor();
    let rh = (w * 0.125).floor();
    let ch = rh * rows.len() as f32;
    let r = (w * 0.055).floor();
    let (tw, th) = ((w * 0.135).floor(), (w * 0.08).floor());
    let tx = cw - (w * 0.05).floor() - tw;
    let base = e.cache("base", || {
        let mut im = rrect(cw, ch, r, (255, 255, 255, 250), None);
        for (i, s) in rows.iter().enumerate() {
            text(&mut im, w * 0.05, i as f32 * rh + rh / 2.0, s, Kind::Ui, w * 0.05, (18, 18, 20, 255), Anchor::LM);
            if i > 0 {
                canvas::line_flat(&mut im, &[(w * 0.05, i as f32 * rh), (cw, i as f32 * rh)], (222, 222, 226, 255), (w / 900.0).max(2.0));
            }
        }
        shadowed(&im, (w / 60.0).floor().max(10.0), (w / 90.0).floor(), 110)
    });
    let pad = (w / 60.0).floor().max(10.0) * 2.0 + (w / 90.0).floor();
    let ts: Vec<f32> = if e.has("ticks") { e.s("ticks", "").split('|').map(|x| x.trim().parse().unwrap_or(0.0)).collect() } else { (0..rows.len()).map(|i| 0.45 + 0.34 * i as f32).collect() };
    let mut im = (*base).clone();
    let on = e.color("color", (52, 199, 89));
    let off = (233.0, 233.0, 235.0);
    for i in 0..rows.len() {
        let u = if i < ts.len() { ease_out((c.lt - ts[i]) / 0.2) } else { 0.0 };
        let col = ((off.0 * (1.0 - u) + f32::from(on.0) * u) as u8, (off.1 * (1.0 - u) + f32::from(on.1) * u) as u8, (off.2 * (1.0 - u) + f32::from(on.2) * u) as u8, 255);
        let (x0, y0) = (pad + tx, pad + i as f32 * rh + (rh - th) / 2.0);
        canvas::rounded(&mut im, x0, y0, x0 + tw, y0 + th, (th / 2.0).floor(), Some(col), None);
        let kr = th / 2.0 - w * 0.006;
        let (kx, ky) = (x0 + th / 2.0 + (tw - th) * u, y0 + th / 2.0);
        canvas::ellipse(&mut im, kx - kr, ky - kr + w * 0.002, kx + kr, ky + kr + w * 0.004, Some((0, 0, 0, 40)), None);
        canvas::ellipse(&mut im, kx - kr, ky - kr, kx + kr, ky + kr, Some(op(WHITE)), None);
    }
    let q = pop(c, 0.0, 0.38);
    let al = e.fade(c, 0.2) * clamp(c.lt / 0.1);
    if below {
        let y = (c.h as f32 - ch * 0.75).min(c.fy + c.fw * 2.5);
        c.place(&im, w / 2.0, y, 0.6 + 0.4 * q, 0.0, al, false, true);
        return;
    }
    let cy = (c.h as f32 * 0.08 + ch / 2.0).max(c.fy - c.fw * 0.95 + c.fw * e.f("overlap", 0.08) - ch / 2.0);
    c.place(&im, w / 2.0, cy + (1.0 - q) * w * 0.04, 0.6 + 0.4 * q, 0.0, al, true, false);
}

/// The LED sign's dot matrix: which dots are letters, the dot size, and
/// each dot's flicker phase.
struct Matrix {
    lit: Vec<bool>,
    rows: usize,
    cols: usize,
    cell: usize,
    phase: Vec<f32>,
}

pub(crate) const LED_SIGN: Effect = Effect {
    name: "led-sign",
    title: "LED Sign",
    desc: "The lights drop and a glowing LED sign types itself out behind you",
    options: "text: lines joined with |; color; push (1.08)",
    cards: false,
    draw: led_sign,
};
fn led_sign(c: &mut Ctx, e: &mut Run) {
    let k = e.ramp(c, 0.2, 0.2);
    c.face_cam(1.0 + (e.f("push", 1.08) - 1.0) * ease_inout(c.lt / e.d), 0.42);
    let lines = parts(&e.s("text", "ON AIR").to_uppercase());
    let col = e.color("color", (255, 146, 48));
    let col = [f32::from(col.0) / 255.0, f32::from(col.1) / 255.0, f32::from(col.2) / 255.0];
    let (wf, hf) = (c.w as f32, c.h as f32);
    let mx = e.cache("m", || {
        // each line rasterised small, eleven dots tall
        let mats: Vec<(Vec<bool>, usize)> = lines
            .iter()
            .map(|s| {
                let size = 40.0;
                let tw = canvas::length(Kind::Uib, size, s) + 8.0;
                let mut im = blank(tw, 56.0);
                text(&mut im, 4.0, 2.0, s, Kind::Uib, size, (255, 255, 255, 255), Anchor::LA);
                let (iw, ih) = (im.width() as usize, im.height() as usize);
                let a: Vec<f32> = im.data().chunks_exact(4).map(|p| f32::from(p[3]) / 255.0).collect();
                // the ink's bounding box
                let (mut x0, mut y0, mut x1, mut y1) = (iw, ih, 0, 0);
                for y in 0..ih {
                    for x in 0..iw {
                        if a[y * iw + x] > 0.05 {
                            x0 = x0.min(x);
                            y0 = y0.min(y);
                            x1 = x1.max(x + 1);
                            y1 = y1.max(y + 1);
                        }
                    }
                }
                if x1 <= x0 || y1 <= y0 {
                    return (vec![false; 11], 1);
                }
                let (bw, bh) = (x1 - x0, y1 - y0);
                let rows = 11;
                let cols = ((bw as f32 / bh as f32 * rows as f32).round() as usize).max(1);
                let mut lit = vec![false; rows * cols];
                for r in 0..rows {
                    for cc in 0..cols {
                        let (ya, yb) = (y0 + r * bh / rows, (y0 + (r + 1) * bh / rows).max(y0 + r * bh / rows + 1));
                        let (xa, xb) = (x0 + cc * bw / cols, (x0 + (cc + 1) * bw / cols).max(x0 + cc * bw / cols + 1));
                        let mut s = 0.0;
                        for y in ya..yb {
                            for x in xa..xb {
                                s += a[y * iw + x];
                            }
                        }
                        lit[r * cols + cc] = s / ((yb - ya) * (xb - xa)) as f32 > 0.42;
                    }
                }
                (lit, cols)
            })
            .collect();
        let cols = mats.iter().map(|m| m.1).max().unwrap_or(1) + 2;
        let rows = mats.len() * 12 - 1;
        let cell = ((wf * 0.96 / cols as f32).min(hf * 0.3 / (mats.len() * 12) as f32)) as usize;
        let mut lit = vec![false; rows * cols];
        for (i, (m, mc)) in mats.iter().enumerate() {
            let o = (cols - mc) / 2;
            for r in 0..11 {
                for cc in 0..*mc {
                    lit[(i * 12 + r) * cols + o + cc] = m[r * mc + cc];
                }
            }
        }
        let mut r = Rng::new(3);
        let phase = (0..rows * cols).map(|_| r.next()).collect();
        Matrix { lit, rows, cols, cell: cell.max(2), phase }
    });
    let (rows, cols, cell) = (mx.rows, mx.cols, mx.cell);
    let sweep = clamp((c.lt - 0.15) / 0.75) * (cols + 4) as f32;
    let (pw, ph) = (cols * cell, rows * cell);
    let mut pan = vec![0f32; pw * ph * 3];
    for r in 0..rows {
        for cc in 0..cols {
            let i = r * cols + cc;
            let lit = if mx.lit[i] { ((sweep - cc as f32) / 3.0).clamp(0.0, 1.0) } else { 0.0 };
            let flick = 0.82 + 0.18 * (mx.phase[i] * 50.0 + c.lt * 30.0).sin();
            let b = (lit * flick).max(0.09);
            for y in 0..cell {
                for x in 0..cell {
                    let rr = ((x as f32 - cell as f32 / 2.0 + 0.5).powi(2) + (y as f32 - cell as f32 / 2.0 + 0.5).powi(2)).sqrt();
                    let dot = (cell as f32 * 0.36 - rr + 0.5).clamp(0.0, 1.0) * b;
                    let o = ((r * cell + y) * pw + cc * cell + x) * 3;
                    for ch in 0..3 {
                        pan[o + ch] = dot * col[ch];
                    }
                }
            }
        }
    }
    let mut glow = pan.clone();
    blur_f32(&mut glow, pw, ph, 3, cell as f32 * 0.9);
    for (p, g) in pan.iter_mut().zip(&glow) {
        *p = (*p + g * 1.6).clamp(0.0, 1.0);
    }
    let x0 = (wf / 2.0 - pw as f32 / 2.0) as i64;
    let y0 = ((hf * 0.03).max(c.fy - c.fw * 0.75 - ph as f32)) as i64;
    for y in 0..c.h {
        for x in 0..c.w {
            let i = y * c.w + x;
            let d = ((x as f32 - c.fx) / (wf * 0.6)).powi(2) + ((y as f32 - (c.fy + hf * 0.25)) / (hf * 0.55)).powi(2);
            let bright = 1.0 - k * (0.84 - 0.3 * (1.0 - d).clamp(0.0, 1.0));
            let m = c.m[i];
            let f = bright * (1.0 - m) + m * (1.0 - 0.18 * k);
            for ch in 0..3 {
                c.img[i * 3 + ch] *= f;
            }
            let (px, py) = (x as i64 - x0, y as i64 - y0);
            if px >= 0 && py >= 0 && (px as usize) < pw && (py as usize) < ph {
                let o = (py as usize * pw + px as usize) * 3;
                for ch in 0..3 {
                    c.img[i * 3 + ch] = (c.img[i * 3 + ch] + pan[o + ch] * k * (1.0 - m)).clamp(0.0, 1.0);
                }
            }
        }
    }
}

pub(crate) const SLOW_PUSH: Effect = Effect {
    name: "slow-push",
    title: "Slow Push",
    desc: "A slow cinematic push-in to your face while the room melts out of focus",
    options: "zoom (1.6); blur (0.006)",
    cards: false,
    draw: slow_push,
};
fn slow_push(c: &mut Ctx, e: &mut Run) {
    let p = ease_inout(c.lt / e.d);
    c.face_cam(1.0 + (e.f("zoom", 1.6) - 1.0) * p, 0.4);
    let s = c.w as f32 * e.f("blur", 0.006) * p;
    if s > 0.6 {
        let mut bl = c.img.clone();
        blur_f32(&mut bl, c.w, c.h, 3, s);
        for i in 0..c.w * c.h {
            let m = c.m[i];
            for ch in 0..3 {
                c.img[i * 3 + ch] = c.img[i * 3 + ch] * m + bl[i * 3 + ch] * (1.0 - m);
            }
        }
    }
    c.vignette(0.25 * p);
}

/// A liquid glass panel drawn straight onto the frame (`fx.py`'s
/// `liquid_glass`).
fn liquid_glass_on(c: &mut Ctx, x0: f32, y0: f32, x1: f32, y1: f32) {
    let u = c.w as f32 / 2160.0;
    let (w, h) = ((x1 - x0).round(), (y1 - y0).round());
    if w < 8.0 || h < 8.0 {
        return;
    }
    let panel = c.glass(w, h, 72.0 * u, x0 + w / 2.0, y0 + h / 2.0, 0.0, WHITE, Some(7.0 * u / 0.3), 255.0, 0.0, true);
    c.comp(&panel, x0, y0, 1.0, false, false);
}

/// A see-through glass stat card with a counting number and a line or bar
/// chart (`fx.py`'s `glass_graph`).
#[allow(clippy::too_many_arguments)]
fn glass_graph(c: &mut Ctx, cx: f32, cy: f32, k: f32, prog: f32, label: &str, value: &str, line: bool) {
    if k <= 0.0 {
        return;
    }
    let u = c.w as f32 / 2160.0;
    let sc = 0.82 + 0.18 * ease_back(k);
    let (w, h) = (1760.0 * u * sc, 1056.0 * u * sc);
    let lift = 40.0 * u * (1.0 - k);
    let (x0, y0) = (cx - w / 2.0, cy - h / 2.0 + lift);
    liquid_glass_on(c, x0, y0, x0 + w, y0 + h);
    let v = w / 1500.0;
    let o = 20.0;
    let mut lay = blank(w + 40.0, h + 40.0);
    let (num, suf, dec) = num_parts(value);
    let txt = format!("{:.dec$}{suf}", num * ease_out(prog));
    let label = label.to_uppercase();
    let big = 170.0 * v;
    let lab = 44.0 * v;
    let write = |pm: &mut Pixmap, color: (u8, u8, u8, u8)| {
        let mut xx = o + 64.0 * v;
        for ch in label.chars() {
            let t = ch.to_string();
            text(pm, xx, o + 64.0 * v, &t, Kind::Uim, lab, color, Anchor::LA);
            xx += canvas::length(Kind::Uim, lab, &t) + 9.0 * v;
        }
        text(pm, o + w - 70.0 * v - canvas::length(Kind::Uib, big, &txt), o + 40.0 * v, &txt, Kind::Uib, big, color, Anchor::LA);
    };
    let mut halo = blank(w + 40.0, h + 40.0);
    write(&mut halo, (0, 0, 0, 150));
    canvas::blur(&mut halo, 10.0 * v);
    write(&mut lay, (255, 255, 255, 235));
    let (gx0, gx1, gy0, gy1) = (o + 64.0 * v, o + w - 64.0 * v, o + 290.0 * v, o + h - 80.0 * v);
    for j in 0..3 {
        let y = gy0 + (gy1 - gy0) * j as f32 / 2.0;
        canvas::line_flat(&mut lay, &[(gx0, y), (gx1, y)], (255, 255, 255, 70), (3.0 * v).max(1.0));
    }
    let mut chart = blank(w + 40.0, h + 40.0);
    if line {
        let ys = [0.04, 0.07, 0.10, 0.16, 0.22, 0.35, 0.52, 0.74, 1.0];
        let n = ((ys.len() - 1) as f32 * ease_out(prog)).max(0.02);
        let mut pts = Vec::new();
        for i in 0..=(n as usize) {
            let t = (i as f32).min(n);
            let i0 = (t as usize).min(ys.len() - 2);
            let f = t - i0 as f32;
            let yv = ys[i0] + (ys[i0 + 1] - ys[i0]) * f;
            pts.push((gx0 + (gx1 - gx0) * t / (ys.len() - 1) as f32, gy1 - (gy1 - gy0) * yv));
        }
        if pts.len() > 1 {
            let mut area = pts.clone();
            area.push((pts[pts.len() - 1].0, gy1));
            area.push((pts[0].0, gy1));
            canvas::polygon(&mut chart, &area, Some(with_a(GLOW_GREEN, 70)), None);
            let mut gl = blank(w + 40.0, h + 40.0);
            canvas::line(&mut gl, &pts, op(GLOW_GREEN), 60.0 * v);
            canvas::blur(&mut gl, 26.0 * v);
            over(&mut chart, &gl, 0, 0);
            canvas::line(&mut chart, &pts, op(GLOW_GREEN), (20.0 * v).max(2.0));
            let (ex, ey) = pts[pts.len() - 1];
            canvas::ellipse(&mut chart, ex - 30.0 * v, ey - 30.0 * v, ex + 30.0 * v, ey + 30.0 * v, Some(with_a(GLOW_GREEN, 90)), None);
            canvas::ellipse(&mut chart, ex - 15.0 * v, ey - 15.0 * v, ex + 15.0 * v, ey + 15.0 * v, Some((235, 255, 240, 255)), None);
        }
    } else {
        let vals = [1.0, 0.71, 0.63, 0.42, 0.25, 0.20, 0.19];
        let bw = (gx1 - gx0) / vals.len() as f32;
        for (i, val) in vals.iter().enumerate() {
            let g = ease_out(clamp(prog * 1.6 - i as f32 * 0.1));
            let top = gy1 - (gy1 - gy0) * val * g;
            let bx = gx0 + i as f32 * bw + bw * 0.16;
            if i == 0 {
                let mut gl = blank(w + 40.0, h + 40.0);
                canvas::rounded(&mut gl, bx, top, bx + bw * 0.68, gy1, 18.0 * v, Some(with_a(GLOW_GREEN, 230)), None);
                canvas::blur(&mut gl, 22.0 * v);
                over(&mut chart, &gl, 0, 0);
            }
            canvas::rounded(&mut chart, bx, top, bx + bw * 0.68, gy1, 18.0 * v, Some(with_a(GLOW_GREEN, if i == 0 { 230 } else { 120 })), None);
        }
    }
    over(&mut halo, &chart, 0, 0);
    over(&mut halo, &lay, 0, 0);
    c.comp(&halo, x0 - o, y0 - o, 1.0, false, false);
}

pub(crate) const OVER_THE_SHOULDER_ZOOM: Effect = Effect {
    name: "over-the-shoulder-zoom",
    title: "Over-the-Shoulder Zoom",
    desc: "Pushes into one corner, a liquid glass graph pops beside your head, then the other corner with a second graph",
    options: "text: two labels joined with |; to: two numbers joined with | (197K|672K); zoom (2.0)",
    cards: false,
    draw: over_the_shoulder_zoom,
};
fn over_the_shoulder_zoom(c: &mut Ctx, e: &mut Run) {
    let p = clamp(c.lt / e.d);
    let z0 = e.f("zoom", 2.0);
    let w = c.w as f32;
    let (lx, rx) = (c.fx - w * 0.17, c.fx + w * 0.19);
    let keys = [(0.0, 1.0, c.fx, c.fy), (0.30, 1.04, c.fx, c.fy), (0.46, z0, lx, c.fy), (0.66, z0 + 0.04, lx, c.fy), (0.76, 1.05, c.fx, c.fy), (0.87, z0, rx, c.fy), (1.0, z0 + 0.04, rx, c.fy)];
    let (mut z, mut x, mut y) = (1.0, c.fx, c.fy);
    for win in keys.windows(2) {
        let ((pa, za, xa, ya), (pb, zb, xb, yb)) = (win[0], win[1]);
        if p >= pa && p <= pb {
            let q = ease_inout((p - pa) / (pb - pa));
            z = za + (zb - za) * q;
            x = xa + (xb - xa) * q;
            y = ya + (yb - ya) * q;
            break;
        }
    }
    c.camera(z, x, y);
    let k1 = clamp((p - 0.42) / 0.065) * (1.0 - clamp((p - 0.66) / 0.055));
    let k2 = clamp((p - 0.84) / 0.065);
    if k1 <= 0.0 && k2 <= 0.0 {
        return;
    }
    let me = c.img.clone();
    let labels = parts(&e.s("text", "Views · this reel|Top reels · total"));
    let values = parts(&e.s("to", "197K|672K"));
    let lab = |i: usize| labels.get(i).cloned().unwrap_or_default();
    let val = |i: usize| values.get(i).cloned().unwrap_or_else(|| "0".to_owned());
    let h = c.h as f32;
    glass_graph(c, w * 0.43, h * 0.255, k1, clamp((p - 0.45) / 0.17), &lab(0), &val(0), true);
    glass_graph(c, w * 0.57, h * 0.255, k2, clamp((p - 0.86) / 0.14), &lab(1), &val(1), false);
    // your head stays in front of the cards
    for i in 0..c.w * c.h {
        let m = c.m[i];
        for ch in 0..3 {
            c.img[i * 3 + ch] = c.img[i * 3 + ch] * (1.0 - m) + me[i * 3 + ch] * m;
        }
    }
}

/// One creamy glass notification with a soft halo, centred at height `y`.
fn msg_bubble(c: &mut Ctx, y: f32, title: &str, body: &str, a: f32) {
    let wf = c.w as f32;
    let u = wf / 2160.0;
    let (w, h) = ((1880.0 * u).floor(), (330.0 * u).floor());
    if a <= 0.0 || y - h / 2.0 + h < 0.0 || y - h / 2.0 > c.h as f32 {
        return;
    }
    let s = 0.82 + 0.18 * ease_back(a);
    let (ww, hh) = ((w * s).floor(), (h * s).floor());
    let (xs, ys) = (((wf - ww) / 2.0).floor(), (y - hh / 2.0).floor());
    let p = (120.0 * u).floor();
    let mut halo = blank(ww + 240.0 * u, hh + 240.0 * u);
    canvas::rounded(&mut halo, p - 30.0 * u, p - 30.0 * u, p + ww + 30.0 * u, p + hh + 30.0 * u, 100.0 * u, Some((255, 250, 240, (215.0 * clamp(a * 2.0)) as u8)), None);
    canvas::blur(&mut halo, 56.0 * u);
    c.comp(&halo, xs - p, ys - p, 1.0, false, false);
    // the frosted body: the frame blurred and creamed
    let pane = c.glass(ww, hh, 70.0 * u * s, xs + ww / 2.0, ys + hh / 2.0, 0.78, (252, 246, 238), Some(20.0 * u), 255.0 * clamp(a * 2.5), 0.0, false);
    c.comp(&pane, xs, ys, 1.0, false, false);
    let v = s * u;
    let al = (255.0 * clamp(a * 2.5)) as u8;
    let mut lay = blank(ww, hh);
    canvas::rounded(&mut lay, 1.0, 1.0, ww - 2.0, hh - 2.0, 70.0 * v, None, Some(((255, 255, 255, (170.0 * clamp(a * 2.5)) as u8), (3.0 * v).max(1.0))));
    // a plain round chat icon
    let is = (160.0 * v).floor();
    let mut icon = rrect(is, is, 40.0 * v, with_a(BLUE, al), None);
    canvas::ellipse(&mut icon, 30.0 * v, 34.0 * v, 130.0 * v, 114.0 * v, Some((255, 255, 255, al)), None);
    canvas::polygon(&mut icon, &[(44.0 * v, 102.0 * v), (36.0 * v, 130.0 * v), (72.0 * v, 110.0 * v)], Some((255, 255, 255, al)), None);
    over(&mut lay, &icon, (56.0 * v) as i32, (70.0 * v) as i32);
    text(&mut lay, 262.0 * v, 76.0 * v, title, Kind::Uib, 72.0 * v, (28, 28, 30, al), Anchor::LA);
    text(&mut lay, 262.0 * v, 170.0 * v, body, Kind::Uim, 76.0 * v, (44, 44, 46, al), Anchor::LA);
    c.comp(&lay, xs, ys, 1.0, false, false);
}

pub(crate) const MESSAGES: Effect = Effect {
    name: "messages",
    title: "Messages",
    desc: "Message notifications pop in one after another with a soft glow",
    options: "text: messages joined with |; title (New message); y (0.61)",
    cards: false,
    draw: messages,
};
fn messages(c: &mut Ctx, e: &mut Run) {
    let msgs = parts(&e.s("text", "how did you edit this??|wait, which app is this|send me the effects pls|EFFECTS"));
    let title = e.s("title", "New message");
    let n = msgs.len();
    let u = c.w as f32 / 2160.0;
    let gap = (e.d * 0.75 / n.max(1) as f32).min(0.4);
    let pops: Vec<f32> = (0..n).map(|i| 0.13 + i as f32 * gap).collect();
    let bottom = c.h as f32 * e.f("y", 0.61);
    let out = e.fade(c, 0.2);
    for kk in 0..n {
        if c.lt < pops[kk] {
            continue;
        }
        let newer: f32 = (kk + 1..n).filter(|&j| c.lt >= pops[j]).map(|j| ease_out(clamp((c.lt - pops[j]) / 0.17))).sum();
        msg_bubble(c, bottom - 165.0 * u - newer * 365.0 * u, &title, &msgs[kk], clamp((c.lt - pops[kk]) / 0.17) * out);
    }
}

/// A grid tile: the picture cropped to fill, a play count along its foot.
fn grid_tile(im: &Pixmap, w: f32, h: f32, views: Option<&str>, r: f32) -> Pixmap {
    let mut t = cover(im, w, h, 0.5);
    if let Some(views) = views {
        let u = w / 660.0;
        let fh = (h * 0.28).floor();
        let mut shade = blank(w, fh);
        let sw = shade.width() as usize;
        for (i, px) in shade.data_mut().chunks_exact_mut(4).enumerate() {
            px[3] = ((i / sw) as f32 / fh * 255.0 * 0.75) as u8;
        }
        over(&mut t, &shade, 0, (h - fh) as i32);
        let s = 56.0 * u;
        let (x, y) = (34.0 * u, h - 92.0 * u);
        // a plain play triangle where an app would put its own mark
        canvas::polygon(&mut t, &[(x, y + 6.0 * u), (x, y + s - 2.0 * u), (x + s * 0.8, y + s / 2.0 + 2.0 * u)], Some(op(WHITE)), None);
        text(&mut t, x + s + 14.0 * u, y - 6.0 * u, views, Kind::Uib, 58.0 * u, op(WHITE), Anchor::LA);
    }
    round_corners(&mut t, r);
    canvas::rounded(&mut t, 1.0, 1.0, w - 2.0, h - 2.0, r, None, Some(((255, 255, 255, 150), (4.0 * w / 660.0).max(1.0))));
    t
}

pub(crate) const PLACE_IN_GRID: Effect = Effect {
    name: "place-in-grid",
    title: "Place in Grid",
    desc: "You shrink into the centre of a 3x3 grid, your reels pop in around you with view counts",
    options: "images: your own pictures; views: nine counts joined with | (the ninth is yours)",
    cards: true,
    draw: place_in_grid,
};
fn place_in_grid(c: &mut Ctx, e: &mut Run) {
    let (wf, hf) = (c.w as f32, c.h as f32);
    let (g, m) = ((33.0 * wf / 2160.0).floor(), (68.0 * wf / 2160.0).floor());
    let tw = ((wf - 2.0 * m - 2.0 * g) / 3.0).floor();
    let th = (tw * 16.0 / 9.0).floor();
    let top = ((hf - (3.0 * th + 2.0 * g)) / 2.0).floor();
    let r = (28.0 * wf / 2160.0).floor();
    let rect = |i: usize| (m + (i % 3) as f32 * (tw + g), top + (i / 3) as f32 * (th + g));
    let views = parts(&e.s("views", "2.4M|847K|1.2M|312K|3.4M|596K|268K|1.8M|3.1M"));
    let tiles = e.cache_with("tiles", |e| {
        if e.cards.is_empty() {
            return Vec::new();
        }
        let own = e.has("images");
        (0..8)
            .map(|i| {
                let im = &e.cards[i % e.cards.len()];
                let im = if own { im.clone() } else { vary(im, i) };
                grid_tile(&im, tw, th, Some(&views[i % views.len()]), r)
            })
            .collect::<Vec<Pixmap>>()
    });
    let me = c.whole(wf, hf);
    let p = ease_inout(clamp(c.lt / 0.45));
    // the room behind the grid: the frame shrunk, blurred, a little richer, creamed
    let (sw, sh) = ((c.w / 8).max(1), (c.h / 8).max(1));
    let mut small = c.shrunk(sw, sh);
    blur_f32(&mut small, sw, sh, 3, 14.0 / 8.0);
    for y in 0..c.h {
        for x in 0..c.w {
            let sx = ((x as f32 + 0.5) / 8.0 - 0.5).clamp(0.0, (sw - 1) as f32);
            let sy = ((y as f32 + 0.5) / 8.0 - 0.5).clamp(0.0, (sh - 1) as f32);
            let (x0, y0) = (sx as usize, sy as usize);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let mut v = [0f32; 3];
            for (ch, out) in v.iter_mut().enumerate() {
                *out = (small[(y0 * sw + x0) * 3 + ch] * (1.0 - fx) + small[(y0 * sw + x1) * 3 + ch] * fx) * (1.0 - fy)
                    + (small[(y1 * sw + x0) * 3 + ch] * (1.0 - fx) + small[(y1 * sw + x1) * 3 + ch] * fx) * fy;
            }
            let lum = v[0] * 0.299 + v[1] * 0.587 + v[2] * 0.114;
            let tint = [1.0, 248.0 / 255.0, 238.0 / 255.0];
            let o = (y * c.w + x) * 3;
            for ch in 0..3 {
                let rich = (lum + (v[ch] - lum) * 1.25).clamp(0.0, 1.0);
                c.img[o + ch] = rich * (1.0 - 0.37) + tint[ch] * 0.37;
            }
        }
    }
    for (j, slot) in [1usize, 5, 7, 3, 2, 8, 6, 0].into_iter().enumerate() {
        let a = clamp((c.lt - 0.47 - j as f32 / 30.0) / 0.1);
        if a > 0.0
            && let Some(tile) = tiles.get(j)
        {
            let (x, y) = rect(slot);
            c.place(tile, x + tw / 2.0, y + th / 2.0, 0.82 + 0.18 * ease_back(a), 0.0, clamp(a * 2.0), false, false);
        }
    }
    let (cx, cy) = rect(4);
    let (w2, h2) = ((wf + (tw - wf) * p).floor().max(2.0), (hf + (th - hf) * p).floor().max(2.0));
    let tile = grid_tile(&me, w2, h2, if p > 0.9 { views.get(8).map(String::as_str) } else { None }, r * p);
    c.comp(&tile, cx * p, cy * p, 1.0, false, false);
    if p > 0.95 {
        let mut d = blank(tw + 8.0, th + 8.0);
        canvas::rounded(&mut d, 2.0, 2.0, tw + 5.0, th + 5.0, r + 2.0, None, Some(((217, 119, 87, 255), 8.0)));
        c.comp(&d, cx - 4.0, cy - 4.0, 1.0, false, false);
    }
}

/// How high a picture `h` tall sits above the head.
fn above_head(c: &Ctx, h: f32) -> f32 {
    (c.h as f32 * 0.12 + h / 2.0).max(c.fy - c.fw * 1.75)
}

pub(crate) const TAPE_LABEL: Effect = Effect {
    name: "tape-label",
    title: "Tape Label",
    desc: "A strip of yellow tape with your line lands on your chest",
    options: "text; color: the tape; ink: the words; angle (-3)",
    cards: false,
    draw: tape_label,
};
fn tape_label(c: &mut Ctx, e: &mut Run) {
    let k = pop(c, 0.05, 0.25) * e.fade(c, 0.2);
    if k <= 0.0 {
        return;
    }
    let s = (c.w as f32 * 0.055).floor();
    let line = e.s("text", "your own reels");
    let tape = e.color("color", TAPE);
    let ink = e.color("ink", INKY);
    let im = e.cache("tape", || {
        let t = text_img(&line, s, Kind::Hand, ink, Look::default());
        let (w, h) = ((t.width() as f32 + s * 1.2).floor(), (t.height() as f32 + s * 0.5).floor());
        let tooth = s * 0.18;
        let mut im = blank(w, h);
        // torn ends: a zigzag down each side
        let teeth = ((h / tooth) as usize).max(3);
        let mut pts = vec![(tooth, 0.0), (w - tooth, 0.0)];
        pts.extend((0..=teeth).map(|j| (w - tooth + if j % 2 == 0 { tooth } else { 0.0 }, h * j as f32 / teeth as f32)));
        pts.push((tooth, h));
        pts.extend((0..=teeth).rev().map(|j| (if j % 2 == 0 { 0.0 } else { tooth }, h * j as f32 / teeth as f32)));
        canvas::polygon(&mut im, &pts, Some(with_a(tape, 238)), None);
        // a sheen across the top and fine lines through the tape
        canvas::rect(&mut im, tooth, h * 0.08, w - tooth, h * 0.3, (255, 255, 255, 46));
        for j in 1..4 {
            let y = h * j as f32 / 4.0;
            canvas::line_flat(&mut im, &[(tooth, y), (w - tooth, y)], (0, 0, 0, 14), 1.0);
        }
        over(&mut im, &t, ((w - t.width() as f32) / 2.0) as i32, ((h - t.height() as f32) / 2.0) as i32);
        shadowed(&im, 10.0, 5.0, 90)
    });
    let y = (c.fy + c.fw * 1.6).clamp(c.h as f32 * 0.45, c.h as f32 * 0.68);
    c.place(&im, c.w as f32 / 2.0, y, 0.85 + 0.15 * k, e.f("angle", -3.0), clamp(k * 1.5), false, true);
}

pub(crate) const SELECT_RESIZE: Effect = Effect {
    name: "select-resize",
    title: "Select & Resize",
    desc: "A blue selection box grabs your number above your head and a cursor drags it bigger",
    options: "text (2X); images: your picture in the box",
    cards: false,
    draw: select_resize,
};
fn select_resize(c: &mut Ctx, e: &mut Run) {
    let al = e.fade(c, 0.2) * clamp(c.lt / 0.08);
    if al <= 0.0 {
        return;
    }
    let (w, h) = (c.w as f32, c.h as f32);
    let p = ease_inout((c.lt - 0.25) / 0.7);
    let s = ((w * 0.2 * (0.55 + 0.45 * p)) as i32 / 4 * 4) as f32;
    let word = e.s("text", "2X");
    // your picture in the box when you give one, else the word
    let pic = e.has("images") && !e.cards.is_empty();
    let t = e.cache_with(&format!("t{s}"), |e| {
        if pic {
            let im = &e.cards[0];
            let k = s * 1.6 / im.height().max(1) as f32;
            canvas::resized(im, im.width() as f32 * k, im.height() as f32 * k)
        } else {
            text_img(&word, s, Kind::Didone, CREAMY, Look { shadow: 8.0, ..Look::default() })
        }
    });
    let (bw, bh) = (t.width() as f32 + w * 0.03, t.height() as f32);
    let (cx, cy) = (w / 2.0, above_head(c, bh));
    let (x0, y0) = (cx - bw / 2.0, cy - bh / 2.0);
    let mut lay = blank(w, h);
    let b = op(BLUE);
    let lw = (w / 360.0).floor().max(2.0);
    canvas::rect(&mut lay, x0, y0, x0 + bw, y0 + bh, with_a(BLUE, (130.0 * (1.0 - 0.7 * clamp((c.lt - 0.9) / 0.3))) as u8));
    for (ax, ay, bx, by) in [(x0, y0, x0 + bw, y0), (x0, y0 + bh, x0 + bw, y0 + bh), (x0, y0, x0, y0 + bh), (x0 + bw, y0, x0 + bw, y0 + bh)] {
        let n = (((bx - ax).hypot(by - ay) / (w * 0.018)) as usize).max(1);
        for j in (0..n).step_by(2) {
            let (t0, t1) = (j as f32 / n as f32, (j + 1) as f32 / n as f32);
            canvas::line_flat(&mut lay, &[(ax + (bx - ax) * t0, ay + (by - ay) * t0), (ax + (bx - ax) * t1, ay + (by - ay) * t1)], b, lw);
        }
    }
    let hs = w * 0.009;
    for (qx, qy) in [(x0, y0), (x0 + bw, y0), (x0, y0 + bh), (x0 + bw, y0 + bh)] {
        canvas::rect(&mut lay, qx - hs, qy - hs, qx + hs, qy + hs, op(WHITE));
        canvas::rounded(&mut lay, qx - hs, qy - hs, qx + hs, qy + hs, 0.0, None, Some((b, lw)));
    }
    c.comp(&lay, 0.0, 0.0, al, true, false);
    c.place(&t, cx, cy, 1.0, 0.0, al, true, false);
    let cu = e.cache("cu", || canvas::cursor(w * 0.045));
    let (uw, uh) = (cu.width() as f32, cu.height() as f32);
    c.place(&cu, x0 + bw + uw * 0.4, y0 + bh + uh * 0.4, 1.0, 0.0, al, false, false);
}

pub(crate) const WAVE_TRIM: Effect = Effect {
    name: "wave-trim",
    title: "Wave Trim",
    desc: "The silent parts of your voice are trimmed away on screen until only your words are left",
    options: "x, y: where (0 to 1)",
    cards: false,
    draw: wave_trim,
};
fn wave_trim(c: &mut Ctx, e: &mut Run) {
    let al = pop(c, 0.0, 0.2) * e.fade(c, 0.2);
    if al <= 0.0 {
        return;
    }
    let w = c.w as f32;
    let p = ease_inout((c.lt - 0.25) / 0.5);
    let (pw, ph, ww) = (w * (0.84 - 0.4 * p), w * 0.2, w * 0.3);
    let hts = e.cache("h", || {
        let mut r = Rng::new(3);
        (0..60).map(|_| r.uniform(0.15, 1.0)).collect::<Vec<f32>>()
    });
    let mut lay = blank(pw + 2.0, ph + 2.0);
    canvas::rect(&mut lay, 0.0, 0.0, pw, ph, with_a(INKY, 240));
    let (cx, cy) = (pw / 2.0, ph / 2.0);
    let st = ww / hts.len() as f32;
    for (j, hh) in hts.iter().enumerate() {
        let x = cx - ww / 2.0 + j as f32 * st;
        let v = hh * ph * 0.36 * (0.55 + 0.45 * (j as f32 * 0.33).sin());
        canvas::rect(&mut lay, x, cy - v, x + st * 0.45, cy + v, op(CREAMY));
    }
    let (r, gap) = (w * 0.0045, w * 0.016);
    for sd in [-1.0f32, 1.0] {
        let mut x = cx + sd * (ww / 2.0 + gap);
        while x > 0.0 && x < pw {
            canvas::ellipse(&mut lay, x - r, cy - r, x + r, cy + r, Some(op(RED)), None);
            x += sd * gap;
        }
    }
    // above the head, or where you put it
    let y = if e.has("y") { e.f("y", 0.3) * c.h as f32 } else { above_head(c, ph) };
    let x = if e.has("x") { e.f("x", 0.5) * w } else { w / 2.0 };
    c.place(&lay, x, y, 1.0, 0.0, al, true, false);
}

pub(crate) const HUGE_STRIKE: Effect = Effect {
    name: "huge-strike",
    title: "Huge Strike",
    desc: "One huge serif word wipes in behind your head, then a red line crosses it out",
    options: "text (8 HOURS); at: seconds into the effect the strike lands (0.6)",
    cards: false,
    draw: huge_strike,
};
fn huge_strike(c: &mut Ctx, e: &mut Run) {
    let al = e.fade(c, 0.2);
    let w = c.w as f32;
    let word = e.s("text", "8 HOURS");
    let t = e.cache("t", || fit_text(&word, w * 0.9, Kind::Didone, (w * 0.28).floor(), CREAMY, Look { shadow: 10.0, ..Look::default() }));
    let (tw, th) = (t.width() as f32, t.height() as f32);
    let cw = (tw * ease_out(c.lt / 0.35)).floor();
    let cy = above_head(c, th);
    if cw > 2.0 {
        c.place(&canvas::crop(&t, 0.0, 0.0, cw, th), w / 2.0 - (tw - cw) / 2.0, cy, 1.0, 0.0, al, true, false);
    }
    let at = e.f("at", 0.6);
    let k = ease_out((c.lt - at) / 0.18);
    if k > 0.0 {
        let mut lay = blank(w, c.h as f32);
        let (x0, x1) = (w / 2.0 - tw * 0.46, w / 2.0 + tw * 0.46);
        let pts: Vec<(f32, f32)> = (0..40)
            .map(|i| {
                let u = i as f32 / 39.0;
                (x0 + (x1 - x0) * u, cy + th * 0.04 - u * th * 0.05 + (u * 19.0).sin() * w * 0.002)
            })
            .collect();
        canvas::hand_path(&mut lay, &pts, op(RED), (w / 80.0).floor().max(6.0), k);
        c.comp(&lay, 0.0, 0.0, al, true, false);
    }
}

pub(crate) const CRAYON_WRITE: Effect = Effect {
    name: "crayon-write",
    title: "Crayon Write",
    desc: "A red crayon writes your word behind your head, then underlines it and flies off",
    options: "text (NOBODY)",
    cards: false,
    draw: crayon_write,
};
fn crayon_write(c: &mut Ctx, e: &mut Run) {
    let al = e.fade(c, 0.2);
    let w = c.w as f32;
    let word = e.s("text", "NOBODY");
    let t = e.cache("t", || fit_text(&word, w * 0.82, Kind::Hand, (w * 0.17).floor(), CRAYON, Look { shadow: 4.0, ..Look::default() }));
    let (tw, th) = (t.width() as f32, t.height() as f32);
    let p = ease_inout((c.lt - 0.05) / 0.8);
    let cw = (tw * p).floor();
    let cy = above_head(c, th);
    let x0 = w / 2.0 - tw / 2.0;
    if cw > 2.0 {
        c.place(&canvas::crop(&t, 0.0, 0.0, cw, th), x0 + cw / 2.0, cy, 1.0, 0.0, al, true, false);
    }
    if c.lt > 0.9 {
        let mut lay = blank(w, c.h as f32);
        let yy = cy + th * 0.42;
        let pts: Vec<(f32, f32)> = (0..40)
            .map(|i| {
                let u = i as f32 / 39.0;
                (x0 + tw * (0.06 + 0.88 * u), yy - u * w * 0.006 + (u * 23.0).sin() * w * 0.002)
            })
            .collect();
        canvas::hand_path(&mut lay, &pts, op(CRAYON), (w / 95.0).floor().max(5.0), ease_out((c.lt - 0.9) / 0.25));
        c.comp(&lay, 0.0, 0.0, al, true, false);
    }
    let cr = e.cache("cr", || {
        let l = (w * 0.22).floor();
        let tt = (l * 0.2).floor();
        let mut im = blank(l + tt, tt);
        canvas::polygon(&mut im, &[(0.0, tt / 2.0), (tt, 0.0), (tt, tt)], Some((224, 58, 68, 255)), None);
        canvas::rounded(&mut im, tt * 0.9, 0.0, l + tt - 1.0, tt - 1.0, (tt / 5.0).floor(), Some((224, 58, 68, 255)), None);
        for xx in [tt * 1.5, l + tt * 0.4] {
            canvas::rect(&mut im, xx, 0.0, xx + tt * 0.35, tt - 1.0, (168, 23, 40, 255));
        }
        // turned 38 degrees clockwise, on a canvas grown to hold it
        let (iw, ih) = (im.width() as f32, im.height() as f32);
        let a = 38f32.to_radians();
        let (nw, nh) = (iw * a.cos() + ih * a.sin(), iw * a.sin() + ih * a.cos());
        let mut turned = blank(nw.ceil(), nh.ceil());
        let tf = tiny_skia::Transform::from_translate(nw / 2.0, nh / 2.0).pre_rotate(38.0).pre_translate(-iw / 2.0, -ih / 2.0);
        turned.draw_pixmap(0, 0, im.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..tiny_skia::PixmapPaint::default() }, tf, None);
        shadowed(&turned, 6.0, 4.0, 100)
    });
    let away = ease_inout((c.lt - 1.1) / 0.3);
    if away < 1.0 {
        let tx = x0 + cw + away * w * 0.6;
        let ty = cy + (c.lt * 30.0).sin() * th * 0.12 + away * c.h as f32 * 0.2;
        let (cw_, ch_) = (cr.width() as f32, cr.height() as f32);
        c.place(&cr, tx + cw_ * 0.47, ty + ch_ * 0.45, 1.0, 0.0, al, false, false);
    }
}

pub(crate) const FAN_BUILD: Effect = Effect {
    name: "fan-build",
    title: "Fan Build",
    desc: "Your reel covers deal into a fan behind your head, one card at a time",
    options: "images: your covers; count (9); size: card width (0.17); spread: degrees (110)",
    cards: true,
    draw: fan_build,
};
fn fan_build(c: &mut Ctx, e: &mut Run) {
    let n = (e.f("count", 9.0).round() as usize).clamp(2, 24);
    let al = e.fade(c, 0.2);
    let cw = (c.w as f32 * e.f("size", 0.17)).floor();
    let cs = e.cache_with("cards", |e| {
        if e.cards.is_empty() {
            return Vec::new();
        }
        let own = e.has("images");
        (0..n)
            .map(|i| {
                let im = &e.cards[i % e.cards.len()];
                shadowed(&card(&if own { im.clone() } else { vary(im, i) }, cw), 10.0, 4.0, 110)
            })
            .collect::<Vec<Pixmap>>()
    });
    if cs.is_empty() {
        return;
    }
    // a hand of cards held below the chin: every card turns about the same
    // point, evenly spread, dealt out from the middle one after another
    let spread = e.f("spread", 110.0).clamp(20.0, 180.0);
    let (px, py) = (c.fx, c.fy + c.fw * 1.5);
    let r = c.fw * 2.6;
    let breathe = 1.5 * (c.lt * 1.6).sin();
    for i in 0..n.min(cs.len()) {
        let q = ease_back((c.lt - 0.06 * i as f32) / 0.35);
        if q <= 0.0 {
            continue;
        }
        let target = -spread / 2.0 + spread * i as f32 / (n - 1) as f32;
        let a = target * q + breathe * q;
        let rad = a.to_radians();
        let dist = r * (0.55 + 0.45 * q.min(1.0));
        c.place(&cs[i], px + dist * rad.sin(), py - dist * rad.cos(), 0.8 + 0.2 * q.min(1.0), -a, al * clamp(q * 2.0), true, false);
    }
}
