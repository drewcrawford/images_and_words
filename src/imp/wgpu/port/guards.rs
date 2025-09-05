// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::bind_style::BindTarget;
use crate::bindings::forward::dynamic::buffer::{Buffer, SomeRenderSide};
use crate::bindings::sampler::SamplerType;
use crate::imp;
use crate::imp::wgpu::cell::WgpuCell;
use crate::stable_address_vec::StableAddressVec;
use std::collections::HashMap;
use std::num::NonZero;
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupEntry, BindingResource, BufferBinding};

use super::types::CameraProjection;

/**
Guards and resources acquired during the copy phase.
*/
#[derive(Debug)]
pub struct AcquiredGuards {
    // Combined buffer and vertex buffer guards, keyed by bind index
    pub buffer_guards: HashMap<u32, Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>>,
    pub _copy_guards: Vec<crate::bindings::resource_tracking::GPUGuard<imp::MappableBuffer2>>,
    // Texture guards, keyed by bind index
    pub texture_guards:
        HashMap<u32, Arc<crate::bindings::forward::dynamic::frame_texture::GPUAccess>>,
    // Texture copy guards that need to be kept alive during GPU operations
    pub _texture_copy_guards:
        Vec<Box<dyn crate::bindings::forward::dynamic::frame_texture::DynDirtyGuard>>,
    pub camera_guard: Option<Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>>,
}

impl AcquiredGuards {
    /// Acquires GPU buffers and performs copy operations for dynamic resources.
    /// Returns guards that must be kept alive and copy guards that can be disposed after copying.
    pub async fn new(
        bind_style: &crate::bindings::bind_style::BindStyle,
        copy_info: &mut imp::CopyInfo<'_>,
        camera_buffer: &Buffer<CameraProjection>,
    ) -> Self {
        logwise::trace_sync!("AcquiredGuards::new");
        let mut buffer_guards = HashMap::new();
        let mut copy_guards = Vec::new();
        let mut texture_guards = HashMap::new();
        let mut texture_copy_guards = Vec::new();

        // Handle dynamic buffers, dynamic vertex buffers, and dynamic textures in a single pass
        let mut camera_guard = None;
        for (bind_index, info) in &bind_style.binds {
            logwise::trace_sync!(
                "Acquiring target {bind_index} {info}",
                bind_index = *bind_index,
                info = logwise::privacy::LogIt(info)
            );
            match &info.target {
                BindTarget::DynamicBuffer(buf) => {
                    // Safety: Keep the guard alive
                    let mut gpu_access = unsafe { buf.imp.acquire_gpu_buffer() };

                    // Handle the copy if there's a dirty guard
                    if let Some(mut dirty_guard) = gpu_access.take_dirty_guard() {
                        // Get the source buffer from the dirty guard
                        let source: &mut imp::MappableBuffer2 = &mut dirty_guard;

                        // Perform the copy operation using the new GPUableBuffer2 method
                        gpu_access
                            .underlying_guard
                            .as_imp()
                            .copy_from_mappable_buffer2(source, copy_info.command_encoder)
                            .await;
                        copy_guards.push(dirty_guard);
                    }

                    buffer_guards.insert(*bind_index, Arc::new(gpu_access));
                }

                BindTarget::Camera => {
                    // Safety: Keep the guard alive
                    let mut gpu_access =
                        unsafe { camera_buffer.render_side().acquire_gpu_buffer() };

                    // Handle the copy if there's a dirty guard
                    if let Some(mut dirty_guard) = gpu_access.take_dirty_guard() {
                        // Get the source buffer from the dirty guard
                        let source: &mut imp::MappableBuffer2 = &mut dirty_guard;

                        // Perform the copy operation using the new GPUableBuffer2 method
                        gpu_access
                            .underlying_guard
                            .as_imp()
                            .copy_from_mappable_buffer2(source, copy_info.command_encoder)
                            .await;
                        copy_guards.push(dirty_guard);
                    }
                    camera_guard = Some(Arc::new(gpu_access));
                }

                BindTarget::DynamicVB(_layout, render_side) => {
                    // Safety: guard kept alive
                    let mut gpu_access = unsafe { render_side.imp.acquire_gpu_buffer() };

                    // Handle the copy if there's a dirty guard
                    if let Some(mut dirty_guard) = gpu_access.take_dirty_guard() {
                        // Get the source buffer from the dirty guard
                        let source: &mut imp::MappableBuffer2 = &mut dirty_guard;

                        // Perform the copy operation using the new GPUableBuffer2 method
                        gpu_access
                            .underlying_guard
                            .as_imp()
                            .copy_from_mappable_buffer2(source, copy_info.command_encoder)
                            .await;
                        copy_guards.push(dirty_guard);
                    }

                    buffer_guards.insert(*bind_index, Arc::new(gpu_access));
                }
                BindTarget::DynamicTexture(texture) => {
                    // Safety: keep the guard alive
                    let mut gpu_access = unsafe { texture.acquire_gpu_texture() };

                    if let Some(mut dirty_guard) = gpu_access.take_dirty_guard() {
                        // Get the source texture from the dirty guard
                        let source: &mut dyn imp::MappableTextureWrapped = dirty_guard.as_imp();

                        // Perform the copy operation using the new GPUableTexture2 method
                        //safety: guards are live
                        unsafe { gpu_access.as_imp().copy_from_mappable(source, copy_info) }
                            .await
                            .unwrap();
                        texture_copy_guards.push(dirty_guard);
                    }

                    texture_guards.insert(*bind_index, Arc::new(gpu_access));
                }

                _ => {} // Other targets handled later
            }
        }

        AcquiredGuards {
            buffer_guards,
            _copy_guards: copy_guards,
            camera_guard,
            texture_guards,
            _texture_copy_guards: texture_copy_guards,
        }
    }
}

/**
Wrapper type that contains the bind group
and all guards that are needed to keep the resources alive.
*/
#[derive(Debug, Clone)]
pub struct BindGroupGuard {
    pub bind_group: WgpuCell<BindGroup>,
    #[allow(dead_code)] // guards keep resources alive during GPU execution
    pub guards: Vec<Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>>,
    pub _guards_textures: Vec<Arc<crate::bindings::forward::dynamic::frame_texture::GPUAccess>>,
    pub vertex_buffers: Vec<(u32, WgpuCell<wgpu::Buffer>)>,
    pub dynamic_vertex_buffers: Vec<(
        u32,
        Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>,
    )>,
    pub index_buffer: Option<WgpuCell<wgpu::Buffer>>,
}

impl BindGroupGuard {
    /// Creates a BindGroupGuard using pre-acquired guards from acquire_and_copy_guards.
    fn new_from_guards(
        bind_device: &crate::images::BoundDevice,
        bind_style: &crate::bindings::bind_style::BindStyle,
        name: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        mipmapped_sampler: &WgpuCell<wgpu::Sampler>,
        acquired_guards: &mut AcquiredGuards,
        _copy_info: &mut imp::CopyInfo,
    ) -> Self {
        let mut entries = Vec::new();
        //these need to be kept alive during GPU execution
        let build_dynamic_buffers_gpu = StableAddressVec::with_capactiy(5);
        let build_dynamic_textures_gpu = StableAddressVec::with_capactiy(5);

        //these are only used for the bind group
        let build_static_texture_views = StableAddressVec::with_capactiy(5);
        let build_static_buffers = StableAddressVec::with_capactiy(5);
        let build_dynamic_texture_views = StableAddressVec::with_capactiy(5);

        let clone_buffers = StableAddressVec::with_capactiy(5);

        let camera_buffers = StableAddressVec::with_capactiy(5);

        let sampler_guards = StableAddressVec::with_capactiy(5);

        for (pass_index, info) in &bind_style.binds {
            let resource = match &info.target {
                BindTarget::DynamicBuffer(buf) => {
                    // Remove the guard from the acquired guards map
                    let build_buffer = acquired_guards
                        .buffer_guards
                        .remove(pass_index)
                        .expect("Dynamic buffer guard should be in acquired_guards");
                    let guard = build_dynamic_buffers_gpu.push(build_buffer);
                    let clone_buffer = clone_buffers.push(
                        guard
                            .underlying_guard
                            .as_imp()
                            .buffer()
                            .clone()
                            .assume(|wgpu_guard| wgpu_guard.clone()),
                    );
                    BindingResource::Buffer(BufferBinding {
                        buffer: clone_buffer,
                        offset: 0,
                        size: Some(NonZero::new(buf.byte_size as u64).unwrap()),
                    })
                }
                BindTarget::StaticBuffer(buf) => {
                    let gpu_buffer = buf.buffer().lock();
                    let stored_buffer = build_static_buffers.push(gpu_buffer);
                    BindingResource::Buffer(BufferBinding {
                        buffer: stored_buffer,
                        offset: 0,
                        size: Some(NonZero::new(stored_buffer.size()).unwrap()),
                    })
                }
                BindTarget::Camera => {
                    let gpu_buffer = acquired_guards.camera_guard.as_ref().unwrap().clone();
                    let stored_buffer = build_dynamic_buffers_gpu.push(gpu_buffer);
                    let camera_clone = stored_buffer
                        .underlying_guard
                        .as_imp()
                        .buffer()
                        .assume(|e| e.clone());
                    let camera_clone = camera_buffers.push(camera_clone);
                    BindingResource::Buffer(BufferBinding {
                        buffer: camera_clone,
                        offset: 0,
                        size: Some(
                            NonZero::new(std::mem::size_of::<CameraProjection>() as u64).unwrap(),
                        ),
                    })
                }
                BindTarget::FrameCounter => {
                    todo!()
                }
                BindTarget::StaticTexture(texture_render_side, _sampler_type) => {
                    let view = texture_render_side.texture.assume(|texture| {
                        texture.create_view(&wgpu::TextureViewDescriptor {
                            label: None,
                            format: None,
                            dimension: None,
                            usage: None,
                            aspect: Default::default(),
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: 0,
                            array_layer_count: None,
                        })
                    });
                    let view = build_static_texture_views.push(view);
                    BindingResource::TextureView(view)
                }
                BindTarget::DynamicTexture(_texture) => {
                    // Remove the guard from the acquired texture guards map
                    let gpu_access = acquired_guards
                        .texture_guards
                        .remove(pass_index)
                        .expect("Dynamic texture guard should be in acquired_guards");

                    // Store the guard
                    let guard = build_dynamic_textures_gpu.push(gpu_access);

                    // Use the render_side from GPUAccess
                    let view = guard.render_side.texture.assume(|texture| {
                        texture.create_view(&wgpu::TextureViewDescriptor {
                            label: None,
                            format: None,
                            dimension: None,
                            usage: None,
                            aspect: Default::default(),
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: 0,
                            array_layer_count: None,
                        })
                    });
                    let view = build_dynamic_texture_views.push(view);
                    BindingResource::TextureView(view)
                }
                BindTarget::Sampler(sampler) => match sampler {
                    SamplerType::Mipmapped => {
                        let guard = sampler_guards.push(mipmapped_sampler.assume(|e| e.clone()));
                        BindingResource::Sampler(guard)
                    }
                },
                BindTarget::VB(..) | BindTarget::DynamicVB(..) => {
                    continue; //not considered as a binding
                }
            };

            let entry = BindGroupEntry {
                binding: *pass_index,
                resource,
            };
            entries.push(entry);
        }

        let bind_group = bind_device.0.device().assume(|device| {
            WgpuCell::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(name),
                layout: bind_group_layout,
                entries: entries.as_slice(),
            }))
        });

        //find vertex buffers
        let mut vertex_buffers = Vec::new();
        let mut dynamic_vertex_buffers = Vec::new();
        for (b, buffer) in &bind_style.binds {
            match &buffer.target {
                BindTarget::StaticBuffer(_)
                | BindTarget::DynamicBuffer(_)
                | BindTarget::Camera
                | BindTarget::FrameCounter
                | BindTarget::DynamicTexture(_)
                | BindTarget::StaticTexture(..)
                | BindTarget::Sampler(_) => {}
                BindTarget::VB(_layout, render_side) => {
                    let buffer = render_side.buffer();
                    vertex_buffers.push((*b, buffer.clone()));
                }
                BindTarget::DynamicVB(..) => {
                    // Remove the guard from the acquired guards map
                    let guard = acquired_guards
                        .buffer_guards
                        .remove(b)
                        .expect("Dynamic vertex buffer guard should be in acquired_guards");
                    dynamic_vertex_buffers.push((*b, guard));
                }
            }
        }

        let index_buffer = if let Some(buffer) = &bind_style.index_buffer {
            let buffer = buffer.buffer().clone();
            Some(buffer)
        } else {
            None
        };

        // Convert StableAddressVec to Vec
        let gpu_guard_buffers = build_dynamic_buffers_gpu.into_vec();
        let gpu_guard_texture_views = build_dynamic_textures_gpu.into_vec();
        // dynamic_vertex_buffers is already in the correct format

        BindGroupGuard {
            bind_group,
            guards: gpu_guard_buffers,
            _guards_textures: gpu_guard_texture_views,
            vertex_buffers,
            dynamic_vertex_buffers,
            index_buffer,
        }
    }

    pub async fn new(
        bind_device: &crate::images::BoundDevice,
        bind_style: &crate::bindings::bind_style::BindStyle,
        name: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        camera_buffer: &Buffer<CameraProjection>,
        mipmapped_sampler: &WgpuCell<wgpu::Sampler>,
        copy_info: &mut imp::CopyInfo<'_>,
    ) -> (Self, AcquiredGuards) {
        // First acquire guards and perform copies
        let mut acquired_guards = AcquiredGuards::new(bind_style, copy_info, camera_buffer).await;

        // Then create the bind group using the acquired guards
        let s = Self::new_from_guards(
            bind_device,
            bind_style,
            name,
            bind_group_layout,
            mipmapped_sampler,
            &mut acquired_guards,
            copy_info,
        );
        (s, acquired_guards)
    }
}
