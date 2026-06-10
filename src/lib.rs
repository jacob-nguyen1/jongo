use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;

thread_local! {
    static CONTROLLER: RefCell<JongoController> = RefCell::new(JongoController::new());
}

struct JongoController {
    mouse_x: i32,
    mouse_y: i32,
}

impl JongoController {
    fn new() -> Self {
        Self { mouse_x: 0, mouse_y: 0 }
    }

    fn on_mousemove(&mut self, x: i32, y: i32, shift_held: bool) {
        self.mouse_x = x;
        self.mouse_y = y;
        if shift_held {
            self.analyze();
        }
    }

    fn on_keydown(&self, key: &str) {
        if key == "Shift" {
            self.analyze();
        }
    }

    fn analyze(&self) {
        console::log_1(&format!("analyzing at {} {}", self.mouse_x, self.mouse_y).into());
    }
}

#[wasm_bindgen]
pub fn content_start() {
    let window = web_sys::window().unwrap();

    let mouse_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
        CONTROLLER.with(|c| {
            c.borrow_mut().on_mousemove(e.client_x(), e.client_y(), e.shift_key());
        });
    });
    window
        .add_event_listener_with_callback("mousemove", mouse_cb.as_ref().unchecked_ref())
        .unwrap();
    mouse_cb.forget();

    let key_cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
        if e.key() == "Shift" {
            CONTROLLER.with(|c| {
                c.borrow().on_keydown("Shift");
            });
        }
    });
    window
        .add_event_listener_with_callback("keydown", key_cb.as_ref().unchecked_ref())
        .unwrap();
    key_cb.forget();
}