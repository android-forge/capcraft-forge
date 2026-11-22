// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! Light people: whether the people in a picture are lit as they should be,
//! and by how much they are not.
//!
//! People are never brought to one brightness: skin is light and dark, and
//! dark skin in good light is not underexposed. What is looked at is the
//! bright end of them - the light on a cheek or a forehead, teeth, the white
//! of an eye, a shirt - which good light puts well up the scale whoever it
//! falls on, and how they stand against what is round them.

use concat_core::frame::{BYTES_PER_PIXEL, Frame};
use concat_vision::{Mask, Segmenter};

use crate::session::MobileSession;
use crate::{MobileError, seconds};

/// How the people in a picture are lit, and what would put it right.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PeopleLight {
    /// How much of the picture is people, 0 to 1.
    pub people: f32,
    /// How much light to give them, in stops; less than 0 takes some.
    pub exposure: f32,
    /// How far to lift their shadows, 0 to 100.
    pub shadows: f32,
    /// How far to ease their highlights, 0 to -100.
    pub highlights: f32,
}

/// A pixel this likely is a person's.
const SURELY: f32 = 0.7;
/// Where the bright end of well-lit people lies, as a picture stores it.
const BRIGHT_END: f32 = 0.74;
/// Under this it is too dark, over [`BLOWN`] too bright.
const DIM: f32 = 0.6;
const BLOWN: f32 = 0.95;
/// No more light than this is given, or taken.
const MOST: f32 = 1.8;

fn light(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// What `frame` asks for, its people where `mask` is.
fn measured(frame: &Frame, mask: &Mask) -> PeopleLight {
    let (w, h) = (frame.width() as usize, frame.height() as usize);
    let pixels = frame.pixels();
    let mut people = Vec::new();
    let mut others = Vec::new();
    for y in 0..h {
        let v = (y as f32 + 0.5) / h as f32;
        for x in 0..w {
            let at = (y * w + x) * BYTES_PER_PIXEL;
            let c = |i: usize| f32::from(pixels[at + i]) / 255.0;
            let bright = 0.2126 * c(0) + 0.7152 * c(1) + 0.0722 * c(2);
            let likely = mask.sample((x as f32 + 0.5) / w as f32, v);
            if likely >= SURELY {
                people.push(bright);
            } else if likely < 0.2 {
                others.push(bright);
            }
        }
    }
    let share = people.len() as f32 / (w * h).max(1) as f32;
    let mut asked = PeopleLight {
        people: share,
        exposure: 0.0,
        shadows: 0.0,
        highlights: 0.0,
    };
    if people.len() < 200 || share < 0.004 {
        asked.people = 0.0;
        return asked;
    }
    people.sort_by(f32::total_cmp);
    others.sort_by(f32::total_cmp);
    let at = |list: &[f32], part: f32| {
        list.get(((list.len() as f32 * part) as usize).min(list.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.5)
    };
    let (half, end) = (at(&people, 0.5), at(&people, 0.95));
    let round = at(&others, 0.5);
    if end < DIM {
        // four fifths of the way: a dark scene is meant to be one
        let whole = (light(BRIGHT_END) / light(end).max(0.002)).log2();
        // the middle of them is not pushed past the middle of the scale
        let room = (light(0.6) / light(half).max(0.002)).log2().max(0.0);
        asked.exposure = (whole * 0.8).min(room).clamp(0.0, MOST);
    }
    // lit from behind: darker than the scene by a long way, with no light on
    // them anywhere to say their skin or clothes are only dark
    if !others.is_empty() && half < round * 0.6 && half < 0.35 && end < 0.68 {
        asked.shadows = ((round * 0.6 - half) * 160.0).clamp(10.0, 45.0);
        asked.exposure = asked.exposure.max(0.4);
    }
    // burnt out: a tenth of them at the very top
    if at(&people, 0.9) > BLOWN || half > 0.8 {
        asked.exposure = -((light(half) / light(0.66)).log2()).clamp(0.2, 0.9);
        asked.highlights = -35.0;
        asked.shadows = 0.0;
    }
    // too little to see is nothing
    if asked.exposure.abs() < 0.1 && asked.shadows < 10.0 {
        asked.exposure = 0.0;
        asked.shadows = 0.0;
    }
    asked
}

#[uniffi::export]
impl MobileSession {
    /// How the people in clip `clip_id` are lit `at_us` into the timeline.
    pub fn people_light(&self, clip_id: String, at_us: i64) -> Result<PeopleLight, MobileError> {
        let (path, time) = self.with_session(|session| {
            let project = session.project();
            let clip = project
                .active()
                .clips
                .iter()
                .find(|clip| clip.id == clip_id)
                .ok_or_else(|| MobileError::Failed("That clip is gone.".to_owned()))?;
            let media = project
                .media
                .iter()
                .find(|item| item.id == clip.media_id)
                .ok_or_else(|| MobileError::Failed("The clip's file is gone.".to_owned()))?;
            let into = (seconds(at_us) - clip.start).clamp(0.0, clip.duration.max(0.0));
            Ok((
                media.path.clone(),
                clip.source_start + into * clip.speed.max(1e-6),
            ))
        })?;
        let frame = concat_host::media::still_at(&path, time, 640)?;
        let picture = crate::Picture::of(frame);
        let frame = Frame::from_rgba(picture.width, picture.height, picture.rgba)
            .ok_or_else(|| MobileError::Failed("The picture could not be read.".to_owned()))?;
        let mask = Segmenter::selfie()
            .and_then(|model| model.mask(&frame))
            .map_err(MobileError::Failed)?;
        Ok(measured(&frame, &mask))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture of one brightness with a person in its middle, some of them
    /// a little brighter.
    fn scene(round: u8, person: u8, bright: u8) -> (Frame, Mask) {
        let mut frame = Frame::black(200, 200);
        let mut mask = Mask::filled(200, 200, 0);
        for y in 0..200u32 {
            for x in 0..200u32 {
                let inside = (70..130).contains(&x) && (40..200).contains(&y);
                let v = if !inside {
                    round
                } else if y < 60 {
                    bright
                } else {
                    person
                };
                frame.set_pixel(x, y, [v, v, v, 255]);
                if inside {
                    mask.bytes_mut()[(y * 200 + x) as usize] = 255;
                }
            }
        }
        (frame, mask)
    }

    fn asked(round: u8, person: u8, bright: u8) -> PeopleLight {
        let (frame, mask) = scene(round, person, bright);
        measured(&frame, &mask)
    }

    #[test]
    fn dark_skin_in_good_light_is_left_alone() {
        let light = asked(130, 75, 195);
        assert_eq!((light.exposure, light.shadows), (0.0, 0.0), "{light:?}");
    }

    #[test]
    fn someone_in_the_dark_is_given_light_and_not_too_much() {
        let light = asked(60, 30, 70);
        assert!(light.exposure > 0.6 && light.exposure <= MOST, "{light:?}");
    }

    #[test]
    fn someone_against_the_light_has_their_shadows_lifted() {
        let light = asked(220, 45, 110);
        assert!(light.shadows >= 10.0 && light.exposure > 0.0, "{light:?}");
    }

    #[test]
    fn someone_burnt_out_is_given_less() {
        let light = asked(150, 235, 252);
        assert!(light.exposure < -0.1 && light.highlights < 0.0, "{light:?}");
    }

    #[test]
    fn with_no_one_in_it_nothing_is_asked() {
        let (frame, _) = scene(100, 100, 100);
        let light = measured(&frame, &Mask::filled(8, 8, 0));
        assert_eq!((light.people, light.exposure), (0.0, 0.0));
    }
}
