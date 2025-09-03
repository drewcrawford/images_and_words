fn request_animation_frame<F>(callback: F)
where
    F: FnOnce() + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        use web_sys::wasm_bindgen::JsCast;
        use web_sys::wasm_bindgen::closure::Closure;
        web_sys::window()
            .expect("No window found")
            .request_animation_frame(Closure::once_into_js(callback).as_ref().unchecked_ref())
            .expect("Failed to request animation frame");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        callback();
    }
}

pub async fn request_animation_frame_async<F, R>(callback: F) -> R
where
    F: FnOnce() -> R + 'static,
    R: Send + 'static,
{
    let (c, s) = r#continue::continuation();
    request_animation_frame(move || {
        let r = callback();
        c.send(r);
    });
    s.await
}
