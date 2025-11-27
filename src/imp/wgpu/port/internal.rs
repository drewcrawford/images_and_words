// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::forward::dynamic::buffer::Buffer;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::camera::Camera;
use crate::images::port::{FrameGuard, PortReporterSend};
use crate::images::render_pass::PassDescriptor;
#[cfg(feature = "exfiltrate")]
use crate::imp::DUMP_NEXT_FRAME;
use crate::imp::wgpu::cell::WgpuCell;
use crate::imp::wgpu::context::smuggle_async;
use crate::imp::{CopyInfo, Error};
#[cfg(feature = "exfiltrate")]
use exfiltrate::command::ImageInfo;
#[cfg(feature = "exfiltrate")]
use exfiltrate::rgb::RGBA8;
use std::sync::Arc;
#[cfg(feature = "exfiltrate")]
use wgpu::wgt::BufferDescriptor;
use wgpu::{
    Color, CommandEncoder, CompositeAlphaMode, LoadOp, Operations,
    RenderPassDepthStencilAttachment, StoreOp, TextureFormat,
};

use super::guards::{AcquiredGuards, BindGroupGuard};
use super::prepared_pass::PreparedPass;
#[cfg(feature = "exfiltrate")]
use super::types::DebugCaptureData;
use super::types::{CameraProjection, PassConfig, RenderInput};

#[cfg(feature = "exfiltrate")]
#[derive(Clone, Copy)]
enum DumpImageFormat {
    Color(TextureFormat),
    Depth16Unorm,
}

#[cfg(feature = "exfiltrate")]
fn dump_image(
    map_result: Result<(), wgpu::BufferAsyncError>,
    buffer: wgpu::Buffer,
    bytes_per_row: u32,
    scaled_size: (u32, u32),
    sender: Option<wasm_safe_mutex::mpsc::Sender<crate::imp::DumpMessage>>,
    remark: &'static str,
    format: DumpImageFormat,
) {
    if let Err(err) = map_result {
        logwise::error_sync!(
            "Failed to map buffer for debug capture: {err}",
            err = logwise::privacy::LogIt(&err)
        );
        return;
    }

    if scaled_size.0 == 0 || scaled_size.1 == 0 {
        logwise::warn_sync!("Skipping debug capture for zero-sized surface");
        buffer.unmap();
        return;
    }

    let pixels = {
        let mapped = buffer.slice(..).get_mapped_range();
        let mapped_slice: &[u8] = &mapped;
        let result = match format {
            DumpImageFormat::Color(surface_format) => {
                read_color_pixels(mapped_slice, bytes_per_row, scaled_size, surface_format)
            }
            DumpImageFormat::Depth16Unorm => {
                read_depth_pixels(mapped_slice, bytes_per_row, scaled_size)
            }
        };
        drop(mapped);
        result
    };
    buffer.unmap();

    let Some(pixels) = pixels else {
        return;
    };

    if let Some(sender) = sender {
        let image_info = ImageInfo::new(pixels, scaled_size.0, Some(remark.to_string()));
        if let Err(err) = sender.send_sync(crate::imp::DumpMessage::Image(image_info)) {
            logwise::error_sync!(
                "Failed to send dumped image to exfiltrate.  The receiver was likely dropped.  Error: {err}",
                err = logwise::privacy::LogIt(&err)
            );
        }
    }
}

#[cfg(feature = "exfiltrate")]
fn read_color_pixels(
    mapped: &[u8],
    bytes_per_row: u32,
    scaled_size: (u32, u32),
    surface_format: TextureFormat,
) -> Option<Vec<RGBA8>> {
    let width = scaled_size.0 as usize;
    let height = scaled_size.1 as usize;
    let stride = bytes_per_row as usize;
    let row_bytes = width * 4;
    if stride < row_bytes {
        logwise::error_sync!(
            "Row stride smaller than expected pixel data for framebuffer dump",
            stride = stride,
            expected = row_bytes
        );
        return None;
    }

    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        let offset = row * stride;
        let row_slice = &mapped[offset..offset + row_bytes];
        match surface_format {
            TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
                for chunk in row_slice.chunks_exact(4) {
                    pixels.push(RGBA8::new(chunk[2], chunk[1], chunk[0], chunk[3]));
                }
            }
            TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => {
                for chunk in row_slice.chunks_exact(4) {
                    pixels.push(RGBA8::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                }
            }
            _ => {
                logwise::error_sync!(
                    "Unsupported texture format for framebuffer dump",
                    format = logwise::privacy::LogIt(&surface_format)
                );
                return None;
            }
        }
    }

    Some(pixels)
}

#[cfg(feature = "exfiltrate")]
fn read_depth_pixels(
    mapped: &[u8],
    bytes_per_row: u32,
    scaled_size: (u32, u32),
) -> Option<Vec<RGBA8>> {
    let width = scaled_size.0 as usize;
    let height = scaled_size.1 as usize;
    let stride = bytes_per_row as usize;
    let row_bytes = width * 2;
    if stride < row_bytes {
        logwise::error_sync!(
            "Row stride smaller than expected depth data",
            stride = stride,
            expected = row_bytes
        );
        return None;
    }

    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        let offset = row * stride;
        let row_slice = &mapped[offset..offset + row_bytes];
        for chunk in row_slice.chunks_exact(2) {
            let depth = u16::from_le_bytes([chunk[0], chunk[1]]);
            let normalized = (depth >> 8) as u8;
            pixels.push(RGBA8::new(normalized, normalized, normalized, 255));
        }
    }

    Some(pixels)
}

#[derive(Debug)]
pub struct PortInternal {
    pub engine: Arc<crate::images::Engine>,
    pub pass_config: RenderInput<PassConfig>,
    pub prepared_passes: Vec<PreparedPass>,
    pub view: crate::images::view::View,
    pub port_reporter_send: PortReporterSend,
    pub frame: u32,
    pub scaled_size: RenderInput<Option<(u32, u32)>>,
    pub camera_buffer: Buffer<CameraProjection>,
    pub camera: Camera,
    pub mipmapped_sampler: WgpuCell<wgpu::Sampler>,
    #[cfg(feature = "exfiltrate")]
    pub next_frame_dump_oneshot: Option<wasm_safe_mutex::mpsc::Sender<crate::imp::DumpMessage>>,
    pub surface_texture_usage: RenderInput<wgpu::TextureUsages>,
}

impl PortInternal {
    pub async fn new(
        engine: &Arc<crate::images::Engine>,
        view: crate::images::view::View,
        camera: Camera,
        port_reporter_send: PortReporterSend,
    ) -> Result<Self, Error> {
        //create camera buffer
        let camera_buffer = Buffer::new(
            engine.bound_device().clone(),
            1,
            GPUBufferUsage::VertexShaderRead,
            "Camera",
            |_initialize| {
                let projection = camera.copy_projection_and_clear_dirty_bit();
                CameraProjection {
                    projection: [
                        *projection.matrix().columns()[0].x(),
                        *projection.matrix().columns()[0].y(),
                        *projection.matrix().columns()[0].z(),
                        *projection.matrix().columns()[0].w(),
                        *projection.matrix().columns()[1].x(),
                        *projection.matrix().columns()[1].y(),
                        *projection.matrix().columns()[1].z(),
                        *projection.matrix().columns()[1].w(),
                        *projection.matrix().columns()[2].x(),
                        *projection.matrix().columns()[2].y(),
                        *projection.matrix().columns()[2].z(),
                        *projection.matrix().columns()[2].w(),
                        *projection.matrix().columns()[3].x(),
                        *projection.matrix().columns()[3].y(),
                        *projection.matrix().columns()[3].z(),
                        *projection.matrix().columns()[3].w(),
                    ],
                }
            },
        )
        .await
        .expect("Create camera buffer");
        let mipmapped_sampler = engine
            .bound_device()
            .0
            .device()
            .with(|device| {
                let s = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("mipmapped sampler"),
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    lod_min_clamp: 0.0,
                    lod_max_clamp: 14.0,
                    compare: None,
                    anisotropy_clamp: 1,
                    border_color: None,
                });
                WgpuCell::new(s)
            })
            .await;
        //find surface format
        let view_gpu_impl = view.gpu_impl.as_ref().expect("gpu_impl").surface.clone();
        let format = match view_gpu_impl {
            None => {
                logwise::info_sync!(
                    "Port surface not initialized - picking Bgra8UnormSrgb as default format"
                );
                // For test views, set a default surface format if not already set
                TextureFormat::Bgra8UnormSrgb
            }
            Some(surface) => {
                engine
                    .bound_device()
                    .0
                    .adapter()
                    .with(move |adapter| {
                        let capabilities =
                            surface.assume(|surface| surface.get_capabilities(adapter));
                        let selected_format = capabilities.formats[0];

                        logwise::info_sync!(
                            "Available surface formats: {formats}, Selected: {selected}",
                            formats = logwise::privacy::LogIt(&capabilities.formats),
                            selected = logwise::privacy::LogIt(&selected_format)
                        );

                        selected_format
                    })
                    .await
            }
        };

        Ok(PortInternal {
            engine: engine.clone(),
            camera_buffer,
            camera,
            pass_config: RenderInput::new(PassConfig::new(format)),
            prepared_passes: Vec::new(),
            view,
            port_reporter_send,
            frame: 0,
            scaled_size: RenderInput::new(None),
            mipmapped_sampler,
            #[cfg(feature = "exfiltrate")]
            next_frame_dump_oneshot: None,
            surface_texture_usage: RenderInput::new(wgpu::TextureUsages::empty()),
        })
    }

    fn setup_depth_buffer(&self) -> (wgpu::Texture, wgpu::TextureView) {
        let depth_extra_usage = if self.needs_framedump() {
            wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::empty()
        };

        let device = self.engine.bound_device().as_ref();
        let scaled_size = self.scaled_size.requested.unwrap();
        let depth_texture = device.0.device().assume(|device| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth texture"),
                size: wgpu::Extent3d {
                    width: scaled_size.0,
                    height: scaled_size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth16Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | depth_extra_usage,
                view_formats: &[],
            })
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("depth view"),
            format: None,
            dimension: None,
            usage: None,
            aspect: Default::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        (depth_texture, depth_view)
    }

    async fn update_pass_configuration(
        &mut self,
        enable_depth: bool,
        copy_info: &mut CopyInfo<'_>,
    ) {
        if self.pass_config.is_dirty() {
            self.prepared_passes.clear();

            let device = self.engine.bound_device().as_ref();
            for descriptor in &self.pass_config.requested.pass_descriptors {
                let pipeline = PreparedPass::new(
                    device,
                    descriptor.clone(),
                    enable_depth,
                    &self.camera_buffer,
                    &self.mipmapped_sampler,
                    copy_info,
                    &self.pass_config.requested,
                )
                .await;
                self.prepared_passes.push(pipeline);
            }

            self.pass_config.mark_submitted();
        }
    }

    async fn update_camera_buffer(&mut self) {
        let camera_dirty_receiver = self.camera.dirty_receiver();
        if camera_dirty_receiver.is_dirty() {
            let projection = self.camera.copy_projection_and_clear_dirty_bit();
            let camera_projection = CameraProjection {
                projection: [
                    *projection.matrix().columns()[0].x(),
                    *projection.matrix().columns()[0].y(),
                    *projection.matrix().columns()[0].z(),
                    *projection.matrix().columns()[0].w(),
                    *projection.matrix().columns()[1].x(),
                    *projection.matrix().columns()[1].y(),
                    *projection.matrix().columns()[1].z(),
                    *projection.matrix().columns()[1].w(),
                    *projection.matrix().columns()[2].x(),
                    *projection.matrix().columns()[2].y(),
                    *projection.matrix().columns()[2].z(),
                    *projection.matrix().columns()[2].w(),
                    *projection.matrix().columns()[3].x(),
                    *projection.matrix().columns()[3].y(),
                    *projection.matrix().columns()[3].z(),
                    *projection.matrix().columns()[3].w(),
                ],
            };
            let mut write_guard = self.camera_buffer.access_write().await;
            write_guard.write(&[camera_projection], 0);
        }
    }

    fn needs_framedump(&self) -> bool {
        #[cfg(feature = "exfiltrate")]
        {
            self.next_frame_dump_oneshot.is_some()
        }
        #[cfg(not(feature = "exfiltrate"))]
        {
            false
        }
    }

    #[cfg(feature = "exfiltrate")]
    fn setup_debug_framebuffer_capture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame_texture: &wgpu::Texture,
        depth_texture: &wgpu::Texture,
    ) -> Option<DebugCaptureData> {
        if !self.needs_framedump() {
            return None;
        }

        let device = self.engine.bound_device().as_ref();
        let scaled_size = self.scaled_size.requested.unwrap();

        let wgpu_bytes_per_row_256 = (scaled_size.0 * 4)
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            .unwrap()
            .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .unwrap();

        let buf = device.0.device().assume(|device| {
            device.create_buffer(&BufferDescriptor {
                label: "dump framebuffer".into(),
                size: (scaled_size.1 * wgpu_bytes_per_row_256) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: frame_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(wgpu_bytes_per_row_256),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: scaled_size.0,
                height: scaled_size.1,
                depth_or_array_layers: 1,
            },
        );

        let depth_wgpu_bytes_per_row_256 = (scaled_size.0 * 2)
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            .unwrap()
            .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .unwrap();

        let depth_buf = device.0.device().assume(|device| {
            device.create_buffer(&BufferDescriptor {
                label: "dump depth buffer".into(),
                size: (scaled_size.1 * depth_wgpu_bytes_per_row_256) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: depth_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &depth_buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(depth_wgpu_bytes_per_row_256),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: scaled_size.0,
                height: scaled_size.1,
                depth_or_array_layers: 1,
            },
        );

        Some(DebugCaptureData {
            dump_buf: buf,
            dump_buff_bytes_per_row: wgpu_bytes_per_row_256,
            depth_dump_buf: depth_buf,
            depth_dump_buff_bytes_per_row: depth_wgpu_bytes_per_row_256,
        })
    }

    #[cfg(feature = "exfiltrate")]
    fn submit_and_present_frame(
        &mut self,
        encoder: wgpu::CommandEncoder,
        frame: Option<wgpu::SurfaceTexture>,
        frame_bind_groups: Vec<BindGroupGuard>,
        frame_acquired_guards: Vec<AcquiredGuards>,
        frame_guard: std::sync::Arc<crate::images::port::FrameGuard>,
        debug_capture: Option<DebugCaptureData>,
    ) {
        logwise::trace_sync!("submit_and_present_frame");
        let device = self.engine.bound_device().as_ref();
        let encoded = encoder.finish();

        let frame_guard_for_callback = frame_guard.clone();
        let callback_guard = frame_guard_for_callback.clone();
        //this closure requires Send but I don't think we actually do on wgpu
        let frame_acquired_guards = WgpuCell::new(frame_acquired_guards);

        device.0.queue().assume(|queue| {
            queue.on_submitted_work_done(move || {
                //at runtime, on non-wasm32 platforms, this is polled
                //from a different thread
                std::mem::drop(frame_bind_groups);
                std::mem::drop(frame_acquired_guards);
                callback_guard.mark_gpu_complete();
            });
            queue.submit(std::iter::once(encoded));
        });
        logwise::trace_sync!("submitted");
        if let Some(f) = frame {
            f.present();
        }
        logwise::trace_sync!("presented");

        self.frame += 1;

        #[cfg(feature = "exfiltrate")]
        if let Some(debug_capture) = debug_capture.as_ref() {
            let has_depth = self.pass_config.requested.enable_depth;
            let expected_count = if has_depth { 2 } else { 1 };

            if let Some(sender) = &self.next_frame_dump_oneshot {
                let _ = sender.send_sync(crate::imp::DumpMessage::Expect(expected_count));
            }

            let color_buffer = debug_capture.dump_buf.clone();
            let bytes_per_row = debug_capture.dump_buff_bytes_per_row;
            let scaled_size = self.scaled_size.requested.unwrap();
            let sender = self.next_frame_dump_oneshot.clone();
            let surface_format = self.pass_config.requested.surface_format;
            color_buffer
                .clone()
                .map_async(wgpu::MapMode::Read, .., move |result| {
                    dump_image(
                        result,
                        color_buffer,
                        bytes_per_row,
                        scaled_size,
                        sender,
                        "Color buffer",
                        DumpImageFormat::Color(surface_format),
                    )
                });
            //for map_async to work, we need to combine with needs_poll, maybe others?
            device.0.set_needs_poll();

            if has_depth {
                let bytes_per_row = debug_capture.depth_dump_buff_bytes_per_row;
                let depth_buffer = debug_capture.depth_dump_buf.clone();
                let scaled_size = self.scaled_size.requested.unwrap();
                let sender = self.next_frame_dump_oneshot.clone();
                depth_buffer
                    .clone()
                    .map_async(wgpu::MapMode::Read, .., move |result| {
                        dump_image(
                            result,
                            depth_buffer,
                            bytes_per_row,
                            scaled_size,
                            sender,
                            "depth buffer",
                            DumpImageFormat::Depth16Unorm,
                        )
                    });
                //for map_async to work, we need to combine with needs_poll, maybe others?
                device.0.set_needs_poll()
            }
        }

        frame_guard_for_callback.mark_cpu_complete();
        logwise::trace_sync!("submit_and_present_frame done");
    }

    #[cfg(not(feature = "exfiltrate"))]
    fn submit_and_present_frame(
        &mut self,
        encoder: wgpu::CommandEncoder,
        frame: Option<wgpu::SurfaceTexture>,
        frame_bind_groups: Vec<BindGroupGuard>,
        frame_acquired_guards: Vec<AcquiredGuards>,
        frame_guard: std::sync::Arc<crate::images::port::FrameGuard>,
    ) {
        logwise::trace_sync!("submit_and_present_frame");
        let device = self.engine.bound_device().as_ref();
        let encoded = encoder.finish();

        let frame_guard_for_callback = frame_guard.clone();
        let callback_guard = frame_guard_for_callback.clone();
        //this closure requires Send but I don't think we actually do on wgpu
        let frame_acquired_guards = WgpuCell::new(frame_acquired_guards);

        device.0.queue().assume(|queue| {
            queue.on_submitted_work_done(move || {
                //at runtime, on non-wasm32 platforms, this is polled
                //from a different thread
                std::mem::drop(frame_bind_groups);
                std::mem::drop(frame_acquired_guards);
                callback_guard.mark_gpu_complete();
            });
            queue.submit(std::iter::once(encoded));
        });
        logwise::trace_sync!("submitted");

        if let Some(f) = frame {
            f.present();
        }
        logwise::trace_sync!("presented");

        self.frame += 1;

        frame_guard_for_callback.mark_cpu_complete();
        logwise::trace_sync!("submit_and_present_frame done");
    }

    pub async fn add_fixed_pass(&mut self, descriptor: PassDescriptor) {
        let mut new_config = self.pass_config.requested.clone();
        new_config.add_pass(descriptor);
        self.pass_config.update(new_config);
    }
    pub async fn begin_render_frame_internal(&mut self) -> (CommandEncoder, FrameGuard) {
        let frame_guard = self.port_reporter_send.create_frame_guard(self.frame);

        //basically we want to bunch up all our awaits here,
        //so we don't interrupt the frame

        self.update_camera_buffer().await;
        let mut encoder = {
            let device = self.engine.bound_device().as_ref();
            device.0.device().assume(|device| {
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("wgpu port"),
                })
            })
        };
        // First, recreate acquired guards for all prepared passes to ensure buffers are up to date
        {
            let mut copy_info = CopyInfo {
                command_encoder: &mut encoder,
            };
            for prepared_pass in &mut self.prepared_passes {
                prepared_pass
                    .recreate_acquired_guards(&self.camera_buffer, &mut copy_info)
                    .await
            }
        }
        let enable_depth = self.pass_config.requested.enable_depth;

        // Then update pass configuration and camera buffer (which creates bind groups with fresh buffer data)
        {
            let mut copy_info = CopyInfo {
                command_encoder: &mut encoder,
            };
            self.update_pass_configuration(enable_depth, &mut copy_info)
                .await;
        }
        (encoder, frame_guard)
    }

    //a synchronous function to finish the render frame
    pub fn finish_render_frame(
        &mut self,
        mut encoder: wgpu::CommandEncoder,
        frame_guard: crate::images::port::FrameGuard,
        fast_size_scale: (u16, u16, f64),
    ) {
        logwise::trace_sync!("finish_render_frame begin");
        // Setup frame reporting and surface configuration
        let current_scaled_size = (
            (fast_size_scale.0 as f64 * fast_size_scale.2) as u32,
            (fast_size_scale.1 as f64 * fast_size_scale.2) as u32,
        );
        self.scaled_size.update(Some(current_scaled_size));
        //move dump_frame from global object to struct
        #[cfg(feature = "exfiltrate")]
        DUMP_NEXT_FRAME.with_mut_sync(|e| self.next_frame_dump_oneshot = e.take());

        let surface = self.view.gpu_impl.as_ref().unwrap().surface.as_ref();
        match surface {
            None => {
                logwise::debuginternal_sync!("Port surface not initialized");
            }
            Some(surface) => {
                let extra_usage = if self.needs_framedump() {
                    wgpu::TextureUsages::COPY_SRC
                } else {
                    wgpu::TextureUsages::empty()
                };
                self.surface_texture_usage.update(extra_usage);
                if self.scaled_size.is_dirty() || self.surface_texture_usage.is_dirty() {
                    logwise::trace_sync!("Configuring surface for new size");
                    let device = self.engine.bound_device().as_ref();
                    let scaled_size = self.scaled_size.requested.unwrap();

                    // Update the surface format to match what we'll actually use
                    device.0.device().assume(|device| {
                        surface.assume(|surface| {
                            //On WebGPU we're sometimes forbidden to use srgb formats
                            //so we need to use with a view
                            let mut view_formats = Vec::new();
                            if !self.pass_config.requested.surface_format.is_srgb() {
                                view_formats.push(TextureFormat::Bgra8UnormSrgb)
                            }
                            logwise::trace_sync!(
                                "Size is {width} x {height}",
                                width = scaled_size.0,
                                height = scaled_size.1
                            );
                            logwise::trace_sync!(
                                "Format is {format}",
                                format = logwise::privacy::LogIt(
                                    &self.pass_config.requested.surface_format
                                )
                            );
                            logwise::trace_sync!("surface.configure");
                            surface.configure(
                                device,
                                &wgpu::SurfaceConfiguration {
                                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | extra_usage,
                                    format: self.pass_config.requested.surface_format,
                                    width: scaled_size.0,
                                    height: scaled_size.1,
                                    present_mode: wgpu::PresentMode::Fifo,
                                    desired_maximum_frame_latency: 1,
                                    alpha_mode: CompositeAlphaMode::Opaque,
                                    view_formats,
                                },
                            );
                            logwise::trace_sync!("surface.configure complete");
                        });
                    });
                    self.scaled_size.mark_submitted();
                    self.surface_texture_usage.mark_submitted();
                }
            }
        }
        logwise::trace_sync!("wgpu::port::A");

        // Create per-frame resources
        let wgpu_view;
        let frame;
        let color_attachment;
        #[cfg(feature = "exfiltrate")]
        let frame_texture;
        match surface {
            None => {
                let scaled_size = self.scaled_size.requested.unwrap();
                let device = self.engine.bound_device().as_ref();
                let texture = device.0.device().assume(|device| {
                    device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("dummy texture"),
                        size: wgpu::Extent3d {
                            width: scaled_size.0,
                            height: scaled_size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.pass_config.requested.surface_format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                });
                wgpu_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("dummy view"),
                    format: None,
                    dimension: None,
                    usage: None,
                    aspect: Default::default(),
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                });
                frame = None;
                #[cfg(feature = "exfiltrate")]
                {
                    frame_texture = texture;
                }
                color_attachment = wgpu::RenderPassColorAttachment {
                    view: &wgpu_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Default::default(),
                };
            }
            Some(surface) => {
                logwise::trace_sync!("wgpu::port::A0");
                let surface_texture = surface
                    .assume(|surface| surface.get_current_texture())
                    .expect("Acquire swapchain texture");
                #[cfg(feature = "exfiltrate")]
                {
                    frame_texture = surface_texture.texture.clone();
                }
                logwise::trace_sync!("wgpu::port::A1");

                frame = Some(surface_texture);
                let format = if self.pass_config.requested.surface_format.is_srgb() {
                    None
                } else {
                    // If the surface format is not sRGB, we need to use a view with sRGB format
                    Some(TextureFormat::Bgra8UnormSrgb)
                };
                logwise::trace_sync!("wgpu::port::A2");

                let descriptor = wgpu::TextureViewDescriptor {
                    label: "surface texture view".into(),
                    format,
                    dimension: None,
                    usage: None,
                    aspect: Default::default(),
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                };

                wgpu_view = frame.as_ref().unwrap().texture.create_view(&descriptor);
                logwise::trace_sync!("wgpu::port::A3");
                color_attachment = wgpu::RenderPassColorAttachment {
                    view: &wgpu_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                };
            }
        };
        logwise::trace_sync!("port::A.5");
        // Setup depth buffer
        #[cfg(feature = "exfiltrate")]
        let (depth_texture, depth_view) = self.setup_depth_buffer();
        #[cfg(not(feature = "exfiltrate"))]
        let (_depth_texture, depth_view) = self.setup_depth_buffer();
        // Execute render passes
        let depth_store = if self.needs_framedump() {
            StoreOp::Store
        } else {
            StoreOp::Discard
        };
        let depth_stencil_attachment = if self.prepared_passes.iter().any(|e| e.depth_pass) {
            Some(RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: depth_store,
                }),
                stencil_ops: None,
            })
        } else {
            None
        };
        logwise::trace_sync!("wgpu::port::B");

        // Extract bind groups and acquired guards from prepared passes
        let mut frame_bind_groups = Vec::new();
        let mut frame_acquired_guards = Vec::new();
        for prepared in &mut self.prepared_passes {
            frame_bind_groups.push(prepared.bind_group_guard.clone());
            if let Some(acquired) = prepared.acquired_guards.take() {
                frame_acquired_guards.push(acquired);
            }
        }

        logwise::trace_sync!("wgpu::port::C");
        // Encode render passes
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Port render"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for (p, prepared) in self.prepared_passes.iter().enumerate() {
            render_pass.push_debug_group(prepared.pass_descriptor.name());
            prepared
                .pipeline
                .assume(|pipeline| render_pass.set_pipeline(pipeline));

            let bind_group = &frame_bind_groups[p];
            bind_group.bind_group.assume(|bind_group| {
                render_pass.set_bind_group(0, bind_group, &[]);
            });

            for (v, buffer) in &bind_group.vertex_buffers {
                buffer.assume(|buffer| {
                    render_pass.set_vertex_buffer(*v, buffer.slice(..));
                })
            }
            for (v, buffer) in &bind_group.dynamic_vertex_buffers {
                buffer.underlying_guard.as_imp().buffer().assume(|buffer| {
                    let buffer_slice = buffer.slice(..);
                    render_pass.set_vertex_buffer(*v, buffer_slice);
                });
            }
            if let Some(buffer) = &bind_group.index_buffer {
                buffer.assume(|buffer| {
                    render_pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint16);
                });
                render_pass.draw_indexed(0..prepared.vertex_count, 0, 0..1);
            } else {
                render_pass.draw(0..prepared.vertex_count, 0..1);
            }
            render_pass.pop_debug_group();
        }

        std::mem::drop(render_pass);
        logwise::trace_sync!("wgpu::port::D");

        // Submit and present frame
        let frame_guard_arc = std::sync::Arc::new(frame_guard);
        logwise::trace_sync!("wgpu::port::E");

        #[cfg(feature = "exfiltrate")]
        {
            // Setup debug framebuffer capture
            let debug_capture =
                self.setup_debug_framebuffer_capture(&mut encoder, &frame_texture, &depth_texture);

            self.submit_and_present_frame(
                encoder,
                frame,
                frame_bind_groups,
                frame_acquired_guards,
                frame_guard_arc,
                debug_capture,
            );
        }

        #[cfg(not(feature = "exfiltrate"))]
        {
            self.submit_and_present_frame(
                encoder,
                frame,
                frame_bind_groups,
                frame_acquired_guards,
                frame_guard_arc,
            );
        }
        logwise::trace_sync!("finish_render_frame end");
    }

    pub async fn render_frame(mut self) -> Self {
        logwise::debuginternal_sync!("Rendering frame...");
        smuggle_async("render_frame".to_string(), || async move {
            let (encoder, frame_guard) = self.begin_render_frame_internal().await;
            let size_scale = self.view.size_scale().await;

            crate::images::request_animation_frame::request_animation_frame_async(move || {
                self.finish_render_frame(encoder, frame_guard, size_scale);
                self
            })
            .await
        })
        .await
    }
}
