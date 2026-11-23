// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Android MediaCodec surface encoder. Frames are submitted through an
//! ANativeWindow; no RGBA frame is read back to CPU memory. HEVC Main10 and
//! HDR color signaling are requested only for a 10-bit Vulkan surface.

use std::ffi::c_void;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr;
use std::time::{Duration, Instant};

use concat_core::frame::Signal;
use ndk_sys as sys;

/// An Android hardware video encoder whose input is a native rendering surface.
pub struct SurfaceEncoder {
    codec: *mut sys::AMediaCodec,
    muxer: *mut sys::AMediaMuxer,
    window: *mut sys::ANativeWindow,
    output: File,
    track: Option<usize>,
    muxer_started: bool,
    codec_started: bool,
    eos: bool,
    samples: usize,
}

impl SurfaceEncoder {
    /// Opens a surface-input AVC or HEVC encoder and MP4 muxer. Ten-bit
    /// output is HEVC Main10; HDR needs HLG/PQ signaling and a 10-bit surface.
    pub fn open(
        path: &Path,
        width: u32,
        height: u32,
        fps: f64,
        bitrate: u32,
        mime: &str,
        color_range: crate::ColorRange,
        ten_bit: bool,
        signal: Signal,
    ) -> Result<Self, String> {
        if width == 0
            || height == 0
            || width % 2 != 0
            || height % 2 != 0
            || !fps.is_finite()
            || fps <= 0.0
        {
            return Err(
                "surface encoder requires positive even dimensions and a valid frame rate".into(),
            );
        }
        let mime = match mime {
            "video/avc" => c"video/avc",
            "video/hevc" => c"video/hevc",
            _ => return Err(format!("unsupported surface encoder MIME {mime}")),
        };
        if ten_bit && mime != c"video/hevc" {
            return Err("10-bit surface export requires an HEVC encoder".into());
        }
        if matches!(signal, Signal::Hlg | Signal::Pq) && !ten_bit {
            return Err("HDR surface export requires a 10-bit input surface".into());
        }
        let codec = unsafe { sys::AMediaCodec_createEncoderByType(mime.as_ptr()) };
        if codec.is_null() {
            return Err(format!("no Android encoder for {}", mime.to_string_lossy()));
        }
        let format = unsafe { sys::AMediaFormat_new() };
        if format.is_null() {
            unsafe { sys::AMediaCodec_delete(codec) };
            return Err("could not allocate MediaFormat".into());
        }
        unsafe {
            sys::AMediaFormat_setString(format, c"mime".as_ptr(), mime.as_ptr());
            sys::AMediaFormat_setInt32(format, c"width".as_ptr(), width as i32);
            sys::AMediaFormat_setInt32(format, c"height".as_ptr(), height as i32);
            sys::AMediaFormat_setInt32(format, c"bitrate".as_ptr(), bitrate as i32);
            sys::AMediaFormat_setInt32(
                format,
                c"frame-rate".as_ptr(),
                fps.round().clamp(1.0, 240.0) as i32,
            );
            sys::AMediaFormat_setInt32(format, c"i-frame-interval".as_ptr(), 1);
            sys::AMediaFormat_setInt32(format, c"color-format".as_ptr(), 0x7f000789); // COLOR_FormatSurface
            sys::AMediaFormat_setInt32(format, c"max-bframes".as_ptr(), 0);
            sys::AMediaFormat_setInt32(
                format,
                c"bitrate-mode".as_ptr(),
                1, // VBR is more reliable than CBR on Android hardware encoders.
            );
            sys::AMediaFormat_setInt32(
                format,
                c"color-range".as_ptr(),
                if color_range == crate::ColorRange::Full {
                    1
                } else {
                    2
                },
            );
            if ten_bit {
                // HEVCProfileMain10. Devices that cannot accept a 10-bit
                // surface reject configure and the caller takes the regular
                // encoder path.
                sys::AMediaFormat_setInt32(format, c"profile".as_ptr(), 2);
            }
            let hdr = matches!(signal, Signal::Hlg | Signal::Pq);
            sys::AMediaFormat_setInt32(format, c"color-standard".as_ptr(), if hdr { 6 } else { 1 });
            sys::AMediaFormat_setInt32(
                format,
                c"color-transfer".as_ptr(),
                match signal {
                    Signal::Pq => 6,  // ST 2084
                    Signal::Hlg => 7, // HLG
                    _ => 3,           // SDR video
                },
            );
        }
        let status = unsafe {
            sys::AMediaCodec_configure(
                codec,
                format,
                ptr::null_mut(),
                ptr::null_mut(),
                sys::AMEDIACODEC_CONFIGURE_FLAG_ENCODE as u32,
            )
        };
        unsafe { sys::AMediaFormat_delete(format) };
        if status != sys::media_status_t::AMEDIA_OK {
            unsafe { sys::AMediaCodec_delete(codec) };
            return Err(format!("MediaCodec configure failed: {status:?}"));
        }
        let mut window = ptr::null_mut();
        let status = unsafe { sys::AMediaCodec_createInputSurface(codec, &mut window) };
        if status != sys::media_status_t::AMEDIA_OK || window.is_null() {
            unsafe { sys::AMediaCodec_delete(codec) };
            return Err(format!("MediaCodec input surface failed: {status:?}"));
        }
        if ten_bit {
            // Dataspace tells the codec which RGB surface transfer to encode.
            // Keep the API 28 symbol optional at load time for older phones;
            // those devices cleanly fall back to the non-surface encoder.
            let library = unsafe {
                libc::dlopen(
                    c"libnativewindow.so".as_ptr(),
                    libc::RTLD_NOW | libc::RTLD_LOCAL,
                )
            };
            if library.is_null() {
                unsafe {
                    sys::ANativeWindow_release(window);
                    sys::AMediaCodec_delete(codec);
                }
                return Err("Android native-window dataspace API is unavailable".into());
            }
            let symbol =
                unsafe { libc::dlsym(library, c"ANativeWindow_setBuffersDataSpace".as_ptr()) };
            if symbol.is_null() {
                unsafe {
                    libc::dlclose(library);
                    sys::ANativeWindow_release(window);
                    sys::AMediaCodec_delete(codec);
                }
                return Err("Android native-window dataspace API is unavailable".into());
            }
            let set_dataspace = unsafe {
                std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(*mut sys::ANativeWindow, i32) -> i32,
                >(symbol)
            };
            // Standard, transfer and limited/full range from android/data_space.h.
            let dataspace = match signal {
                Signal::Pq => 6 << 16 | 7 << 22,
                Signal::Hlg => 6 << 16 | 8 << 22,
                _ => 1 << 16 | 3 << 22,
            } | if color_range == crate::ColorRange::Full {
                1 << 27
            } else {
                2 << 27
            };
            let result = unsafe { set_dataspace(window, dataspace) };
            unsafe { libc::dlclose(library) };
            if result != 0 {
                unsafe {
                    sys::ANativeWindow_release(window);
                    sys::AMediaCodec_delete(codec);
                }
                return Err(format!(
                    "setting MediaCodec input dataspace failed: {result}"
                ));
            }
        }
        let status = unsafe { sys::AMediaCodec_start(codec) };
        if status != sys::media_status_t::AMEDIA_OK {
            unsafe {
                sys::ANativeWindow_release(window);
                sys::AMediaCodec_delete(codec);
            }
            return Err(format!("MediaCodec start failed: {status:?}"));
        }
        let output = match File::create(path) {
            Ok(file) => file,
            Err(error) => {
                unsafe {
                    sys::AMediaCodec_stop(codec);
                    sys::ANativeWindow_release(window);
                    sys::AMediaCodec_delete(codec);
                }
                return Err(format!("could not create {}: {error}", path.display()));
            }
        };
        let muxer = unsafe {
            sys::AMediaMuxer_new(
                output.as_raw_fd(),
                sys::OutputFormat::AMEDIAMUXER_OUTPUT_FORMAT_MPEG_4,
            )
        };
        if muxer.is_null() {
            unsafe {
                sys::AMediaCodec_stop(codec);
                sys::ANativeWindow_release(window);
                sys::AMediaCodec_delete(codec);
            }
            return Err("could not create MP4 muxer".into());
        }
        Ok(Self {
            codec,
            muxer,
            window,
            output,
            track: None,
            muxer_started: false,
            codec_started: true,
            eos: false,
            samples: 0,
        })
    }

    /// Returns the ANativeWindow used to create a compatible Vulkan surface.
    pub fn native_window(&self) -> *mut c_void {
        self.window.cast()
    }

    /// Waits until the hardware encoder has produced all frames presented so
    /// far; this backpressure prevents the Vulkan surface queue dropping them.
    pub fn wait_for_samples(&mut self, expected: usize) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.samples < expected && Instant::now() < deadline {
            self.drain()?;
            if self.samples < expected {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if self.samples < expected {
            return Err(format!(
                "MediaCodec produced {} of {expected} expected frames",
                self.samples
            ));
        }
        Ok(())
    }

    /// Signals end of input and closes the MP4 after all encoded frames drain.
    pub fn finish(mut self) -> Result<(), String> {
        let status = unsafe { sys::AMediaCodec_signalEndOfInputStream(self.codec) };
        if status != sys::media_status_t::AMEDIA_OK {
            return Err(format!("MediaCodec EOS failed: {status:?}"));
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while !self.eos && Instant::now() < deadline {
            self.drain()?;
            if !self.eos {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        if !self.eos {
            return Err("MediaCodec did not finish within 30 seconds".into());
        }
        if !self.muxer_started {
            return Err("MediaCodec never supplied its output format".into());
        }
        let status = unsafe { sys::AMediaMuxer_stop(self.muxer) };
        if status != sys::media_status_t::AMEDIA_OK {
            return Err(format!("MediaMuxer stop failed: {status:?}"));
        }
        self.muxer_started = false;
        unsafe {
            sys::AMediaCodec_stop(self.codec);
        }
        self.codec_started = false;
        self.output
            .sync_all()
            .map_err(|error| format!("flush encoded MP4: {error}"))?;
        Ok(())
    }

    fn drain(&mut self) -> Result<(), String> {
        loop {
            let mut info = sys::AMediaCodecBufferInfo {
                offset: 0,
                size: 0,
                presentationTimeUs: 0,
                flags: 0,
            };
            let index = unsafe { sys::AMediaCodec_dequeueOutputBuffer(self.codec, &mut info, 0) };
            if index == sys::AMEDIACODEC_INFO_TRY_AGAIN_LATER as isize {
                return Ok(());
            }
            if index == sys::AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED as isize {
                let format = unsafe { sys::AMediaCodec_getOutputFormat(self.codec) };
                if format.is_null() {
                    return Err("MediaCodec output format was null".into());
                }
                let track = unsafe { sys::AMediaMuxer_addTrack(self.muxer, format) };
                unsafe { sys::AMediaFormat_delete(format) };
                if track < 0 {
                    return Err(format!("MediaMuxer addTrack failed: {track}"));
                }
                let status = unsafe { sys::AMediaMuxer_start(self.muxer) };
                if status != sys::media_status_t::AMEDIA_OK {
                    return Err(format!("MediaMuxer start failed: {status:?}"));
                }
                self.track = Some(track as usize);
                self.muxer_started = true;
                continue;
            }
            if index < 0 {
                continue;
            }
            let mut capacity = 0usize;
            let data = unsafe {
                sys::AMediaCodec_getOutputBuffer(self.codec, index as usize, &mut capacity)
            };
            if data.is_null() {
                return Err(format!("MediaCodec output buffer {index} was null"));
            }
            if info.size > 0 && info.flags & sys::AMEDIACODEC_BUFFER_FLAG_CODEC_CONFIG == 0 {
                let track = self
                    .track
                    .ok_or("encoded sample arrived before output format")?;
                let status =
                    unsafe { sys::AMediaMuxer_writeSampleData(self.muxer, track, data, &info) };
                if status != sys::media_status_t::AMEDIA_OK {
                    return Err(format!("MediaMuxer write failed: {status:?}"));
                }
                self.samples += 1;
            }
            let eos = info.flags & sys::AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM != 0;
            let status =
                unsafe { sys::AMediaCodec_releaseOutputBuffer(self.codec, index as usize, false) };
            if status != sys::media_status_t::AMEDIA_OK {
                return Err(format!("MediaCodec buffer release failed: {status:?}"));
            }
            if eos {
                self.eos = true;
                return Ok(());
            }
        }
    }
}

impl Drop for SurfaceEncoder {
    fn drop(&mut self) {
        unsafe {
            if self.muxer_started {
                sys::AMediaMuxer_stop(self.muxer);
            }
            if !self.muxer.is_null() {
                sys::AMediaMuxer_delete(self.muxer);
            }
            if self.codec_started {
                sys::AMediaCodec_stop(self.codec);
            }
            if !self.codec.is_null() {
                sys::AMediaCodec_delete(self.codec);
            }
            if !self.window.is_null() {
                sys::ANativeWindow_release(self.window);
            }
        }
    }
}
