use crate::bindings::bind_style::BindTarget;
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{DrawCommand, PassDescriptor};
use crate::imp::{CopyInfo, Error};
use std::num::NonZero;
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferBinding, BufferBindingType, BufferSize, ColorTargetState, CompareFunction, CompositeAlphaMode, DepthStencilState, Face, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPassDepthStencilAttachment, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType, StencilFaceState, StencilState, StoreOp, TextureFormat, TextureSampleType, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode};
use crate::bindings::forward::dynamic::buffer::{CRepr, SomeGPUAccess};
use crate::bindings::sampler::SamplerType;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::PassClient;
use crate::images::vertex_layout::VertexFieldType;
use crate::imp::wgpu::buffer::StorageType;
use crate::stable_address_vec::StableAddressVec;

#[repr(C)]
pub struct CameraProjection {
    pub projection: [f32; 16],
}

unsafe impl CRepr for CameraProjection {}

#[derive(Debug)]
pub struct Port {
    engine: Arc<crate::images::Engine>,
    pass_descriptors: Vec<PassDescriptor>,
    view: crate::images::view::View,
    camera: Camera,
    pub(crate) pass_client: PassClient,
}

/**
A pass that is prepared to be rendered (compiled, layout calculated, etc.)
*/
pub struct PreparedPass {
    pipeline: RenderPipeline,
    pass_descriptor: PassDescriptor,
    instance_count: u32,
    vertex_count: u32,
    depth_pass: bool,
}

fn prepare_pass_descriptor(
    bind_device: &crate::images::BoundDevice,
    descriptor: PassDescriptor,
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
            },
            BindTarget::StaticBuffer(imp) => {
                let buffer_binding_type = match imp.imp.storage_type() {
                    StorageType::Uniform => BufferBindingType::Uniform,
                    StorageType::Storage => BufferBindingType::Storage { read_only: true },
                    StorageType::Vertex | StorageType::Index => unreachable!(),
                };

                BindingType::Buffer {
                    ty: buffer_binding_type,
                    has_dynamic_offset: false,
                    min_binding_size: NonZero::new(imp.imp.buffer.size()),
                }
            },
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
            BindTarget::StaticTexture(_texture, sampler_type) => {
                BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: sampler_type.is_some() },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                }
            }
            BindTarget::DynamicTexture(_texture) => {
                BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false }, //??
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                }
            }
            BindTarget::Sampler(sampler) => BindingType::Sampler(SamplerBindingType::Filtering),
            BindTarget::VB(..) => {
                continue //not considered as a binding
            }
            BindTarget::DynamicVB(..) => {
                continue //not considered as a binding
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

    let bind_group_layout =
        bind_device
            .0
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(descriptor.name()),
                entries: layouts.as_slice(),
            });


    let pipeline_layout = bind_device
        .0
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(descriptor.name()),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[], //not yet supported
        });

    let vertex_module = bind_device
        .0
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&descriptor.vertex_shader.label),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                &descriptor.vertex_shader.wgsl_code,
            )),
        });

    //calculate vertex buffers
    let mut vertex_buffers = Vec::new();
    let all_vertex_attributes = StableAddressVec::with_capactiy(5);

    for (b,buffer) in &descriptor.bind_style.binds {
        match &buffer.target {
            BindTarget::StaticBuffer(_) | BindTarget::DynamicBuffer(_) | BindTarget::Camera | BindTarget::FrameCounter | BindTarget::DynamicTexture(_) | BindTarget::StaticTexture(..) | BindTarget::Sampler(_)  => {}
            BindTarget::VB(layout,_)  | BindTarget::DynamicVB(layout,_) => {
                let mut each_vertex_attributes = Vec::new();
                let mut offset = 0;
                for (f,field) in layout.fields.iter().enumerate() {
                    let attribute = VertexAttribute {
                        format: match field.r#type {
                            VertexFieldType::F32 => { wgpu::VertexFormat::Float32 },
                        },
                        offset,
                        shader_location: f as u32,
                    };
                    offset += field.r#type.stride() as u64;
                    each_vertex_attributes.push(attribute);
                }
                let strong_vertex_attributes = all_vertex_attributes.push(each_vertex_attributes);
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
        DrawCommand::TriangleStrip(count) => PrimitiveTopology::TriangleStrip,
        DrawCommand::TriangleList(..) => PrimitiveTopology::TriangleList,
    };
    let vertex_count = match descriptor.draw_command {
        DrawCommand::TriangleStrip(count) => count,
        DrawCommand::TriangleList(triangles) => triangles * 3,
    };
    let instance_count = match descriptor.draw_command {
        DrawCommand::TriangleStrip(..) => 1,
        DrawCommand::TriangleList(triangles) => triangles,
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

    // let depth_state = if descriptor.depth {
    //     Some(DepthStencilState {
    //         format: TextureFormat::Depth16Unorm,
    //         depth_write_enabled: false,                   //??
    //         depth_compare: CompareFunction::GreaterEqual, //?
    //         stencil: StencilState {
    //             front: StencilFaceState::IGNORE,
    //             back: StencilFaceState::IGNORE,
    //             read_mask: 0,
    //             write_mask: 0,
    //         },
    //         bias: Default::default(), //?
    //     })
    // } else {
    //     None
    // };

    //because everything is in one render pass, we need all the depth states to match
    //at the moment let's just assume that all passes need depth, whether they report that
    //or not!

    let depth_state = Some(DepthStencilState {
        format: TextureFormat::Depth16Unorm,
        depth_write_enabled: true,                   //??
        depth_compare: CompareFunction::LessEqual, //?
        stencil: StencilState {
            front: StencilFaceState::IGNORE,
            back: StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias: Default::default(), //?
    });

    let multisample_state = MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    let fragment_module = bind_device
        .0
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(descriptor.fragment_shader.label),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                &descriptor.fragment_shader.wgsl_code,
            )),
        });
    let blend = if descriptor.alpha {
        Some(BlendState::ALPHA_BLENDING)
    }
    else {
        None
    };
    let color_target_state = ColorTargetState {
        format: TextureFormat::Bgra8UnormSrgb,
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
    let pipeline = bind_device.0.device.create_render_pipeline(&render_descriptor);
    PreparedPass {
        pipeline,
        vertex_count,
        instance_count,
        depth_pass: render_descriptor.depth_stencil.is_some(),
        pass_descriptor: descriptor.clone(),
    }
}

/**
Wrapper type that contains the bind group
and all guards that are needed to keep the resources alive.
*/
pub struct BindGroupGuard {
    bind_group: BindGroup,
    guards: StableAddressVec<Box<dyn SomeGPUAccess>>,
    vertex_buffers: Vec<(u32, wgpu::Buffer)>,
    dynamic_vertex_buffers: Vec<(u32, Box<dyn SomeGPUAccess>)>,
    index_buffer: Option<wgpu::Buffer>,
}

pub fn prepare_bind_group(
    bind_device: &crate::images::BoundDevice,
    prepared: &PreparedPass,
    pass_client: &PassClient,
    camera_buffer: &dyn SomeGPUAccess,
    pixel_linear_sampler: &wgpu::Sampler,
    mipmapped_sampler: &wgpu::Sampler,
    copy_info: &mut CopyInfo,
) -> BindGroupGuard {
    let mut entries = Vec::new();
    let build_resources = StableAddressVec::with_capactiy(5);

    let gpu_guard_buffers = StableAddressVec::with_capactiy(5);
    let gpu_guard_textures = StableAddressVec::with_capactiy(5);


    for (pass_index, info) in &prepared.pass_descriptor.bind_style().binds {
        let resource = match &info.target {
            BindTarget::DynamicBuffer(buf) => {
                //safety: Keep the guard alive
                let build_buffer = unsafe{gpu_guard_buffers.push(buf.imp.acquire_gpu_buffer(copy_info))};
                BindingResource::Buffer(BufferBinding {
                    buffer: &build_buffer.as_imp().buffer,
                    offset: 0,
                    size: Some(NonZero::new(buf.byte_size as u64).unwrap()),
                })
            }
            BindTarget::StaticBuffer(buf) => {
                BindingResource::Buffer(BufferBinding {
                    buffer: &buf.imp.buffer,
                    offset: 0,
                    size: Some(NonZero::new(buf.imp.buffer.size() as u64).unwrap()),
                })
            }
            BindTarget::Camera => {
                BindingResource::Buffer(BufferBinding {
                    buffer: &camera_buffer.as_imp().buffer,
                    offset: 0,
                    size: Some(NonZero::new(std::mem::size_of::<CameraProjection>() as u64).unwrap()),
                })
            }
            BindTarget::FrameCounter => { todo!() }
            BindTarget::StaticTexture(texture, sampler_type) => {
                let lookup = pass_client.lookup_static_texture(*texture);
                let view = build_resources.push(lookup.imp.texture.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: None,
                    dimension: None,
                    usage: None,
                    aspect: Default::default(),
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                }));
                BindingResource::TextureView(&view)
            }
            BindTarget::DynamicTexture(texture) => {
                //safety: keep the guard alive
                let texture = unsafe{gpu_guard_textures.push(texture.acquire_gpu_texture(copy_info))};
                let view = build_resources.push(texture.texture.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: None,
                    dimension: None,
                    usage: None,
                    aspect: Default::default(),
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                }));
                BindingResource::TextureView(&view)
            }
            BindTarget::Sampler(sampler) => {
                match sampler {
                    SamplerType::PixelLinear => { BindingResource::Sampler(pixel_linear_sampler) }
                    SamplerType::Mipmapped => { BindingResource::Sampler(mipmapped_sampler) }
                }
            }
            BindTarget::VB(..) | BindTarget::DynamicVB(..) => {
                continue //not considered as a binding
            }

        };

        let entry = BindGroupEntry {
            binding: *pass_index,
            resource: resource,
        };
        entries.push(entry);
    }
    let bind_group = bind_device.0.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(prepared.pass_descriptor.name()),
        layout: &prepared.pipeline.get_bind_group_layout(0),
        entries: entries.as_slice(),
    });

    //find vertex buffers
    let mut vertex_buffers = Vec::new();
    let mut dynamic_vertex_buffers = Vec::new();
    for (b,buffer) in &prepared.pass_descriptor.bind_style().binds {
        match &buffer.target {
            BindTarget::StaticBuffer(_) | BindTarget::DynamicBuffer(_) | BindTarget::Camera | BindTarget::FrameCounter | BindTarget::DynamicTexture(_) | BindTarget::StaticTexture(..) | BindTarget::Sampler(_) => {}
            BindTarget::VB(layout,render_side) => {
                let buffer = render_side.imp.buffer.clone();
                vertex_buffers.push((*b, buffer));
            }
            BindTarget::DynamicVB(layout,render_side) => {
                //safety: guard kept alive
                let buffer = unsafe{render_side.imp.acquire_gpu_buffer(copy_info)};
                dynamic_vertex_buffers.push((*b, buffer));
            }
        }
    }

    let index_buffer = if let Some(buffer) = &prepared.pass_descriptor.bind_style().index_buffer {
        let buffer = buffer.imp.buffer.clone();
        Some(buffer)
    } else {
        None
    };


    BindGroupGuard {
        bind_group,
        guards: gpu_guard_buffers,
        vertex_buffers,
        dynamic_vertex_buffers,
        index_buffer,
    }
}

impl Port {
    pub(crate) fn new(
        _engine: &Arc<crate::images::Engine>,
        view: crate::images::view::View,
        camera: Camera,
        _port_reporter_send: PortReporterSend,
    ) -> Result<Self, Error> {
        let pass_client = PassClient::new(_engine.bound_device().clone());
        Ok(Port {
            engine: _engine.clone(),
            pass_descriptors: Vec::new(),
            view,
            pass_client,
            camera,
        })
    }
    pub async fn add_fixed_pass(
        &mut self,
        descriptor: PassDescriptor,
    )  {
        self.pass_descriptors.push(descriptor);
        println!("now up to {} passes", self.pass_descriptors.len());
    }
    pub async fn render_frame(&mut self) {
        //todo: We are currently doing a lot of setup work on each frame, that ought to be moved to initialization?
        let device = self.engine.bound_device().as_ref();
        let mut prepared = Vec::new();
        for descriptor in &self.pass_descriptors {
            let pipeline = prepare_pass_descriptor(device, descriptor.clone());
            prepared.push(pipeline);
        }
        let size = self.view.size().await;
        let surface = &self
            .view
            .imp
            .as_ref()
            .expect("View not initialized")
            .surface;

        surface.configure(
            &device.0.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                width: size.0.into(),
                height: size.1.into(),
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 1,
                alpha_mode: CompositeAlphaMode::Opaque,
                view_formats: Vec::new(),
            },
        );

        let pixel_linear_sampler = device.0.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixel linear sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        let mipmapped_sampler = device.0.device.create_sampler(&wgpu::SamplerDescriptor {
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

        let camera_mappable_buffer = crate::bindings::forward::dynamic::buffer::Buffer::new(self.engine.bound_device().clone(), 1, GPUBufferUsage::VertexShaderRead, "Camera", |initialize| {
            let projection = self.camera.copy_projection_and_clear_dirty_bit();
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
                ]
            }
        }).expect("Create camera buffer");


        //create per-frame resources
        let frame_guards = StableAddressVec::with_capactiy(10);
        let frame = surface
            .get_current_texture()
            .expect("Acquire swapchain texture");
        let wgpu_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device
            .0
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("wgpu port"),
            });
        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &wgpu_view,
            resolve_target: None,
            ops: Default::default(),
        };

        let depth_texture = device.0.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: wgpu::Extent3d {
                width: size.0.into(),
                height: size.1.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth16Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
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

        //prepare to copy data
        let mut copy_info = CopyInfo {
            command_encoder: &mut encoder,
        };

        let camera_render_side = camera_mappable_buffer.render_side();
        let camera_gpu_access = unsafe {
            use crate::bindings::forward::dynamic::buffer::SomeRenderSide;
            let camera_gpu_access = camera_render_side.acquire_gpu_buffer(&mut copy_info);
            frame_guards.push(camera_gpu_access) //safety
        };

        let depth_stencil_attachment = if prepared.iter().any(|e| e.depth_pass) {
            Some(RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: StoreOp::Discard,
                }),
                stencil_ops: None,
            })
        } else {
            None
        };

        //we're going to do two passes.  The first pass is to prepare our bind groups.
        //note that we do this per-frame, because in dynamic cases we want to bind to a buffer only known at runtime
        let mut copy_info = CopyInfo {
            command_encoder: &mut encoder,
        };
        let mut frame_bind_groups = Vec::new();
        for prepared in &prepared {
            let camera_gpu_deref: &dyn SomeGPUAccess = &**camera_gpu_access;
            let bind_group = prepare_bind_group(device, prepared, &self.pass_client, camera_gpu_deref, &pixel_linear_sampler, &mipmapped_sampler, &mut copy_info);
            frame_bind_groups.push(bind_group);
        }
        //in the second pass, we encode our render pass
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Port render"),
            color_attachments: &[Some(color_attachment.clone())],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for (p,prepared) in prepared.iter().enumerate() {
            render_pass.push_debug_group(prepared.pass_descriptor.name());
            render_pass.set_pipeline(&prepared.pipeline);

            //use the bind group we declared earlier
            let bind_group = &frame_bind_groups[p];

            render_pass.set_bind_group(0, &bind_group.bind_group, &[]);

            for (v,buffer) in &bind_group.vertex_buffers {
                render_pass.set_vertex_buffer(*v, buffer.slice(..));
            }
            for (v, buffer) in &bind_group.dynamic_vertex_buffers {
                let buffer = unsafe { buffer.as_imp().buffer.slice(..) };
                render_pass.set_vertex_buffer(*v, buffer);
            }
            if let Some(buffer) = &bind_group.index_buffer {
                render_pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..prepared.vertex_count, 0, 0..1);
            }
            else {
                render_pass.draw(0..prepared.vertex_count, 0..1);
            }
            render_pass.pop_debug_group();
        }
        // println!("encoded {passes} passes", passes = prepared.len());

        std::mem::drop(render_pass); //stop mutably borrowing the encoder
        let encoded = encoder.finish();
        device.0.queue.submit(std::iter::once(encoded));
        device.0.queue.on_submitted_work_done(move || {
            //callbacks must be alive for full GPU-side render
            std::mem::drop(frame_bind_groups);
            // println!("frame guards dropped");
        });
        frame.present();
        // println!("frame presented");
    }
}
