// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! The flattened clip list becoming the engine's timeline.
//!
//! `flatten` turns the document into `ExportClip`s; this turns those into
//! a `concat_core::Timeline` plus the per-clip facts the decoders and the
//! compositor need that the engine's model has no field for. It is the
//! one place a document value stops being approximate: every time is
//! quantised to the frame grid here, and every chain, pass, size and mask
//! a clip renders with is decided here. `render` and the preview read
//! what this builds and never look at an `ExportClip` again.

use super::*;
use concat_vision::stabilize::{self, Correction, Motion};

/// The tint a highlighted cutout wears: the interface's accent, the same
/// lime the brushes and the selection are drawn in.
pub(crate) const HIGHLIGHT: [u8; 3] = [0xcb, 0xf5, 0x3f];

/// A cutout as the frame loop runs it: the masks, what to paint on them,
/// and how a decoded pixel finds its place in the source.
pub(crate) struct CutoutJob {
    pub(crate) store: MaskStore,
    pub(crate) cutout: Cutout,
    pub(crate) mapping: Mapping,
    /// The source mapping in the frame-plan path, where crop and flips are
    /// drawn by the compositor instead of baked into decoded pixels.
    matte_mapping: Mapping,
    /// The source's width over its height, for round brushes.
    pub(crate) aspect: f32,
    /// A short cache of alpha mattes ready for the compositor's mask input.
    matte_frames:
        std::sync::Mutex<std::collections::VecDeque<(Arc<concat_vision::Mask>, Arc<Frame>)>>,
}

impl CutoutJob {
    /// The job for a clip, or `None` when it has no cutout or no masks to
    /// cut with.
    /// `treated` says whether the frame the cut is given has the clip's
    /// crop and flips in it already: it has when they run in the decoder's
    /// chain, and not when the frame plan draws them (see
    /// [`planned_geometry`]); the mask is mapped onto the frame as it is.
    pub(crate) fn of(clip: &ExportClip, treated: bool) -> Option<CutoutJob> {
        let cutout = clip.cutout.clone()?;
        if clip.mask_dir.is_empty() {
            return None;
        }
        let aspect = match (clip.media_width, clip.media_height) {
            (Some(width), Some(height)) if width > 0 && height > 0 => width as f32 / height as f32,
            _ => 1.0,
        };
        Some(CutoutJob {
            store: MaskStore::open(Path::new(&clip.mask_dir)),
            cutout,
            mapping: Mapping {
                crop: clip
                    .crop
                    .filter(|_| treated)
                    .map(|edges| edges.map(|edge| edge as f32))
                    .unwrap_or([0.0; 4]),
                flip_h: clip.flip_h && treated,
                flip_v: clip.flip_v && treated,
            },
            matte_mapping: Mapping {
                crop: clip
                    .crop
                    .map(|edges| edges.map(|edge| edge as f32))
                    .unwrap_or([0.0; 4]),
                flip_h: clip.flip_h,
                flip_v: clip.flip_v,
            },
            aspect,
            matte_frames: std::sync::Mutex::new(std::collections::VecDeque::new()),
        })
    }

    fn resolved_mask(&self, source_time: Rational) -> Option<Arc<concat_vision::Mask>> {
        let mask = self
            .store
            .resolved(source_time.as_f64(), &self.cutout, self.aspect)?;
        if !self.cutout.invert {
            return Some(mask);
        }
        let mut turned = (*mask).clone();
        turned.invert();
        Some(Arc::new(turned))
    }

    /// Builds a small alpha texture for the GPU. The decoded video remains
    /// on its imported GPU texture; only the cutout matte is resampled.
    pub(crate) fn matte_frame(&self, source_time: Rational) -> Option<Arc<Frame>> {
        let mask = self
            .store
            .resolved(source_time.as_f64(), &self.cutout, self.aspect)?;
        if let Ok(cache) = self.matte_frames.lock()
            && let Some((_, frame)) = cache.iter().find(|(cached, _)| Arc::ptr_eq(cached, &mask))
        {
            return Some(Arc::clone(frame));
        }
        let (width, height) = (mask.width(), mask.height());
        let mut rgba = vec![0; Frame::byte_len(width, height)];
        for y in 0..height {
            for x in 0..width {
                let (u, v) = self.matte_mapping.source_of(
                    (x as f32 + 0.5) / width as f32,
                    (y as f32 + 0.5) / height as f32,
                );
                let keep = mask.sample(u, v);
                let alpha =
                    ((if self.cutout.invert { 1.0 - keep } else { keep }) * 255.0 + 0.5) as u8;
                let at = ((y * width + x) * 4) as usize;
                rgba[at..at + 4].copy_from_slice(&[255, 255, 255, alpha]);
            }
        }
        let frame = Arc::new(Frame::from_rgba(width, height, rgba)?);
        if let Ok(mut cache) = self.matte_frames.lock() {
            cache.push_back((mask, Arc::clone(&frame)));
            while cache.len() > 3 {
                cache.pop_front();
            }
        }
        Some(frame)
    }

    /// The frame with its background gone, when the instant has a mask.
    /// `None` leaves the picture whole: an instant not analysed yet is
    /// shown as shot rather than not at all.
    pub(crate) fn cut(&self, frame: &Frame, source_time: Rational) -> Option<Frame> {
        let mask = self.resolved_mask(source_time)?;
        let mut out = frame.clone();
        concat_vision::cut(&mut out, &mask, &self.mapping);
        Some(out)
    }

    /// The frame whole, with what the cutout keeps tinted over it: the
    /// painting view. `None` as for `cut`.
    pub(crate) fn highlight(&self, frame: &Frame, source_time: Rational) -> Option<Frame> {
        let mask = self.resolved_mask(source_time)?;
        let mut out = frame.clone();
        concat_vision::highlight(&mut out, &mask, &self.mapping, HIGHLIGHT);
        Some(out)
    }
}

/// Forge's reel effects as the frame loop runs them: the clip's effects,
/// drawn by `capcraft_reelfx` on each of its frames with the person its
/// masks keep in front and the face its track follows. An instant the
/// person or the face has not been found at yet is drawn with nobody in
/// front, the face a third of the way down the middle.
pub(crate) struct FxJob {
    /// Where the clip starts on the timeline, in seconds.
    clip_start: f64,
    runs: std::sync::Mutex<Vec<capcraft_reelfx::Run>>,
    person: MaskStore,
    faces: concat_vision::faces::FaceTrack,
    /// From the frame the effects are given to the source the masks and
    /// the face are kept in.
    mapping: Mapping,
    /// Where the card effects' pictures come from.
    media: String,
    source: (f64, f64),
    still: bool,
    images: Vec<String>,
    cards: std::sync::OnceLock<Arc<Vec<tiny_skia::Pixmap>>>,
}

impl FxJob {
    /// The job for a clip with reel effects; `treated` as for
    /// [`CutoutJob::of`].
    pub(crate) fn of(clip: &ExportClip, treated: bool) -> Option<FxJob> {
        let runs: Vec<capcraft_reelfx::Run> = clip
            .reel_fx
            .iter()
            .filter_map(|e| {
                let mut run = capcraft_reelfx::Run::new(&e.fx, e.start as f32, e.end as f32, e.options.clone())?;
                run.strength = e.strength.iter().map(|&(t, k)| (t as f32, k as f32)).collect();
                Some(run)
            })
            .collect();
        if runs.is_empty() {
            return None;
        }
        let images = runs
            .iter()
            .filter_map(|r| r.o.get("images"))
            .flat_map(|v| v.split('|').map(str::to_owned).collect::<Vec<_>>())
            .filter(|p| !p.is_empty())
            .collect();
        Some(FxJob {
            clip_start: clip.start,
            runs: std::sync::Mutex::new(runs),
            person: MaskStore::open(Path::new(&clip.person_dir)),
            faces: concat_vision::faces::FaceTrack::open(Path::new(&clip.face_file)),
            mapping: if treated {
                Mapping {
                    crop: clip.crop.map(|edges| edges.map(|edge| edge as f32)).unwrap_or([0.0; 4]),
                    flip_h: clip.flip_h,
                    flip_v: clip.flip_v,
                }
            } else {
                Mapping::IDENTITY
            },
            media: clip.path.clone(),
            source: (clip.source_start, clip.duration * clip.speed.max(0.0625)),
            still: clip.kind == ClipKind::Image,
            images,
            cards: std::sync::OnceLock::new(),
        })
    }

    /// Whether an effect is on at `time` of the timeline.
    pub(crate) fn covers(&self, time: Rational) -> bool {
        let t = (time.as_f64() - self.clip_start) as f32;
        self.runs.lock().is_ok_and(|runs| runs.iter().any(|r| r.covers(t)))
    }

    /// The pictures the card effects lay out: the user's own, or ten of
    /// the clip's frames across what it shows.
    fn cards(&self) -> Arc<Vec<tiny_skia::Pixmap>> {
        Arc::clone(self.cards.get_or_init(|| {
            let read = |path: &str, at: f64, long: u32| -> Option<tiny_skia::Pixmap> {
                let info = concat_media::probe::probe(Path::new(path)).ok()?;
                let video = info.video?;
                let scale = long as f32 / video.width.max(video.height).max(1) as f32;
                let (w, h) = (((video.width as f32 * scale) as u32).max(2) & !1, ((video.height as f32 * scale) as u32).max(2) & !1);
                let options = concat_media::DecodeOptions::default()
                    .starting_at(Rational::approximate(at).unwrap_or(Rational::ZERO))
                    .scaled_to(w, h)
                    .limited_to(1);
                let mut decoder = concat_media::Decoder::open(Path::new(path), &options).ok()?;
                let frame = concat_media::FrameSource::next_frame(&mut decoder).ok()??;
                let mut pm = tiny_skia::Pixmap::new(frame.width(), frame.height())?;
                if frame.pixels().len() != pm.data().len() {
                    return None;
                }
                pm.data_mut().copy_from_slice(frame.pixels());
                Some(pm)
            };
            let own: Vec<tiny_skia::Pixmap> = self.images.iter().filter_map(|p| read(p, 0.0, 540)).collect();
            if !own.is_empty() {
                return Arc::new(own);
            }
            let (from, len) = self.source;
            let n = if self.still { 1 } else { 10 };
            Arc::new((0..n).filter_map(|j| read(&self.media, from + len * (f64::from(j) + 0.5) / f64::from(n), 480)).collect())
        }))
    }

    /// `frame` with the effects on at `time` drawn on it, or `None` when
    /// none is, or the frame is not eight-bit pixels in memory.
    pub(crate) fn apply(&self, frame: &Frame, time: Rational, source_time: Rational) -> Option<Frame> {
        let t = (time.as_f64() - self.clip_start) as f32;
        let mut runs = self.runs.lock().ok()?;
        if !runs.iter().any(|r| r.covers(t)) {
            return None;
        }
        let (w, h) = (frame.width() as usize, frame.height() as usize);
        if w == 0 || h == 0 || frame.pixels().len() != w * h * 4 {
            return None;
        }
        if runs.iter().any(|r| r.covers(t) && (r.effect.cards || r.o.contains_key("images")) && r.cards.is_empty()) {
            let cards = self.cards();
            for r in runs.iter_mut().filter(|r| r.effect.cards || r.o.contains_key("images")) {
                r.cards = Arc::clone(&cards);
            }
        }
        let seconds = source_time.as_f64();
        let mask = self.person.mask_at(seconds).map(|mask| {
            let mut m = vec![0f32; w * h];
            for y in 0..h {
                for x in 0..w {
                    let (u, v) = self.mapping.source_of((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                    m[y * w + x] = mask.sample(u, v);
                }
            }
            m
        });
        let face = self.faces.at(seconds).and_then(|f| {
            let (x, y) = self.mapping.frame_of(f.x, f.y)?;
            let [left, _, right, _] = self.mapping.crop;
            Some(capcraft_reelfx::Face { x, y, w: f.w / (1.0 - left - right).max(1e-3) })
        });
        let mut rgba = frame.pixels().to_vec();
        if !capcraft_reelfx::draw(&mut rgba, w, h, mask.as_deref(), face, &mut runs, t) {
            return None;
        }
        Frame::from_rgba(frame.width(), frame.height(), rgba)
    }
}

/// Retouch's skin as the frame loop reads it: the skin model's masks of the
/// clip's media, mapped onto the picture its passes see, as a map the
/// Retouch shader reads through `reveal_order()` (see
/// `capcraft.retouch/effect.wgsl`). A pixel is `1 + 254 × skin`, so the
/// shader can tell a map from none, which reads 0 everywhere: an instant
/// not analysed yet is retouched by colour, as before.
pub(crate) struct SkinJob {
    store: MaskStore,
    mapping: Mapping,
    /// The maps made last, by the mask they were made from.
    made: std::sync::Mutex<std::collections::VecDeque<(Arc<concat_vision::Mask>, Arc<RevealMap>)>>,
}

impl SkinJob {
    /// The job for a retouched clip whose skin has been analysed, or `None`.
    /// `treated` as for [`CutoutJob::of`]: whether the clip's crop and flips
    /// are in the frame already. When they are not, the compositor draws
    /// them before the clip's effects, so the map takes them too.
    pub(crate) fn of(clip: &ExportClip, treated: bool) -> Option<SkinJob> {
        if clip.skin_dir.is_empty() {
            return None;
        }
        let store = MaskStore::open(Path::new(&clip.skin_dir));
        if store.is_empty() {
            return None;
        }
        Some(SkinJob {
            store,
            mapping: if treated {
                Mapping::IDENTITY
            } else {
                Mapping {
                    crop: clip
                        .crop
                        .map(|edges| edges.map(|edge| edge as f32))
                        .unwrap_or([0.0; 4]),
                    flip_h: clip.flip_h,
                    flip_v: clip.flip_v,
                }
            },
            made: std::sync::Mutex::new(std::collections::VecDeque::new()),
        })
    }

    /// The skin map at `source_time`, or `None` when that instant has no
    /// mask yet.
    pub(crate) fn map_at(&self, source_time: Rational) -> Option<Arc<RevealMap>> {
        let mask = self.store.mask_at(source_time.as_f64())?;
        if let Ok(made) = self.made.lock()
            && let Some((_, map)) = made.iter().find(|(from, _)| Arc::ptr_eq(from, &mask))
        {
            return Some(Arc::clone(map));
        }
        let (width, height) = (mask.width(), mask.height());
        let mut gray = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                let (u, v) = self.mapping.source_of(
                    (x as f32 + 0.5) / width as f32,
                    (y as f32 + 0.5) / height as f32,
                );
                gray.push(1 + (mask.sample(u, v).clamp(0.0, 1.0) * 254.0 + 0.5) as u8);
            }
        }
        // the identity the renderer caches the upload by: what the map holds
        let mut id: u64 = 0xcbf2_9ce4_8422_2325 ^ 0x736b_696e;
        for &byte in &gray {
            id ^= u64::from(byte);
            id = id.wrapping_mul(0x0100_0000_01b3);
        }
        let map = Arc::new(RevealMap {
            id,
            width,
            height,
            gray: gray.into(),
        });
        if let Ok(mut made) = self.made.lock() {
            made.push_back((mask, Arc::clone(&map)));
            while made.len() > 3 {
                made.pop_front();
            }
        }
        Some(map)
    }
}

/// A stabilization as the frame loop runs it: the clip's correction for
/// every frame it shows, worked out once from the media's measured
/// movement and the clip's settings when the timeline is built. Nothing
/// is cached past that: the working out is a few milliseconds even for a
/// long clip, and a changed setting is a rebuilt timeline anyway.
pub(crate) struct StabilizeJob {
    correction: Correction,
}

impl StabilizeJob {
    /// The job for a clip, or `None` when it is not stabilized or its
    /// media's movement has not been measured.
    pub(crate) fn of(clip: &ExportClip) -> Option<StabilizeJob> {
        let settings = clip.stabilize?;
        if clip.kind != ClipKind::Video || clip.motion_dir.is_empty() {
            return None;
        }
        let motion = Motion::read(Path::new(&clip.motion_dir))?;
        // The same clamp the picture's own pacing applies; a curve's mean
        // is its speed.
        let speed = audio::clamp_speed(clip.speed);
        let range = (
            clip.source_start,
            clip.source_start + clip.duration.max(0.0) * speed,
        );
        let correction = stabilize::path::correct(
            &motion,
            range,
            speed,
            &stabilize::Settings {
                smoothness: settings.smoothness,
                max_zoom: settings.max_zoom,
                tripod: settings.tripod,
                keep_rotation: settings.keep_rotation,
            },
        );
        (!correction.is_empty()).then_some(StabilizeJob { correction })
    }

    /// The correction for the frame on screen at `source_time`, as the
    /// compositor takes it.
    pub(crate) fn warp_at(&self, source_time: Rational) -> concat_render::Warp {
        let map = self.correction.at(source_time.as_f64());
        concat_render::Warp {
            a: map.a as f32,
            b: map.b as f32,
            tx: map.tx as f32,
            ty: map.ty as f32,
        }
    }

    /// The zoom the clip is drawn at, `>= 1`.
    pub(crate) fn zoom(&self) -> f64 {
        self.correction.zoom
    }
}

/// An engine timeline plus the per-clip facts the decoders need that the
/// engine's model has no field for.
pub(crate) struct BuiltTimeline {
    pub(crate) timeline: Timeline,
    /// Clips that are stills: one-frame streams, decoded looping.
    pub(crate) stills: std::collections::HashSet<ClipId>,
    /// Contain-fitted decode size per clip, where the source's size is known.
    pub(crate) decode_sizes: HashMap<ClipId, (u32, u32)>,
    /// Each picture's track, so a treatment knows what lies beneath it.
    pub(crate) tracks: HashMap<ClipId, usize>,
    /// The crop and flips the frame plan draws. See [`planned_geometry`].
    pub(crate) geometry: HashMap<ClipId, PlannedGeometry>,
    /// The fades to a colour and wipes the frame plan draws, per clip.
    pub(crate) shapes: HashMap<ClipId, Vec<crate::TransitionShape>>,
    /// The levels the clip's file is read as, where the person has said.
    pub(crate) ranges: HashMap<ClipId, concat_media::ColorRange>,
    /// The clip's applied effects: the passes are resolved from them at
    /// each frame, because a knob with keys is worth something different
    /// each frame and the resolution is cheap.
    pub(crate) chains: HashMap<ClipId, Vec<concat_effects::AppliedFilter>>,
    /// A title's per-word reveal order, for the clips that have one -
    /// carried beside `chains` rather than inside it, since it is baked
    /// once by the host and never resolved per frame the way effects are.
    pub(crate) reveal_maps: HashMap<ClipId, Arc<RevealMap>>,
    /// The clip whose cutout is drawn tinted rather than cut, if one is.
    pub(crate) highlight: Option<ClipId>,
    /// The layers: treatments over the stack, by span.
    pub(crate) treatments: Vec<Treatment>,
    /// The packaged transitions over cuts, resolved before the timeline.
    pub(crate) transitions: Vec<TransitionSpan>,
    /// The clips whose background a mask takes away.
    pub(crate) cutouts: HashMap<ClipId, CutoutJob>,
    /// The clips with Forge's reel effects.
    pub(crate) fx: HashMap<ClipId, FxJob>,
    /// The retouched clips whose skin has been found.
    pub(crate) skins: HashMap<ClipId, SkinJob>,
    /// What fills the frame behind each picture that has a background; see
    /// [`crate::with_backdrops`].
    pub(crate) backdrops: HashMap<ClipId, crate::ExportBackdrop>,
    /// The clips whose camera shake is taken out.
    pub(crate) stabilized: HashMap<ClipId, StabilizeJob>,
}

/// A transition, as the compositor draws it: over `[start, end)`, the
/// stack beneath `track` drawn twice - as it is before the cut, `from`'s
/// clips in it and `to`'s left out, and as it is after, the other way
/// round - and the two combined through the package's two-input shader.
/// With no cut under it both sets are empty and the two pictures are the
/// same. A machine with no GPU dissolves between them instead.
#[derive(Clone, Debug)]
pub(crate) struct TransitionSpan {
    pub(crate) start: Rational,
    pub(crate) end: Rational,
    pub(crate) track: usize,
    pub(crate) id: String,
    pub(crate) params: BTreeMap<String, f64>,
    /// The outgoing side's clips: the clip before the cut and its stand-in
    /// after it. Filled in as the timeline is built.
    pub(crate) from: std::collections::HashSet<ClipId>,
    /// The incoming side's: the clip after the cut and its stand-in before.
    pub(crate) to: std::collections::HashSet<ClipId>,
}

impl TransitionSpan {
    pub(crate) fn covers(&self, time: Rational) -> bool {
        self.start <= time && time < self.end
    }

    /// How far through the cut `time` is, `0..=1`.
    pub(crate) fn progress(&self, time: Rational) -> f64 {
        let span = self.end.as_f64() - self.start.as_f64();
        if span <= 0.0 {
            return 1.0;
        }
        ((time.as_f64() - self.start.as_f64()) / span).clamp(0.0, 1.0)
    }
}

/// A layer clip, as the compositor needs it: when, over which tracks, what
/// effects, and how hard.
#[derive(Clone, Debug)]
pub(crate) struct Treatment {
    pub(crate) start: Rational,
    pub(crate) end: Rational,
    pub(crate) track: usize,
    /// The layer's applied effects, resolved to passes at each frame, so a
    /// keyed knob rides.
    pub(crate) effects: Vec<concat_effects::AppliedFilter>,
    pub(crate) strength: f32,
    pub(crate) ramp_in: f64,
    pub(crate) ramp_out: f64,
}

impl Treatment {
    pub(crate) fn covers(&self, time: Rational) -> bool {
        self.start <= time && time < self.end
    }

    /// The shader passes at `time`, each keyed knob at its value there. A
    /// layer is never a title, so it never carries a reveal map.
    pub(crate) fn passes_at(&self, time: Rational) -> Vec<ShaderPass> {
        let span = (self.end - self.start).as_f64();
        let at = if span > 0.0 {
            ((time - self.start).as_f64() / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Catalogue::builtin().shader_passes_at(&self.effects, at, None)
    }

    /// How hard the treatment is applied at `time`: the strength, eased in
    /// and out over the ramps at either end.
    pub(crate) fn strength_at(&self, time: Rational) -> f32 {
        let at = time.as_f64() - self.start.as_f64();
        let left = self.end.as_f64() - time.as_f64();
        let mut ramp = 1.0_f64;
        if self.ramp_in > 0.0 && at < self.ramp_in {
            ramp = ramp.min(at / self.ramp_in);
        }
        if self.ramp_out > 0.0 && left < self.ramp_out {
            ramp = ramp.min(left / self.ramp_out);
        }
        (f64::from(self.strength) * ramp.clamp(0.0, 1.0)) as f32
    }
}

/// Converts the flattened clip list into an engine timeline.
pub(crate) fn build_timeline(
    request: &ExportRequest,
    rate: FrameRate,
    visible: &[&ExportClip],
    mut transitions: Vec<TransitionSpan>,
) -> BuiltTimeline {
    let mut timeline = Timeline::new(request.width, request.height, rate);
    timeline.canvas = request.canvas;
    let mut stills = std::collections::HashSet::new();
    let mut decode_sizes: HashMap<ClipId, (u32, u32)> = HashMap::new();
    let mut tracks_of: HashMap<ClipId, usize> = HashMap::new();
    let mut treatments: Vec<Treatment> = Vec::new();
    let mut geometry: HashMap<ClipId, PlannedGeometry> = HashMap::new();
    let mut shapes: HashMap<ClipId, Vec<crate::TransitionShape>> = HashMap::new();
    let mut ranges: HashMap<ClipId, concat_media::ColorRange> = HashMap::new();
    let mut chains: HashMap<ClipId, Vec<concat_effects::AppliedFilter>> = HashMap::new();
    let mut reveal_maps: HashMap<ClipId, Arc<RevealMap>> = HashMap::new();
    let mut cutouts: HashMap<ClipId, CutoutJob> = HashMap::new();
    let mut fx: HashMap<ClipId, FxJob> = HashMap::new();
    let mut skins: HashMap<ClipId, SkinJob> = HashMap::new();
    let mut backdrops: HashMap<ClipId, crate::ExportBackdrop> = HashMap::new();
    let mut stabilized: HashMap<ClipId, StabilizeJob> = HashMap::new();
    let mut highlight: Option<ClipId> = None;

    let lanes = visible.iter().map(|clip| clip.track).max().unwrap_or(0) + 1;
    let tracks: Vec<_> = (0..lanes)
        .map(|index| timeline.add_track(Track::new(format!("T{index}"), TrackKind::Video)))
        .collect();

    for clip in visible {
        // Quantise to the frame grid on the way in. The UI works in f64
        // seconds; the engine works in exact rationals, and this is the seam
        // where a value stops being approximate. The two ends are rounded,
        // not the start and the length: two clips that touch then touch on
        // the grid too, where a start and a length rounded apart left a
        // frame between them black (#756) or drew one frame twice over.
        let start = quantise_at(clip.start, rate, request.effect_time_offset);
        let duration = clip_end(clip, rate, request.effect_time_offset) - start;
        if duration.is_zero() {
            continue;
        }

        // A layer has no pixels to decode: it is a treatment over the
        // stack, kept beside the timeline rather than in it.
        if clip.kind == ClipKind::Layer {
            let effects = shaded(&clip.effects);
            if !effects.is_empty() {
                treatments.push(Treatment {
                    start,
                    end: start + duration,
                    track: clip.track,
                    effects,
                    strength: clip.opacity.clamp(0.0, 1.0) as f32,
                    ramp_in: clip.fade_in.max(0.0),
                    ramp_out: clip.fade_out.max(0.0),
                });
            }
            continue;
        }

        let mut engine_clip = Clip::new(MediaRef::new(&clip.path), start, duration);
        engine_clip.source_start = quantise(clip.source_start, rate);
        // A still is one picture, held: read once and the same frame drawn
        // at every instant, which the compositor then never uploads twice.
        // Paced like footage, a title was decoded, converted, copied and
        // uploaded again at every frame it was on (#754).
        engine_clip.hold = clip.hold || clip.kind == ClipKind::Image;
        // The same clamp the audio path applies, so a 2x clip means the same
        // thing to picture and sound. A still has no meaningful rate.
        if clip.kind != ClipKind::Image {
            engine_clip.speed =
                Rational::approximate(audio::clamp_speed(clip.speed)).unwrap_or(Rational::ONE);
            engine_clip.retime = clip.engine_curve();
        }
        engine_clip.animation = animation_of(&clip.animation);
        engine_clip.motion = clip.motion.as_ref().and_then(|motion| motion.engine());
        engine_clip.blend = concat_core::timeline::Blend::parse(&clip.blend);
        engine_clip.masks = clip.masks.iter().map(Mask::engine).collect();
        engine_clip.overscan = overscan_of(clip);
        engine_clip.transform = Transform {
            scale: clip.scale,
            offset_x: clip.offset_x,
            offset_y: clip.offset_y,
            rotation: clip.rotation,
            stretch_x: clip.stretch_x,
            stretch_y: clip.stretch_y,
        };
        engine_clip.opacity = clip.opacity.clamp(0.0, 1.0) as f32;
        // Quantised like every other time: the ramp must land on the same
        // frame grid the overlap does, or the dissolve ends a frame early.
        engine_clip.video_fade_in = quantise(clip.video_fade_in, rate);

        if let Some(id) = timeline.add_clip(tracks[clip.track], engine_clip) {
            // A stand-in is stacked where the clip it extends is.
            tracks_of.insert(id, clip.stack_at.unwrap_or(clip.track));
            for &(span, incoming) in &clip.roles {
                if let Some(span) = transitions.get_mut(span) {
                    if incoming {
                        span.to.insert(id);
                    } else {
                        span.from.insert(id);
                    }
                }
            }
            if clip.kind == ClipKind::Image {
                stills.insert(id);
            }
            let planned = planned_geometry(clip);
            let steadied = StabilizeJob::of(clip);
            if let Some(size) = fitted_size(request, clip, planned.is_some()) {
                // A stabilized picture is drawn larger by its zoom: decoded
                // that much larger too, up to the file's own size, so the
                // zoom costs no sharpness the file had.
                let size = match (&steadied, clip.media_width, clip.media_height) {
                    (Some(job), Some(width), Some(height)) if job.zoom() > 1.0 => {
                        let grow = job
                            .zoom()
                            .min(f64::from(width) / f64::from(size.0.max(1)))
                            .min(f64::from(height) / f64::from(size.1.max(1)))
                            .max(1.0);
                        (
                            ((f64::from(size.0) * grow).round() as u32).max(2) & !1,
                            ((f64::from(size.1) * grow).round() as u32).max(2) & !1,
                        )
                    }
                    _ => size,
                };
                decode_sizes.insert(id, size);
            }
            if let Some(job) = steadied {
                stabilized.insert(id, job);
            }
            if !clip.transition_shapes.is_empty() {
                shapes.insert(id, clip.transition_shapes.clone());
            }
            if let Some(planned) = planned {
                geometry.insert(id, planned);
            }
            if let Some(range) = clip.color_range {
                ranges.insert(id, crate::engine_range(range));
            }
            let effects = shaded(&clip.effects);
            if !effects.is_empty() {
                chains.insert(id, effects);
            }
            if let Some(map) = &clip.reveal_map {
                reveal_maps.insert(id, Arc::clone(map));
            }
            if let Some(job) = CutoutJob::of(clip, planned.is_none()) {
                cutouts.insert(id, job);
            }
            if let Some(job) = FxJob::of(clip, planned.is_none()) {
                fx.insert(id, job);
            if let Some(job) = SkinJob::of(clip, planned.is_none()) {
                skins.insert(id, job);
            }
            if clip.highlighted {
                highlight = Some(id);
            }
            if let Some(backdrop) = &clip.backdrop {
                backdrops.insert(id, backdrop.clone());
            }
        }
    }

    BuiltTimeline {
        timeline,
        stills,
        decode_sizes,
        tracks: tracks_of,
        treatments,
        transitions,
        geometry,
        shapes,
        ranges,
        chains,
        reveal_maps,
        cutouts,
        fx,
        skins,
        stabilized,
        highlight,
        backdrops,
    }
}

/// The crop and flips the frame plan draws for a clip.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct PlannedGeometry {
    pub(crate) crop: concat_render::Crop,
    pub(crate) flip_h: bool,
    pub(crate) flip_v: bool,
}

/// The crop and flips the frame plan draws for this clip, or `None` when
/// it has neither. The plan crops and flips the picture as it draws it,
/// before the clip's effects run over it: a vignette is centred on the
/// picture as cropped, a wipe on a mirrored clip still wipes the way the
/// frame is seen. A stabilized clip always has them drawn by the plan,
/// even when it has neither: its correction is of the whole picture as
/// shot, before any crop, so the decoder must hand over the whole of it.
pub(crate) fn planned_geometry(clip: &ExportClip) -> Option<PlannedGeometry> {
    let crop = clip
        .crop
        .map_or(concat_render::Crop::NONE, concat_render::Crop::of);
    let stabilized = clip.stabilize.is_some() && !clip.motion_dir.is_empty();
    if crop.is_none() && !clip.flip_h && !clip.flip_v && !stabilized {
        return None;
    }
    Some(PlannedGeometry {
        crop,
        flip_h: clip.flip_h,
        flip_v: clip.flip_v,
    })
}

/// The enabled entries of a chain whose package has a shader: the ones
/// resolved to passes each frame. A link to a package not installed here
/// draws nothing.
pub(crate) fn shaded(effects: &[AppliedFilter]) -> Vec<concat_effects::AppliedFilter> {
    let catalogue = Catalogue::builtin();
    effects
        .iter()
        .filter(|applied| {
            applied.enabled
                && catalogue
                    .get(&applied.id)
                    .is_some_and(|package| package.shader().is_some())
        })
        .map(crate::chains::engine_filter)
        .collect()
}

/// The engine's keys for a flattened clip's animation, or None for none.
pub(crate) fn animation_of(keys: &[ExportKey]) -> Option<Animation> {
    use concat_core::animate::{Ease, Key, Track};
    if keys.is_empty() {
        return None;
    }
    let mut tracks: [Vec<Key>; 8] = Default::default();
    for key in keys {
        let slot = match key.property.as_str() {
            "scale" => 0,
            "offsetX" => 1,
            "offsetY" => 2,
            "rotation" => 3,
            "stretchX" => 4,
            "stretchY" => 5,
            "opacity" => 6,
            "volume" => 7,
            _ => continue,
        };
        let [x1, y1, x2, y2] = key.ease;
        tracks[slot].push(Key {
            at: key.at,
            value: key.value,
            ease: Ease::new(x1, y1, x2, y2),
        });
    }
    let [scale, x, y, rotation, wide, tall, opacity, volume] = tracks;
    let animation = Animation {
        scale: Track::new(scale),
        offset_x: Track::new(x),
        offset_y: Track::new(y),
        rotation: Track::new(rotation),
        stretch_x: Track::new(wide),
        stretch_y: Track::new(tall),
        opacity: Track::new(opacity),
        volume: Track::new(volume),
    };
    (!animation.is_empty()).then_some(animation)
}

/// The source's contain-fitted size inside the output frame, or `None` when
/// the UI never learnt the source's dimensions.
/// `whole` asks for the uncropped picture - the frame plan crops it - at
/// the scale that fits the part the crop keeps, so the kept part lands at
/// the same size either way and a crop costs no sharpness.
pub(crate) fn fitted_size(
    request: &ExportRequest,
    clip: &ExportClip,
    whole: bool,
) -> Option<(u32, u32)> {
    let source_width = clip.media_width.filter(|value| *value > 0)?;
    let source_height = clip.media_height.filter(|value| *value > 0)?;
    let (media_width, media_height) = (source_width, source_height);
    // What is left after the crop is what gets fitted.
    let (media_width, media_height) = match clip.crop {
        Some([left, top, right, bottom]) => (
            (f64::from(media_width) * (1.0 - left - right).max(0.1))
                .round()
                .max(2.0) as u32,
            (f64::from(media_height) * (1.0 - top - bottom).max(0.1))
                .round()
                .max(2.0) as u32,
        ),
        None => (media_width, media_height),
    };

    let fit = (f64::from(request.width) / f64::from(media_width))
        .min(f64::from(request.height) / f64::from(media_height))
        * overscan_of(clip);
    let (fit_width, fit_height) = if whole {
        (source_width, source_height)
    } else {
        (media_width, media_height)
    };
    let width = ((f64::from(fit_width) * fit).round() as u32).max(2);
    let height = ((f64::from(fit_height) * fit).round() as u32).max(2);
    // Even, because a decoder asked for an odd width may round it itself and
    // then every frame read is misaligned by a pixel's worth of bytes.
    Some((width & !1, height & !1))
}

/// The clip's overscan, held to what it can mean: one, or more for a
/// picture several frames across; a number that is not one is one.
fn overscan_of(clip: &ExportClip) -> f64 {
    if clip.overscan.is_finite() {
        clip.overscan.max(1.0)
    } else {
        1.0
    }
}

pub(crate) fn quantise(seconds: f64, rate: FrameRate) -> Rational {
    rate.time_of_frame((seconds * rate.fps().as_f64()).round().max(0.0) as i64)
}

/// Where a clip ends on the frame grid: its end rounded as its start is,
/// so the clip after it, starting where it ends, starts on the same frame.
pub(crate) fn clip_end(clip: &ExportClip, rate: FrameRate, origin: f64) -> Rational {
    quantise_at(clip.start + clip.duration, rate, origin)
}

/// Rebase a frame-grid timestamp without rounding a raw subtraction. Audio
/// keeps the raw local placement; video retains its original global grid.
pub(crate) fn quantise_at(seconds: f64, rate: FrameRate, origin: f64) -> Rational {
    quantise(seconds + origin, rate) - quantise(origin, rate)
}
