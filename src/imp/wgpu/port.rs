use std::num::NonZero;
use std::sync::Arc;
use wgpu::{BindGroupLayoutEntry, BindingType, BufferBindingType, CompareFunction, DepthStencilState, Face, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType, StencilFaceState, StencilState, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use crate::bindings::bind_style::BindTarget;
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{DrawCommand, PassDescriptor, PassTrait};
use crate::imp::{Error};

#[derive(Debug)]
pub struct Port {
    engine: Arc<crate::images::Engine>,
    pass_descriptors: Vec<PassDescriptor>,
}

fn pass_descriptor_to_pipeline(bind_device: &crate::images::BoundDevice, descriptor: &PassDescriptor) -> RenderPipeline {
    let mut layouts = Vec::new();

    for (pass_index, info) in &descriptor.bind_style().binds {
        let stage = match info.stage {
            crate::bindings::bind_style::Stage::Fragment => wgpu::ShaderStages::FRAGMENT,
            crate::bindings::bind_style::Stage::Vertex => wgpu::ShaderStages::VERTEX,
        };
        let binding_type = match info.target {
            BindTarget::Buffer => {
                BindingType::Buffer {
                    ty: BufferBindingType::Storage {
                        read_only: true, //this is currently, true, false is still unimplemented
                    },
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZero::new(1).unwrap()), //???
                }
            }
            BindTarget::Camera => {
                //I guess these are implemented with buffers for now...
                BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZero::new(1).unwrap()), //???
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
                    sample_type: TextureSampleType::Float{filterable: false}, //??
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                }
            }
            BindTarget::Sampler => {
                BindingType::Sampler(SamplerBindingType::NonFiltering)
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
    println!("Will create bind group layout {:?}",layouts);

    let bind_group_layout = bind_device.0.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(descriptor.name()),
        entries: layouts.as_slice(),
    });

    let pipeline_layout = bind_device.0.device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(descriptor.name()),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[], //not yet supported
    });

    let vertex_module = bind_device.0.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&(descriptor.name().to_owned() + "_vtx")),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&descriptor.vertex_shader.wgsl_code)),
    });

    let vertex_state = VertexState {
        module: &vertex_module,
        entry_point: None,
        compilation_options: Default::default(),
        buffers: &[],
    };
    let topology = match descriptor.draw_command() {
        DrawCommand::TriangleStrip(count) => {PrimitiveTopology::TriangleStrip}
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
           depth_write_enabled: false, //??
           depth_compare: CompareFunction::GreaterEqual, //?
           stencil: StencilState {
               front: StencilFaceState::IGNORE,
               back: StencilFaceState::IGNORE,
               read_mask: 0,
               write_mask: 0,
           },
           bias: Default::default(), //?
       })
    }
    else {
        None
    };

    let multisample_state = MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    let fragment_module = bind_device.0.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&(descriptor.name().to_owned() + "_frag")),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&descriptor.fragment_shader.wgsl_code)),
    });
    let fragment_state = wgpu::FragmentState {
        module: &fragment_module,
        entry_point: None,
        compilation_options: Default::default(),
        targets: &[],
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
    bind_device.0.device.create_render_pipeline(&descriptor)
}

impl Port {
    pub(crate) fn new(_engine: &Arc<crate::images::Engine>, _view: crate::images::view::View, _camera: Camera, _port_reporter_send:PortReporterSend) -> Result<Self,Error> {
        Ok(Port{
            engine: _engine.clone(),
            pass_descriptors: Vec::new(),
        })
    }
    pub async fn add_fixed_pass<const N: usize, P: PassTrait<N>>(&mut self, p: P) -> P::DescriptorResult {
        let mut pass_client = crate::images::PassClient::new(self.engine.bound_device().clone());
        let (descriptors, result) = p.into_descriptor(&mut pass_client).await;
        self.pass_descriptors.extend(descriptors);
        result
    }
    pub async fn start(&mut self) -> Result<(),Error> {
        let device = self.engine.bound_device().as_ref();
        for descriptor in &self.pass_descriptors {
            let pipeline = pass_descriptor_to_pipeline(device, descriptor);
            todo!()
        }
        todo!()
    }
}

