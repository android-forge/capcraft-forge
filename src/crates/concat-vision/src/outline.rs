// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! The outline of a thing whose box is known, or that a point is on: the
//! brush model's answer, asked of a close-up so a small thing has an edge,
//! in more than one way, and the one that fits the box taken.

use concat_core::frame::Frame;

use crate::Mask;
use crate::brush::{Brush, Embedding};
use crate::detect::Found;
use crate::window::{self, Rect};

/// An outline: a mask over a part of the picture, in fractions of it.
#[derive(Clone, Debug)]
pub struct Outline {
    /// The part's left edge.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
    /// The thing in it.
    pub mask: Mask,
}

/// How much round a thing's box the close-up shows, of its longer side.
const ROUND: f32 = 0.35;
/// How far past its box an outline may reach, of its longer side.
const REACH: f32 = 0.04;
/// Answers that fit nearly as well as the best are joined to it: one often
/// has the arm or the bag another leaves out.
const NEARLY: f32 = 0.82;
/// A pixel this likely is the thing's.
const IN: u8 = 128;

/// Where a person is asked after, in their box: down the middle, for one
/// standing; and lower for one cut off at the waist.
const PERSON: [&[[f32; 2]]; 4] = [
    &[[0.5, 0.12], [0.5, 0.35], [0.5, 0.6]],
    &[[0.5, 0.3], [0.5, 0.5]],
    &[
        [0.5, 0.12],
        [0.5, 0.3],
        [0.4, 0.5],
        [0.6, 0.5],
        [0.42, 0.85],
        [0.58, 0.85],
    ],
    &[[0.5, 0.45]],
];
/// Where any other thing is.
const THING: [&[[f32; 2]]; 3] = [
    &[[0.5, 0.5]],
    &[[0.5, 0.3], [0.5, 0.7], [0.3, 0.5], [0.7, 0.5]],
    &[[0.35, 0.35], [0.65, 0.65], [0.35, 0.65], [0.65, 0.35]],
];

/// The outline of `found` in `frame`.
pub fn of(brush: &Brush, frame: &Frame, found: &Found) -> Result<Outline, String> {
    let (fw, fh) = (frame.width() as f32, frame.height() as f32);
    let (bx, by) = (found.x * fw, found.y * fh);
    let (bw, bh) = (found.width * fw, found.height * fh);
    let long = bw.max(bh);
    let left = (bx - long * ROUND).max(0.0);
    let top = (by - long * ROUND).max(0.0);
    let right = (bx + bw + long * ROUND).min(fw);
    let bottom = (by + bh + long * ROUND).min(fh);
    let (w, h) = ((right - left).max(1.0), (bottom - top).max(1.0));
    let close = Rect {
        x: left,
        y: top,
        width: w,
        height: h,
    };
    let embedding = read(brush, frame, close)?;
    // the box in the close-up's fractions
    let inside = [
        (bx - left) / w,
        (by - top) / h,
        (bx + bw - left) / w,
        (by + bh - top) / h,
    ];
    let person = found.kind == "person";
    let asked: Vec<&[[f32; 2]]> = if person {
        PERSON.to_vec()
    } else {
        THING.to_vec()
    };
    // just outside the box, to say what is not the thing
    let (mx, my) = (0.06 * long / w, 0.06 * long / h);
    let mid = (inside[1] + inside[3]) / 2.0;
    let not: Vec<[f64; 2]> = [
        [inside[0] - mx, inside[1] - my],
        [inside[2] + mx, inside[1] - my],
        [inside[0] - mx, mid],
        [inside[2] + mx, mid],
    ]
    .into_iter()
    .filter(|p| p.iter().all(|v| (0.0..1.0).contains(v)))
    .map(|p| [f64::from(p[0]), f64::from(p[1])])
    .collect();
    let bounds = [
        inside[0] - REACH * long / w,
        inside[1] - REACH * long / h,
        inside[2] + REACH * long / w,
        inside[3] + REACH * long / h,
    ];
    let mut answers: Vec<(f32, Mask)> = Vec::new();
    for points in asked {
        let points: Vec<[f64; 2]> = points
            .iter()
            .map(|[u, v]| {
                [
                    f64::from(inside[0] + u * (inside[2] - inside[0])),
                    f64::from(inside[1] + v * (inside[3] - inside[1])),
                ]
            })
            .collect();
        for (score, mask) in brush.answers(&embedding, &points, &not)? {
            if let Some((fit, held)) = fitted(mask, score, inside, bounds) {
                answers.push((fit, held));
            }
        }
    }
    let best = answers
        .iter()
        .map(|(fit, _)| *fit)
        .fold(f32::MIN, f32::max);
    let mut joined: Option<Mask> = None;
    for (fit, mask) in answers {
        if fit < best * NEARLY {
            continue;
        }
        match &mut joined {
            None => joined = Some(mask),
            Some(whole) => {
                for (a, b) in whole.bytes_mut().iter_mut().zip(mask.bytes()) {
                    *a = (*a).max(*b);
                }
            }
        }
    }
    let mask = joined.ok_or("Its outline could not be found.")?;
    Ok(Outline {
        x: left / fw,
        y: top / fh,
        width: w / fw,
        height: h / fh,
        mask: whole(mask),
    })
}

/// The whole picture read once, for outlines by point.
pub fn read_whole(brush: &Brush, frame: &Frame) -> Result<Embedding, String> {
    read(
        brush,
        frame,
        Rect {
            x: 0.0,
            y: 0.0,
            width: frame.width() as f32,
            height: frame.height() as f32,
        },
    )
}

/// The outline of what is at `point`, fractions of a picture read by
/// [`read_whole`]: the surest answer that is not most of the picture.
pub fn at(brush: &Brush, embedding: &Embedding, point: [f64; 2]) -> Result<Outline, String> {
    let mut answers = brush.answers(embedding, &[point], &[])?;
    answers.sort_by(|a, b| b.0.total_cmp(&a.0));
    let share = |mask: &Mask| {
        mask.bytes().iter().filter(|b| **b >= IN).count() as f32
            / mask.bytes().len().max(1) as f32
    };
    let at = answers
        .iter()
        .position(|(_, mask)| (0.0002..0.6).contains(&share(mask)))
        .ok_or("Nothing was found there.")?;
    let (_, mask) = answers.swap_remove(at);
    Ok(Outline {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
        mask: whole(mask),
    })
}

fn read(brush: &Brush, frame: &Frame, rect: Rect) -> Result<Embedding, String> {
    let (w, h) = Brush::input_size(
        rect.width.round().max(1.0) as u32,
        rect.height.round().max(1.0) as u32,
    );
    brush.embed(&window::frame(frame, rect, w, h))
}

/// `mask` held to `bounds`, and how well it fits the box `inside`: how much
/// of the box's area its own box shares, less what it spills, by how sure the
/// model is. None for one with next to nothing in the box.
fn fitted(mut mask: Mask, score: f32, inside: [f32; 4], bounds: [f32; 4]) -> Option<(f32, Mask)> {
    let (w, h) = (mask.width() as usize, mask.height() as usize);
    let (mut all, mut held) = (0usize, 0usize);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    let bytes = mask.bytes_mut();
    for y in 0..h {
        let v = (y as f32 + 0.5) / h as f32;
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let byte = &mut bytes[y * w + x];
            let on = *byte >= IN;
            all += usize::from(on);
            let within = u >= bounds[0] && u <= bounds[2] && v >= bounds[1] && v <= bounds[3];
            if !within {
                *byte = 0;
            } else if on {
                held += 1;
                (x0, y0) = (x0.min(x), y0.min(y));
                (x1, y1) = (x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if held < 20 {
        return None;
    }
    let own = [
        x0 as f32 / w as f32,
        y0 as f32 / h as f32,
        x1 as f32 / w as f32,
        y1 as f32 / h as f32,
    ];
    let area = |b: [f32; 4]| ((b[2] - b[0]) * (b[3] - b[1])).max(1e-9);
    let both = (own[2].min(inside[2]) - own[0].max(inside[0])).max(0.0)
        * (own[3].min(inside[3]) - own[1].max(inside[1])).max(0.0);
    let shared = both / (area(own) + area(inside) - both);
    let spill = held as f32 / all.max(1) as f32;
    let filled = held as f32 / (w * h) as f32 / area(inside);
    // a person fills between a quarter and nine tenths of their box
    let shape = if (0.2..0.92).contains(&filled) {
        1.0
    } else {
        0.6
    };
    Some((
        shared * spill * (0.5 + 0.5 * score.clamp(0.0, 1.0)) * shape,
        mask,
    ))
}

/// `mask` with what it goes all the way round filled in: a bag against a
/// coat, the gap the model leaves in a dark sleeve. A wide gap - between an
/// arm and a side - is left.
fn whole(mut mask: Mask) -> Mask {
    let (w, h) = (mask.width() as usize, mask.height() as usize);
    let held = mask.bytes().iter().filter(|b| **b >= IN).count();
    let small = (held / 60).max(12);
    let mut seen: Vec<bool> = mask.bytes().iter().map(|b| *b >= IN).collect();
    for start in 0..w * h {
        if seen[start] {
            continue;
        }
        let mut gap = vec![start];
        let mut edge = false;
        seen[start] = true;
        let mut next = 0;
        while next < gap.len() {
            let (x, y) = (gap[next] % w, gap[next] / w);
            next += 1;
            edge |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
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
        if !edge && gap.len() <= small {
            for at in gap {
                mask.bytes_mut()[at] = 255;
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disc(side: u32, hole: u32) -> Mask {
        let mut mask = Mask::filled(side, side, 0);
        let middle = side as f32 / 2.0;
        for y in 0..side {
            for x in 0..side {
                let r = ((x as f32 + 0.5 - middle).powi(2) + (y as f32 + 0.5 - middle).powi(2)).sqrt();
                if r < middle * 0.8 && r >= hole as f32 {
                    mask.bytes_mut()[(y * side + x) as usize] = 255;
                }
            }
        }
        mask
    }

    #[test]
    fn a_small_hole_is_filled_and_a_wide_gap_is_left() {
        let middle = |mask: &Mask| mask.at(32, 32);
        assert_eq!(middle(&whole(disc(64, 3))), 255);
        assert_eq!(middle(&whole(disc(64, 18))), 0);
    }

    #[test]
    fn an_answer_that_fits_the_box_beats_one_that_spills() {
        let inside = [0.1, 0.1, 0.9, 0.9];
        let good = fitted(disc(64, 0), 0.9, inside, [0.05, 0.05, 0.95, 0.95]).expect("fits").0;
        let spill = fitted(Mask::filled(64, 64, 255), 0.9, inside, [0.05, 0.05, 0.95, 0.95])
            .expect("fits")
            .0;
        assert!(good > spill, "{good} {spill}");
        assert!(fitted(Mask::filled(64, 64, 0), 0.9, inside, inside).is_none());
    }
}
