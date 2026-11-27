// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Finding the subject in a picture.
//!
//! Three models, one interface: a frame in, a probability per pixel out.
//!
//! - **Selfie**: MediaPipe's selfie segmentation, compiled into the binary
//!   (see `models/NOTICE.md`). A person, at 256 × 256, in a few
//!   milliseconds; the answer when nothing has been downloaded.
//! - **Person**: Robust Video Matting on MobileNetV3. A person with a true
//!   alpha edge - hair, motion blur - at the picture's own aspect, carrying
//!   a recurrent state from one frame to the next so an outline does not
//!   flicker. Frames must therefore arrive in order: [`Segmenter::begin`]
//!   starts a run.
//! - **Object**: IS-Net. The one thing the picture is of, whatever it is,
//!   at 1024 × 1024; the answer for a car, a product, a cartoon.
//! - **Skin**: MediaPipe's multiclass selfie segmentation, compiled in like
//!   the selfie model (see `models/NOTICE.md`). Not a subject to cut out:
//!   the face's and the body's skin, which Retouch smooths and nothing else.
//!
//! A source of any shape is resampled to the model's size on the way in
//! and the mask is read as covering the whole source on the way out, which
//! is the convention every other part of this crate keeps.

use std::path::Path;
use std::sync::Mutex;

use concat_core::frame::{BYTES_PER_PIXEL, Frame};

use crate::models::ModelId;
use crate::runtime::{Input, Model, Output};
use crate::{MODEL_ID, MODEL_SIZE, Mask};

/// The compiled-in model.
const SELFIE: &[u8] = include_bytes!("../models/selfie-segmentation.onnx");
/// The compiled-in skin model: six classes a pixel, of which body skin and
/// face skin are kept.
const SKIN: &[u8] = include_bytes!("../models/selfie-multiclass.onnx");
/// The name a mask store records for the skin model's masks.
pub const SKIN_MODEL_ID: &str = "selfie-multiclass-256";
/// The skin model's fixed input.
const SKIN_SIZE: u32 = 256;
/// The skin model's classes: background, hair, body skin, face skin,
/// clothes, others. Body skin and face skin are skin.
const SKIN_CLASSES: usize = 6;
const BODY_SKIN: usize = 2;
const FACE_SKIN: usize = 3;

/// The person model's long edge: its stage one works at up to 512, and a
/// larger frame would only be shrunk inside it.
const PERSON_EDGE: u32 = 512;
/// The object model's fixed input.
const OBJECT_SIZE: u32 = 1024;
/// The object model's answer is stored at half its size: a quarter of the
/// bytes, and still twice the person model's.
const OBJECT_SHRINK: u32 = 2;

/// Which model a segmenter runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// The compiled-in person model.
    Selfie,
    /// The downloaded person matting model.
    Person,
    /// The downloaded object model.
    Object,
    /// The compiled-in skin model, for Retouch.
    Skin,
}

/// A loaded model, ready to run. `Send + Sync`; the runtime's own threads
/// take a frame across the cores, so one per process is enough.
pub struct Segmenter {
    kind: Kind,
    model: Mutex<Model>,
    /// The person model's recurrent state from the frame before, within
    /// one run.
    state: Mutex<Option<Vec<Output>>>,
}

impl Segmenter {
    /// The compiled-in person model. A few tens of milliseconds, once.
    pub fn selfie() -> Result<Segmenter, String> {
        Ok(Segmenter {
            kind: Kind::Selfie,
            model: Mutex::new(Model::from_bytes(SELFIE)?),
            state: Mutex::new(None),
        })
    }

    /// The compiled-in model; see [`Segmenter::selfie`].
    pub fn load() -> Result<Segmenter, String> {
        Segmenter::selfie()
    }

    /// The compiled-in skin model: where the face's and the body's skin are.
    pub fn skin() -> Result<Segmenter, String> {
        Ok(Segmenter {
            kind: Kind::Skin,
            model: Mutex::new(Model::from_bytes(SKIN)?),
            state: Mutex::new(None),
        })
    }

    /// The person matting model, from its downloaded file.
    pub fn person(file: &Path) -> Result<Segmenter, String> {
        Ok(Segmenter {
            kind: Kind::Person,
            model: Mutex::new(Model::from_file(file)?),
            state: Mutex::new(None),
        })
    }

    /// The object model, from its downloaded file.
    pub fn object(file: &Path) -> Result<Segmenter, String> {
        Ok(Segmenter {
            kind: Kind::Object,
            model: Mutex::new(Model::from_file(file)?),
            state: Mutex::new(None),
        })
    }

    /// Which model this is.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The name a mask store records for this model's masks.
    pub fn name(&self) -> &'static str {
        match self.kind {
            Kind::Selfie => MODEL_ID,
            Kind::Person => ModelId::Person.name(),
            Kind::Object => ModelId::Object.name(),
            Kind::Skin => SKIN_MODEL_ID,
        }
    }

    /// The size to decode a source of `width` × `height` to for this
    /// model: its fixed square, or the source's shape at the person model's
    /// working edge.
    pub fn input_size(&self, width: u32, height: u32) -> (u32, u32) {
        match self.kind {
            Kind::Selfie => (MODEL_SIZE, MODEL_SIZE),
            Kind::Object => (OBJECT_SIZE, OBJECT_SIZE),
            Kind::Skin => (SKIN_SIZE, SKIN_SIZE),
            Kind::Person => {
                let (width, height) = (width.max(1) as f64, height.max(1) as f64);
                let scale = f64::from(PERSON_EDGE) / width.max(height);
                // Multiples of sixteen, so every stride inside the network
                // divides evenly and nothing is padded off the edge.
                let round = |edge: f64| (((edge * scale) / 16.0).round() as u32 * 16).max(64);
                (round(width), round(height))
            }
        }
    }

    /// Starts a run of frames in source order. The person model forgets
    /// what it carried from the frame before; the others carry nothing.
    pub fn begin(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = None;
        }
    }

    /// The subject mask of `frame`. A frame of any size: it is resampled
    /// to the model's size first, so the decoder is best asked for
    /// [`Segmenter::input_size`] and the resample is a copy.
    pub fn mask(&self, frame: &Frame) -> Result<Mask, String> {
        if frame.width() == 0 || frame.height() == 0 {
            return Err("cutout: an empty frame".to_owned());
        }
        let (width, height) = self.input_size(frame.width(), frame.height());
        let planes = planes(frame, width, height);
        match self.kind {
            Kind::Selfie => self.selfie_mask(planes, width, height),
            Kind::Object => self.object_mask(planes, width, height),
            Kind::Person => self.person_mask(planes, width, height),
            Kind::Skin => self.skin_mask(planes, width, height),
        }
    }

    fn run(&self, inputs: Vec<Input<'_>>, outputs: &[&str]) -> Result<Vec<Output>, String> {
        self.model
            .lock()
            .map_err(|_| "cutout model poisoned".to_owned())?
            .run(inputs, outputs)
    }

    fn selfie_mask(&self, planes: Vec<f32>, width: u32, height: u32) -> Result<Mask, String> {
        let size = (width * height) as usize;
        let input = Input {
            name: "pixel_values",
            dims: vec![1, 3, height as usize, width as usize],
            data: planes.into(),
        };
        let outputs = self.run(vec![input], &["alphas"])?;
        probabilities(&outputs[0].data, size, width, height)
    }

    fn skin_mask(&self, planes: Vec<f32>, width: u32, height: u32) -> Result<Mask, String> {
        // The model was converted from LiteRT and keeps its layout: one
        // pixel's three channels together, in 0..=1.
        let size = (width * height) as usize;
        let mut interleaved = vec![0f32; 3 * size];
        for (at, pixel) in interleaved.chunks_exact_mut(3).enumerate() {
            for (c, value) in pixel.iter_mut().enumerate() {
                *value = planes[c * size + at];
            }
        }
        let input = Input {
            name: "input_29",
            dims: vec![1, height as usize, width as usize, 3],
            data: interleaved.into(),
        };
        let outputs = self.run(vec![input], &["Identity"])?;
        skin_of(&outputs[0].data, size, width, height)
    }

    fn object_mask(&self, mut planes: Vec<f32>, width: u32, height: u32) -> Result<Mask, String> {
        // As the model was trained: the picture scaled by its brightest
        // value, then centred on a half.
        let brightest = planes.iter().copied().fold(0.0f32, f32::max).max(1e-6);
        for value in &mut planes {
            *value = *value / brightest - 0.5;
        }
        let size = (width * height) as usize;
        let input = Input {
            name: "input_image",
            dims: vec![1, 3, height as usize, width as usize],
            data: planes.into(),
        };
        let outputs = self.run(vec![input], &["output_image"])?;
        // The model answers in its own range; the span of the answer is
        // the mask, as the reference implementation reads it.
        let data = &outputs[0].data[..size.min(outputs[0].data.len())];
        let low = data.iter().copied().fold(f32::INFINITY, f32::min);
        let high = data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let span = (high - low).max(1e-6);
        let bytes: Vec<u8> = data
            .iter()
            .map(|&p| (((p - low) / span).clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
            .collect();
        let mask = Mask::from_bytes(width, height, bytes)
            .ok_or_else(|| "cutout: the model's answer is the wrong size".to_owned())?;
        Ok(mask.shrunk(OBJECT_SHRINK))
    }

    fn person_mask(&self, planes: Vec<f32>, width: u32, height: u32) -> Result<Mask, String> {
        let size = (width * height) as usize;
        let mut inputs = vec![Input {
            name: "src",
            dims: vec![1, 3, height as usize, width as usize],
            data: planes.into(),
        }];
        let carried = self
            .state
            .lock()
            .map_err(|_| "cutout state poisoned".to_owned())?
            .take();
        const STATES: [&str; 4] = ["r1i", "r2i", "r3i", "r4i"];
        match carried {
            Some(state) if state.len() == 4 => {
                for (name, output) in STATES.iter().zip(state) {
                    inputs.push(Input {
                        name,
                        dims: output.dims,
                        data: output.data.into(),
                    });
                }
            }
            _ => {
                for name in STATES {
                    inputs.push(Input {
                        name,
                        dims: vec![1, 1, 1, 1],
                        data: vec![0.0f32].into(),
                    });
                }
            }
        }
        // The frame is already at the working size; the model's own
        // downsample stays at one.
        inputs.push(Input {
            name: "downsample_ratio",
            dims: vec![1],
            data: vec![1.0f32].into(),
        });
        let mut outputs = self.run(inputs, &["pha", "r1o", "r2o", "r3o", "r4o"])?;
        let alpha = outputs.remove(0);
        if let Ok(mut state) = self.state.lock() {
            *state = Some(outputs);
        }
        probabilities(&alpha.data, size, width, height)
    }
}

/// The frame as the models read it: three planes of `0..=1`, red then
/// green then blue, at `width` × `height`, nearest-sampled from the frame.
fn planes(frame: &Frame, width: u32, height: u32) -> Vec<f32> {
    let (fw, fh) = (frame.width() as usize, frame.height() as usize);
    let (width, height) = (width as usize, height as usize);
    let pixels = frame.pixels();
    let mut data = vec![0f32; 3 * width * height];
    for c in 0..3 {
        for y in 0..height {
            let sy = (y * fh / height).min(fh - 1);
            for x in 0..width {
                let sx = (x * fw / width).min(fw - 1);
                data[(c * height + y) * width + x] =
                    f32::from(pixels[(sy * fw + sx) * BYTES_PER_PIXEL + c]) / 255.0;
            }
        }
    }
    data
}

/// A model's `0..=1` answer as a mask.
fn probabilities(data: &[f32], size: usize, width: u32, height: u32) -> Result<Mask, String> {
    let bytes: Vec<u8> = data
        .iter()
        .take(size)
        .map(|&p| (p.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
        .collect();
    Mask::from_bytes(width, height, bytes)
        .ok_or_else(|| "cutout: the model's answer is the wrong size".to_owned())
}

/// The skin model's answer - six classes' scores a pixel - as how likely
/// each pixel is skin, the face's or the body's.
fn skin_of(logits: &[f32], size: usize, width: u32, height: u32) -> Result<Mask, String> {
    if logits.len() < size * SKIN_CLASSES {
        return Err("skin: the model's answer is the wrong size".to_owned());
    }
    let bytes: Vec<u8> = logits
        .chunks_exact(SKIN_CLASSES)
        .take(size)
        .map(|scores| {
            let top = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let mut total = 0.0;
            let mut skin = 0.0;
            for (class, &score) in scores.iter().enumerate() {
                let weight = (score - top).exp();
                total += weight;
                if class == BODY_SKIN || class == FACE_SKIN {
                    skin += weight;
                }
            }
            ((skin / total.max(1e-12)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        })
        .collect();
    Mask::from_bytes(width, height, bytes)
        .ok_or_else(|| "skin: the model's answer is the wrong size".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_loads_and_answers_a_mask_of_its_size() {
        let segmenter = Segmenter::selfie().expect("loads");
        // A flat grey picture is nobody: the mask should lean background.
        let mut frame = Frame::black(64, 48);
        for pixel in frame.pixels_mut().chunks_exact_mut(4) {
            pixel[..3].copy_from_slice(&[128, 128, 128]);
        }
        let mask = segmenter.mask(&frame).expect("runs");
        assert_eq!((mask.width(), mask.height()), (MODEL_SIZE, MODEL_SIZE));
        let mean = mask.bytes().iter().map(|&v| u32::from(v)).sum::<u32>() / (256 * 256);
        assert!(mean < 128, "a blank picture read as mostly person: {mean}");
    }

    #[test]
    fn skin_is_the_face_and_the_body_and_nothing_else() {
        // three pixels: face skin, body skin, clothes - each class clearly ahead
        let mut logits = vec![0f32; 3 * SKIN_CLASSES];
        logits[FACE_SKIN] = 8.0;
        logits[SKIN_CLASSES + BODY_SKIN] = 8.0;
        logits[2 * SKIN_CLASSES + 4] = 8.0;
        let mask = skin_of(&logits, 3, 3, 1).expect("a mask");
        let values: Vec<f32> = (0..3).map(|x| mask.sample((x as f32 + 0.5) / 3.0, 0.5)).collect();
        assert!(values[0] > 0.95 && values[1] > 0.95, "{values:?}");
        assert!(values[2] < 0.05, "{values:?}");
        assert!(skin_of(&logits[..5], 3, 3, 1).is_err());
    }

    #[test]
    fn the_skin_model_loads_and_finds_no_skin_in_grey() {
        let segmenter = Segmenter::skin().expect("loads");
        assert_eq!(segmenter.input_size(1920, 1080), (SKIN_SIZE, SKIN_SIZE));
        assert_eq!(segmenter.name(), SKIN_MODEL_ID);
        let mut frame = Frame::black(64, 48);
        for pixel in frame.pixels_mut().chunks_exact_mut(4) {
            pixel[..3].copy_from_slice(&[90, 140, 200]);
        }
        let mask = segmenter.mask(&frame).expect("runs");
        assert_eq!((mask.width(), mask.height()), (SKIN_SIZE, SKIN_SIZE));
        assert!(mask.sample(0.5, 0.5) < 0.2, "a flat blue is not skin");
    }

    #[test]
    fn the_person_model_works_at_the_pictures_shape() {
        let segmenter = Segmenter {
            kind: Kind::Person,
            model: Mutex::new(Model::from_bytes(SELFIE).expect("loads")),
            state: Mutex::new(None),
        };
        assert_eq!(segmenter.input_size(1920, 1080), (512, 288));
        assert_eq!(segmenter.input_size(1080, 1920), (288, 512));
        assert_eq!(segmenter.input_size(1000, 1000), (512, 512));
    }
}

#[cfg(test)]
mod picture_mask {
    /// The selfie mask of a picture (`MASK_IN`, raw RGBA, `MASK_SIZE` "w h"),
    /// written as a PNG to `MASK_OUT`: for looking at by eye. `MASK_MODEL=skin`
    /// asks the skin model instead.
    #[test]
    #[ignore = "needs a picture"]
    fn a_picture_is_masked() {
        let (Ok(input), Ok(size), Ok(out)) = (std::env::var("MASK_IN"), std::env::var("MASK_SIZE"), std::env::var("MASK_OUT")) else { return };
        let mut size = size.split(' ').map(|v| v.parse::<u32>().unwrap());
        let (w, h) = (size.next().unwrap(), size.next().unwrap());
        let frame = concat_core::frame::Frame::from_rgba(w, h, std::fs::read(input).unwrap()).unwrap();
        let segmenter = if std::env::var("MASK_MODEL").as_deref() == Ok("skin") {
            super::Segmenter::skin().unwrap()
        } else {
            super::Segmenter::selfie().unwrap()
        };
        let mask = segmenter.mask(&frame).unwrap();
        std::fs::write(out, mask.to_png()).unwrap();
    }
}
