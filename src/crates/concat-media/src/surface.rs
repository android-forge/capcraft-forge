// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! A phone's decoder read with no copy through the CPU: the platform's own
//! MediaCodec, rendering into an `AImageReader`'s hardware buffers, each
//! handed to the compositor's importer (`concat_core::gpu_import`), which
//! makes it a texture on its GPU. The frames this makes are
//! [`Frame::on_gpu`]: they hold no pixels, and the pool hands them only to a
//! playing preview, which draws them where they are.
//!
//! The demuxing is the platform's too (`AMediaExtractor`): a buffer it decodes
//! is one FFmpeg never sees. A file this cannot read - no importer, no seat on
//! the chip, a codec or a depth it does not take - gets `None` from
//! [`SurfaceDecoder::open`], and the pool reads it as it always did.

use std::ffi::c_void;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::{Duration, Instant};

use concat_core::frame::Frame;
use concat_core::time::Rational;
use ndk::hardware_buffer::HardwareBufferUsage;
use ndk::media::image_reader::{AcquireResult, ImageFormat, ImageReader};
use ndk::media::media_codec::{
    DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection,
};
use ndk::media::media_format::MediaFormat;
use ndk::native_window::NativeWindow;
use ndk_sys as ffi;

use crate::error::{Error, Result};
use crate::hardware::Seat;

/// MediaCodec's end-of-stream flag.
const END_OF_STREAM: u32 = 4;
/// `AMEDIAEXTRACTOR_SEEK_PREVIOUS_SYNC`.
const SEEK_PREVIOUS_SYNC: u32 = 0;
/// The decoder's buffers a reader holds at once: the one being imported and
/// a few the decoder runs ahead into.
const IMAGES: i32 = 4;
/// A picture a working decoder hands back well within this.
const PATIENCE: Duration = Duration::from_secs(2);

/// One file decoded on the chip into GPU memory; see the module.
pub(crate) struct SurfaceDecoder {
    path: PathBuf,
    extractor: *mut ffi::AMediaExtractor,
    _file: File,
    codec: MediaCodec,
    reader: ImageReader,
    _window: NativeWindow,
    /// The picture's size out, upright.
    size: (u32, u32),
    /// Degrees clockwise the decoded picture is turned to stand upright.
    rotation: u32,
    /// The first sample's time, microseconds: the file's zero.
    origin: i64,
    /// Pictures before this, microseconds from zero, are decoded and not shown:
    /// a seek lands on the keyframe before its target.
    skip_until: i64,
    input_done: bool,
    ended: bool,
    position: Option<Rational>,
    _seat: Seat,
}

// SAFETY: the extractor is used only through `&mut self`, by one thread at a
// time, as the pool's reader lock guarantees.
unsafe impl Send for SurfaceDecoder {}

fn failed(path: &Path, detail: impl std::fmt::Display) -> Error {
    Error::Ffi {
        operation: "zero-copy decode",
        path: path.to_path_buf(),
        detail: detail.to_string(),
    }
}

impl SurfaceDecoder {
    /// `path` decoded into pictures `size` across and down, from the frame at
    /// `start` on; `None` where this road does not take the file (see the
    /// module), and an error only where it took it and then failed.
    pub(crate) fn open(path: &Path, size: (u32, u32), start: Rational) -> Result<Option<Self>> {
        if concat_core::gpu_import::importer().is_none() {
            return Ok(None);
        }
        let Ok(file) = File::open(path) else {
            return Ok(None);
        };
        let length = file.metadata().map(|m| m.len()).unwrap_or(0);
        // SAFETY: a new extractor, reading a descriptor this decoder keeps.
        let extractor = unsafe { ffi::AMediaExtractor_new() };
        if extractor.is_null() {
            return Ok(None);
        }
        let status = unsafe {
            ffi::AMediaExtractor_setDataSourceFd(extractor, file.as_raw_fd(), 0, length as i64)
        };
        let release = || unsafe {
            ffi::AMediaExtractor_delete(extractor);
        };
        if status.0 != 0 {
            release();
            return Ok(None);
        }
        let tracks = unsafe { ffi::AMediaExtractor_getTrackCount(extractor) };
        let mut chosen = None;
        for track in 0..tracks {
            let raw = unsafe { ffi::AMediaExtractor_getTrackFormat(extractor, track) };
            let Some(raw) = NonNull::new(raw) else { continue };
            // SAFETY: a format the extractor handed over.
            let mut format = unsafe { MediaFormat::from_ptr(raw) };
            let mime = format.str("mime").unwrap_or_default().to_owned();
            if mime.starts_with("video/") {
                chosen = Some((track, format, mime));
                break;
            }
        }
        let Some((track, format, mime)) = chosen else {
            release();
            return Ok(None);
        };
        // Eight-bit H.264 and HEVC, SDR: what the conversion is checked for.
        // Ten bits and HDR stay with the road that keeps their light.
        let hdr = matches!(format.i32("color-transfer"), Some(6 | 7));
        let ten_bit = mime == "video/hevc" && matches!(format.i32("profile"), Some(2 | 4096 | 8192));
        if !(mime == "video/avc" || mime == "video/hevc") || hdr || ten_bit {
            release();
            log::info!(
                "{}: not on the zero-copy road: {mime}{}{}",
                path.display(),
                if hdr { ", HDR" } else { "" },
                if ten_bit { ", ten bits" } else { "" }
            );
            return Ok(None);
        }
        let (Some(coded_width), Some(coded_height)) = (format.i32("width"), format.i32("height"))
        else {
            release();
            return Ok(None);
        };
        let rotation = format.i32("rotation-degrees").unwrap_or(0).rem_euclid(360) as u32;
        let Some(seat) = crate::hardware::mediacodec_seat() else {
            release();
            log::info!("{}: not on the zero-copy road: no seat on the chip", path.display());
            return Ok(None);
        };
        unsafe { ffi::AMediaExtractor_selectTrack(extractor, track) };
        let origin = unsafe { ffi::AMediaExtractor_getSampleTime(extractor) }.max(0);
        let opened = (|| -> std::result::Result<_, String> {
            let reader = ImageReader::new_with_usage(
                coded_width,
                coded_height,
                ImageFormat::PRIVATE,
                HardwareBufferUsage::GPU_SAMPLED_IMAGE,
                IMAGES,
            )
            .map_err(|e| format!("image reader: {e:?}"))?;
            let window = reader.window().map_err(|e| format!("reader window: {e:?}"))?;
            let codec = MediaCodec::from_decoder_type(&mime).ok_or("no decoder")?;
            codec
                .configure(&format, Some(&window), MediaCodecDirection::Decoder)
                .map_err(|e| format!("configure: {e:?}"))?;
            codec.start().map_err(|e| format!("start: {e:?}"))?;
            Ok((reader, window, codec))
        })();
        let (reader, window, codec) = match opened {
            Ok(opened) => opened,
            Err(detail) => {
                release();
                log::info!("{}: not on the zero-copy road: {detail}", path.display());
                // Told apart, so the pool can free a session and try again.
                return Err(Error::Ffi {
                    operation: "zero-copy start",
                    path: path.to_path_buf(),
                    detail,
                });
            }
        };
        let mut decoder = Self {
            path: path.to_path_buf(),
            extractor,
            _file: file,
            codec,
            reader,
            _window: window,
            size,
            rotation,
            origin,
            skip_until: i64::MIN,
            input_done: false,
            ended: false,
            position: None,
            _seat: seat,
        };
        let at = micros(start);
        if at > 0 {
            decoder.seek_micros(at)?;
        }
        Ok(Some(decoder))
    }

    /// Where the last picture handed out sits, from the file's zero.
    pub(crate) fn position(&self) -> Option<Rational> {
        self.position
    }

    /// The picture size out.
    pub(crate) fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The next picture, or `None` at the end of the file.
    pub(crate) fn next_frame(&mut self) -> Result<Option<Frame>> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if self.ended {
                return Ok(None);
            }
            self.feed()?;
            let dequeued = self
                .codec
                .dequeue_output_buffer(Duration::from_millis(5))
                .map_err(|e| failed(&self.path, format!("dequeue: {e:?}")))?;
            if let DequeuedOutputBufferInfoResult::Buffer(output) = dequeued {
                let info = *output.info();
                let end = info.flags() & END_OF_STREAM != 0;
                let at = info.presentation_time_us() - self.origin;
                let wanted = !(end && info.size() <= 0) && at >= self.skip_until;
                self.codec
                    .release_output_buffer(output, wanted)
                    .map_err(|e| failed(&self.path, format!("release: {e:?}")))?;
                if end {
                    self.ended = true;
                }
                if wanted {
                    let frame = self.picture()?;
                    self.position = Some(Rational::new(at, 1_000_000));
                    return Ok(Some(frame));
                }
                continue;
            }
            if Instant::now() > deadline {
                return Err(failed(&self.path, "the decoder stopped handing pictures back"));
            }
        }
    }

    /// Moves to the keyframe at or before `to`; the next picture handed out is
    /// the first at or after `to`.
    pub(crate) fn seek(&mut self, to: Rational) -> Result<()> {
        self.seek_micros(micros(to))
    }

    fn seek_micros(&mut self, at: i64) -> Result<()> {
        // SAFETY: the extractor this decoder owns.
        unsafe {
            ffi::AMediaExtractor_seekTo(
                self.extractor,
                at.max(0) + self.origin,
                ffi::SeekMode(SEEK_PREVIOUS_SYNC),
            )
        };
        self.codec
            .flush()
            .map_err(|e| failed(&self.path, format!("flush: {e:?}")))?;
        // Pictures rendered before the flush may wait in the reader still.
        while let Ok(AcquireResult::Image(image)) = self.reader.acquire_next_image() {
            drop(image);
        }
        self.skip_until = at;
        self.input_done = false;
        self.ended = false;
        self.position = None;
        Ok(())
    }

    /// Hands the decoder compressed pictures while it has room.
    fn feed(&mut self) -> Result<()> {
        while !self.input_done {
            let Ok(DequeuedInputBufferResult::Buffer(mut input)) =
                self.codec.dequeue_input_buffer(Duration::ZERO)
            else {
                return Ok(());
            };
            let room = input.buffer_mut();
            // SAFETY: the decoder's input buffer, `room.len()` bytes long.
            let read = unsafe {
                ffi::AMediaExtractor_readSampleData(self.extractor, room.as_mut_ptr().cast(), room.len())
            };
            if read < 0 {
                self.input_done = true;
                self.codec
                    .queue_input_buffer(input, 0, 0, 0, END_OF_STREAM)
                    .map_err(|e| failed(&self.path, format!("end of input: {e:?}")))?;
            } else {
                let time = unsafe { ffi::AMediaExtractor_getSampleTime(self.extractor) }.max(0);
                self.codec
                    .queue_input_buffer(input, 0, read as usize, time as u64, 0)
                    .map_err(|e| failed(&self.path, format!("queue: {e:?}")))?;
                unsafe { ffi::AMediaExtractor_advance(self.extractor) };
            }
        }
        Ok(())
    }

    /// The picture just rendered, from the reader, made the compositor's.
    fn picture(&mut self) -> Result<Frame> {
        let waited = Instant::now();
        let image = loop {
            match self.reader.acquire_next_image() {
                Ok(AcquireResult::Image(image)) => break image,
                Ok(_) if waited.elapsed() < Duration::from_millis(500) => {
                    std::thread::sleep(Duration::from_micros(300));
                }
                Ok(_) => return Err(failed(&self.path, "the reader got no picture")),
                Err(e) => return Err(failed(&self.path, format!("acquire: {e:?}"))),
            }
        };
        let crop = image
            .crop_rect()
            .map_err(|e| failed(&self.path, format!("crop: {e:?}")))?;
        let buffer = image
            .hardware_buffer()
            .map_err(|e| failed(&self.path, format!("buffer: {e:?}")))?;
        let importer = concat_core::gpu_import::importer()
            .ok_or_else(|| failed(&self.path, "no importer"))?;
        let rect = [
            crop.left.max(0) as u32,
            crop.top.max(0) as u32,
            (crop.right - crop.left).max(1) as u32,
            (crop.bottom - crop.top).max(1) as u32,
        ];
        let picture = importer
            .import(buffer.as_ptr().cast::<c_void>(), rect, self.rotation, self.size)
            .ok_or_else(|| failed(&self.path, "the compositor could not import the picture"))?;
        drop(image);
        Ok(Frame::on_gpu(self.size.0, self.size.1, Arc::clone(&picture)))
    }
}

impl Drop for SurfaceDecoder {
    fn drop(&mut self) {
        let _ = self.codec.stop();
        // SAFETY: made in `open`, freed once.
        unsafe { ffi::AMediaExtractor_delete(self.extractor) };
    }
}

fn micros(time: Rational) -> i64 {
    (time.as_f64() * 1_000_000.0).round() as i64
}
