use std::sync::Arc;
use wgpu::RenderPipelineDescriptor;
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{PassDescriptor, PassTrait};
use crate::imp::Error;

#[derive(Debug)]
pub struct Port {
    engine: Arc<crate::images::Engine>,
    pass_descriptors: Vec<PassDescriptor>,
}

fn pass_descriptor_to_pipeline_descriptor(descriptor: &PassDescriptor) -> RenderPipelineDescriptor {
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
        for descriptor in &self.pass_descriptors {
            let pipeline_descriptor = pass_descriptor_to_pipeline_descriptor(&descriptor);
            todo!()
        }
        todo!()
    }
}

