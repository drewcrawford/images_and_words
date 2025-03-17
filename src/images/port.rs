//! Implements 'image ports', which control how we render to a single surface.

use std::fmt::Formatter;
use std::sync::{Arc, Mutex};
use crate::images::device::BoundDevice;
use crate::images::render_pass::{PassDescriptor, PassTrait};
use crate::images::Engine;
use crate::bindings::forward::r#static::texture::Texture;
use crate::pixel_formats::{R32Float, R8UNorm, RGBA16Unorm, RGBA8UNorm, RGFloat, BGRA8UNormSRGB, R16Float};
use std::sync::atomic::{Ordering,AtomicU32};
use crate::images::camera::{Camera};
use std::time::{Instant};
use slotmap::{DefaultKey, SlotMap};
use crate::bindings::bind_style::BindTarget;
use crate::bindings::BindStyle;
use crate::bindings::dirty_tracking::DirtyAggregateReceiver;
use crate::bittricks::{u16s_to_u32, u32_to_u16s};
use crate::images::frame::Frame;
use crate::images::projection::{Projection, WorldCoord};
use crate::images::view::View;
use crate::imp;



#[derive(Debug)]
pub struct InstanceTicket<T> {
    slot: slotmap::DefaultKey,
    _phantom: std::marker::PhantomData<T>,
}
impl<T> Copy for InstanceTicket<T> {}

impl<T> Clone for InstanceTicket<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug,Clone,Copy)]
pub(crate) enum InternalStaticTextureTicket {
    R32Float(InstanceTicket<Texture<R32Float>>),
    RGBA16UNorm(InstanceTicket<Texture<RGBA16Unorm>>),
    RGFloat(InstanceTicket<Texture<RGFloat>>),
    RGBA8Unorm(InstanceTicket<Texture<RGBA8UNorm>>),
    R8UNorm(InstanceTicket<Texture<R8UNorm>>),
    BGRA8UnormSRGB(InstanceTicket<Texture<BGRA8UNormSRGB>>),
    R16Float(InstanceTicket<Texture<R16Float>>),
}


//design note: static texture tickets have a distinct type because
//dynamic textures have a guard type when they're popped, and we want to resolve that to some
//specific type inside the port.

#[derive(Debug,Clone,Copy)] pub  struct StaticTextureTicket(pub(crate) InternalStaticTextureTicket);


/**
This type is provided to render passes to perform their internal operations.

We want to separate it out from the main port for a few reasons:
1.  Just basic API hygiene
2.  The render passes need an object-safe API because from Port's perspective, they are a heterogeneous collection.
    So we can't pass in Port<View> anywhere, without knowing the view.
 */
#[derive(Debug)]
pub struct PassClient {
    //static textures
    pub(crate) texture_r32float: SlotMap<DefaultKey,Texture<R32Float>>,
    pub(crate) texture_r16float: SlotMap<DefaultKey, Texture<R16Float>>,
    pub(crate) texture_rgba16unorm: SlotMap<DefaultKey, Texture<RGBA16Unorm>>,
    pub(crate) texture_rgfloat: SlotMap<DefaultKey, Texture<RGFloat>>,
    pub(crate) texture_rgba8unorm: SlotMap<DefaultKey,Texture<RGBA8UNorm>>,
    pub(crate) texture_r8unorm: SlotMap<DefaultKey, Texture<R8UNorm>>,
    pub(crate) texture_bgra8unorm_srgb: SlotMap<DefaultKey, Texture<BGRA8UNormSRGB>>,
    //frame textures

    bound_device: Arc<BoundDevice>,
}
impl PassClient {
    pub fn add_texture_r32float(&mut self, texture: Texture<R32Float>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::R32Float(InstanceTicket{slot: self.texture_r32float.insert(texture), _phantom: std::marker::PhantomData} ))
    }

    pub fn add_texture_r16float(&mut self, texture: Texture<R16Float>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::R16Float(InstanceTicket{slot: self.texture_r16float.insert(texture), _phantom: std::marker::PhantomData} ))
    }
    pub fn add_texture_rgba16unorm(&mut self, texture: Texture<RGBA16Unorm>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::RGBA16UNorm(InstanceTicket{slot: self.texture_rgba16unorm.insert(texture), _phantom: std::marker::PhantomData}))
    }
    pub fn add_texture_rgfloat(&mut self, texture: Texture<RGFloat>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::RGFloat(InstanceTicket{slot: self.texture_rgfloat.insert(texture), _phantom: std::marker::PhantomData} ))
    }
    pub fn add_texture_rgba8unorm(&mut self, texture: Texture<RGBA8UNorm>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::RGBA8Unorm(InstanceTicket{slot: self.texture_rgba8unorm.insert(texture), _phantom: std::marker::PhantomData} ))
    }
    pub fn add_texture_bgra8_unorm_srgb(&mut self, texture: Texture<BGRA8UNormSRGB>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::BGRA8UnormSRGB(InstanceTicket{slot: self.texture_bgra8unorm_srgb.insert(texture), _phantom: std::marker::PhantomData} ))
    }
    pub fn add_texture_r8unorm(&mut self, texture: Texture<R8UNorm>) -> StaticTextureTicket {
        StaticTextureTicket(InternalStaticTextureTicket::R8UNorm(InstanceTicket{slot: self.texture_r8unorm.insert(texture), _phantom: std::marker::PhantomData} ))
    }

    pub(crate) fn lookup_static_texture(&self, ticket: StaticTextureTicket) -> crate::bindings::forward::r#static::texture::RenderSide {
        match ticket.0 {
            InternalStaticTextureTicket::R32Float(t) => {
                let m = self.texture_r32float.get(t.slot).expect("Texture not found");
                m.render_side()
            }
            InternalStaticTextureTicket::R16Float(t) => {
                let m = self.texture_r16float.get(t.slot).expect("Texture not found");
                m.render_side()
            }
            InternalStaticTextureTicket::RGBA16UNorm(t) => {
                let m = self.texture_rgba16unorm.get(t.slot).expect("Texture not found");
                m.render_side()
            }
            InternalStaticTextureTicket::RGFloat(t) => {
                let m = self.texture_rgfloat.get(t.slot).expect("Texture not found");
                m.render_side()
            }
            InternalStaticTextureTicket::RGBA8Unorm(t) => {
                let m = self.texture_rgba8unorm.get(t.slot).expect("Texture not found");
                m.render_side()
            }
            InternalStaticTextureTicket::R8UNorm(t) => {
                let m = self.texture_r8unorm.get(t.slot).expect("Texture not found");
                m.render_side()
            }
            InternalStaticTextureTicket::BGRA8UnormSRGB(t) => {
                let m = self.texture_bgra8unorm_srgb.get(t.slot).expect("Texture not found");
                m.render_side()
            }
        }
    }


    pub(crate) fn new(bound_device: Arc<BoundDevice>) -> Self {
        PassClient {
            texture_r32float: SlotMap::new(),
            texture_r16float: SlotMap::new(),
            texture_rgba16unorm: SlotMap::new(),
            texture_rgfloat: SlotMap::new(),
            texture_rgba8unorm: SlotMap::new(),
            texture_r8unorm: SlotMap::new(),
            texture_bgra8unorm_srgb: SlotMap::new(),
            bound_device,
        }
    }

    pub fn bound_device(&self) -> &BoundDevice {
        &self.bound_device
    }
    pub fn bound_device_arc(&self) -> &Arc<BoundDevice> {
        &self.bound_device
    }
}

#[derive(Debug)]
pub struct Port {
    imp: crate::imp::Port,
    port_reporter: PortReporter,
    descriptors: Vec<PassDescriptor>,
    camera: Camera,
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
    fps: Arc<Mutex<i32>>,
    ms: Arc<Mutex<i32>>,
    cpu_ms: Arc<Mutex<i32>>,
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
    pub fn fps(&self) -> &Arc<Mutex<i32>> {
        &self.fps
    }
    pub fn ms(&self) ->&Arc<Mutex<i32>> {
        &self.ms
    }
    pub fn cpu_ms(&self) -> &Arc<Mutex<i32>> {
        &self.cpu_ms
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
}
impl PortReporterImpl {

}

#[derive(Debug)]
struct GPUFinishReporterImpl {
    fps_sender: Arc<Mutex<i32>>,
    gpu_time_sender: Arc<Mutex<i32>>,
    cpu_time_sender: Arc<Mutex<i32>>,
    //this is the time between end_frame calls.
    recent_elapsed: Mutex<Vec<f32>>,
    //because we need to store our elapsed frame in an atomic, we need to calculate it relative to some epoch.
    epoch: Instant,
    last_instant: Arc<Mutex<f32>>, //relative to epoch
}



/**
Special type that is moved into GPU completion blocks, typically wrapped in Arc.
*/
#[derive(Debug,Clone)]
pub(crate) struct GPUFinishReporter {
    imp: Arc<GPUFinishReporterImpl>,
    begin_frame: Instant,
    last_commit: Instant,
}
impl GPUFinishReporter {
    fn new(fps_sender: Arc<Mutex<i32>>, ms_sender: Arc<Mutex<i32>>,cpu_time_sender: Arc<Mutex<i32>>) -> Self {
        let recent_elapsed = Mutex::new(Vec::new());
        let epoch = Instant::now();
        let last_instant = Arc::new(Mutex::new(0.0));
        let commit = Instant::now();
        let imp = Arc::new(GPUFinishReporterImpl {
            fps_sender,
            gpu_time_sender: ms_sender,
            cpu_time_sender: cpu_time_sender,
            recent_elapsed,
            epoch,
            last_instant,
        });
        Self {
            imp,
            last_commit: commit,
            begin_frame: commit,
        }
    }
    pub(crate) fn begin_frame(&mut self) {
        self.begin_frame = Instant::now();
    }
    pub(crate) fn commit(&mut self) {
        self.last_commit = Instant::now();
        let begin_elapsed = self.last_commit.duration_since(self.begin_frame).as_micros() / (1000 * 10);
        *self.imp.cpu_time_sender.lock().unwrap() = begin_elapsed as i32;
    }
    pub(crate) fn end_frame(&self) {
        let now = Instant::now();
        let this_instant = now.duration_since(self.imp.epoch).as_secs_f32();
        let last_instant = {
            let mut lock = self.imp.last_instant.lock().unwrap();
            let last_instant = *lock;
            *lock = this_instant;
            last_instant
        };
        let elapsed = this_instant - last_instant;
        let mut lock = self.imp.recent_elapsed.lock().unwrap();

        let mut avg = 0.0;
        for i in &*lock {
            avg += i;
        }
        avg += elapsed;
        avg /= (lock.len() + 1) as f32;
        let fps = 1.0 / avg;
        lock.push(elapsed);
        while lock.len() > 60 {
            lock.remove(0);
        }
        *self.imp.fps_sender.lock().unwrap() = fps.round() as i32;

        let commit_elapsed = now.duration_since(self.last_commit).as_micros() / (1000 * 10);
        *self.imp.gpu_time_sender.lock().unwrap() = commit_elapsed as i32;
    }
}

#[derive(Debug)]
pub(crate) struct PortReporterSend {
    imp: Arc<PortReporterImpl>,
    finish_reporter: GPUFinishReporter,
}
impl PortReporterSend {
    pub(crate) fn begin_frame(&self, frame: u32) {
        self.imp.frame_begun.store(frame, Ordering::Relaxed);
    }
    //todo: read this, mt2-491
    #[allow(dead_code)]
    pub(crate) fn drawable_size(&self, size: (u16,u16)) {
        self.imp.drawable_size.store(u16s_to_u32(size.0, size.1), Ordering::Relaxed);
    }

    pub(crate) fn gpu_finisher(&self) -> GPUFinishReporter {
        self.finish_reporter.clone()
    }
}

fn port_reporter(initial_frame: u32, camera: &Camera) -> (PortReporterSend,PortReporter) {
    let fps = Arc::new(Mutex::new(0));
    let ms = Arc::new(Mutex::new(0));
    let cpu_ms = Arc::new(Mutex::new(0));
    let imp = Arc::new(PortReporterImpl {
        frame_begun: AtomicU32::new(initial_frame),
        drawable_size: AtomicU32::new(0),
    });
    (
        PortReporterSend {
            imp: imp.clone(),
            finish_reporter: GPUFinishReporter::new(fps.clone(), ms.clone(), cpu_ms.clone()),
        },
        PortReporter {
            imp,
            camera: camera.clone(),
            fps,
            ms,
            cpu_ms,
        }
    )

}


impl Port {
    pub fn new(engine: &Arc<Engine>, view: View, initial_camera_position: WorldCoord,window_size: (u16,u16)) -> Result<Self,Error> {
        let camera = Camera::new(window_size, initial_camera_position);
        let (port_sender,port_reporter) = port_reporter(0, &camera);

        Ok(Self{
            imp: crate::imp::Port::new(engine, view,  camera.clone(),port_sender).map_err(|e| Error(e))?,
            port_reporter,
            descriptors: Default::default(),
            camera,
        })
    }
    /**
    Adds a fixed pass to the port.

    # Limitations
    Currently, this doesn't work when the new port is running. mt2-242

    There is currently no way to remove a pass.  mt2-243

    */

    pub async fn add_fixed_pass<'s, const DESCRIPTORS: usize, P: PassTrait<DESCRIPTORS> + 'static>(&'s mut self, pass: P) -> P::DescriptorResult  {
        let (descriptors, result) = pass.into_descriptor(&mut self.imp.pass_client).await;
        for descriptor in descriptors {
            self.imp.add_fixed_pass(descriptor.clone()).await;
            self.descriptors.push(descriptor);
        }
        result
    }
    ///Start rendering on the port.  Ports are not rendered by default.
    pub async fn start(&mut self) -> Result<(),Error> {
        self.imp.render_frame().await;
        loop {
            //we need to figure out all the dirty stuff
            let mut dirty_receivers = Vec::new();
            for pass in &self.descriptors {
                for (_, bind) in &pass.bind_style.binds {
                    match &bind.target {
                        BindTarget::DynamicBuffer(a) => {
                            dirty_receivers.push(a.imp.dirty_receiver());
                        }
                        BindTarget::Camera => {
                            dirty_receivers.push(self.camera.dirty_receiver());
                        }
                        BindTarget::StaticBuffer(_) => { /* nothing to do, not considered dirty */}
                        BindTarget::FrameCounter => {/* nothing to do - not considered dirty */}
                        BindTarget::DynamicTexture(texture) => {
                            dirty_receivers.push(texture.gpu_dirty_receiver())
                        }
                        BindTarget::StaticTexture(_, _) => { /* also not considered dirty the 2nd+ time */}
                        BindTarget::Sampler(_) => { /* also not considered dirty */}
                        BindTarget::VB(..)  => { /* also not considered dirty */}
                    }
                }
            }
            let receiver = DirtyAggregateReceiver::new(dirty_receivers);
            receiver.wait_for_dirty().await;
            self.imp.render_frame().await;
        }
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