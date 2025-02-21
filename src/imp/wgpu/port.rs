use crate::bindings::bind_style::BindTarget;
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{DrawCommand, PassDescriptor, PassTrait};
use crate::imp::Error;
use std::num::NonZero;
use std::sync::Arc;
use wgpu::util::RenderEncoder;
use wgpu::{
    BindGroupLayoutEntry, BindingType, BufferBindingType, BufferSize, ColorTargetState,
    CompareFunction, CompositeAlphaMode, DepthStencilState, Face, FrontFace, MultisampleState,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    RenderPassDepthStencilAttachment, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    StencilFaceState, StencilState, StoreOp, TextureFormat, TextureSampleType,
    TextureViewDimension, VertexState,
};

#[derive(Debug)]
pub struct Port {
    engine: Arc<crate::images::Engine>,
    pass_descriptors: Vec<PassDescriptor>,
    view: crate::images::view::View,
}

/**
A pass that is prepared to be rendered.
*/
pub struct PreparedPass {
    pipeline: RenderPipeline,
    instance_count: u32,
    vertex_count: u32,
    depth_pass: bool,
}

fn pass_descriptor_to_pipeline(
    bind_device: &crate::images::BoundDevice,
    descriptor: &PassDescriptor,
) -> PreparedPass {
    let mut layouts = Vec::new();

    for (pass_index, info) in &descriptor.bind_style().binds {
        let stage = match info.stage {
            crate::bindings::bind_style::Stage::Fragment => wgpu::ShaderStages::FRAGMENT,
            crate::bindings::bind_style::Stage::Vertex => wgpu::ShaderStages::VERTEX,
        };
        let binding_type = match &info.target {
            BindTarget::Buffer(imp) => BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(BufferSize::new(imp.element_size as u64).unwrap()),
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
            BindTarget::Texture => {
                BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false }, //??
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                }
            }
            BindTarget::Sampler => BindingType::Sampler(SamplerBindingType::NonFiltering),
        };
        let layout = BindGroupLayoutEntry {
            binding: *pass_index,
            visibility: stage,
            ty: binding_type,
            count: None, //not array
        };
        layouts.push(layout);
    }
    println!("Will create bind group layout {:?}", layouts);

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
            label: Some(&(descriptor.name().to_owned() + "_vtx")),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                &descriptor.vertex_shader.wgsl_code,
            )),
        });

    let vertex_state = VertexState {
        module: &vertex_module,
        entry_point: None,
        compilation_options: Default::default(),
        buffers: &[],
    };
    let topology = match descriptor.draw_command() {
        DrawCommand::TriangleStrip(count) => PrimitiveTopology::TriangleStrip,
    };
    let vertex_count = match descriptor.draw_command {
        DrawCommand::TriangleStrip(count) => count,
    };
    let instance_count = match descriptor.draw_command {
        DrawCommand::TriangleStrip(..) => 1,
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

    let depth_state = if descriptor.depth {
        Some(DepthStencilState {
            format: TextureFormat::Depth16Unorm,
            depth_write_enabled: false,                   //??
            depth_compare: CompareFunction::GreaterEqual, //?
            stencil: StencilState {
                front: StencilFaceState::IGNORE,
                back: StencilFaceState::IGNORE,
                read_mask: 0,
                write_mask: 0,
            },
            bias: Default::default(), //?
        })
    } else {
        None
    };

    let multisample_state = MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    let fragment_module = bind_device
        .0
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&(descriptor.name().to_owned() + "_frag")),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                &descriptor.fragment_shader.wgsl_code,
            )),
        });
    let color_target_state = ColorTargetState {
        format: TextureFormat::Bgra8UnormSrgb,
        blend: None,
        write_mask: Default::default(),
    };
    let fragment_state = wgpu::FragmentState {
        module: &fragment_module,
        entry_point: None,
        compilation_options: Default::default(),
        targets: &[Some(color_target_state)],
    };

    let descriptor = RenderPipelineDescriptor {
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
    let pipeline = bind_device.0.device.create_render_pipeline(&descriptor);
    PreparedPass {
        pipeline,
        vertex_count,
        instance_count,
        depth_pass: descriptor.depth_stencil.is_some(),
    }
}

impl Port {
    pub(crate) fn new(
        _engine: &Arc<crate::images::Engine>,
        view: crate::images::view::View,
        _camera: Camera,
        _port_reporter_send: PortReporterSend,
    ) -> Result<Self, Error> {
        Ok(Port {
            engine: _engine.clone(),
            pass_descriptors: Vec::new(),
            view,
        })
    }
    pub async fn add_fixed_pass<const N: usize, P: PassTrait<N>>(
        &mut self,
        p: P,
    ) -> P::DescriptorResult {
        let mut pass_client = crate::images::PassClient::new(self.engine.bound_device().clone());
        let (descriptors, result) = p.into_descriptor(&mut pass_client).await;
        self.pass_descriptors.extend(descriptors);
        result
    }
    pub async fn start(&mut self) -> Result<(), Error> {
        let device = self.engine.bound_device().as_ref();
        let mut prepared = Vec::new();
        for descriptor in &self.pass_descriptors {
            let pipeline = pass_descriptor_to_pipeline(device, descriptor);
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

        for prepared in &prepared {
            let depth_stencil_attachment = if prepared.depth_pass {
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
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(color_attachment.clone())],
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&prepared.pipeline);
            render_pass.draw(0..prepared.vertex_count, 0..1);
        }

        todo!()
    }
}
