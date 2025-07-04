// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::bind_style::BindTarget;
use crate::bindings::forward::dynamic::buffer::CRepr;
use crate::bindings::forward::dynamic::buffer::{Buffer, SomeRenderSide};
use crate::bindings::sampler::SamplerType;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{DrawCommand, PassDescriptor};
use crate::images::vertex_layout::VertexFieldType;
use crate::imp;
use crate::imp::wgpu::buffer::StorageType;
use crate::imp::wgpu::cell::WgpuCell;
use crate::imp::wgpu::context::smuggle_async;
use crate::imp::{CopyInfo, Error};
use crate::stable_address_vec::StableAddressVec;
use send_cells::send_cell::SendCell;
use std::collections::HashMap;
use std::num::NonZero;
use std::sync::Arc;
use wgpu::wgt::BufferDescriptor;
use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    BufferBinding, BufferBindingType, BufferSize, Color, ColorTargetState, CompareFunction,
    CompositeAlphaMode, DepthStencilState, Face, FrontFace, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    RenderPassDepthStencilAttachment, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    StencilFaceState, StencilState, StoreOp, TextureFormat, TextureSampleType,
    TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode,
};

#[repr(C)]
#[derive(Debug)]
pub struct CameraProjection {
    pub projection: [f32; 16],
}

unsafe impl CRepr for CameraProjection {}

#[derive(Debug, Clone, PartialEq)]
struct PassConfig {
    pass_descriptors: Vec<PassDescriptor>,
    enable_depth: bool,
    surface_format: Option<TextureFormat>,
}

impl PassConfig {
    fn new() -> Self {
        PassConfig {
            pass_descriptors: Vec::new(),
            enable_depth: false,
            surface_format: None,
        }
    }

    fn add_pass(&mut self, descriptor: PassDescriptor) {
        if descriptor.depth {
            self.enable_depth = true;
        }
        self.pass_descriptors.push(descriptor);
    }
}

/**
A render input is a pair of requested and submitted values.

The *requested* value contains the latest value that the external system (e.g. a game loop) has requested to be rendered.
The *submitted* value contains the value that has been submitted to the GPU for rendering.
*/
#[derive(Debug)]
struct RenderInput<T> {
    requested: T,
    submitted: Option<T>,
}

impl<T> RenderInput<T> {
    fn new(requested: T) -> Self {
        RenderInput {
            requested,
            submitted: None,
        }
    }
    fn update(&mut self, requested: T) {
        self.requested = requested;
    }
    fn is_dirty(&self) -> bool
    where
        T: PartialEq,
    {
        match &self.submitted {
            Some(submitted) => self.requested != *submitted,
            None => true, //if not submitted, it is dirty
        }
    }
    fn mark_submitted(&mut self)
    where
        T: Clone,
    {
        self.submitted = Some(self.requested.clone());
    }
}

#[derive(Debug)]
pub struct PortInternal {
    engine: Arc<crate::images::Engine>,
    pass_config: RenderInput<PassConfig>,
    prepared_passes: Vec<PreparedPass>,
    view: crate::images::view::View,
    port_reporter_send: PortReporterSend,
    frame: u32,
    dump_framebuffer: bool, //for debugging
    scaled_size: RenderInput<Option<(u32, u32)>>,
    camera_buffer: Buffer<CameraProjection>,
    camera: Camera,
    mipmapped_sampler: WgpuCell<wgpu::Sampler>,
}

/**
A pass that is prepared to be rendered (compiled, layout calculated, etc.)
*/
#[derive(Debug)]
pub struct PreparedPass {
    pipeline: RenderPipeline,
    pass_descriptor: PassDescriptor,
    #[allow(dead_code)] //instance counts are not used yet
    instance_count: u32,
    vertex_count: u32,
    depth_pass: bool,
    bind_group_guard: BindGroupGuard,
    acquired_guards: Option<AcquiredGuards>,
}

impl PreparedPass {
    fn new(
        bind_device: &crate::images::BoundDevice,
        descriptor: PassDescriptor,
        enable_depth: bool,
        camera_buffer: &Buffer<CameraProjection>,
        mipmapped_sampler: &WgpuCell<wgpu::Sampler>,
        copy_info: &mut CopyInfo<'_>,
        pass_config: &PassConfig,
    ) -> PreparedPass {
        let mut layouts = Vec::new();

        for (pass_index, info) in &descriptor.bind_style().binds {
            let stage = match info.stage {
                crate::bindings::bind_style::Stage::Fragment => wgpu::ShaderStages::FRAGMENT,
                crate::bindings::bind_style::Stage::Vertex => wgpu::ShaderStages::VERTEX,
            };
            let binding_type = match &info.target {
                BindTarget::DynamicBuffer(imp) => {
                    //safe because we're not using the buffer
                    let storage_type = unsafe { imp.imp.unsafe_imp().storage_type() };
                    let buffer_binding_type = match storage_type {
                        StorageType::Uniform => BufferBindingType::Uniform,
                        StorageType::Storage => BufferBindingType::Storage { read_only: true },
                        StorageType::Vertex | StorageType::Index => unreachable!(),
                    };
                    BindingType::Buffer {
                        ty: buffer_binding_type,
                        has_dynamic_offset: false,
                        min_binding_size: Some(BufferSize::new(imp.element_size as u64).unwrap()),
                    }
                }
                BindTarget::StaticBuffer(imp) => {
                    let buffer_binding_type = match imp.storage_type() {
                        StorageType::Uniform => BufferBindingType::Uniform,
                        StorageType::Storage => BufferBindingType::Storage { read_only: true },
                        StorageType::Vertex | StorageType::Index => unreachable!(),
                    };

                    BindingType::Buffer {
                        ty: buffer_binding_type,
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::new(imp.buffer().assume(|b| b.size())),
                    }
                }
                BindTarget::Camera => {
                    //I guess these are implemented with buffers for now...
                    BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZero::new(64).unwrap()), //This value determined experimentally?
                    }
                }
                BindTarget::FrameCounter => {
                    //I guess these are implemented with buffers for now...
                    BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZero::new(1).unwrap()), //???
                    }
                }
                BindTarget::StaticTexture(_texture, sampler_type) => BindingType::Texture {
                    sample_type: TextureSampleType::Float {
                        filterable: sampler_type.is_some(),
                    },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                BindTarget::DynamicTexture(_texture) => {
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false }, //??
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    }
                }
                BindTarget::Sampler(_sampler) => {
                    BindingType::Sampler(SamplerBindingType::Filtering)
                }
                BindTarget::VB(..) => {
                    continue; //not considered as a binding
                }
                BindTarget::DynamicVB(..) => {
                    continue; //not considered as a binding
                }
            };
            let layout = BindGroupLayoutEntry {
                binding: *pass_index,
                visibility: stage,
                ty: binding_type,
                count: None, //not array
            };
            layouts.push(layout);
        }
        // println!("Will create bind group layout {:?}", layouts);

        let bind_group_layout = bind_device.0.device.assume(|device| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(descriptor.name()),
                entries: layouts.as_slice(),
            })
        });

        let pipeline_layout = bind_device.0.device.assume(|device| {
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(descriptor.name()),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[], //not yet supported
            })
        });

        let vertex_module = bind_device.0.device.assume(|device| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(descriptor.vertex_shader.label),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                    &descriptor.vertex_shader.wgsl_code,
                )),
            })
        });

        //calculate vertex buffers
        let mut vertex_buffers = Vec::new();
        let all_vertex_attributes = StableAddressVec::with_capactiy(5);

        for buffer in descriptor.bind_style.binds.values() {
            match &buffer.target {
                BindTarget::StaticBuffer(_)
                | BindTarget::DynamicBuffer(_)
                | BindTarget::Camera
                | BindTarget::FrameCounter
                | BindTarget::DynamicTexture(_)
                | BindTarget::StaticTexture(..)
                | BindTarget::Sampler(_) => {}
                BindTarget::VB(layout, _) | BindTarget::DynamicVB(layout, _) => {
                    let mut each_vertex_attributes = Vec::new();
                    let mut offset = 0;
                    for (f, field) in layout.fields.iter().enumerate() {
                        let attribute = VertexAttribute {
                            format: match field.r#type {
                                VertexFieldType::F32 => wgpu::VertexFormat::Float32,
                            },
                            offset,
                            shader_location: f as u32,
                        };
                        offset += field.r#type.stride() as u64;
                        each_vertex_attributes.push(attribute);
                    }
                    let strong_vertex_attributes =
                        all_vertex_attributes.push(each_vertex_attributes);
                    let layout = VertexBufferLayout {
                        array_stride: layout.element_stride() as u64,
                        step_mode: VertexStepMode::Vertex,
                        attributes: strong_vertex_attributes,
                    };
                    vertex_buffers.push(layout);
                }
            }
        }

        let vertex_state = VertexState {
            module: &vertex_module,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &vertex_buffers,
        };
        let topology = match descriptor.draw_command() {
            DrawCommand::TriangleStrip(_count) => PrimitiveTopology::TriangleStrip,
            DrawCommand::TriangleList(..) => PrimitiveTopology::TriangleList,
        };
        let vertex_count = match descriptor.draw_command {
            DrawCommand::TriangleStrip(count) => count * 3,
            DrawCommand::TriangleList(count) => count * 3,
        };
        let instance_count = match descriptor.draw_command {
            DrawCommand::TriangleStrip(..) => 1,
            DrawCommand::TriangleList(..) => 1,
        };

        let primitive_state = PrimitiveState {
            topology,
            strip_index_format: None,
            front_face: FrontFace::Ccw, //?
            cull_mode: Some(Face::Back),
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        };

        //because everything is in one render pass, we need all the depth states to match
        //enable depth if any pass wants it
        let depth_state = if enable_depth {
            Some(DepthStencilState {
                format: TextureFormat::Depth16Unorm,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: Default::default(),
            })
        } else {
            None
        };

        let multisample_state = MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        let fragment_module = bind_device.0.device.assume(|device| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(descriptor.fragment_shader.label),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                    &descriptor.fragment_shader.wgsl_code,
                )),
            })
        });
        let blend = if descriptor.alpha {
            Some(BlendState::ALPHA_BLENDING)
        } else {
            None
        };
        let color_target_state = ColorTargetState {
            format: pass_config
                .surface_format
                .unwrap_or(TextureFormat::Bgra8UnormSrgb),
            blend,
            write_mask: Default::default(),
        };
        let fragment_state = wgpu::FragmentState {
            module: &fragment_module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(color_target_state)],
        };

        let render_descriptor = RenderPipelineDescriptor {
            label: Some(descriptor.name()),
            //https://docs.rs/wgpu/24.0.1/wgpu/struct.RenderPipelineDescriptor.html
            layout: Some(&pipeline_layout),
            vertex: vertex_state,
            primitive: primitive_state,
            depth_stencil: depth_state,
            multisample: multisample_state,
            fragment: Some(fragment_state),
            multiview: None,
            cache: None, //todo, caching?
        };
        let pipeline = bind_device
            .0
            .device
            .assume(|device| device.create_render_pipeline(&render_descriptor));

        // Create the BindGroupGuard using the constructed bind_group_layout
        let (bind_group_guard, acquired_guards) = BindGroupGuard::new(
            bind_device,
            descriptor.bind_style(),
            descriptor.name(),
            &bind_group_layout,
            camera_buffer,
            mipmapped_sampler,
            copy_info,
        );
        PreparedPass {
            pipeline,
            vertex_count,
            instance_count,
            depth_pass: render_descriptor.depth_stencil.is_some(),
            pass_descriptor: descriptor.clone(),
            bind_group_guard,
            acquired_guards: Some(acquired_guards),
        }
    }

    fn recreate_acquired_guards(
        &mut self,
        camera_buffer: &Buffer<CameraProjection>,
        copy_info: &mut CopyInfo<'_>,
    ) {
        // Recreate only the acquired_guards field, leaving bind_group_guard unchanged
        let new_acquired_guards =
            AcquiredGuards::new(self.pass_descriptor.bind_style(), copy_info, camera_buffer);
        self.acquired_guards = Some(new_acquired_guards);
    }
}

/**
Guards and resources acquired during the copy phase.
*/
#[derive(Debug)]
pub struct AcquiredGuards {
    // Combined buffer and vertex buffer guards, keyed by bind index
    buffer_guards: HashMap<u32, Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>>,
    _copy_guards: Vec<crate::bindings::resource_tracking::GPUGuard<imp::MappableBuffer>>,
    // Texture guards, keyed by bind index
    texture_guards: HashMap<u32, Arc<crate::bindings::forward::dynamic::frame_texture::GPUAccess>>,
    // Texture copy guards that need to be kept alive during GPU operations
    _texture_copy_guards: Vec<Box<dyn crate::bindings::forward::dynamic::frame_texture::DynGuard>>,
    camera_guard: Option<Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>>,
}

impl AcquiredGuards {
    /// Acquires GPU buffers and performs copy operations for dynamic resources.
    /// Returns guards that must be kept alive and copy guards that can be disposed after copying.
    pub fn new(
        bind_style: &crate::bindings::bind_style::BindStyle,
        copy_info: &mut CopyInfo<'_>,
        camera_buffer: &Buffer<CameraProjection>,
    ) -> Self {
        let mut buffer_guards = HashMap::new();
        let mut copy_guards = Vec::new();
        let mut texture_guards = HashMap::new();
        let mut texture_copy_guards = Vec::new();

        // Handle dynamic buffers, dynamic vertex buffers, and dynamic textures in a single pass
        let mut camera_guard = None;
        for (bind_index, info) in &bind_style.binds {
            match &info.target {
                BindTarget::DynamicBuffer(buf) => {
                    // Safety: Keep the guard alive
                    let mut gpu_access = unsafe { buf.imp.acquire_gpu_buffer() };

                    // Handle the copy if there's a dirty guard
                    if let Some(mut dirty_guard) = gpu_access.take_dirty_guard() {
                        // Get the source buffer from the dirty guard
                        let byte_len = dirty_guard.byte_len();
                        let source: &mut imp::MappableBuffer = &mut dirty_guard;

                        // Perform the copy operation
                        imp::copy_mappable_to_gpuable_buffer(
                            source,
                            &gpu_access.gpu_buffer,
                            0,
                            0,
                            byte_len,
                            copy_info,
                        );
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
                        let byte_len = dirty_guard.byte_len();
                        let source: &mut imp::MappableBuffer = &mut dirty_guard;

                        // Perform the copy operation
                        imp::copy_mappable_to_gpuable_buffer(
                            source,
                            &gpu_access.gpu_buffer,
                            0,
                            0,
                            byte_len,
                            copy_info,
                        );
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
                        let byte_len = dirty_guard.byte_len();
                        let source: &mut imp::MappableBuffer = &mut dirty_guard;

                        // Perform the copy operation
                        imp::copy_mappable_to_gpuable_buffer(
                            source,
                            &gpu_access.gpu_buffer,
                            0,
                            0,
                            byte_len,
                            copy_info,
                        );
                        copy_guards.push(dirty_guard);
                    }

                    buffer_guards.insert(*bind_index, Arc::new(gpu_access));
                }
                BindTarget::DynamicTexture(texture) => {
                    // Safety: keep the guard alive
                    let mut gpu_access = unsafe { texture.acquire_gpu_texture() };

                    // Handle the copy if there's a dirty guard
                    if let Some(mut dirty_guard) = gpu_access.take_dirty_guard() {
                        // Perform the texture copy using copy_from_mappable without hardcoding format
                        if let Err(e) =
                            dirty_guard.perform_copy(&mut *gpu_access.gpu_texture, copy_info)
                        {
                            panic!("Texture copy failed: {}", e);
                        }
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
    bind_group: BindGroup,
    #[allow(dead_code)] // guards keep resources alive during GPU execution
    guards: Vec<Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>>,
    guards_textures: Vec<Arc<crate::bindings::forward::dynamic::frame_texture::GPUAccess>>,
    vertex_buffers: Vec<(u32, WgpuCell<wgpu::Buffer>)>,
    dynamic_vertex_buffers: Vec<(
        u32,
        Arc<crate::bindings::forward::dynamic::buffer::GPUAccess>,
    )>,
    index_buffer: Option<WgpuCell<wgpu::Buffer>>,
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
        _copy_info: &mut CopyInfo,
    ) -> Self {
        let mut entries = Vec::new();
        //these need to be kept alive during GPU execution
        let build_dynamic_buffers_gpu = StableAddressVec::with_capactiy(5);
        let build_dynamic_textures_gpu = StableAddressVec::with_capactiy(5);

        //these are only used for the bind group
        let build_static_texture_views = StableAddressVec::with_capactiy(5);
        let mut build_static_buffers = StableAddressVec::with_capactiy(5);
        let mut build_dynamic_texture_views = StableAddressVec::with_capactiy(5);

        let mut clone_buffers = StableAddressVec::with_capactiy(5);

        let mut camera_buffers = StableAddressVec::with_capactiy(5);

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
                            .gpu_buffer
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
                    let camera_clone = stored_buffer.gpu_buffer.buffer().assume(|e| e.clone());
                    let camera_clone = camera_buffers.push(camera_clone);
                    BindingResource::Buffer(BufferBinding {
                        buffer: &camera_clone,
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
                    BindingResource::TextureView(&view)
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

        let bind_group = bind_device.0.device.assume(|device| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(name),
                layout: bind_group_layout,
                entries: entries.as_slice(),
            })
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
            guards_textures: gpu_guard_texture_views,
            vertex_buffers,
            dynamic_vertex_buffers,
            index_buffer,
        }
    }

    fn new(
        bind_device: &crate::images::BoundDevice,
        bind_style: &crate::bindings::bind_style::BindStyle,
        name: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        camera_buffer: &Buffer<CameraProjection>,
        mipmapped_sampler: &WgpuCell<wgpu::Sampler>,
        copy_info: &mut CopyInfo<'_>,
    ) -> (Self, AcquiredGuards) {
        // First acquire guards and perform copies
        let mut acquired_guards = AcquiredGuards::new(bind_style, copy_info, camera_buffer);

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

impl PortInternal {
    pub(crate) async fn new(
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
            .device
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
        Ok(PortInternal {
            engine: engine.clone(),
            camera_buffer,
            camera,
            pass_config: RenderInput::new(PassConfig::new()),
            prepared_passes: Vec::new(),
            view,
            port_reporter_send,
            frame: 0,
            dump_framebuffer: std::env::var("IW_DUMP_FRAMEBUFFER")
                .map(|e| e == "1")
                .unwrap_or(false),
            scaled_size: RenderInput::new(None),
            mipmapped_sampler,
        })
    }

    fn setup_depth_buffer(&self) -> (wgpu::Texture, wgpu::TextureView) {
        let depth_extra_usage = if self.dump_framebuffer {
            wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::empty()
        };

        let device = self.engine.bound_device().as_ref();
        let scaled_size = self.scaled_size.requested.unwrap();
        let depth_texture = device.0.device.assume(|device| {
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

    fn update_pass_configuration(&mut self, enable_depth: bool, copy_info: &mut CopyInfo<'_>) {
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
                );
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
            write_guard.async_drop().await;
        }
    }

    fn setup_debug_framebuffer_capture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame_texture: &wgpu::Texture,
        depth_texture: &wgpu::Texture,
    ) -> (
        Option<wgpu::Buffer>,
        Option<u32>,
        Option<wgpu::Buffer>,
        Option<u32>,
    ) {
        if !self.dump_framebuffer {
            return (None, None, None, None);
        }

        let device = self.engine.bound_device().as_ref();
        let scaled_size = self.scaled_size.requested.unwrap();

        let wgpu_bytes_per_row_256 = (scaled_size.0 * 4)
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            .unwrap()
            .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .unwrap();

        let buf = device.0.device.assume(|device| {
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

        let depth_buf = device.0.device.assume(|device| {
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

        (
            Some(buf),
            Some(wgpu_bytes_per_row_256),
            Some(depth_buf),
            Some(depth_wgpu_bytes_per_row_256),
        )
    }

    fn submit_and_present_frame(
        &mut self,
        encoder: wgpu::CommandEncoder,
        frame: Option<wgpu::SurfaceTexture>,
        frame_bind_groups: Vec<BindGroupGuard>,
        frame_acquired_guards: Vec<AcquiredGuards>,
        frame_guard: std::sync::Arc<crate::images::port::FrameGuard>,
        dump_buf: Option<wgpu::Buffer>,
        dump_buff_bytes_per_row: Option<u32>,
        depth_dump_buf: Option<wgpu::Buffer>,
        depth_dump_buff_bytes_per_row: Option<u32>,
    ) {
        let device = self.engine.bound_device().as_ref();
        let encoded = encoder.finish();

        let frame_guard_for_callback = frame_guard.clone();
        let callback_guard = frame_guard_for_callback.clone();
        //this closure requires Send but I don't think we actually do on wgpu
        let frame_acquired_guards = SendCell::new(frame_acquired_guards);

        device.0.queue.assume(|queue| {
            queue.on_submitted_work_done(move || {
                //at runtime, on non-wasm32 platforms, this is polled
                //from a different thread
                std::mem::drop(frame_bind_groups);
                std::mem::drop(frame_acquired_guards);
                callback_guard.mark_gpu_complete();
            });
            queue.submit(std::iter::once(encoded));
        });
        if let Some(f) = frame {
            f.present();
        }
        self.frame += 1;

        if let Some(tx) = dump_buf {
            let move_tx = tx.clone();
            let move_frame = self.frame;
            let scaled_size = self.scaled_size.requested.unwrap();
            tx.map_async(wgpu::MapMode::Read, .., move |result| {
                if let Err(e) = result {
                    panic!("Failed to map framebuffer buffer: {:?}", e);
                } else {
                    let data = move_tx.slice(..).get_mapped_range();
                    let wgpu_bytes_per_row_256 = dump_buff_bytes_per_row.unwrap();
                    let mut pixels = Vec::new();
                    for y in 0..scaled_size.1 {
                        for x in 0..scaled_size.0 {
                            let offset = (y * wgpu_bytes_per_row_256 + x * 4) as usize;
                            let pixel_bgra = tgar::PixelBGRA {
                                b: data[offset],
                                g: data[offset + 1],
                                r: data[offset + 2],
                                a: data[offset + 3],
                            };
                            let zero = tgar::PixelBGRA {
                                b: 0,
                                g: 0,
                                r: 0,
                                a: 0,
                            };
                            if pixel_bgra != zero {}
                            pixels.push(pixel_bgra);
                        }
                    }

                    let tgar = tgar::BGRA::new(
                        scaled_size.0.try_into().unwrap(),
                        scaled_size.1.try_into().unwrap(),
                        &pixels,
                    );
                    let data = tgar.into_data();
                    std::fs::write(format!("frame_{}.tga", move_frame), data)
                        .expect("Failed to write framebuffer dump");
                }
                move_tx.unmap();
            });
            //for map_async to work, we need to combine with needs_poll, maybe others?
            device.0.set_needs_poll()
        }

        if let Some(depth_tx) = depth_dump_buf {
            let move_depth_tx = depth_tx.clone();
            let move_frame = self.frame;
            let scaled_size = self.scaled_size.requested.unwrap();
            depth_tx.map_async(wgpu::MapMode::Read, .., move |result| {
                if let Err(e) = result {
                    panic!("Failed to map depth buffer: {:?}", e);
                } else {
                    let data = move_depth_tx.slice(..).get_mapped_range();
                    let depth_wgpu_bytes_per_row_256 = depth_dump_buff_bytes_per_row.unwrap();
                    let mut depth_pixels = Vec::new();
                    for y in 0..scaled_size.1 {
                        for x in 0..scaled_size.0 {
                            let offset = (y * depth_wgpu_bytes_per_row_256 + x * 2) as usize;
                            let depth_u16 = u16::from_le_bytes([data[offset], data[offset + 1]]);
                            let depth_u8 = (depth_u16 as f32 / 65535.0 * 255.0) as u8;
                            let depth_pixel = tgar::PixelBGRA {
                                b: depth_u8,
                                g: depth_u8,
                                r: depth_u8,
                                a: 255,
                            };
                            depth_pixels.push(depth_pixel);
                        }
                    }
                    let depth_tgar = tgar::BGRA::new(
                        scaled_size.0.try_into().unwrap(),
                        scaled_size.1.try_into().unwrap(),
                        &depth_pixels,
                    );
                    let depth_data = depth_tgar.into_data();
                    std::fs::write(format!("depth_{}.tga", move_frame), depth_data)
                        .expect("Failed to write depth buffer dump");
                }
                move_depth_tx.unmap();
            });
            //for map_async to work, we need to combine with needs_poll, maybe others?
            device.0.set_needs_poll()
        }
        frame_guard_for_callback.mark_cpu_complete();
    }

    pub async fn add_fixed_pass(&mut self, descriptor: PassDescriptor) {
        let mut new_config = self.pass_config.requested.clone();
        new_config.add_pass(descriptor);
        self.pass_config.update(new_config);
        println!(
            "now up to {} passes",
            self.pass_config.requested.pass_descriptors.len()
        );
    }
    async fn render_frame_internal(&mut self) {
        self.update_camera_buffer().await;
        //basically we want to bunch up all our awaits here,
        //so we don't interrupt the frame
        self.port_reporter_send.begin_frame(self.frame);
        let frame_guard = self.port_reporter_send.create_frame_guard();

        let enable_depth = self.pass_config.requested.enable_depth;
        let unscaled_size = self.view.fast_size_scale();
        // Setup frame reporting and surface configuration
        let current_scaled_size = (
            (unscaled_size.0 as f64 * unscaled_size.2) as u32,
            (unscaled_size.1 as f64 * unscaled_size.2) as u32,
        );
        self.scaled_size.update(Some(current_scaled_size));
        let surface = self.view.gpu_impl.as_ref().unwrap().surface.as_ref();
        match surface {
            None => {
                println!("Port surface not initialized");
                // For test views, set a default surface format if not already set
                if self.pass_config.requested.surface_format.is_none() {
                    self.pass_config.requested.surface_format = Some(TextureFormat::Bgra8UnormSrgb);
                }
            }
            Some(surface) => {
                let extra_usage = if self.dump_framebuffer {
                    wgpu::TextureUsages::COPY_SRC
                } else {
                    wgpu::TextureUsages::empty()
                };
                if self.scaled_size.is_dirty() {
                    let surface_capabilities = self
                        .engine
                        .bound_device()
                        .0
                        .adapter
                        .assume(|adapter| surface.get_capabilities(adapter));

                    let device = self.engine.bound_device().as_ref();
                    let scaled_size = self.scaled_size.requested.unwrap();

                    // Update the surface format to match what we'll actually use
                    let preferred_format = surface_capabilities.formats[0];
                    self.pass_config.requested.surface_format = Some(preferred_format);
                    device.0.device.assume(|device| {
                        surface.configure(
                            device,
                            &wgpu::SurfaceConfiguration {
                                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | extra_usage,
                                format: preferred_format,
                                width: scaled_size.0,
                                height: scaled_size.1,
                                present_mode: wgpu::PresentMode::Fifo,
                                desired_maximum_frame_latency: 1,
                                alpha_mode: CompositeAlphaMode::Opaque,
                                view_formats: Vec::new(),
                            },
                        );
                    });
                    self.scaled_size.mark_submitted();
                }
            }
        }

        // Create per-frame resources
        let wgpu_view;
        let frame;
        let color_attachment;
        let frame_texture;
        match surface {
            None => {
                let scaled_size = self.scaled_size.requested.unwrap();
                let device = self.engine.bound_device().as_ref();
                let texture = device.0.device.assume(|device| {
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
                        format: self
                            .pass_config
                            .requested
                            .surface_format
                            .expect("configured format"),
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
                frame_texture = texture;
                color_attachment = wgpu::RenderPassColorAttachment {
                    view: &wgpu_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Default::default(),
                };
            }
            Some(surface) => {
                let surface_texture = surface
                    .get_current_texture()
                    .expect("Acquire swapchain texture");
                frame_texture = surface_texture.texture.clone();

                frame = Some(surface_texture);

                wgpu_view = frame
                    .as_ref()
                    .unwrap()
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
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

        let mut encoder = {
            let device = self.engine.bound_device().as_ref();
            device.0.device.assume(|device| {
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("wgpu port"),
                })
            })
        };

        // Setup depth buffer
        let (depth_texture, depth_view) = self.setup_depth_buffer();

        // Update pass configuration and camera buffer
        {
            let mut copy_info = CopyInfo {
                command_encoder: &mut encoder,
            };
            self.update_pass_configuration(enable_depth, &mut copy_info);
        }

        // Execute render passes
        let depth_store = if self.dump_framebuffer {
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

        // Recreate acquired guards for all prepared passes
        let mut copy_info = CopyInfo {
            command_encoder: &mut encoder,
        };
        for prepared_pass in &mut self.prepared_passes {
            prepared_pass.recreate_acquired_guards(&self.camera_buffer, &mut copy_info)
        }

        // Extract bind groups and acquired guards from prepared passes
        let mut frame_bind_groups = Vec::new();
        let mut frame_acquired_guards = Vec::new();
        for prepared in &mut self.prepared_passes {
            frame_bind_groups.push(prepared.bind_group_guard.clone());
            if let Some(acquired) = prepared.acquired_guards.take() {
                frame_acquired_guards.push(acquired);
            }
        }

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
            render_pass.set_pipeline(&prepared.pipeline);

            let bind_group = &frame_bind_groups[p];
            render_pass.set_bind_group(0, &bind_group.bind_group, &[]);

            for (v, buffer) in &bind_group.vertex_buffers {
                buffer.assume(|buffer| {
                    render_pass.set_vertex_buffer(*v, buffer.slice(..));
                })
            }
            for (v, buffer) in &bind_group.dynamic_vertex_buffers {
                buffer.gpu_buffer.buffer().assume(|buffer| {
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

        // Setup debug framebuffer capture
        let (dump_buf, dump_buff_bytes_per_row, depth_dump_buf, depth_dump_buff_bytes_per_row) =
            self.setup_debug_framebuffer_capture(&mut encoder, &frame_texture, &depth_texture);

        // Submit and present frame
        let frame_guard_arc = std::sync::Arc::new(frame_guard);
        self.submit_and_present_frame(
            encoder,
            frame,
            frame_bind_groups,
            frame_acquired_guards,
            frame_guard_arc,
            dump_buf,
            dump_buff_bytes_per_row,
            depth_dump_buf,
            depth_dump_buff_bytes_per_row,
        );
    }
}

#[derive(Debug)]
pub struct Port {
    internal: Option<PortInternal>,
}

impl Port {
    pub(crate) async fn new(
        engine: &Arc<crate::images::Engine>,
        view: crate::images::view::View,
        camera: Camera,
        port_reporter_send: PortReporterSend,
    ) -> Result<Self, Error> {
        let internal = PortInternal::new(engine, view, camera, port_reporter_send).await?;
        Ok(Port {
            internal: Some(internal),
        })
    }

    pub async fn add_fixed_pass(&mut self, descriptor: PassDescriptor) {
        self.internal
            .as_mut()
            .expect("Port internal missing")
            .add_fixed_pass(descriptor)
            .await;
    }

    pub async fn render_frame(&mut self) {
        logwise::info_sync!("Rendering frame...");
        let mut internal = self.internal.take().expect("Port internal missing");
        internal = smuggle_async("render_frame".to_string(), || async move {
            internal.render_frame_internal().await;
            internal
        })
        .await;
        self.internal = Some(internal);
    }
}
