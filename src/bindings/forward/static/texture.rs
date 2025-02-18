use std::fmt::{Debug, Display};
use std::path::Path;
use std::sync::Arc;
use crate::bindings::visible_to::TextureUsage;
use crate::images::device::BoundDevice;
use crate::{imp, Priority};
use crate::bindings::software::texture::Texel;
use crate::bindings::software::texture::vtexture::VTexture;
use crate::pixel_formats::sealed::PixelFormat;

#[cfg(target_os = "windows")]
use crate::vulkan::forward::r#static::texture as imp;

/**
Cross-platform, forward, static texture.*/
#[derive(Debug)]
pub struct Texture<Format> (
    pub(crate) imp::Texture<Format>
);
#[derive(Debug,thiserror::Error)]
pub struct Error(
    #[from] imp::Error
);

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {:?}", self.0)
    }
}
impl<Format: PixelFormat> Texture<Format> {
    ///- data: Layout for metal, not sure what it is.
    pub async fn new<Initializer: Fn(Texel) -> Format::CPixel>(device: &Arc<BoundDevice>, width: u16, height: u16, visible_to: TextureUsage, debug_name: &str, priority: Priority, initialize_to: Initializer) -> Result<Self,Error>  {
        Ok(Self(imp::Texture::new(device, width, height, visible_to, debug_name, priority, initialize_to).await?))
    }
    /**
    Create a texture, copying the data from the attached soft texture.
    */
    pub async fn from_software(device: &Arc<BoundDevice>, texture: &crate::bindings::software::texture::Texture<Format>, visible_to: TextureUsage, debug_name: &str, priority: Priority) -> Result<Self,Error> {
        Self::new(device, texture.width(), texture.height(), visible_to, debug_name, priority, |texel| {
            texture.read(texel)
        }).await
    }
    /**Create a texture from an asset of given path. */
    pub async fn new_asset(_path: &Path, _bound_device: &Arc<BoundDevice>, _visible_to: TextureUsage, _mipmaps: bool,_debug_name: &str, _priority: Priority) -> Result<Self,Error> {
        todo!()
    }
    pub async fn new_slice(slice: &[Format::CPixel], width: u16, bound_device: &Arc<BoundDevice>, visible_to: TextureUsage, mipmaps: bool, debug_name: &str, priority: Priority) -> Result<Self,Error> {
        Self::new(bound_device, width, slice.len() as u16 / width, visible_to, debug_name, priority, |texel| {
            slice[texel.y as usize * width as usize + texel.x as usize].clone()
        }).await
    }

}



