use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;

thread_local! {
    static CONTROLLER: RefCell<JongoController> = RefCell::new(JongoController::new());
}

struct JongoController {
    mouse_x: f32,
    mouse_y: f32,
}

impl JongoController {
    fn new() -> Self {
        Self { mouse_x: 0.0, mouse_y: 0.0 }
    }

    fn on_mousemove(&mut self, x: i32, y: i32, shift_held: bool) {
        self.mouse_x = x as f32;
        self.mouse_y = y as f32;
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
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let Some(caret) = document.caret_position_from_point(self.mouse_x, self.mouse_y) else { return; };
        let Some(node) = caret.offset_node() else { return; };
        let offset = caret.offset();
        let text = node.text_content().unwrap_or_default();
        let Some(character) = text.chars().nth(offset as usize) else { return; };
        let Ok(range) = document.create_range() else { return; };

        let mut sentence_start = offset;
        let mut sentence_end = offset+1;
        let text_vec: Vec<u16> = text.encode_utf16().collect();
        for i in (0..offset+1).rev() {
            if [' ' as u16, '.' as u16].contains(&text_vec[i as usize]) {
                break;
            }
            sentence_start = i;
        }
        for i in (offset..(text_vec.len() as u32)) {
            if [' ' as u16, '.' as u16].contains(&text_vec[i as usize]) {
                break;
            }
            sentence_end = i;
        }
        if range.set_start(&node, sentence_start).is_err() { return; }
        if range.set_end(&node, sentence_end).is_err() { return; }
        let rect = range.get_bounding_client_rect();
        if self.mouse_x >= rect.left() as f32 && self.mouse_x <= rect.right() as f32
        && self.mouse_y >= rect.top() as f32 && self.mouse_y <= rect.bottom() as f32 {
            console::log_1(&format!("{}", character).into());
            let selection = window.get_selection().unwrap().unwrap();
            selection.remove_all_ranges();
            selection.add_range(&range);
        }
    }
}

#[wasm_bindgen]
pub fn content_start() {
    console_error_panic_hook::set_once();

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
