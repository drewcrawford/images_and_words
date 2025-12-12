// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::entry_point::{EntryPoint, EntryPointError};
use crate::images::device::BoundDevice;
use crate::images::device::{BindError, PickError, UnboundDevice};
use crate::images::port::Port;
use crate::images::projection::WorldCoord;
use crate::images::view::View;
use crate::imp;
use std::sync::Arc;
use std::sync::OnceLock;

/// Main GPU rendering engine that coordinates graphics resources and rendering operations.
///
/// The Engine manages the graphics pipeline by coordinating between the GPU device,
/// rendering ports, and the underlying backend implementation. It provides thread-safe
/// access to the main rendering port and maintains the lifetime of critical GPU resources.
///
/// Engines are typically created via [`Engine::rendering_to`] and shared using `Arc`.
#[derive(Debug)]
pub struct Engine {
    //note that drop order is significant here.
    /// Engine's main rendering port.
    /// Uses OnceLock since the port is set once during construction and then only read.
    /// Port uses interior mutability for all its operations, so we can safely
    /// share references to it without additional synchronization.
    main_port: OnceLock<Port>,
    //device we bound to this engine.  Arc because it gets moved into the render_thread.
    device: Arc<BoundDevice>,
    _entry_point: Arc<EntryPoint>,
    _engine: crate::imp::Engine,
}

impl Engine {
    /// Creates a test engine instance for unit testing.
    ///
    /// This creates an engine with a test view suitable for automated testing
    /// without requiring an actual window or display surface.
    pub async fn for_testing() -> Result<Arc<Self>, CreateError> {
        Self::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await
    }
    /// Creates a new rendering engine targeting the specified view.
    ///
    /// # Arguments
    /// * `view` - The rendering target (window surface or test view)
    /// * `initial_camera_position` - Starting camera position in world coordinates
    ///
    /// # Returns
    /// An Arc-wrapped engine instance, or an error if initialization fails.
    pub async fn rendering_to(
        mut view: View,
        initial_camera_position: WorldCoord,
    ) -> Result<Arc<Self>, CreateError> {
        // Register exfiltrate commands on first engine creation
        #[cfg(feature = "exfiltrate")]
        {
            use std::sync::Once;
            static REGISTER_COMMANDS: Once = Once::new();
            REGISTER_COMMANDS.call_once(|| {
                crate::exfiltrate_commands::register_commands();
            });
        }

        logwise::info_sync!("Engine::rendering_to() started");

        logwise::info_sync!("Creating EntryPoint...");
        let entry_point = Arc::new(EntryPoint::new().await?);
        logwise::info_sync!("EntryPoint created successfully");

        logwise::info_sync!("Providing EntryPoint to view...");
        view.provide_entry_point(&entry_point).await?;
        logwise::info_sync!("EntryPoint provided to view successfully");

        logwise::info_sync!("Getting view size and scale...");
        let (initial_width, initial_height, initial_scale) = view.size_scale().await;
        logwise::info_sync!(
            "View size: {}x{}, scale: {}",
            initial_width,
            initial_height,
            initial_scale
        );

        logwise::info_sync!("Picking unbound device...");
        let unbound_device = UnboundDevice::pick(&view, &entry_point).await?;
        logwise::info_sync!("Unbound device picked successfully");

        logwise::info_sync!("Binding device...");
        let bound_device = Arc::new(BoundDevice::bind(unbound_device, entry_point.clone()).await?);
        logwise::info_sync!("Device bound successfully");

        logwise::info_sync!("Creating implementation engine...");
        let imp = crate::imp::Engine::rendering_to_view(&bound_device).await;
        logwise::info_sync!("Implementation engine created successfully");

        logwise::info_sync!("Creating Engine struct...");
        let r = Arc::new(Engine {
            main_port: OnceLock::new(),
            device: bound_device,
            _entry_point: entry_point,
            _engine: imp,
        });
        logwise::info_sync!("Engine struct created successfully");

        logwise::info_sync!("Creating final port...");
        let final_port = Port::new(
            &r,
            view,
            initial_camera_position,
            (initial_width, initial_height, initial_scale),
        )
        .await
        .unwrap();
        logwise::info_sync!("Final port created successfully");

        logwise::info_sync!("Setting main port...");
        r.main_port
            .set(final_port)
            .expect("main_port already initialized");
        logwise::info_sync!("Engine::rendering_to() completed successfully");
        Ok(r)
    }

    /// Returns a reference to the main rendering port.
    ///
    /// Port methods use interior mutability, so this returns `&Port` rather than
    /// requiring mutable access. This allows the port to be accessed concurrently
    /// from multiple contexts (e.g., the render loop and user code).
    pub fn main_port(&self) -> &Port {
        self.main_port.get().expect("main_port not initialized")
    }

    /// Returns a mutable guard to the main rendering port.
    ///
    /// This is a compatibility method that returns the same as `main_port()`.
    /// Prefer using `main_port()` directly.
    #[deprecated(note = "Use main_port() instead - Port uses interior mutability")]
    pub fn main_port_mut(&self) -> &Port {
        self.main_port()
    }
    /// Returns the GPU device bound to this engine.
    pub fn bound_device(&self) -> &Arc<BoundDevice> {
        &self.device
    }
}

// Boilerplate section

// Clone: Intentionally not implemented. Engine is a resource manager that coordinates
// exclusive GPU resources. The intended sharing pattern is via Arc<Engine>, not cloning
// the Engine itself. Cloning would be confusing and potentially unsafe given the
// "drop order is significant" comment and resource management semantics.

/// Errors that can occur during engine creation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CreateError {
    /// Failed to create the GPU entry point.
    #[error("Can't create engine {0}")]
    EntryPoint(#[from] EntryPointError),
    /// Failed to select a suitable GPU device.
    #[error("Can't find a GPU {0}")]
    Gpu(#[from] PickError),
    /// Failed to bind to the selected GPU device.
    #[error("Can't bind GPU {0}")]
    Bind(#[from] BindError),
    /// Failed to create the rendering port.
    #[error("Can't create port {0}")]
    Port(#[from] super::port::Error),
    /// Backend implementation error.
    #[error("Implementation error {0}")]
    Imp(#[from] imp::Error),
    /// View initialization error.
    #[error("View error {0}")]
    View(#[from] crate::images::view::Error),
}
