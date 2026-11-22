// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! The people and things in a photo: found, outlined, taken out of it, or
//! put over the video as a picture of their own. The clip may be on the main
//! track or an overlay. Every model runs on the phone, downloaded the first
//! time it is wanted.
//!
//! Taking out writes a new picture in the project and shows it in the
//! clip's place, as one undo step; the photo itself is not changed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use concat_core::frame::Frame;
use concat_project::Command;
use concat_project::commands::{ClipPatch, TrimEdge};
use concat_project::model::{Clip, ClipKind, KeyProperty, MediaKind};
use concat_vision::detect::{Detector, Found};
use concat_vision::erase::{Eraser, Gaps};
use concat_vision::models::{ModelId, installed, model_file};
use concat_vision::outline::{self, Outline};
use concat_vision::{Brush, Embedding, Mask};

use crate::lanes::RowKind;
use crate::session::{MobileSession, TimelineView};
use crate::speech::SpeechListener;
use crate::{MobileError, context};

/// A person or thing found in a photo: its box, in fractions of the picture.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Thing {
    /// What it is ("person", "dog").
    pub kind: String,
    /// The box's left edge.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
}

/// The outline of a thing, over a part of the picture.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Patch {
    /// The part's left edge, in fractions of the picture.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
    /// The mask's width, pixels.
    pub mask_width: u32,
    /// Its height.
    pub mask_height: u32,
    /// How much of each pixel is the thing, 0 to 255, rows top to bottom.
    pub mask: Vec<u8>,
}

/// What looking for things, or at one, reports.
#[uniffi::export(with_foreign)]
pub trait ThingListener: Send + Sync {
    /// How far along it is, 0 to 1, and what it is doing ("download", "work").
    fn on_progress(&self, fraction: f32, stage: String);
    /// What is in the photo, people first.
    fn on_found(&self, things: Vec<Thing>);
    /// The outline asked for.
    fn on_outline(&self, patch: Patch);
    /// It failed or was cancelled.
    fn on_failed(&self, message: String);
}

/// The photo looked at last, as decoded, and as the outline model read it
/// whole: a second tap on it is quick.
struct Looked {
    path: String,
    frame: Arc<Frame>,
    read: Option<Arc<Embedding>>,
}

#[derive(Default)]
struct Models {
    detector: Option<Detector>,
    brush: Option<Arc<Brush>>,
    eraser: Option<Eraser>,
    looked: Option<Looked>,
}

/// The models, one job at a time.
fn models() -> &'static Mutex<Models> {
    static ONE: OnceLock<Mutex<Models>> = OnceLock::new();
    ONE.get_or_init(Mutex::default)
}

static STOP: AtomicBool = AtomicBool::new(false);

/// Asks the job in progress to stop.
#[uniffi::export]
pub fn cancel_objects() {
    STOP.store(true, Ordering::Relaxed);
}

/// Gives the models' memory back, for when the panel closes.
#[uniffi::export]
pub fn release_objects() {
    STOP.store(true, Ordering::Relaxed);
    if let Ok(mut held) = models().try_lock() {
        *held = Models::default();
    }
}

/// How big a download the models still wanted are, in bytes; 0 when they are
/// all here.
#[uniffi::export]
pub fn objects_download() -> u64 {
    let Ok(context) = context() else { return 0 };
    [
        ModelId::Detect,
        ModelId::BrushEncoder,
        ModelId::BrushDecoder,
        ModelId::Erase,
    ]
    .into_iter()
    .filter(|id| !installed(&context.dirs.data, *id))
    .map(|id| id.spec().bytes)
    .sum()
}

/// The model's file, downloaded first where it is not here.
fn fetched(id: ModelId, report: &mut dyn FnMut(f32)) -> Result<PathBuf, String> {
    let data = context().map_err(|error| error.to_string())?.dirs.data.clone();
    let file = model_file(&data, id);
    if installed(&data, id) {
        return Ok(file);
    }
    let spec = id.spec();
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("could not create {}: {error}", folder.display()))?;
    }
    let partial = file.with_extension("part");
    concat_host::models::fetch(
        spec.file,
        spec.upstream,
        &partial,
        spec.bytes,
        spec.sha256,
        &STOP,
        "Stopped.",
        &mut |received, total| report(received as f32 / total.max(1) as f32),
    )?;
    std::fs::rename(&partial, &file)
        .map_err(|error| format!("could not keep the model: {error}"))?;
    Ok(file)
}

impl Models {
    fn detector(&mut self, report: &mut dyn FnMut(f32)) -> Result<&mut Detector, String> {
        if self.detector.is_none() {
            self.detector = Some(Detector::load(&fetched(ModelId::Detect, report)?)?);
        }
        self.detector.as_mut().ok_or_else(|| "no detector".to_owned())
    }

    fn brush(&mut self, report: &mut dyn FnMut(f32)) -> Result<Arc<Brush>, String> {
        if let Some(brush) = &self.brush {
            return Ok(Arc::clone(brush));
        }
        let encoder = fetched(ModelId::BrushEncoder, &mut |f| report(f * 0.6))?;
        let decoder = fetched(ModelId::BrushDecoder, &mut |f| report(0.6 + f * 0.4))?;
        let brush = Arc::new(Brush::load(&encoder, &decoder)?);
        self.brush = Some(Arc::clone(&brush));
        Ok(brush)
    }

    fn eraser(&mut self, report: &mut dyn FnMut(f32)) -> Result<&mut Eraser, String> {
        if self.eraser.is_none() {
            self.eraser = Some(Eraser::load(&fetched(ModelId::Erase, report)?)?);
        }
        self.eraser.as_mut().ok_or_else(|| "no eraser".to_owned())
    }

    /// The photo at `path`, decoded once.
    fn look(&mut self, path: &str) -> Result<Arc<Frame>, String> {
        if let Some(looked) = &self.looked
            && looked.path == path
        {
            return Ok(Arc::clone(&looked.frame));
        }
        let frame = Arc::new(photo(path)?);
        self.looked = Some(Looked {
            path: path.to_owned(),
            frame: Arc::clone(&frame),
            read: None,
        });
        Ok(frame)
    }

    /// The photo read whole by the outline model.
    fn read(&mut self, brush: &Brush) -> Result<Arc<Embedding>, String> {
        let looked = self.looked.as_mut().ok_or("no photo")?;
        if let Some(read) = &looked.read {
            return Ok(Arc::clone(read));
        }
        let read = Arc::new(outline::read_whole(brush, &looked.frame)?);
        looked.read = Some(Arc::clone(&read));
        Ok(read)
    }
}

/// The picture in `path` at its own size, eight bits a channel.
pub(crate) fn photo(path: &str) -> Result<Frame, String> {
    let info = concat_media::probe(path).map_err(|error| error.to_string())?;
    let video = info.require_video().map_err(|error| error.to_string())?;
    let frame = concat_host::media::still_at(path, 0.0, video.width.max(2))?;
    let picture = crate::Picture::of(frame);
    Frame::from_rgba(picture.width, picture.height, picture.rgba)
        .ok_or_else(|| "The photo could not be read.".to_owned())
}

fn patch_of(outline: Outline) -> Patch {
    Patch {
        x: outline.x,
        y: outline.y,
        width: outline.width,
        height: outline.height,
        mask_width: outline.mask.width(),
        mask_height: outline.mask.height(),
        mask: outline.mask.bytes().to_vec(),
    }
}

fn outline_of(patch: Patch) -> Option<Outline> {
    Some(Outline {
        x: patch.x,
        y: patch.y,
        width: patch.width,
        height: patch.height,
        mask: Mask::from_bytes(patch.mask_width, patch.mask_height, patch.mask)?,
    })
}

/// A photo clip and its file.
pub(crate) struct Shown {
    pub clip: Clip,
    pub path: String,
    pub project: String,
}

pub(crate) fn shown(session: &MobileSession, clip_id: &str) -> Result<Shown, MobileError> {
    session.with_session(|session| {
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
        if clip.kind != ClipKind::Image || media.kind != MediaKind::Image {
            return Err(MobileError::Failed(
                "This works on photos. Freeze a frame of the video first.".to_owned(),
            ));
        }
        Ok(Shown {
            clip: Clip::clone(clip),
            path: media.path.clone(),
            project: session.path().to_owned(),
        })
    })
}

/// Runs `work` on a thread of its own, its failure told to `listener`.
fn on_thread(
    name: &str,
    listener: Arc<dyn ThingListener>,
    work: impl FnOnce(&mut Models, &dyn ThingListener) -> Result<(), String> + Send + 'static,
) -> Result<(), MobileError> {
    STOP.store(false, Ordering::Relaxed);
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let done = models()
                .lock()
                .map_err(|_| "the models are unwell".to_owned())
                .and_then(|mut held| work(&mut held, listener.as_ref()));
            if let Err(message) = done {
                listener.on_failed(message);
            }
        })
        .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
    Ok(())
}

#[uniffi::export]
impl MobileSession {
    /// Finds the people and things in photo `clip_id`, people first.
    pub fn find_things(
        self: Arc<Self>,
        clip_id: String,
        listener: Arc<dyn ThingListener>,
    ) -> Result<(), MobileError> {
        let shown = shown(&self, &clip_id)?;
        on_thread("capcraft-find", listener, move |models, listener| {
            let frame = models.look(&shown.path)?;
            let detector =
                models.detector(&mut |f| listener.on_progress(f, "download".to_owned()))?;
            listener.on_progress(0.0, "work".to_owned());
            let things = detector
                .find(&frame)?
                .into_iter()
                .take(24)
                .map(|found| Thing {
                    kind: found.kind.to_owned(),
                    x: found.x,
                    y: found.y,
                    width: found.width,
                    height: found.height,
                })
                .collect();
            listener.on_found(things);
            Ok(())
        })
    }

    /// The outline of `thing` in photo `clip_id`.
    pub fn outline_thing(
        self: Arc<Self>,
        clip_id: String,
        thing: Thing,
        listener: Arc<dyn ThingListener>,
    ) -> Result<(), MobileError> {
        let shown = shown(&self, &clip_id)?;
        on_thread("capcraft-outline", listener, move |models, listener| {
            let frame = models.look(&shown.path)?;
            let brush = models.brush(&mut |f| listener.on_progress(f, "download".to_owned()))?;
            listener.on_progress(0.0, "work".to_owned());
            let found = Found {
                kind: if thing.kind == "person" {
                    "person"
                } else {
                    "thing"
                },
                score: 1.0,
                x: thing.x,
                y: thing.y,
                width: thing.width,
                height: thing.height,
            };
            listener.on_outline(patch_of(outline::of(&brush, &frame, &found)?));
            Ok(())
        })
    }

    /// The outline of what is at (`x`, `y`) of photo `clip_id`, fractions of
    /// its picture.
    pub fn outline_at(
        self: Arc<Self>,
        clip_id: String,
        x: f32,
        y: f32,
        listener: Arc<dyn ThingListener>,
    ) -> Result<(), MobileError> {
        let shown = shown(&self, &clip_id)?;
        on_thread("capcraft-outline", listener, move |models, listener| {
            models.look(&shown.path)?;
            let brush = models.brush(&mut |f| listener.on_progress(f, "download".to_owned()))?;
            listener.on_progress(0.0, "work".to_owned());
            let read = models.read(&brush)?;
            let found = outline::at(&brush, &read, [f64::from(x), f64::from(y)])?;
            listener.on_outline(patch_of(found));
            Ok(())
        })
    }

    /// Takes what `patches` outline out of photo `clip_id` and fills in what
    /// was behind, as one undo step. Runs on a thread of its own.
    pub fn erase(
        self: Arc<Self>,
        clip_id: String,
        patches: Vec<Patch>,
        listener: Arc<dyn SpeechListener>,
    ) -> Result<(), MobileError> {
        let shown = shown(&self, &clip_id)?;
        let outlines: Vec<Outline> = patches.into_iter().filter_map(outline_of).collect();
        STOP.store(false, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("capcraft-erase".to_owned())
            .spawn(move || {
                let made = models()
                    .lock()
                    .map_err(|_| "the models are unwell".to_owned())
                    .and_then(|mut held| {
                        let frame = held.look(&shown.path)?;
                        let gaps = Gaps::of(&outlines, frame.width(), frame.height());
                        if gaps.is_empty() {
                            return Err("Choose something to remove.".to_owned());
                        }
                        // a picture with see-through parts has nothing behind it
                        if frame.pixels().chunks_exact(4).any(|p| p[3] < 128) {
                            return Ok(gaps.cleared(Frame::clone(&frame)));
                        }
                        let eraser =
                            held.eraser(&mut |f| listener.on_progress(f, "download".to_owned()))?;
                        eraser.erase(Frame::clone(&frame), &gaps, &mut |f| {
                            listener.on_progress(f, "work".to_owned());
                            !STOP.load(Ordering::Relaxed)
                        })
                    })
                    .and_then(|frame| keep(&shown, "removed", frame))
                    .map_err(MobileError::Failed)
                    .and_then(|path| {
                        let item = concat_host::media::probe(&path)?.to_new_media();
                        self.run(vec![Command::ReplaceClipMedia {
                            clip_id,
                            item,
                            source_start: Some(0.0),
                            enhanced: false,
                            offset: 0.0,
                        }])
                    });
                match made {
                    Ok(view) => listener.on_done(Some(view)),
                    Err(error) => listener.on_failed(error.to_string()),
                }
            })
            .map_err(|error| MobileError::Failed(format!("could not start: {error}")))?;
        Ok(())
    }

    /// Puts what `patch` outlines in photo `clip_id` over the video as a
    /// picture of its own, just where it is in the photo and for as long, as
    /// one undo step.
    pub fn lift(&self, clip_id: String, patch: Patch) -> Result<TimelineView, MobileError> {
        let shown = shown(self, &clip_id)?;
        let outline = outline_of(patch)
            .ok_or_else(|| MobileError::Failed("Its outline could not be read.".to_owned()))?;
        let frame = models()
            .lock()
            .map_err(|_| MobileError::Failed("the models are unwell".to_owned()))?
            .look(&shown.path)
            .map_err(MobileError::Failed)?;
        let (piece, at) = cut_out(&frame, &outline)
            .ok_or_else(|| MobileError::Failed("There is nothing there.".to_owned()))?;
        let path = keep(&shown, "cutout", piece).map_err(MobileError::Failed)?;
        let media = self.import(path)?;
        let frame_size = self.with_session(|session| {
            let settings = session.settings();
            Ok((f64::from(settings.width), f64::from(settings.height)))
        })?;
        let place = placed(
            &shown.clip,
            (f64::from(frame.width()), f64::from(frame.height())),
            frame_size,
            at,
        );
        let clip = shown.clip;
        self.place_many(
            RowKind::Overlay,
            clip.start,
            clip.duration,
            |track| {
                vec![Command::AddClip {
                    media_id: media.id.clone(),
                    track_id: track,
                    start: clip.start,
                    ripple: false,
                }]
            },
            |timeline| {
                let Some(new) = timeline.clips.iter().find(|c| c.media_id == media.id) else {
                    return Vec::new();
                };
                vec![
                    Command::TrimClip {
                        clip_id: new.id.clone(),
                        edge: TrimEdge::End,
                        delta: clip.duration - new.duration,
                        ripple: false,
                    },
                    Command::SetClipTransform {
                        clip_id: new.id.clone(),
                        scale: Some(place.scale),
                        offset_x: Some(place.x),
                        offset_y: Some(place.y),
                        rotation: Some(place.rotation),
                        stretch_x: Some(clip.stretch_x),
                        stretch_y: Some(clip.stretch_y),
                    },
                    Command::UpdateClip {
                        clip_id: new.id.clone(),
                        patch: ClipPatch {
                            flip_h: Some(clip.flip_h),
                            flip_v: Some(clip.flip_v),
                            ..ClipPatch::default()
                        },
                    },
                ]
            },
        )
    }
}

/// Writes `frame` in the project's media, named for the photo it is of.
pub(crate) fn keep(shown: &Shown, what: &str, frame: Frame) -> Result<String, String> {
    let folder = Path::new(&shown.project).join("media");
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("could not create {}: {error}", folder.display()))?;
    let stem = Path::new(&shown.path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    // a photo taken from more than once keeps its name
    let stem = stem.split(" - ").next().unwrap_or_default().to_owned();
    let path = (1..)
        .map(|n| folder.join(format!("{stem} - {what} {n}.png")))
        .find(|path| !path.exists())
        .unwrap_or_else(|| folder.join(format!("{stem}.png")));
    let picture = crate::Picture {
        width: frame.width(),
        height: frame.height(),
        rgba: frame.into_pixels(),
    };
    crate::session::write_png(&path, &picture).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// What `outline` is over, cut out with a soft edge, and where it is in the
/// picture: left, top, width and height, in fractions.
fn cut_out(frame: &Frame, outline: &Outline) -> Option<(Frame, [f64; 4])> {
    let (w, h) = (frame.width() as usize, frame.height() as usize);
    let share = |x: usize, y: usize| {
        let u = ((x as f32 + 0.5) / w as f32 - outline.x) / outline.width;
        let v = ((y as f32 + 0.5) / h as f32 - outline.y) / outline.height;
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return 0.0;
        }
        // the edge a little inside the outline, so nothing from behind shows
        ((outline.mask.sample(u, v) - 0.45) / 0.35).clamp(0.0, 1.0)
    };
    let x0 = (outline.x * w as f32).floor().max(0.0) as usize;
    let y0 = (outline.y * h as f32).floor().max(0.0) as usize;
    let x1 = (((outline.x + outline.width) * w as f32).ceil() as usize).min(w);
    let y1 = (((outline.y + outline.height) * h as f32).ceil() as usize).min(h);
    let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            if share(x, y) > 0.02 {
                (left, top) = (left.min(x), top.min(y));
                (right, bottom) = (right.max(x + 1), bottom.max(y + 1));
            }
        }
    }
    if right <= left + 1 || bottom <= top + 1 {
        return None;
    }
    // even sides, as the engine's pictures are
    let mut width = right - left;
    let mut height = bottom - top;
    width -= width % 2;
    height -= height % 2;
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in top..top + height {
        for x in left..left + width {
            let p = frame.pixel(x as u32, y as u32)?;
            let alpha = f32::from(p[3]) * share(x, y);
            pixels.extend_from_slice(&[p[0], p[1], p[2], (alpha + 0.5) as u8]);
        }
    }
    let at = [
        left as f64 / w as f64,
        top as f64 / h as f64,
        width as f64 / w as f64,
        height as f64 / h as f64,
    ];
    Frame::from_rgba(width as u32, height as u32, pixels).map(|piece| (piece, at))
}

/// Where an overlay goes.
struct Placed {
    scale: f64,
    x: f64,
    y: f64,
    rotation: f64,
}

/// Where a piece of a photo, at `at` in it, lies on a frame `frame_size`
/// when the photo (`size` pixels) is shown as `clip` shows it at its start.
fn placed(clip: &Clip, size: (f64, f64), frame_size: (f64, f64), at: [f64; 4]) -> Placed {
    let value = |property| clip.value_at(property, 0.0);
    let scale = value(KeyProperty::Scale);
    let rotation = value(KeyProperty::Rotation);
    let (cl, ct, cr, cb) = clip
        .crop
        .as_ref()
        .map_or((0.0, 0.0, 0.0, 0.0), |c| (c.left, c.top, c.right, c.bottom));
    // the part of the photo shown, and how much it is enlarged to fit
    let (sw, sh) = (
        size.0 * (1.0 - cl - cr).max(0.01),
        size.1 * (1.0 - ct - cb).max(0.01),
    );
    let fit = (frame_size.0 / sw).min(frame_size.1 / sh) * scale;
    // the piece's middle from the shown part's, in the frame's pixels
    let mut dx = ((at[0] + at[2] / 2.0) * size.0 - cl * size.0 - sw / 2.0) * fit * clip.stretch_x;
    let mut dy = ((at[1] + at[3] / 2.0) * size.1 - ct * size.1 - sh / 2.0) * fit * clip.stretch_y;
    if clip.flip_h {
        dx = -dx;
    }
    if clip.flip_v {
        dy = -dy;
    }
    let turn = rotation.to_radians();
    let (tx, ty) = (
        dx * turn.cos() - dy * turn.sin(),
        dx * turn.sin() + dy * turn.cos(),
    );
    let (pw, ph) = (at[2] * size.0, at[3] * size.1);
    let own = (frame_size.0 / pw.max(1.0)).min(frame_size.1 / ph.max(1.0));
    Placed {
        scale: fit / own,
        x: value(KeyProperty::OffsetX) + tx / frame_size.0,
        y: value(KeyProperty::OffsetY) + ty / frame_size.1,
        rotation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_piece_is_cut_to_its_own_size_with_nothing_round_it() {
        let mut frame = Frame::black(100, 80);
        frame.fill([10, 20, 30, 255]);
        let outline = Outline {
            x: 0.2,
            y: 0.25,
            width: 0.4,
            height: 0.5,
            mask: Mask::filled(32, 32, 255),
        };
        let (piece, at) = cut_out(&frame, &outline).expect("something");
        assert_eq!((piece.width(), piece.height()), (40, 40));
        assert_eq!(piece.pixel(20, 20), Some([10, 20, 30, 255]));
        assert!((at[0] - 0.2).abs() < 0.02 && (at[2] - 0.4).abs() < 0.03, "{at:?}");
        let nothing = Outline {
            mask: Mask::filled(32, 32, 0),
            ..outline
        };
        assert!(cut_out(&frame, &nothing).is_none());
    }

    #[test]
    fn a_piece_lies_over_where_it_was() {
        let clip = Clip::default();
        // the right half of a photo twice as wide as tall, on a square frame
        let place = placed(&clip, (2000.0, 1000.0), (1000.0, 1000.0), [0.5, 0.0, 0.5, 1.0]);
        // half a square frame's size, its middle a quarter to the right
        assert!((place.scale - 0.5).abs() < 1e-9, "{}", place.scale);
        assert!((place.x - 0.25).abs() < 1e-9, "{}", place.x);
        assert!(place.y.abs() < 1e-9);
        let flipped = Clip {
            flip_h: true,
            ..Clip::default()
        };
        let place = placed(&flipped, (2000.0, 1000.0), (1000.0, 1000.0), [0.5, 0.0, 0.5, 1.0]);
        assert!((place.x + 0.25).abs() < 1e-9);
    }
}
