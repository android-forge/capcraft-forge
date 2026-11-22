// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! The graphics library: vector pictures drawn at the size asked for, and
//! when on the timeline a placed one shows.

use concat_project::Command;
use concat_project::commands::{ClipMove, TrimEdge};
use resvg::{tiny_skia, usvg};

use crate::session::{MobileSession, TimelineView};
use crate::{MobileError, Picture, lanes};

/// The longest side a graphic is drawn at.
const LARGEST: u32 = 2048;

/// Draws an SVG body `width` by `height` in its own units, `side` pixels
/// along its longer side, its `currentColor` being `color` ("#rrggbb").
#[uniffi::export]
pub fn draw_graphic(
    body: String,
    width: f32,
    height: f32,
    side: u32,
    color: String,
) -> Result<Picture, MobileError> {
    if !(width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite()) {
        return Err(MobileError::Failed("That graphic has no size.".to_owned()));
    }
    let ink = if color.len() == 7 && color.starts_with('#') {
        color.as_str()
    } else {
        "#ffffff"
    };
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">{}</svg>",
        body.replace("currentColor", ink)
    );
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
        .map_err(|error| MobileError::Failed(format!("That graphic could not be read: {error}")))?;
    let scale = side.clamp(8, LARGEST) as f32 / width.max(height);
    let (w, h) = (
        ((width * scale).round() as u32).max(1),
        ((height * scale).round() as u32).max(1),
    );
    let mut drawn = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| MobileError::Failed("That graphic has no size.".to_owned()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut drawn.as_mut(),
    );
    Ok(Picture {
        width: w,
        height: h,
        // a Bitmap's pixels are premultiplied too, so these go as they are
        rgba: drawn.take(),
    })
}

#[uniffi::export]
impl MobileSession {
    /// Has clip `clip_id`, of a row other than main, show from `start_us`
    /// for `duration_us`: on its own lane where there is room, else on
    /// another of its kind. One undo step.
    pub fn set_window(
        &self,
        clip_id: String,
        start_us: i64,
        duration_us: i64,
    ) -> Result<TimelineView, MobileError> {
        let start = (start_us.max(0) as f64) / 1e6;
        let duration = (duration_us as f64 / 1e6).max(0.1);
        let mut state = self.lock();
        let timeline = state.session.project().active().clone();
        let clip = timeline
            .clips
            .iter()
            .find(|clip| clip.id == clip_id)
            .cloned()
            .ok_or_else(|| MobileError::Failed("That clip is gone.".to_owned()))?;
        if lanes::on_main(&timeline, &clip_id) {
            return Err(MobileError::Failed(
                "A clip of the main track stays in its place.".to_owned(),
            ));
        }
        let gesture = self.gesture();
        let done = (|| {
            let free =
                timeline.nearest_free_start(&clip.track_id, start, duration, &[clip_id.as_str()]);
            let lane = if (free - start).abs() < 1e-6 {
                clip.track_id.clone()
            } else {
                let kind = lanes::kind_of(&clip);
                match lanes::free_lane(&timeline, kind, start, duration) {
                    Some(lane) => lane,
                    None => Self::apply_all(
                        &mut state,
                        vec![Command::InsertTrack {
                            index: lanes::insert_index(&timeline, kind),
                        }],
                        Some(&gesture),
                    )?
                    .ok_or_else(|| MobileError::Failed("the new lane has no id".to_owned()))?,
                }
            };
            let trim = Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::End,
                delta: duration - clip.duration,
                ripple: false,
            };
            let moved = Command::MoveClips {
                moves: vec![ClipMove {
                    clip_id: clip_id.clone(),
                    track_id: lane,
                    start,
                }],
            };
            // a shorter one is cut before it moves, a longer one after, so
            // that neither runs into what is beside it
            let commands = if duration < clip.duration {
                vec![trim, moved]
            } else {
                vec![moved, trim]
            };
            Self::apply_all(&mut state, commands, Some(&gesture))
        })();
        state.session.end_gesture();
        done?;
        Ok(Self::changed(&mut state, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graphic_is_drawn_in_the_colour_asked_for() {
        let body = "<path fill=\"currentColor\" d=\"M0 0h16v16H0z\"/>".to_owned();
        let drawn = draw_graphic(body, 16.0, 16.0, 64, "#ff8000".to_owned()).expect("drawn");
        assert_eq!((drawn.width, drawn.height), (64, 64));
        let middle = (32 * 64 + 32) * 4;
        assert_eq!(&drawn.rgba[middle..middle + 4], &[255, 128, 0, 255]);
    }

    #[test]
    fn a_wide_graphic_keeps_its_shape() {
        let body = "<rect width=\"640\" height=\"200\" fill=\"#000\"/>".to_owned();
        let drawn = draw_graphic(body, 640.0, 200.0, 320, String::new()).expect("drawn");
        assert_eq!((drawn.width, drawn.height), (320, 100));
        assert!(draw_graphic("<".to_owned(), 10.0, 10.0, 32, String::new()).is_err());
        assert!(draw_graphic(String::new(), 0.0, 10.0, 32, String::new()).is_err());
    }
}
