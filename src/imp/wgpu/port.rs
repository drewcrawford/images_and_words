use std::num::NonZero;
use std::sync::Arc;
use wgpu::{BindGroupLayoutEntry, BindingType, BufferBindingType, PipelineLayoutDescriptor, RenderPipelineDescriptor};
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{PassDescriptor, PassTrait};
use crate::imp::{Error};

#[derive(Debug)]
pub struct Port {
    engine: Arc<crate::images::Engine>,
    pass_descriptors: Vec<PassDescriptor>,
}

fn pass_descriptor_to_pipeline_descriptor(bind_device: &crate::images::BoundDevice, descriptor: &PassDescriptor) -> RenderPipelineDescriptor<'static> {
    let mut layouts = Vec::new();

    for (slot,render_side) in descriptor.bind_style().buffers() {
        let stage = match slot.stage {
            crate::bindings::bind_style::Stage::Fragment => wgpu::ShaderStages::FRAGMENT,
            crate::bindings::bind_style::Stage::Vertex => wgpu::ShaderStages::VERTEX,
        };
        let layout = BindGroupLayoutEntry {
            binding: slot.pass_index,
            visibility: stage,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage {
                    read_only: true, //this is currently, true, false is still unimplemented
                },
                has_dynamic_offset: false,
                min_binding_size: Some(NonZero::new(1).unwrap()), //???
            },
            count: None, //not array
        };
        layouts.push(layout);
    }
    for bind_info in descriptor.bind_style().texture_style().static_textures() {
        let stage = match bind_info.slot.stage {
            crate::bindings::bind_style::Stage::Fragment => wgpu::ShaderStages::FRAGMENT,
            crate::bindings::bind_style::Stage::Vertex => wgpu::ShaderStages::VERTEX,
        };
        let layout = BindGroupLayoutEntry {
            binding: bind_info.slot.pass_index,
            visibility: stage,
            ty: BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D1, // ??
                multisampled: false,
            },
            count: None,
        };
        layouts.push(layout);
    }
    if descriptor.bind_style().binds_camera_matrix {
        todo!()
    }
    if descriptor.bind_style().frame_counter.is_some() {
        todo!()
    }

    let bind_group_layout = bind_device.0.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(descriptor.name()),
        entries: layouts.as_slice(),
    });

    let pipeline_layout = bind_device.0.device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(descriptor.name()),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[], //not yet supported
    });




    RenderPipelineDescriptor {
        label: Some(descriptor.name()),
        //https://docs.rs/wgpu/24.0.1/wgpu/struct.RenderPipelineDescriptor.html
        layout: todo!(),
        vertex: todo!(),
        primitive: todo!(),
        depth_stencil: todo!(),
        multisample: todo!(),
        fragment: todo!(),
        multiview: todo!(),
        cache: todo!(),
    }
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
            let pipeline_descriptor = pass_descriptor_to_pipeline_descriptor(device, descriptor);
            todo!()
        }
        todo!()
    }
}

