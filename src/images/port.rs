//! Implements 'image ports', which control how we render to a single surface.

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









#[derive(Debug)]
pub struct Port {
    imp: crate::imp::Port,
    port_reporter: PortReporter,
    descriptors: Vec<PassDescriptor>,
    camera: Camera,
    engine: Arc<Engine>,
}

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

/**
A type that clients can use to find out about port activity and perform frame pacing.
 */
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

    /**
    Returns the frame most recently begun.

     Figuring out "which" frame you are "on" is kind of a nonsense question for two reasons:
     1.  Multiple frames are in flight at any one time, so the answer is often something like "several"
     2.  IW is pretty fast so we spend a lot of time NOT running frames actually.  So the answer is often like "none".

     Sometimes the behavior you want is to run code every frame.  For that purpose the right approach might be [crate::bindings::forward::dynamic::frame_texture::FrameTexture].

     Alternatively though maybe you are handling *requests* from clients who are doing their own frame pacing, maybe using that.  Then the problem is, you're going to return some data,
     should it be cached data or new data?

     This function is one way to answer that question, it reflects the most recent frame IW started.  Of course this value can change at any time, including immediately after
     you called it.  There is no guarantee your caller is encoding the same frame and it will not provide any transactional isolation.  However it provides a basic way to throw
     out data that is stale.
     */
    pub fn latest_begun(&self) -> Frame {
        Frame::new(self.imp.frame_begun.load(Ordering::Relaxed))
    }
    /**
    Returns the camera projection.

    Note that there is no particular synchronization guarantee around this type; it is something resembling the same frame as `latest_begin`, but
    it is not guaranteed to be exactly any particular projection.
    */
    pub fn camera_projection(&self) -> Projection {
        self.camera.projection()
    }

    /**
    Returns a recent drawable size.

    Note that there is no particular synchronization guarantee around this type; it is something resembling the same frame as `latest_begin`, but
    it is not guaranteed to be exactly any particular projection.
    */
    pub fn drawable_size(&self) -> (u16,u16) {
        let u = self.imp.drawable_size.load(Ordering::Relaxed);
        u32_to_u16s(u)
    }
    pub fn fps(&self) -> &Observer<i32> {
        &self.fps
    }
    pub fn ms(&self) -> &Observer<i32> {
        &self.ms
    }
    pub fn cpu_ms(&self) -> &Observer<i32> {
        &self.cpu_ms
    }
    
    /**
    Returns the minimum elapsed time between frames from recent samples, in milliseconds.
    
    This can be used by clients to predict their processing times for frame pacing.
    */
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
    pub fn new(engine: &Arc<Engine>, view: View, initial_camera_position: WorldCoord,window_size: (u16,u16,f64)) -> Result<Self,Error> {
        let camera = Camera::new(window_size, initial_camera_position);
        let (port_sender,port_reporter) = port_reporter(0, &camera);

        Ok(Self{
            imp: crate::imp::Port::new(engine, view,  camera.clone(),port_sender).map_err(|e| Error(e))?,
            port_reporter,
            descriptors: Default::default(),
            camera,
            engine: engine.clone(),
        })
    }
    /**
    Adds a fixed pass to the port.

    # Limitations
    Currently, this doesn't work when the new port is running. mt2-242

    There is currently no way to remove a pass.  mt2-243

    */

    pub async fn add_fixed_pass(&mut self, descriptor: PassDescriptor) {
        self.imp.add_fixed_pass(descriptor.clone()).await;
        self.descriptors.push(descriptor);
    }

    /**
    Adds multiple fixed passes to the port.

    # Limitations
    Currently, this doesn't work when the new port is running. mt2-242

    There is currently no way to remove a pass.  mt2-243

    */

    pub async fn add_fixed_passes(&mut self, descriptors: Vec<PassDescriptor>) {
        for descriptor in descriptors {
            self.imp.add_fixed_pass(descriptor.clone()).await;
            self.descriptors.push(descriptor);
        }
    }

    /**
    Provides access to the BoundDevice for building pass descriptors.
    */
    pub fn bound_device(&self) -> &Arc<crate::images::BoundDevice> {
        self.engine.bound_device()
    }

    /**
    Forces a render of the next frame, even if nothing is dirty.
    This is useful for debugging or when you want to ensure the port is rendered immediately.
    */
    pub async fn force_render(&mut self) {
        //force render the next frame, even if nothing is dirty
        self.imp.render_frame().await;
    }



    fn collect_dirty_receivers(&self) -> Vec<DirtyReceiver> {
        //we need to figure out all the dirty stuff
        let mut dirty_receivers = Vec::new();
        for pass in &self.descriptors {
            for (_, bind) in &pass.bind_style.binds {
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

    ///Start rendering on the port.  Ports are not rendered by default.
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

    pub fn port_reporter(&self) -> &PortReporter {
        &self.port_reporter
    }
    /**
    Accesses the camera for the port.
    */
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

}