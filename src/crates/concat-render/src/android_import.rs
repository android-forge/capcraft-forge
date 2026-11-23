// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

//! Zero-copy frames on a phone: a picture the phone's video decoder wrote into
//! an `AHardwareBuffer` imported into the compositor's own Vulkan device,
//! converted from Y'CbCr, turned upright and scaled by one compute pass, and
//! handed to the compositor as a texture - with nothing copied through the CPU.
//!
//! Today's road copies each decoded picture out of the decoder into memory,
//! converts and scales it on the cores and uploads it again: for 4K, some 23 ms
//! of CPU a frame. This one took 4 to 5 (`crates/zerocopy-proto`, an Adreno
//! 825), and a 4K60 file decoded at 160 frames a second rather than 45.
//!
//! The device is opened with Android's hardware-buffer import and, where the
//! queue family has it, a second queue of its own ([`open_device`]): the
//! imports are submitted from the decoders' threads, which may not share
//! wgpu's queue with it. Where anything is missing - no Vulkan, no Y'CbCr
//! sampler conversion, no second queue - no importer is installed, and every
//! frame is read the way it always was.

use std::any::Any;
use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::sync::{Arc, Mutex};

use ash::vk;
use wgpu::hal::api::Vulkan;

/// A picture an [`Importer`] made: an RGBA texture on the compositor's device.
pub struct GpuPicture {
    pub(crate) texture: wgpu::Texture,
    /// The raw Vulkan device it is on: a compositor on another device cannot
    /// draw it.
    pub(crate) device: u64,
}

/// The device extensions an import needs beside what wgpu enables.
const EXTENSIONS: [&CStr; 2] = [
    ash::android::external_memory_android_hardware_buffer::NAME,
    ash::ext::queue_family_foreign::NAME,
];

/// Both queues of the family at the same priority.
static PRIORITIES: [f32; 2] = [1.0, 1.0];

/// The converting compute shader; see `shaders/android_import.comp`.
const SHADER: &[u8] = include_bytes!("../shaders/android_import.spv");

/// The raw Vulkan device under `device`, as a number to compare.
pub(crate) fn raw_device_of(device: &wgpu::Device) -> Option<u64> {
    use ash::vk::Handle as _;
    // SAFETY: read for its handle only.
    let hal = unsafe { device.as_hal::<Vulkan>() }?;
    Some(hal.raw_device().handle().as_raw())
}

/// `adapter`'s device opened as `desc` asks, plus what an import needs, and an
/// importer for it installed for the decoders. `None` - and the caller opens
/// the device its usual way - where the adapter cannot import.
pub(crate) fn open_device(
    adapter: &wgpu::Adapter,
    desc: &wgpu::DeviceDescriptor<'_>,
) -> Option<(wgpu::Device, wgpu::Queue)> {
    if adapter.get_info().backend != wgpu::Backend::Vulkan
        || !adapter.features().contains(wgpu::Features::TEXTURE_FORMAT_NV12)
    {
        return None;
    }
    let features = desc.required_features | wgpu::Features::TEXTURE_FORMAT_NV12;
    let open = {
        // SAFETY: a Vulkan adapter, as the backend says.
        let hal = unsafe { adapter.as_hal::<Vulkan>() }?;
        let instance = hal.shared_instance().raw_instance();
        let physical = hal.raw_physical_device();
        // SAFETY: queries of a live physical device.
        let available = unsafe { instance.enumerate_device_extension_properties(physical) }.ok()?;
        let has = |name: &CStr| {
            available
                .iter()
                .any(|e| e.extension_name_as_c_str().is_ok_and(|n| n == name))
        };
        if !EXTENSIONS.iter().all(|name| has(name)) {
            log::info!("compositor: no zero-copy frames: the device cannot import hardware buffers");
            return None;
        }
        let families = unsafe { instance.get_physical_device_queue_family_properties(physical) };
        let second = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let asked = Arc::clone(&second);
        // SAFETY: extensions the device has are added and a queue its family
        // has is asked for; nothing is removed.
        let opened = unsafe {
            hal.open_with_callback(
                features,
                &desc.required_limits,
                &desc.memory_hints,
                Some(Box::new(move |args| {
                    for name in EXTENSIONS {
                        if !args.extensions.contains(&name) {
                            args.extensions.push(name);
                        }
                    }
                    if let Some(info) = args.queue_create_infos.first_mut() {
                        let family = info.queue_family_index as usize;
                        if families.get(family).is_some_and(|f| f.queue_count >= 2) {
                            *info = info.queue_priorities(&PRIORITIES);
                            asked.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                })),
            )
        };
        let open = match opened {
            Ok(open) => open,
            Err(error) => {
                log::warn!("compositor: no zero-copy frames: {error}");
                return None;
            }
        };
        if !second.load(std::sync::atomic::Ordering::Relaxed) {
            // Opened, so it is used; but with one queue there is no importing.
            log::info!("compositor: no zero-copy frames: one queue only");
        }
        (open, second.load(std::sync::atomic::Ordering::Relaxed))
    };
    let (open, second) = open;
    // SAFETY: opened from this adapter with these features.
    let (device, queue) = unsafe {
        adapter.create_device_from_hal(
            open,
            &wgpu::DeviceDescriptor {
                required_features: features,
                ..desc.clone()
            },
        )
    }
    .map_err(|error| log::warn!("compositor: no zero-copy device: {error}"))
    .ok()?;
    if second {
        match Importer::new(&device) {
            Ok(importer) => {
                concat_core::gpu_import::install(Arc::new(importer));
                log::info!("compositor: zero-copy frames from the phone's decoder");
            }
            Err(error) => log::warn!("compositor: no zero-copy frames: {error}"),
        }
    }
    Some((device, queue))
}

/// One output picture's Vulkan objects, made once and used again.
struct Output {
    image: vk::Image,
    _memory: vk::DeviceMemory,
    view: vk::ImageView,
}

/// The conversion and pipeline for one of the decoder's buffer formats.
struct Convert {
    _conversion: vk::SamplerYcbcrConversion,
    _sampler: vk::Sampler,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    set: vk::DescriptorSet,
}

struct Inner {
    vk: ash::Device,
    ahb: ash::android::external_memory_android_hardware_buffer::Device,
    memory: vk::PhysicalDeviceMemoryProperties,
    queue: vk::Queue,
    family: u32,
    _pool: vk::CommandPool,
    commands: vk::CommandBuffer,
    fence: vk::Fence,
    descriptors: vk::DescriptorPool,
    converts: HashMap<(u64, i32), Convert>,
    /// Output pictures no texture holds now, by size.
    free: Arc<Mutex<HashMap<(u32, u32), Vec<Output>>>>,
}

// SAFETY: Vulkan handles, used only under the importer's lock.
unsafe impl Send for Inner {}

/// Imports a decoder's buffers into the compositor's device; see the module.
pub(crate) struct Importer {
    inner: Mutex<Inner>,
    device: wgpu::Device,
    raw: u64,
}

type Res<T> = Result<T, String>;

fn fail(what: &'static str) -> impl FnOnce(vk::Result) -> String {
    move |error| format!("{what}: {error:?}")
}

fn color_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .level_count(1)
        .layer_count(1)
}

fn memory_type(
    properties: &vk::PhysicalDeviceMemoryProperties,
    bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..properties.memory_type_count).find(|&i| {
        bits & (1 << i) != 0 && properties.memory_types[i as usize].property_flags.contains(flags)
    })
}

impl Importer {
    fn new(device: &wgpu::Device) -> Res<Self> {
        let (vk_device, instance, physical, family) = {
            // SAFETY: a Vulkan device, read for its handles.
            let hal = unsafe { device.as_hal::<Vulkan>() }.ok_or("not a Vulkan device")?;
            (
                hal.raw_device().clone(),
                hal.shared_instance().raw_instance().clone(),
                hal.raw_physical_device(),
                hal.queue_family_index(),
            )
        };
        let raw = raw_device_of(device).ok_or("no raw device")?;
        let ahb = ash::android::external_memory_android_hardware_buffer::Device::new(&instance, &vk_device);
        // SAFETY: the second queue of the family, asked for when the device
        // was opened; wgpu has the first.
        let queue = unsafe { vk_device.get_device_queue(family, 1) };
        let memory = unsafe { instance.get_physical_device_memory_properties(physical) };
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
        // A Y'CbCr sampler may take several descriptors of its kind.
        let sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(64),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(16),
        ];
        let descriptors = unsafe {
            vk_device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(16)
                    .pool_sizes(&sizes),
                None,
            )
        }
        .map_err(fail("descriptor pool"))?;
        Ok(Self {
            inner: Mutex::new(Inner {
                vk: vk_device,
                ahb,
                memory,
                queue,
                family,
                _pool: pool,
                commands,
                fence,
                descriptors,
                converts: HashMap::new(),
                free: Arc::new(Mutex::new(HashMap::new())),
            }),
            device: device.clone(),
            raw,
        })
    }
}

impl concat_core::gpu_import::Importer for Importer {
    fn import(
        &self,
        buffer: *mut c_void,
        crop: [u32; 4],
        rotation: u32,
        size: (u32, u32),
    ) -> Option<Arc<dyn Any + Send + Sync>> {
        let mut inner = self.inner.lock().ok()?;
        match inner.import(buffer.cast(), crop, rotation, size) {
            Ok(output) => {
                let free = Arc::clone(&inner.free);
                drop(inner);
                Some(Arc::new(self.adopt(output, size, free)))
            }
            Err(error) => {
                static ONCE: std::sync::Once = std::sync::Once::new();
                ONCE.call_once(|| log::warn!("zero-copy import failed: {error}"));
                None
            }
        }
    }
}

impl Importer {
    /// `output` as a wgpu texture; given back to the free outputs when wgpu
    /// is done with it.
    fn adopt(
        &self,
        output: Output,
        (width, height): (u32, u32),
        free: Arc<Mutex<HashMap<(u32, u32), Vec<Output>>>>,
    ) -> GpuPicture {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let desc = wgpu::TextureDescriptor {
            label: Some("zero-copy frame"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: desc.label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: desc.dimension,
            format: desc.format,
            usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_SRC,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        let image = output.image;
        let back = Box::new(move || {
            if let Ok(mut free) = free.lock() {
                free.entry((width, height)).or_default().push(output);
            }
        });
        // SAFETY: an image of this device, in SHADER_READ_ONLY_OPTIMAL (wgpu's
        // RESOURCE), whose memory stays ours until the callback has it back.
        let texture = unsafe {
            let hal = self.device.as_hal::<Vulkan>().expect("a Vulkan device");
            let raw = hal.texture_from_raw(
                image,
                &hal_desc,
                Some(back),
                wgpu::hal::vulkan::TextureMemory::External,
            );
            drop(hal);
            self.device
                .create_texture_from_hal::<Vulkan>(raw, &desc, wgpu::TextureUses::RESOURCE)
        };
        GpuPicture { texture, device: self.raw }
    }
}

impl Inner {
    /// The conversion for a buffer of this format, made the first time.
    fn convert_for(
        &mut self,
        format: &vk::AndroidHardwareBufferFormatPropertiesANDROID<'_>,
    ) -> Res<(vk::PipelineLayout, vk::Pipeline, vk::DescriptorSet)> {
        let key = (format.external_format, format.format.as_raw());
        if let Some(c) = self.converts.get(&key) {
            return Ok((c.pipeline_layout, c.pipeline, c.set));
        }
        let vkd = &self.vk;
        let linear = format
            .format_features
            .contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_YCBCR_CONVERSION_LINEAR_FILTER);
        let filter = if linear { vk::Filter::LINEAR } else { vk::Filter::NEAREST };
        let mut external = vk::ExternalFormatANDROID::default().external_format(format.external_format);
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
        // SAFETY: objects of this device, made from its own answers.
        unsafe {
            let conversion = vkd
                .create_sampler_ycbcr_conversion(&info, None)
                .map_err(fail("Y'CbCr conversion"))?;
            let mut with = vk::SamplerYcbcrConversionInfo::default().conversion(conversion);
            let sampler = vkd
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(filter)
                        .min_filter(filter)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .push_next(&mut with),
                    None,
                )
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
            let set_layout = vkd
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .map_err(fail("set layout"))?;
            let layouts = [set_layout];
            let ranges = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .offset(0)
                .size(20)];
            let pipeline_layout = vkd
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&layouts)
                        .push_constant_ranges(&ranges),
                    None,
                )
                .map_err(fail("pipeline layout"))?;
            let code = ash::util::read_spv(&mut std::io::Cursor::new(SHADER))
                .map_err(|e| format!("shader: {e}"))?;
            let module = vkd
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)
                .map_err(fail("shader module"))?;
            let stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(module)
                .name(c"main");
            let pipeline = vkd
                .create_compute_pipelines(
                    vk::PipelineCache::null(),
                    &[vk::ComputePipelineCreateInfo::default()
                        .stage(stage)
                        .layout(pipeline_layout)],
                    None,
                )
                .map_err(|(_, e)| format!("pipeline: {e:?}"))?[0];
            vkd.destroy_shader_module(module, None);
            let set = vkd
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(self.descriptors)
                        .set_layouts(&layouts),
                )
                .map_err(fail("descriptor set"))?[0];
            self.converts.insert(
                key,
                Convert {
                    _conversion: conversion,
                    _sampler: sampler,
                    pipeline_layout,
                    pipeline,
                    set,
                },
            );
            log::info!(
                "zero-copy: decoder buffers in {:?} (external {:#x}), {:?} {:?}, {} chroma",
                format.format,
                format.external_format,
                format.suggested_ycbcr_model,
                format.suggested_ycbcr_range,
                if linear { "linear" } else { "nearest" }
            );
            Ok((pipeline_layout, pipeline, set))
        }
    }

    /// An output picture of `size`: a free one, or a new one.
    fn output(&mut self, (width, height): (u32, u32)) -> Res<Output> {
        if let Some(output) = self
            .free
            .lock()
            .ok()
            .and_then(|mut free| free.get_mut(&(width, height)).and_then(Vec::pop))
        {
            return Ok(output);
        }
        let vkd = &self.vk;
        // SAFETY: a new image and its memory on this device.
        unsafe {
            let image = vkd
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .extent(vk::Extent3D { width, height, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(
                            vk::ImageUsageFlags::STORAGE
                                | vk::ImageUsageFlags::SAMPLED
                                | vk::ImageUsageFlags::TRANSFER_SRC,
                        )
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                    None,
                )
                .map_err(fail("output image"))?;
            let needs = vkd.get_image_memory_requirements(image);
            let kind = memory_type(&self.memory, needs.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)
                .ok_or("no memory for an output picture")?;
            let memory = vkd
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(needs.size)
                        .memory_type_index(kind),
                    None,
                )
                .map_err(fail("output memory"))?;
            vkd.bind_image_memory(image, memory, 0).map_err(fail("bind output"))?;
            let view = vkd
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .subresource_range(color_range()),
                    None,
                )
                .map_err(fail("output view"))?;
            Ok(Output { image, _memory: memory, view })
        }
    }

    /// The buffer's picture converted into an output picture, waited for.
    fn import(
        &mut self,
        buffer: *mut vk::AHardwareBuffer,
        crop: [u32; 4],
        rotation: u32,
        size: (u32, u32),
    ) -> Res<Output> {
        let mut format = vk::AndroidHardwareBufferFormatPropertiesANDROID::default();
        let mut properties = vk::AndroidHardwareBufferPropertiesANDROID::default().push_next(&mut format);
        // SAFETY: a live buffer the decoder lent for this call.
        unsafe { self.ahb.get_android_hardware_buffer_properties(buffer, &mut properties) }
            .map_err(fail("buffer properties"))?;
        let (bytes, bits) = (properties.allocation_size, properties.memory_type_bits);
        let (external, vk_format) = (format.external_format, format.format);
        let (layout, pipeline, set) = self.convert_for(&format)?;
        let output = self.output(size)?;
        let vkd = self.vk.clone();
        let (buffer_width, buffer_height) = {
            let mut desc = std::mem::MaybeUninit::<AHardwareBufferDesc>::zeroed();
            // SAFETY: AHardwareBuffer_describe fills the struct it is given.
            unsafe { AHardwareBuffer_describe(buffer.cast(), desc.as_mut_ptr()) };
            let desc = unsafe { desc.assume_init() };
            (desc.width, desc.height)
        };
        // SAFETY: Vulkan objects made, used and destroyed here, the buffer
        // imported for this conversion only, everything waited for.
        unsafe {
            let mut external_memory = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID);
            let mut external_format = vk::ExternalFormatANDROID::default().external_format(external);
            let mut info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk_format)
                .extent(vk::Extent3D { width: buffer_width, height: buffer_height, depth: 1 })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::SAMPLED)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .push_next(&mut external_memory);
            if vk_format == vk::Format::UNDEFINED {
                info = info.push_next(&mut external_format);
            }
            let image = vkd.create_image(&info, None).map_err(fail("import image"))?;
            let kind = memory_type(&self.memory, bits, vk::MemoryPropertyFlags::empty())
                .ok_or("no memory type for the decoder's buffer")?;
            let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
            let mut import = vk::ImportAndroidHardwareBufferInfoANDROID::default().buffer(buffer);
            let memory = vkd
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(bytes)
                        .memory_type_index(kind)
                        .push_next(&mut dedicated)
                        .push_next(&mut import),
                    None,
                )
                .map_err(fail("import memory"))?;
            let cleanup = |view: Option<vk::ImageView>| {
                if let Some(view) = view {
                    vkd.destroy_image_view(view, None);
                }
                vkd.destroy_image(image, None);
                vkd.free_memory(memory, None);
            };
            if let Err(error) = vkd.bind_image_memory(image, memory, 0) {
                cleanup(None);
                return Err(format!("bind import: {error:?}"));
            }
            let conversion = self
                .converts
                .get(&(external, vk_format.as_raw()))
                .map(|c| c._conversion)
                .ok_or("no conversion")?;
            let mut with = vk::SamplerYcbcrConversionInfo::default().conversion(conversion);
            let view = match vkd.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk_format)
                    .subresource_range(color_range())
                    .push_next(&mut with),
                None,
            ) {
                Ok(view) => view,
                Err(error) => {
                    cleanup(None);
                    return Err(format!("import view: {error:?}"));
                }
            };
            let decoded = [vk::DescriptorImageInfo::default()
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let picture = [vk::DescriptorImageInfo::default()
                .image_view(output.view)
                .image_layout(vk::ImageLayout::GENERAL)];
            vkd.update_descriptor_sets(
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
            );
            let cb = self.commands;
            let recorded = (|| -> Res<()> {
                vkd.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())
                    .map_err(fail("reset"))?;
                vkd.begin_command_buffer(
                    cb,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(fail("begin"))?;
                let acquire = vk::ImageMemoryBarrier::default()
                    .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .dst_queue_family_index(self.family)
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .image(image)
                    .subresource_range(color_range());
                let fresh = vk::ImageMemoryBarrier::default()
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .dst_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .image(output.image)
                    .subresource_range(color_range());
                vkd.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[acquire, fresh],
                );
                vkd.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipeline);
                vkd.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, layout, 0, &[set], &[]);
                let (bw, bh) = (buffer_width.max(1) as f32, buffer_height.max(1) as f32);
                let mut place = [0u8; 20];
                for (i, value) in [
                    crop[0] as f32 / bw,
                    crop[1] as f32 / bh,
                    crop[2] as f32 / bw,
                    crop[3] as f32 / bh,
                ]
                .into_iter()
                .enumerate()
                {
                    place[i * 4..i * 4 + 4].copy_from_slice(&value.to_ne_bytes());
                }
                place[16..20].copy_from_slice(&(rotation as i32).to_ne_bytes());
                vkd.cmd_push_constants(cb, layout, vk::ShaderStageFlags::COMPUTE, 0, &place);
                vkd.cmd_dispatch(cb, size.0.div_ceil(16), size.1.div_ceil(16), 1);
                let release = vk::ImageMemoryBarrier::default()
                    .src_queue_family_index(self.family)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_access_mask(vk::AccessFlags::SHADER_READ)
                    .image(image)
                    .subresource_range(color_range());
                let ready = vk::ImageMemoryBarrier::default()
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .image(output.image)
                    .subresource_range(color_range());
                vkd.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[release, ready],
                );
                vkd.end_command_buffer(cb).map_err(fail("end"))?;
                vkd.queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[cb])],
                    self.fence,
                )
                .map_err(fail("submit"))?;
                vkd.wait_for_fences(&[self.fence], true, u64::MAX)
                    .map_err(fail("wait"))?;
                vkd.reset_fences(&[self.fence]).map_err(fail("reset fence"))?;
                Ok(())
            })();
            cleanup(Some(view));
            match recorded {
                Ok(()) => Ok(output),
                Err(error) => {
                    if let Ok(mut free) = self.free.lock() {
                        free.entry(size).or_default().push(output);
                    }
                    Err(error)
                }
            }
        }
    }
}

/// `AHardwareBuffer_Desc`, as the NDK lays it out.
#[repr(C)]
struct AHardwareBufferDesc {
    width: u32,
    height: u32,
    layers: u32,
    format: u32,
    usage: u64,
    stride: u32,
    rfu0: u32,
    rfu1: u64,
}

#[link(name = "android")]
unsafe extern "C" {
    fn AHardwareBuffer_describe(buffer: *const c_void, desc: *mut AHardwareBufferDesc);
}
