use std::sync::Arc;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use crate::entry_point::EntryPoint;

#[derive(Debug)]
enum OSImpl {
    #[cfg(feature = "app_window")]
    AppWindow(app_window::surface::Surface, RawWindowHandle, RawDisplayHandle),
}
#[derive(thiserror::Error,Debug)]
pub struct Error(#[from] crate::imp::Error);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}",self.0)
    }
}

#[derive(Debug)]
pub struct View{
    os_impl: OSImpl,
    //late initialized once entrypoint is ready
    pub(crate) imp: Option<crate::imp::View>,
}

//we need this to port across to render thread
unsafe impl Send for View {

}

impl View {
    pub(crate) async fn provide_entry_point(&mut self, entry_point: &EntryPoint) -> Result<(),Error> {
        let (window_handle, display_handle): (RawWindowHandle, RawDisplayHandle) = match &self.os_impl {
            #[cfg(feature = "app_window")]
            OSImpl::AppWindow(_, window_handle, display_handle) => (*window_handle, *display_handle),
            _ => todo!(),
        };
        self.imp = Some(
            crate::imp::View::from_surface(entry_point, window_handle, display_handle).await?
        );
        Ok(())
    }
}

impl View {
    pub(crate) async fn size(&self) -> (u16,u16) {
        match &self.os_impl {
            #[cfg(feature = "app_window")]
            OSImpl::AppWindow(surface, _, _) => {
                let size = surface.size().await;
                (size.width() as u16,size.height() as u16)
            }
            _ => todo!(),
        }
    }

    /**
    Creates the view from an app_window surface.
    */
    #[cfg(feature = "app_window")]
    pub fn from_surface(surface: app_window::surface::Surface) -> Result<Self,Error> {
        let handle = surface.raw_window_handle();
        let display_handle = surface.raw_display_handle();
        Ok(View{
            os_impl: OSImpl::AppWindow(surface, handle, display_handle),
            imp: None,
        })
    }
}