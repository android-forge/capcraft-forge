// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Where the face is: OpenCV's YuNet face detector (MIT, compiled in; see
//! `models/NOTICE.md`) and the track of a media file's face kept between
//! runs, for the reel effects that follow it.
//!
//! The track is one small text file per media file under the project's
//! `cache/faces/`: a line per analysed source instant, its milliseconds
//! and the face's centre and width as fractions of the picture, or a dash
//! where no face was found. Read back it is smoothed forwards and
//! backwards, so the effects follow the face without lagging it, as
//! claude-video-effects' `smooth` does.

use std::path::{Path, PathBuf};

use crate::store::fnv1a;

/// A face: its centre across and down and its width, as fractions of the
/// picture.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FaceBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

/// The file a media file's face track lives in under `project`.
pub fn face_file(project: &Path, media_path: &str) -> PathBuf {
    project.join("cache").join("faces").join(format!("{:016x}.txt", fnv1a(media_path.as_bytes())))
}

/// A media file's face track as analysed: what was found at each instant.
#[derive(Clone, Debug, Default)]
pub struct FaceTrack {
    /// `(milliseconds, face)` in ascending time; `None` where nobody was found.
    samples: Vec<(u64, Option<FaceBox>)>,
    /// The samples smoothed and with their gaps filled, once read back.
    smooth: Vec<(u64, FaceBox)>,
}

impl FaceTrack {
    /// The track in `file`; empty when there is none.
    pub fn open(file: &Path) -> FaceTrack {
        let mut track = FaceTrack::default();
        if let Ok(text) = std::fs::read_to_string(file) {
            for line in text.lines() {
                let mut parts = line.split_whitespace();
                let Some(ms) = parts.next().and_then(|v| v.parse::<u64>().ok()) else {
                    continue;
                };
                let vals: Vec<f32> = parts.filter_map(|v| v.parse().ok()).collect();
                let face = (vals.len() == 3).then(|| FaceBox { x: vals[0], y: vals[1], w: vals[2] });
                track.samples.push((ms, face));
            }
        }
        track.samples.sort_by_key(|s| s.0);
        track.samples.dedup_by_key(|s| s.0);
        track.smooth = smoothed(&track.samples);
        track
    }

    /// Whether `ms` has been looked at.
    pub fn has(&self, ms: u64) -> bool {
        self.samples.binary_search_by_key(&ms, |s| s.0).is_ok()
    }

    /// Whether nothing has been analysed.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Records what was found at `ms`.
    pub fn put(&mut self, ms: u64, face: Option<FaceBox>) {
        match self.samples.binary_search_by_key(&ms, |s| s.0) {
            Ok(i) => self.samples[i].1 = face,
            Err(i) => self.samples.insert(i, (ms, face)),
        }
    }

    /// Writes the track to `file`, by way of a temporary beside it.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        }
        let mut text = String::new();
        for (ms, face) in &self.samples {
            match face {
                Some(f) => text.push_str(&format!("{ms} {:.5} {:.5} {:.5}\n", f.x, f.y, f.w)),
                None => text.push_str(&format!("{ms} -\n")),
            }
        }
        let part = file.with_extension("part");
        std::fs::write(&part, text).map_err(|e| format!("could not write {}: {e}", part.display()))?;
        std::fs::rename(&part, file).map_err(|e| format!("could not write {}: {e}", file.display()))
    }

    /// The smoothed face at `seconds` of the source, between the samples
    /// around it; `None` when no face was found anywhere.
    pub fn at(&self, seconds: f64) -> Option<FaceBox> {
        let s = &self.smooth;
        if s.is_empty() {
            return None;
        }
        let ms = (seconds.max(0.0) * 1000.0) as u64;
        let i = s.partition_point(|p| p.0 <= ms);
        if i == 0 {
            return Some(s[0].1);
        }
        if i >= s.len() {
            return Some(s[s.len() - 1].1);
        }
        let ((t0, a), (t1, b)) = (s[i - 1], s[i]);
        let k = if t1 > t0 { (ms - t0) as f32 / (t1 - t0) as f32 } else { 0.0 };
        Some(FaceBox { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k, w: a.w + (b.w - a.w) * k })
    }
}

/// The samples with their gaps filled from their neighbours and smoothed
/// forwards then backwards over about a fifth of a second.
fn smoothed(samples: &[(u64, Option<FaceBox>)]) -> Vec<(u64, FaceBox)> {
    let found: Vec<(u64, FaceBox)> = samples.iter().filter_map(|(t, f)| f.map(|f| (*t, f))).collect();
    if found.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(u64, FaceBox)> = samples
        .iter()
        .map(|&(t, f)| {
            let f = f.unwrap_or_else(|| {
                let i = found.partition_point(|p| p.0 <= t);
                match (i.checked_sub(1).map(|j| found[j]), found.get(i).copied()) {
                    (Some((t0, a)), Some((t1, b))) => {
                        let k = (t - t0) as f32 / (t1 - t0).max(1) as f32;
                        FaceBox { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k, w: a.w + (b.w - a.w) * k }
                    }
                    (Some((_, a)), None) => a,
                    (None, Some((_, b))) => b,
                    (None, None) => found[0].1,
                }
            });
            (t, f)
        })
        .collect();
    let step = |a: FaceBox, b: FaceBox, k: f32| FaceBox { x: a.x + k * (b.x - a.x), y: a.y + k * (b.y - a.y), w: a.w + k * (b.w - a.w) };
    for i in 1..out.len() {
        let dt = (out[i].0 - out[i - 1].0) as f32 / 1000.0;
        let k = 1.0 - (-dt / 0.18).exp();
        out[i].1 = step(out[i - 1].1, out[i].1, k);
    }
    for i in (0..out.len().saturating_sub(1)).rev() {
        let dt = (out[i + 1].0 - out[i].0) as f32 / 1000.0;
        let k = 1.0 - (-dt / 0.18).exp();
        out[i].1 = step(out[i + 1].1, out[i].1, k);
    }
    out
}

#[cfg(feature = "infer")]
pub use detector::FaceDetector;

#[cfg(feature = "infer")]
mod detector {
    use std::sync::Mutex;

    use concat_core::frame::{BYTES_PER_PIXEL, Frame};

    use super::FaceBox;
    use crate::runtime::{Data, Input, Model};

    /// The compiled-in detector: YuNet, face_detection_yunet_2023mar.
    const YUNET: &[u8] = include_bytes!("../models/yunet.onnx");
    /// Its fixed input side.
    const SIDE: usize = 640;
    /// A face below this score is not one.
    const SCORE: f32 = 0.6;

    /// YuNet, loaded once.
    pub struct FaceDetector {
        model: Mutex<Model>,
    }

    impl FaceDetector {
        /// The compiled-in detector.
        pub fn new() -> Result<FaceDetector, String> {
            Ok(FaceDetector { model: Mutex::new(Model::from_bytes(YUNET)?) })
        }

        /// The most prominent face in `frame` - the largest, weighed by how
        /// sure the detector is - or `None`.
        pub fn detect(&self, frame: &Frame) -> Result<Option<FaceBox>, String> {
            let (fw, fh) = (frame.width() as usize, frame.height() as usize);
            if fw == 0 || fh == 0 {
                return Ok(None);
            }
            // the picture fitted into the square, from its top-left, as BGR 0..255
            let scale = SIDE as f32 / fw.max(fh) as f32;
            let (sw, sh) = (((fw as f32 * scale) as usize).min(SIDE), ((fh as f32 * scale) as usize).min(SIDE));
            let px = frame.pixels();
            let mut data = vec![0f32; 3 * SIDE * SIDE];
            for y in 0..sh {
                let sy = ((y as f32 / scale) as usize).min(fh - 1);
                for x in 0..sw {
                    let sx = ((x as f32 / scale) as usize).min(fw - 1);
                    let s = (sy * fw + sx) * BYTES_PER_PIXEL;
                    for (c, from) in [2usize, 1, 0].into_iter().enumerate() {
                        data[(c * SIDE + y) * SIDE + x] = f32::from(px[s + from]);
                    }
                }
            }
            let names = ["cls_8", "cls_16", "cls_32", "obj_8", "obj_16", "obj_32", "bbox_8", "bbox_16", "bbox_32"];
            let out = self
                .model
                .lock()
                .map_err(|_| "face model poisoned".to_owned())?
                .run(vec![Input { name: "input", dims: vec![1, 3, SIDE, SIDE], data: Data::F32(data) }], &names)?;
            let mut best: Option<(f32, FaceBox)> = None;
            for (k, stride) in [8usize, 16, 32].into_iter().enumerate() {
                let (cls, obj, bbox) = (&out[k].data, &out[3 + k].data, &out[6 + k].data);
                let cols = SIDE / stride;
                for i in 0..cls.len().min(obj.len()) {
                    let score = (cls[i].clamp(0.0, 1.0) * obj[i].clamp(0.0, 1.0)).sqrt();
                    if score < SCORE {
                        continue;
                    }
                    let (row, col) = ((i / cols) as f32, (i % cols) as f32);
                    let s = stride as f32;
                    let cx = (col + bbox[i * 4]) * s;
                    let cy = (row + bbox[i * 4 + 1]) * s;
                    let w = bbox[i * 4 + 2].exp() * s;
                    let h = bbox[i * 4 + 3].exp() * s;
                    let weight = w * h * score;
                    if best.is_none_or(|(b, _)| weight > b) {
                        best = Some((weight, FaceBox { x: cx / scale / fw as f32, y: cy / scale / fh as f32, w: w / scale / fw as f32 }));
                    }
                }
            }
            Ok(best.map(|(_, f)| f))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_flat_picture_has_no_face() {
            let detector = FaceDetector::new().expect("loads");
            let mut frame = Frame::black(320, 240);
            for p in frame.pixels_mut().chunks_exact_mut(4) {
                p[..3].copy_from_slice(&[120, 130, 140]);
            }
            assert_eq!(detector.detect(&frame).expect("runs"), None);
        }

        /// The face of a picture (`FACE_IN`, raw RGBA, `FACE_SIZE` "w h"),
        /// printed: for checking against another detector by hand.
        #[test]
        #[ignore = "needs a picture"]
        fn a_picture_has_its_face_found() {
            let (Ok(input), Ok(size)) = (std::env::var("FACE_IN"), std::env::var("FACE_SIZE")) else { return };
            let mut size = size.split(' ').map(|v| v.parse::<u32>().unwrap());
            let (w, h) = (size.next().unwrap(), size.next().unwrap());
            let frame = Frame::from_rgba(w, h, std::fs::read(input).unwrap()).unwrap();
            let face = FaceDetector::new().unwrap().detect(&frame).unwrap();
            println!("face {face:?}");
            assert!(face.is_some());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_round_trips_and_fills_its_gaps() {
        let dir = std::env::temp_dir().join(format!("faces-{}", std::process::id()));
        let file = dir.join("t.txt");
        let mut track = FaceTrack::default();
        track.put(0, Some(FaceBox { x: 0.4, y: 0.3, w: 0.2 }));
        track.put(100, None);
        track.put(200, Some(FaceBox { x: 0.6, y: 0.3, w: 0.2 }));
        track.save(&file).expect("saves");
        let back = FaceTrack::open(&file);
        assert!(back.has(100));
        let mid = back.at(0.1).expect("a face");
        assert!(mid.x > 0.42 && mid.x < 0.58, "{mid:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
