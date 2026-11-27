// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Following the face for Forge's reel effects: each media file a clip
//! with reel effects shows is looked at ten times a second over the
//! stretches its clips show, and where the face is kept in its track (see
//! `concat_vision::faces`). The person the effects keep in front is Remove
//! BG's person mask, found by [`crate::Cutouts`] for the same clips.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use concat_core::time::{FrameRate, Rational};
use concat_media::{DecodeOptions, Decoder, FrameSource};
use concat_project::model::{MediaKind, Project, Subject};
use concat_vision::MASK_RATE;
use concat_vision::faces::{FaceDetector, FaceTrack, face_file};

use crate::cutout::{AnalyseRequest, Progress};
use crate::jobs::SingleFlight;

/// One media file's face to follow.
#[derive(Clone, Debug)]
pub struct FaceRequest {
    /// The project folder the track is kept under.
    pub project: PathBuf,
    /// The media file.
    pub media_path: String,
    /// Its picture size.
    pub media_size: (u32, u32),
    /// A still: one look answers for every instant.
    pub still: bool,
    /// The stretches of source, in seconds, the clips show.
    pub ranges: Vec<(f64, f64)>,
}

/// The face service: the detector, loaded once, and one job at a time.
pub struct Faces {
    gate: Arc<SingleFlight>,
    detector: OnceLock<Result<Arc<FaceDetector>, String>>,
}

impl Default for Faces {
    fn default() -> Faces {
        Faces::new()
    }
}

/// The instants of `ranges` on the analysis grid that `track` lacks.
fn missing(track: &FaceTrack, ranges: &[(f64, f64)], still: bool) -> Vec<u64> {
    if still {
        return if track.has(0) { Vec::new() } else { vec![0] };
    }
    let step = 1000 / u64::from(MASK_RATE);
    let mut out: Vec<u64> = Vec::new();
    for &(from, to) in ranges {
        let mut at = ((from.max(0.0) * 1000.0) as u64) / step * step;
        let last = (to.max(from).max(0.0) * 1000.0).ceil() as u64;
        while at <= last {
            if !track.has(at) {
                out.push(at);
            }
            at += step;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

impl Faces {
    /// A service with nothing loaded yet.
    pub fn new() -> Faces {
        Faces {
            gate: Arc::new(SingleFlight::new()),
            detector: OnceLock::new(),
        }
    }

    /// Stops the job in progress.
    pub fn cancel(&self) {
        self.gate.cancel();
    }

    fn detector(&self) -> Result<Arc<FaceDetector>, String> {
        self.detector.get_or_init(|| FaceDetector::new().map(Arc::new)).clone()
    }

    /// The face tracks and the person masks a project's reel effects need,
    /// by media: what [`Faces::analyse`] and [`crate::Cutouts::analyse`] are
    /// asked for, paired with the media's id.
    pub fn requests(project: &Project, project_dir: &Path) -> Vec<(String, FaceRequest, AnalyseRequest)> {
        let mut out: Vec<(String, FaceRequest, AnalyseRequest)> = Vec::new();
        for clip in &project.active().clips {
            if clip.reel_fx.is_empty() || !clip.kind.is_visual() {
                continue;
            }
            let Some(media) = project.media_by_id(&clip.media_id) else {
                continue;
            };
            if !matches!(media.kind, MediaKind::Video | MediaKind::Image) {
                continue;
            }
            // only the stretches the effects cover, and half a second either side
            let speed = clip.speed.max(0.0625);
            let ranges: Vec<(f64, f64)> = clip
                .reel_fx
                .iter()
                .map(|e| {
                    let a = clip.source_start + (e.start - 0.5).max(0.0) * speed;
                    let b = clip.source_start + (e.end + 0.5).min(clip.duration) * speed;
                    (a, b.max(a))
                })
                .collect();
            let size = (media.width.unwrap_or(0), media.height.unwrap_or(0));
            let still = media.kind == MediaKind::Image;
            match out.iter_mut().find(|(id, _, _)| *id == media.id) {
                Some((_, f, m)) => {
                    f.ranges.extend(&ranges);
                    m.ranges.extend(&ranges);
                }
                None => out.push((
                    media.id.clone(),
                    FaceRequest {
                        project: project_dir.to_path_buf(),
                        media_path: media.path.clone(),
                        media_size: size,
                        still,
                        ranges: ranges.clone(),
                    },
                    AnalyseRequest {
                        project: project_dir.to_path_buf(),
                        media_path: media.path.clone(),
                        media_size: size,
                        still,
                        subject: Subject::Person,
                        ranges,
                    },
                )),
            }
        }
        out
    }

    /// How many instants `request` still needs.
    pub fn outstanding(request: &FaceRequest) -> usize {
        let track = FaceTrack::open(&face_file(&request.project, &request.media_path));
        missing(&track, &request.ranges, request.still).len()
    }

    /// Finds the face at every instant `request` is missing. Blocks for the
    /// whole run; `progress` hears how far it got. Returns how many
    /// instants were looked at.
    pub fn analyse(&self, request: &FaceRequest, progress: &mut dyn FnMut(Progress)) -> Result<usize, String> {
        let job = self.gate.begin("face tracking")?;
        let file = face_file(&request.project, &request.media_path);
        let mut track = FaceTrack::open(&file);
        let wanted = missing(&track, &request.ranges, request.still);
        if wanted.is_empty() {
            return Ok(0);
        }
        let detector = self.detector()?;
        // a long side of 640, the detector's own square
        let (w, h) = request.media_size;
        let scale = if w.max(h) > 0 { 640.0 / w.max(h) as f32 } else { 1.0 };
        let (dw, dh) = (((w as f32 * scale) as u32).max(2) & !1, ((h as f32 * scale) as u32).max(2) & !1);
        let step = 1000 / u64::from(MASK_RATE);
        let total = wanted.len();
        let mut done = 0usize;
        progress(Progress::Analysing(0.0));
        let mut runs: Vec<Vec<u64>> = Vec::new();
        for at in wanted {
            match runs.last_mut() {
                Some(run) if run.last().is_some_and(|&last| last + step == at) => run.push(at),
                _ => runs.push(vec![at]),
            }
        }
        for run in runs {
            let mut options = DecodeOptions::default().starting_at(Rational::new(run[0] as i64, 1000)).limited_to(run.len() as u64);
            if dw > 2 && dh > 2 {
                options = options.scaled_to(dw, dh);
            }
            if !request.still {
                options = options.at_rate(FrameRate::new(Rational::new(i64::from(MASK_RATE), 1)));
            }
            let mut decoder = Decoder::open(&request.media_path, &options).map_err(|e| e.to_string())?;
            for &at in &run {
                if job.cancelled() {
                    let _ = track.save(&file);
                    return Err("face tracking cancelled".to_owned());
                }
                let Some(frame) = decoder.next_frame().map_err(|e| e.to_string())? else {
                    break;
                };
                track.put(at, detector.detect(&frame)?);
                done += 1;
                if done % 25 == 0 {
                    let _ = track.save(&file);
                }
                progress(Progress::Analysing(done as f32 / total as f32));
            }
        }
        track.save(&file)?;
        progress(Progress::Analysing(1.0));
        Ok(done)
    }
}
