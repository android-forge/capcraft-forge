// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Light in 3D: a picture's depth read by Depth Anything V2 Small, and the
//! shadows a light at a point of it throws worked out from that depth, as
//! the scene's own shapes would throw them.
//!
//! The depth model gives each pixel how near it is (relative, larger is
//! nearer). That is taken as a height field standing towards the camera.
//! The light sits at its brightest point in the picture, a little in
//! front of what is there. From every pixel a ray is walked
//! to the light, its height going from the pixel's to the light's; where
//! the scene stands above the ray on the way, something is between the
//! pixel and the light, and the pixel is in shadow. Near the pixel the
//! test is forgiving, so a surface does not shade itself, and the shadow
//! is softened where only a little of the way is blocked. Where the picture
//! is bright already, lit by its own lamps, the shadow fades: that light
//! fills it in.

use concat_vision::models::ModelId;
use concat_vision::runtime::{Input, Model};

use crate::Picture;

/// The long side the depth model is given: its own training size.
const MODEL_SIDE: usize = 518;
/// The model's patch: its input sides are multiples of this.
const PATCH: usize = 14;
/// The long side shadows are worked out at.
const SHADE_SIDE: usize = 256;
/// The steps a ray to the light is walked in.
const STEPS: usize = 48;

/// How near each point of a picture is: `width` by `height`, 0 the
/// furthest and 1 the nearest.
pub(crate) struct Depth {
    pub width: usize,
    pub height: usize,
    pub near: Vec<f32>,
}

/// Reads the depth of `picture` with the model at `model`.
pub(crate) fn depth_of(model: &std::path::Path, picture: &Picture) -> Result<Depth, String> {
    let (pw, ph) = (picture.width as usize, picture.height as usize);
    if pw < 2 || ph < 2 {
        return Err("The picture is too small.".to_owned());
    }
    let scale = MODEL_SIDE as f32 / pw.max(ph) as f32;
    let side = |n: usize| (((n as f32 * scale) / PATCH as f32).round() as usize).max(1) * PATCH;
    let (w, h) = (side(pw), side(ph));
    // ImageNet's colours, as the model learnt them, in planes
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];
    let mut data = vec![0f32; 3 * w * h];
    for y in 0..h {
        let sy = ((y as f32 + 0.5) / h as f32 * ph as f32) as usize;
        for x in 0..w {
            let sx = ((x as f32 + 0.5) / w as f32 * pw as f32) as usize;
            let i = (sy.min(ph - 1) * pw + sx.min(pw - 1)) * 4;
            for c in 0..3 {
                data[c * w * h + y * w + x] = (picture.rgba[i + c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }
    let mut session = Model::from_file_on_cpu(model)?;
    let out = session.run(vec![Input { name: "pixel_values", dims: vec![1, 3, h, w], data: data.into() }], &["predicted_depth"])?;
    let out = out.into_iter().next().ok_or_else(|| "The depth model said nothing.".to_owned())?;
    let (oh, ow) = match out.dims.as_slice() {
        [.., oh, ow] => (*oh, *ow),
        _ => return Err("The depth model's answer has no shape.".to_owned()),
    };
    // to 0..1, the furthest a twentieth in and the nearest likewise, so a few stray values do not set the range
    let mut sorted = out.data.clone();
    sorted.sort_by(f32::total_cmp);
    let (lo, hi) = (sorted[sorted.len() / 20], sorted[sorted.len() * 19 / 20]);
    let span = (hi - lo).max(1e-6);
    Ok(Depth { width: ow, height: oh, near: out.data.iter().map(|v| ((v - lo) / span).clamp(0.0, 1.0)).collect() })
}

/// How far a point is, from how near the model says it is (0 to 1): its
/// depth for the camera, near things about one, far things about ten.
fn distance(near: f32) -> f32 {
    1.0 / (0.1 + near)
}

/// Where `depth` is in shadow of a light at (`light_x`, `light_y`) - fractions
/// of the picture from its top left - as 0 (lit) to 1 (in full shadow), on a
/// grid `SHADE_SIDE` on its long side: its width, height and values.
///
/// Every pixel is put back in the scene as a point in front of the camera,
/// and so is the light, a little nearer than what is at its point. The way
/// from each point to the light is walked; each step, seen from the camera,
/// lands on some pixel, and when what is at that pixel is nearer than the
/// step - but not so much nearer that it is a thing far in front of it - it
/// stands between the point and the light.
pub(crate) fn shadows(depth: &Depth, light_x: f32, light_y: f32) -> (usize, usize, Vec<f32>) {
    let scale = SHADE_SIDE as f32 / depth.width.max(depth.height) as f32;
    let (w, h) = (((depth.width as f32 * scale) as usize).max(2), ((depth.height as f32 * scale) as usize).max(2));
    let near_at = |u: f32, v: f32| -> Option<f32> {
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return None;
        }
        let x = ((u * depth.width as f32) as usize).min(depth.width - 1);
        let y = ((v * depth.height as f32) as usize).min(depth.height - 1);
        Some(depth.near[y * depth.width + x])
    };
    // the camera's view: a fraction of the picture across is this much across at unit distance
    let aspect = h as f32 / w as f32;
    let point = |u: f32, v: f32, z: f32| [(u - 0.5) * z, (v - 0.5) * aspect * z, z];
    let (lu, lv) = (light_x.clamp(0.0, 1.0), light_y.clamp(0.0, 1.0));
    let light = point(lu, lv, distance(near_at(lu.min(0.999), lv.min(0.999)).unwrap_or(0.2)) * 0.85);
    let mut shade = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            let Some(here) = near_at(u, v) else { continue };
            let p = point(u, v, distance(here));
            let mut blocked = 0f32;
            for step in 1..STEPS {
                let t = step as f32 / STEPS as f32;
                let s = [p[0] + (light[0] - p[0]) * t, p[1] + (light[1] - p[1]) * t, p[2] + (light[2] - p[2]) * t];
                if s[2] <= 0.05 {
                    break;
                }
                // where the step is seen, and how far what is there is
                let (su, sv) = (s[0] / s[2] + 0.5, s[1] / (s[2] * aspect) + 0.5);
                let Some(there) = near_at(su, sv) else { break };
                let z = distance(there);
                let (bias, thick) = (0.03 * s[2], 0.45 * s[2]);
                if z < s[2] - bias && z > s[2] - thick {
                    blocked += 1.0;
                }
            }
            shade[y * w + x] = (blocked / 5.0).min(1.0);
        }
    }
    (w, h, soften(&shade, w, h, 2))
}

/// The shadows `shade` (on a `w` by `h` grid) where the picture's own lights
/// leave them: faded where the picture is bright around a point already -
/// a lamp, a lit wall - as other light fills a shadow in; whole where it is
/// dim.
pub(crate) fn filled_by_other_lights(shade: &[f32], w: usize, h: usize, picture: &Picture) -> Vec<f32> {
    let (pw, ph) = (picture.width as usize, picture.height as usize);
    if pw == 0 || ph == 0 {
        return shade.to_vec();
    }
    // the picture's brightness on the grid, spread a little: the light round each point
    let mut lum = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let sx = ((x as f32 + 0.5) / w as f32 * pw as f32) as usize;
            let sy = ((y as f32 + 0.5) / h as f32 * ph as f32) as usize;
            let i = (sy.min(ph - 1) * pw + sx.min(pw - 1)) * 4;
            let [r, g, b] = [0, 1, 2].map(|c| picture.rgba[i + c] as f32 / 255.0);
            lum[y * w + x] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        }
    }
    let lum = soften(&lum, w, h, 4);
    let smooth = |e0: f32, e1: f32, v: f32| {
        let t = ((v - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    shade.iter().zip(&lum).map(|(s, l)| s * (1.0 - 0.9 * smooth(0.35, 0.85, *l))).collect()
}

/// A box blur `radius` across, twice: the shadow's edge made soft.
fn soften(values: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let mut a = values.to_vec();
    let mut b = vec![0f32; values.len()];
    for _ in 0..2 {
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x.saturating_sub(radius), (x + radius).min(w - 1));
                b[y * w + x] = (x0..=x1).map(|i| a[y * w + i]).sum::<f32>() / (x1 - x0 + 1) as f32;
            }
        }
        for y in 0..h {
            for x in 0..w {
                let (y0, y1) = (y.saturating_sub(radius), (y + radius).min(h - 1));
                a[y * w + x] = (y0..=y1).map(|i| b[i * w + x]).sum::<f32>() / (y1 - y0 + 1) as f32;
            }
        }
    }
    a
}

/// The colour a shadow takes: the light that still reaches into it, which
/// is the picture's own (its average colour, made as bright as white), with
/// a little of the far side of `key` (red, green, blue, alpha) - golden
/// light leaves shadows a touch cooler. Each channel 0 to 1, the largest 1.
pub(crate) fn shadow_colour(picture: &Picture, key: u32) -> [f32; 3] {
    let mut sum = [0f64; 3];
    let n = (picture.rgba.len() / 4).max(1);
    for px in picture.rgba.chunks_exact(4).step_by(7) {
        for c in 0..3 {
            sum[c] += f64::from(px[c]);
        }
    }
    let counted = (n / 7).max(1) as f64;
    let ambient = sum.map(|v| (v / counted / 255.0) as f32);
    let key = [(key >> 24) & 0xFF, (key >> 16) & 0xFF, (key >> 8) & 0xFF].map(|c| c as f32 / 255.0);
    let top = key.iter().cloned().fold(1e-3, f32::max);
    let opposite = key.map(|c| 1.0 - 0.5 * c / top);
    let mixed: Vec<f32> = (0..3).map(|c| 0.75 * ambient[c] + 0.25 * opposite[c]).collect();
    let top = mixed.iter().cloned().fold(1e-3, f32::max);
    [mixed[0] / top, mixed[1] / top, mixed[2] / top]
}

/// The shadows as a picture to multiply the clip by: white where lit, and
/// in shadow `strength` (0 to 1, the light's own strength) darker, in the
/// shadow's `colour`.
pub(crate) fn shade_picture(w: usize, h: usize, shade: &[f32], strength: f32, colour: [f32; 3]) -> Picture {
    let mut rgba = Vec::with_capacity(w * h * 4);
    for &s in shade {
        let k = (s * strength).clamp(0.0, 0.85);
        // towards the shadow's colour at its darkness
        let px = colour.map(|c| (255.0 * (1.0 - k) * (1.0 - k * (1.0 - c) * 0.6).max(0.0)) as u8);
        rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    Picture { width: w as u32, height: h as u32, rgba }
}

/// The depth model, fetched the first time.
pub(crate) fn model(report: &mut dyn FnMut(f32)) -> Result<std::path::PathBuf, String> {
    crate::objects::fetched(ModelId::Depth, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor running from the camera to a wall, a person standing on it,
    /// and a light high on the right at the back: the person's shadow falls
    /// on the floor in front of them, down to the left.
    #[test]
    fn a_person_throws_their_shadow_on_the_floor_away_from_the_light() {
        let (w, h) = (90, 160);
        let mut near = vec![0f32; w * h];
        for y in 0..h {
            let v = y as f32 / h as f32;
            for x in 0..w {
                // the wall far back above, the floor coming nearer below it
                near[y * w + x] = if v < 0.45 { 0.12 } else { 0.12 + (v - 0.45) / 0.55 * 0.88 };
            }
        }
        // the person: as near as the floor at their feet, standing up from it
        let feet = 0.12 + (0.75f32 - 0.45) / 0.55 * 0.88;
        for y in (0.3 * h as f32) as usize..(0.75 * h as f32) as usize {
            for x in (0.45 * w as f32) as usize..(0.55 * w as f32) as usize {
                near[y * w + x] = feet;
            }
        }
        let depth = Depth { width: w, height: h, near };
        let (sw, sh, shade) = shadows(&depth, 0.9, 0.1);
        let at = |x: f32, y: f32| shade[((y * sh as f32) as usize).min(sh - 1) * sw + ((x * sw as f32) as usize).min(sw - 1)];
        // on the floor down to the left of the feet it is darker than down to the right
        assert!(at(0.35, 0.82) > at(0.65, 0.82) + 0.2, "{} {}", at(0.35, 0.82), at(0.65, 0.82));
        // the wall high up, nothing between it and the light, is lit
        assert!(at(0.2, 0.1) < 0.2, "{}", at(0.2, 0.1));
    }

    /// In a warm-lit picture shadows are warm; golden light cools them a little.
    #[test]
    fn a_shadow_takes_the_colour_of_the_light_still_reaching_it() {
        let warm = Picture { width: 4, height: 4, rgba: [230u8, 170, 110, 255].repeat(16) };
        let [r, g, b] = shadow_colour(&warm, 0xFFC4_68FF);
        assert!(r > g && g > b && (r - 1.0).abs() < 1e-6, "{r} {g} {b}");
        let grey = Picture { width: 4, height: 4, rgba: [150u8, 150, 150, 255].repeat(16) };
        let [r, _, b] = shadow_colour(&grey, 0xFFC4_68FF);
        assert!(b > r, "under golden light a grey scene's shadows lean blue: {r} {b}");
        // a strong light makes a darker shadow than a faint one
        let shade = vec![1.0f32; 4];
        let strong = shade_picture(2, 2, &shade, 0.9, [1.0, 1.0, 1.0]);
        let faint = shade_picture(2, 2, &shade, 0.3, [1.0, 1.0, 1.0]);
        assert!(strong.rgba[0] < faint.rgba[0]);
    }

    /// A shadow over a bright part of the picture, lit by its own lamp, fades;
    /// over a dim part it stays.
    #[test]
    fn the_pictures_own_light_fills_a_shadow_in() {
        let (w, h) = (20, 10);
        let shade = vec![1f32; w * h];
        // the left half dim, the right half bright
        let rgba: Vec<u8> = (0..h).flat_map(|_| (0..w).flat_map(|x| if x < w / 2 { [40, 40, 40, 255] } else { [250, 245, 235, 255] })).collect();
        let picture = Picture { width: w as u32, height: h as u32, rgba };
        let filled = filled_by_other_lights(&shade, w, h, &picture);
        assert!(filled[5 * w + 1] > 0.9, "{}", filled[5 * w + 1]);
        assert!(filled[5 * w + w - 2] < 0.3, "{}", filled[5 * w + w - 2]);
    }
}
