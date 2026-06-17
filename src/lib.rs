pub mod grammar;

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;

const SENTENCE_DELIMITERS: [u16; 4] = ['.' as u16, '。' as u16, '\n' as u16, '…' as u16];

thread_local! {
    static CONTROLLER: RefCell<JongoController> = RefCell::new(JongoController::new());
}

struct JongoController {
    mouse_x: f32,
    mouse_y: f32,
    prompt: Option<web_sys::HtmlElement>,
}

impl JongoController {
    fn new() -> Self {
        Self { mouse_x: 0.0, mouse_y: 0.0, prompt: None }
    }

    fn prompt(&mut self) {
        if let Some(old) = self.prompt.take() {
            old.remove();
        }
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let Some(caret) = document.caret_position_from_point(self.mouse_x, self.mouse_y) else { return; };
        let Some(node) = caret.offset_node() else { return; };
        let offset = caret.offset();
        let Ok(range) = document.create_range() else { return; };

        if range.set_start(&node, offset).is_err() { return; }
        if range.set_end(&node, offset + 1).is_err() { return; }
        let rect = range.get_bounding_client_rect();
        if self.mouse_x < rect.left() as f32 || self.mouse_x > rect.right() as f32
            || self.mouse_y < rect.top() as f32 || self.mouse_y > rect.bottom() as f32 {
            return;
        }

        let Some(block) = find_block_ancestor(&node) else { return; };

        let mut text_nodes = Vec::new();
        let mut absolute_offset = 0;
        let mut found_target = false;
        collect_text_nodes(&block, &node, &mut text_nodes, &mut absolute_offset, offset as usize, &mut found_target);

        let block_text = block.text_content().unwrap_or_default();
        let text_vec: Vec<u16> = block_text.encode_utf16().collect();
        if absolute_offset >= text_vec.len() { return; }


        let mut sentence_start = absolute_offset;
        for i in (0..=absolute_offset).rev() {
            if SENTENCE_DELIMITERS.contains(&text_vec[i]) {
                break;
            }
            sentence_start = i;
        }

        let mut sentence_end = absolute_offset;
        for i in absolute_offset..text_vec.len() {
            sentence_end = i + 1;
            if SENTENCE_DELIMITERS.contains(&text_vec[i]) {
                break;
            }
        }

        let Some((start_node, start_offset)) = map_offset_to_node(&text_nodes, sentence_start) else { return; };
        let Some((end_node, end_offset)) = map_offset_to_node(&text_nodes, sentence_end) else { return; };

        if range.set_start(&start_node, start_offset as u32).is_err() { return; }
        if range.set_end(&end_node, end_offset as u32).is_err() { return; }

        let selection = window.get_selection().unwrap().unwrap();
        let _ = selection.remove_all_ranges();
        let _ = selection.add_range(&range);

        let sentence_str = String::from_utf16(&text_vec[sentence_start..sentence_end]).unwrap_or_default();
        console::log_1(&format!("Sentence: {}", sentence_str).into());

<<<<<<< Updated upstream
        let tokens = grammar::PARSER.parse(&sentence_str).unwrap();
        tokens.iter().for_each(|x| console::log_1(&format!("{} {:?}", x.surface, x.pos).into()));
=======
        // spawn element
        let prompt = document.create_element("div").unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();

        prompt.style().set_property("position", "fixed").unwrap();
        prompt.style().set_property("top", &format!("{}px", rect.bottom())).unwrap();
        prompt.style().set_property("left", &format!("{}px", rect.left())).unwrap();
        prompt.style().set_property("background", "white").unwrap();
        prompt.style().set_property("border", "1px solid black").unwrap();
        prompt.style().set_property("padding", "10px").unwrap();
        prompt.style().set_property("z-index", "9999").unwrap();
        prompt.style().set_property("color", "black").unwrap();

        prompt.set_inner_html(&format!("<p>{}</p>", sentence_str));

        document.body().unwrap().append_child(&prompt).unwrap();
        self.prompt = Some(prompt);
    }

    fn analyze(&mut self) {
>>>>>>> Stashed changes
    }
}


#[wasm_bindgen]
pub fn content_start() {
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();

    let mouse_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
        let shift_held = e.shift_key();
        CONTROLLER.with(|c| {
            let Ok(mut ctrl) = c.try_borrow_mut() else { return; };
            ctrl.mouse_x = e.client_x() as f32;
            ctrl.mouse_y = e.client_y() as f32;
        });
        if shift_held {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.prompt();
                }
            });
        }
    });
    window
        .add_event_listener_with_callback("mousemove", mouse_cb.as_ref().unchecked_ref())
        .unwrap();
    mouse_cb.forget();

    let key_cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
        if e.key() == "Shift" {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.prompt();
                }
            });
        }
    });
    window
        .add_event_listener_with_callback("keydown", key_cb.as_ref().unchecked_ref())
        .unwrap();
    key_cb.forget();
}

fn find_block_ancestor(start_node: &web_sys::Node) -> Option<web_sys::Element> {
    let mut current = start_node.clone();
    
    loop {
        let parent = current.parent_node()?;
        
        if let Some(element) = parent.dyn_ref::<web_sys::Element>() {
            let tag = element.tag_name();
            
            if matches!(tag.as_str(), "P" | "DIV" | "LI" | "TD" | "H1" | "H2" | "H3" | "H4" | "H5" | "H6" | "SECTION" | "ARTICLE") {
                return Some(element.clone());
            }
        }
        
        current = parent;
    }
}

fn collect_text_nodes(
    node: &web_sys::Node,
    target_node: &web_sys::Node,
    text_nodes: &mut Vec<web_sys::Node>,
    absolute_offset: &mut usize,
    local_offset: usize,
    found_target: &mut bool,
) {
    if node.node_type() == web_sys::Node::TEXT_NODE {
        if node == target_node {
            *absolute_offset += local_offset;
            *found_target = true;
        } else if !*found_target {
            *absolute_offset += node.text_content().unwrap_or_default().encode_utf16().count();
        }
        text_nodes.push(node.clone());
    } else {
        let children = node.child_nodes();
        for i in 0..children.length() {
            if let Some(child) = children.item(i) {
                collect_text_nodes(&child, target_node, text_nodes, absolute_offset, local_offset, found_target);
            }
        }
    }
}

fn map_offset_to_node(text_nodes: &[web_sys::Node], mut target_offset: usize) -> Option<(web_sys::Node, usize)> {
    for node in text_nodes {
        let len = node.text_content().unwrap_or_default().encode_utf16().count();
        if target_offset <= len {
            return Some((node.clone(), target_offset));
        }
        target_offset -= len;
    }
    None
}