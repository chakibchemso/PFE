pub mod app;
pub mod components;
pub mod crypto;

// only on client side
// #[cfg(feature = "hydrate")]
// only on server side
#[cfg(feature = "ssr")]
pub mod mqtt;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
