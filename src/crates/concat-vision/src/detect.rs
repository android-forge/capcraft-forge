// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! The things in a picture - people first - each with a box round it, found
//! by YOLOX (Apache-2.0, Megvii): eighty everyday kinds, the COCO ones.

use std::path::Path;

use concat_core::frame::Frame;

use crate::runtime::{Input, Model};
use crate::window::{self, Rect};

/// The model's square.
const SIDE: usize = 640;
/// What fills the square beyond the picture.
const PAD: f32 = 114.0;
/// A thing less sure than this is not one. People are wanted more than
/// things are, so less is asked of them.
const SURE_PERSON: f32 = 0.35;
const SURE_THING: f32 = 0.45;
/// Two boxes of one kind that overlap more than this are one thing.
const SAME: f32 = 0.5;

/// The kinds the model knows, in its order.
const KINDS: [&str; 80] = [
    "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck", "boat",
    "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat", "dog",
    "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack", "umbrella",
    "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "ball", "kite", "baseball bat",
    "baseball glove", "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup",
    "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange", "broccoli",
    "carrot", "hot dog", "pizza", "donut", "cake", "chair", "couch", "potted plant", "bed",
    "dining table", "toilet", "tv", "laptop", "mouse", "remote", "keyboard", "phone", "microwave",
    "oven", "toaster", "sink", "refrigerator", "book", "clock", "vase", "scissors", "teddy bear",
    "hair drier", "toothbrush",
];

/// A thing found: what it is, how sure the model is, and its box in fractions
/// of the picture.
#[derive(Clone, PartialEq, Debug)]
pub struct Found {
    /// What it is ("person", "dog").
    pub kind: &'static str,
    /// How sure the model is, 0 to 1.
    pub score: f32,
    /// The box's left edge.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
}

/// The detector, loaded.
pub struct Detector {
    model: Model,
}

impl Detector {
    /// Loads the model from its downloaded file.
    pub fn load(file: &Path) -> Result<Detector, String> {
        Ok(Detector {
            model: Model::from_file(file)?,
        })
    }

    /// The things in `frame`, people first, then the surest.
    pub fn find(&mut self, frame: &Frame) -> Result<Vec<Found>, String> {
        let (fw, fh) = (frame.width() as f32, frame.height() as f32);
        if fw < 1.0 || fh < 1.0 {
            return Err("detect: an empty frame".to_owned());
        }
        let scale = (SIDE as f32 / fw).min(SIDE as f32 / fh);
        let (w, h) = (
            ((fw * scale) as usize).clamp(1, SIDE),
            ((fh * scale) as usize).clamp(1, SIDE),
        );
        let whole = Rect {
            x: 0.0,
            y: 0.0,
            width: fw,
            height: fh,
        };
        let small = window::rgb(frame, whole, w, h);
        // blue, green, red, as it was taught, each 0 to 255
        let mut data = vec![PAD; 3 * SIDE * SIDE];
        for y in 0..h {
            for x in 0..w {
                for c in 0..3 {
                    data[c * SIDE * SIDE + y * SIDE + x] =
                        f32::from(small[(y * w + x) * 3 + 2 - c]);
                }
            }
        }
        let answer = self
            .model
            .run(
                vec![Input {
                    name: "images",
                    dims: vec![1, 3, SIDE, SIDE],
                    data: data.into(),
                }],
                &["output"],
            )?
            .pop()
            .ok_or("detect: no answer")?;
        Ok(read(&answer.data, scale, fw, fh))
    }
}

/// The model's answer - five numbers and eighty likelihoods for each place it
/// looked - as things in fractions of a picture `fw` by `fh`.
fn read(answer: &[f32], scale: f32, fw: f32, fh: f32) -> Vec<Found> {
    let row = 5 + KINDS.len();
    let mut all: Vec<(usize, Found)> = Vec::new();
    let mut index = 0;
    for stride in [8usize, 16, 32] {
        let side = SIDE / stride;
        for gy in 0..side {
            for gx in 0..side {
                let Some(r) = answer.get(index * row..(index + 1) * row) else {
                    return nms(all);
                };
                index += 1;
                if r[4] < SURE_PERSON {
                    continue;
                }
                let (kind, likely) = r[5..]
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map_or((0, 0.0), |(kind, likely)| (kind, *likely));
                let score = r[4] * likely;
                if score < if kind == 0 { SURE_PERSON } else { SURE_THING } {
                    continue;
                }
                let s = stride as f32;
                let (cx, cy) = ((r[0] + gx as f32) * s, (r[1] + gy as f32) * s);
                let (w, h) = (r[2].exp() * s, r[3].exp() * s);
                let left = ((cx - w / 2.0) / scale / fw).clamp(0.0, 1.0);
                let top = ((cy - h / 2.0) / scale / fh).clamp(0.0, 1.0);
                let right = ((cx + w / 2.0) / scale / fw).clamp(0.0, 1.0);
                let bottom = ((cy + h / 2.0) / scale / fh).clamp(0.0, 1.0);
                if right - left < 0.005 || bottom - top < 0.005 {
                    continue;
                }
                all.push((
                    kind,
                    Found {
                        kind: KINDS[kind],
                        score,
                        x: left,
                        y: top,
                        width: right - left,
                        height: bottom - top,
                    },
                ));
            }
        }
    }
    nms(all)
}

fn overlap(a: &Found, b: &Found) -> f32 {
    let w = ((a.x + a.width).min(b.x + b.width) - a.x.max(b.x)).max(0.0);
    let h = ((a.y + a.height).min(b.y + b.height) - a.y.max(b.y)).max(0.0);
    let both = w * h;
    both / (a.width * a.height + b.width * b.height - both).max(1e-9)
}

/// The surest of every thing found more than once, people first.
fn nms(mut all: Vec<(usize, Found)>) -> Vec<Found> {
    all.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));
    let mut kept: Vec<(usize, Found)> = Vec::new();
    for (kind, found) in all {
        if !kept
            .iter()
            .any(|(k, f)| *k == kind && overlap(f, &found) > SAME)
        {
            kept.push((kind, found));
        }
    }
    kept.sort_by_key(|(kind, _)| usize::from(*kind != 0));
    kept.into_iter().map(|(_, found)| found).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(kind: usize, score: f32, x: f32) -> (usize, Found) {
        (
            kind,
            Found {
                kind: KINDS[kind],
                score,
                x,
                y: 0.1,
                width: 0.2,
                height: 0.5,
            },
        )
    }

    #[test]
    fn a_thing_found_twice_is_one_and_people_come_first() {
        let kept = nms(vec![
            found(16, 0.95, 0.1),
            found(0, 0.6, 0.5),
            found(0, 0.8, 0.52),
            found(0, 0.7, 0.1),
        ]);
        let read: Vec<(&str, f32)> = kept.iter().map(|f| (f.kind, f.score)).collect();
        assert_eq!(read, vec![("person", 0.8), ("person", 0.7), ("dog", 0.95)]);
    }

    #[test]
    fn a_place_the_model_is_sure_of_is_a_box_in_the_picture() {
        let row = 5 + KINDS.len();
        let mut answer = vec![0f32; 8400 * row];
        // the first grid, 10 across and 20 down: centre (84, 164), 80 by 160
        let r = &mut answer[(20 * 80 + 10) * row..][..row];
        r[0] = 0.5;
        r[1] = 0.5;
        r[2] = (10f32).ln();
        r[3] = (20f32).ln();
        r[4] = 0.9;
        r[5] = 0.9;
        let all = read(&answer, 0.5, 1280.0, 1280.0);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].kind, "person");
        assert!((all[0].x - 44.0 / 640.0).abs() < 1e-4, "{:?}", all[0]);
        assert!((all[0].height - 160.0 / 640.0).abs() < 1e-4);
    }
}
