use crate::images::view::View;

pub struct UnboundDevice {
    pub(super) adapter: wgpu::Adapter,
}

impl UnboundDevice {
    pub async fn pick(
        view: &View,
        entry_point: &crate::entry_point::EntryPoint,
    ) -> Result<UnboundDevice, super::Error> {
        // let wgpu_surface = wgpu::Surface::
        let options = wgpu::RequestAdapterOptions {
            power_preference: Default::default(),
            force_fallback_adapter: false,
            compatible_surface: view
                .imp
                .as_ref()
                .expect("View not initialized")
                .surface
                .as_ref(),
        };
        let adapter = entry_point.0.0.request_adapter(&options).await;
        let adapter = adapter.map_err(|_| super::Error::NoSuchAdapter)?;

        Ok(UnboundDevice { adapter })
    }
}
