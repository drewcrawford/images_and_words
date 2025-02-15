use crate::imp;
/**
This is a cross-platform concept of a 'surface', which is primarily an object used to pick the best Device.
*/
use std::fmt::Formatter;
use std::sync::Arc;
use crate::entry_point::EntryPoint;
use crate::images::view::View;

///Cross-platform surface strategy type.
#[derive(Clone)]
//assists in transmuting to child type
#[repr(transparent)]
pub struct SurfaceStrategy(
    imp::SurfaceStrategy,
);
impl SurfaceStrategy {
    #[cfg(target_os="windows")]
    pub const fn as_vulkan(&self) -> &imp::SurfaceStrategy {
        &self.0
    }
    pub const fn reinterpret(imp: &imp::SurfaceStrategy) -> &Self {
        //since we're repr-transparent
        unsafe{std::mem::transmute(imp)}
    }
}

/**
Wrapping type for View (some [Imageable]), implements additional I&W functionality on top of view.

* Needs to own an Imageable, so that implementors of Imageable can get Drop behavior to work as they expect
* and so Imageable interface can contain non-movable types.  For more details, see [Imageable].
* Needs to be a generic type to get the right Imageable
*/
#[derive(Debug)]
pub struct Surface(
    crate::imp::Surface,
);
impl Surface {
    ///Create a new surface
    pub fn new(view: View, entry_point: &Arc<EntryPoint>) -> Result<Self,Error> {
        let surface = crate::imp::Surface::new(view,entry_point)
            .map_err(|e| Error(e))?;
        Ok(Self(surface))
    }
    #[cfg(target_os="windows")]
    pub const fn as_vulkan(&self) -> &super::vulkan::surface::Surface {
        &self.0
    }
    #[cfg(target_os="macos")]
    pub const fn as_metal(&self) -> &crate::imp::Surface { &self.0 }
    #[cfg(target_os="macos")]
    pub fn as_metal_mut(&mut self) -> &mut crate::imp::Surface { &mut self.0 }
}

#[derive(Debug)]
pub struct Error(
    imp::Error,
);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::error::Error for Error {}

