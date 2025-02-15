enum Impl {
    #[cfg(feature = "app_window")]
    AppWindow(app_window::surface::Surface),
}

pub struct View(Impl);
impl View {
    pub(crate) fn size(&self) -> (u16,u16) {
        todo!()
    }

    /**
    Creates the view from an app_window surface.
    */
    #[cfg(feature = "app_window")]
    pub fn from_surface(surface: app_window::surface::Surface) -> Self {
        Self(Impl::AppWindow(surface))
    }
}