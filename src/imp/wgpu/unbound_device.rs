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
        let entry_point = entry_point.clone();
        let adapter = match view.surface.as_ref() {
            None => {
                let options = wgpu::RequestAdapterOptions {
                    power_preference: Default::default(),
                    force_fallback_adapter: false,
                    compatible_surface: None,
                };
                entry_point
                    .0
                    .0
                    .assume_async(|instance: &wgpu::Instance| {
                        let fut = instance.request_adapter(&options);
                        async move { WgpuCell::new(fut.await.unwrap()) }
                    })
                    .await
            }
            Some(surface) => {
                surface
                    .with_async(async move |surface| {
                        let options = wgpu::RequestAdapterOptions {
                            power_preference: Default::default(),
                            force_fallback_adapter: false,
                            compatible_surface: Some(surface),
                        };
                        entry_point
                            .0
                            .0
                            .assume_async(|instance: &wgpu::Instance| {
                                let fut = instance.request_adapter(&options);
                                async move { WgpuCell::new(fut.await.unwrap()) }
                            })
                            .await
                    })
                    .await
            }
        };

        Ok(UnboundDevice {
            adapter: adapter.into(),
        })
    }
}
