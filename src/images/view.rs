// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Rendering surfaces and views for the images_and_words graphics system.
//!
//! A [`View`] represents a rendering surface that can be either:
//! - A window surface (when using the `app_window` feature)
//! - A test surface (always available)
//!
//! Views are the primary way to create rendering targets for the [`Engine`](crate::images::Engine).
//! They manage the connection between the OS window system and the underlying graphics backend.
//!
//! # Examples
//!
//! ## Creating a test view
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! use images_and_words::images::view::View;
//!
//! let view = View::for_testing();
//! ```
//!
//! ## Creating a view from a window surface
//!
//! ```no_run
//! # #[cfg(feature = "app_window")]
//! # {
//! use images_and_words::images::view::View;
//! # let surface: app_window::surface::Surface = todo!();
//!
//! let view = View::from_surface(surface).expect("Failed to create view");
//! # }
//! ```
//!
//! ## Using a view with an engine
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! # test_executors::spawn_local(async {
//! use images_and_words::images::{Engine, view::View};
//! use images_and_words::images::projection::WorldCoord;
//!
//! let view = View::for_testing();
//! let camera_position = WorldCoord::new(0.0, 0.0, 10.0);
//! let engine = Engine::rendering_to(view, camera_position)
//!     .await
//!     .expect("Failed to create engine");
//! # }, "view_doctest");
//! ```

use crate::entry_point::EntryPoint;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use std::sync::Arc;

/// Error type for view operations.
///
/// This wraps the underlying implementation errors that can occur when
/// creating or initializing views.
#[derive(thiserror::Error, Debug)]
pub struct Error(#[from] crate::imp::Error);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A rendering surface that can display graphics content.
///
/// A `View` represents either an OS window surface or a test surface,
/// depending on the enabled features. It serves as the connection point
/// between the graphics system and the display surface.
///
/// Views are typically created and then passed to [`Engine::rendering_to`](crate::images::Engine::rendering_to)
/// to create a complete rendering pipeline.
///
/// # Platform Support
///
/// - **Window surfaces**: Requires the `app_window` feature
/// - **Test surfaces**: Always available
///
/// # Thread Safety
///
/// Views implement `Send` to allow them to be moved between threads,
/// which is necessary for the rendering architecture.
#[derive(Debug)]
pub struct View {
    #[allow(dead_code)] //nop implementation does not use
    //lazily created when we connect the entrypoint
    pub(crate) gpu_impl: Option<crate::imp::View>,
    //wgpu wants the windowing implementation to be dropped AFTER gpu implementation
    windowing_impl: WindowingImpl,
}

#[derive(Debug, Clone)]
enum WindowingImpl {
    Testing,
    #[cfg(feature = "app_window")]
    AppWindow(Arc<app_window::surface::Surface>),
}

impl WindowingImpl {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        match self {
            WindowingImpl::Testing => Err(HandleError::NotSupported),
            #[cfg(feature = "app_window")]
            WindowingImpl::AppWindow(surface) => Ok(surface.window_handle()),
        }
    }

    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        match self {
            WindowingImpl::Testing => Err(HandleError::NotSupported),
            #[cfg(feature = "app_window")]
            WindowingImpl::AppWindow(surface) => Ok(surface.display_handle()),
        }
    }
}

impl WindowingImpl {
    async fn size_scale(&self) -> (u16, u16, f64) {
        match self {
            WindowingImpl::Testing => (800, 600, 1.0),
            #[cfg(feature = "app_window")]
            WindowingImpl::AppWindow(surface) => {
                let (size, scale) = surface.size_scale().await;
                (size.width() as u16, size.height() as u16, scale)
            }
        }
    }

    fn fast_size_scale(&self) -> (u16, u16, f64) {
        match self {
            WindowingImpl::Testing => (800, 600, 1.0),
            #[cfg(feature = "app_window")]
            WindowingImpl::AppWindow(surface) => {
                let (size, scale) = surface.size_main();
                (size.width() as u16, size.height() as u16, scale)
            }
        }
    }
}

//we need this to port across to render thread
unsafe impl Send for View {}

impl View {
    /// Internal method to initialize the view with an entry point.
    ///
    /// This method is called by the engine during initialization to connect
    /// the view to the graphics backend. For window surfaces, this creates
    /// the platform-specific view implementation. For test views, this is
    /// a no-op as the implementation is already initialized.
    ///
    /// # Arguments
    ///
    /// * `_entry_point` - The entry point that provides access to the graphics backend
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if initialization succeeded, or an `Error` if the
    /// backend view could not be created.
    pub(crate) async fn provide_entry_point(
        &mut self,
        entry_point: &EntryPoint,
    ) -> Result<(), Error> {
        let imp = crate::imp::View::from_surface(
            entry_point,
            ViewForImp {
                windowing_impl: self.windowing_impl.clone(),
            },
        )
        .await?;
        self.gpu_impl = Some(imp);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ViewForImp {
    windowing_impl: WindowingImpl,
}

impl HasWindowHandle for ViewForImp {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.windowing_impl.window_handle()
    }
}
impl HasDisplayHandle for ViewForImp {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.windowing_impl.display_handle()
    }
}

impl View {
    /// Internal method to get the view's size and scale factor.
    ///
    /// Returns the current dimensions of the view in pixels and its scale factor
    /// for high-DPI displays. The scale factor is used to convert between logical
    /// and physical pixels.
    ///
    /// # Returns
    ///
    /// A tuple of `(width, height, scale_factor)` where:
    /// - `width` - The width in pixels (u16)
    /// - `height` - The height in pixels (u16)
    /// - `scale_factor` - The DPI scale factor (f64)
    ///
    /// For test views, this always returns (800, 600, 1.0).
    pub(crate) async fn size_scale(&self) -> (u16, u16, f64) {
        self.windowing_impl.size_scale().await
    }

    /// Creates a view from an OS window surface.
    ///
    /// This method creates a `View` that renders to an actual window managed
    /// by the operating system through the `app_window` crate.
    ///
    /// # Arguments
    ///
    /// * `surface` - The window surface from the `app_window` crate
    ///
    /// # Returns
    ///
    /// Returns `Ok(View)` if the view was created successfully, or an `Error`
    /// if there was a problem accessing the window handles.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "app_window")]
    /// # {
    /// use images_and_words::images::view::View;
    /// # let surface: app_window::surface::Surface = todo!();
    ///
    /// let view = View::from_surface(surface)
    ///     .expect("Failed to create view from surface");
    /// # }
    /// ```
    ///
    /// # Platform Requirements
    ///
    /// This method is only available when the `app_window` feature is enabled.
    #[cfg(feature = "app_window")]
    pub fn from_surface(surface: app_window::surface::Surface) -> Result<Self, Error> {
        Ok(View {
            gpu_impl: None,
            windowing_impl: WindowingImpl::AppWindow(Arc::new(surface)),
        })
    }

    pub fn testing() -> Self {
        View {
            gpu_impl: None,
            windowing_impl: WindowingImpl::Testing,
        }
    }

    /**
        Implements a fast, inline size-scale system.

        This may require some coordination with the OS windowing system,
    */
    pub(crate) fn fast_size_scale(&self) -> (u16, u16, f64) {
        self.windowing_impl.fast_size_scale()
    }

    /// Creates a view suitable for testing.
    ///
    /// This method creates a `View` that doesn't require an actual window surface,
    /// making it ideal for unit tests and integration tests. The test view provides
    /// a fixed size of 800x600 pixels with a scale factor of 1.0.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// use images_and_words::images::view::View;
    ///
    /// let test_view = View::for_testing();
    /// // Use with an engine for testing
    /// ```
    ///
    /// # Testing Features
    ///
    /// The test view:
    /// - Reports a fixed size of 800x600 pixels
    /// - Uses a scale factor of 1.0
    /// - Bypasses window system requirements
    /// - Provides immediate initialization
    ///
    /// # Availability
    ///
    /// This method is always available.
    pub fn for_testing() -> Self {
        View {
            gpu_impl: None,
            windowing_impl: WindowingImpl::Testing,
        }
    }
}
