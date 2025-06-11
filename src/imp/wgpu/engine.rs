// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use std::sync::Arc;

#[derive(Debug)]
pub struct Engine;
impl Engine {
    pub async fn rendering_to_view(_bound_device: &Arc<crate::images::BoundDevice>) -> Self {
        //do we actually need to do anything?
        Engine
    }
}
