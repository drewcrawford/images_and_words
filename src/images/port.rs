//! Ports control rendering to a single surface with render passes and frame pacing.
//! 
//! A [`Port`] represents a rendering target (like a window or offscreen surface) that manages:
//! - Multiple render passes that execute in sequence
//! - Frame pacing and synchronization
//! - Performance metrics through [`PortReporter`]
//! - Camera management for the viewport
//!
//! ## Basic Usage
//! 
//! Ports are typically created through an [`Engine`] rather than directly:
//!
//! ```
//! # use images_and_words::images::{Engine, view::View};
//! # use images_and_words::images::projection::WorldCoord;
//! # test_executors::sleep_on(async {
//! let view = View::for_testing();
//! let camera_position = WorldCoord::new(0.0, 0.0, 10.0);
//! let engine = Engine::rendering_to(view, camera_position)
//!     .await
//!     .expect("Failed to create engine");
//! 
//! // Access the main port
//! let port = engine.main_port_mut();
//! // Port is ready to accept render passes
//! # });
//! ```
//!
//! ## Frame Synchronization
//!
//! The port automatically handles frame synchronization through dirty tracking.
//! When any bound resource (buffer, texture, camera) is modified, the port
//! schedules a new frame to be rendered.

use std::fmt::Formatter;
use std::sync::{Arc, Mutex};
use crate::images::render_pass::PassDescriptor;
use crate::images::Engine;
use std::sync::atomic::{Ordering,AtomicU32};
use crate::images::camera::{Camera};
use std::time::{Instant};
use crate::bindings::bind_style::BindTarget;
use crate::bindings::dirty_tracking::{DirtyAggregateReceiver, DirtyReceiver};
use crate::bittricks::{u16s_to_u32, u32_to_u16s};
use crate::images::frame::Frame;
use crate::images::projection::{Projection, WorldCoord};
use crate::images::view::View;
use crate::imp;
use await_values::{Value, Observer};

/**
Guard type for tracking frame timing information.
Created when a frame begins and dropped when frame is complete.
*/
#[derive(Debug)]
pub(crate) struct FrameGuard {
    frame_start: Instant,
    cpu_end: Mutex<Option<Instant>>,
    gpu_end: Mutex<Option<Instant>>,
    port_reporter: Arc<PortReporterImpl>,
}

impl FrameGuard {
    pub(crate) fn new(port_reporter: Arc<PortReporterImpl>) -> Self {
        Self {
            frame_start: Instant::now(),
            cpu_end: Mutex::new(None),
            gpu_end: Mutex::new(None),
            port_reporter,
        }
    }
    #[allow(dead_code)]
    pub(crate) fn mark_cpu_complete(&self) {
        *self.cpu_end.lock().unwrap() = Some(Instant::now());
    }
    #[allow(dead_code)]
    pub(crate) fn mark_gpu_complete(&self) {
        *self.gpu_end.lock().unwrap() = Some(Instant::now());
    }
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        let cpu_end = self.cpu_end.lock().unwrap().expect("CPU end time not set");
        let gpu_end = self.gpu_end.lock().unwrap().expect("GPU end time not set");
        
        let frame_info = FrameInfo {
            frame_start: self.frame_start,
            cpu_end,
            gpu_end,
        };
        
        self.port_reporter.add_frame_info(frame_info);
    }
}

/**
Complete timing information for a finished frame.
*/
#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameInfo {
    frame_start: Instant,
    cpu_end: Instant,
    gpu_end: Instant,
}

impl FrameInfo {
    #[allow(dead_code)]
    pub(crate) fn cpu_duration_ms(&self) -> i32 {
        self.cpu_end.duration_since(self.frame_start).as_millis() as i32
    }

    #[allow(dead_code)]
    pub(crate) fn gpu_duration_ms(&self) -> i32 {
        self.gpu_end.duration_since(self.cpu_end).as_millis() as i32
    }
    
    #[allow(dead_code)]
    pub(crate) fn total_duration_ms(&self) -> i32 {
        self.gpu_end.duration_since(self.frame_start).as_millis() as i32
    }
}









/// A rendering port that manages render passes for a single surface.
///
/// A `Port` represents a complete rendering pipeline for a single output surface
/// (like a window or offscreen render target). It manages multiple render passes,
/// handles frame synchronization, and provides performance monitoring.
///
/// # Creation
///
/// Ports are typically created through an [`Engine`] using [`Engine::main_port_mut()`]
/// rather than directly via [`Port::new()`].
///
/// # Render Passes
///
/// A port executes render passes in the order they were added. Each pass can:
/// - Bind different resources (buffers, textures, samplers)
/// - Use different shaders
/// - Draw different geometry
///
/// # Frame Synchronization
///
/// The port uses dirty tracking to automatically render new frames when bound
/// resources change. This ensures efficient rendering without unnecessary frames.
#[derive(Debug)]
pub struct Port {
    imp: crate::imp::Port,
    port_reporter: PortReporter,
    descriptors: Vec<PassDescriptor>,
    camera: Camera,
    engine: Arc<Engine>,
}

/// Error type for port operations.
#[derive(Debug)]
pub struct Error (
    imp::Error,
);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::error::Error for Error {}
impl From<imp::Error> for Error {
    fn from(e: imp::Error) -> Self {
        Self(e)
    }
}

/// Provides performance metrics and frame synchronization for a port.
///
/// `PortReporter` allows applications to monitor rendering performance and
/// implement custom frame pacing strategies. It provides real-time metrics
/// including:
/// - Frames per second (FPS)
/// - GPU rendering time
/// - CPU preparation time
/// - Minimum elapsed time between frames
///
/// # Example
///
/// ```
/// # use images_and_words::images::{Engine, view::View};
/// # use images_and_words::images::projection::WorldCoord;
/// # test_executors::sleep_on(async {
/// # let engine = Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 10.0))
/// #     .await.expect("Failed to create engine");
/// # let port = engine.main_port_mut();
/// let reporter = port.port_reporter();
/// 
/// // Access performance observers
/// let fps_observer = reporter.fps();
/// let gpu_ms_observer = reporter.ms();
/// 
/// // Check current camera projection
/// let projection = reporter.camera_projection();
/// 
/// // Get the drawable size
/// let (width, height) = reporter.drawable_size();
/// println!("Drawable size: {}x{}", width, height);
/// # });
/// ```
#[derive(Clone,Debug)]
pub struct PortReporter {
    imp: Arc<PortReporterImpl>,
    camera: Camera,
    fps: Observer<i32>,
    ms: Observer<i32>,
    cpu_ms: Observer<i32>,
    min_elapsed_ms: Observer<i32>,
}
impl PortReporter {

    /// Returns the frame number of the most recently started frame.
    ///
    /// This method helps with cache invalidation and synchronization decisions.
    /// Since multiple frames may be in flight concurrently, this represents the
    /// newest frame that has started processing.
    ///
    /// # Synchronization Notes
    ///
    /// - Multiple frames may be processing simultaneously
    /// - This value can change at any time after calling
    /// - No transactional isolation is provided
    /// - Useful for determining if cached data is stale
    ///
    pub fn latest_begun(&self) -> Frame {
        Frame::new(self.imp.frame_begun.load(Ordering::Relaxed))
    }
    /// Returns the current camera projection.
    ///
    /// The returned projection approximates the state at the latest frame,
    /// but exact synchronization is not guaranteed.
    pub fn camera_projection(&self) -> Projection {
        self.camera.projection()
    }

    /// Returns the current drawable size as (width, height).
    ///
    /// The returned size approximates the state at the latest frame,
    /// but exact synchronization is not guaranteed.
    pub fn drawable_size(&self) -> (u16,u16) {
        let u = self.imp.drawable_size.load(Ordering::Relaxed);
        u32_to_u16s(u)
    }
    /// Returns an observer for the current frames per second.
    pub fn fps(&self) -> &Observer<i32> {
        &self.fps
    }
    
    /// Returns an observer for the average GPU time per frame in milliseconds.
    pub fn ms(&self) -> &Observer<i32> {
        &self.ms
    }
    
    /// Returns an observer for the average CPU time per frame in milliseconds.
    pub fn cpu_ms(&self) -> &Observer<i32> {
        &self.cpu_ms
    }
    
    /// Returns an observer for the minimum elapsed time between frames in milliseconds.
    ///
    /// This metric is useful for frame pacing, as it indicates the fastest
    /// frame rate achieved in recent samples.
    pub fn min_elapsed_ms(&self) -> &Observer<i32> {
        &self.min_elapsed_ms
    }

    //awaits the completion of the next frame.
    pub async fn await_frame(&self) {
        todo!()
    }
}

#[derive(Debug)]
pub(crate) struct PortReporterImpl {
    frame_begun: AtomicU32,
    drawable_size: AtomicU32,
    fps: Value<i32>,
    ms: Value<i32>,
    cpu_ms: Value<i32>,
    min_elapsed_ms: Value<i32>,
    frame_history: Mutex<Vec<FrameInfo>>,
}
impl PortReporterImpl {
    pub(crate) fn create_frame_guard(self: &Arc<Self>) -> FrameGuard {
        FrameGuard::new(self.clone())
    }
    
    pub(crate) fn add_frame_info(&self, frame_info: FrameInfo) {
        const MAX_HISTORY: usize = 60; // Keep last 60 frames
        
        let mut history = self.frame_history.lock().unwrap();
        history.push(frame_info);
        
        // Keep only the most recent frames
        while history.len() > MAX_HISTORY {
            history.remove(0);
        }
        
        // Recalculate statistics
        if !history.is_empty() {
            // Calculate FPS from frame intervals
            let mut total_interval = 0.0;
            let mut min_interval = f64::MAX;
            
            for i in 1..history.len() {
                let interval = history[i].frame_start.duration_since(history[i-1].frame_start).as_secs_f64();
                total_interval += interval;
                min_interval = min_interval.min(interval);
            }
            
            if history.len() > 1 {
                let avg_interval = total_interval / (history.len() - 1) as f64;
                let fps = (1.0 / avg_interval).round() as i32;
                self.fps.set(fps);
                
                let min_elapsed_ms = (min_interval * 1000.0) as i32;
                self.min_elapsed_ms.set(min_elapsed_ms);
            }
            
            // Calculate average GPU and CPU times
            let total_gpu_ms: i32 = history.iter().map(|f| f.gpu_duration_ms()).sum();
            let total_cpu_ms: i32 = history.iter().map(|f| f.cpu_duration_ms()).sum();
            
            let avg_gpu_ms = total_gpu_ms / history.len() as i32;
            let avg_cpu_ms = total_cpu_ms / history.len() as i32;
            
            self.ms.set(avg_gpu_ms);
            self.cpu_ms.set(avg_cpu_ms);
        }
    }
}


#[derive(Debug)]
pub(crate) struct PortReporterSend {
    imp: Arc<PortReporterImpl>,
}
impl PortReporterSend {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn begin_frame(&mut self, frame: u32) {
        self.imp.frame_begun.store(frame, Ordering::Relaxed);
    }
    //todo: read this, mt2-491
    #[allow(dead_code)]
    pub(crate) fn drawable_size(&self, size: (u16,u16)) {
        self.imp.drawable_size.store(u16s_to_u32(size.0, size.1), Ordering::Relaxed);
    }

    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn create_frame_guard(&self) -> FrameGuard {
        self.imp.create_frame_guard()
    }
}

fn port_reporter(initial_frame: u32, camera: &Camera) -> (PortReporterSend,PortReporter) {
    let fps = Value::new(0);
    let ms = Value::new(0);
    let cpu_ms = Value::new(0);
    let min_elapsed_ms = Value::new(0);
    
    let fps_observer = fps.observe();
    let ms_observer = ms.observe();
    let cpu_ms_observer = cpu_ms.observe();
    let min_elapsed_ms_observer = min_elapsed_ms.observe();
    
    let imp = Arc::new(PortReporterImpl {
        frame_begun: AtomicU32::new(initial_frame),
        drawable_size: AtomicU32::new(0),
        fps,
        ms,
        cpu_ms,
        min_elapsed_ms,
        frame_history: Mutex::new(Vec::new()),
    });

    (
        PortReporterSend {
            imp: imp.clone(),
        },
        PortReporter {
            imp,
            camera: camera.clone(),
            fps: fps_observer,
            ms: ms_observer,
            cpu_ms: cpu_ms_observer,
            min_elapsed_ms: min_elapsed_ms_observer,
        }
    )

}


impl Port {
    /// Creates a new port for rendering to the specified view.
    ///
    /// # Parameters
    ///
    /// - `engine`: The rendering engine to use
    /// - `view`: The view/surface to render to
    /// - `initial_camera_position`: Starting position for the camera
    /// - `window_size`: Initial window dimensions as (width, height, dpi_scale)
    ///
    /// # Example
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use images_and_words::images::{Engine, view::View, port::Port};
    /// # use images_and_words::images::projection::WorldCoord;
    /// # test_executors::sleep_on(async {
    /// # let view = View::for_testing();
    /// # let engine = Arc::new(Engine::rendering_to(view, WorldCoord::new(0.0, 0.0, 10.0))
    /// #     .await
    /// #     .expect("Failed to create engine"));
    /// # 
    /// # let another_view = View::for_testing();
    /// let port = Port::new(
    ///     &engine,
    ///     another_view,
    ///     WorldCoord::new(0.0, 0.0, 5.0),
    ///     (800, 600, 1.0)
    /// ).expect("Failed to create port");
    /// # });
    /// ```
    pub fn new(engine: &Arc<Engine>, view: View, initial_camera_position: WorldCoord,window_size: (u16,u16,f64)) -> Result<Self,Error> {
        let camera = Camera::new(window_size, initial_camera_position);
        let (port_sender,port_reporter) = port_reporter(0, &camera);

        Ok(Self{
            imp: crate::imp::Port::new(engine, view,  camera.clone(),port_sender).map_err(Error)?,
            port_reporter,
            descriptors: Default::default(),
            camera,
            engine: engine.clone(),
        })
    }
    /// Adds a fixed render pass to the port.
    ///
    /// Render passes are executed in the order they were added. Each pass
    /// defines its own shaders, bindings, and draw commands.
    ///
    /// # Example
    ///
    /// ```
    /// # use images_and_words::images::{Engine, view::View};
    /// # use images_and_words::images::projection::WorldCoord;
    /// # use images_and_words::images::render_pass::{PassDescriptor, DrawCommand};
    /// # use images_and_words::images::shader::{VertexShader, FragmentShader};
    /// # use images_and_words::bindings::BindStyle;
    /// # test_executors::sleep_on(async {
    /// # let engine = Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 10.0))
    /// #     .await.expect("Failed to create engine");
    /// # let mut port = engine.main_port_mut();
    /// let vertex_shader = VertexShader::new("test", 
    ///     "@vertex fn vs_main() -> @builtin(position) vec4<f32> { 
    ///         return vec4<f32>(0.0, 0.0, 0.0, 1.0); 
    ///     }".to_string());
    /// let fragment_shader = FragmentShader::new("test",
    ///     "@fragment fn fs_main() -> @location(0) vec4<f32> { 
    ///         return vec4<f32>(1.0, 0.0, 0.0, 1.0); 
    ///     }".to_string());
    /// 
    /// let pass = PassDescriptor::new(
    ///     "test_pass".to_string(),
    ///     vertex_shader,
    ///     fragment_shader,
    ///     BindStyle::new(),
    ///     DrawCommand::TriangleStrip(3),
    ///     false,  // depth test
    ///     false   // depth write
    /// );
    /// 
    /// port.add_fixed_pass(pass).await;
    /// # });
    /// ```
    ///
    /// # Limitations
    /// 
    /// - Currently cannot add passes while the port is running (mt2-242)
    /// - There is no way to remove passes once added (mt2-243)
    pub async fn add_fixed_pass(&mut self, descriptor: PassDescriptor) {
        self.imp.add_fixed_pass(descriptor.clone()).await;
        self.descriptors.push(descriptor);
    }

    /// Adds multiple fixed render passes to the port.
    ///
    /// This is a convenience method for adding multiple passes at once.
    /// Passes are executed in the order they appear in the vector.
    ///
    /// See [`add_fixed_pass`](Self::add_fixed_pass) for details and limitations.
    pub async fn add_fixed_passes(&mut self, descriptors: Vec<PassDescriptor>) {
        for descriptor in descriptors {
            self.imp.add_fixed_pass(descriptor.clone()).await;
            self.descriptors.push(descriptor);
        }
    }

    /// Returns the bound device associated with this port's engine.
    ///
    /// The bound device is used to create GPU resources like buffers and textures.
    pub fn bound_device(&self) -> &Arc<crate::images::BoundDevice> {
        self.engine.bound_device()
    }

    /// Forces immediate rendering of the next frame.
    ///
    /// This bypasses the dirty tracking system and renders a frame even if
    /// no resources have changed. Useful for debugging or ensuring immediate
    /// visual updates.
    pub async fn force_render(&mut self) {
        //force render the next frame, even if nothing is dirty
        self.imp.render_frame().await;
    }



    fn collect_dirty_receivers(&self) -> Vec<DirtyReceiver> {
        //we need to figure out all the dirty stuff
        let mut dirty_receivers = Vec::new();
        for pass in &self.descriptors {
            for bind in pass.bind_style.binds.values() {
                match &bind.target {
                    BindTarget::DynamicBuffer(a) => {
                        dirty_receivers.push(a.dirty_receiver());
                    }
                    BindTarget::DynamicVB(_,a) => {
                        dirty_receivers.push(a.dirty_receiver());
                    }
                    BindTarget::Camera => {
                        dirty_receivers.push(self.camera.dirty_receiver());
                    }
                    BindTarget::DynamicTexture(texture) => {
                        dirty_receivers.push(texture.gpu_dirty_receiver())
                    }
                    BindTarget::StaticBuffer(_) => { /* nothing to do, not considered dirty */}
                    BindTarget::FrameCounter => {/* nothing to do - not considered dirty */}

                    BindTarget::StaticTexture(_, _) => { /* also not considered dirty the 2nd+ time */}
                    BindTarget::Sampler(_) => { /* also not considered dirty */}
                    BindTarget::VB(..)  => { /* also not considered dirty */}

                }
            }
        }
        dirty_receivers
    }

    /// Starts the port's rendering loop.
    ///
    /// Once started, the port will automatically render frames whenever bound
    /// resources are marked dirty. The method runs indefinitely, monitoring
    /// for changes and rendering as needed.
    ///
    /// # Example
    ///
    /// ```
    /// # use images_and_words::images::{Engine, view::View};
    /// # use images_and_words::images::projection::WorldCoord;
    /// # test_executors::sleep_on(async {
    /// # let engine = Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 10.0))
    /// #     .await.expect("Failed to create engine");
    /// # let mut port = engine.main_port_mut();
    /// // Add render passes first
    /// // port.add_fixed_pass(pass).await;
    /// 
    /// // Start rendering - this runs forever
    /// // port.start().await?;
    /// # });
    /// ```
    ///
    /// # Note
    ///
    /// Ports do not render by default - you must call this method to begin
    /// the render loop.
    pub async fn start(&mut self) -> Result<(),Error> {
        //render first frame regardless
        self.force_render().await;
        loop {
            let receiver = DirtyAggregateReceiver::new(self.collect_dirty_receivers());
            receiver.wait_for_dirty().await;
            self.force_render().await;
        }
    }

    #[cfg(feature="testing")]
    pub fn needs_render(&self) -> bool {
        //this is a test-only function that returns true if the port needs to render.
        //it is used in tests to check if the port is rendering correctly.
        let receiver = DirtyAggregateReceiver::new(self.collect_dirty_receivers());
        receiver.is_dirty()
    }

    /// Returns the port reporter for monitoring performance metrics.
    pub fn port_reporter(&self) -> &PortReporter {
        &self.port_reporter
    }
    
    /// Returns the camera associated with this port.
    ///
    /// The camera controls the view transformation and projection for rendering.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

}