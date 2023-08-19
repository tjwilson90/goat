use wasm_bindgen::prelude::*;

pub use client::*;
pub use game::*;
pub use one_action::*;

mod client;
mod game;
mod one_action;

#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}
