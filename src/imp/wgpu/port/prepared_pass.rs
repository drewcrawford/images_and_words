// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::bind_style::BindTarget;
use crate::bindings::forward::dynamic::buffer::Buffer;
use crate::images::render_pass::{DrawCommand, PassDescriptor};
use crate::images::vertex_layout::VertexFieldType;
use crate::imp;
use crate::imp::wgpu::buffer::StorageType;
use crate::imp::wgpu::cell::WgpuCell;
use crate::stable_address_vec::StableAddressVec;
use std::num::NonZero;
use wgpu::{
    BindGroupLayoutEntry, BindingType, BlendState, BufferBindingType, BufferSize, ColorTargetState,
    CompareFunction, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    StencilFaceState, StencilState, TextureFormat, TextureSampleType, TextureViewDimension,
    VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode,
};

use super::guards::{AcquiredGuards, BindGroupGuard};
use super::types::{CameraProjection, PassConfig};

/**
A pass that is prepared to be rendered (compiled, layout calculated, etc.)
*/
#[derive(Debug)]
pub struct PreparedPass {
    pub pipeline: WgpuCell<RenderPipeline>,
    pub pass_descriptor: PassDescriptor,
    #[allow(dead_code)] //instance counts are not used yet
    pub instance_count: u32,
    pub vertex_count: u32,
    pub depth_pass: bool,
    pub bind_group_guard: BindGroupGuard,
    pub acquired_guards: Option<AcquiredGuards>,
}

impl PreparedPass {
    pub async fn new(
        bind_device: &crate::images::BoundDevice,
        descriptor: PassDescriptor,
        enable_depth: bool,
        camera_buffer: &Buffer<CameraProjection>,
        mipmapped_sampler: &WgpuCell<wgpu::Sampler>,
        copy_info: &mut imp::CopyInfo<'_>,
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

        let bind_group_layout = bind_device.0.device().assume(|device| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(descriptor.name()),
                entries: layouts.as_slice(),
            })
        });

        let pipeline_layout = bind_device.0.device().assume(|device| {
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(descriptor.name()),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[], //not yet supported
            })
        });

        let vertex_module = bind_device.0.device().assume(|device| {
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
            front_face: wgpu::FrontFace::Ccw, //?
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        };

        //because everything is in one render pass, we need all the depth states to match
        //enable depth if any pass wants it
        let depth_state = if enable_depth {
            Some(wgpu::DepthStencilState {
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

        let fragment_module = bind_device.0.device().assume(|device| {
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
        logwise::debuginternal_sync!(
            "surface format is {surface_format}",
            surface_format = logwise::privacy::LogIt(pass_config.surface_format)
        );

        let color_target_format = if pass_config.surface_format.is_srgb() {
            pass_config.surface_format
        } else {
            //in this case we accomplish this via view
            TextureFormat::Bgra8UnormSrgb
        };
        let color_target_state = ColorTargetState {
            format: color_target_format,
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
            .device()
            .assume(|device| device.create_render_pipeline(&render_descriptor));
        logwise::trace_sync!("Created render pipeline");

        // Create the BindGroupGuard using the constructed bind_group_layout
        let (bind_group_guard, acquired_guards) = BindGroupGuard::new(
            bind_device,
            descriptor.bind_style(),
            descriptor.name(),
            &bind_group_layout,
            camera_buffer,
            mipmapped_sampler,
            copy_info,
        )
        .await;
        logwise::trace_sync!("Created bindgroup guard");
        PreparedPass {
            pipeline: WgpuCell::new(pipeline),
            vertex_count,
            instance_count,
            depth_pass: render_descriptor.depth_stencil.is_some(),
            pass_descriptor: descriptor.clone(),
            bind_group_guard,
            acquired_guards: Some(acquired_guards),
        }
    }

    pub async fn recreate_acquired_guards(
        &mut self,
        camera_buffer: &Buffer<CameraProjection>,
        copy_info: &mut imp::CopyInfo<'_>,
    ) {
        // Recreate only the acquired_guards field, leaving bind_group_guard unchanged
        let new_acquired_guards =
            AcquiredGuards::new(self.pass_descriptor.bind_style(), copy_info, camera_buffer).await;
        self.acquired_guards = Some(new_acquired_guards);
    }
}
