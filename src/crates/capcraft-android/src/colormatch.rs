// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Color match: what the color-match package is set to so one picture takes
//! on the colours of another. Each picture is read as light and two colour
//! differences, and the average and spread of each are brought to the
//! reference's.

use serde_json::json;

use crate::{MobileError, Picture};

/// The average and the spread of one measure.
#[derive(Clone, Copy, Default)]
struct Spread {
    mean: f64,
    deviation: f64,
}

/// How far the light may be stretched or flattened: a flat reference must
/// not wash a picture out, nor a hard one crush it.
const LIGHT: (f64, f64) = (0.6, 1.6);
/// How far colour may be drained or pushed: past this the slight differences
/// of a picture of nearly one colour would stand out.
const COLOUR: (f64, f64) = (0.3, 1.5);
/// The spread below which a measure is one flat value, and only shifted.
const FLAT: f64 = 0.004;

/// The spreads of a picture's light and its blue and red differences. Pixels
/// at or near black are left out, being bars round a picture as often as
/// part of it, unless there is little else.
fn spreads(picture: &Picture) -> Result<[Spread; 3], MobileError> {
    let pixels = picture.width as usize * picture.height as usize;
    if pixels == 0 || picture.rgba.len() < pixels * 4 {
        return Err(MobileError::Failed(
            "The picture could not be read.".to_owned(),
        ));
    }
    let measures = |pixel: &[u8]| {
        let [r, g, b] = [0, 1, 2].map(|at| f64::from(pixel[at]) / 255.0);
        let light = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        [light, b - light, r - light]
    };
    let all = || picture.rgba[..pixels * 4].chunks_exact(4);
    let lit = all().filter(|pixel| measures(pixel)[0] > 0.02).count();
    let every = lit * 20 < pixels;
    let mut sums = [(0.0, 0.0); 3];
    let mut count = 0.0_f64;
    for pixel in all() {
        let found = measures(pixel);
        if !every && found[0] <= 0.02 {
            continue;
        }
        count += 1.0;
        for (sum, value) in sums.iter_mut().zip(found) {
            sum.0 += value;
            sum.1 += value * value;
        }
    }
    Ok(sums.map(|(sum, squares)| {
        let mean = sum / count.max(1.0);
        Spread {
            mean,
            deviation: (squares / count.max(1.0) - mean * mean).max(0.0).sqrt(),
        }
    }))
}

/// The scale and the shift that bring `from` to `to`.
fn bring(from: Spread, to: Spread, (least, most): (f64, f64)) -> (f64, f64) {
    let gain = if from.deviation < FLAT {
        1.0
    } else {
        (to.deviation / from.deviation).clamp(least, most)
    };
    (gain, (to.mean - from.mean * gain).clamp(-2.0, 2.0))
}

/// The color-match package's parameters, as JSON, that give `source` the
/// colours of `reference`. Both are small copies of the pictures.
#[uniffi::export]
pub fn color_match(reference: Picture, source: Picture) -> Result<String, MobileError> {
    let (to, from) = (spreads(&reference)?, spreads(&source)?);
    let (light_gain, light_shift) = bring(from[0], to[0], LIGHT);
    let (blue_gain, blue_shift) = bring(from[1], to[1], COLOUR);
    let (red_gain, red_shift) = bring(from[2], to[2], COLOUR);
    Ok(json!({
        "light_gain": light_gain,
        "light_shift": light_shift,
        "blue_gain": blue_gain,
        "blue_shift": blue_shift,
        "red_gain": red_gain,
        "red_shift": red_shift,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose pixels run between two colours.
    fn run(from: [u8; 3], to: [u8; 3]) -> Picture {
        let mut rgba = Vec::new();
        for step in 0..64_u32 {
            for at in 0..3 {
                let (a, b) = (f64::from(from[at]), f64::from(to[at]));
                rgba.push((a + (b - a) * f64::from(step) / 63.0).round() as u8);
            }
            rgba.push(255);
        }
        Picture {
            width: 8,
            height: 8,
            rgba,
        }
    }

    /// `picture` through the package at `set`, as its shader draws it.
    fn drawn(picture: &Picture, set: &str) -> Picture {
        let set: serde_json::Value = serde_json::from_str(set).expect("parses");
        let of = |key: &str| set[key].as_f64().expect("a number");
        let mut rgba = Vec::new();
        for pixel in picture.rgba.chunks_exact(4) {
            let [r, g, b] = [0, 1, 2].map(|at| f64::from(pixel[at]) / 255.0);
            let light = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let y = light * of("light_gain") + of("light_shift");
            let u = (b - light) * of("blue_gain") + of("blue_shift");
            let v = (r - light) * of("red_gain") + of("red_shift");
            let (r, b) = (y + v, y + u);
            let g = (y - 0.2126 * r - 0.0722 * b) / 0.7152;
            rgba.extend([r, g, b].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8));
            rgba.push(255);
        }
        Picture {
            width: picture.width,
            height: picture.height,
            rgba,
        }
    }

    #[test]
    fn a_picture_matched_to_itself_is_left_alone() {
        let picture = run([40, 60, 90], [200, 180, 150]);
        let set: serde_json::Value =
            serde_json::from_str(&color_match(picture.clone(), picture).expect("matched"))
                .expect("parses");
        for key in ["light", "blue", "red"] {
            let gain = set[format!("{key}_gain")].as_f64().expect("a number");
            let shift = set[format!("{key}_shift")].as_f64().expect("a number");
            assert!((gain - 1.0).abs() < 1e-9 && shift.abs() < 1e-9, "{set}");
        }
    }

    #[test]
    fn a_cool_picture_takes_a_warm_ones_colours() {
        let cool = run([40, 70, 120], [150, 160, 190]);
        let warm = run([110, 80, 50], [220, 180, 130]);
        let set = color_match(warm.clone(), cool.clone()).expect("matched");
        let got = spreads(&drawn(&cool, &set)).expect("read");
        let want = spreads(&warm).expect("read");
        for (got, want) in got.iter().zip(want) {
            assert!(
                (got.mean - want.mean).abs() < 0.01,
                "{} {}",
                got.mean,
                want.mean
            );
            assert!((got.deviation - want.deviation).abs() < 0.01);
        }
    }

    #[test]
    fn bars_round_the_reference_do_not_count() {
        let plain = run([110, 80, 50], [220, 180, 130]);
        let mut barred = plain.clone();
        barred.height = 16;
        barred.rgba.extend([0, 0, 0, 255].repeat(64));
        let source = run([50, 70, 110], [140, 160, 200]);
        assert_eq!(
            color_match(plain, source.clone()).expect("matched"),
            color_match(barred, source).expect("matched")
        );
    }

    #[test]
    fn one_flat_colour_is_only_shifted_and_black_is_read() {
        let flat = run([90, 90, 90], [90, 90, 90]);
        let black = run([0, 0, 0], [0, 0, 0]);
        let set: serde_json::Value =
            serde_json::from_str(&color_match(black, flat).expect("matched")).expect("parses");
        assert_eq!(set["light_gain"], 1.0);
        assert!(set["light_shift"].as_f64().expect("a number") < -0.3);
    }

    #[test]
    fn a_picture_cut_short_is_refused() {
        let mut short = run([0, 0, 0], [255, 255, 255]);
        short.rgba.truncate(40);
        assert!(color_match(short.clone(), short).is_err());
    }
}
