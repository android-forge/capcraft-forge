// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! A prototype for rendering directly into Android MediaCodec's input surface.
//!
//! Run on a phone as:
//! `surface-encode-proto <output.mp4> [frames] [fps] [width] [height]`.
//! This deliberately encodes a changing color field rather than a timeline:
//! it proves wgpu/Vulkan -> MediaCodec surface timestamps -> MP4 muxing before
//! the production exporter is connected to the same surface.

#[cfg(target_os = "android")]
fn main() {
    if let Err(error) = run() {
        eprintln!("surface encoder prototype: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "android"))]
fn main() {
    eprintln!("surface-encode-proto runs on Android");
    std::process::exit(2);
}

#[cfg(target_os = "android")]
fn run() -> Result<(), String> {
    use std::ffi::c_void;
    use std::fs::OpenOptions;
    use std::os::fd::AsRawFd;
    use std::ptr::{self, NonNull};
    use std::time::Instant;

    use ndk_sys as sys;

    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or_else(|| {
        "usage: surface-encode-proto <output.mp4> [frames] [fps] [width] [height]".to_owned()
    })?;
    let frames: u32 = args.get(2).map_or(Ok(90), |s| s.parse()).map_err(|e| format!("frames: {e}"))?;
    let fps: u32 = args.get(3).map_or(Ok(30), |s| s.parse()).map_err(|e| format!("fps: {e}"))?;
    let width: u32 = args.get(4).map_or(Ok(1280), |s| s.parse()).map_err(|e| format!("width: {e}"))?;
    let height: u32 = args.get(5).map_or(Ok(720), |s| s.parse()).map_err(|e| format!("height: {e}"))?;
    if frames == 0 || fps == 0 || width < 2 || height < 2 || width % 2 != 0 || height % 2 != 0 {
        return Err("frames and fps must be positive; width and height must be positive even numbers".into());
    }

    // Standalone shell launches do not get the app's Binder thread pool.
    unsafe {
        let binder = libc::dlopen(c"libbinder_ndk.so".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if !binder.is_null() {
            let start = libc::dlsym(binder, c"ABinderProcess_startThreadPool".as_ptr());
            if !start.is_null() {
                let start: unsafe extern "C" fn() = std::mem::transmute(start);
                start();
            }
        }
    }

    let codec = unsafe { sys::AMediaCodec_createEncoderByType(c"video/avc".as_ptr()) };
    if codec.is_null() {
        return Err("Android has no AVC encoder".into());
    }
    let format = unsafe { sys::AMediaFormat_new() };
    if format.is_null() {
        unsafe { sys::AMediaCodec_delete(codec) };
        return Err("could not allocate MediaFormat".into());
    }
    unsafe {
        sys::AMediaFormat_setString(format, c"mime".as_ptr(), c"video/avc".as_ptr());
        sys::AMediaFormat_setInt32(format, c"width".as_ptr(), width as i32);
        sys::AMediaFormat_setInt32(format, c"height".as_ptr(), height as i32);
        sys::AMediaFormat_setInt32(format, c"bitrate".as_ptr(), (width as i64 * height as i64 * fps as i64 / 5).min(i32::MAX as i64) as i32);
        sys::AMediaFormat_setInt32(format, c"frame-rate".as_ptr(), fps as i32);
        sys::AMediaFormat_setInt32(format, c"i-frame-interval".as_ptr(), 1);
        sys::AMediaFormat_setInt32(format, c"color-format".as_ptr(), 0x7f000789); // COLOR_FormatSurface
    }
    let configured = unsafe {
        sys::AMediaCodec_configure(
            codec,
            format,
            ptr::null_mut(),
            ptr::null_mut(),
            sys::AMEDIACODEC_CONFIGURE_FLAG_ENCODE as u32,
        )
    };
    unsafe { sys::AMediaFormat_delete(format) };
    if configured != sys::media_status_t::AMEDIA_OK {
        unsafe { sys::AMediaCodec_delete(codec) };
        return Err(format!("MediaCodec configure returned {configured:?}"));
    }

    let mut window: *mut sys::ANativeWindow = ptr::null_mut();
    let status = unsafe { sys::AMediaCodec_createInputSurface(codec, &mut window) };
    if status != sys::media_status_t::AMEDIA_OK || window.is_null() {
        unsafe { sys::AMediaCodec_delete(codec) };
        return Err(format!("MediaCodec input surface returned {status:?}"));
    }

    // This platform function tags the next native-window buffer with the
    // requested media timestamp. If the OS does not export it, fail closed:
    // auto timestamps would make a fast render produce the wrong video duration.
    let native_window = unsafe { libc::dlopen(c"libnativewindow.so".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    let timestamp_symbol = if native_window.is_null() {
        ptr::null_mut()
    } else {
        unsafe { libc::dlsym(native_window, c"ANativeWindow_setBuffersTimestamp".as_ptr()) }
    };
    if timestamp_symbol.is_null() {
        unsafe {
            sys::ANativeWindow_release(window);
            sys::AMediaCodec_delete(codec);
            if !native_window.is_null() { libc::dlclose(native_window); }
        }
        return Err("Android does not expose ANativeWindow_setBuffersTimestamp; refusing timestamps based on render speed".into());
    }
    let set_timestamp: unsafe extern "C" fn(*mut sys::ANativeWindow, i64) -> i32 =
        unsafe { std::mem::transmute(timestamp_symbol) };

    let started = unsafe { sys::AMediaCodec_start(codec) };
    if started != sys::media_status_t::AMEDIA_OK {
        unsafe {
            sys::ANativeWindow_release(window);
            sys::AMediaCodec_delete(codec);
            libc::dlclose(native_window);
        }
        return Err(format!("MediaCodec start returned {started:?}"));
    }

    let output = OpenOptions::new().create(true).truncate(true).write(true).open(path)
        .map_err(|e| format!("open {path}: {e}"))?;
    let muxer = unsafe { sys::AMediaMuxer_new(output.as_raw_fd(), sys::OutputFormat::AMEDIAMUXER_OUTPUT_FORMAT_MPEG_4) };
    if muxer.is_null() {
        return Err("could not create MP4 muxer".into());
    }

    let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(instance_desc);
    let window_handle = wgpu::rwh::AndroidNdkWindowHandle::new(
        NonNull::new(window.cast::<c_void>()).ok_or("null ANativeWindow")?,
    );
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(wgpu::rwh::AndroidDisplayHandle::new().into()),
        raw_window_handle: window_handle.into(),
    };
    // SAFETY: MediaCodec owns the ANativeWindow. It remains alive until the
    // wgpu surface is dropped below, before ANativeWindow_release is called.
    let surface = unsafe { instance.create_surface_unsafe(target) }
        .map_err(|e| format!("create wgpu surface: {e}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        ..Default::default()
    })).map_err(|e| format!("no Vulkan adapter for encoder surface: {e}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .map_err(|e| format!("request Vulkan device: {e}"))?;
    let caps = surface.get_capabilities(&adapter);
    let surface_format = [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Bgra8Unorm]
        .into_iter().find(|f| caps.formats.contains(f))
        .ok_or_else(|| format!("encoder surface has no 8-bit RGBA format: {:?}", caps.formats))?;
    let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        *caps.present_modes.first().ok_or("encoder surface has no present mode")?
    };
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width,
        height,
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode: *caps.alpha_modes.first().ok_or("encoder surface has no alpha mode")?,
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    let file_started = Instant::now();
    let mut track: Option<usize> = None;
    let mut muxer_started = false;
    let mut samples = 0usize;
    let mut eos = false;
    for frame in 0..frames {
        let pts_ns = (i64::from(frame) * 1_000_000_000i64) / i64::from(fps);
        let timestamp_status = unsafe { set_timestamp(window, pts_ns) };
        if timestamp_status != 0 {
            return Err(format!("set frame {frame} timestamp returned {timestamp_status}"));
        }
        let surface_frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                surface.configure(&device, &config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout => return Err("encoder surface acquisition timed out".into()),
            wgpu::CurrentSurfaceTexture::Occluded => return Err("encoder surface is occluded".into()),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                surface.configure(&device, &config);
                return Err("encoder surface became outdated; retry the export".into());
            }
            wgpu::CurrentSurfaceTexture::Validation => return Err("wgpu validation rejected the encoder surface".into()),
        };
        let view = surface_frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let t = frame as f64 / frames as f64;
        let mut commands = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("surface encode prototype frame"),
        });
        {
            let _pass = commands.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear encoder surface"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: t, g: 0.2, b: 1.0 - t, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        queue.submit([commands.finish()]);
        queue.present(surface_frame);

        // Do not let the Vulkan swapchain discard frames when MediaCodec is
        // slower than the renderer. Each input timestamp must become a sample.
        let expected_samples = frame as usize + 1;
        let frame_deadline = Instant::now() + std::time::Duration::from_secs(10);
        while samples < expected_samples && Instant::now() < frame_deadline {
            drain(codec, muxer, &mut track, &mut muxer_started, &mut samples, &mut eos)?;
            if samples < expected_samples {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        if samples < expected_samples {
            return Err(format!("MediaCodec produced {samples} frames after submitting frame {frame}"));
        }
        if frame % 30 == 0 {
            eprintln!("submitted {}/{} frames", frame + 1, frames);
        }
    }

    let eos_status = unsafe { sys::AMediaCodec_signalEndOfInputStream(codec) };
    if eos_status != sys::media_status_t::AMEDIA_OK {
        return Err(format!("signal encoder EOS returned {eos_status:?}"));
    }
    let drain_deadline = Instant::now() + std::time::Duration::from_secs(30);
    while !eos && Instant::now() < drain_deadline {
        drain(codec, muxer, &mut track, &mut muxer_started, &mut samples, &mut eos)?;
        if !eos { std::thread::sleep(std::time::Duration::from_millis(2)); }
    }
    if !eos { return Err("encoder did not finish within 30 seconds".into()); }
    if !muxer_started { return Err("MediaCodec never supplied its output format".into()); }

    drop(surface);
    let stopped = unsafe { sys::AMediaCodec_stop(codec) };
    if stopped != sys::media_status_t::AMEDIA_OK {
        eprintln!("MediaCodec stop returned {stopped:?}");
    }
    let muxer_stopped = unsafe { sys::AMediaMuxer_stop(muxer) };
    if muxer_stopped != sys::media_status_t::AMEDIA_OK {
        eprintln!("MediaMuxer stop returned {muxer_stopped:?}");
    }
    unsafe {
        sys::AMediaMuxer_delete(muxer);
        sys::AMediaCodec_delete(codec);
        sys::ANativeWindow_release(window);
        libc::dlclose(native_window);
    }
    eprintln!("wrote {samples} encoded samples to {path} in {:.2}s ({fps} fps, {width}x{height})",
        file_started.elapsed().as_secs_f64());
    Ok(())
}

#[cfg(target_os = "android")]
fn drain(
    codec: *mut ndk_sys::AMediaCodec,
    muxer: *mut ndk_sys::AMediaMuxer,
    track: &mut Option<usize>,
    muxer_started: &mut bool,
    samples: &mut usize,
    eos: &mut bool,
) -> Result<(), String> {
    use ndk_sys as sys;
    loop {
        let mut info = sys::AMediaCodecBufferInfo { offset: 0, size: 0, presentationTimeUs: 0, flags: 0 };
        let index = unsafe { sys::AMediaCodec_dequeueOutputBuffer(codec, &mut info, 0) };
        if index == sys::AMEDIACODEC_INFO_TRY_AGAIN_LATER as isize { return Ok(()); }
        if index == sys::AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED as isize {
            let format = unsafe { sys::AMediaCodec_getOutputFormat(codec) };
            if format.is_null() { return Err("encoder output format was null".into()); }
            let added = unsafe { sys::AMediaMuxer_addTrack(muxer, format) };
            unsafe { sys::AMediaFormat_delete(format) };
            if added < 0 { return Err(format!("MediaMuxer addTrack returned {added}")); }
            let status = unsafe { sys::AMediaMuxer_start(muxer) };
            if status != sys::media_status_t::AMEDIA_OK { return Err(format!("MediaMuxer start returned {status:?}")); }
            *track = Some(added as usize);
            *muxer_started = true;
            continue;
        }
        if index < 0 { continue; }
        let mut capacity = 0usize;
        let data = unsafe { sys::AMediaCodec_getOutputBuffer(codec, index as usize, &mut capacity) };
        if data.is_null() { return Err(format!("encoder output buffer {index} was null")); }
        if info.size > 0 && info.flags & sys::AMEDIACODEC_BUFFER_FLAG_CODEC_CONFIG == 0 {
            let track = track.ok_or("encoded sample arrived before output format")?;
            let status = unsafe { sys::AMediaMuxer_writeSampleData(muxer, track, data, &info) };
            if status != sys::media_status_t::AMEDIA_OK { return Err(format!("MediaMuxer write returned {status:?}")); }
            *samples += 1;
        }
        let ended = info.flags & sys::AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM != 0;
        let status = unsafe { sys::AMediaCodec_releaseOutputBuffer(codec, index as usize, false) };
        if status != sys::media_status_t::AMEDIA_OK { return Err(format!("release output buffer returned {status:?}")); }
        if ended { *eos = true; return Ok(()); }
    }
}
