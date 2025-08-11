fn request_animation_frame<F>(mut callback: F)
where
    F: FnOnce() + 'static,
{
    use web_sys::wasm_bindgen::JsCast;
    use web_sys::wasm_bindgen::closure::Closure;
    web_sys::window()
        .expect("No window found")
        .request_animation_frame(Closure::once_into_js(callback).as_ref().unchecked_ref())
        .expect("Failed to request animation frame");
}

pub async fn request_animation_frame_async<F, R>(mut callback: F) -> R
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
