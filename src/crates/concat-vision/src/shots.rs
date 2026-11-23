// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! How a finished video was put together, read back from its pictures: where
//! it cuts and how it goes from shot to shot, how the picture moves within a
//! shot, where it flashes, and where something stays put over the cuts.
//!
//! No model: the pictures are looked at small, one after another. What was
//! under an overlay, and layers that were flattened, cannot be had back; this
//! finds the times and the moves, for other pictures to be put in.

/// The width the pictures are looked at.
pub const WIDTH: usize = 96;

/// How one shot gives way to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    /// At once.
    Cut,
    /// The one showing through the other.
    Dissolve,
    /// Through black.
    Black,
    /// Through white.
    White,
    /// Swept sideways (or up and down) in a blur.
    Whip,
    /// Rushed into, or out of.
    Zoom,
    /// Through a blur.
    Blur,
    /// The one pushing the other out of the frame.
    Push,
    /// The one uncovered over the other, an edge going across.
    Wipe,
}

/// Where the picture is in a shot: larger than the frame by `scale` and off
/// its middle by `x` and `y` (fractions of the frame), `at` of the way
/// through (0 to 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// How far through the shot.
    pub at: f64,
    /// How large: 1 fills the frame.
    pub scale: f64,
    /// To the right of the middle.
    pub x: f64,
    /// Below the middle.
    pub y: f64,
}

/// One shot.
#[derive(Debug, Clone)]
pub struct Shot {
    /// When it starts, seconds.
    pub start: f64,
    /// When it ends.
    pub end: f64,
    /// How it came in (the first shot's is a cut).
    pub join: Join,
    /// How long that took, seconds; none for a cut.
    pub join_seconds: f64,
    /// How the picture moves, or empty when it stays.
    pub moves: Vec<Place>,
    /// Whether it moves as an editor moves a picture, quicker than a hand
    /// holding a camera would.
    pub edited: bool,
    /// Whether it shakes.
    pub shakes: bool,
}

/// Something that stays where it is while the shots change under it: words,
/// a sticker, a mark. Fractions of the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    /// When it is first there, seconds.
    pub start: f64,
    /// When it is last there.
    pub end: f64,
    /// Its left edge.
    pub left: f64,
    /// Its top edge.
    pub top: f64,
    /// Its right edge.
    pub right: f64,
    /// Its bottom edge.
    pub bottom: f64,
}

/// What was read.
#[derive(Debug, Clone, Default)]
pub struct Reading {
    /// The shots, end to end.
    pub shots: Vec<Shot>,
    /// When the picture flashes within a shot, seconds.
    pub flashes: Vec<f64>,
    /// What stays over the cuts.
    pub held: Vec<Held>,
}

const BINS: usize = 16;

struct Look {
    luma: Vec<u8>,
    hist: [f32; BINS * 3],
    mean: f32,
    spread: f32,
    sharp: f32,
}

/// Reads pictures in their order.
pub struct Reader {
    w: usize,
    h: usize,
    fps: f64,
    looks: Vec<Look>,
}

impl Reader {
    /// For pictures `w` by `h`, `fps` a second.
    pub fn new(w: usize, h: usize, fps: f64) -> Reader {
        Reader {
            w,
            h,
            fps: fps.max(1.0),
            looks: Vec::new(),
        }
    }

    /// The next picture, as `step` bytes a pixel, red, green and blue first.
    pub fn push(&mut self, pixels: &[u8], step: usize) {
        let n = self.w * self.h;
        if step < 3 || pixels.len() < n * step {
            return;
        }
        let mut luma = vec![0u8; n];
        let mut counts = [0u32; BINS * 3];
        for (i, out) in luma.iter_mut().enumerate() {
            let p = &pixels[i * step..i * step + 3];
            *out =
                ((u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) >> 8) as u8;
            for c in 0..3 {
                counts[c * BINS + usize::from(p[c]) * BINS / 256] += 1;
            }
        }
        let mean = luma.iter().map(|v| f32::from(*v)).sum::<f32>() / n as f32;
        let spread = (luma
            .iter()
            .map(|v| (f32::from(*v) - mean).powi(2))
            .sum::<f32>()
            / n as f32)
            .sqrt();
        let mut sharp = 0f32;
        for y in 1..self.h - 1 {
            for x in 1..self.w - 1 {
                let at = y * self.w + x;
                let v = i32::from(luma[at]) * 4
                    - i32::from(luma[at - 1])
                    - i32::from(luma[at + 1])
                    - i32::from(luma[at - self.w])
                    - i32::from(luma[at + self.w]);
                sharp += v.unsigned_abs() as f32;
            }
        }
        sharp /= ((self.w - 2) * (self.h - 2)) as f32;
        let mut hist = [0f32; BINS * 3];
        for (out, count) in hist.iter_mut().zip(counts) {
            *out = count as f32 / n as f32;
        }
        self.looks.push(Look {
            luma,
            hist,
            mean,
            spread,
            sharp,
        });
    }

    /// How many pictures it has read.
    pub fn len(&self) -> usize {
        self.looks.len()
    }

    /// Whether it has read none.
    pub fn is_empty(&self) -> bool {
        self.looks.is_empty()
    }

    /// How unlike two pictures' colours are, 0 to 1.
    fn colours(&self, a: usize, b: usize) -> f32 {
        let (a, b) = (&self.looks[a].hist, &self.looks[b].hist);
        // a bin's share may have slid into the next: the running sums tell less of that
        let mut total = 0f32;
        for c in 0..3 {
            let mut run = 0f32;
            for i in 0..BINS {
                run += a[c * BINS + i] - b[c * BINS + i];
                total += run.abs();
            }
        }
        (total / (3.0 * 5.0)).min(1.0)
    }

    /// How unlike two pictures are after each piece of the one has been
    /// looked for about its place in the other: near 0 for the same thing
    /// moved, about 1 for another thing.
    fn pieces(&self, a: usize, b: usize) -> f32 {
        const SIDE: usize = 8;
        const REACH: i32 = 5;
        let (pa, pb) = (&self.looks[a].luma, &self.looks[b].luma);
        let (mut miss, mut grain) = (0f32, 0f32);
        let mut by = 0;
        while by + SIDE <= self.h {
            let mut bx = 0;
            while bx + SIDE <= self.w {
                let mut mean = 0u32;
                for y in 0..SIDE {
                    for x in 0..SIDE {
                        mean += u32::from(pb[(by + y) * self.w + bx + x]);
                    }
                }
                let mean = (mean / (SIDE * SIDE) as u32) as i32;
                let mut own = 0u32;
                for y in 0..SIDE {
                    for x in 0..SIDE {
                        own += (i32::from(pb[(by + y) * self.w + bx + x]) - mean).unsigned_abs();
                    }
                }
                let mut best = u32::MAX;
                let mut dy = -REACH;
                while dy <= REACH {
                    let sy = by as i32 + dy;
                    if sy >= 0 && sy as usize + SIDE <= self.h {
                        for dx in -REACH..=REACH {
                            let sx = bx as i32 + dx;
                            if sx < 0 || sx as usize + SIDE > self.w {
                                continue;
                            }
                            let mut sum = 0u32;
                            for y in 0..SIDE {
                                let ra = (sy as usize + y) * self.w + sx as usize;
                                let rb = (by + y) * self.w + bx;
                                for x in 0..SIDE {
                                    sum += u32::from(pa[ra + x].abs_diff(pb[rb + x]));
                                }
                                if sum >= best {
                                    break;
                                }
                            }
                            best = best.min(sum);
                        }
                    }
                    dy += 1;
                }
                miss += best as f32;
                grain += own as f32;
                bx += SIDE;
            }
            by += SIDE;
        }
        let blocks = ((self.w / SIDE) * (self.h / SIDE) * SIDE * SIDE) as f32;
        (miss / blocks) / (grain / blocks + 4.0)
    }

    /// How alike two pictures' outlines are, -1 to 1: alike when only the
    /// light has changed.
    fn outlines(&self, a: usize, b: usize) -> f32 {
        let (pa, pb) = (&self.looks[a].luma, &self.looks[b].luma);
        let (mut ab, mut aa, mut bb) = (0f64, 0f64, 0f64);
        for y in 1..self.h - 1 {
            for x in 1..self.w - 1 {
                let at = y * self.w + x;
                for step in [1, self.w] {
                    let ga = f64::from(pa[at + step]) - f64::from(pa[at - step]);
                    let gb = f64::from(pb[at + step]) - f64::from(pb[at - step]);
                    ab += ga * gb;
                    aa += ga * ga;
                    bb += gb * gb;
                }
            }
        }
        (ab / (aa * bb).sqrt().max(1.0)) as f32
    }

    /// Reads what it was shown.
    pub fn finish(&self) -> Reading {
        let n = self.looks.len();
        if n < 2 {
            let end = n as f64 / self.fps;
            return Reading {
                shots: vec![Shot {
                    start: 0.0,
                    end,
                    join: Join::Cut,
                    join_seconds: 0.0,
                    moves: Vec::new(),
                    edited: false,
                    shakes: false,
                }],
                ..Reading::default()
            };
        }
        let colour: Vec<f32> = (0..n)
            .map(|i| if i == 0 { 0.0 } else { self.colours(i - 1, i) })
            .collect();
        let piece: Vec<f32> = (0..n)
            .map(|i| if i == 0 { 0.0 } else { self.pieces(i - 1, i) })
            .collect();

        // the fades first: what is found in one is part of it
        let mut joins: Vec<(usize, usize, usize, Join)> = Vec::new();
        self.fades(&mut joins);
        self.slides(&mut joins);
        let inside = |joins: &[(usize, usize, usize, Join)], i: usize, slack: usize| {
            joins
                .iter()
                .any(|(_, from, to, _)| i + slack >= *from && i <= *to + slack)
        };
        let mut flashes = Vec::new();
        let mut skip_to = 0;
        for i in 1..n {
            if i < skip_to || inside(&joins, i, 1) {
                continue;
            }
            let score = colour[i] * 1.6 + piece[i];
            let strong = (piece[i] > 0.62 && colour[i] > 0.1)
                || (piece[i] > 0.8 && colour[i] > 0.05)
                || (colour[i] > 0.42 && piece[i] > 0.35);
            if !strong {
                continue;
            }
            let about: Vec<f32> = (i.saturating_sub(9)..(i + 10).min(n))
                .filter(|j| j.abs_diff(i) > 1 && *j > 0)
                .map(|j| colour[j] * 1.6 + piece[j])
                .collect();
            let mut sorted = about.clone();
            sorted.sort_by(f32::total_cmp);
            let usual = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
            let most = (i.saturating_sub(2)..(i + 3).min(n))
                .filter(|j| *j != i)
                .map(|j| colour[j] * 1.6 + piece[j])
                .fold(0f32, f32::max);
            if score < usual * 1.7 + 0.12 || score < most {
                continue;
            }
            // what comes back within a few pictures was a flash, not a cut
            let back = (1..=5usize).find(|k| {
                i + k < n && self.colours(i - 1, i + k) < 0.07 && self.pieces(i - 1, i + k) < 0.5
            });
            if let Some(k) = back {
                if self.looks[i].mean > self.looks[i - 1].mean + 12.0 {
                    flashes.push(i as f64 / self.fps);
                }
                skip_to = i + k + 1;
                continue;
            }
            joins.push((i, i, i, Join::Cut));
        }
        self.dissolves(&mut joins);
        joins.sort_by_key(|join| join.0);
        // two joins nearer than a tenth of a second are one
        let least = (self.fps * 0.1).ceil() as usize;
        joins.dedup_by(|b, a| {
            let near = b.0 < a.0 + least || b.1 <= a.2;
            if near && a.3 == Join::Cut && b.3 != Join::Cut {
                *a = *b;
            }
            near
        });
        joins.retain(|join| join.0 >= least.min(3) && join.0 + least.min(3) <= n);
        self.rushes(&mut joins);

        let mut shots = Vec::with_capacity(joins.len() + 1);
        let time = |i: usize| i as f64 / self.fps;
        let mut at = 0;
        let mut came = (0usize, 0usize, 0usize, Join::Cut);
        for join in joins
            .iter()
            .copied()
            .chain(std::iter::once((n, n, n, Join::Cut)))
        {
            let (from, to) = (at, join.0);
            let inner = (
                came.2.max(from) + usize::from(came.2 > came.1),
                join.1.min(to),
            );
            let (moves, edited, shakes) = self.moves(inner.0, inner.1);
            shots.push(Shot {
                start: time(from),
                end: time(to),
                join: came.3,
                join_seconds: if came.3 == Join::Cut {
                    0.0
                } else {
                    ((came.2 - came.1 + 1) as f64 / self.fps).clamp(0.15, 1.5)
                },
                moves,
                edited,
                shakes,
            });
            at = to;
            came = join;
        }
        let held = self.held(&shots);
        Reading {
            shots,
            flashes,
            held,
        }
    }

    /// Goes through black and white.
    fn fades(&self, joins: &mut Vec<(usize, usize, usize, Join)>) {
        let n = self.looks.len();
        for (kind, bare) in [
            (
                Join::Black,
                Box::new(|l: &Look| l.mean < 14.0 && l.spread < 9.0) as Box<dyn Fn(&Look) -> bool>,
            ),
            (
                Join::White,
                Box::new(|l: &Look| l.mean > 236.0 && l.spread < 14.0),
            ),
        ] {
            let mut i = 0;
            while i < n {
                if !bare(&self.looks[i]) {
                    i += 1;
                    continue;
                }
                let from = i;
                while i < n && bare(&self.looks[i]) {
                    i += 1;
                }
                let to = i - 1;
                // at the very start or end there is no shot on the other side
                if from < 3 || to + 3 >= n {
                    continue;
                }
                // how far out the picture is still on its way
                let way = |j: usize, k: usize| match kind {
                    Join::Black => self.looks[j].mean + 1.0 < self.looks[k].mean,
                    _ => self.looks[j].mean > self.looks[k].mean + 1.0,
                };
                let most = (self.fps * 0.9) as usize;
                let mut start = from;
                while start > 0 && from - start < most && way(start, start - 1) {
                    start -= 1;
                }
                let mut end = to;
                while end + 1 < n && end - to < most && way(end, end + 1) {
                    end += 1;
                }
                if to - from < (self.fps * 1.5) as usize {
                    joins.push(((from + to).div_ceil(2), start, end, kind));
                }
            }
        }
    }

    /// Shots pushed out of the frame, or wiped from it.
    fn slides(&self, joins: &mut Vec<(usize, usize, usize, Join)>) {
        let n = self.looks.len();
        let (w, h) = (self.w, self.h);
        let rows: Vec<usize> = (0..h).step_by(5).collect();
        let cols: Vec<usize> = (0..w).step_by(3).collect();
        // how far from being A and B with an edge at `p` (0 to 1) going `way`,
        // pushed or wiped, picture `f` is
        let cost = |f: usize, a: usize, b: usize, way: usize, push: bool, p: f64| -> f32 {
            let (pf, pa, pb) = (
                &self.looks[f].luma,
                &self.looks[a].luma,
                &self.looks[b].luma,
            );
            let across = way < 2;
            let size = if across { w } else { h };
            let edge = (p * size as f64) as usize;
            let mut sum = 0u32;
            for &y in &rows {
                for &x in &cols {
                    let v = if across { x } else { y };
                    // way 0 and 2: the new one comes from the right, or below
                    let (new, along) = if way % 2 == 0 {
                        let new = v >= size - edge;
                        (
                            new,
                            match (push, new) {
                                (false, _) => v,
                                (true, true) => v - (size - edge),
                                (true, false) => v + edge,
                            },
                        )
                    } else {
                        let new = v < edge;
                        (
                            new,
                            match (push, new) {
                                (false, _) => v,
                                (true, true) => v + size - edge,
                                (true, false) => v - edge,
                            },
                        )
                    };
                    let along = along.min(size - 1);
                    let at = if across { y * w + along } else { along * w + x };
                    let from = if new { pb[at] } else { pa[at] };
                    sum += u32::from(from.abs_diff(pf[y * w + x]));
                }
            }
            sum as f32 / (rows.len() * cols.len()) as f32
        };
        let best = |f: usize, a: usize, b: usize, way: usize, push: bool| -> (f32, f64) {
            let mut best = (f32::MAX, 0.0);
            for k in 1..32 {
                let p = f64::from(k) / 32.0;
                let c = cost(f, a, b, way, push, p);
                if c < best.0 {
                    best = (c, p);
                }
            }
            best
        };
        let mut found: Vec<(f32, usize, usize, Join)> = Vec::new();
        for seconds in [0.25, 0.4, 0.6] {
            let span = (self.fps * seconds).round() as usize;
            if span < 5 || span + 2 > n {
                continue;
            }
            for from in 0..n - span {
                let to = from + span;
                if joins.iter().any(|(_, a, b, _)| from <= *b && *a <= to) {
                    continue;
                }
                let far = cost(from, from, to, 0, false, 1.0);
                if far < 18.0 {
                    continue;
                }
                let mid = from + span / 2;
                let nearer =
                    cost(mid, from, to, 0, false, 0.0).min(cost(mid, from, to, 0, false, 1.0));
                if nearer < far * 0.15 {
                    continue;
                }
                let mut top: Option<(f32, usize, bool)> = None;
                for way in 0..4 {
                    for push in [true, false] {
                        let (c, p) = best(mid, from, to, way, push);
                        if c < nearer * 0.42
                            && (0.12..0.88).contains(&p)
                            && top.is_none_or(|t| c < t.0)
                        {
                            top = Some((c, way, push));
                        }
                    }
                }
                let Some((c, way, push)) = top else {
                    continue;
                };
                let mut last = -0.01;
                let mut fits = true;
                let mut total = 0f32;
                for t in 1..span {
                    let (k, p) = best(from + t, from, to, way, push);
                    fits &= p >= last - 0.05 && k < far * 0.4;
                    last = p;
                    total += k;
                }
                let (_, early) = best(from + span / 4, from, to, way, push);
                let (_, late) = best(to - span / 4, from, to, way, push);
                if fits && late - early > 0.2 {
                    found.push((
                        total / (span - 1) as f32 / far + c / far,
                        from,
                        to,
                        if push { Join::Push } else { Join::Wipe },
                    ));
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut kept: Vec<(usize, usize)> = Vec::new();
        for (_, from, to, kind) in found {
            if kept.iter().all(|(a, b)| to < *a || from > *b) {
                kept.push((from, to));
                joins.push(((from + to).div_ceil(2), from, to, kind));
            }
        }
    }

    /// Shots that show through each other as they change.
    fn dissolves(&self, joins: &mut Vec<(usize, usize, usize, Join)>) {
        let n = self.looks.len();
        let taken: Vec<(usize, usize)> = joins.iter().map(|j| (j.1, j.2)).collect();
        let mut found: Vec<(f32, usize, usize)> = Vec::new();
        for seconds in [0.25, 0.4, 0.6, 0.85, 1.2] {
            let span = (self.fps * seconds).round() as usize;
            if span < 4 || span + 2 > n {
                continue;
            }
            for from in 0..n - span {
                let to = from + span;
                if taken.iter().any(|(a, b)| from <= *b + 1 && *a <= to + 1) {
                    continue;
                }
                let (a, b) = (&self.looks[from].luma, &self.looks[to].luma);
                // every fourth pixel tells enough for a first look
                let (mut dd, mut count) = (0f64, 0f64);
                for k in (0..a.len()).step_by(3) {
                    let d = f64::from(b[k]) - f64::from(a[k]);
                    dd += d * d;
                    count += 1.0;
                }
                if (dd / count).sqrt() < 22.0 {
                    continue;
                }
                let (mut last, mut worst, mut left, mut steady) = (-0.05f64, 0f64, 0f64, true);
                // where within it the change really is
                let (mut begins, mut ends) = (0, span);
                for t in 1..span {
                    let f = &self.looks[from + t].luma;
                    let (mut along, mut off) = (0f64, 0f64);
                    for k in (0..a.len()).step_by(3) {
                        along += (f64::from(f[k]) - f64::from(a[k]))
                            * (f64::from(b[k]) - f64::from(a[k]));
                    }
                    let alpha = along / dd;
                    for k in (0..a.len()).step_by(3) {
                        let d = f64::from(f[k])
                            - f64::from(a[k])
                            - alpha * (f64::from(b[k]) - f64::from(a[k]));
                        off += d * d;
                    }
                    if alpha < last - 0.04 {
                        steady = false;
                        break;
                    }
                    last = alpha;
                    if alpha < 0.04 {
                        begins = t;
                    }
                    if alpha > 0.96 && ends == span {
                        ends = t;
                    }
                    worst = worst.max((alpha - t as f64 / span as f64).abs());
                    left += off / dd;
                }
                let left = left / (span - 1) as f64;
                if !steady || ends < begins + 4 || worst > 0.3 || left > 0.2 {
                    continue;
                }
                // a change of light is a blend of its ends too, but its outlines stay
                if self.outlines(from, to) > 0.45 || self.pieces(from, to) < 0.5 {
                    continue;
                }
                // nor is a move one, which takes the first to the last
                let moved = |a: usize, b: usize| {
                    self.similarity(a, b).is_some_and(|m| {
                        m[0].abs() * self.w as f64 / 2.0 + m[2].abs() + m[3].abs() >= 0.4
                    })
                };
                if moved(from, (from + to) / 2) || moved(from, to) {
                    continue;
                }
                // of those that fit, the one that takes in the most of it
                found.push((
                    left as f32 + worst as f32 * 0.5 - span as f32 / self.fps as f32 * 0.25,
                    from + begins,
                    from + ends,
                ));
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut kept: Vec<(usize, usize)> = Vec::new();
        for (_, from, to) in found {
            if kept.iter().all(|(a, b)| to < *a || from > *b) {
                kept.push((from, to));
            }
        }
        joins.extend(
            kept.into_iter()
                .map(|(from, to)| ((from + to).div_ceil(2), from, to, Join::Dissolve)),
        );
    }

    /// Cuts hidden in a rush: which of them are, and how they rush.
    fn rushes(&self, joins: &mut [(usize, usize, usize, Join)]) {
        let n = self.looks.len();
        let reach = (self.fps * 0.2).round().max(2.0) as usize;
        for j in 0..joins.len() {
            let (at, _, _, kind) = joins[j];
            if kind != Join::Cut {
                continue;
            }
            let before = if j == 0 { 0 } else { joins[j - 1].0 };
            let after = if j + 1 < joins.len() {
                joins[j + 1].0
            } else {
                n
            };
            let usual = |from: usize, to: usize| {
                let mut v: Vec<f32> = self.looks[from..to].iter().map(|l| l.sharp).collect();
                v.sort_by(f32::total_cmp);
                v.get(v.len() * 2 / 3).copied().unwrap_or(0.0)
            };
            let (left, right) = (
                at.saturating_sub(reach).max(before),
                (at + reach).min(after),
            );
            if at - left < 2 || right - at < 2 || at - before < reach * 2 || after - at < reach * 2
            {
                continue;
            }
            let (was, is) = (usual(before, at), usual(at, after));
            let near = |a: usize, b: usize| {
                self.looks[a..b]
                    .iter()
                    .map(|l| l.sharp)
                    .fold(f32::INFINITY, f32::min)
            };
            let blurred_out = near(at.saturating_sub(2).max(left), at) < was * 0.6;
            let blurred_in = near(at, (at + 2).min(right)) < is * 0.6;
            if !(blurred_out && blurred_in) {
                continue;
            }
            let (from, to) = (left, right.min(n - 1));
            let outward = self.similarity(at.saturating_sub(3).max(left), at - 1);
            let inward = self.similarity(at, (at + 2).min(to));
            let pan = |m: &Option<[f64; 4]>| {
                m.map_or(0.0, |m| {
                    (m[2].abs() / self.w as f64).max(m[3].abs() / self.h as f64)
                })
            };
            let zoom = |m: &Option<[f64; 4]>| m.map_or(0.0, |m| m[0].abs());
            joins[j] = (
                at,
                from,
                to,
                if zoom(&outward).max(zoom(&inward)) > 0.06 {
                    Join::Zoom
                } else if pan(&outward).max(pan(&inward)) > 0.04 {
                    Join::Whip
                } else {
                    Join::Blur
                },
            );
        }
    }

    /// How picture `b` is picture `a` scaled (less one), turned and moved, in
    /// pixels about the middle: `[scale, turn, x, y]`. None when no such move
    /// explains it.
    fn similarity(&self, a: usize, b: usize) -> Option<[f64; 4]> {
        let (w, h) = (self.w, self.h);
        let (pa, pb) = (&self.looks[a].luma, &self.looks[b].luma);
        let smooth = |p: &[u8], by: usize| -> (usize, usize, Vec<f32>) {
            let (sw, sh) = (w / by, h / by);
            let mut out = vec![0f32; sw * sh];
            for y in 0..sh {
                for x in 0..sw {
                    let mut sum = 0f32;
                    for dy in 0..by {
                        for dx in 0..by {
                            sum += f32::from(p[(y * by + dy) * w + x * by + dx]);
                        }
                    }
                    out[y * sw + x] = sum / (by * by) as f32;
                }
            }
            (sw, sh, out)
        };
        // x' = x + s x - r y + tx; y' = y + r x + s y + ty
        let mut m = [0f64; 4];
        let mut first = 0f64;
        let mut last = 0f64;
        for by in [4usize, 2] {
            let (sw, sh, fa) = smooth(pa, by);
            let (_, _, fb) = smooth(pb, by);
            let (cx, cy) = (sw as f64 / 2.0, sh as f64 / 2.0);
            let sample = |f: &[f32], x: f64, y: f64| -> Option<f64> {
                if x < 0.0 || y < 0.0 || x > (sw - 2) as f64 || y > (sh - 2) as f64 {
                    return None;
                }
                let (x0, y0) = (x as usize, y as usize);
                let (u, v) = (x - x0 as f64, y - y0 as f64);
                let g = |x: usize, y: usize| f64::from(f[y * sw + x]);
                Some(
                    (g(x0, y0) * (1.0 - u) + g(x0 + 1, y0) * u) * (1.0 - v)
                        + (g(x0, y0 + 1) * (1.0 - u) + g(x0 + 1, y0 + 1) * u) * v,
                )
            };
            let mut t = [m[0], m[1], m[2] / by as f64, m[3] / by as f64];
            for round in 0..10 {
                let mut ata = [[0f64; 4]; 4];
                let mut atb = [0f64; 4];
                let (mut off, mut count) = (0f64, 0f64);
                for y in 1..sh - 1 {
                    for x in 1..sw - 1 {
                        let (px, py) = (x as f64 - cx, y as f64 - cy);
                        let qx = px + t[0] * px - t[1] * py + t[2] + cx;
                        let qy = py + t[1] * px + t[0] * py + t[3] + cy;
                        let (Some(v), Some(l), Some(r), Some(u), Some(d)) = (
                            sample(&fb, qx, qy),
                            sample(&fb, qx - 1.0, qy),
                            sample(&fb, qx + 1.0, qy),
                            sample(&fb, qx, qy - 1.0),
                            sample(&fb, qx, qy + 1.0),
                        ) else {
                            continue;
                        };
                        let e = f64::from(fa[y * sw + x]) - v;
                        // what moves by itself weighs little
                        let weight = 1.0 / (1.0 + (e / 14.0).powi(2));
                        let (gx, gy) = ((r - l) / 2.0, (d - u) / 2.0);
                        let row = [gx * px + gy * py, gy * px - gx * py, gx, gy];
                        for i in 0..4 {
                            for k in 0..4 {
                                ata[i][k] += weight * row[i] * row[k];
                            }
                            atb[i] += weight * row[i] * e;
                        }
                        off += e.abs().min(40.0);
                        count += 1.0;
                    }
                }
                if count < (sw * sh) as f64 * 0.3 {
                    return None;
                }
                if by == 2 && round == 0 {
                    // what it would be with no move at all
                    let mut still = 0f64;
                    for k in 0..fa.len() {
                        still += f64::from((fa[k] - fb[k]).abs()).min(40.0);
                    }
                    first = still / fa.len() as f64;
                }
                last = off / count;
                let step = solve(ata, atb)?;
                let size = (step[0].abs() + step[1].abs()) * cx + step[2].abs() + step[3].abs();
                for i in 0..4 {
                    t[i] += step[i];
                }
                if t[0].abs() > 0.6 || t[2].abs() > sw as f64 * 0.6 || t[3].abs() > sh as f64 * 0.6
                {
                    return None;
                }
                if size < 0.02 {
                    break;
                }
            }
            m = [t[0], t[1], t[2] * by as f64, t[3] * by as f64];
        }
        let moved = m[0].abs() * w as f64 / 2.0 + m[2].abs() + m[3].abs();
        // a move that explains little is not one
        (moved < 0.4 || first < 1.5 || last < first * 0.78).then_some(m)
    }

    /// How the picture moves from picture `from` to `to` (not counting it).
    fn moves(&self, from: usize, to: usize) -> (Vec<Place>, bool, bool) {
        let stride = (self.fps / 10.0).round().max(1.0) as usize;
        if to <= from + stride * 2 {
            return (Vec::new(), false, false);
        }
        let mut path = vec![(0f64, 0f64, 0f64)]; // ln scale, x, y
        let mut known = 0usize;
        let mut steps = 0usize;
        // each picture is held against one a way back, where a slow move has
        // gone far enough to be seen, and the way on is taken from there
        let mut anchor = (from, (0f64, 0f64, 0f64));
        let mut i = from + stride;
        while i < to {
            steps += 1;
            let last = *path.last().unwrap_or(&(0.0, 0.0, 0.0));
            let on = |base: (f64, f64, f64), m: [f64; 4]| {
                let grow = (1.0 + m[0]).max(0.2);
                (
                    base.0 + grow.ln(),
                    base.1 * grow + m[2] / self.w as f64,
                    base.2 * grow + m[3] / self.h as f64,
                )
            };
            let next = match self.similarity(anchor.0, i) {
                Some(m) => {
                    known += 1;
                    let next = on(anchor.1, m);
                    if m[0].abs() > 0.1
                        || m[2].abs() > self.w as f64 * 0.1
                        || m[3].abs() > self.h as f64 * 0.1
                    {
                        anchor = (i, next);
                    }
                    next
                }
                None => match self.similarity(i - stride, i) {
                    Some(m) => {
                        known += 1;
                        anchor = (i, on(last, m));
                        anchor.1
                    }
                    None => last,
                },
            };
            path.push(next);
            i += stride;
        }
        if steps == 0 || known * 3 < steps * 2 {
            return (Vec::new(), false, false);
        }
        let seconds = (path.len() - 1) as f64 * stride as f64 / self.fps;
        // the shake is what is left when the way is smoothed
        let calm = |of: &dyn Fn(&(f64, f64, f64)) -> f64| -> Vec<f64> {
            (0..path.len())
                .map(|k| {
                    let a = k.saturating_sub(3);
                    let b = (k + 4).min(path.len());
                    path[a..b].iter().map(of).sum::<f64>() / (b - a) as f64
                })
                .collect()
        };
        let (sx, sy) = (calm(&|p| p.1), calm(&|p| p.2));
        let (mut rms, mut turns) = (0f64, 0usize);
        let rest: Vec<(f64, f64)> = path
            .iter()
            .enumerate()
            .map(|(k, p)| (p.1 - sx[k], p.2 - sy[k]))
            .collect();
        for (k, r) in rest.iter().enumerate() {
            rms += r.0 * r.0 + r.1 * r.1;
            if k > 0 && (rest[k - 1].0 * r.0 < 0.0 || rest[k - 1].1 * r.1 < 0.0) {
                turns += 1;
            }
        }
        let rms = (rms / path.len() as f64).sqrt();
        let shakes = seconds > 0.4 && rms > 0.012 && turns as f64 / seconds > 3.0;

        let most = |of: &dyn Fn(&(f64, f64, f64)) -> f64| {
            let v = path.iter().map(of);
            v.clone().fold(f64::MIN, f64::max) - v.fold(f64::MAX, f64::min)
        };
        let (zoomed, panned) = (most(&|p| p.0), most(&|p| p.1).max(most(&|p| p.2)));
        if zoomed < 0.06 && panned < 0.06 {
            return (Vec::new(), false, shakes);
        }
        // the fastest half second of it: a hand does not zoom so
        let half = ((self.fps * 0.5 / stride as f64).round() as usize).clamp(1, path.len() - 1);
        let fastest = (0..path.len() - half)
            .map(|k| (path[k + half].0 - path[k].0).abs())
            .fold(0f64, f64::max);
        let edited = fastest > 0.1 || zoomed > 0.06 && panned < 0.03;
        let points = thinned(&path, 0.02);
        // large enough that the frame stays covered wherever it goes
        let mut least = f64::MAX;
        for k in &points {
            least = least.min(path[*k].0);
        }
        let mut base = 1f64;
        for k in &points {
            let p = path[*k];
            let s = (p.0 - least).exp();
            base = base.max((1.0 + 2.0 * p.1.abs().max(p.2.abs())) / s);
        }
        let base = base.min(1.5);
        let moves = points
            .iter()
            .map(|k| {
                let p = path[*k];
                let scale = ((p.0 - least).exp() * base).min(4.0);
                let room = (scale - 1.0) / 2.0;
                Place {
                    at: *k as f64 / (path.len() - 1) as f64,
                    scale,
                    x: p.1.clamp(-room, room),
                    y: p.2.clamp(-room, room),
                }
            })
            .collect();
        (moves, edited, shakes)
    }

    /// What keeps its place over the cuts.
    fn held(&self, shots: &[Shot]) -> Vec<Held> {
        const CELL: usize = 4;
        let (w, h) = (self.w, self.h);
        let (cw, ch) = (w / CELL, h / CELL);
        let frame = |t: f64| ((t * self.fps).round() as usize).min(self.looks.len() - 1);
        let edge = |p: &[u8], at: usize| {
            (
                i32::from(p[at + 1]) - i32::from(p[at - 1]),
                i32::from(p[at + w]) - i32::from(p[at - w]),
            )
        };
        let same = |a: usize, b: usize, x: usize, y: usize| -> (bool, bool) {
            let at = y * w + x;
            let (pa, pb) = (&self.looks[a].luma, &self.looks[b].luma);
            let (ax, ay) = edge(pa, at);
            let (bx, by) = edge(pb, at);
            let strong = ax.abs() + ay.abs() > 46;
            let alike = (ax - bx).abs() + (ay - by).abs() < 26 && pa[at].abs_diff(pb[at]) < 22;
            (strong, strong && alike)
        };
        // the black bars of a wide picture in a tall frame have edges that stay,
        // and nothing is told by them
        let dark = |y: usize| {
            self.looks
                .iter()
                .step_by(5)
                .filter(|l| l.luma[y * w..(y + 1) * w].iter().all(|v| *v < 26))
                .count()
                * 10
                > self.looks.len().div_ceil(5) * 9
        };
        let top = (0..h / 2).take_while(|y| dark(*y)).count();
        let bottom = h - (h / 2..h).rev().take_while(|y| dark(*y)).count();
        let bar = |cy: usize| {
            (top > 0 && (cy + 1) * CELL + 2 > top.saturating_sub(CELL) && cy * CELL < top + 4)
                || (bottom < h && (cy + 1) * CELL + 4 > bottom && cy * CELL < bottom + CELL + 2)
        };
        let mut found: Vec<Held> = Vec::new();
        for pair in shots.windows(2) {
            let (a, b) = (
                frame(
                    (pair[0].end - pair[1].join_seconds / 2.0 - 2.5 / self.fps).max(pair[0].start),
                ),
                frame(
                    (pair[1].start + pair[1].join_seconds / 2.0 + 2.5 / self.fps).min(pair[1].end),
                ),
            );
            if a >= b {
                continue;
            }
            let (mut busy, mut stays) = (0, 0);
            let mut on = vec![false; cw * ch];
            for cy in (0..ch).filter(|cy| !bar(*cy)) {
                for cx in 0..cw {
                    let (mut strong, mut alike) = (0, 0);
                    for y in (cy * CELL).max(1)..((cy + 1) * CELL).min(h - 1) {
                        for x in (cx * CELL).max(1)..((cx + 1) * CELL).min(w - 1) {
                            let (s, k) = same(a, b, x, y);
                            strong += usize::from(s);
                            alike += usize::from(k);
                        }
                    }
                    let here = alike >= 3 && alike * 5 >= strong * 2;
                    on[cy * cw + cx] = here;
                    busy += usize::from(strong >= 4);
                    stays += usize::from(here);
                }
            }
            // where most of the picture stays, the two are one view, and there is
            // no telling what is over it
            if stays * 2 > busy {
                continue;
            }
            for group in grouped(&on, cw, ch) {
                let (l, t, r, bt) = group;
                let (bw, bh) = (r - l, bt - t);
                let area = (bw * bh) as f64 / (cw * ch) as f64;
                // a border, a speck, or all of it, is not words or a sticker
                if bw * bh < 4
                    || bw < 2
                    || area > 0.45
                    || (bw + 1 >= cw && bh <= 1)
                    || (bh + 1 >= ch && bw <= 1)
                {
                    continue;
                }
                // from where, and until when, it is still there
                let cells = |f: usize, g: usize| {
                    let (mut strong, mut alike) = (0, 0);
                    for y in (t * CELL).max(1)..(bt * CELL).min(h - 1) {
                        for x in (l * CELL).max(1)..(r * CELL).min(w - 1) {
                            if on[(y / CELL) * cw + x / CELL] {
                                let (s, k) = same(f, g, x, y);
                                strong += usize::from(s);
                                alike += usize::from(k);
                            }
                        }
                    }
                    alike * 2 >= strong && alike > 3
                };
                let mut from = a;
                while from > 0 && cells(a, from - 1) {
                    from -= 1;
                }
                let mut to = b;
                while to + 1 < self.looks.len() && cells(b, to + 1) {
                    to += 1;
                }
                let one = Held {
                    start: from as f64 / self.fps,
                    end: (to + 1) as f64 / self.fps,
                    left: l as f64 / cw as f64,
                    top: t as f64 / ch as f64,
                    right: r as f64 / cw as f64,
                    bottom: bt as f64 / ch as f64,
                };
                match found
                    .iter_mut()
                    .find(|other| overlap(other, &one) > 0.3 && one.start <= other.end + 0.1)
                {
                    Some(other) => {
                        other.start = other.start.min(one.start);
                        other.end = other.end.max(one.end);
                        other.left = other.left.min(one.left);
                        other.top = other.top.min(one.top);
                        other.right = other.right.max(one.right);
                        other.bottom = other.bottom.max(one.bottom);
                    }
                    None => found.push(one),
                }
            }
        }
        found.retain(|one| one.end - one.start > 0.3);
        found
    }
}

fn overlap(a: &Held, b: &Held) -> f64 {
    let w = (a.right.min(b.right) - a.left.max(b.left)).max(0.0);
    let h = (a.bottom.min(b.bottom) - a.top.max(b.top)).max(0.0);
    let area = |x: &Held| (x.right - x.left) * (x.bottom - x.top);
    w * h / area(a).min(area(b)).max(1e-9)
}

/// The boxes (left, top, right, bottom) about cells that are on, those within
/// a cell of each other together.
fn grouped(on: &[bool], w: usize, h: usize) -> Vec<(usize, usize, usize, usize)> {
    let mut seen = vec![false; on.len()];
    let mut out = Vec::new();
    for start in 0..on.len() {
        if !on[start] || seen[start] {
            continue;
        }
        let (mut l, mut t, mut r, mut b) = (w, h, 0, 0);
        let mut todo = vec![start];
        seen[start] = true;
        while let Some(at) = todo.pop() {
            let (x, y) = (at % w, at / w);
            l = l.min(x);
            t = t.min(y);
            r = r.max(x + 1);
            b = b.max(y + 1);
            // words have gaps between them across, lines a smaller one down
            for ny in y.saturating_sub(1)..(y + 2).min(h) {
                for nx in x.saturating_sub(3)..(x + 4).min(w) {
                    let next = ny * w + nx;
                    if on[next] && !seen[next] {
                        seen[next] = true;
                        todo.push(next);
                    }
                }
            }
        }
        out.push((l, t, r, b));
    }
    out
}

/// The points of a path that tell it to within `slack`, its ends among them
/// and six at the most.
fn thinned(path: &[(f64, f64, f64)], slack: f64) -> Vec<usize> {
    let mut kept = vec![0, path.len() - 1];
    while kept.len() < 6 {
        let mut worst = (slack, 0usize);
        for pair in kept.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            for k in a + 1..b {
                let u = (k - a) as f64 / (b - a) as f64;
                let off = |of: fn(&(f64, f64, f64)) -> f64| {
                    (of(&path[k]) - (of(&path[a]) * (1.0 - u) + of(&path[b]) * u)).abs()
                };
                let d = off(|p| p.0).max(off(|p| p.1)).max(off(|p| p.2));
                if d > worst.0 {
                    worst = (d, k);
                }
            }
        }
        if worst.1 == 0 {
            break;
        }
        kept.push(worst.1);
        kept.sort_unstable();
    }
    kept
}

/// Solves four equations.
fn solve(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> Option<[f64; 4]> {
    for i in 0..4 {
        let p = (i..4).max_by(|x, y| a[*x][i].abs().total_cmp(&a[*y][i].abs()))?;
        if a[p][i].abs() < 1e-9 {
            return None;
        }
        a.swap(i, p);
        b.swap(i, p);
        for r in i + 1..4 {
            let f = a[r][i] / a[i][i];
            for c in i..4 {
                a[r][c] -= f * a[i][c];
            }
            b[r] -= f * b[i];
        }
    }
    let mut out = [0f64; 4];
    for i in (0..4).rev() {
        let mut v = b[i];
        for c in i + 1..4 {
            v -= a[i][c] * out[c];
        }
        out[i] = v / a[i][i];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 72;
    const H: usize = 128;

    /// A made-up picture, smooth and full of shapes, seen `scale` times as
    /// large from (`x`, `y`).
    fn scene(seed: u32, scale: f64, x: f64, y: f64) -> Vec<u8> {
        let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(12345);
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            f64::from(s) / f64::from(u32::MAX)
        };
        let blobs: Vec<[f64; 6]> = (0..40)
            .map(|_| [next(), next(), 0.04 + next() * 0.12, next(), next(), next()])
            .collect();
        let base = [next(), next(), next()];
        let mut out = vec![0u8; W * H * 3];
        for py in 0..H {
            for px in 0..W {
                let u = ((px as f64 / W as f64) - 0.5 - x) / scale + 0.5;
                let v = ((py as f64 / H as f64) - 0.5 - y) / scale + 0.5;
                let mut c = base;
                for b in &blobs {
                    let d = ((u - b[0]).powi(2) + ((v - b[1]) * 1.78).powi(2)).sqrt();
                    let k = (1.0 - (d - b[2]) * 40.0).clamp(0.0, 1.0);
                    for i in 0..3 {
                        c[i] = c[i] * (1.0 - k) + b[3 + i] * k;
                    }
                }
                for i in 0..3 {
                    out[(py * W + px) * 3 + i] = (c[i] * 255.0) as u8;
                }
            }
        }
        out
    }

    fn mix(a: &[u8], b: &[u8], t: f64) -> Vec<u8> {
        a.iter()
            .zip(b)
            .map(|(a, b)| (f64::from(*a) * (1.0 - t) + f64::from(*b) * t) as u8)
            .collect()
    }

    fn read(frames: &[Vec<u8>]) -> Reading {
        let mut reader = Reader::new(W, H, 30.0);
        for frame in frames {
            reader.push(frame, 3);
        }
        reader.finish()
    }

    #[test]
    fn cuts_are_found_to_the_picture() {
        let mut frames = Vec::new();
        for (seed, n) in [(1, 20), (2, 33), (3, 12), (4, 40)] {
            for i in 0..n {
                frames.push(scene(seed, 1.0, i as f64 * 0.002, 0.0));
            }
        }
        let got = read(&frames);
        let starts: Vec<usize> = got
            .shots
            .iter()
            .map(|s| (s.start * 30.0).round() as usize)
            .collect();
        assert_eq!(starts, [0, 20, 53, 65]);
        assert!(got.shots.iter().all(|s| s.join == Join::Cut));
    }

    #[test]
    fn dissolves_and_fades_are_told_from_cuts() {
        let mut frames = Vec::new();
        for _ in 0..25 {
            frames.push(scene(1, 1.0, 0.0, 0.0));
        }
        for t in 1..16 {
            frames.push(mix(
                &scene(1, 1.0, 0.0, 0.0),
                &scene(2, 1.0, 0.0, 0.0),
                f64::from(t) / 16.0,
            ));
        }
        for _ in 0..25 {
            frames.push(scene(2, 1.0, 0.0, 0.0));
        }
        let black = vec![0u8; W * H * 3];
        for t in 1..8 {
            frames.push(mix(&scene(2, 1.0, 0.0, 0.0), &black, f64::from(t) / 8.0));
        }
        frames.push(black.clone());
        frames.push(black.clone());
        for t in 1..8 {
            frames.push(mix(&black, &scene(3, 1.0, 0.0, 0.0), f64::from(t) / 8.0));
        }
        for _ in 0..25 {
            frames.push(scene(3, 1.0, 0.0, 0.0));
        }
        let got = read(&frames);
        let joins: Vec<Join> = got.shots.iter().map(|s| s.join).collect();
        assert_eq!(joins, [Join::Cut, Join::Dissolve, Join::Black], "{got:?}");
        assert!((got.shots[1].start * 30.0 - 32.5).abs() < 3.0, "{got:?}");
        assert!((0.3..0.8).contains(&got.shots[1].join_seconds), "{got:?}");
    }

    #[test]
    fn a_change_of_light_is_not_a_dissolve_and_a_flash_is_not_a_cut() {
        let plain = scene(5, 1.0, 0.0, 0.0);
        let dim: Vec<u8> = plain.iter().map(|v| (f64::from(*v) * 0.45) as u8).collect();
        let mut frames = vec![plain.clone(); 20];
        for t in 1..16 {
            frames.push(mix(&plain, &dim, f64::from(t) / 16.0));
        }
        frames.extend(vec![dim.clone(); 20]);
        let bright: Vec<u8> = dim.iter().map(|v| v.saturating_add(150)).collect();
        frames.push(bright.clone());
        frames.push(bright);
        frames.extend(vec![dim; 20]);
        let got = read(&frames);
        assert_eq!(got.shots.len(), 1, "{got:?}");
        assert_eq!(got.flashes.len(), 1, "{got:?}");
    }

    #[test]
    fn a_zoom_and_a_pan_are_read() {
        let zoom: Vec<_> = (0..45)
            .map(|i| scene(7, 1.0 + i as f64 * 0.3 / 44.0, 0.0, 0.0))
            .collect();
        let got = read(&zoom);
        let moves = &got.shots[0].moves;
        assert!(moves.len() >= 2, "{got:?}");
        let grew = moves.last().map_or(0.0, |m| m.scale) / moves[0].scale;
        assert!((1.2..1.4).contains(&grew), "{grew} {moves:?}");
        assert!(got.shots[0].edited);
        let pan: Vec<_> = (0..45)
            .map(|i| scene(8, 1.4, -0.1 + i as f64 * 0.2 / 44.0, 0.0))
            .collect();
        let got = read(&pan);
        let moves = &got.shots[0].moves;
        let went = moves.last().map_or(0.0, |m| m.x) - moves[0].x;
        assert!((0.1..0.28).contains(&went), "{went} {moves:?}");
        // wherever it goes the frame is covered
        for m in moves {
            assert!(m.x.abs() <= (m.scale - 1.0) / 2.0 + 1e-9);
        }
    }

    #[test]
    fn a_still_shot_does_not_move_and_a_shaken_one_shakes() {
        let still = vec![scene(9, 1.0, 0.0, 0.0); 40];
        let got = read(&still);
        assert!(got.shots[0].moves.is_empty() && !got.shots[0].shakes);
        let shaken: Vec<_> = (0..45)
            .map(|i| scene(9, 1.15, if i / 3 % 2 == 0 { 0.03 } else { -0.03 }, 0.0))
            .collect();
        assert!(read(&shaken).shots[0].shakes);
    }

    #[test]
    fn words_over_the_cuts_are_found() {
        let label = |frame: &mut Vec<u8>| {
            for y in 90..104 {
                for x in 12..60 {
                    // strokes with gaps, as letters have
                    if (x / 3) % 2 == 0 && (y / 4) % 2 == 0 || y == 90 || y == 103 {
                        for c in 0..3 {
                            frame[(y * W + x) * 3 + c] = 255;
                        }
                    } else {
                        for c in 0..3 {
                            frame[(y * W + x) * 3 + c] = 0;
                        }
                    }
                }
            }
        };
        let mut frames = Vec::new();
        for (seed, n, with) in [
            (1, 25, false),
            (2, 25, true),
            (3, 25, true),
            (4, 25, true),
            (6, 25, false),
        ] {
            for _ in 0..n {
                let mut f = scene(seed, 1.0, 0.0, 0.0);
                if with {
                    label(&mut f);
                }
                frames.push(f);
            }
        }
        let got = read(&frames);
        assert_eq!(got.shots.len(), 5, "{got:?}");
        assert_eq!(got.held.len(), 1, "{:?}", got.held);
        let held = got.held[0];
        assert!(
            (held.start * 30.0 - 25.0).abs() < 2.0 && (held.end * 30.0 - 100.0).abs() < 2.0,
            "{held:?}"
        );
        assert!(
            held.top > 0.6 && held.bottom < 0.9 && held.left < 0.25 && held.right > 0.75,
            "{held:?}"
        );
    }
}
