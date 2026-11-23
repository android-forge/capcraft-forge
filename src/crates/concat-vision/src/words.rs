// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Words in a picture, read: PaddleOCR's (Apache-2.0) PP-OCRv3 finds the
//! lines and its English PP-OCRv5 reads them, both small and on the phone.

use std::path::Path;

use concat_core::frame::Frame;

use crate::runtime::{Input, Model};
use crate::window::{self, Rect};

/// The letters the reader knows, in its order after the blank; a space
/// follows them.
const LETTERS: &str = include_str!("words-en.txt");

/// The longest side the line finder looks at.
const FINDER: usize = 960;

/// The reader's height for a line.
const LINE: usize = 48;

/// A line of words found: what it says, how sure the reader is (0 to 1),
/// and its box in pixels of the picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// What it says.
    pub text: String,
    /// How sure the reader is.
    pub sure: f32,
    /// Its box.
    pub rect: Rect,
}

/// The finder and the reader, loaded.
pub struct Words {
    find: Model,
    read: Model,
    letters: Vec<String>,
}

impl Words {
    /// Loads the two from their files.
    pub fn load(finder: &Path, reader: &Path) -> Result<Words, String> {
        let mut letters: Vec<String> = vec![String::new()];
        letters.extend(LETTERS.split('\n').filter(|l| !l.is_empty()).map(str::to_owned));
        letters.push(" ".to_owned());
        Ok(Words {
            find: Model::from_file_on_cpu(finder)?,
            read: Model::from_file_on_cpu(reader)?,
            letters,
        })
    }

    /// The lines of words in `frame`, top to bottom.
    pub fn lines(&mut self, frame: &Frame) -> Result<Vec<Line>, String> {
        let boxes = self.boxes(frame)?;
        let mut out = Vec::new();
        for rect in boxes {
            let (text, sure) = self.read(frame, rect)?;
            if !text.trim().is_empty() {
                out.push(Line { text: text.trim().to_owned(), sure, rect });
            }
        }
        Ok(out)
    }

    /// Where the lines are.
    fn boxes(&mut self, frame: &Frame) -> Result<Vec<Rect>, String> {
        let (fw, fh) = (frame.width() as f32, frame.height() as f32);
        if fw < 8.0 || fh < 8.0 {
            return Ok(Vec::new());
        }
        let k = (FINDER as f32 / fw.max(fh)).min(1.0);
        let w = (((fw * k) as usize) / 32 * 32).max(32);
        let h = (((fh * k) as usize) / 32 * 32).max(32);
        let rgb = window::rgb(frame, Rect { x: 0.0, y: 0.0, width: fw, height: fh }, w, h);
        let (mean, spread) = ([0.485f32, 0.456, 0.406], [0.229f32, 0.224, 0.225]);
        let mut data = vec![0f32; 3 * w * h];
        for i in 0..w * h {
            for c in 0..3 {
                data[c * w * h + i] = (f32::from(rgb[i * 3 + c]) / 255.0 - mean[c]) / spread[c];
            }
        }
        let answer = self
            .find
            .run(vec![Input { name: "x", dims: vec![1, 3, h, w], data: data.into() }], &["sigmoid_0.tmp_0"])?
            .pop()
            .ok_or("words: no answer from the finder")?;
        if answer.data.len() < w * h {
            return Err("words: the finder answered short".to_owned());
        }
        // the likely places, grown a little, are the lines
        let on: Vec<bool> = answer.data[..w * h].iter().map(|p| *p > 0.3).collect();
        let mut grown = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                if on[y * w + x] {
                    for gy in y.saturating_sub(2)..(y + 3).min(h) {
                        for gx in x.saturating_sub(2)..(x + 3).min(w) {
                            grown[gy * w + gx] = true;
                        }
                    }
                }
            }
        }
        let mut seen = vec![false; w * h];
        let mut out = Vec::new();
        for start in 0..w * h {
            if !grown[start] || seen[start] {
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
                let mut near = Vec::with_capacity(4);
                if x > 0 {
                    near.push(at - 1);
                }
                if x + 1 < w {
                    near.push(at + 1);
                }
                if y > 0 {
                    near.push(at - w);
                }
                if y + 1 < h {
                    near.push(at + w);
                }
                for n in near {
                    if grown[n] && !seen[n] {
                        seen[n] = true;
                        todo.push(n);
                    }
                }
            }
            if b - t < 4 || r - l < 6 {
                continue;
            }
            let (sx, sy) = (fw / w as f32, fh / h as f32);
            out.push(Rect {
                x: l as f32 * sx,
                y: t as f32 * sy,
                width: (r - l) as f32 * sx,
                height: (b - t) as f32 * sy,
            });
        }
        out.sort_by(|a, b| (a.y, a.x).partial_cmp(&(b.y, b.x)).unwrap_or(std::cmp::Ordering::Equal));
        Ok(out)
    }

    /// What the line in `rect` of `frame` says, and how sure the reader is.
    fn read(&mut self, frame: &Frame, rect: Rect) -> Result<(String, f32), String> {
        let pad = rect.height * 0.15;
        let rect = Rect {
            x: (rect.x - pad).max(0.0),
            y: (rect.y - pad).max(0.0),
            width: (rect.width + pad * 2.0).min(frame.width() as f32),
            height: (rect.height + pad * 2.0).min(frame.height() as f32),
        };
        let w = ((rect.width * LINE as f32 / rect.height.max(1.0)) as usize).clamp(16, 1600);
        let rgb = window::rgb(frame, rect, w, LINE);
        let mut data = vec![0f32; 3 * w * LINE];
        for i in 0..w * LINE {
            for c in 0..3 {
                data[c * w * LINE + i] = (f32::from(rgb[i * 3 + c]) / 255.0 - 0.5) / 0.5;
            }
        }
        let answer = self
            .read
            .run(vec![Input { name: "x", dims: vec![1, 3, LINE, w], data: data.into() }], &["fetch_name_0"])?
            .pop()
            .ok_or("words: no answer from the reader")?;
        let kinds = *answer.dims.last().unwrap_or(&0);
        if kinds == 0 {
            return Ok((String::new(), 0.0));
        }
        Ok(decode(&answer.data, kinds, &self.letters))
    }
}

/// The reader's answer, a row of likelihoods for each step along the line, as
/// the letters it most likely says: each run of one letter once, the blank
/// between them none.
fn decode(answer: &[f32], kinds: usize, letters: &[String]) -> (String, f32) {
    let (mut text, mut sure, mut count, mut last) = (String::new(), 0f32, 0usize, 0usize);
    for row in answer.chunks_exact(kinds) {
        let (best, p) = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or((0, 0.0), |(i, p)| (i, *p));
        if best != 0 && best != last {
            if let Some(letter) = letters.get(best) {
                text.push_str(letter);
                sure += p;
                count += 1;
            }
        }
        last = best;
    }
    (text, if count == 0 { 0.0 } else { sure / count as f32 })
}

/// The colour of the words in `rect` of `frame` (`#rrggbb`), and of an
/// outline round them where they have one.
pub fn colours(frame: &Frame, rect: Rect) -> (String, Option<String>) {
    let (w, h) = ((rect.width as usize).clamp(4, 400), (rect.height as usize).clamp(4, 120));
    let rgb = window::rgb(frame, rect, w, h);
    let luma: Vec<f32> = rgb.chunks_exact(3).map(|p| 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])).collect();
    // the box's edge is what is behind the words; the words are what is least like it
    let mut edge: Vec<f32> = (0..w).flat_map(|x| [luma[x], luma[(h - 1) * w + x]]).collect();
    edge.sort_by(f32::total_cmp);
    let behind = edge[edge.len() / 2];
    let mut far: Vec<(f32, usize)> = luma.iter().enumerate().map(|(i, l)| ((l - behind).abs(), i)).collect();
    far.sort_by(|a, b| b.0.total_cmp(&a.0));
    let take = (far.len() / 8).max(1);
    let mut sum = [0f32; 3];
    for (_, i) in &far[..take] {
        for c in 0..3 {
            sum[c] += f32::from(rgb[i * 3 + c]);
        }
    }
    let ink = sum.map(|v| (v / take as f32).round() as u8);
    let ink_light = 0.299 * f32::from(ink[0]) + 0.587 * f32::from(ink[1]) + 0.114 * f32::from(ink[2]);
    // light words with a dark line close round them, or dark ones with a light line
    let mut against = 0usize;
    let mut near = 0usize;
    for (_, i) in &far[..take] {
        let (x, y) = (i % w, i / w);
        for (dx, dy) in [(2i64, 0i64), (-2, 0), (0, 2), (0, -2)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let l = luma[ny as usize * w + nx as usize];
            near += 1;
            if (l - ink_light).abs() > 110.0 && (l - behind).abs() > 25.0 {
                against += 1;
            }
        }
    }
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let outline = (near > 0 && against * 12 > near).then(|| if ink_light > 128.0 { "#000000".to_owned() } else { "#ffffff".to_owned() });
    (hex(ink), outline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_read_from_the_likeliest_letters() {
        let letters: Vec<String> = ["", "A", "B", " "].iter().map(|s| (*s).to_owned()).collect();
        // A A blank B B space A
        let rows = [
            [0.1, 0.8, 0.05, 0.05],
            [0.1, 0.9, 0.0, 0.0],
            [0.9, 0.05, 0.05, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.1, 0.9, 0.0],
            [0.1, 0.0, 0.0, 0.9],
            [0.0, 0.7, 0.0, 0.3],
        ];
        let flat: Vec<f32> = rows.iter().flatten().copied().collect();
        let (text, sure) = decode(&flat, 4, &letters);
        assert_eq!(text, "AB A");
        assert!(sure > 0.7);
    }

    /// With the two models and a picture of words at hand (`WORDS_DIR`):
    /// `cargo test -p concat-vision --features infer -- --ignored words`.
    #[test]
    #[ignore = "needs the models"]
    fn words_are_read_from_a_picture() {
        let Ok(dir) = std::env::var("WORDS_DIR") else { return };
        let dir = std::path::Path::new(&dir);
        let mut words = Words::load(&dir.join("det.onnx"), &dir.join("rec.onnx")).expect("loaded");
        let frame = Frame::from_rgba(1080, 1920, std::fs::read(dir.join("c4.rgba")).expect("picture")).expect("frame");
        let lines = words.lines(&frame).expect("read");
        let said: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(said, ["@me", "SUMMER 2026"], "{lines:?}");
        let (ink, outline) = colours(&frame, lines[1].rect);
        assert_eq!(outline.as_deref(), Some("#000000"), "{ink}");
        assert!(ink.starts_with("#f") || ink.starts_with("#e"), "{ink}");
        let (ink, _) = colours(&frame, lines[0].rect);
        assert!(ink.starts_with("#f") || ink.starts_with("#e"), "yellow {ink}");
    }

    #[test]
    fn the_letters_are_the_readers() {
        assert_eq!(LETTERS.split('\n').filter(|l| !l.is_empty()).count(), 436);
    }
}
