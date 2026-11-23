// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! The two roads, on the phone. See the crate's doc.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write as _;
use std::os::fd::AsRawFd;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use ash::vk;
use ndk::hardware_buffer::HardwareBufferUsage;
use ndk::media::image_reader::{AcquireResult, ImageFormat, ImageReader};
use ndk::media::media_codec::{
    DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection,
};
use ndk::media::media_format::MediaFormat;
use ndk_sys as ffi;
use wgpu::hal::api::Vulkan;

/// The picture both roads end at: the size the editor's preview draws at on
/// the test phone, from its log.
const OUT_W: u32 = 690;
const OUT_H: u32 = 1228;
/// A frame a road stops at, unless asked for another number.
const FRAMES: usize = 600;
/// MediaCodec's end-of-stream flag.
const END_OF_STREAM: u32 = 4;
/// MediaCodec's NV12 colour format, COLOR_FormatYUV420SemiPlanar.
const NV12: i32 = 21;

type Res<T> = Result<T, String>;

fn fail<E: std::fmt::Debug>(what: &str) -> impl FnOnce(E) -> String + '_ {
    move |error| format!("{what}: {error:?}")
}

/// The decoder lives in a service and reports each picture back over binder:
/// a process run from a shell has no threads listening for that until it
/// starts some, as an app's has from the start, and the decoder then never
/// hands back a buffer. The calls are the platform's own, not the NDK's stubs,
/// so they are looked up in the phone's library.
fn start_binder_threads() -> Res<()> {
    // SAFETY: dlopen and dlsym of a system library and two of its functions,
    // called with the signatures libbinder_ndk declares.
    unsafe {
        let library = libc::dlopen(c"libbinder_ndk.so".as_ptr(), libc::RTLD_NOW);
        if library.is_null() {
            return Err("no libbinder_ndk.so".into());
        }
        let max = libc::dlsym(
            library,
            c"ABinderProcess_setThreadPoolMaxThreadCount".as_ptr(),
        );
        let start = libc::dlsym(library, c"ABinderProcess_startThreadPool".as_ptr());
        if max.is_null() || start.is_null() {
            return Err("libbinder_ndk.so has no thread pool calls".into());
        }
        let max: extern "C" fn(u32) -> bool = std::mem::transmute(max);
        let start: extern "C" fn() = std::mem::transmute(start);
        max(4);
        start();
    }
    Ok(())
}

pub fn run() -> Res<()> {
    start_binder_threads()?;
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: zerocopy-proto <video.mp4> [cpu|gpu|both] [frames]")?;
    let mode = args.get(2).map_or("both", String::as_str);
    let frames = args.get(3).and_then(|n| n.parse().ok()).unwrap_or(FRAMES);
    let mut gpu = Gpu::new()?;
    println!("GPU: {}", gpu.adapter_name);
    let source = Source::open(path)?;
    println!(
        "file: {}x{} {} ({} frames timed)",
        source.width, source.height, source.mime, frames
    );
    drop(source);
    if mode == "cpu" || mode == "both" {
        let stats = cpu_road(path, &gpu, frames)?;
        stats.report("today: decode to memory, CPU convert + shrink, upload");
    }
    if mode == "gpu" || mode == "both" {
        let stats = gpu_road(path, &mut gpu, frames)?;
        stats.report("zero-copy: decode to a GPU buffer, Vulkan convert + shrink");
    }
    if mode == "both" {
        compare(
            &kept("zerocopy-cpu-last.ppm"),
            &kept("zerocopy-gpu-last.ppm"),
        )?;
    }
    if std::env::var_os("ZEROCOPY_KEEP").is_none() {
        // The pictures are of someone's own video: compared here, then gone.
        let _ = std::fs::remove_file(kept("zerocopy-cpu-last.ppm"));
        let _ = std::fs::remove_file(kept("zerocopy-gpu-last.ppm"));
    }
    Ok(())
}

/// How far the two roads' last pictures - the same frame of the file - are
/// apart, by numbers alone: the mean difference of each channel out of 255,
/// and the share of pixels any channel of which is more than 24 apart.
fn compare(cpu: &str, gpu: &str) -> Res<()> {
    let read = |path: &str| -> Res<Vec<u8>> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let header = format!("P6\n{OUT_W} {OUT_H}\n255\n").len();
        Ok(bytes[header..].to_vec())
    };
    let (a, b) = (read(cpu)?, read(gpu)?);
    if a.len() != b.len() {
        return Err("the two pictures differ in size".into());
    }
    let mut sum = [0u64; 3];
    let mut far = 0usize;
    for (pa, pb) in a.chunks_exact(3).zip(b.chunks_exact(3)) {
        let mut any = false;
        for c in 0..3 {
            let d = pa[c].abs_diff(pb[c]);
            sum[c] += u64::from(d);
            any |= d > 24;
        }
        far += usize::from(any);
    }
    let pixels = (a.len() / 3).max(1) as f64;
    println!();
    println!(
        "== the same frame by both roads: mean difference R {:.1}, G {:.1}, B {:.1} of 255; {:.1}% of pixels more than 24 apart",
        sum[0] as f64 / pixels,
        sum[1] as f64 / pixels,
        sum[2] as f64 / pixels,
        100.0 * far as f64 / pixels
    );
    Ok(())
}

/// What one road took.
struct Stats {
    frames: usize,
    wall: Duration,
    own_cpu: f64,
    phone_cpu: Option<f64>,
}

impl Stats {
    fn report(&self, road: &str) {
        let seconds = self.wall.as_secs_f64().max(1e-9);
        println!();
        println!("== {road}");
        println!(
            "   {} frames in {:.2} s: {:.1} frames a second",
            self.frames,
            seconds,
            self.frames as f64 / seconds
        );
        println!(
            "   this process: {:.0}% of one core ({:.2} ms of CPU a frame)",
            100.0 * self.own_cpu / seconds,
            1000.0 * self.own_cpu / self.frames.max(1) as f64
        );
        match self.phone_cpu {
            Some(busy) => println!(
                "   whole phone, decoder included: {:.0}% of one core ({:.2} ms a frame)",
                100.0 * busy / seconds,
                1000.0 * busy / self.frames.max(1) as f64
            ),
            None => println!("   whole phone: /proc/stat not readable here"),
        }
    }
}

/// Seconds of CPU this process has used.
fn own_cpu() -> f64 {
    // SAFETY: getrusage fills the struct it is given.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let seconds = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    seconds(usage.ru_utime) + seconds(usage.ru_stime)
}

/// Seconds every core of the phone has spent busy, from /proc/stat.
fn phone_cpu() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|f| f.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal: busy is all but idle
    // and iowait.
    let busy: u64 = fields
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 3 && *i != 4)
        .map(|(_, v)| *v)
        .sum();
    // SAFETY: sysconf has no preconditions.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
    Some(busy as f64 / ticks)
}

/// Times `road` between two readings of the clocks.
fn timed(frames: impl FnOnce() -> Res<usize>) -> Res<Stats> {
    let (own, phone, start) = (own_cpu(), phone_cpu(), Instant::now());
    let frames = frames()?;
    let wall = start.elapsed();
    Ok(Stats {
        frames,
        wall,
        own_cpu: own_cpu() - own,
        phone_cpu: phone.zip(phone_cpu()).map(|(a, b)| b - a),
    })
}

/// The file's video track, demuxed by the platform.
struct Source {
    extractor: *mut ffi::AMediaExtractor,
    _file: File,
    format: MediaFormat,
    mime: String,
    width: i32,
    height: i32,
    ended: bool,
}

impl Source {
    fn open(path: &str) -> Res<Self> {
        let file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        // SAFETY: a new extractor, given a file descriptor this struct keeps
        // open for as long as it lives.
        let extractor = unsafe { ffi::AMediaExtractor_new() };
        let status = unsafe {
            ffi::AMediaExtractor_setDataSourceFd(extractor, file.as_raw_fd(), 0, length as i64)
        };
        if status.0 != 0 {
            return Err(format!(
                "{path}: the platform will not read it ({})",
                status.0
            ));
        }
        let tracks = unsafe { ffi::AMediaExtractor_getTrackCount(extractor) };
        for track in 0..tracks {
            let raw = unsafe { ffi::AMediaExtractor_getTrackFormat(extractor, track) };
            let Some(raw) = NonNull::new(raw) else {
                continue;
            };
            // SAFETY: an AMediaFormat the extractor handed over to us.
            let mut format = unsafe { MediaFormat::from_ptr(raw) };
            let mime = format.str("mime").unwrap_or_default().to_owned();
            if mime.starts_with("video/") {
                unsafe { ffi::AMediaExtractor_selectTrack(extractor, track) };
                let width = format.i32("width").unwrap_or(0);
                let height = format.i32("height").unwrap_or(0);
                return Ok(Self {
                    extractor,
                    _file: file,
                    format,
                    mime,
                    width,
                    height,
                    ended: false,
                });
            }
        }
        Err(format!("{path}: no video track"))
    }

    /// Hands the decoder the next compressed picture, if it has room.
    fn feed(&mut self, codec: &MediaCodec) -> Res<()> {
        if self.ended {
            return Ok(());
        }
        let Ok(DequeuedInputBufferResult::Buffer(mut input)) =
            codec.dequeue_input_buffer(Duration::ZERO)
        else {
            return Ok(());
        };
        let room = input.buffer_mut();
        // SAFETY: the decoder's input buffer, `room.len()` bytes long.
        let read = unsafe {
            ffi::AMediaExtractor_readSampleData(
                self.extractor,
                room.as_mut_ptr().cast(),
                room.len(),
            )
        };
        if read < 0 {
            self.ended = true;
            codec
                .queue_input_buffer(input, 0, 0, 0, END_OF_STREAM)
                .map_err(fail("end of stream"))
        } else {
            let at = unsafe { ffi::AMediaExtractor_getSampleTime(self.extractor) }.max(0);
            codec
                .queue_input_buffer(input, 0, read as usize, at as u64, 0)
                .map_err(fail("queue input"))?;
            unsafe { ffi::AMediaExtractor_advance(self.extractor) };
            Ok(())
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: made in `open`, freed once.
        unsafe { ffi::AMediaExtractor_delete(self.extractor) };
    }
}

/// wgpu on Vulkan, opened with Android's hardware-buffer import turned on,
/// and the raw Vulkan objects the zero-copy road draws with.
struct Gpu {
    adapter_name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    vk: ash::Device,
    ahb: ash::android::external_memory_android_hardware_buffer::Device,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    raw_queue: vk::Queue,
    family: u32,
    _pool: vk::CommandPool,
    commands: vk::CommandBuffer,
    fence: vk::Fence,
    /// The preview-size picture the conversion writes, made by us and
    /// adopted by wgpu.
    picture: vk::Image,
    picture_view: vk::ImageView,
    _picture_memory: vk::DeviceMemory,
    picture_in_general: bool,
    wgpu_picture: Option<wgpu::Texture>,
    /// Made with the first decoded buffer, whose format they are for.
    convert: Option<Convert>,
    /// Each of the decoder's buffers, imported once: the reader hands the same
    /// few round and round.
    imported: HashMap<usize, Imported>,
}

struct Convert {
    conversion: vk::SamplerYcbcrConversion,
    _sampler: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptors: vk::DescriptorPool,
    external_format: u64,
    format: vk::Format,
}

struct Imported {
    image: vk::Image,
    _memory: vk::DeviceMemory,
    _view: vk::ImageView,
    set: vk::DescriptorSet,
}

impl Gpu {
    fn new() -> Res<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(fail("no Vulkan adapter"))?;
        let adapter_name = adapter.get_info().name;
        // NV12 brings Vulkan's Y'CbCr sampler conversion with it in wgpu-hal.
        let features = wgpu::Features::TEXTURE_FORMAT_NV12;
        if !adapter.features().contains(features) {
            return Err("this GPU has no Y'CbCr sampler conversion in wgpu".into());
        }
        let limits = wgpu::Limits::default()
            .or_worse_values_from(&adapter.limits())
            .using_resolution(adapter.limits());
        let open = {
            // SAFETY: the adapter is Vulkan, as the instance only has Vulkan.
            let hal = unsafe { adapter.as_hal::<Vulkan>() }.ok_or("not a Vulkan adapter")?;
            let extensions: [&'static std::ffi::CStr; 2] = [
                ash::android::external_memory_android_hardware_buffer::NAME,
                ash::ext::queue_family_foreign::NAME,
            ];
            // SAFETY: only extensions are added, both of which every Vulkan
            // Android device of API 26 and later has.
            unsafe {
                hal.open_with_callback(
                    features,
                    &limits,
                    &wgpu::MemoryHints::Performance,
                    Some(Box::new(move |args| {
                        for name in extensions {
                            if !args.extensions.contains(&name) {
                                args.extensions.push(name);
                            }
                        }
                    })),
                )
            }
            .map_err(fail("open the Vulkan device"))?
        };
        // SAFETY: opened from this adapter, with exactly these features.
        let (device, queue) = unsafe {
            adapter.create_device_from_hal(
                open,
                &wgpu::DeviceDescriptor {
                    label: Some("zero-copy prototype"),
                    required_features: features,
                    required_limits: limits,
                    ..Default::default()
                },
            )
        }
        .map_err(fail("wgpu device"))?;
        let (vk_device, instance_raw, physical, family, raw_queue) = {
            // SAFETY: a Vulkan device, read for its handles only.
            let hal = unsafe { device.as_hal::<Vulkan>() }.ok_or("not a Vulkan device")?;
            (
                hal.raw_device().clone(),
                hal.shared_instance().raw_instance().clone(),
                hal.raw_physical_device(),
                hal.queue_family_index(),
                hal.raw_queue(),
            )
        };
        let ahb = ash::android::external_memory_android_hardware_buffer::Device::new(
            &instance_raw,
            &vk_device,
        );
        let memory_properties =
            unsafe { instance_raw.get_physical_device_memory_properties(physical) };
        let pool = unsafe {
            vk_device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .map_err(fail("command pool"))?;
        let commands = unsafe {
            vk_device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .command_buffer_count(1),
            )
        }
        .map_err(fail("command buffer"))?[0];
        let fence = unsafe { vk_device.create_fence(&vk::FenceCreateInfo::default(), None) }
            .map_err(fail("fence"))?;
        let picture = unsafe {
            vk_device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: OUT_W,
                        height: OUT_H,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(
                        vk::ImageUsageFlags::STORAGE
                            | vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::TRANSFER_SRC
                            | vk::ImageUsageFlags::TRANSFER_DST,
                    )
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )
        }
        .map_err(fail("picture"))?;
        let needs = unsafe { vk_device.get_image_memory_requirements(picture) };
        let kind = memory_type(
            &memory_properties,
            needs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .ok_or("no memory for the picture")?;
        let picture_memory = unsafe {
            vk_device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(needs.size)
                    .memory_type_index(kind),
                None,
            )
        }
        .map_err(fail("picture memory"))?;
        unsafe { vk_device.bind_image_memory(picture, picture_memory, 0) }
            .map_err(fail("bind picture"))?;
        let picture_view = unsafe {
            vk_device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(picture)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .subresource_range(color_range()),
                None,
            )
        }
        .map_err(fail("picture view"))?;
        Ok(Self {
            adapter_name,
            device,
            queue,
            vk: vk_device,
            ahb,
            memory_properties,
            raw_queue,
            family,
            _pool: pool,
            commands,
            fence,
            picture,
            picture_view,
            _picture_memory: picture_memory,
            picture_in_general: false,
            wgpu_picture: None,
            convert: None,
            imported: HashMap::new(),
        })
    }

    /// The conversion, sampler and pipeline for the decoder's buffers, made
    /// from the first one's properties.
    fn convert_for(
        &mut self,
        format: &vk::AndroidHardwareBufferFormatPropertiesANDROID,
    ) -> Res<()> {
        if self.convert.is_some() {
            return Ok(());
        }
        let linear = format
            .format_features
            .contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_YCBCR_CONVERSION_LINEAR_FILTER);
        let filter = if linear {
            vk::Filter::LINEAR
        } else {
            vk::Filter::NEAREST
        };
        let mut external =
            vk::ExternalFormatANDROID::default().external_format(format.external_format);
        let mut info = vk::SamplerYcbcrConversionCreateInfo::default()
            .format(format.format)
            .ycbcr_model(format.suggested_ycbcr_model)
            .ycbcr_range(format.suggested_ycbcr_range)
            .components(format.sampler_ycbcr_conversion_components)
            .x_chroma_offset(format.suggested_x_chroma_offset)
            .y_chroma_offset(format.suggested_y_chroma_offset)
            .chroma_filter(filter);
        if format.format == vk::Format::UNDEFINED {
            info = info.push_next(&mut external);
        }
        let conversion = unsafe { self.vk.create_sampler_ycbcr_conversion(&info, None) }
            .map_err(fail("Y'CbCr conversion"))?;
        let mut with = vk::SamplerYcbcrConversionInfo::default().conversion(conversion);
        let sampler = unsafe {
            self.vk.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(filter)
                    .min_filter(filter)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .push_next(&mut with),
                None,
            )
        }
        .map_err(fail("sampler"))?;
        let samplers = [sampler];
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .immutable_samplers(&samplers),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];
        let set_layout = unsafe {
            self.vk.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .map_err(fail("set layout"))?;
        let layouts = [set_layout];
        let pipeline_layout = unsafe {
            self.vk.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts),
                None,
            )
        }
        .map_err(fail("pipeline layout"))?;
        let code = ash::util::read_spv(&mut std::io::Cursor::new(
            &include_bytes!("../shaders/convert.spv")[..],
        ))
        .map_err(fail("shader"))?;
        let module = unsafe {
            self.vk
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
        }
        .map_err(fail("shader module"))?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(module)
            .name(c"main");
        let pipeline = unsafe {
            self.vk.create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default()
                    .stage(stage)
                    .layout(pipeline_layout)],
                None,
            )
        }
        .map_err(|(_, e)| format!("pipeline: {e:?}"))?[0];
        unsafe { self.vk.destroy_shader_module(module, None) };
        // A Y'CbCr sampler may take more than one descriptor of its kind.
        let sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(64),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(32),
        ];
        let descriptors = unsafe {
            self.vk.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(32)
                    .pool_sizes(&sizes),
                None,
            )
        }
        .map_err(fail("descriptor pool"))?;
        self.convert = Some(Convert {
            conversion,
            _sampler: sampler,
            set_layout,
            pipeline_layout,
            pipeline,
            descriptors,
            external_format: format.external_format,
            format: format.format,
        });
        Ok(())
    }

    /// The descriptor set reading `buffer`, imported into Vulkan the first
    /// time it is seen.
    fn import(
        &mut self,
        buffer: &ndk::hardware_buffer::HardwareBuffer,
    ) -> Res<(vk::DescriptorSet, vk::Image)> {
        let key = buffer.as_ptr() as usize;
        if let Some(imported) = self.imported.get(&key) {
            return Ok((imported.set, imported.image));
        }
        let raw: *mut vk::AHardwareBuffer = buffer.as_ptr().cast();
        let mut format = vk::AndroidHardwareBufferFormatPropertiesANDROID::default();
        let mut properties =
            vk::AndroidHardwareBufferPropertiesANDROID::default().push_next(&mut format);
        unsafe {
            self.ahb
                .get_android_hardware_buffer_properties(raw, &mut properties)
        }
        .map_err(fail("hardware buffer properties"))?;
        let (size, bits) = (properties.allocation_size, properties.memory_type_bits);
        let format = format;
        if self.imported.is_empty() {
            println!(
                "   decoder buffer: Vulkan format {:?}, external format {:#x}, model {:?}, range {:?}",
                format.format,
                format.external_format,
                format.suggested_ycbcr_model,
                format.suggested_ycbcr_range
            );
        }
        self.convert_for(&format)?;
        let (conversion, external_format, vk_format, set_layout, descriptors) = {
            let c = self.convert.as_ref().ok_or("no conversion")?;
            (
                c.conversion,
                c.external_format,
                c.format,
                c.set_layout,
                c.descriptors,
            )
        };
        let desc = buffer.describe();
        let mut external_memory = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID);
        let mut external = vk::ExternalFormatANDROID::default().external_format(external_format);
        let mut image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(vk::Extent3D {
                width: desc.width,
                height: desc.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external_memory);
        if vk_format == vk::Format::UNDEFINED {
            image_info = image_info.push_next(&mut external);
        }
        let image =
            unsafe { self.vk.create_image(&image_info, None) }.map_err(fail("import image"))?;
        let kind = memory_type(
            &self.memory_properties,
            bits,
            vk::MemoryPropertyFlags::empty(),
        )
        .ok_or("no memory type for the decoder's buffer")?;
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut import = vk::ImportAndroidHardwareBufferInfoANDROID::default().buffer(raw);
        let memory = unsafe {
            self.vk.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(size)
                    .memory_type_index(kind)
                    .push_next(&mut dedicated)
                    .push_next(&mut import),
                None,
            )
        }
        .map_err(fail("import memory"))?;
        unsafe { self.vk.bind_image_memory(image, memory, 0) }.map_err(fail("bind import"))?;
        let mut with = vk::SamplerYcbcrConversionInfo::default().conversion(conversion);
        let view = unsafe {
            self.vk.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk_format)
                    .subresource_range(color_range())
                    .push_next(&mut with),
                None,
            )
        }
        .map_err(fail("import view"))?;
        let layouts = [set_layout];
        let set = unsafe {
            self.vk.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptors)
                    .set_layouts(&layouts),
            )
        }
        .map_err(fail("descriptor set"))?[0];
        let decoded = [vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let picture = [vk::DescriptorImageInfo::default()
            .image_view(self.picture_view)
            .image_layout(vk::ImageLayout::GENERAL)];
        unsafe {
            self.vk.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&decoded),
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(1)
                        .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                        .image_info(&picture),
                ],
                &[],
            )
        };
        self.imported.insert(
            key,
            Imported {
                image,
                _memory: memory,
                _view: view,
                set,
            },
        );
        Ok((set, image))
    }

    /// One decoded buffer converted and shrunk into the picture, waited for.
    /// With `keep`, the picture is also read back and written to `keep`.
    fn convert(
        &mut self,
        buffer: &ndk::hardware_buffer::HardwareBuffer,
        keep: Option<&str>,
    ) -> Res<()> {
        let (set, decoded) = self.import(buffer)?;
        let (pipeline, pipeline_layout) = {
            let c = self.convert.as_ref().ok_or("no conversion")?;
            (c.pipeline, c.pipeline_layout)
        };
        let vk_device = self.vk.clone();
        let cb = self.commands;
        unsafe {
            vk_device
                .reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())
                .map_err(fail("reset"))?;
            vk_device
                .begin_command_buffer(
                    cb,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(fail("begin"))?;
            // The decoder's buffer comes over from the decoder, a queue
            // outside Vulkan; the picture is written in GENERAL.
            let acquire = vk::ImageMemoryBarrier::default()
                .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                .dst_queue_family_index(self.family)
                .old_layout(vk::ImageLayout::GENERAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .image(decoded)
                .subresource_range(color_range());
            let picture = vk::ImageMemoryBarrier::default()
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .old_layout(if self.picture_in_general {
                    vk::ImageLayout::GENERAL
                } else {
                    vk::ImageLayout::UNDEFINED
                })
                .new_layout(vk::ImageLayout::GENERAL)
                .src_access_mask(vk::AccessFlags::SHADER_WRITE | vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_WRITE)
                .image(self.picture)
                .subresource_range(color_range());
            vk_device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TOP_OF_PIPE
                    | vk::PipelineStageFlags::COMPUTE_SHADER
                    | vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[acquire, picture],
            );
            vk_device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipeline);
            vk_device.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::COMPUTE,
                pipeline_layout,
                0,
                &[set],
                &[],
            );
            vk_device.cmd_dispatch(cb, OUT_W.div_ceil(16), OUT_H.div_ceil(16), 1);
            // Back to the decoder, which writes into it again.
            let release = vk::ImageMemoryBarrier::default()
                .src_queue_family_index(self.family)
                .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::GENERAL)
                .src_access_mask(vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::empty())
                .image(decoded)
                .subresource_range(color_range());
            vk_device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[release],
            );
            vk_device.end_command_buffer(cb).map_err(fail("end"))?;
            vk_device
                .queue_submit(
                    self.raw_queue,
                    &[vk::SubmitInfo::default().command_buffers(&[cb])],
                    self.fence,
                )
                .map_err(fail("submit"))?;
            vk_device
                .wait_for_fences(&[self.fence], true, u64::MAX)
                .map_err(fail("wait"))?;
            vk_device
                .reset_fences(&[self.fence])
                .map_err(fail("reset fence"))?;
        }
        self.picture_in_general = true;
        if let Some(path) = keep {
            // The hand-over the engine needs: our picture as a wgpu texture,
            // read back through wgpu itself.
            let texture = self.adopt()?;
            let pixels = read_texture(&self.device, &self.queue, &texture)?;
            write_ppm(path, &pixels)?;
        }
        Ok(())
    }

    /// The picture as a wgpu texture, adopted once: wgpu draws from what the
    /// conversion wrote, with nothing copied.
    fn adopt(&mut self) -> Res<wgpu::Texture> {
        if let Some(texture) = &self.wgpu_picture {
            return Ok(texture.clone());
        }
        let desc = wgpu::TextureDescriptor {
            label: Some("zero-copy picture"),
            size: wgpu::Extent3d {
                width: OUT_W,
                height: OUT_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: desc.label,
            size: desc.size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: desc.dimension,
            format: desc.format,
            usage: wgpu::TextureUses::RESOURCE
                | wgpu::TextureUses::STORAGE_READ_WRITE
                | wgpu::TextureUses::COPY_SRC,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        let texture = unsafe {
            let hal = self
                .device
                .as_hal::<Vulkan>()
                .ok_or("not a Vulkan device")?;
            // The image and its memory stay ours: freed with the prototype.
            let raw = hal.texture_from_raw(
                self.picture,
                &hal_desc,
                Some(Box::new(|| {})),
                wgpu::hal::vulkan::TextureMemory::External,
            );
            drop(hal);
            self.device.create_texture_from_hal::<Vulkan>(
                raw,
                &desc,
                wgpu::TextureUses::STORAGE_READ_WRITE,
            )
        };
        self.wgpu_picture = Some(texture.clone());
        Ok(texture)
    }
}

/// The first memory type among `bits` with every flag of `flags`.
fn memory_type(
    properties: &vk::PhysicalDeviceMemoryProperties,
    bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..properties.memory_type_count).find(|&i| {
        bits & (1 << i) != 0
            && properties.memory_types[i as usize]
                .property_flags
                .contains(flags)
    })
}

fn color_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .level_count(1)
        .layer_count(1)
}

/// An RGBA8 texture of the picture's size read back through wgpu, row by row.
fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Res<Vec<u8>> {
    let row = (OUT_W * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("read back"),
        size: u64::from(row * OUT_H),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(OUT_H),
            },
        },
        wgpu::Extent3d {
            width: OUT_W,
            height: OUT_H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(fail("read back"))?;
    let mapped = buffer.slice(..).get_mapped_range().map_err(fail("map"))?;
    let mut pixels = Vec::with_capacity((OUT_W * OUT_H * 4) as usize);
    for y in 0..OUT_H as usize {
        let start = y * row as usize;
        pixels.extend_from_slice(&mapped[start..start + (OUT_W * 4) as usize]);
    }
    Ok(pixels)
}

/// RGBA pixels of the picture's size, as a binary PPM.
fn write_ppm(path: &str, rgba: &[u8]) -> Res<()> {
    let mut file = File::create(path).map_err(|e| format!("{path}: {e}"))?;
    write!(file, "P6\n{OUT_W} {OUT_H}\n255\n").map_err(|e| e.to_string())?;
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    file.write_all(&rgb).map_err(|e| e.to_string())?;
    println!("   first picture written to {path}");
    Ok(())
}

/// Where the first picture of a road is written: beside the binary.
fn kept(name: &str) -> String {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| "/data/local/tmp".into());
    dir.join(name).to_string_lossy().into_owned()
}

/// Today's road: the decoder writes NV12 into memory, swscale converts and
/// shrinks it on the CPU, bilinear, and the result is uploaded to a texture.
fn cpu_road(path: &str, gpu: &Gpu, frames: usize) -> Res<Stats> {
    use ffmpeg_the_third::ffi as av;
    let mut source = Source::open(path)?;
    let codec = MediaCodec::from_decoder_type(&source.mime).ok_or("no hardware decoder")?;
    codec
        .configure(&source.format, None, MediaCodecDirection::Decoder)
        .map_err(fail("configure"))?;
    codec.start().map_err(fail("start"))?;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uploaded picture"),
        size: wgpu::Extent3d {
            width: OUT_W,
            height: OUT_H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut rgba = vec![0u8; (OUT_W * OUT_H * 4) as usize];
    let mut scaler: *mut av::SwsContext = std::ptr::null_mut();
    let mut layout: Option<(i32, i32, i32, i32)> = None;
    let stats = timed(|| {
        let mut done = 0;
        while done < frames {
            source.feed(&codec)?;
            match codec
                .dequeue_output_buffer(Duration::from_millis(10))
                .map_err(fail("dequeue"))?
            {
                DequeuedOutputBufferInfoResult::Buffer(output) => {
                    let info = *output.info();
                    let end = info.flags() & END_OF_STREAM != 0;
                    if info.size() > 0 {
                        let (width, height, stride, rows) = match layout {
                            Some(l) => l,
                            None => {
                                let format = output.format();
                                let colour = format.i32("color-format").unwrap_or(-1);
                                if colour != NV12 {
                                    return Err(format!(
                                        "the decoder writes colour format {colour:#x}, not NV12"
                                    ));
                                }
                                let width = format.i32("width").unwrap_or(source.width);
                                let height = format.i32("height").unwrap_or(source.height);
                                let stride = format.i32("stride").unwrap_or(width).max(width);
                                let rows = format.i32("slice-height").unwrap_or(height).max(height);
                                // SAFETY: a scaler for these sizes and formats.
                                scaler = unsafe {
                                    av::sws_getContext(
                                        width,
                                        height,
                                        ffmpeg_the_third::format::Pixel::NV12.into(),
                                        OUT_W as i32,
                                        OUT_H as i32,
                                        ffmpeg_the_third::format::Pixel::RGBA.into(),
                                        ffmpeg_the_third::software::scaling::Flags::BILINEAR.bits(),
                                        std::ptr::null_mut(),
                                        std::ptr::null_mut(),
                                        std::ptr::null(),
                                    )
                                };
                                if scaler.is_null() {
                                    return Err("no scaler".into());
                                }
                                let l = (width, height, stride, rows);
                                layout = Some(l);
                                l
                            }
                        };
                        let bytes = &output.buffer()[info.offset() as usize..];
                        let planes = [bytes.as_ptr(), bytes[(stride * rows) as usize..].as_ptr()];
                        let strides = [stride, stride];
                        let out = [rgba.as_mut_ptr()];
                        let out_strides = [(OUT_W * 4) as i32];
                        // SAFETY: two NV12 planes of `stride` bytes a row, into
                        // one RGBA plane of the picture's size.
                        unsafe {
                            av::sws_scale(
                                scaler,
                                planes.as_ptr(),
                                strides.as_ptr(),
                                0,
                                height,
                                out.as_ptr(),
                                out_strides.as_ptr(),
                            )
                        };
                        let _ = width;
                        gpu.queue.write_texture(
                            texture.as_image_copy(),
                            &rgba,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(OUT_W * 4),
                                rows_per_image: Some(OUT_H),
                            },
                            wgpu::Extent3d {
                                width: OUT_W,
                                height: OUT_H,
                                depth_or_array_layers: 1,
                            },
                        );
                        gpu.queue.submit([]);
                        gpu.device
                            .poll(wgpu::PollType::wait_indefinitely())
                            .map_err(fail("upload"))?;
                        done += 1;
                    }
                    codec
                        .release_output_buffer(output, false)
                        .map_err(fail("release"))?;
                    if end {
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(done)
    })?;
    let _ = codec.stop();
    if !scaler.is_null() {
        // SAFETY: made above, freed once.
        unsafe { av::sws_freeContext(scaler) };
    }
    let pixels = read_texture(&gpu.device, &gpu.queue, &texture)?;
    write_ppm(&kept("zerocopy-cpu-last.ppm"), &pixels)?;
    Ok(stats)
}

/// The zero-copy road: the decoder renders into an image reader's buffers,
/// each imported into Vulkan and converted and shrunk by one compute pass.
fn gpu_road(path: &str, gpu: &mut Gpu, frames: usize) -> Res<Stats> {
    let mut source = Source::open(path)?;
    let reader = ImageReader::new_with_usage(
        source.width,
        source.height,
        ImageFormat::PRIVATE,
        HardwareBufferUsage::GPU_SAMPLED_IMAGE,
        4,
    )
    .map_err(fail("image reader"))?;
    let window = reader.window().map_err(fail("reader window"))?;
    let codec = MediaCodec::from_decoder_type(&source.mime).ok_or("no hardware decoder")?;
    codec
        .configure(&source.format, Some(&window), MediaCodecDirection::Decoder)
        .map_err(fail("configure"))?;
    codec.start().map_err(fail("start"))?;
    let last = kept("zerocopy-gpu-last.ppm");
    let stats = timed(|| {
        let mut done = 0;
        let mut ended = false;
        while done < frames && !ended {
            source.feed(&codec)?;
            if let DequeuedOutputBufferInfoResult::Buffer(output) = codec
                .dequeue_output_buffer(Duration::from_millis(10))
                .map_err(fail("dequeue"))?
            {
                ended = output.info().flags() & END_OF_STREAM != 0;
                let shown = output.info().size() > 0;
                codec
                    .release_output_buffer(output, shown)
                    .map_err(fail("render"))?;
                if !shown {
                    continue;
                }
                // The picture reaches the reader a moment after it is rendered.
                let waited = Instant::now();
                let image = loop {
                    match reader.acquire_next_image().map_err(fail("acquire"))? {
                        AcquireResult::Image(image) => break image,
                        _ if waited.elapsed() > Duration::from_millis(500) => {
                            return Err("the reader got no picture".into());
                        }
                        _ => std::thread::sleep(Duration::from_micros(200)),
                    }
                };
                let buffer = image.hardware_buffer().map_err(fail("hardware buffer"))?;
                done += 1;
                let keep = (done == frames).then_some(last.as_str());
                gpu.convert(&buffer, keep)?;
                drop(image);
            }
        }
        Ok(done)
    })?;
    let _ = codec.stop();
    if stats.frames < frames {
        // Ended early: the last picture converted is still in the texture.
        let texture = gpu.adopt()?;
        let pixels = read_texture(&gpu.device, &gpu.queue, &texture)?;
        write_ppm(&last, &pixels)?;
    }
    Ok(stats)
}
