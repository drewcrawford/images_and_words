enum Impl {
    #[cfg(feature = "app_window")]
    AppWindow(app_window::surface::Surface),
}

pub struct View(Impl);
impl View {
    pub(crate) async fn size(&self) -> (u16,u16) {
        match &self.0 {
            #[cfg(feature = "app_window")]
            Impl::AppWindow(surface) => {
                let size = surface.size().await;
                (size.width() as u16,size.height() as u16)
            }
        }
    }

    /**
    Creates the view from an app_window surface.
    */
    #[cfg(feature = "app_window")]
    pub fn from_surface(surface: app_window::surface::Surface) -> Self {
        Self(Impl::AppWindow(surface))
    }
}