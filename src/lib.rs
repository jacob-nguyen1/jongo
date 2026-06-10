pub mod grammar;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn popup_start() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    let el = document.create_element("h1").unwrap();
    el.set_text_content(Some("Hello world"));
    body.append_child(&el).unwrap();
}
