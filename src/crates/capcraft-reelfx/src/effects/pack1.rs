// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors
// SPDX-FileCopyrightText: 2026 Vibha Ramprakash (claude-video-effects, MIT)

//! Effects 1 to 22 of `fx.py` (its first pack), less the comment box, which
//! is drawn by its later sheet in [`super::pack2`].

use std::f32::consts::PI;

use tiny_skia::Pixmap;

use super::{Rng, cards, parts};
use crate::canvas::{self, Anchor, Kind, Look, blank, fit_text, line, op, over, rrect, shadowed, text, text_img, with_a};
use crate::ctx::{Ctx, Grade, clamp, ease_back, ease_inout, ease_out};
use crate::{CREAM, Effect, INK, PEACH, Run, YELLOW};

const NAMES: &str = "The Noise|The Lineup|Locked On|Orbit|Lean-In|Backdrop Word|Spotlight|The Bill|Moody|Swipe|The To-Do|Comment Box|The Tap|Pause & Point|Uninstall|Reel Frame|Shortcut Rain|Reject Stamp|Countdown Flip|Rabbit Hole|Cover Star|Scribble";

pub(crate) const NOISE: Effect = Effect {
    name: "noise",
    title: "The Noise",
    desc: "Reels pile up behind your head, one every quarter second",
    options: "images: your own pictures; size: card width (0.34)",
    cards: true,
    draw: noise,
};
fn noise(c: &mut Ctx, e: &mut Run) {
    let cw = (c.w as f32 * e.f("size", 0.34)).floor();
    let d = e.d;
    let pos = e.cache("pos", || {
        let mut r = Rng::new(7);
        let n = ((d / 0.22) as usize).clamp(4, 16);
        (0..n).map(|_| (r.uniform(0.14, 0.86), r.uniform(0.12, 0.52), r.uniform(-7.0, 7.0))).collect::<Vec<_>>()
    });
    let cs = e.cache_with("cards", |e| cards(e, cw));
    if cs.is_empty() {
        return;
    }
    let out = clamp((e.d - c.lt) / 0.2);
    for (i, &(x, y, r)) in pos.iter().enumerate() {
        let t0 = i as f32 * 0.22;
        if c.lt < t0 {
            break;
        }
        let k = ease_back((c.lt - t0) / 0.25);
        let (w, h) = (c.w as f32, c.h as f32);
        c.place(&cs[i % cs.len()], x * w, y * h, 0.6 + 0.4 * k, r, clamp((c.lt - t0) / 0.08) * out, true, false);
    }
}

pub(crate) const LINEUP: Effect = Effect {
    name: "lineup",
    title: "The Lineup",
    desc: "Numbered tiles fan out in an arc above your head",
    options: "text: tile names joined with |",
    cards: false,
    draw: lineup,
};
fn lineup(c: &mut Ctx, e: &mut Run) {
    let names = parts(&e.s("text", NAMES));
    let tw = (c.w as f32 * 0.105).floor();
    let tiles = e.cache("tiles", || {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut im = rrect(tw, (tw * 1.25).floor(), tw * 0.16, (24, 24, 28, 225), Some(((255, 255, 255, 60), 2.0)));
                let num = text_img(&format!("{:02}", i + 1), tw * 0.42, Kind::Serif, CREAM, Look::default());
                over(&mut im, &num, ((tw - num.width() as f32) / 2.0) as i32, (tw * 0.04) as i32);
                let words: Vec<&str> = name.split(' ').collect();
                let half = (words.len() / 2).max(1);
                let lines: Vec<String> = if name.len() > 10 {
                    vec![words[..half].join(" "), words[half..].join(" ")]
                } else {
                    vec![name.clone()]
                };
                for (j, ln) in lines.iter().filter(|l| !l.is_empty()).enumerate() {
                    text(&mut im, tw / 2.0, tw * (0.78 + 0.19 * j as f32), ln, Kind::Ui, tw * 0.15, (255, 255, 255, 235), Anchor::MM);
                }
                shadowed(&im, 8.0, 5.0, 90)
            })
            .collect::<Vec<_>>()
    });
    let n = tiles.len();
    let outer = (n as f32 / 2.0 + 0.5).ceil() as usize;
    let inner = n.saturating_sub(outer);
    let (cx, cy) = (c.w as f32 / 2.0 * 0.6 + c.fx * 0.4, c.fy + c.fw * 0.3);
    let mut slots = Vec::new();
    for (cnt, rad) in [(outer, c.w as f32 * 0.47), (inner, c.w as f32 * 0.33)] {
        for k in 0..cnt {
            let a = PI * (0.94 - 0.88 * k as f32 / (cnt.max(2) - 1) as f32);
            slots.push((cx + rad * a.cos(), cy - rad * a.sin() * 1.05, (a - PI / 2.0).to_degrees() * 0.3));
        }
    }
    let out = clamp((e.d - c.lt) / 0.2);
    for (i, &(x, y, r)) in slots.iter().take(n).enumerate() {
        let t0 = 0.035 * i as f32;
        if c.lt < t0 {
            continue;
        }
        let k = ease_back((c.lt - t0) / 0.3);
        c.place(&tiles[i], x, y + (1.0 - k) * 40.0, 0.5 + 0.5 * k, r, clamp((c.lt - t0) / 0.1) * out, true, false);
    }
}

pub(crate) const LOCKED_ON: Effect = Effect {
    name: "locked-on",
    title: "Locked On",
    desc: "Corner brackets snap onto your face, the camera follows you",
    options: "zoom: how close (1.35); text: the tag (LOCKED ON)",
    cards: false,
    draw: locked_on,
};
fn locked_on(c: &mut Ctx, e: &mut Run) {
    c.face_cam(1.0 + (e.f("zoom", 1.35) - 1.0) * ease_out(c.lt / 0.6), 0.40);
    let snap = ease_out(c.lt / 0.22);
    let s = c.fw * 0.85 * (1.0 + 0.6 * (1.0 - snap));
    let al = clamp(c.lt / 0.12) * clamp((e.d - c.lt) / 0.2);
    let size = (2.0 * s + 40.0).floor();
    let mut im = blank(size, (size * 1.25).floor());
    let (x0, y0, x1, y1) = (20.0, 20.0, size - 20.0, im.height() as f32 - 20.0);
    let l = (s * 0.32).floor();
    let wd = (c.w as f32 / 200.0).max(4.0).floor();
    for (px, py, dx, dy) in [(x0, y0, 1.0, 1.0), (x1, y0, -1.0, 1.0), (x0, y1, 1.0, -1.0), (x1, y1, -1.0, -1.0)] {
        line(&mut im, &[(px, py + dy * l), (px, py), (px + dx * l, py)], (255, 255, 255, 255), wd);
    }
    let (fx, fy) = (c.fx, c.fy);
    c.place(&shadowed(&im, 6.0, 2.0, 120), fx, fy, 1.0, 0.0, al, false, false);
    let tag = text_img(&e.s("text", "LOCKED ON").to_uppercase(), c.w as f32 * 0.022, Kind::Mono, INK, Look::default());
    let mut pill = rrect(tag.width() as f32 + 10.0, tag.height() as f32, tag.height() as f32 / 2.0, (255, 255, 255, 235), None);
    over(&mut pill, &tag, 5, 0);
    let ph = pill.height() as f32;
    c.place(&pill, fx, fy + s * 1.25 + ph, 1.0, 0.0, clamp((c.lt - 0.15) / 0.15) * al, false, true);
}

pub(crate) const ORBIT: Effect = Effect {
    name: "orbit",
    title: "Orbit",
    desc: "Your reels circle your head: in front at the bottom, behind you at the top",
    options: "images: your own pictures; count (6); speed (1.9); size (0.2)",
    cards: true,
    draw: orbit,
};
fn orbit(c: &mut Ctx, e: &mut Run) {
    let k_n = e.f("count", 6.0).max(1.0) as usize;
    let cw = (c.w as f32 * e.f("size", 0.2)).floor();
    let cs = e.cache_with("cards", |e| cards(e, cw));
    if cs.is_empty() {
        return;
    }
    let (intro, out) = (ease_out(c.lt / 0.4), clamp((e.d - c.lt) / 0.25));
    let (rx, ry) = (c.w as f32 * 0.38 * intro, (c.fw * 1.5).max(c.h as f32 * 0.1));
    let oy = c.fy + c.fw * 0.9;
    let speed = e.f("speed", 1.9);
    let mut items: Vec<(f32, f32, usize)> = (0..k_n)
        .map(|i| {
            let th = i as f32 * 2.0 * PI / k_n as f32 + c.lt * speed;
            (th.sin(), th, i)
        })
        .collect();
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let fx = c.fx;
    for (depth, th, i) in items {
        c.place(
            &cs[i % cs.len()],
            fx + rx * th.cos(),
            oy + ry * th.sin(),
            (0.78 + 0.24 * (depth + 1.0) / 2.0) * (0.5 + 0.5 * intro),
            -6.0 * th.cos(),
            out * clamp(c.lt / 0.15),
            depth < 0.0,
            depth >= 0.0,
        );
    }
}

pub(crate) const LEAN_IN: Effect = Effect {
    name: "lean-in",
    title: "Lean-In",
    desc: "A fast punch-in on one word",
    options: "zoom (1.45, at most 2); speed (0.18)",
    cards: false,
    draw: lean_in,
};
fn lean_in(c: &mut Ctx, e: &mut Run) {
    let z = e.f("zoom", 1.45);
    c.face_cam(1.0 + (z - 1.0) * ease_out(c.lt / e.f("speed", 0.18)), 0.42);
}

pub(crate) const BACKDROP_WORD: Effect = Effect {
    name: "backdrop-word",
    title: "Backdrop Word",
    desc: "A giant word drops in behind you",
    options: "text; color (#rrggbb); font (bold or serif)",
    cards: false,
    draw: backdrop_word,
};
fn backdrop_word(c: &mut Ctx, e: &mut Run) {
    let word = e.s("text", "FORGE");
    let kind = Kind::named(&e.s("font", "bold"));
    let col = e.color("color", CREAM);
    let w = c.w as f32;
    let im = e.cache("w", || fit_text(&word, w * 0.94, kind, 400.0, col, Look { shadow: 10.0, ..Look::default() }));
    let k = ease_back(c.lt / 0.42);
    let y_end = c.fy - c.fw * 1.05;
    let ih = im.height() as f32;
    let y = -ih + (y_end + ih) * k;
    let x = w / 2.0 * 0.6 + c.fx * 0.4;
    c.place(&im, x, y, 1.0, 0.0, clamp((e.d - c.lt) / 0.2), true, false);
}

pub(crate) const SPOTLIGHT: Effect = Effect {
    name: "spotlight",
    title: "Spotlight",
    desc: "Kills the lights, keeps you lit",
    options: "text: a glowing word behind you; color; push (1.1); look: sparkle for golden rays",
    cards: false,
    draw: spotlight,
};
fn spotlight(c: &mut Ctx, e: &mut Run) {
    if e.s("look", "") == "sparkle" {
        return super::pack3::spotlight_sparkle(c, e);
    }
    c.face_cam(1.0 + (e.f("push", 1.1) - 1.0) * ease_inout(c.lt / e.d), 0.40);
    let k = e.ramp(c, 0.3, 0.25);
    let (w, h) = (c.w as f32, c.h as f32);
    let (fx, fy) = (c.fx, c.fy + h * 0.2);
    for y in 0..c.h {
        for x in 0..c.w {
            let i = y * c.w + x;
            let d = ((x as f32 - fx) / (w * 0.5)).powi(2) + ((y as f32 - fy) / (h * 0.46)).powi(2);
            let pool = (1.0 - d).clamp(0.0, 1.0).powf(1.6);
            let bright = 1.0 - k * (1.0 - (0.2 + 0.45 * pool));
            let m = c.m[i];
            let f = bright * (1.0 - m) + m * (1.0 + 0.04 * k);
            for ch in 0..3 {
                c.img[i * 3 + ch] *= f;
            }
        }
    }
    if e.has("text") {
        let word = e.s("text", "");
        let col = e.color("color", PEACH);
        let kind = Kind::named(&e.s("font", "serif"));
        let im = e.cache("w", || fit_text(&word, w * 0.9, kind, 400.0, col, Look { glow: Some((col, 26.0)), ..Look::default() }));
        let (fx, fy, fw) = (c.fx, c.fy, c.fw);
        let _ = fx;
        c.place(&im, w / 2.0, fy - fw * 1.1, 1.0, 0.0, k, true, false);
    }
}

pub(crate) const THE_BILL: Effect = Effect {
    name: "the-bill",
    title: "The Bill",
    desc: "Prints your instructions like a receipt into the wall above your head",
    options: "text: lines joined with |",
    cards: false,
    draw: the_bill,
};
fn the_bill(c: &mut Ctx, e: &mut Run) {
    let lines = parts(&e.s("text", "ORDER #1021|1x orbit|1x spotlight|1x lean-in|cut the dead air|TOTAL: 1 take"));
    let w = c.w as f32;
    let rec = e.cache("r", || {
        let pw = (w * 0.5).floor();
        let lh = (pw * 0.085).floor();
        let pad = (pw * 0.08).floor();
        let h = pad * 2.0 + lh * (lines.len() + 1) as f32;
        let mut im = rrect(pw, h, 0.0, op(CREAM), None);
        let n = lines.len();
        for (i, ln) in lines.iter().enumerate() {
            let last = i == n - 1;
            let y = pad + lh * i as f32 + if last { lh } else { 0.0 };
            if last {
                let mut x = pad;
                while x < pw - pad {
                    canvas::line_flat(&mut im, &[(x, y - lh * 0.45), (x + 7.0, y - lh * 0.45)], with_a(INK, 140), 2.0);
                    x += 14.0;
                }
            }
            text(&mut im, pad, y, ln, Kind::Mono, lh * 0.62, op(INK), Anchor::LA);
        }
        im
    });
    let (slot_y, sw) = (c.h as f32 * 0.075, w * 0.62);
    let max_h = (c.fy - c.fw * 1.3 - slot_y).max(80.0);
    let s = (max_h / rec.height() as f32).min(1.0);
    let pr = ease_inout(c.lt / (e.d * 0.75));
    let l = (rec.height() as f32 * pr).floor();
    let al = clamp((e.d - c.lt) / 0.25);
    let cx = w / 2.0;
    if l > 2.0 {
        let rw = rec.width() as f32;
        let z = 14.0;
        let mut jag = blank(rw, l + z);
        over(&mut jag, &canvas::crop(&rec, 0.0, 0.0, rw, l), 0, 0);
        let mut pts: Vec<(f32, f32)> = Vec::new();
        let mut x = 0.0;
        while x <= rw {
            pts.push((x, l));
            x += 2.0 * z;
        }
        pts.push((rw, l));
        let mut x = rw - 2.0 * z;
        while x >= 0.0 {
            pts.push((x + z, l + z));
            x -= 2.0 * z;
        }
        canvas::polygon(&mut jag, &pts, Some(op(CREAM)), None);
        let mut paper = shadowed(&jag, 10.0, 6.0, 90);
        if s < 1.0 {
            paper = canvas::resized(&paper, paper.width() as f32 * s, paper.height() as f32 * s);
        }
        let pw = paper.width() as f32;
        c.comp(&paper, cx - pw / 2.0, slot_y - 10.0 * s, al, true, false);
    }
    let slot = rrect(sw, c.h as f32 * 0.03, c.h as f32 * 0.015, (30, 30, 34, 255), Some(((90, 90, 96, 255), 3.0)));
    c.place(&shadowed(&slot, 10.0, 6.0, 140), cx, slot_y, 1.0, 0.0, al * clamp(c.lt / 0.15), true, false);
}

pub(crate) const MOODY: Effect = Effect {
    name: "moody",
    title: "Moody",
    desc: "A moody colour grade: darker, softer, teal shadows",
    options: "grade: moody, bw, warm, cool or pop",
    cards: false,
    draw: moody,
};
fn moody(c: &mut Ctx, e: &mut Run) {
    let k = e.ramp(c, 0.35, 0.0);
    let g = c.graded(Grade::named(&e.s("grade", "moody")));
    c.mix_with(&g, k);
    c.vignette(0.4 * k);
}

pub(crate) const SWIPE: Effect = Effect {
    name: "swipe",
    title: "Swipe",
    desc: "A before and after slider sweeps across you",
    options: "grade: the after look (moody)",
    cards: false,
    draw: swipe,
};
fn swipe(c: &mut Ctx, e: &mut Run) {
    let after = c.graded(Grade::named(&e.s("grade", "moody")));
    let p = c.lt / e.d;
    let sp = if p < 0.5 { 1.05 - 0.95 * ease_inout(p / 0.5) } else { 0.1 + 0.4 * ease_inout((p - 0.5) / 0.4) };
    let (w, h) = (c.w as f32, c.h as f32);
    for y in 0..c.h {
        let line_x = sp * w + 0.22 * (y as f32 - h / 2.0);
        for x in 0..c.w {
            let i = y * c.w + x;
            let a = ((x as f32 - line_x) / 3.0 + 0.5).clamp(0.0, 1.0);
            let edge = (1.0 - (x as f32 - line_x).abs() / 3.5).clamp(0.0, 1.0);
            for ch in 0..3 {
                let v = c.img[i * 3 + ch] * (1.0 - a) + after[i * 3 + ch] * a;
                c.img[i * 3 + ch] = v * (1.0 - edge) + edge;
            }
        }
    }
    let hy = h * 0.62;
    let hx = sp * w + 0.22 * (hy - h / 2.0);
    let r = (w * 0.035).floor();
    let mut handle = blank(2.0 * r + 4.0, 2.0 * r + 4.0);
    canvas::ellipse(&mut handle, 2.0, 2.0, 2.0 * r + 2.0, 2.0 * r + 2.0, Some((255, 255, 255, 255)), None);
    text(&mut handle, r + 2.0, r + 2.0, "< >", Kind::Mono, r * 0.7, op(INK), Anchor::MM);
    c.put(&shadowed(&handle, 8.0, 4.0, 110), hx, hy, 1.0, false, false);
    for (lab, x) in [("before", 0.14), ("after", 0.86)] {
        let t = text_img(lab, w * 0.03, Kind::Serif, INK, Look::default());
        let mut pill = rrect(t.width() as f32 + 16.0, t.height() as f32, t.height() as f32 / 2.0, (255, 255, 255, 225), None);
        over(&mut pill, &t, 8, 0);
        c.put(&pill, w * x, h * 0.88, 1.0, false, false);
    }
}

pub(crate) const TODO: Effect = Effect {
    name: "todo",
    title: "The To-Do",
    desc: "A checklist card beside your head ticks off as you talk",
    options: "text: items joined with |; ticks: seconds of the clip, joined with |; title",
    cards: false,
    draw: todo,
};
fn todo(c: &mut Ctx, e: &mut Run) {
    let items = parts(&e.s("text", "film the take|pick an effect|post it"));
    let n = items.len();
    let ticks: Vec<f32> = if e.has("ticks") {
        e.s("ticks", "").split('|').map(|t| t.trim().parse::<f32>().unwrap_or(0.0) - e.start).collect()
    } else {
        (0..n).map(|i| e.d * (i + 1) as f32 / (n + 1) as f32).collect()
    };
    let w = c.w as f32;
    let cw = (w * 0.5).floor();
    let lh = (cw * 0.13).floor();
    let pad = (cw * 0.07).floor();
    let mut im = rrect(cw, pad * 2.0 + lh * (n + 1) as f32, cw * 0.07, (250, 248, 242, 235), None);
    let done = ticks.iter().filter(|&&t| c.lt >= t).count();
    text(&mut im, pad, pad, &e.s("title", "TO DO").to_uppercase(), Kind::Ui, lh * 0.42, (200, 140, 20, 255), Anchor::LA);
    text(&mut im, cw - pad, pad, &format!("{done} of {n}"), Kind::Ui, lh * 0.36, (120, 120, 120, 255), Anchor::RA);
    for (i, it) in items.iter().enumerate() {
        let y = pad + lh * (i + 1) as f32;
        let bx = lh * 0.42;
        let (b0, b1) = (y + lh * 0.12, y + lh * 0.12 + bx);
        canvas::rounded(&mut im, pad, b0, pad + bx, b1, 6.0, None, Some(((160, 160, 160, 255), 3.0)));
        text(&mut im, pad + bx * 1.5, y + lh * 0.1, it, Kind::Ui, lh * 0.4, op(INK), Anchor::LA);
        if c.lt >= ticks.get(i).copied().unwrap_or(f32::MAX) {
            let k = ease_out((c.lt - ticks[i]) / 0.25);
            canvas::rounded(&mut im, pad, b0, pad + bx, b1, 6.0, Some((52, 199, 89, 255)), None);
            line(&mut im, &[(pad + bx * 0.22, b0 + bx * 0.52), (pad + bx * 0.42, b0 + bx * 0.74), (pad + bx * 0.8, b0 + bx * 0.28)], (255, 255, 255, 255), 4.0);
            let tw = canvas::length(Kind::Ui, lh * 0.4, it) * k;
            canvas::line_flat(&mut im, &[(pad + bx * 1.5, y + lh * 0.36), (pad + bx * 1.5 + tw, y + lh * 0.36)], op(INK), 3.0);
        }
    }
    let left = c.fx > w / 2.0;
    let x = if left { w * 0.05 + cw / 2.0 } else { w * 0.95 - cw / 2.0 };
    let pop = ease_back(c.lt / 0.3);
    let ih = im.height() as f32;
    let y = (ih / 2.0 + c.h as f32 * 0.06).max(c.fy - c.fw * 1.4);
    c.place(&shadowed(&im, 14.0, 8.0, 90), x, y, 0.8 + 0.2 * pop, 0.0, clamp(c.lt / 0.12) * clamp((e.d - c.lt) / 0.2), true, false);
}

pub(crate) use super::pack2::COMMENT_BOX;

pub(crate) const TAP: Effect = Effect {
    name: "tap",
    title: "The Tap",
    desc: "A Follow button pops up and flips to Following",
    options: "at: second of the clip the button flips; x, y: where (0 to 1); color: once flipped",
    cards: false,
    draw: tap,
};
fn tap(c: &mut Ctx, e: &mut Run) {
    let at = if e.has("at") { e.f("at", 0.0) - e.start } else { e.d * 0.45 };
    let (w, h) = (c.w as f32, c.h as f32);
    let (bw, bh) = ((w * 0.3).floor(), (w * 0.09).floor());
    let x = if e.has("x") { e.f("x", 0.5) * w } else { c.fx + c.fw * 1.9 }.clamp(bw / 2.0 + 30.0, w - bw / 2.0 - 30.0);
    let y = if e.has("y") { e.f("y", 0.5) * h } else { c.fy + c.fw * 1.6 }.clamp(bh + 30.0, h - bh - 30.0);
    let on = c.lt >= at;
    let col = if on { e.color("color", crate::PURPLE) } else { (40, 40, 46) };
    let mut im = rrect(bw, bh, bh * 0.28, op(col), None);
    text(&mut im, bw / 2.0, bh / 2.0, if on { "Following" } else { "+ Follow" }, Kind::Ui, bh * 0.4, (255, 255, 255, 255), Anchor::MM);
    let pop = ease_back(c.lt / 0.3);
    let mut sq = 1.0;
    if on {
        let q = (c.lt - at) / 0.3;
        if q < 1.0 {
            sq = 1.0 - 0.12 * (PI * clamp(q)).sin();
        }
    }
    c.place(&shadowed(&im, 12.0, 6.0, 110), x, y, (0.6 + 0.4 * pop) * sq, 0.0, clamp(c.lt / 0.1) * clamp((e.d - c.lt) / 0.2), false, true);
    if on && c.lt - at < 0.4 {
        let q = (c.lt - at) / 0.4;
        let r = (bh * (0.3 + 0.9 * q)).floor();
        let mut ring = blank(2.0 * r + 8.0, 2.0 * r + 8.0);
        canvas::ellipse(&mut ring, 4.0, 4.0, 2.0 * r + 4.0, 2.0 * r + 4.0, None, Some(((255, 255, 255, (220.0 * (1.0 - q)) as u8), 5.0)));
        c.put(&ring, x, y, 1.0, false, true);
    }
}

/// What a pause keeps of its first frame.
struct Frozen {
    img: Vec<f32>,
    m: Vec<f32>,
    face: (f32, f32, f32),
}

pub(crate) const PAUSE_POINT: Effect = Effect {
    name: "pause-point",
    title: "Pause & Point",
    desc: "Freezes the frame and points at you with a hand-drawn arrow",
    options: "text: the label (look here); color; label_x (0.72)",
    cards: false,
    draw: pause_point,
};
fn pause_point(c: &mut Ctx, e: &mut Run) {
    let (img, m, face) = (c.img.clone(), c.m.clone(), (c.fx, c.fy, c.fw));
    let frozen = e.cache("f", || Frozen { img, m, face });
    if frozen.img.len() == c.img.len() {
        c.img.copy_from_slice(&frozen.img);
        c.m.copy_from_slice(&frozen.m);
        (c.fx, c.fy, c.fw) = frozen.face;
    }
    c.face_cam(1.0 + 0.06 * ease_out(c.lt / e.d), 0.4);
    let bw = c.graded(Grade::Bw);
    c.mix_with(&bw, 0.08);
    let col = op(e.color("color", YELLOW));
    let (w, h) = (c.w as f32, c.h as f32);
    let mut lay = blank(w, h);
    canvas::rounded(&mut lay, w * 0.05, h * 0.06, w * 0.05 + w * 0.1, h * 0.06 + w * 0.1, 16.0, Some((0, 0, 0, 150)), None);
    for bx in [0.075, 0.11] {
        canvas::rect(&mut lay, w * bx, h * 0.06 + w * 0.025, w * (bx + 0.017), h * 0.06 + w * 0.075, (255, 255, 255, 230));
    }
    let (tx, ty) = (w * e.f("label_x", 0.72), c.fy - c.fw * 2.2);
    let pts = canvas::arrow_pts(tx - w * 0.05, ty + w * 0.06, c.fx + c.fw * 0.75, c.fy - c.fw * 0.35, 0.3, 40);
    let k = ease_out((c.lt - 0.15) / 0.4);
    let wd = (w / 140.0).floor().max(6.0);
    canvas::hand_path(&mut lay, &pts, col, wd, k);
    if k >= 0.99 {
        canvas::arrow_head(&mut lay, &pts, col, wd, w * 0.04);
    }
    c.comp(&lay, 0.0, 0.0, 1.0, false, false);
    let lab = text_img(&e.s("text", "look here"), w * 0.07, Kind::Hand, (col.0, col.1, col.2), Look { shadow: 6.0, ..Look::default() });
    let lh = lab.height() as f32;
    c.place(&lab, tx, ty - lh * 0.2, 0.7 + 0.3 * ease_back(c.lt / 0.3), -6.0, clamp(c.lt / 0.1), false, true);
}

pub(crate) const UNINSTALL: Effect = Effect {
    name: "uninstall",
    title: "Uninstall",
    desc: "A glitchy uninstall bar breaks the frame apart",
    options: "text (Uninstalling...)",
    cards: false,
    draw: uninstall,
};
fn uninstall(c: &mut Ctx, e: &mut Run) {
    let p = clamp(c.lt / e.d);
    let g = p.powf(1.6);
    let mut rnd = Rng::new((c.t * 30.0) as u64);
    let (w, h) = (c.w, c.h);
    let sh = (g * w as f32 * 0.025) as i64;
    if sh != 0 {
        let src = c.img.clone();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                let xr = ((x as i64 - sh).rem_euclid(w as i64)) as usize;
                let xb = ((x as i64 + sh).rem_euclid(w as i64)) as usize;
                c.img[i] = src[(y * w + xr) * 3];
                c.img[i + 2] = src[(y * w + xb) * 3 + 2];
            }
        }
    }
    for _ in 0..(2.0 + g * 10.0) as usize {
        let y0 = rnd.uniform(0.0, h as f32) as usize;
        let hh = rnd.uniform(8.0, 60.0) as usize;
        let off = (rnd.uniform(-1.0, 1.0) * g * w as f32 * 0.08) as i64;
        for y in y0..(y0 + hh).min(h) {
            let row: Vec<f32> = c.img[y * w * 3..(y + 1) * w * 3].to_vec();
            for x in 0..w {
                let sx = ((x as i64 - off).rem_euclid(w as i64)) as usize;
                c.img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&row[sx * 3..sx * 3 + 3]);
            }
        }
    }
    if p > 0.82 {
        let q = (p - 0.82) / 0.18;
        let f = 1 + (q * 40.0) as usize;
        let (sw, shh) = ((w / f).max(1), (h / f).max(1));
        let small = c.shrunk(sw, shh);
        for y in 0..h {
            for x in 0..w {
                let s = ((y * shh / h) * sw + x * sw / w) * 3;
                for ch in 0..3 {
                    c.img[(y * w + x) * 3 + ch] = small[s + ch] * (1.0 - 0.6 * q);
                }
            }
        }
    }
    let wf = w as f32;
    let (bw, bh) = ((wf * 0.7).floor(), (wf * 0.2).floor());
    let mut im = rrect(bw, bh, 22.0, (245, 245, 247, 240), None);
    text(&mut im, bw * 0.06, bh * 0.28, &e.s("text", "Uninstalling..."), Kind::Ui, bh * 0.18, op(INK), Anchor::LM);
    canvas::rounded(&mut im, bw * 0.06, bh * 0.56, bw * 0.94, bh * 0.7, 8.0, Some((215, 215, 220, 255)), None);
    canvas::rounded(&mut im, bw * 0.06, bh * 0.56, bw * 0.06 + bw * 0.88 * ease_inout(p).max(0.03), bh * 0.7, 8.0, Some((0, 122, 255, 255)), None);
    text(&mut im, bw * 0.94, bh * 0.85, &format!("{}%", (ease_inout(p) * 100.0) as i32), Kind::Ui, bh * 0.13, (110, 110, 110, 255), Anchor::RM);
    c.place(&shadowed(&im, 16.0, 8.0, 120), wf / 2.0, h as f32 * 0.16, 0.8 + 0.2 * ease_back(c.lt / 0.3), 0.0, clamp(c.lt / 0.1), false, false);
}

/// The column of counts down a reel's right edge: a heart, a bubble and a
/// paper plane, with numbers under the first two.
fn reel_icons(size: f32) -> Pixmap {
    let s = size.floor();
    let mut im = blank(s, s * 3.6);
    let wd = (s / 12.0).floor().max(3.0);
    let hrt = canvas::heart(s * 0.8, (255, 255, 255));
    over(&mut im, &hrt, (s * 0.1) as i32, 0);
    text(&mut im, s / 2.0, s * 0.95, "26.9K", Kind::Ui, s * 0.24, (255, 255, 255, 255), Anchor::MM);
    canvas::ellipse(&mut im, s * 0.15, s * 1.25, s * 0.85, s * 1.95, None, Some(((255, 255, 255, 255), wd)));
    text(&mut im, s / 2.0, s * 2.15, "412", Kind::Ui, s * 0.24, (255, 255, 255, 255), Anchor::MM);
    canvas::polygon(&mut im, &[(s * 0.12, s * 2.75), (s * 0.9, s * 2.45), (s * 0.55, s * 3.25), (s * 0.45, s * 2.88)], None, Some(((255, 255, 255, 255), wd)));
    im
}

pub(crate) const REEL_FRAME: Effect = Effect {
    name: "reel-frame",
    title: "Reel Frame",
    desc: "Your video shrinks into a reel card",
    options: "text: the handle (@your.handle)",
    cards: false,
    draw: reel_frame,
};
fn reel_frame(c: &mut Ctx, e: &mut Run) {
    let k = ease_out(c.lt / 0.35) * clamp((e.d - c.lt) / 0.3);
    if k <= 0.0 {
        return;
    }
    let s = 1.0 - 0.2 * k;
    let (w, h) = (c.w, c.h);
    let (wf, hf) = (w as f32, h as f32);
    // the room: the frame shrunk to a sixteenth, blown up and blurred
    let (sw, sh) = ((w / 16).max(1), (h / 16).max(1));
    let small = c.shrunk(sw, sh);
    let mut bg = vec![0f32; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let sx = ((x as f32 + 0.5) * sw as f32 / wf - 0.5).clamp(0.0, (sw - 1) as f32);
            let sy = ((y as f32 + 0.5) * sh as f32 / hf - 0.5).clamp(0.0, (sh - 1) as f32);
            let (x0, y0) = (sx as usize, sy as usize);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (tx, ty) = (sx - x0 as f32, sy - y0 as f32);
            for ch in 0..3 {
                let v = (small[(y0 * sw + x0) * 3 + ch] * (1.0 - tx) + small[(y0 * sw + x1) * 3 + ch] * tx) * (1.0 - ty)
                    + (small[(y1 * sw + x0) * 3 + ch] * (1.0 - tx) + small[(y1 * sw + x1) * 3 + ch] * tx) * ty;
                bg[(y * w + x) * 3 + ch] = v;
            }
        }
    }
    canvas::blur_f32(&mut bg, w, h, 3, 12.0);
    bg.iter_mut().for_each(|v| *v *= 1.0 - 0.45 * k);
    let (iw, ih) = ((wf * s) as usize, (hf * s) as usize);
    let inset = c.whole(iw as f32, ih as f32);
    let mut mask = rrect(iw as f32, ih as f32, wf * 0.05 * k + 1.0, (255, 255, 255, 255), None);
    let _ = &mut mask;
    let (x0, y0) = ((w - iw) / 2, ((h - ih) as f32 * 0.42) as usize);
    let idata = inset.data();
    let mdata = mask.data();
    for y in 0..ih {
        for x in 0..iw {
            let a = f32::from(mdata[(y * iw + x) * 4 + 3]) / 255.0;
            let o = ((y0 + y) * w + x0 + x) * 3;
            for ch in 0..3 {
                bg[o + ch] = bg[o + ch] * (1.0 - a) + f32::from(idata[(y * iw + x) * 4 + ch]) / 255.0 * a;
            }
        }
    }
    let mut mm = vec![0f32; w * h];
    for y in 0..ih {
        for x in 0..iw {
            let sx = ((x as f32 / s) as usize).min(w - 1);
            let sy = ((y as f32 / s) as usize).min(h - 1);
            mm[(y0 + y) * w + x0 + x] = c.m[sy * w + sx];
        }
    }
    c.img = bg;
    c.m = mm;
    c.fx = x0 as f32 + c.fx * s;
    c.fy = y0 as f32 + c.fy * s;
    c.fw *= s;
    let ic = e.cache("ic", || shadowed(&reel_icons(wf * 0.075), 6.0, 2.0, 120));
    c.comp(&ic, (x0 + iw) as f32 - ic.width() as f32 - wf * 0.01, y0 as f32 + ih as f32 * 0.52, k, false, false);
    let hdl = text_img(&e.s("text", "@your.handle"), wf * 0.032, Kind::Ui, (255, 255, 255), Look { shadow: 4.0, ..Look::default() });
    c.comp(&hdl, x0 as f32 + wf * 0.03, (y0 + ih) as f32 - hdl.height() as f32 - wf * 0.04, k, false, false);
}

pub(crate) const SHORTCUT_RAIN: Effect = Effect {
    name: "shortcut-rain",
    title: "Shortcut Rain",
    desc: "Keyboard keys rain down behind you",
    options: "text: keys joined with |; count (26)",
    cards: false,
    draw: shortcut_rain,
};
fn shortcut_rain(c: &mut Ctx, e: &mut Run) {
    let keys = parts(&e.s("text", "cmd|/|C|V|Z|esc|tab|shift|enter|opt"));
    let ks = (c.w as f32 * 0.1).floor();
    let caps = e.cache("caps", || {
        keys.iter()
            .map(|k| {
                let w = ks.max((canvas::length(Kind::Mono, ks * 0.36, k) + ks * 0.5).floor());
                let mut im = rrect(w, ks, ks * 0.18, (190, 190, 198, 255), None);
                let top = rrect(w - 8.0, ks - 14.0, ks * 0.16, (246, 246, 248, 255), None);
                over(&mut im, &top, 4, 3);
                text(&mut im, w / 2.0, ks * 0.44, k, Kind::Mono, ks * 0.36, op(INK), Anchor::MM);
                shadowed(&im, 8.0, 6.0, 100)
            })
            .collect::<Vec<_>>()
    });
    let n = e.f("count", 26.0) as u64;
    let d = e.d;
    let ps = e.cache("p", || {
        (0..n)
            .map(|i| {
                (
                    Rng::new(i).next(),
                    Rng::new(i + 99).next() * (d - 0.8).max(0.1),
                    0.45 + 0.4 * Rng::new(i + 7).next(),
                    Rng::new(i + 3).uniform(-25.0, 25.0),
                    Rng::new(i + 5).uniform(-60.0, 60.0),
                )
            })
            .collect::<Vec<_>>()
    });
    let (w, h) = (c.w as f32, c.h as f32);
    let al = clamp((e.d - c.lt) / 0.2);
    for (i, &(x, t0, v, r0, wv)) in ps.iter().enumerate() {
        if c.lt < t0 {
            continue;
        }
        let y = -ks + (c.lt - t0) * v * h;
        c.place(&caps[i % caps.len()], w * (0.04 + 0.92 * x), y, 0.75 + 0.35 * v, r0 + wv * (c.lt - t0), al, true, false);
    }
}

pub(crate) const REJECT_STAMP: Effect = Effect {
    name: "reject-stamp",
    title: "Reject Stamp",
    desc: "A big red stamp slams onto the screen",
    options: "text (REJECTED); color; angle (12); y (0 to 1)",
    cards: false,
    draw: reject_stamp,
};
fn reject_stamp(c: &mut Ctx, e: &mut Run) {
    let word = e.s("text", "REJECTED").to_uppercase();
    let col = e.color("color", (214, 40, 40));
    let w = c.w as f32;
    let st = e.cache("s", || {
        let t = fit_text(&word, w * 0.62, Kind::Bold, 200.0, col, Look::default());
        let pad = (t.height() as f32 * 0.12).floor();
        let mut im = blank(t.width() as f32 + 4.0 * pad, t.height() as f32 + 2.0 * pad);
        let (iw, ih) = (im.width() as f32, im.height() as f32);
        canvas::rounded(&mut im, 3.0, 3.0, iw - 4.0, ih - 4.0, 18.0, None, Some((op(col), (pad / 2.0).max(6.0))));
        over(&mut im, &t, (2.0 * pad) as i32, pad as i32);
        // the ink, worn: every pixel's alpha scaled by a grain
        let mut r = Rng::new(3);
        for px in im.data_mut().chunks_exact_mut(4) {
            let k = r.uniform(0.55, 1.6).min(1.0);
            for v in px.iter_mut() {
                *v = (f32::from(*v) * k) as u8;
            }
        }
        im
    });
    let k = clamp(c.lt / 0.16);
    let sc = 2.3 - 1.3 * k * k;
    if c.lt > 0.16 && c.lt < 0.45 {
        let a = (0.45 - c.lt) / 0.29 * w * 0.012;
        let mut r = Rng::new((c.t * 30.0) as u64);
        let (dx, dy) = (r.uniform(-a, a), r.uniform(-a, a));
        c.warp(|x, y| (x - dx, y - dy), true);
    }
    let y = if e.has("y") { e.f("y", 0.5) * c.h as f32 } else { c.fy + c.fw * 2.7 }.clamp(0.0, c.h as f32 * 0.85);
    c.place(&st, w / 2.0, y, sc, e.f("angle", 12.0), k * clamp((e.d - c.lt) / 0.2), false, true);
}

pub(crate) const COUNTDOWN_FLIP: Effect = Effect {
    name: "countdown-flip",
    title: "Countdown Flip",
    desc: "A 3, 2, 1 flip clock above your head",
    options: "from (3)",
    cards: false,
    draw: countdown_flip,
};
fn countdown_flip(c: &mut Ctx, e: &mut Run) {
    let n0 = e.f("from", 3.0).max(1.0) as i32;
    let seg = e.d / n0 as f32;
    let i = ((c.lt / seg) as i32).min(n0 - 1);
    let (cur, prev) = (n0 - i, n0 - i + 1);
    let f = if i > 0 { clamp((c.lt - i as f32 * seg) / 0.3) } else { 1.0 };
    let (tw, th) = ((c.w as f32 * 0.3).floor(), (c.w as f32 * 0.38).floor());
    let hv = e.cache("h", || {
        let mut out = Vec::new();
        for num in 0..=n0 + 1 {
            let mut im = rrect(tw, th, tw * 0.1, (30, 30, 34, 255), None);
            text(&mut im, tw / 2.0, th / 2.0, &num.to_string(), Kind::Bold, th * 0.62, op(CREAM), Anchor::MM);
            let top = canvas::crop(&im, 0.0, 0.0, tw, (th / 2.0).floor());
            let bottom = canvas::crop(&im, 0.0, (th / 2.0).floor(), tw, th - (th / 2.0).floor());
            out.push((top, bottom));
        }
        out
    });
    let half = (th / 2.0).floor();
    let mut im = blank(tw, th);
    let get = |n: i32| &hv[n.clamp(0, n0 + 1) as usize];
    over(&mut im, &get(cur).0, 0, 0);
    over(&mut im, &get(if f < 1.0 { prev } else { cur }).1, 0, half as i32);
    if f < 0.5 {
        let hh = (half * (1.0 - 2.0 * f)).max(1.0);
        let fl = canvas::resized(&get(prev).0, tw, hh);
        over(&mut im, &fl, 0, (half - hh) as i32);
    } else if f < 1.0 {
        let hh = (half * (2.0 * f - 1.0)).max(1.0);
        let fl = canvas::resized(&get(cur).1, tw, hh);
        over(&mut im, &fl, 0, half as i32);
    }
    canvas::line_flat(&mut im, &[(0.0, half), (tw, half)], (0, 0, 0, 255), 4.0);
    let y = (th / 2.0 + c.h as f32 * 0.05).max(c.fy - c.fw * 2.3);
    let x = c.w as f32 / 2.0 * 0.6 + c.fx * 0.4;
    c.place(&shadowed(&im, 16.0, 10.0, 120), x, y, 0.7 + 0.3 * ease_back(c.lt / 0.3), 0.0, clamp((e.d - c.lt) / 0.2), true, false);
}

pub(crate) const RABBIT_HOLE: Effect = Effect {
    name: "rabbit-hole",
    title: "Rabbit Hole",
    desc: "The frame tunnels into itself, spinning",
    options: "ratio (0.62); speed (0.8); spin (10)",
    cards: false,
    draw: rabbit_hole,
};
fn rabbit_hole(c: &mut Ctx, e: &mut Run) {
    let k = ease_out(c.lt / 0.4) * clamp((e.d - c.lt) / 0.3);
    if k <= 0.0 {
        return;
    }
    let (w, h) = (c.w, c.h);
    let t = (w / 120).max(4) as f32;
    let r = e.f("ratio", 0.62);
    let z = (c.lt * e.f("speed", 0.8)) % 1.0;
    let spin = e.f("spin", 10.0);
    let src = c.img.clone();
    let mut out = src.clone();
    let (cx, cy) = (c.fx, c.fy);
    for lev in 1..7 {
        let s = r.powf(lev as f32 - z * k);
        if s < 0.03 {
            break;
        }
        let ang = ((lev as f32 - z) * spin * k).to_radians();
        let (cs, sn) = (ang.cos(), ang.sin());
        for y in 0..h {
            for x in 0..w {
                // where the shrunk, turned copy of the frame puts this pixel
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let sx = (dx * cs - dy * sn) / s + cx;
                let sy = (dx * sn + dy * cs) / s + cy;
                if sx < 0.0 || sy < 0.0 || sx >= w as f32 || sy >= h as f32 {
                    continue;
                }
                let border = sx < t || sy < t || sx >= w as f32 - t || sy >= h as f32 - t;
                let o = (y * w + x) * 3;
                let si = ((sy as usize) * w + sx as usize) * 3;
                for ch in 0..3 {
                    let v = out[o + ch] * (1.0 - k) + src[si + ch] * k;
                    out[o + ch] = if border { v * (1.0 - k) + k } else { v };
                }
            }
        }
    }
    // you stay in front while the room tunnels behind you
    for i in 0..w * h {
        let m = c.m[i] * k;
        for ch in 0..3 {
            c.img[i * 3 + ch] = out[i * 3 + ch] * (1.0 - m) + src[i * 3 + ch] * m;
        }
    }
}

pub(crate) const COVER_STAR: Effect = Effect {
    name: "cover-star",
    title: "Cover Star",
    desc: "You become a magazine cover, title behind your head",
    options: "text: the title; lines joined with |; issue; color; look: editorial",
    cards: false,
    draw: cover_star,
};
fn cover_star(c: &mut Ctx, e: &mut Run) {
    if e.s("look", "") == "editorial" {
        return super::pack3::cover_editorial(c, e);
    }
    let k = ease_out(c.lt / 0.35);
    let al = clamp((e.d - c.lt) / 0.2);
    let pop = c.graded(Grade::Pop);
    c.mix_with(&pop, 0.6 * k);
    let title = e.s("text", "VIBE").to_uppercase();
    let col = e.color("color", (232, 36, 52));
    let (w, h) = (c.w as f32, c.h as f32);
    let mast = e.cache("m", || fit_text(&title, w * 0.92, Kind::Bold, 600.0, col, Look::default()));
    let mh = mast.height() as f32;
    let mast_h = mh.min(h * 0.24);
    c.place(&mast, w / 2.0, h * 0.035 + mast_h / 2.0 - (1.0 - k) * mast_h, mast_h / mh, 0.0, al * k, true, false);
    let issue = e.s("issue", "THE AI ISSUE  ·  2026");
    let iss = e.cache("i", || text_img(&issue, w * 0.025, Kind::Mono, (255, 255, 255), Look { shadow: 3.0, ..Look::default() }));
    c.place(&iss, w / 2.0, h * 0.03, 1.0, 0.0, al * k, true, false);
    let lines = parts(&e.s("lines", "51 effects|you can steal|no editor|needed"));
    let mut y = h * 0.5;
    for (j, ln) in lines.iter().enumerate() {
        let big = j == 0;
        let im = e.cache(&format!("l{j}"), || {
            text_img(ln, w * if big { 0.085 } else { 0.045 }, if big { Kind::Serif } else { Kind::Bold }, if big { YELLOW } else { (255, 255, 255) }, Look { shadow: 5.0, ..Look::default() })
        });
        let kj = ease_out((c.lt - 0.15 - 0.08 * j as f32) / 0.3);
        c.comp(&im, w * 0.05 - (1.0 - kj) * 60.0, y, al * kj, false, true);
        y += im.height() as f32 * 0.86;
    }
    let bc = e.cache("b", || {
        let mut r = Rng::new(1);
        let (bw, bh) = ((w * 0.16).floor(), (w * 0.09).floor());
        let mut im = rrect(bw, bh, 0.0, (255, 255, 255, 255), None);
        let mut x = 8.0;
        while x < bw - 8.0 {
            let ww = [2.0, 2.0, 3.0, 5.0][(r.next() * 4.0) as usize % 4];
            canvas::rect(&mut im, x, 6.0, x + ww, bh - 18.0, (0, 0, 0, 255));
            x += ww + [2.0, 3.0, 4.0][(r.next() * 3.0) as usize % 3];
        }
        im
    });
    c.comp(&bc, w * 0.95 - bc.width() as f32, h * 0.93 - bc.height() as f32, al * k, false, true);
}

pub(crate) const SCRIBBLE: Effect = Effect {
    name: "scribble",
    title: "Scribble",
    desc: "Marker doodles draw themselves around you",
    options: "text: the label (me!); color",
    cards: false,
    draw: scribble,
};
fn scribble(c: &mut Ctx, e: &mut Run) {
    let col = op(e.color("color", YELLOW));
    let (w, h) = (c.w as f32, c.h as f32);
    let wd = (w / 130.0).floor().max(6.0);
    let mut lay = blank(w, h);
    let (rx, ry) = (c.fw * 1.0, c.fw * 1.3);
    let ring: Vec<(f32, f32)> = (0..70)
        .map(|i| {
            let a = -1.9 + 2.0 * PI * 1.08 * i as f32 / 69.0;
            (c.fx + rx * (1.0 + 0.04 * (3.0 * a).sin()) * a.cos(), c.fy + ry * (1.0 + 0.04 * (2.0 * a).cos()) * a.sin())
        })
        .collect();
    canvas::hand_path(&mut lay, &ring, col, wd, ease_inout(c.lt / 0.5));
    let lx = (c.fx + c.fw * 2.3).clamp(w * 0.2, w * 0.8);
    let ly = (h * 0.08).max(c.fy - c.fw * 2.4);
    let pts = canvas::arrow_pts(lx - c.fw * 0.3, ly + c.fw * 0.5, c.fx + rx * 0.85, c.fy - ry * 0.75, -0.3, 40);
    let k2 = ease_out((c.lt - 0.45) / 0.35);
    if k2 > 0.0 {
        canvas::hand_path(&mut lay, &pts, col, wd, k2);
    }
    if k2 >= 0.99 {
        canvas::arrow_head(&mut lay, &pts, col, wd, w * 0.035);
    }
    for (j, (sx, sy)) in [(c.fx - c.fw * 1.9, c.fy - c.fw * 1.2), (c.fx - c.fw * 1.5, c.fy + c.fw * 0.6), (c.fx + c.fw * 1.6, c.fy + c.fw * 0.3)].into_iter().enumerate() {
        let ks = ease_back((c.lt - 0.8 - 0.1 * j as f32) / 0.25) * c.fw * 0.28;
        if ks > 0.0 {
            canvas::line_flat(&mut lay, &[(sx - ks, sy), (sx + ks, sy)], col, wd);
            canvas::line_flat(&mut lay, &[(sx, sy - ks), (sx, sy + ks)], col, wd);
        }
    }
    let al = clamp((e.d - c.lt) / 0.2);
    c.comp(&lay, 0.0, 0.0, al, false, false);
    if c.lt > 0.7 {
        let lab = text_img(&e.s("text", "me!"), w * 0.075, Kind::Hand, (col.0, col.1, col.2), Look { shadow: 6.0, ..Look::default() });
        let lh = lab.height() as f32;
        c.place(&lab, lx, ly - lh * 0.15, 0.7 + 0.3 * ease_back((c.lt - 0.7) / 0.3), 8.0, al, false, false);
    }
}
