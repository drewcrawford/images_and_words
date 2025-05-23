#[cfg(feature = "app_window")]
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
    #[allow(dead_code)] //nop implementation does not use
    os_impl: OSImpl,
    //late initialized once entrypoint is ready
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) imp: Option<crate::imp::View>,
}

//we need this to port across to render thread
unsafe impl Send for View {

}

impl View {
    pub(crate) async fn provide_entry_point(&mut self, _entry_point: &EntryPoint) -> Result<(),Error> {
        #[cfg(feature = "app_window")]
        {
            let (_window_handle, _display_handle): (RawWindowHandle, RawDisplayHandle) = match &self.os_impl {
                OSImpl::AppWindow(_, window_handle, display_handle) => (*window_handle, *display_handle),
            };
            self.imp = Some(
                crate::imp::View::from_surface(_entry_point, _window_handle, _display_handle).await?
            );
            return Ok(());
        }
        #[cfg(not(feature = "app_window"))]
        {
            todo!("app_window feature not enabled")
        }
    }
}

impl View {
    pub(crate) async fn size_scale(&self) -> (u16,u16,f64) {
        #[cfg(feature = "app_window")]
        {
            return match &self.os_impl {
                OSImpl::AppWindow(surface, _, _) => {
                    let (size,scale) = surface.size_scale().await;
                    (size.width() as u16,size.height() as u16, scale)
                }
            };
        }
        #[cfg(not(feature = "app_window"))]
        {
            todo!("app_window feature not enabled")
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