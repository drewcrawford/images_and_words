use std::sync::Arc;
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::{PassDescriptor, PassTrait};
use crate::imp::Error;

#[derive(Debug)]
pub struct Port {
    engine: Arc<crate::images::Engine>,
    pass_descriptors: Vec<PassDescriptor>,
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
        todo!()
    }
}

