pub mod grammar;
pub mod jmdict;
pub mod jmnedict;
pub mod sentence;
pub mod labels;

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;

const SENTENCE_DELIMITERS: [u16; 4] = ['.' as u16, '。' as u16, '\n' as u16, '…' as u16];

thread_local! {
    static CONTROLLER: RefCell<JongoController> = RefCell::new(JongoController::new());
}

struct AnalysisWindow {
    id: u32,
    element: web_sys::HtmlElement,
    _closures: Vec<Closure<dyn FnMut()>>,
}

struct JongoController {
    mouse_x: f32,
    mouse_y: f32,
    prompt: Option<web_sys::HtmlElement>,
    analyses: Vec<AnalysisWindow>,
    next_id: u32,
}

impl JongoController {
    fn new() -> Self {
        Self { mouse_x: 0.0, mouse_y: 0.0, prompt: None, analyses: Vec::new(), next_id: 0 }
    }

    fn prompt(&mut self) {
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

        // initate new prompt 
        let element = document.create_element("div").unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();

        element.style().set_property("position", "absolute").unwrap();
        let scroll_x = window.scroll_x().unwrap_or(0.0);
        let scroll_y = window.scroll_y().unwrap_or(0.0);
        element.style().set_property("top", &format!("{}px", rect.bottom() + scroll_y)).unwrap();
        let left = (rect.left() + scroll_x).max(10.0);
        element.style().set_property("left", &format!("{}px", left)).unwrap();
        element.style().set_property("transform", "translateX(-50%)").unwrap();
        element.style().set_property("z-index", "9999").unwrap();
        element.style().set_property("background", "white").unwrap();
        element.style().set_property("border", "1px solid black").unwrap();
        element.style().set_property("padding", "10px").unwrap();
        element.style().set_property("color", "black").unwrap();

        element.set_inner_html("<button style='all:revert'>jong</button>");

        // delete old prompt
        if let Some(old) = self.prompt.take() {
            old.remove();
        }

        // spawn new prompt
        document.body().unwrap().append_child(&element).unwrap();

        // prevent clicks inside the prompt from dismissing it
        let stop_prop = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
            e.stop_propagation();
        });
        element.add_event_listener_with_callback("click", stop_prop.as_ref().unchecked_ref()).unwrap();
        stop_prop.forget();

        let sentence = sentence_str;
        let btn = element.query_selector("button").unwrap().unwrap();

        // closure that runs when jong is clicked
        let cb = Closure::<dyn FnMut()>::new(move || {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.analyze(&sentence);
                    if let Some(old) = ctrl.prompt.take() {
                        old.remove();
                    }
                }
            });
        });

        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).unwrap();
        cb.forget();

        self.prompt = Some(element);
    }

    fn analyze(&mut self, sentence: &str) {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        let prompt_rect = self.prompt.as_ref().unwrap().get_bounding_client_rect();

        let element = document.create_element("div").unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();
        element.style().set_property("position", "absolute").unwrap();
        let scroll_x = window.scroll_x().unwrap_or(0.0);
        let scroll_y = window.scroll_y().unwrap_or(0.0);
        element.style().set_property("top", &format!("{}px", prompt_rect.top() + scroll_y)).unwrap();
        element.style().set_property("left", &format!("{}px", prompt_rect.left() + scroll_x)).unwrap();
        element.style().set_property("background", "white").unwrap();
        element.style().set_property("border", "1px solid black").unwrap();
        element.style().set_property("padding", "10px").unwrap();
        element.style().set_property("z-index", "9999").unwrap();
        element.style().set_property("color", "black").unwrap();
        element.style().set_property("max-height", "300px").unwrap();
        element.style().set_property("overflow-y", "auto").unwrap();
        let results = grammar::analyze_sentence(sentence);

        let mut html = String::from("<button class='jong-close' style='position:absolute;top:4px;right:4px;background:red;color:white;border:none;cursor:pointer;padding:2px 6px'>✕</button>");
        for f in &results {
            let is_proper_noun = f.sub1 == crate::grammar::PartOfSpeechSubcategory1::ProperNoun;
            if let Some(hit) = crate::jmdict::lookup_first_result(&f.base, f.pos, is_proper_noun) {
                let type_hint = match hit.source {
                    crate::jmdict::DictSource::JMnedict => {
                        format!(" [{}]", hit.noun_type.label())
                    }
                    crate::jmdict::DictSource::JMdict => String::new(),
                };
                html.push_str(&format!(
                    "<div>
                        <strong style='font-weight: bold;'>{}</strong> ({}) {}{}
                    </div>",
                    f.full,
                    hit.kana,
                    hit.glosses.join(", "),
                    type_hint
                ));
            } else {
                html.push_str(&format!("{} - ?<br>", f.full));
            }
        }

        element.set_inner_html(&html);

        // stop clicks inside from dismissing the prompt
        let stop_prop = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
            e.stop_propagation();
        });
        element.add_event_listener_with_callback("click", stop_prop.as_ref().unchecked_ref()).unwrap();
        stop_prop.forget();

        document.body().unwrap().append_child(&element).unwrap();

        // close button
        let close_btn = element.query_selector(".jong-close").unwrap().unwrap();
        let element_clone = element.clone();
        let id = self.next_id;
        self.next_id += 1;
        let close_cb = Closure::<dyn FnMut()>::new(move || {
            element_clone.remove();
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.analyses.retain(|a| a.id != id);
                }
            });
        });
        close_btn.add_event_listener_with_callback("click", close_cb.as_ref().unchecked_ref()).unwrap();

        let analysis = AnalysisWindow {
            id,
            element,
            _closures: vec![close_cb],
        };
        self.analyses.push(analysis);

        console::log_1(&format!("Analyzing: {}", sentence).into());
    }
}


#[wasm_bindgen]
pub fn content_start() {
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();

    // mouse move
    let mouse_move_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
        CONTROLLER.with(|c| {
            let Ok(mut ctrl) = c.try_borrow_mut() else { return; };
            ctrl.mouse_x = e.client_x() as f32;
            ctrl.mouse_y = e.client_y() as f32;
        });
        if e.shift_key() {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.prompt();
                }
            });
        }
    });
    window
        .add_event_listener_with_callback("mousemove", mouse_move_cb.as_ref().unchecked_ref())
        .unwrap();
    mouse_move_cb.forget();

    // mouse click
    let mouse_click_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|_e: web_sys::MouseEvent| {
        CONTROLLER.with(|c| {
            if let Ok(mut ctrl) = c.try_borrow_mut() {
                if let Some(old) = ctrl.prompt.take() {
                    old.remove();
                }
            }
        });
    });
    window
        .add_event_listener_with_callback("click", mouse_click_cb.as_ref().unchecked_ref())
        .unwrap();
    mouse_click_cb.forget();

    // key press
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