use std::collections::HashMap;
use std::fmt::Debug;
use crate::bindings::forward::dynamic::buffer::{ErasedRenderSide, RenderSide as DynamicRenderSide};
use crate::bindings::forward::dynamic::frame_texture::{ErasedTextureRenderSide,TextureRenderSide};
use crate::bindings::sampler::SamplerType;
use crate::images::port::InstanceTicket;
/*
Defines the way resources are bound for a render pass.

This is a high-level description that does not always map to an underlying resource.
For example, the camera matrix is just a placeholder that is resolved later.
 */
#[derive(Debug,Clone)]
pub struct BindStyle {
    pub(crate) binds: HashMap<u32,BindInfo>,
}



#[derive(Debug,Clone)]
pub enum BindTarget {
    Buffer(ErasedRenderSide),
    Camera,
    FrameCounter,
    DynamicTexture(ErasedTextureRenderSide),
    StaticTexture(StaticTextureTicket, Option<SamplerType>),
    Sampler(SamplerType),
}

#[derive(Debug,Clone)]
pub struct BindInfo {
    pub(crate) stage: Stage,
    pub(crate) target: BindTarget,
}

#[derive(Debug)]
pub struct SamplerInfo {
    ///The slot to bind to.
    pub pass_index: u32,
    ///The sampler type to use.
    pub sampler_type: SamplerType,
}
impl BindStyle {
    pub fn new() -> Self {
        BindStyle{
            binds: HashMap::new(),
        }
    }

    fn bind(&mut self, slot: BindSlot,target: BindTarget) {
        let old = self.binds.insert(slot.pass_index, BindInfo {
            stage: slot.stage,
            target,
        });
        assert!(old.is_none(), "Already bound to slot {:?}", slot);
    }


    ///Indicates we want to bind the camera matrix.  By default, we do not.
    ///
    /// This will be bound to the well-known slot position IMAGES_CAMERA_SLOT.
    pub fn bind_camera_matrix(&mut self, slot: BindSlot) {
        self.bind(slot, BindTarget::Camera);
    }

    /**
    Binds a framecounter to the specified slot.

    This will send a framecounter to your shader in the specified slot as a 32-bit unsigned integer.  It runs from \[0,max] inclusive.  After `max`, it will roll
    over back to 0.

     */
    pub fn bind_frame_counter(&mut self, slot: BindSlot) {
        self.bind(slot, BindTarget::FrameCounter);
    }

    pub fn bind_dynamic_buffer<Element>(&mut self, slot: BindSlot, render_side: DynamicRenderSide<Element>) where Element: Send + Sync + 'static {
        self.bind(slot, BindTarget::Buffer(render_side.erased_render_side()));
    }

    pub fn bind_static_texture(&mut self, slot: BindSlot, texture: StaticTextureTicket, sampler_type: Option<SamplerInfo>) {
        self.bind(slot, BindTarget::StaticTexture(texture, sampler_type.as_ref().map(|x| x.sampler_type)));
        if let Some(sampler) = sampler_type {
            self.bind(BindSlot::new(slot.stage, sampler.pass_index), BindTarget::Sampler(sampler.sampler_type));
        }
    }
    pub fn bind_dynamic_texture<Format>(&mut self, slot: BindSlot, texture: TextureRenderSide<Format>) where Format: crate::pixel_formats::sealed::PixelFormat {
        self.bind(slot, BindTarget::DynamicTexture(texture.erased()));
    }

}

///A slot where we will bind something.
#[derive(Copy,Clone,Debug,Hash,Eq,PartialEq)]
pub enum Stage {
    ///bound to fragment shaders
    Fragment,
    ///Bound to vertex shaders
    Vertex,
}
#[derive(Copy,Clone,Debug)]
pub struct BindSlot {
    pub(crate) stage: Stage,
    pub(crate) pass_index: u32,
}
impl BindSlot {
    pub fn new(namespace: Stage, pass_index: u32) -> Self {
        Self {
            stage: namespace,
            pass_index,
        }
    }
}
use crate::images::StaticTextureTicket;
use crate::imp::{BindTargetBufferImp, PixelFormat};






