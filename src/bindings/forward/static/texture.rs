use std::fmt::{Debug, Display};
use std::path::Path;
use std::sync::Arc;
use crate::bindings::visible_to::TextureUsage;
use crate::images::device::BoundDevice;
use crate::{imp, Priority};
use crate::pixel_formats::PixelFormat;

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
    pub async fn new(device: &Arc<BoundDevice>, width: u16, height: u16, data: &[Format::CPixel], visible_to: TextureUsage, debug_name: &str, priority: Priority) -> Result<Self,Error>  {
        Ok(Self(imp::Texture::new(device, width, height, visible_to,data,debug_name, priority).await?))
    }
    /**
    Create a texture, copying the data from the attached soft texture.
    */
    pub async fn from_software(device: &Arc<BoundDevice>, texture: &crate::bindings::software::texture::Texture<Format>, visible_to: TextureUsage, debug_name: &str, priority: Priority) -> Result<Self,Error> {
        Self::new(device, texture.width(), texture.height(), texture.texture_data(), visible_to, debug_name, priority).await
    }
    /**Create a texture from an asset of given path. */
    pub async fn new_asset(_path: &Path, _bound_device: &Arc<BoundDevice>, _visible_to: TextureUsage, _mipmaps: bool,_debug_name: &str, _priority: Priority) -> Result<Self,Error> {
        todo!()
    }
    pub async fn new_slice(_slice: &[u8], _bound_device: &Arc<BoundDevice>, _visible_to: TextureUsage, _mipmaps: bool, _debug_name: &str, _priority: Priority) -> Result<Self,Error> {
        todo!()
    }

}



