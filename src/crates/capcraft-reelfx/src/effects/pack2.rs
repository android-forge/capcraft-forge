// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! `fx.py`'s second pack, the app-style effects, and its comment sheet.

use std::f32::consts::PI;

use tiny_skia::Pixmap;

use super::{Rng, cover, parts, round_corners, thumb_grid};
use crate::canvas::{self, Anchor, Kind, Look, Rgb, blank, fit_text, line, lines_fit, op, over, rrect, shadowed, text, text_img, with_a};
use crate::ctx::{Ctx, Grade, clamp, ease_back, ease_inout, ease_out};
use crate::{BLUE, Effect, GREEN, ORANGE, PURPLE, RED, Run, YELLOW, pop};

const WHITE: Rgb = (255, 255, 255);

/// A round avatar cut from the face, grabbed once at the start of the
/// effect (`fx.py`'s `face_chip`).
pub(crate) fn face_chip(c: &Ctx, e: &mut Run, size: f32) -> Pixmap {
    let r = c.fw * 0.9;
    let (x0, y0) = ((c.fx - r).max(0.0), (c.fy - r * 1.05).max(0.0));
    let src = e.cache("face", || c.region(x0, y0, 2.0 * r, 2.0 * r, 2.0 * r, 2.0 * r));
    let mut im = canvas::resized(&src, size, size);
    let mut m = blank(size, size);
    canvas::ellipse(&mut m, 0.0, 0.0, size - 1.0, size - 1.0, Some((255, 255, 255, 255)), None);
    let a: Vec<u8> = m.data().chunks_exact(4).map(|p| p[3]).collect();
    canvas::mask_alpha(&mut im, &a);
    im
}

/// `n` as text with `dp` decimals and `suf`.
fn num(v: f32, dp: usize, suf: &str) -> String {
    format!("{v:.dp$}{suf}")
}

/// `'197K'` as `(197, "K", 0)`, `'4.8'` as `(4.8, "", 1)` (`num_parts`).
pub(crate) fn num_parts(s: &str) -> (f32, String, usize) {
    let s = s.trim();
    let end = s.rfind(|ch: char| ch.is_ascii_digit()).map_or(0, |i| i + 1);
    let n = s[..end].replace(',', "");
    let n = if n.is_empty() { "0".to_owned() } else { n };
    let dp = n.split('.').nth(1).map_or(0, str::len);
    (n.parse().unwrap_or(0.0), s[end..].to_owned(), dp)
}

/// A bobbing, popping widget of the glass set.
#[allow(clippy::too_many_arguments)]
fn show(c: &mut Ctx, im: &Pixmap, x: f32, y: f32, i: usize, front: bool, al: f32, u: f32) {
    let t0 = 0.07 * i as f32;
    if c.lt < t0 {
        return;
    }
    let bob = 6.0 * u * (2.0 * PI * (c.lt * 0.55 + i as f32 * 0.17)).sin();
    c.place(im, x, y + bob, 0.6 + 0.4 * pop(c, t0, 0.32), 0.0, al * clamp((c.lt - t0) / 0.08), !front, front);
}

pub(crate) const GLASS_WIDGETS: Effect = Effect {
    name: "glass-widgets",
    title: "Glass Widgets",
    desc: "Frosted glass widgets and mini cards float around your head, with an Edited in pill",
    options: "text: the big number; title; name; handle; pill (Edited in |Forge); look: simple for three widgets",
    cards: false,
    draw: glass_widgets,
};
fn glass_widgets(c: &mut Ctx, e: &mut Run) {
    let k0 = e.ramp(c, 0.3, 0.0);
    let grade = e.s("grade", "warm");
    if grade != "none" {
        let g = c.graded(Grade::named(&grade));
        c.mix_with(&g, 0.6 * k0);
    }
    if e.s("look", "") == "simple" {
        return glass_widgets_simple(c, e);
    }
    let (w, h) = (c.w as f32, c.h as f32);
    let u = w / 1080.0;
    let al = e.fade(c, 0.2);
    let gd = e.f("dark", 0.3);
    // one accent through every widget
    let acc = e.color("color", (255, 159, 10));
    let g = |c: &Ctx, ww: f32, hh: f32, cx: f32, cy: f32, r: Option<f32>| c.glass(ww, hh, r.unwrap_or(ww.min(hh) * 0.2), cx, cy, 0.14, WHITE, None, 240.0, gd, true);
    // 1 · stat tile
    let s = 300.0 * u;
    let (ax, ay) = (50.0 * u + s / 2.0, 118.0 * u + s / 2.0);
    let mut a = g(c, s, s, ax, ay, None);
    canvas::ellipse(&mut a, s * 0.1, s * 0.115, s * 0.16, s * 0.175, Some(op(acc)), None);
    text(&mut a, s * 0.2, s * 0.145, &e.s("title", "Studio"), Kind::Uim, s * 0.085, with_a(WHITE, 235), Anchor::LM);
    text(&mut a, s * 0.08, s * 0.2, &e.s("text", "51"), Kind::Uib, s * 0.44, op(WHITE), Anchor::LA);
    for (j, ln) in parts(&e.s("sub", "effects|one clip")).iter().enumerate() {
        text(&mut a, s * 0.1, s * (0.7 + 0.11 * j as f32), ln, Kind::Uim, s * 0.085, with_a(WHITE, 235), Anchor::LA);
    }
    show(c, &a, ax, ay, 0, false, al, u);
    // 2 · render bar
    let (bw, bh) = (400.0 * u, 150.0 * u);
    let (bx, by) = (w - 50.0 * u - bw / 2.0, 118.0 * u + bh / 2.0);
    let mut b = g(c, bw, bh, bx, by, Some(bh * 0.28));
    let p = ease_inout(c.lt / (e.d * 0.85).max(0.3));
    canvas::ellipse(&mut b, bh * 0.17, bh * 0.2, bh * 0.28, bh * 0.31, Some(op(acc)), None);
    text(&mut b, bh * 0.36, bh * 0.255, "Rendering", Kind::Uim, bh * 0.19, with_a(WHITE, 240), Anchor::LM);
    canvas::rounded(&mut b, bh * 0.17, bh * 0.5, bw - bh * 0.17, bh * 0.6, bh * 0.05, Some(with_a(WHITE, 70)), None);
    canvas::rounded(&mut b, bh * 0.17, bh * 0.5, bh * 0.17 + (bw - bh * 0.34) * p.max(0.04), bh * 0.6, bh * 0.05, Some(op(acc)), None);
    text(&mut b, bh * 0.17, bh * 0.78, &format!("4K render  ·  {}%", (p * 100.0) as i32), Kind::Uim, bh * 0.14, with_a(WHITE, 210), Anchor::LM);
    show(c, &b, bx, by, 1, false, al, u);
    // 3 · profile
    let cy3 = by + bh / 2.0 + 22.0 * u + bh / 2.0;
    let mut gp = g(c, bw, bh, bx, cy3, Some(bh * 0.28));
    let av = face_chip(c, e, bh * 0.62);
    over(&mut gp, &av, (bw - bh * 0.19 - av.width() as f32) as i32, ((bh - av.height() as f32) / 2.0) as i32);
    text(&mut gp, bh * 0.2, bh * 0.36, &e.s("name", "Your Name"), Kind::Uib, bh * 0.22, op(WHITE), Anchor::LM);
    text(&mut gp, bh * 0.2, bh * 0.66, &e.s("handle", "@your.handle"), Kind::Uim, bh * 0.14, with_a(WHITE, 215), Anchor::LM);
    show(c, &gp, bx, cy3, 2, false, al, u);
    // 4 · a mini views page inside the glass
    let (aw, ah) = (330.0 * u, 330.0 * u);
    let (dx, dy) = (50.0 * u + aw / 2.0, ay + s / 2.0 + 26.0 * u + ah / 2.0);
    let mut art = g(c, aw, ah, dx, dy, Some(44.0 * u));
    text(&mut art, 26.0 * u, 40.0 * u, "Top reels", Kind::Uim, 20.0 * u, with_a(WHITE, 225), Anchor::LM);
    text(&mut art, 26.0 * u, 92.0 * u, &e.s("views", "197K"), Kind::Uib, 58.0 * u, op(WHITE), Anchor::LM);
    let vals = [197.0, 140.0, 124.0, 83.0, 50.0, 40.0, 38.0];
    let (gx0, gx1, gy1) = (26.0 * u, aw - 26.0 * u, ah - 30.0 * u);
    let bwid = (gx1 - gx0) / 7.0 * 0.62;
    for (j, v) in vals.iter().enumerate() {
        let q = ease_out((c.lt - 0.35 - 0.05 * j as f32) / 0.4);
        let hh = (ah * 0.4) * v / 197.0 * q;
        let x0 = gx0 + (gx1 - gx0) * j as f32 / 7.0;
        if hh > 1.0 {
            canvas::rounded(&mut art, x0, gy1 - hh, x0 + bwid, gy1, bwid / 2.0, Some(if j == 0 { op(acc) } else { with_a(WHITE, 150) }), None);
        }
    }
    show(c, &art, dx, dy, 3, false, al, u);
    // 5 · now editing
    let (mw, mh) = (360.0 * u, 220.0 * u);
    let (mx, my) = (w - 50.0 * u - mw / 2.0, cy3 + bh / 2.0 + 22.0 * u + mh / 2.0);
    let mut mu = g(c, mw, mh, mx, my, Some(40.0 * u));
    let aa = (96.0 * u).floor();
    let mut art2 = blank(aa, aa);
    for (y, row) in art2.data_mut().chunks_exact_mut(aa as usize * 4).enumerate() {
        let t = y as f32 / (aa - 1.0).max(1.0);
        let col = [255.0 * (1.0 - t) + 217.0 * t, 150.0 * (1.0 - t) + 90.0 * t, 110.0 * (1.0 - t) + 70.0 * t];
        for px in row.chunks_exact_mut(4) {
            px[0] = col[0] as u8;
            px[1] = col[1] as u8;
            px[2] = col[2] as u8;
            px[3] = 255;
        }
    }
    round_corners(&mut art2, 16.0 * u);
    for k in 0..6 {
        let ang = k as f32 * PI / 3.0;
        canvas::line_flat(&mut art2, &[(aa / 2.0 - 26.0 * u * ang.cos(), aa / 2.0 - 26.0 * u * ang.sin()), (aa / 2.0 + 26.0 * u * ang.cos(), aa / 2.0 + 26.0 * u * ang.sin())], op(WHITE), 7.0 * u);
    }
    over(&mut mu, &art2, (22.0 * u) as i32, (22.0 * u) as i32);
    text(&mut mu, 134.0 * u, 44.0 * u, "NOW EDITING", Kind::Mono, 15.0 * u, with_a(WHITE, 200), Anchor::LM);
    text(&mut mu, 134.0 * u, 78.0 * u, &e.s("song", "soft UI pops"), Kind::Uib, 28.0 * u, op(WHITE), Anchor::LM);
    text(&mut mu, 134.0 * u, 108.0 * u, "sfx · 25.7s", Kind::Uim, 20.0 * u, with_a(WHITE, 200), Anchor::LM);
    let pp = clamp(c.lt / e.d.max(0.5));
    canvas::rounded(&mut mu, 22.0 * u, 140.0 * u, mw - 22.0 * u, 146.0 * u, 3.0 * u, Some(with_a(WHITE, 70)), None);
    canvas::rounded(&mut mu, 22.0 * u, 140.0 * u, 22.0 * u + (mw - 44.0 * u) * pp, 146.0 * u, 3.0 * u, Some(with_a(WHITE, 240)), None);
    let (cxm, cym) = (mw / 2.0, 184.0 * u);
    canvas::polygon(&mut mu, &[(cxm - 12.0 * u, cym - 16.0 * u), (cxm - 12.0 * u, cym + 16.0 * u), (cxm + 16.0 * u, cym)], Some(op(WHITE)), None);
    for sgn in [-1.0f32, 1.0] {
        let x = cxm + sgn * 80.0 * u;
        canvas::polygon(&mut mu, &[(x - sgn * 10.0 * u, cym - 11.0 * u), (x - sgn * 10.0 * u, cym + 11.0 * u), (x + sgn * 6.0 * u, cym)], Some(with_a(WHITE, 220)), None);
        canvas::polygon(&mut mu, &[(x + sgn * 4.0 * u, cym - 11.0 * u), (x + sgn * 4.0 * u, cym + 11.0 * u), (x + sgn * 20.0 * u, cym)], Some(with_a(WHITE, 220)), None);
    }
    show(c, &mu, mx, my, 4, false, al, u);
    // 6 · calendar
    let (kw, kh) = (330.0 * u, 190.0 * u);
    let (kx, ky) = (50.0 * u + kw / 2.0, dy + ah / 2.0 + 26.0 * u + kh / 2.0);
    let mut ca = g(c, kw, kh, kx, ky, Some(40.0 * u));
    text(&mut ca, 26.0 * u, 36.0 * u, &e.s("day", "WED"), Kind::Uib, 22.0 * u, op(acc), Anchor::LM);
    text(&mut ca, 26.0 * u, 82.0 * u, &e.s("date", "21"), Kind::Uib, 56.0 * u, op(WHITE), Anchor::LM);
    canvas::rounded(&mut ca, 22.0 * u, 120.0 * u, kw - 22.0 * u, 168.0 * u, 12.0 * u, Some(with_a(acc, 220)), None);
    text(&mut ca, 36.0 * u, 144.0 * u, &e.s("event", "Post 2 reels · tonight"), Kind::Uib, 22.0 * u, op(WHITE), Anchor::LM);
    show(c, &ca, kx, ky, 5, false, al, u);
    // 7 · 4K ring
    let rr_ = 250.0 * u;
    let (rx, ry) = (w - 50.0 * u - rr_ / 2.0, my + mh / 2.0 + 26.0 * u + rr_ / 2.0);
    let mut rg = g(c, rr_, rr_, rx, ry, Some(56.0 * u));
    let rp = ease_inout((c.lt - 0.4) / 1.0);
    let rr = rr_ * 0.34;
    canvas::ellipse(&mut rg, rr_ / 2.0 - rr, rr_ / 2.0 - rr - 8.0 * u, rr_ / 2.0 + rr, rr_ / 2.0 + rr - 8.0 * u, None, Some((with_a(WHITE, 60), 16.0 * u)));
    if rp > 0.0 {
        canvas::arc(&mut rg, rr_ / 2.0 - rr, rr_ / 2.0 - rr - 8.0 * u, rr_ / 2.0 + rr, rr_ / 2.0 + rr - 8.0 * u, -90.0, -90.0 + 360.0 * rp, op(acc), 16.0 * u);
    }
    text(&mut rg, rr_ / 2.0, rr_ / 2.0 - 8.0 * u, "4K", Kind::Uib, 52.0 * u, op(WHITE), Anchor::MM);
    text(&mut rg, rr_ / 2.0, rr_ - 26.0 * u, "render", Kind::Uim, 20.0 * u, with_a(WHITE, 210), Anchor::MM);
    show(c, &rg, rx, ry, 6, false, al, u);
    // 8 · the pill, in front
    let q = edited_pill(c, e, h * 0.875);
    show(c, &q, w / 2.0, h * 0.875, 7, true, al, u);
}

/// The glass pill along the bottom: `pre` in a regular weight, `hi` bold
/// and peach.
fn edited_pill(c: &Ctx, e: &Run, py: f32) -> Pixmap {
    let w = c.w as f32;
    let pill = e.s("pill", "Edited in |Forge");
    let (pre, hi) = pill.split_once('|').map_or((pill.clone(), String::new()), |(a, b)| (a.to_owned(), b.to_owned()));
    let size = w * 0.045;
    let tw = canvas::length(Kind::Uim, size, &pre) + canvas::length(Kind::Uib, size, &hi);
    let ph = w * 0.105;
    let pw = tw + ph * 0.9;
    let mut q = c.glass(pw, ph, ph / 2.0, w / 2.0, py, 0.16, (70, 70, 76), None, 240.0, 0.0, true);
    text(&mut q, (pw - tw) / 2.0, ph / 2.0, &pre, Kind::Uim, size, op(WHITE), Anchor::LM);
    text(&mut q, (pw - tw) / 2.0 + canvas::length(Kind::Uim, size, &pre), ph / 2.0, &hi, Kind::Uib, size, (255, 176, 146, 255), Anchor::LM);
    shadowed(&q, 14.0, 6.0, 70)
}

/// The first glass set: three widgets and the pill (`glass_widgets_v1`).
fn glass_widgets_simple(c: &mut Ctx, e: &mut Run) {
    let al = e.fade(c, 0.2);
    let (w, h) = (c.w as f32, c.h as f32);
    let mg = w * 0.055;
    let s = w * 0.31;
    let y_top = h * 0.07;
    let gd = e.f("dark", 0.2);
    let mut a = c.glass(s, s, s * 0.2, mg + s / 2.0, y_top + s / 2.0, 0.14, WHITE, None, 240.0, gd, true);
    canvas::ellipse(&mut a, s * 0.1, s * 0.115, s * 0.16, s * 0.175, Some(op(ORANGE)), None);
    text(&mut a, s * 0.2, s * 0.145, &e.s("title", "Studio"), Kind::Uim, s * 0.085, with_a(WHITE, 235), Anchor::LM);
    text(&mut a, s * 0.08, s * 0.2, &e.s("text", "51"), Kind::Uib, s * 0.44, op(WHITE), Anchor::LA);
    for (j, ln) in parts(&e.s("sub", "effects|one clip")).iter().enumerate() {
        text(&mut a, s * 0.1, s * (0.7 + 0.11 * j as f32), ln, Kind::Uim, s * 0.085, with_a(WHITE, 235), Anchor::LA);
    }
    c.place(&a, mg + s / 2.0, y_top + s / 2.0, 0.6 + 0.4 * pop(c, 0.0, 0.3), 0.0, al * clamp(c.lt / 0.1), true, false);
    let (bw, bh) = (w * 0.37, w * 0.15);
    let bx = w - mg - bw / 2.0;
    let mut b = c.glass(bw, bh, bh * 0.28, bx, y_top + bh / 2.0, 0.14, WHITE, None, 240.0, gd, true);
    canvas::ellipse(&mut b, bh * 0.17, bh * 0.2, bh * 0.28, bh * 0.31, Some((255, 69, 58, 255)), None);
    text(&mut b, bh * 0.36, bh * 0.255, "Rendering", Kind::Uim, bh * 0.19, with_a(WHITE, 240), Anchor::LM);
    let p = ease_inout(c.lt / (e.d * 0.85).max(0.3));
    canvas::rounded(&mut b, bh * 0.17, bh * 0.5, bw - bh * 0.17, bh * 0.6, bh * 0.05, Some(with_a(WHITE, 70)), None);
    canvas::rounded(&mut b, bh * 0.17, bh * 0.5, bh * 0.17 + (bw - bh * 0.34) * p.max(0.04), bh * 0.6, bh * 0.05, Some(with_a(WHITE, 245)), None);
    text(&mut b, bh * 0.17, bh * 0.78, &format!("{}  ·  {}%", e.s("label", "glass UI"), (p * 100.0) as i32), Kind::Uim, bh * 0.14, with_a(WHITE, 210), Anchor::LM);
    c.place(&b, bx, y_top + bh / 2.0, 0.6 + 0.4 * pop(c, 0.12, 0.3), 0.0, al * clamp((c.lt - 0.12) / 0.1), true, false);
    let cy2 = y_top + bh + mg * 0.45 + bh / 2.0;
    let mut g = c.glass(bw, bh, bh * 0.28, bx, cy2, 0.14, WHITE, None, 240.0, gd, true);
    let av = face_chip(c, e, bh * 0.62);
    over(&mut g, &av, (bw - bh * 0.19 - av.width() as f32) as i32, ((bh - av.height() as f32) / 2.0) as i32);
    text(&mut g, bh * 0.2, bh * 0.36, &e.s("name", "Your Name"), Kind::Uib, bh * 0.22, op(WHITE), Anchor::LM);
    text(&mut g, bh * 0.2, bh * 0.66, &e.s("handle", "@your.handle"), Kind::Uim, bh * 0.14, with_a(WHITE, 215), Anchor::LM);
    c.place(&g, bx, cy2, 0.6 + 0.4 * pop(c, 0.24, 0.3), 0.0, al * clamp((c.lt - 0.24) / 0.1), true, false);
    let py = h * 0.865;
    let q = edited_pill(c, e, py);
    c.place(&q, w / 2.0, py, 0.7 + 0.3 * pop(c, 0.4, 0.3), 0.0, al * clamp((c.lt - 0.4) / 0.1), false, true);
}

pub(crate) const RED_COLLAGE: Effect = Effect {
    name: "red-collage",
    title: "Red Collage",
    desc: "Black and white you, a giant red word behind, three lines slam in",
    options: "text: the red word; lines joined with |; tag; color; photo: bw or color; strength: how solid the word is (0 to 1)",
    cards: false,
    draw: red_collage,
};
fn red_collage(c: &mut Ctx, e: &mut Run) {
    let k = e.ramp(c, 0.15, 0.0);
    // the room in black and white (or as shot), darkened behind you
    let bw = if e.s("photo", "bw") == "color" { c.img.clone() } else { c.graded(Grade::Bw) };
    let mut out = bw.clone();
    for i in 0..c.w * c.h {
        let m = c.m[i];
        for ch in 0..3 {
            out[i * 3 + ch] = bw[i * 3 + ch] * m + bw[i * 3 + ch] * 0.62 * (1.0 - m);
        }
    }
    c.mix_with(&out, k);
    c.vignette(0.45 * k);
    let red = e.color("color", RED);
    let al = e.fade(c, 0.2);
    let (w, h) = (c.w as f32, c.h as f32);
    let word = e.s("text", "EDIT").to_uppercase();
    // the word sits in the room rather than glowing on it
    let wim = e.cache("w", || fit_text(&word, w * 0.98, Kind::Uib, 700.0, red, Look { shadow: 14.0, ..Look::default() }));
    let s = 1.12 - 0.12 * ease_out(c.lt / 0.4);
    let y = (wim.height() as f32 * 0.42).max(c.fy - c.fw * 1.05);
    c.place(&wim, w / 2.0, y, s, 0.0, clamp(c.lt / 0.12) * al * e.f("strength", 0.8), true, false);
    let lines = parts(&e.s("lines", "ONE CLIP.|51 EFFECTS.|ZERO APPS."));
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let size = *e.cache("sz", || lines_fit(&refs, w * 0.8, Kind::Uib, (w * 0.14).floor()));
    let mut y = (h * 0.5).max(c.fy + c.fw * 1.55);
    for (j, ln) in lines.iter().enumerate() {
        let im = e.cache(&format!("l{j}"), || text_img(ln, size, Kind::Uib, if j == 1 { red } else { WHITE }, Look { shadow: 8.0, ..Look::default() }));
        let kk = clamp((c.lt - 0.18 - 0.13 * j as f32) / 0.16);
        if kk > 0.0 {
            let (iw, ih) = (im.width() as f32, im.height() as f32);
            c.place(&im, w * 0.05 + iw / 2.0, y + ih / 2.0, 1.4 - 0.4 * ease_out(kk), 0.0, kk * al, false, true);
        }
        y += size;
    }
    let tag_s = e.s("tag", "REC  ·  RED COLLAGE");
    let tag = e.cache("tag", || text_img(&tag_s, w * 0.027, Kind::Mono, red, Look::default()));
    let ds = (tag.height() as f32 * 0.4).floor();
    let mut dot = blank(ds, ds);
    canvas::ellipse(&mut dot, 0.0, 0.0, ds - 1.0, ds - 1.0, Some(op(red)), None);
    let blink = if (c.lt * 2.5) as i32 % 2 == 0 { 1.0 } else { 0.35 };
    c.put(&dot, w * 0.07, h * 0.93, al * k * blink, false, true);
    c.put(&tag, w * 0.07 + ds + tag.width() as f32 / 2.0, h * 0.93, al * k, false, true);
}

pub(crate) const SPLIT_HEADLINE: Effect = Effect {
    name: "split-headline",
    title: "Split Headline",
    desc: "A headline card slides down and pushes you to the bottom half",
    options: "text: lines joined with |; hl: the highlighted word; pill; color; height (0.36)",
    cards: false,
    draw: split_headline,
};
fn split_headline(c: &mut Ctx, e: &mut Run) {
    let k = ease_out(c.lt / 0.38) * clamp((e.d - c.lt) / 0.25);
    let ch = c.h as f32 * e.f("height", 0.36);
    c.shift(ch * e.f("push", 0.72) * k);
    let top = ch * k;
    let w = c.w as f32;
    let col = e.color("color", GREEN);
    let mut lay = rrect(w, ch, 0.0, (244, 242, 236, 255), None);
    let lines = parts(&e.s("text", "One clip|edits my|reels."));
    let hl = e.s("hl", "edits");
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let s = *e.cache("sz", || lines_fit(&refs, w * 0.84, Kind::Uib, (w * 0.135).floor()));
    let (x0, y0) = (w * 0.07, ch * 0.13);
    let len = |t: &str| canvas::length(Kind::Uib, s, t);
    for (j, ln) in lines.iter().enumerate() {
        let kj = clamp((c.lt - 0.12 - 0.08 * j as f32) / 0.18);
        if kj <= 0.0 {
            continue;
        }
        let y = y0 + j as f32 * s * 1.04 + (1.0 - ease_out(kj)) * s * 0.35;
        let a = (255.0 * kj) as u8;
        let mut x = x0;
        for word in ln.split(' ') {
            let (core, tail) = match word.strip_suffix('.') {
                Some(core) => (core, "."),
                None => (word, ""),
            };
            let wl = len(core);
            if core == hl {
                let hk = ease_out((c.lt - 0.42) / 0.22);
                let pad = s * 0.12;
                if hk > 0.0 {
                    canvas::rounded(&mut lay, x - pad, y + s * 0.02, x - pad + (wl + 2.0 * pad) * hk, y + s * 1.1, s * 0.16, Some(with_a(col, a)), None);
                }
                text(&mut lay, x, y, core, Kind::Uib, s, if hk > 0.55 { (255, 255, 255, a) } else { (20, 20, 22, a) }, Anchor::LA);
            } else {
                text(&mut lay, x, y, core, Kind::Uib, s, (20, 20, 22, a), Anchor::LA);
            }
            x += wl;
            if !tail.is_empty() {
                text(&mut lay, x, y, tail, Kind::Uib, s, with_a(col, a), Anchor::LA);
                x += len(tail);
            }
            x += len(" ") + if core == hl { s * 0.12 } else { 0.0 };
        }
    }
    canvas::rect(&mut lay, 0.0, ch - (w / 180.0).max(5.0), w, ch, op(col));
    c.comp(&lay, 0.0, top - ch, 1.0, false, false);
    let pt = text_img(&e.s("pill", "in seconds ↗"), w * 0.034, Kind::Uib, WHITE, Look::default());
    let mut pill = rrect(pt.width() as f32 + w * 0.02, pt.height() as f32 * 0.95, pt.height() as f32 * 0.47, op(col), None);
    over(&mut pill, &pt, (w * 0.01) as i32, (-(pt.height() as f32) * 0.025) as i32);
    let (pw, ph) = (pill.width() as f32, pill.height() as f32);
    c.place(&shadowed(&pill, 10.0, 4.0, 80), w * 0.93 - pw / 2.0, top + ph * 0.95, 0.6 + 0.4 * pop(c, 0.62, 0.3), 0.0, clamp((c.lt - 0.62) / 0.1) * k, false, true);
}

pub(crate) const PASTEL_WORD: Effect = Effect {
    name: "pastel-word",
    title: "Pastel Word",
    desc: "Lavender room, a giant word behind you, a profile card gets followed",
    options: "text; name; handle; role; color: the wall at the top; bg: the wall at the foot; ink: the word",
    cards: false,
    draw: pastel_word,
};
fn pastel_word(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let k = e.ramp(c, 0.2, 0.0);
    let al = e.fade(c, 0.2);
    let mut target = vec![0f32; c.img.len()];
    // the wall: one pastel at the top fading to another at the foot
    let (top, foot) = (e.color("color", (179, 171, 247)), e.color("bg", (237, 230, 255)));
    let (top, foot) = ([top.0, top.1, top.2].map(|v| f32::from(v) / 255.0), [foot.0, foot.1, foot.2].map(|v| f32::from(v) / 255.0));
    for y in 0..c.h {
        for x in 0..c.w {
            let i = y * c.w + x;
            let t = ((y as f32 / h) * 0.8 + (x as f32 / w) * 0.2).clamp(0.0, 1.0);
            let g = [0, 1, 2].map(|ch| top[ch] * (1.0 - t) + foot[ch] * t);
            let p = c.px(i);
            let lum = p[0] * 0.299 + p[1] * 0.587 + p[2] * 0.114;
            let m = c.m[i];
            for ch in 0..3 {
                let bg = g[ch] * (0.86 + 0.22 * lum);
                let you = (p[ch] * 0.93 + 0.05).clamp(0.0, 1.0);
                target[i * 3 + ch] = you * m + bg * (1.0 - m);
            }
        }
    }
    c.mix_with(&target, k);
    let word = e.s("text", "REELS").to_uppercase();
    let ink = e.color("ink", WHITE);
    let wim = e.cache("w", || shadowed(&fit_text(&word, w * 0.96, Kind::Uib, 700.0, ink, Look::default()), 24.0, 12.0, 60));
    let dk = ease_out(c.lt / 0.45);
    let y = (wim.height() as f32 * 0.45).max(c.fy - c.fw * 1.2) - (1.0 - dk) * h * 0.06;
    c.place(&wim, w / 2.0, y, 1.0, 0.0, al * clamp(c.lt / 0.15), true, false);
    let (cw, chh) = (w * 0.86, w * 0.15);
    let cy = h * 0.865 + (1.0 - ease_out((c.lt - 0.15) / 0.35)) * h * 0.2;
    let mut card = rrect(cw, chh, chh * 0.3, (255, 255, 255, 248), None);
    let av = face_chip(c, e, chh * 0.64);
    over(&mut card, &av, (chh * 0.18) as i32, ((chh - av.height() as f32) / 2.0) as i32);
    text(&mut card, chh * 0.98, chh * 0.38, &e.s("name", "Your Name"), Kind::Uib, chh * 0.22, (20, 20, 22, 255), Anchor::LM);
    let h1 = e.s("handle", "@your.handle  ·  ");
    text(&mut card, chh * 0.98, chh * 0.66, &h1, Kind::Uim, chh * 0.15, (140, 140, 150, 255), Anchor::LM);
    text(&mut card, chh * 0.98 + canvas::length(Kind::Uim, chh * 0.15, &h1), chh * 0.66, &e.s("role", "creator"), Kind::Uim, chh * 0.15, op(PURPLE), Anchor::LM);
    let at = e.d * 0.6;
    let on = c.lt >= at;
    text(&mut card, cw - chh * 0.22, chh / 2.0, if on { "Following" } else { "Follow" }, Kind::Uib, chh * 0.2, if on { (140, 140, 150, 255) } else { op(PURPLE) }, Anchor::RM);
    let sq = if on { 1.0 - 0.06 * (PI * clamp((c.lt - at) / 0.25)).sin() } else { 1.0 };
    c.place(&shadowed(&card, 18.0, 8.0, 90), w / 2.0, cy, sq, 0.0, al * clamp((c.lt - 0.15) / 0.12), false, true);
    let cu = e.cache("cur", || canvas::cursor(w * 0.06));
    let ck = ease_inout((c.lt - 0.25) / (at - 0.3).max(0.2));
    let (tx, ty) = (w / 2.0 + cw / 2.0 - chh * 0.55, cy + chh * 0.12);
    let (sx, sy) = (w * 1.02, h * 1.02);
    let (uw, uh) = (cu.width() as f32, cu.height() as f32);
    c.place(&cu, sx + (tx - sx) * ck + uw / 2.0, sy + (ty - sy) * ck + uh / 2.0, if on && c.lt - at < 0.15 { 0.85 } else { 1.0 }, 0.0, al * clamp((c.lt - 0.25) / 0.1), false, false);
}

pub(crate) const CHECK_PILLS: Effect = Effect {
    name: "check-pills",
    title: "Check Pills",
    desc: "Grid-paper room, reel boards behind you, a cursor clicks blue pills",
    options: "text: pills joined with |; images: your own pictures; handle; board",
    cards: true,
    draw: check_pills,
};
fn check_pills(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let k = e.ramp(c, 0.15, 0.0);
    let al = e.fade(c, 0.2);
    let st = (c.w / 20).max(20);
    let lw = (c.w / 700).max(1);
    let mut target = c.img.clone();
    for y in 0..c.h {
        for x in 0..c.w {
            let i = y * c.w + x;
            let on_line = x % st < lw || y % st < lw;
            let g = if on_line { [0.86, 0.875, 0.9] } else { [0.935, 0.945, 0.96] };
            let m = c.m[i];
            for ch in 0..3 {
                target[i * 3 + ch] = c.img[i * 3 + ch] * m + g[ch] * (1.0 - m);
            }
        }
    }
    c.mix_with(&target, k);
    let board = |e: &Run, i: usize, title: &str| {
        let bw = (w * 0.44).floor();
        let mut im = rrect(bw, (bw * 1.12).floor(), bw * 0.07, op(WHITE), None);
        canvas::ellipse(&mut im, bw * 0.06, bw * 0.055, bw * 0.06 + bw * 0.09, bw * 0.145, Some((255, 160, 140, 255)), None);
        text(&mut im, bw * 0.19, bw * 0.1, title, Kind::Uib, bw * 0.06, (20, 20, 22, 255), Anchor::LM);
        let gr = thumb_grid(&e.cards, 3, 3, (bw * 0.28).floor(), (bw * 0.025).floor(), i * 3, true);
        over(&mut im, &gr, ((bw - gr.width() as f32) / 2.0) as i32, (bw * 0.2) as i32);
        shadowed(&im, 16.0, 8.0, 70)
    };
    let (h1, h2) = (e.s("handle", "@your.handle"), e.s("board", "Reels  ·  51 effects"));
    let b1 = e.cache_with("b1", |e| board(e, 0, &h1));
    let b2 = e.cache_with("b2", |e| board(e, 1, &h2));
    let by = (h * 0.17).max(c.fy - c.fw * 0.9);
    c.place(&b1, w * 0.25, by, 0.6 + 0.4 * pop(c, 0.0, 0.3), 8.0, al * clamp(c.lt / 0.1), true, false);
    c.place(&b2, w * 0.76, by - w * 0.02, 0.6 + 0.4 * pop(c, 0.1, 0.3), -8.0, al * clamp((c.lt - 0.1) / 0.1), true, false);
    let names = parts(&e.s("text", "Hook|Captions|Cuts|B-roll"));
    let spots = [(-0.95, 2.35), (0.55, 2.0), (-0.35, 2.95), (0.95, 2.75), (0.1, 3.45), (-1.1, 3.3)];
    let ph = w * 0.078;
    let cu = e.cache("cur", || canvas::cursor(w * 0.055));
    let mut pos = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let (sx, sy) = spots[i % spots.len()];
        let x = (c.fx + sx * c.fw * 1.6).clamp(w * 0.16, w * 0.84);
        let y = (c.fy + sy * c.fw).clamp(h * 0.3, h * 0.9);
        let t0 = 0.22 + 0.2 * i as f32;
        pos.push((x, y, t0));
        if c.lt < t0 {
            continue;
        }
        let clicked = c.lt >= t0 + 0.12;
        let t = text_img(n, ph * 0.42, Kind::Uib, WHITE, Look::default());
        let pw = (t.width() as f32 + ph * if clicked { 0.75 } else { 0.35 }).floor();
        let mut pill = rrect(pw, ph, ph / 2.0, op(BLUE), None);
        over(&mut pill, &t, (ph * 0.18) as i32, ((ph - t.height() as f32) / 2.0) as i32);
        if clicked {
            canvas::tick(&mut pill, pw - ph * 0.55, ph * 0.33, ph * 0.33, op(WHITE), None);
        }
        let sq = 1.0 - 0.1 * (PI * clamp((c.lt - t0 - 0.12) / 0.2)).sin();
        c.place(&shadowed(&pill, 10.0, 5.0, 90), x, y, (0.5 + 0.5 * pop(c, t0, 0.25)) * sq, 0.0, al, false, true);
    }
    if let Some(&(_, _, first)) = pos.first()
        && c.lt > first - 0.1
    {
        let seg: Vec<_> = pos.iter().filter(|p| p.2 <= c.lt + 0.1).copied().collect();
        let (tx, ty, t0) = *seg.last().expect("one at least");
        let (px, py) = if seg.len() > 1 { (pos[seg.len() - 2].0, pos[seg.len() - 2].1) } else { (w * 0.9, h * 0.95) };
        let q = ease_inout((c.lt - (t0 - 0.12)) / 0.2);
        let (uw, uh) = (cu.width() as f32, cu.height() as f32);
        c.place(&cu, px + (tx - px) * q + ph * 0.3 + uw / 2.0, py + (ty - py) * q + ph * 0.1 + uh / 2.0, 1.0, 0.0, al, false, false);
    }
}

pub(crate) const TIMELINE: Effect = Effect {
    name: "timeline",
    title: "Timeline",
    desc: "A dark red film look with an editing timeline, a REC timecode and a glowing playhead",
    options: "text: the title; sub; tc; color; opacity (1)",
    cards: false,
    draw: timeline,
};
fn timeline(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let see = e.f("opacity", 1.0).clamp(0.0, 1.0);
    let k = e.ramp(c, 0.2, 0.0) * see;
    let al = e.fade(c, 0.2) * see;
    let mut target = c.img.clone();
    for i in 0..c.w * c.h {
        let p = c.px(i);
        let lum = p[0] * 0.299 + p[1] * 0.587 + p[2] * 0.114;
        let m = c.m[i];
        let bg = [lum * 1.05 * 0.85, lum * 0.3 * 0.85, lum * 0.27 * 0.85];
        let you = [p[0] * 1.04, p[1] * 0.86, p[2] * 0.82];
        for ch in 0..3 {
            target[i * 3 + ch] = you[ch].clamp(0.0, 1.0) * m + bg[ch].clamp(0.0, 1.0) * (1.0 - m);
        }
    }
    c.mix_with(&target, k);
    c.vignette(0.5 * k);
    let red = e.color("color", (255, 52, 44));
    let sl = (1.0 - ease_out(c.lt / 0.35)) * w * 0.08;
    let tb = clamp(c.lt / 0.15) * al;
    let secs = 21 + (c.lt * 3.0) as i32;
    let rec = text_img(&format!("{} 00:{secs:02}", e.s("tc", "REC")), w * 0.032, Kind::Mono, red, Look::default());
    let ds = (rec.height() as f32 * 0.36).floor();
    let mut dot = blank(ds, ds);
    canvas::ellipse(&mut dot, 0.0, 0.0, ds - 1.0, ds - 1.0, Some(op(red)), None);
    let rh = rec.height() as f32;
    c.comp(&dot, w * 0.065 - sl, h * 0.075 + (rh - ds) / 2.0, tb * if (c.lt * 2.5) as i32 % 2 == 0 { 1.0 } else { 0.35 }, false, true);
    c.comp(&rec, w * 0.065 + ds * 1.4 - sl, h * 0.075, tb, false, true);
    let ts = e.s("text", "AUTO-EDIT");
    let ti = e.cache("ti", || text_img(&ts, w * 0.13, Kind::Uib, WHITE, Look { shadow: 10.0, ..Look::default() }));
    c.comp(&ti, w * 0.055 - sl, h * 0.075 + rh * 0.85, tb, false, true);
    let ss = e.s("sub", "timeline by Forge");
    let su = e.cache("su", || text_img(&ss, w * 0.037, Kind::Uim, red, Look::default()));
    c.comp(&su, w * 0.065 - sl, h * 0.075 + rh * 0.85 + ti.height() as f32 * 0.88, tb, false, true);
    let (pw, phh) = ((w * 0.9).floor(), (h * 0.19).floor());
    let py = h * 0.755 + (1.0 - ease_out((c.lt - 0.1) / 0.35)) * h * 0.3;
    let mut lay = rrect(pw, phh, w * 0.03, (14, 10, 10, 215), Some(((255, 70, 60, 90), 2.0)));
    for j in 0..4 {
        text(&mut lay, pw * (0.05 + 0.3 * j as f32), phh * 0.12, &format!("00:0{j}"), Kind::Mono, w * 0.022, (170, 170, 175, 255), Anchor::LM);
    }
    let rows: [&[(f32, f32)]; 3] = [&[(0.04, 0.24), (0.26, 0.5), (0.53, 0.76), (0.79, 0.96)], &[(0.04, 0.3), (0.33, 0.45), (0.48, 0.7)], &[(0.04, 0.55), (0.6, 0.96)]];
    let p = 0.04 + 0.92 * clamp(c.lt / (e.d * 0.95));
    let ph_x = pw * p;
    let mut glow = blank(pw, phh);
    for (r, segs) in rows.iter().enumerate() {
        for &(a, b) in segs.iter() {
            let y0 = phh * (0.27 + 0.235 * r as f32);
            let (x0, x1) = (pw * a, pw * b);
            let on = x0 < ph_x;
            canvas::rounded(&mut lay, x0, y0, x1, y0 + phh * 0.17, phh * 0.04, Some(if on { op(red) } else { (120, 30, 28, 255) }), None);
            if on {
                canvas::rounded(&mut glow, x0, y0, x1, y0 + phh * 0.17, phh * 0.04, Some(with_a(red, 200)), None);
            }
        }
    }
    canvas::line_flat(&mut lay, &[(ph_x, phh * 0.2), (ph_x, phh * 0.97)], op(WHITE), (w / 300.0).max(3.0));
    canvas::line_flat(&mut glow, &[(ph_x, phh * 0.2), (ph_x, phh * 0.97)], op(WHITE), (w / 100.0).max(8.0));
    canvas::blur(&mut glow, (w / 90.0).floor());
    over(&mut glow, &lay, 0, 0);
    c.comp(&glow, (w - pw) / 2.0, py, al, false, true);
}

pub(crate) const HEADLINE_BOX: Effect = Effect {
    name: "headline-box",
    title: "Headline Box",
    desc: "A punch-in with a clean two-line headline, the second line in a white box",
    options: "text: lines joined with |; zoom (1.2); box: which line is boxed (1 to 3); color: the box; ink: its words",
    cards: false,
    draw: headline_box,
};
fn headline_box(c: &mut Ctx, e: &mut Run) {
    c.face_cam(1.0 + (e.f("zoom", 1.2) - 1.0) * ease_out(c.lt / 0.22), 0.45);
    let w = c.w as f32;
    let al = e.fade(c, 0.2);
    let lines = parts(&e.s("text", "How I edit my reels|in one take."));
    boxed_lines(c, e, &lines, (w * 0.085).floor(), true, al);
}

/// Lines stacked above the head, the last in a white box (`headline-box`,
/// `phone-fan`).
fn boxed_lines(c: &mut Ctx, e: &mut Run, lines: &[String], start: f32, grow: bool, al: f32) {
    let w = c.w as f32;
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let s = *e.cache("sz", || lines_fit(&refs, w * 0.84, Kind::Uib, start));
    let n = lines.len();
    let mut y = (c.h as f32 * 0.06).max(c.fy - c.fw * 1.3 - s * 1.3 * n as f32);
    for (j, ln) in lines.iter().enumerate() {
        let kj = clamp((c.lt - if grow { 0.1 + 0.12 * j as f32 } else { 0.05 + 0.1 * j as f32 }) / 0.18);
        if kj <= 0.0 {
            y += s * 1.3;
            continue;
        }
        // the line in the box: the last unless one is chosen
        let boxed = j + 1 == (e.f("box", n as f32).round() as usize).clamp(1, n);
        let (paper, ink) = (e.color("color", WHITE), e.color("ink", (16, 16, 18)));
        let t = e.cache(&format!("t{j}"), || text_img(ln, s, Kind::Uib, if boxed { ink } else { WHITE }, Look { shadow: if boxed { 0.0 } else { 9.0 }, ..Look::default() }));
        let tw = t.width() as f32;
        if boxed {
            let bx = rrect(tw - s * 0.15, s * 1.22, s * 0.08, op(paper), None);
            let bx = if grow {
                let wk = ease_out(kj);
                canvas::crop(&bx, 0.0, 0.0, (bx.width() as f32 * wk).max(1.0), bx.height() as f32)
            } else {
                bx
            };
            c.comp(&bx, w / 2.0 - (tw - s * 0.15) / 2.0, y, if grow { al } else { al * kj }, false, true);
        }
        let sc = if grow { 0.92 + 0.08 * ease_out(kj) } else { 1.0 };
        c.place(&t, w / 2.0, y + s * 0.6, sc, 0.0, al * kj, false, true);
        y += s * 1.3;
    }
}

pub(crate) const PROFILE_WALL: Effect = Effect {
    name: "profile-wall",
    title: "Profile Wall",
    desc: "Your reels grid fills the wall behind you while a counter ticks up",
    options: "images: your own pictures; count (197000); label; name; sub",
    cards: true,
    draw: profile_wall,
};
fn profile_wall(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let al = e.fade(c, 0.2);
    let wl = e.cache_with("wall", |e| {
        let pw = (w * 0.92).floor();
        let ph = (h * 0.36).floor();
        let mut im = rrect(pw, ph, w * 0.035, (16, 16, 18, 240), None);
        for (j, tb) in ["Posts", "Videos", "Saved"].iter().enumerate() {
            let x = pw * (0.2 + 0.3 * j as f32);
            text(&mut im, x, ph * 0.06, tb, Kind::Uim, w * 0.032, if j == 1 { op(WHITE) } else { (130, 130, 136, 255) }, Anchor::MM);
            if j == 1 {
                canvas::rect(&mut im, x - pw * 0.08, ph * 0.1, x + pw * 0.08, ph * 0.1 + 4.0, op(WHITE));
            }
        }
        let gr = thumb_grid(&e.cards, 4, 2, (pw * 0.225).floor(), (pw * 0.012).floor(), 0, false);
        over(&mut im, &gr, ((pw - gr.width() as f32) / 2.0) as i32, (ph * 0.14) as i32);
        shadowed(&im, 20.0, 10.0, 110)
    });
    let wh = wl.height() as f32;
    c.place(&wl, w / 2.0, h * 0.04 + wh / 2.0, 0.94 + 0.06 * ease_out(c.lt / 0.35), 0.0, al * clamp(c.lt / 0.15), true, false);
    let (cw, chh) = (w * 0.76, w * 0.25);
    let cy = (h * 0.86).min(c.fy + c.fw * 2.6);
    let mut g = c.glass(cw, chh, chh * 0.18, w / 2.0, cy, 0.5, (12, 12, 14), None, 240.0, 0.0, true);
    let target = e.f("count", 197_000.0);
    let n = (target * ease_out((c.lt - 0.2) / 0.8)) as i64;
    let swap = clamp((c.lt - 1.15) / 0.2);
    if swap < 1.0 {
        text(&mut g, cw / 2.0, chh * 0.33, &thousands(n), Kind::Uib, chh * 0.3, with_a(WHITE, (255.0 * (1.0 - swap)) as u8), Anchor::MM);
    }
    if swap > 0.0 {
        text(&mut g, cw / 2.0, chh * 0.33, &e.s("label", "views"), Kind::Uib, chh * 0.3, with_a(WHITE, (255.0 * swap) as u8), Anchor::MM);
    }
    let av = face_chip(c, e, chh * 0.3);
    over(&mut g, &av, (chh * 0.14) as i32, (chh * 0.58) as i32);
    text(&mut g, chh * 0.52, chh * 0.68, &e.s("name", "your.handle"), Kind::Uib, chh * 0.09, op(WHITE), Anchor::LM);
    text(&mut g, chh * 0.52, chh * 0.8, &e.s("sub", "top reel"), Kind::Uim, chh * 0.075, (170, 170, 176, 255), Anchor::LM);
    let on = c.lt >= e.d * 0.7;
    canvas::rounded(&mut g, cw - chh * 0.95, chh * 0.62, cw - chh * 0.14, chh * 0.86, chh * 0.12, Some(if on { (60, 60, 66, 255) } else { op(WHITE) }), None);
    text(&mut g, cw - chh * 0.545, chh * 0.74, if on { "Following" } else { "Follow" }, Kind::Uib, chh * 0.08, if on { op(WHITE) } else { (16, 16, 18, 255) }, Anchor::MM);
    c.place(&g, w / 2.0, cy + (1.0 - ease_out((c.lt - 0.05) / 0.3)) * h * 0.1, 1.0, 0.0, al * clamp((c.lt - 0.05) / 0.12), false, true);
}

/// `n` with commas between its thousands.
fn thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub(crate) const TIER_LIST: Effect = Effect {
    name: "tier-list",
    title: "Tier List",
    desc: "A tier list drops in and your pick flies into S tier",
    options: "title; item: what flies in; others: what fills B, C, D and F, joined with |",
    cards: false,
    draw: tier_list,
};
fn tier_list(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let al = e.fade(c, 0.2);
    let title = e.s("title", "best way to edit a reel?");
    let ttl = e.cache("ttl", || fit_text(&title, w * 0.88, Kind::Serif, (w * 0.11).floor(), YELLOW, Look { shadow: 8.0, ..Look::default() }));
    c.place(&ttl, w / 2.0, (ttl.height() as f32 * 0.6).max(c.fy - c.fw * 1.65), 0.9 + 0.1 * ease_out(c.lt / 0.3), 0.0, al * clamp(c.lt / 0.15), true, false);
    let tiers = [("S", (255, 127, 127)), ("A", (255, 191, 127)), ("B", (255, 223, 127)), ("C", (191, 255, 127)), ("D", (127, 191, 255)), ("F", (191, 127, 255))];
    let cw = (w * 0.66).floor();
    let rh = (w * 0.082).floor();
    let pad = (w * 0.015).floor();
    let chh = pad * 2.0 + rh * 6.0 + pad * 5.0;
    let others = parts(&e.s("others", "templates|timelines|apps|doing it by hand"));
    let bd = e.cache("bd", || {
        let mut im = rrect(cw, chh, w * 0.025, (22, 22, 26, 240), None);
        for (i, (lab, col)) in tiers.iter().enumerate() {
            let y = pad + i as f32 * (rh + pad);
            canvas::rounded(&mut im, pad, y, pad + rh, y + rh, rh * 0.14, Some(op(*col)), None);
            text(&mut im, pad + rh / 2.0, y + rh / 2.0, lab, Kind::Uib, rh * 0.5, (20, 20, 22, 255), Anchor::MM);
            canvas::rounded(&mut im, pad * 2.0 + rh, y, cw - pad, y + rh, rh * 0.14, Some((40, 40, 46, 255)), None);
            let slot = "BCDF".find(lab).and_then(|j| others.get(j));
            if let Some(fill) = slot {
                let tw = canvas::length(Kind::Uim, rh * 0.3, fill) + rh * 0.4;
                canvas::rounded(&mut im, pad * 3.0 + rh, y + rh * 0.15, pad * 3.0 + rh + tw, y + rh * 0.85, rh * 0.12, Some((78, 78, 86, 255)), None);
                text(&mut im, pad * 3.0 + rh + rh * 0.2, y + rh / 2.0, fill, Kind::Uim, rh * 0.3, (225, 225, 230, 255), Anchor::LM);
            }
        }
        shadowed(&im, 18.0, 10.0, 120)
    });
    let (bdw, bdh) = (bd.width() as f32, bd.height() as f32);
    let top = (h * 0.95 - bdh).min((c.fy + c.fw * 1.4).max(h * 0.45));
    let k = ease_back(c.lt / 0.35);
    let (bx, by) = (w / 2.0, top + bdh / 2.0 + (1.0 - k) * h * 0.15);
    c.place(&bd, bx, by, 1.0, 0.0, al * clamp(c.lt / 0.12), false, true);
    let item = e.s("item", "one take");
    let tl = e.cache("tl", || {
        let mut tile = shadowed(&rrect((rh * 2.6).floor(), (rh * 0.8).floor(), rh * 0.16, op(ORANGE), None), 8.0, 4.0, 100);
        let tx = text_img(&item, rh * 0.36, Kind::Uib, WHITE, Look::default());
        let (tw, th) = (tile.width() as i32, tile.height() as i32);
        over(&mut tile, &tx, (tw - tx.width() as i32) / 2, (th - tx.height() as i32) / 2 - 3);
        tile
    });
    let (sx, sy) = (w * 0.86, c.fy + c.fw * 0.4);
    let ex = bx - bdw / 2.0 + 18.0 + pad * 3.0 + rh + tl.width() as f32 / 2.0 - 10.0;
    let ey = by - bdh / 2.0 + 18.0 + pad + rh / 2.0;
    let q = ease_back((c.lt - 0.45) / 0.4);
    if c.lt > 0.4 {
        c.place(&tl, sx + (ex - sx) * q, sy + (ey - sy) * q, 1.25 - 0.25 * clamp(q), 18.0 * (1.0 - clamp(q)), al, false, true);
    }
}

pub(crate) const BROLL_SPLIT: Effect = Effect {
    name: "broll-split",
    title: "B-roll Split",
    desc: "B-roll on top with a price-tag sticker, you below",
    options: "images: your b-roll pictures; text: the tag; pick: which picture (0)",
    cards: true,
    draw: broll_split,
};
fn broll_split(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let k = ease_out(c.lt / 0.3) * clamp((e.d - c.lt) / 0.2);
    let al = e.fade(c, 0.2);
    c.shift((h * 0.72 - c.fy) * k);
    let sh = (h * 0.5 * k) as usize;
    if sh > 2 && !e.cards.is_empty() {
        let src = &e.cards[e.f("pick", 0.0) as usize % e.cards.len()];
        let z = 1.0 + 0.06 * clamp(c.lt / e.d);
        let (tw, th) = (w * z, h * 0.5 * z);
        let half = (h * 0.5).floor();
        let big = cover(src, tw, th, 0.35);
        let im = canvas::crop(&big, (big.width() as f32 - w) / 2.0, 0.0, w, half);
        let data = im.data();
        let skip = half as usize - sh;
        for y in 0..sh.min(c.h) {
            for x in 0..c.w {
                let s = ((skip + y) * c.w + x) * 4;
                let o = (y * c.w + x) * 3;
                for ch in 0..3 {
                    c.img[o + ch] = f32::from(data[s + ch]) / 255.0;
                }
                c.m[y * c.w + x] = 0.0;
            }
        }
        for y in sh.saturating_sub(3)..(sh + 3).min(c.h) {
            for x in 0..c.w {
                let o = (y * c.w + x) * 3;
                c.img[o..o + 3].copy_from_slice(&[1.0, 1.0, 1.0]);
            }
        }
    }
    let ts = e.s("text", "4 hrs  →  4 min");
    let st = e.cache("tag", || {
        let tag = text_img(&ts, w * 0.06, Kind::Uib, (16, 16, 18), Look::default());
        let (tw, th) = ((tag.width() as f32 + w * 0.11).floor(), (tag.height() as f32 * 1.25).floor());
        let mut st = blank(tw, th);
        let n = th * 0.5;
        canvas::polygon(&mut st, &[(0.0, th / 2.0), (n, 0.0), (tw, 0.0), (tw, th), (n, th)], Some(op(WHITE)), None);
        canvas::ellipse(&mut st, n * 0.55, th / 2.0 - n * 0.16, n * 0.87, th / 2.0 + n * 0.16, Some((200, 200, 205, 255)), None);
        over(&mut st, &tag, (n * 1.05) as i32, ((th - tag.height() as f32) / 2.0) as i32);
        shadowed(&st, 12.0, 6.0, 110)
    });
    c.place(&st, w * 0.66, h * 0.39 * k, 0.5 + 0.5 * pop(c, 0.3, 0.3), 8.0, al * clamp((c.lt - 0.3) / 0.08), false, false);
}

pub(crate) const PHONE_FAN: Effect = Effect {
    name: "phone-fan",
    title: "Phone Fan",
    desc: "Your reels fan out like a hand of phones under a headline",
    options: "images: your own pictures; text: lines joined with |",
    cards: true,
    draw: phone_fan,
};
fn phone_fan(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let al = e.fade(c, 0.2);
    let lines = parts(&e.s("text", "51 effects.|zero editing apps."));
    boxed_lines(c, e, &lines, (w * 0.08).floor(), false, al);
    let pw = (w * 0.23).floor();
    let ph = (pw * 2.0).floor();
    let phones = e.cache_with("ph", |e| {
        if e.cards.is_empty() {
            return Vec::new();
        }
        let own = e.has("images");
        (0..4)
            .map(|i| {
                let mut im = rrect(pw, ph, pw * 0.16, (14, 14, 16, 255), None);
                let src = &e.cards[i % e.cards.len()];
                let src = if own { src.clone() } else { super::vary(src, i) };
                let mut inner = cover(&src, pw - 10.0, ph - 10.0, 0.5);
                round_corners(&mut inner, (pw - 10.0) * 0.07);
                over(&mut im, &inner, 5, 5);
                canvas::rounded(&mut im, pw * 0.36, ph * 0.025, pw * 0.64, ph * 0.045, pw * 0.03, Some((0, 0, 0, 255)), None);
                shadowed(&im, 14.0, 8.0, 120)
            })
            .collect::<Vec<Pixmap>>()
    });
    if phones.is_empty() {
        return;
    }
    let cx = w / 2.0 * 0.5 + c.fx * 0.5;
    let cy = (c.fy + c.fw * 1.1 + ph / 2.0).max(h * 0.62).min(h - ph * 0.42);
    for (i, o) in [-1.5f32, -0.5, 0.5, 1.5].into_iter().enumerate() {
        let q = ease_back((c.lt - 0.2 - 0.06 * i as f32) / 0.4);
        c.place(&phones[i], cx + o * w * 0.19 * q, cy + o * o * w * 0.025 * q, 0.9, -o * 11.0 * q, al * clamp((c.lt - 0.2) / 0.1), false, true);
    }
}

pub(crate) const DOT_REVEAL: Effect = Effect {
    name: "dot-reveal",
    title: "Dot Reveal",
    desc: "Your shot builds out of glowing dots, then snaps into focus",
    options: "color; cell (0.017); bg",
    cards: false,
    draw: dot_reveal,
};
fn dot_reveal(c: &mut Ctx, e: &mut Run) {
    let p = clamp(c.lt / e.d);
    let (w, h) = (c.w, c.h);
    let cell = ((w as f32 * e.f("cell", 0.017)) as usize).max(6);
    let (gh, gw) = (h / cell + 1, w / cell + 1);
    let thr = e.cache("thr", || {
        let mut r = Rng::new(5);
        (0..gh * gw).map(|_| r.next()).collect::<Vec<f32>>()
    });
    // the frame, each cell its mean
    let mut small = vec![0f32; gh * gw * 3];
    let mut count = vec![0f32; gh * gw];
    for y in 0..h {
        for x in 0..w {
            let g = (y / cell) * gw + x / cell;
            for ch in 0..3 {
                small[g * 3 + ch] += c.img[(y * w + x) * 3 + ch];
            }
            count[g] += 1.0;
        }
    }
    for (g, n) in count.iter().enumerate() {
        for ch in 0..3 {
            small[g * 3 + ch] /= n.max(1.0);
        }
    }
    let col = e.color("color", (255, 128, 80));
    let col = [f32::from(col.0) / 255.0, f32::from(col.1) / 255.0, f32::from(col.2) / 255.0];
    let bgc = e.color("bg", (20, 20, 22));
    let bgc = [f32::from(bgc.0) / 255.0, f32::from(bgc.1) / 255.0, f32::from(bgc.2) / 255.0];
    let q2 = clamp((p - 0.3) / 0.35);
    let q = ease_inout((p - 0.6) / 0.4);
    for y in 0..h {
        for x in 0..w {
            let g = (y / cell) * gw + x / cell;
            let s = [small[g * 3], small[g * 3 + 1], small[g * 3 + 2]];
            let lum = s[0] * 0.299 + s[1] * 0.587 + s[2] * 0.114;
            let t = thr[g];
            let on = ((p * 1.7 - t * 0.95) / 0.3).clamp(0.0, 1.0);
            let sparkle = 0.75 + 0.25 * (t * 40.0 + c.lt * 18.0).sin();
            let r = (0.18 + 0.32 * lum.clamp(0.0, 1.0).sqrt()) * cell as f32 * on;
            let (dx, dy) = ((x % cell) as f32 - cell as f32 / 2.0 + 0.5, (y % cell) as f32 - cell as f32 / 2.0 + 0.5);
            let dm = (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            let o = (y * w + x) * 3;
            for ch in 0..3 {
                let dc = col[ch] * (0.55 + 0.6 * lum) * sparkle * (1.0 - q2) + (s[ch] * 1.15).clamp(0.0, 1.0) * q2;
                let dots = bgc[ch] * (1.0 - dm) + dc * dm;
                c.img[o + ch] = dots * (1.0 - q) + c.img[o + ch] * q;
            }
        }
    }
}

pub(crate) const HEADLINE_WALL: Effect = Effect {
    name: "headline-wall",
    title: "Headline Wall",
    desc: "Your headlines pile up behind your head, then a total pops in front",
    options: "text: headline::meta joined with |; total; label; handle; count (13)",
    cards: false,
    draw: headline_wall,
};
fn headline_wall(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let u = w / 1080.0;
    let al = e.fade(c, 0.2);
    let items: Vec<(String, String)> = parts(&e.s(
        "text",
        "Five ways to film yourself alone::197K views|The one-take reel that went viral::124K views|How I plan a week of posts in an hour::83K views|My phone video setup under fifty dollars::50K views|Cutting the dead air out of every take::40K views|Three hooks that stop the scroll::38K views|Lighting a room with one lamp::140K views",
    ))
    .into_iter()
    .map(|x| match x.split_once("::") {
        Some((a, b)) => (a.to_owned(), b.to_owned()),
        None => (x, String::new()),
    })
    .collect();
    let handle = e.s("handle", "@your.handle");
    let looks = [((255, 255, 255), (20, 20, 22)), ((22, 20, 24), (255, 255, 255)), ((246, 238, 226), (30, 24, 20)), ((255, 214, 10), (20, 20, 22))];
    let n = e.f("count", 13.0).max(1.0) as usize;
    let cards = e.cache("cards", || {
        (0..n)
            .map(|i| {
                let (title, meta) = &items[i % items.len()];
                let (bg, fg): (Rgb, Rgb) = looks[i % looks.len()];
                let cw = (w * 0.5).floor();
                let size = cw * 0.085;
                let mut lines: Vec<String> = Vec::new();
                let mut cur = String::new();
                for word in title.split(' ') {
                    let tst = format!("{cur} {word}").trim().to_owned();
                    if canvas::length(Kind::Uib, size, &tst) > cw * 0.84 && !cur.is_empty() {
                        lines.push(cur);
                        cur = word.to_owned();
                    } else {
                        cur = tst;
                    }
                }
                lines.push(cur);
                let lh = cw * 0.1;
                let chh = (cw * 0.1 + lh * lines.len() as f32 + cw * 0.16).floor();
                let mut im = rrect(cw, chh, cw * 0.04, op(bg), None);
                for (j, ln) in lines.iter().enumerate() {
                    text(&mut im, cw * 0.08, cw * 0.07 + j as f32 * lh, ln, Kind::Uib, size, op(fg), Anchor::LA);
                }
                let my = chh - cw * 0.085;
                let s = cw * 0.04;
                canvas::polygon(&mut im, &[(cw * 0.08, my - s / 2.0), (cw * 0.08, my + s / 2.0), (cw * 0.08 + s * 0.9, my)], Some((255, 48, 64, 255)), None);
                let meta_line = if meta.is_empty() { handle.clone() } else { format!("{handle}  ·  {meta}") };
                text(&mut im, cw * 0.08 + s * 1.5, my, &meta_line, Kind::Uim, cw * 0.045, with_a(fg, 170), Anchor::LM);
                shadowed(&im, 14.0, 8.0, 110)
            })
            .collect::<Vec<_>>()
    });
    let pos = e.cache("pos", || {
        (0..n)
            .map(|i| {
                let i = i as u64;
                (Rng::new(i * 7 + 1).uniform(0.2, 0.8), Rng::new(i * 7 + 2).uniform(0.07, 0.42), Rng::new(i * 7 + 3).uniform(-9.0, 9.0))
            })
            .collect::<Vec<_>>()
    });
    let gap = e.f("gap", 0.09);
    for i in 0..n {
        let t0 = i as f32 * gap;
        if c.lt < t0 {
            break;
        }
        let (x, y, r) = pos[i];
        let kk = ease_back((c.lt - t0) / 0.22);
        c.place(&cards[i], x * w, y * h, 0.55 + 0.45 * kk, r, al * clamp((c.lt - t0) / 0.06), true, false);
    }
    let t1 = n as f32 * gap + 0.1;
    if c.lt > t1 {
        let a = text_img(&e.s("total", "672K"), 96.0 * u, Kind::Anton, WHITE, Look::default());
        let b = text_img(&e.s("label", "views from 7 reels"), 34.0 * u, Kind::Uib, WHITE, Look::default());
        let pw = (a.width() as f32 + b.width() as f32 + 30.0 * u).floor();
        let ph = (a.height().max(b.height()) as f32 + 10.0 * u).floor();
        let mut pill = rrect(pw, ph, ph * 0.28, (217, 119, 87, 245), None);
        over(&mut pill, &a, (8.0 * u) as i32, ((ph - a.height() as f32) / 2.0) as i32);
        over(&mut pill, &b, (a.width() as f32 + 8.0 * u) as i32, ((ph - b.height() as f32) / 2.0) as i32);
        c.place(&shadowed(&pill, 18.0, 8.0, 110), w / 2.0, (c.fy + c.fw * 1.8).max(h * 0.6), 0.6 + 0.4 * pop(c, t1, 0.3), -3.0, al * clamp((c.lt - t1) / 0.08), false, true);
    }
}

pub(crate) const CHECKLIST: Effect = Effect {
    name: "checklist",
    title: "Checklist",
    desc: "A task list under your chin: each step spins, then ticks green",
    options: "text: step::meta joined with |; title",
    cards: false,
    draw: checklist,
};
fn checklist(c: &mut Ctx, e: &mut Run) {
    let (w, h) = (c.w as f32, c.h as f32);
    let u = w / 1080.0;
    let al = e.fade(c, 0.2);
    let rows: Vec<(String, String)> = parts(&e.s("text", "Found your face::every frame|Cut you out of the room::AI mask|Put the text behind you::depth|Graded the colour::warm|Rendered in 4K::2160p"))
        .into_iter()
        .map(|x| match x.split_once("::") {
            Some((a, b)) => (a.to_owned(), b.to_owned()),
            None => (x, String::new()),
        })
        .collect();
    let n = rows.len();
    let cw = w * 0.9;
    let rh = 104.0 * u;
    let hh = 120.0 * u;
    let chh = hh + rh * n as f32 + 26.0 * u;
    let y0 = (c.fy + c.fw * 1.35).max(h * 0.42).min(h * 0.93 - chh);
    let cy = y0 + chh / 2.0 + (1.0 - ease_out(c.lt / 0.35)) * h * 0.12;
    let span = (e.d - 0.55).max(0.4);
    let done_at: Vec<f32> = (0..n).map(|i| 0.3 + span * (i + 1) as f32 / n as f32).collect();
    let mut g = c.glass(cw, chh, 40.0 * u, w / 2.0, cy, 0.78, (14, 13, 16), Some(30.0 * u), 240.0, 0.0, true);
    canvas::ellipse(&mut g, 44.0 * u - 14.0 * u, hh / 2.0 - 14.0 * u, 44.0 * u + 14.0 * u, hh / 2.0 + 14.0 * u, Some(op(ORANGE)), None);
    text(&mut g, 80.0 * u, hh / 2.0, &e.s("title", "Editing your clip"), Kind::Uib, 42.0 * u, op(WHITE), Anchor::LM);
    let nd = done_at.iter().filter(|&&t| c.lt >= t).count();
    text(&mut g, cw - 36.0 * u, hh / 2.0, &format!("{nd} of {n}"), Kind::Uim, 28.0 * u, (170, 170, 176, 255), Anchor::RM);
    canvas::line_flat(&mut g, &[(30.0 * u, hh), (cw - 30.0 * u, hh)], (255, 255, 255, 30), (2.0 * u).max(1.0));
    for (i, (txt, meta)) in rows.iter().enumerate() {
        let y = hh + 13.0 * u + rh * i as f32 + rh / 2.0;
        let x = 56.0 * u;
        let r = 24.0 * u;
        let start = if i > 0 { done_at[i - 1] } else { 0.3 };
        let (fc, mc);
        if c.lt >= done_at[i] {
            let q = ease_back((c.lt - done_at[i]) / 0.25);
            let rr = r * (0.6 + 0.4 * q);
            canvas::ellipse(&mut g, x - rr, y - rr, x + rr, y + rr, Some((52, 199, 89, 255)), None);
            canvas::tick(&mut g, x - rr * 0.5, y - rr * 0.45, rr, op(WHITE), Some(5.0 * u));
            fc = op(WHITE);
            mc = (52, 199, 89, 255);
        } else if c.lt >= start {
            let a0 = (c.lt * 540.0) % 360.0;
            canvas::ellipse(&mut g, x - r, y - r, x + r, y + r, None, Some(((255, 255, 255, 50), 5.0 * u)));
            canvas::arc(&mut g, x - r, y - r, x + r, y + r, a0, a0 + 100.0, op(ORANGE), 5.0 * u);
            fc = op(WHITE);
            mc = (170, 170, 176, 255);
        } else {
            canvas::ellipse(&mut g, x - r, y - r, x + r, y + r, None, Some(((255, 255, 255, 60), 4.0 * u)));
            fc = (255, 255, 255, 110);
            mc = (170, 170, 176, 90);
        }
        text(&mut g, x + 48.0 * u, y, txt, Kind::Uib, 40.0 * u, fc, Anchor::LM);
        text(&mut g, cw - 36.0 * u, y, meta, Kind::Mono, 28.0 * u, mc, Anchor::RM);
    }
    c.place(&shadowed(&g, 24.0, 12.0, 120), w / 2.0, cy, 1.0, 0.0, al * clamp(c.lt / 0.12), false, true);
}

pub(crate) const LIQUID_GLASS_GRAPH: Effect = Effect {
    name: "liquid-glass-graph",
    title: "Liquid Glass Graph",
    desc: "A liquid glass card under your chin: a graph climbs while the number counts up",
    options: "text: the label; from; to (e.g. 197K, 59%); color; look: bento",
    cards: false,
    draw: liquid_glass_graph,
};
fn liquid_glass_graph(c: &mut Ctx, e: &mut Run) {
    if e.s("look", "") == "bento" {
        return super::pack3::glass_bento(c, e);
    }
    let q = ease_out(c.lt / 0.4);
    let al = e.fade(c, 0.2) * clamp(c.lt / 0.12);
    if al <= 0.0 {
        return;
    }
    let w = c.w as f32;
    let (cw, ch) = ((w * 0.74).floor(), (w * 0.46).floor());
    let sc = 0.88 + 0.12 * q;
    let cx = w / 2.0;
    let cy = (c.h as f32 - ch * 0.62).min(c.fy + c.fw * 2.7) + (1.0 - q) * w * 0.08;
    let (gw, gh) = ((cw * sc).floor(), (ch * sc).floor());
    let mut im = c.glass(gw, gh, gh * 0.12, cx, cy, 0.16, WHITE, Some((w / 30.0).floor().max(10.0)), 240.0, 0.12, true);
    // a highlight down from the top
    let mut hl = blank(gw, gh);
    for (y, row) in hl.data_mut().chunks_exact_mut(gw as usize * 4).enumerate() {
        let a = ((1.0 - y as f32 / (gh - 1.0).max(1.0) * 2.2).clamp(0.0, 1.0) * 46.0) as u8;
        for px in row.chunks_exact_mut(4) {
            px.copy_from_slice(&[a, a, a, a]);
        }
    }
    round_corners(&mut hl, gh * 0.12);
    over(&mut im, &hl, 0, 0);
    let mut lay = blank(gw, gh);
    let pad = gw * 0.07;
    let g = ease_inout((c.lt - 0.25) / (e.d * 0.72).max(0.3));
    text(&mut lay, pad, pad * 0.9, &e.s("text", "RETENTION").to_uppercase(), Kind::Mono, gw * 0.04, (255, 255, 255, 190), Anchor::LA);
    let (a, _, _) = num_parts(&e.s("from", "0"));
    let (b, suf, dp) = num_parts(&e.s("to", "59%"));
    let s = num(a + (b - a) * g, dp, &suf);
    text(&mut lay, gw - pad - canvas::length(Kind::Uib, gw * 0.12, &s), pad * 0.55, &s, Kind::Uib, gw * 0.12, op(WHITE), Anchor::LA);
    let (x0, x1, y0, y1) = (pad, gw - pad, gh - pad * 0.9, gh * 0.36);
    let ycurve = |u: f32| y0 - (y0 - y1) * u.powf(1.5) + (u * 13.0).sin() * gh * 0.018 * (1.0 - u);
    let n = 60;
    let pts: Vec<(f32, f32)> = (0..=(n as f32 * g) as usize).map(|j| (x0 + (x1 - x0) * j as f32 / n as f32, ycurve(j as f32 / n as f32))).collect();
    let green = e.color("color", (52, 211, 110));
    if pts.len() > 1 {
        graph_line(&mut lay, &pts, y0, x0, green, (gw * 0.011).floor().max(4.0));
    }
    for j in 1..4 {
        let yy = y0 - (y0 - y1) * j as f32 / 3.4;
        canvas::line_flat(&mut lay, &[(x0, yy), (x1, yy)], (255, 255, 255, 40), (gw * 0.002).max(1.0));
    }
    over(&mut im, &lay, 0, 0);
    let shadowed_im = shadowed(&im, (w / 70.0).floor().max(10.0), (w / 90.0).floor(), 90);
    c.put(&shadowed_im, cx, cy, al, false, true);
}

/// A climbing line on `lay`: the area under it, a glow, the line and a dot
/// at its head.
pub(crate) fn graph_line(lay: &mut Pixmap, pts: &[(f32, f32)], y0: f32, x0: f32, green: Rgb, lw: f32) {
    let mut area_pts = pts.to_vec();
    area_pts.push((pts[pts.len() - 1].0, y0));
    area_pts.push((x0, y0));
    canvas::polygon(lay, &area_pts, Some(with_a(green, 55)), None);
    let mut glow = blank(lay.width() as f32, lay.height() as f32);
    line(&mut glow, pts, op(green), lw * 3.0);
    canvas::blur(&mut glow, lw * 2.0);
    over(lay, &glow, 0, 0);
    line(lay, pts, op(green), lw);
    let (hx, hy) = pts[pts.len() - 1];
    let r = lw * 1.9;
    canvas::ellipse(lay, hx - r, hy - r, hx + r, hy + r, Some(op(WHITE)), Some((op(green), (lw / 2.0).max(2.0))));
}

pub(crate) const COMMENT_BOX: Effect = Effect {
    name: "comment-box",
    title: "Comment Box",
    desc: "A comments sheet above your head types your word, posts it, gets a heart and your reply",
    options: "text: the comment (EFFECTS); you: who comments; handle: who replies; reply; chip; look: bar for the plain comment bar",
    cards: false,
    draw: comment_box,
};
fn comment_box(c: &mut Ctx, e: &mut Run) {
    if e.s("look", "") == "bar" {
        return comment_bar(c, e);
    }
    let (w, h) = (c.w as f32, c.h as f32);
    let u = w / 1080.0;
    let al = e.fade(c, 0.2);
    let word = e.s("text", "EFFECTS");
    let handle = e.s("handle", "your.handle");
    let reply = e.s("reply", "Sent you the pack");
    let chip = e.s("chip", "51 effects");
    let me = e.s("you", "you");
    let (sw, sh) = (740.0 * u, 390.0 * u);
    let t_post = 0.3 + 0.06 * word.chars().count() as f32 + 0.1;
    let (t_heart, t_rep) = (t_post + 0.25, t_post + 0.4);
    let mut sheet = rrect(sw, sh, 30.0 * u, (38, 38, 40, 248), None);
    let white = (245, 245, 245, 255);
    let grey = (160, 160, 166, 255);
    canvas::rounded(&mut sheet, sw / 2.0 - 32.0 * u, 14.0 * u, sw / 2.0 + 32.0 * u, 20.0 * u, 3.0 * u, Some((110, 110, 116, 255)), None);
    text(&mut sheet, sw / 2.0, 50.0 * u, "Comments", Kind::Uib, 26.0 * u, white, Anchor::MM);
    canvas::line_flat(&mut sheet, &[(0.0, 80.0 * u), (sw, 80.0 * u)], (60, 60, 64, 255), (2.0 * u).max(1.0));
    let avatar = |sz: f32| {
        let mut a = blank(sz, sz);
        canvas::ellipse(&mut a, 0.0, 0.0, sz - 1.0, sz - 1.0, Some(op(PURPLE)), None);
        text(&mut a, sz / 2.0, sz / 2.0, &me.chars().next().unwrap_or('Y').to_uppercase().to_string(), Kind::Uib, sz * 0.5, op(WHITE), Anchor::MM);
        a
    };
    let a8 = |v: (u8, u8, u8, u8), a: u8| (v.0, v.1, v.2, a);
    if c.lt < t_post {
        text(&mut sheet, sw / 2.0, 190.0 * u, "Start the conversation.", Kind::Uim, 21.0 * u, grey, Anchor::MM);
    } else {
        let q = ease_out((c.lt - t_post) / 0.25);
        let oy = (1.0 - q) * -20.0 * u;
        let a = (255.0 * clamp(q * 1.5)) as u8;
        canvas::over_a(&mut sheet, &avatar(46.0 * u), (26.0 * u) as i32, (100.0 * u + oy) as i32, f32::from(a) / 255.0);
        text(&mut sheet, 88.0 * u, 108.0 * u + oy, &me, Kind::Uib, 19.0 * u, a8(white, a), Anchor::LM);
        text(&mut sheet, 88.0 * u + canvas::length(Kind::Uib, 19.0 * u, &format!("{me} ")) + 4.0 * u, 108.0 * u + oy, "now", Kind::Uim, 18.0 * u, a8(grey, a), Anchor::LM);
        text(&mut sheet, 88.0 * u, 134.0 * u + oy, &word, Kind::Uim, 26.0 * u, a8(white, a), Anchor::LA);
        text(&mut sheet, 88.0 * u, 196.0 * u + oy, "Reply", Kind::Uim, 17.0 * u, a8(grey, a), Anchor::LM);
        let hk = ease_back((c.lt - t_heart) / 0.3);
        if hk > 0.0 {
            let hh = canvas::heart(30.0 * u * hk, (255, 48, 64));
            over(&mut sheet, &hh, (sw - 46.0 * u - hh.width() as f32 / 2.0) as i32, (130.0 * u - hh.height() as f32 / 2.0) as i32);
        }
        let rk = ease_out((c.lt - t_rep) / 0.25);
        if rk > 0.0 {
            let ra = (255.0 * clamp(rk * 1.5)) as u8;
            let (rx, ry) = (88.0 * u, 226.0 * u + (1.0 - rk) * 16.0 * u);
            let fc = face_chip(c, e, 34.0 * u);
            canvas::over_a(&mut sheet, &fc, rx as i32, ry as i32, f32::from(ra) / 255.0);
            text(&mut sheet, rx + 46.0 * u, ry + 8.0 * u, &handle, Kind::Uib, 18.0 * u, a8(white, ra), Anchor::LM);
            text(&mut sheet, rx + 46.0 * u + canvas::length(Kind::Uib, 18.0 * u, &handle) + 10.0 * u, ry + 8.0 * u, "Author", Kind::Uim, 16.0 * u, a8(grey, ra), Anchor::LM);
            text(&mut sheet, rx + 46.0 * u, ry + 36.0 * u, &reply, Kind::Uim, 22.0 * u, a8(white, ra), Anchor::LM);
            let ck = ease_back((c.lt - t_rep - 0.12) / 0.3);
            if ck > 0.0 {
                let ct = text_img(&chip, 19.0 * u, Kind::Uib, (20, 20, 22), Look::default());
                let mut cp = rrect(ct.width() as f32 + 10.0 * u, ct.height() as f32 * 0.9, ct.height() as f32 * 0.45, op(YELLOW), None);
                let dy = ((cp.height() as f32 - ct.height() as f32) / 2.0) as i32;
                over(&mut cp, &ct, (6.0 * u) as i32, dy);
                let cp = canvas::resized(&cp, cp.width() as f32 * (0.6 + 0.4 * ck), cp.height() as f32 * (0.6 + 0.4 * ck));
                over(&mut sheet, &cp, (rx + 46.0 * u) as i32, (ry + 56.0 * u) as i32);
            }
        }
    }
    let iy = sh - 78.0 * u;
    over(&mut sheet, &avatar(48.0 * u), (22.0 * u) as i32, iy as i32);
    canvas::rounded(&mut sheet, 82.0 * u, iy, sw - 22.0 * u, iy + 48.0 * u, 24.0 * u, Some((54, 54, 58, 255)), None);
    let n = word.chars().count().max(1);
    let typed: String = if c.lt < t_post { word.chars().take((clamp((c.lt - 0.3) / (0.06 * n as f32)) * n as f32 + 1e-3) as usize).collect() } else { String::new() };
    if !typed.is_empty() {
        text(&mut sheet, 104.0 * u, iy + 24.0 * u, &typed, Kind::Uim, 22.0 * u, white, Anchor::LM);
        if (c.lt * 4.0) as i32 % 2 == 1 {
            let tl = canvas::length(Kind::Uim, 22.0 * u, &typed);
            canvas::rect(&mut sheet, 104.0 * u + tl + 3.0 * u, iy + 10.0 * u, 106.0 * u + tl + 3.0 * u, iy + 38.0 * u, op(BLUE));
        }
    } else {
        text(&mut sheet, 104.0 * u, iy + 24.0 * u, &format!("Add a comment for {handle}"), Kind::Uim, 20.0 * u, grey, Anchor::LM);
    }
    let pressed = c.lt >= t_post - 0.08 && c.lt < t_post;
    let post_col = if pressed { (140, 200, 255, 255) } else if typed.is_empty() { (70, 110, 160, 255) } else { op(BLUE) };
    text(&mut sheet, sw - 44.0 * u, iy + 24.0 * u, "Post", Kind::Uib, 23.0 * u, post_col, Anchor::RM);
    let sheet = shadowed(&sheet, 22.0, 10.0, 120);
    let shh = sheet.height() as f32;
    let bottom = c.fy - c.fw * 0.5;
    let top = bottom - shh;
    let s = if top < h * 0.02 { ((bottom - h * 0.02) / shh).max(0.6) } else { 1.0 };
    let cy = bottom - shh * s / 2.0 + 3.0 * u * (c.lt * 3.2).sin();
    let x = w / 2.0 * 0.5 + c.fx * 0.5;
    c.place(&sheet, x, cy, s * (0.85 + 0.15 * ease_back(c.lt / 0.3)), 1.2, al * clamp(c.lt / 0.1), true, false);
}

/// The first comment box: a bar along the bottom that types and posts.
fn comment_bar(c: &mut Ctx, e: &mut Run) {
    let word = e.s("text", "EFFECTS");
    let (w, h) = (c.w as f32, c.h as f32);
    let (bw, bh) = ((w * 0.9).floor(), (w * 0.11).floor());
    let slide = ease_out(c.lt / 0.3);
    let y = h * 0.8 + (1.0 - slide) * h * 0.25;
    let n = word.chars().count().max(1) as f32;
    let (type_t, post_t) = (0.35, 0.35 + 0.08 * n + 0.25);
    let typed: String = if c.lt < post_t { word.chars().take((clamp((c.lt - type_t) / (0.08 * n)) * n + 0.001) as usize).collect() } else { String::new() };
    let mut im = rrect(bw, bh, bh / 2.0, (255, 255, 255, 245), None);
    let r = bh * 0.32;
    canvas::ellipse(&mut im, bh * 0.2, bh / 2.0 - r, bh * 0.2 + 2.0 * r, bh / 2.0 + r, Some(op(PURPLE)), None);
    if !typed.is_empty() {
        let cursor = if (c.lt * 4.0) as i32 % 2 == 1 { "|" } else { "" };
        text(&mut im, bh * 0.95, bh / 2.0, &format!("{typed}{cursor}"), Kind::Ui, bh * 0.3, op(crate::INK), Anchor::LM);
    } else if c.lt < post_t {
        text(&mut im, bh * 0.95, bh / 2.0, "Add a comment...", Kind::Ui, bh * 0.3, (150, 150, 150, 255), Anchor::LM);
    }
    text(&mut im, bw - bh * 0.4, bh / 2.0, "Post", Kind::Ui, bh * 0.3, if typed.is_empty() { (180, 200, 230, 255) } else { op(BLUE) }, Anchor::RM);
    let al = clamp((e.d - c.lt) / 0.2);
    c.put(&shadowed(&im, 14.0, 6.0, 100), w / 2.0, y, al, false, true);
    if c.lt >= post_t {
        let k = ease_back((c.lt - post_t) / 0.3);
        let mut b = rrect((w * 0.52).floor(), (bh * 0.85).floor(), bh * 0.3, (255, 255, 255, 240), None);
        let bhh = b.height() as f32;
        let bww = b.width() as f32;
        text(&mut b, bh * 0.3, bhh * 0.32, "you", Kind::Ui, bh * 0.2, (120, 120, 120, 255), Anchor::LM);
        text(&mut b, bh * 0.3, bhh * 0.66, &word, Kind::Ui, bh * 0.3, op(crate::INK), Anchor::LM);
        let hk = ease_back((c.lt - post_t - 0.3) / 0.3);
        if hk > 0.0 {
            let hh = canvas::heart(bh * 0.4 * hk, (255, 48, 64));
            over(&mut b, &hh, (bww - bh * 0.3 - hh.width() as f32) as i32, (bhh / 2.0 - hh.height() as f32 / 2.0) as i32);
        }
        c.place(&shadowed(&b, 12.0, 5.0, 90), w * 0.07 + bww / 2.0, y - bh * 1.05, 0.7 + 0.3 * k, 0.0, al * clamp((c.lt - post_t) / 0.1), false, true);
    }
}
