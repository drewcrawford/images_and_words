use std::collections::HashMap;
use std::fmt::Debug;
use crate::bindings::forward::dynamic::buffer::{ErasedRenderSide, RenderSide as DynamicRenderSide};
use crate::bindings::forward::dynamic::frame_texture::{ErasedTextureRenderSide,TextureRenderSide};
use crate::bindings::sampler::SamplerType;
use crate::bindings::forward::r#static::buffer::RenderSide as StaticBufferRenderSide;
/*
Defines the way resources are bound for a render pass.

This is a high-level description that does not always map to an underlying resource.
For example, the camera matrix is just a placeholder that is resolved later.
 */
#[derive(Debug,Clone)]
pub struct BindStyle {
    pub(crate) binds: HashMap<u32,BindInfo>,
    pub(crate) index_buffer: Option<StaticBufferRenderSide>,
}




#[derive(Debug,Clone)]
pub(crate) enum BindTarget {
    StaticBuffer(StaticBufferRenderSide),
    DynamicBuffer(ErasedRenderSide),
    Camera,
    FrameCounter,
    DynamicTexture(ErasedTextureRenderSide),
    #[allow(dead_code)] //nop implementation does not use
    StaticTexture(StaticTextureTicket, Option<SamplerType>),
    #[allow(dead_code)] //nop implementation does not use
    Sampler(SamplerType),
    #[allow(dead_code)] //nop implementation does not use
    VB(VertexLayout,StaticBufferRenderSide),
    #[allow(dead_code)] //nop implementation does not use
    DynamicVB(VertexLayout,ErasedRenderSide),
}

#[derive(Debug,Clone)]
pub struct BindInfo {
    #[allow(dead_code)] //nop implementation does not use
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
            index_buffer: None,
        }
    }

    fn bind(&mut self, slot: BindSlot, stage: Stage, target: BindTarget) {
        let old = self.binds.insert(slot.pass_index, BindInfo {
            stage,
            target,
        });
        assert!(old.is_none(), "Already bound to slot {:?}", slot);
    }


    ///Indicates we want to bind the camera matrix.  By default, we do not.
    ///
    /// This will be bound to the well-known slot position IMAGES_CAMERA_SLOT.
    pub fn bind_camera_matrix(&mut self, slot: BindSlot, stage: Stage) {
        self.bind(slot, stage, BindTarget::Camera);
    }


    /**
    Binds a framecounter to the specified slot.

    This will send a framecounter to your shader in the specified slot as a 32-bit unsigned integer.  It runs from \[0,max] inclusive.  After `max`, it will roll
    over back to 0.

     */
    pub fn bind_frame_counter(&mut self, slot: BindSlot, stage: Stage) {
        self.bind(slot, stage, BindTarget::FrameCounter);
    }

    pub fn bind_static_buffer(&mut self, slot: BindSlot, stage: Stage, render_side: StaticBufferRenderSide) {
        self.bind(slot, stage, BindTarget::StaticBuffer(render_side));
    }

    pub fn bind_dynamic_buffer<Element>(&mut self, slot: BindSlot, stage: Stage, render_side: DynamicRenderSide<Element>) where Element: Send + Sync + 'static {
        self.bind(slot, stage, BindTarget::DynamicBuffer(render_side.erased_render_side()));
    }

    pub fn bind_static_texture(&mut self, slot: BindSlot, stage: Stage, texture: StaticTextureTicket, sampler_type: Option<SamplerInfo>) {
        self.bind(slot, stage.clone(), BindTarget::StaticTexture(texture, sampler_type.as_ref().map(|x| x.sampler_type)));
        if let Some(sampler) = sampler_type {
            self.bind(BindSlot::new(sampler.pass_index), stage, BindTarget::Sampler(sampler.sampler_type));
        }
    }
    pub fn bind_dynamic_texture<Format>(&mut self, slot: BindSlot, stage: Stage, texture: TextureRenderSide<Format>) where Format: crate::pixel_formats::sealed::PixelFormat {
        self.bind(slot, stage, BindTarget::DynamicTexture(texture.erased()));
    }
    /**
    Binds a static buffer to the specified slot.

    Vertex buffers are separate from other buffers because they are bound differently.
    */
    pub fn bind_static_vertex_buffer(&mut self, slot: BindSlot, buffer: StaticBufferRenderSide, layout: VertexLayout) {
        self.bind(slot, Stage::Vertex, BindTarget::VB(layout, buffer));
    }

    /**
    Binds a dynamic buffer to the specified slot.

    Vertex buffers are separate from other buffers because they are bound differently.
    */
    pub fn bind_dynamic_vertex_buffer<Element>(&mut self, slot: BindSlot, buffer: DynamicRenderSide<Element>, layout: VertexLayout) where Element: Send + Sync + 'static {
        self.bind(slot, Stage::Vertex, BindTarget::DynamicVB(layout, buffer.erased_render_side()));
    }

    /**
    Binds a static index buffer to the specified slot.

    Index buffers are separate from other buffers because they are bound differently.
    */

    pub fn bind_static_index_buffer(&mut self, buffer: &crate::bindings::forward::r#static::buffer::Buffer<u16>) {
        self.index_buffer = Some(buffer.render_side())
    }
}

///A slot where we will bind something.
#[derive(Clone,Debug)]
pub enum Stage {
    ///bound to fragment shaders
    Fragment,
    ///Bound to vertex shaders
    Vertex,
}


#[derive(Clone,Debug)]
pub struct BindSlot {
    pub(crate) pass_index: u32,
}
impl BindSlot {
    pub fn new(pass_index: u32) -> Self {
        Self {
            pass_index,
        }
    }
}
use crate::images::StaticTextureTicket;
use crate::images::vertex_layout::VertexLayout;






