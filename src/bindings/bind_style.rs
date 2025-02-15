use std::collections::HashMap;
use crate::bindings::forward::dynamic::frame_texture::TextureRenderSide;
use crate::bindings::forward::dynamic::buffer::RenderSide as DynamicRenderSide;
/*
Defines the way resources are bound for a render pass.
 */
#[derive(Debug)]
pub struct BindStyle {
    pub(crate) texture_style: TextureBindStyle,
    pub(crate) binds_camera_matrix: bool,
    pub(crate) frame_counter: Option<(BindSlot, u16)>,
    buffers: HashMap<u32,BufferInfo>,

}
#[derive(Debug)]
struct BufferInfo {
    slot: BindSlot,
    render_side: DynamicRenderSide,
}
impl BindStyle {
    pub fn new() -> Self {
        BindStyle{
            texture_style: TextureBindStyle::new(),
            binds_camera_matrix: false,
            frame_counter: None,
            buffers: HashMap::new(),
        }
    }

    pub fn texture_style_mut(&mut self) -> &mut TextureBindStyle {
        &mut self.texture_style
    }
    pub const fn texture_style(&self) -> &TextureBindStyle {
        &self.texture_style
    }

    ///Indicates we want to bind the camera matrix.  By default, we do not.
    ///
    /// This will be bound to the well-known slot position IMAGES_CAMERA_SLOT.
    pub fn bind_camera_matrix(&mut self) {
        self.binds_camera_matrix = true;
    }

    /**
    Binds a framecounter to the specified slot.

    This will send a framecounter to your shader in the specified slot as a 16-bit unsigned integer.  It runs from \[0,max] inclusive.  After `max`, it will roll
    over back to 0.

    This is primarily useful to do some small simulation on GPU, where you can extrapolate multiple frames from a single buffer.
     */
    pub fn bind_frame_counter(&mut self, slot: BindSlot, max: u16) {
        self.frame_counter = Some((slot, max));
    }

    pub fn bind_dynamic_buffer(&mut self, slot: BindSlot, render_side: DynamicRenderSide) {
        self.buffers.insert(slot.pass_index, BufferInfo {
            slot,
            render_side,
        });
    }

    pub(crate) fn buffers(&mut self) -> impl Iterator<Item=(BindSlot, &mut DynamicRenderSide)> {
        self.buffers.iter_mut().map(|i| (i.1.slot, &mut i.1.render_side))
    }
    #[cfg(target_os="windows")] //only used on windows
    pub(crate) fn buffer_len(&self) -> usize {
        self.buffers.len()
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
    //On metal, indexes are set per-stage.
    #[cfg(target_os = "macos")]
    pub(crate) stage_index: u8,
    //On vulkan, indexes are set per pass-descriptor.  In theory, vk supports millions of these per renderpass
    pub(crate) pass_index: u32,
}
impl BindSlot {
    pub fn new(namespace: Stage, pass_index: u32, _stage_index: u8) -> Self {
        Self {
            stage: namespace,
            #[cfg(target_os = "macos")]
            stage_index: _stage_index,
            pass_index,
        }
    }
}
use crate::images::StaticTextureTicket;
use crate::images::SamplerTicket;

#[derive(Debug,Clone)]
pub(crate) struct TextureBindInfo {
    pub(crate) slot: BindSlot,
    pub(crate) ticket: StaticTextureTicket,
    #[cfg(target_os = "windows")]
    pub(crate) sample_type: SampleType
}

#[derive(Debug)]
pub(crate) struct FrameBindInfo {
    pub(crate) slot: BindSlot,
    pub(crate) texture: TextureRenderSide,
}

/**
Specifies how (if at all) we sample the texture.

Note that this has no effect on Metal, where samplers are constants
in sourcecode.  It only has effects in Vulkan where samplers are managed
as CPU objects.
*/
#[derive(Debug,Clone)]
pub enum SampleType {
    /**
    The texture will not be sampled.*/
    None,
    /**
    Placeholder value.
*/
    Sample(SamplerTicket),
}



#[derive(Debug)]
pub struct TextureBindStyle {
    static_textures: HashMap<u32,TextureBindInfo>,
    frame_textures: HashMap<u32,FrameBindInfo>,
}

impl TextureBindStyle {
    fn new() -> Self {
       Self {
           static_textures: HashMap::new(),
           frame_textures: HashMap::new(),
       }
    }
    ///Bind into a texture/sampler slot.  You may bind textures or samplers.
    ///
    /// # Safety
    /// You solemnly swear you will use the texture only for reading.  This is not enforced by Metal; it is enforced by the vulkan
    /// validation layer possibly.
    ///
    /// I reserve the right to add a runtime check for this, in debug builds or whatever, although i have not done so yet.
    ///
    /// For more details on lifting this limit, see mt2-386
    pub unsafe fn bind_static_for_sample(&mut self, slot: BindSlot, ticket: StaticTextureTicket, _sample_type: SampleType) {
        let bind_info = TextureBindInfo {
            slot, ticket,
            #[cfg(target_os="windows")]
           sample_type: _sample_type,
        };
        assert!(!self.frame_textures.contains_key(&slot.pass_index));
        self.static_textures.insert(slot.pass_index, bind_info);
    }

    ///Bind into a texture/sampler slot.  Currently this only supports non-sampled textures.
    ///
    /// # Safety
    /// You solemnly swear you will use the texture only for reading.  This is not enforced by Metal; it is enforced by the vulkan
    /// validation layer possibly.
    pub unsafe fn bind_frame_for_sample(&mut self, slot: BindSlot, texture: TextureRenderSide) {
        let bind_info = FrameBindInfo {
            slot, texture,
        };
        assert!(!self.static_textures.contains_key(&slot.pass_index));
        self.frame_textures.insert(slot.pass_index, bind_info);
    }

    /**
    This iterates over the contents in an *arbitrary order*
*/
    pub(crate) fn static_textures(&self) -> impl Iterator<Item=&TextureBindInfo> + '_ {
        self.static_textures.iter().map(|i| i.1)
    }
    pub(crate) fn frame_textures_mut(&mut self) -> impl Iterator<Item=&mut FrameBindInfo> + '_ {
        self.frame_textures.iter_mut().map(|i| i.1)
    }
    pub(crate) fn frame_texture_len(&self) -> usize { self.frame_textures.len() }
    #[cfg(target_os = "windows")]
    pub(crate) fn static_texture_len(&self) -> usize { self.static_textures.len() }

}
