// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::images::view::View;
use crate::imp::wgpu::cell::WgpuCell;

pub struct UnboundDevice {
    pub(super) adapter: WgpuCell<wgpu::Adapter>,
}

impl UnboundDevice {
    pub async fn pick(
        view: &View,
        entry_point: &crate::entry_point::EntryPoint,
    ) -> Result<UnboundDevice, super::Error> {
        let view = view.gpu_impl.as_ref().unwrap();
        let adapter = match view.surface.as_ref() {
            None => {
                let options = wgpu::RequestAdapterOptions {
                    power_preference: Default::default(),
                    force_fallback_adapter: false,
                    compatible_surface: None,
                };
                entry_point.0.0.request_adapter(&options).await
            }
            Some(surface) => {
                surface
                    .assume_async(async move |surface| {
                        let options = wgpu::RequestAdapterOptions {
                            power_preference: Default::default(),
                            force_fallback_adapter: false,
                            compatible_surface: Some(surface),
                        };
                        entry_point.0.0.request_adapter(&options).await
                    })
                    .await
            }
        };

        let adapter = adapter.map_err(|_| super::Error::NoSuchAdapter)?;
        Ok(UnboundDevice {
            adapter: adapter.into(),
        })
    }
}
