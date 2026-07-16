pub mod grammar;
pub mod jmdict;
pub mod jmnedict;
pub mod sentence;
pub mod labels;
pub mod llm;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;

use crate::labels::{PartOfSpeechSubcategory1, ParticleRole};
use crate::grammar::ProcToken;
use crate::sentence::{Chunk, Clause, Modifier, Sentence};

const SENTENCE_DELIMITERS: [u16; 4] = ['.' as u16, '。' as u16, '\n' as u16, '…' as u16];

async fn fetch_llm(prompt: &str) -> JsValue {
    let global = js_sys::global();
    let func_val = js_sys::Reflect::get(&global, &JsValue::from_str("__jongo_fetch_llm"))
        .unwrap_or(JsValue::UNDEFINED);
    
    let func: js_sys::Function = match func_val.dyn_into() {
        Ok(f) => f,
        Err(_) => {
            console::error_1(&"__jongo_fetch_llm is not defined on globalThis".into());
            return JsValue::NULL;
        }
    };
    
    let promise = match func.call1(&JsValue::NULL, &JsValue::from_str(prompt)) {
        Ok(p) => p,
        Err(e) => {
            console::error_1(&format!("LLM call error: {:?}", e).into());
            return JsValue::NULL;
        }
    };
    let promise: js_sys::Promise = match promise.dyn_into() {
        Ok(p) => p,
        Err(_) => return JsValue::NULL,
    };
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => val,
        Err(e) => {
            console::error_1(&format!("LLM fetch error: {:?}", e).into());
            JsValue::NULL
        }
    }
}

thread_local! {
    static CONTROLLER: RefCell<JongoController> = RefCell::new(JongoController::new());
}

struct AnalysisWindow {
    id: u32,
    element: web_sys::HtmlElement,
    _closures: Vec<Closure<dyn FnMut()>>,
    _mouse_closures: Vec<Closure<dyn FnMut(web_sys::MouseEvent)>>,
}

impl Drop for AnalysisWindow {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            if self._mouse_closures.len() >= 2 {
                let _ = window.remove_event_listener_with_callback("mousemove", self._mouse_closures[0].as_ref().unchecked_ref());
                let _ = window.remove_event_listener_with_callback("mouseup", self._mouse_closures[1].as_ref().unchecked_ref());
            }
        }
    }
}

struct JongoController {
    mouse_x: f32,
    mouse_y: f32,
    enabled: bool,
    prompt: Option<web_sys::HtmlElement>,
    analyses: Vec<AnalysisWindow>,
    next_id: u32,
}

impl JongoController {
    fn new() -> Self {
        Self {
            mouse_x: 0.0,
            mouse_y: 0.0,
            enabled: true,
            prompt: None,
            analyses: Vec::new(),
            next_id: 0,
        }
    }

    fn disable(&mut self) {
        if let Some(old) = self.prompt.take() {
            old.remove();
        }
        for a in self.analyses.drain(..) {
            a.element.remove();
        }
    }

    fn prompt(&mut self) {
        if !self.enabled {
            return;
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

        let mut block_text = String::new();
        for n in &text_nodes {
            block_text.push_str(&n.text_content().unwrap_or_default());
        }
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
        let context = block_text;
        let btn = element.query_selector("button").unwrap().unwrap();

        // closure that runs when jong is clicked
        let cb = Closure::<dyn FnMut()>::new(move || {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.analyze(&sentence, &context);
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

    fn analyze(&mut self, sentence: &str, context: &str) {
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
        element.style().set_property("z-index", "9999").unwrap();
        element.style().set_property("color", "black").unwrap();
        element.style().set_property("width", "680px").unwrap();
        element.style().set_property("height", "400px").unwrap();
        element.style().set_property("box-sizing", "border-box").unwrap();
        element.style().set_property("overflow", "hidden").unwrap();
        let tokens = grammar::analyze_sentence(sentence);
        let mut chunk_data: Vec<ChunkData> = Vec::new();
        let left = crate::sentence::build_sentence(tokens)
            .map(|s| render_structure(&s, &mut chunk_data))
            .unwrap_or_else(|| "<div>could not parse</div>".to_string());

        let chunk_data_rc = Rc::new(RefCell::new(chunk_data));

        let html = format!(
            "<style>\
             .jong-row{{cursor:pointer;border-radius:3px}}\
             .jong-row:hover{{background:#eef2f7}}\
             .jong-drag-handle{{cursor:move;padding:6px 28px 6px 10px;background:#f0f0f0;border-bottom:1px solid #ddd;font-size:11px;color:#666;user-select:none;flex-shrink:0}}\
             .jong-body{{display:flex;gap:16px;flex:1;min-height:0;padding:8px 8px 8px 0;box-sizing:border-box}}\
             .jong-structure-scroll{{direction:rtl;overflow-y:auto;flex:1;min-width:0;scrollbar-width:thin;scrollbar-color:#444 #e8e8e8;margin:0;user-select:none}}\
             .jong-structure-scroll::-webkit-scrollbar{{width:5px}}\
             .jong-structure-scroll::-webkit-scrollbar-thumb{{background:#444;border-radius:0}}\
             .jong-structure-scroll::-webkit-scrollbar-track{{background:#e8e8e8}}\
             .jong-structure-inner{{direction:ltr;padding:0 8px 0 6px}}\
             </style>\
             <div class='jong-drag-handle' style='height:8px'></div>\
             <button class='jong-close' style='position:absolute;top:4px;right:4px;background:red;color:white;border:none;cursor:pointer;padding:2px 6px;z-index:1'>✕</button>\
             <div class='jong-body'>\
               <div class='jong-structure-scroll'><div class='jong-structure-inner'>{left}</div></div>\
               <div class='jong-detail' style='flex:1;min-width:0;overflow-y:auto;border-left:1px solid #ddd;padding-left:12px'>\
                 <div style='color:#888;font-size:12px'>Click a word on the left to see details</div>\
               </div>\
             </div>"
        );
        element.set_inner_html(&html);
        element.style().set_property("display", "flex").unwrap();
        element.style().set_property("flex-direction", "column").unwrap();
        element.style().set_property("padding", "0").unwrap();

        // stop clicks inside from dismissing the prompt
        let stop_prop = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
            e.stop_propagation();
        });
        element.add_event_listener_with_callback("click", stop_prop.as_ref().unchecked_ref()).unwrap();
        stop_prop.forget();

        // delegated click: chunk row -> detail panel
        let detail_panel = element.query_selector(".jong-detail").unwrap().unwrap();
        let chunk_data_for_click = chunk_data_rc.clone();
        
        let detail_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            let Some(target) = e.target() else { return };
            let Ok(el) = target.dyn_into::<web_sys::Element>() else { return };
            let Ok(Some(row)) = el.closest("[data-chunk-id]") else { return };
            let Some(idx) = row
                .get_attribute("data-chunk-id")
                .and_then(|s| s.parse::<usize>().ok())
            else {
                return;
            };
            let cd = chunk_data_for_click.borrow();
            if let Some((word, particle, role, selected_def)) = cd.get(idx) {
                let detail_html = render_detail(word, particle.as_ref(), role.as_ref(), *selected_def);
                detail_panel.set_inner_html(&detail_html);
            }
        });
        element.add_event_listener_with_callback("click", detail_cb.as_ref().unchecked_ref()).unwrap();
        detail_cb.forget();

        // AI button click handler
        if let Some(ai_btn) = element.query_selector(".refine-ai-btn").unwrap() {
            if let Some(ast) = crate::sentence::build_sentence(grammar::analyze_sentence(sentence)) {
                let sentence_str = sentence.to_string();
                let context_str = context.to_string();
                let prompt = crate::llm::generate_prompt(&ast, &sentence_str, &context_str);
                let container = element.clone();
                let chunk_data_for_ai = chunk_data_rc.clone();
                
                let ai_cb = Closure::<dyn FnMut()>::new(move || {
                    let prompt = prompt.clone();
                    let container = container.clone();
                    let chunk_data_for_ai = chunk_data_for_ai.clone();
                    
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(btn) = container.query_selector(".refine-ai-btn").unwrap() {
                            let btn_html = btn.dyn_into::<web_sys::HtmlElement>().unwrap();
                            btn_html.set_inner_text("Loading...");
                            btn_html.style().set_property("pointer-events", "none").unwrap();
                            btn_html.style().set_property("opacity", "0.5").unwrap();
                            
                            let res = fetch_llm(&prompt).await;
                            if let Some(res_str) = res.as_string() {
                                // Strip markdown code fences if the LLM added them
                                let json_str = strip_code_fences(&res_str);
                                console::log_1(&format!("LLM response: {}", json_str).into());
                                match serde_json::from_str::<crate::llm::LlmResponse>(&json_str) {
                                    Ok(parsed) => {
                                        apply_llm_results(&container, parsed, &chunk_data_for_ai);
                                        btn_html.set_inner_text("Disambiguated");
                                    }
                                    Err(e) => {
                                        console::error_1(&format!("JSON parse error: {}", e).into());
                                        btn_html.set_inner_text("JSON Error");
                                    }
                                }
                            } else {
                                btn_html.set_inner_text("Setup Key in Popup");
                            }
                        }
                    });
                });
                ai_btn.add_event_listener_with_callback("click", ai_cb.as_ref().unchecked_ref()).unwrap();
                ai_cb.forget();
            }
        }

        // drag handle
        let dragging = Rc::new(RefCell::new(false));
        let drag_offset_x = Rc::new(RefCell::new(0.0_f64));
        let drag_offset_y = Rc::new(RefCell::new(0.0_f64));
        let element_drag = element.clone();
        let window_drag = window.clone();

        if let Ok(Some(handle)) = element.query_selector(".jong-drag-handle") {
            let dragging_down = Rc::clone(&dragging);
            let offset_x_down = Rc::clone(&drag_offset_x);
            let offset_y_down = Rc::clone(&drag_offset_y);
            let element_down = element.clone();

            let drag_start = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let rect = element_down.get_bounding_client_rect();
                *dragging_down.borrow_mut() = true;
                *offset_x_down.borrow_mut() = e.client_x() as f64 - rect.left();
                *offset_y_down.borrow_mut() = e.client_y() as f64 - rect.top();
                e.prevent_default();
            });
            handle
                .add_event_listener_with_callback("mousedown", drag_start.as_ref().unchecked_ref())
                .unwrap();
            drag_start.forget();
        }

        let dragging_move = Rc::clone(&dragging);
        let offset_x_move = Rc::clone(&drag_offset_x);
        let offset_y_move = Rc::clone(&drag_offset_y);
        let drag_move = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            if !*dragging_move.borrow() {
                return;
            }
            let scroll_x = window_drag.scroll_x().unwrap_or(0.0);
            let scroll_y = window_drag.scroll_y().unwrap_or(0.0);
            let left = e.client_x() as f64 - *offset_x_move.borrow() + scroll_x;
            let top = e.client_y() as f64 - *offset_y_move.borrow() + scroll_y;
            element_drag.style().set_property("left", &format!("{left}px")).unwrap();
            element_drag.style().set_property("top", &format!("{top}px")).unwrap();
        });
        window
            .add_event_listener_with_callback("mousemove", drag_move.as_ref().unchecked_ref())
            .unwrap();

        let dragging_up = Rc::clone(&dragging);
        let drag_end = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |_e: web_sys::MouseEvent| {
            *dragging_up.borrow_mut() = false;
        });
        window
            .add_event_listener_with_callback("mouseup", drag_end.as_ref().unchecked_ref())
            .unwrap();

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
            _mouse_closures: vec![drag_move, drag_end],
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

#[wasm_bindgen]
pub fn set_enabled(on: bool) {
    CONTROLLER.with(|c| {
        if let Ok(mut ctrl) = c.try_borrow_mut() {
            if !on {
                ctrl.disable();
            }
            ctrl.enabled = on;
        }
    });
}

fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    // Strip ```json ... ``` or ``` ... ```
    if trimmed.starts_with("```") {
        let without_open = if let Some(after_first_line) = trimmed.strip_prefix("```json") {
            after_first_line
        } else if let Some(after_first_line) = trimmed.strip_prefix("```") {
            after_first_line
        } else {
            return trimmed.to_string();
        };
        // Remove leading newline after opening fence
        let without_open = without_open.strip_prefix('\n').unwrap_or(without_open);
        // Remove closing fence
        if let Some(body) = without_open.strip_suffix("```") {
            return body.trim_end().to_string();
        }
        return without_open.to_string();
    }
    trimmed.to_string()
}

fn apply_llm_results(container: &web_sys::Element, response: crate::llm::LlmResponse, chunk_data: &Rc<RefCell<Vec<ChunkData>>>) {
    let mut elements_to_click = Vec::new();
    
    {
        let mut cd = chunk_data.borrow_mut();
        
        for dis in response.disambiguations {
            let idx = dis.chunk_id;
            
            if dis.disambiguation_type == "particle_role" {
                if let Some(role_str) = dis.result.as_str() {
                    // Parse the string back to an actual ParticleRole enum
                    if let Some(resolved_role) = ParticleRole::from_str(role_str) {
                        // Mutate the ChunkData to store the resolved role
                        if let Some(entry) = cd.get_mut(idx) {
                            entry.2 = Some(resolved_role.clone());
                        }
                        
                        // Update the badge in the DOM
                        if let Some(row) = container.query_selector(&format!("[data-chunk-id='{}']", idx)).unwrap() {
                            if let Some(badge) = row.query_selector(".ambiguous-badge").unwrap() {
                                badge.set_inner_html(resolved_role.badge());
                                badge.set_class_name("resolved-badge");
                                if let Ok(badge_html) = badge.dyn_into::<web_sys::HtmlElement>() {
                                    let _ = badge_html.style().set_property("border-color", "#4a9");
                                    let _ = badge_html.style().set_property("color", "#4a9");
                                    let _ = badge_html.style().set_property("font-weight", "600");
                                }
                            }
                        }
                    } else {
                        console::warn_1(&format!("LLM returned unknown role '{}' for chunk_id {}", role_str, idx).into());
                    }
                }
            } else if dis.disambiguation_type == "vocabulary" {
                if let Some(def_idx) = dis.result.as_i64() {
                    // Mutate the ChunkData to store the selected definition
                    if let Some(entry) = cd.get_mut(idx) {
                        entry.3 = Some(def_idx as usize);
                    }
                    
                    // Trigger a click on the row to refresh the detail panel
                    if let Some(row) = container.query_selector(&format!("[data-chunk-id='{}']", idx)).unwrap() {
                        if let Ok(row_html) = row.dyn_into::<web_sys::HtmlElement>() {
                            elements_to_click.push(row_html);
                        }
                    }
                }
            }
        }
    }
    
    for el in elements_to_click {
        el.click();
    }
}

type ChunkData = (ProcToken, Option<ProcToken>, Option<ParticleRole>, Option<usize>);

fn render_structure(sentence: &Sentence, chunk_data: &mut Vec<ChunkData>) -> String {
    let mut html = String::from("<div class='jong-structure' style='position:relative'>");
    
    html.push_str(
        "<div style='text-align:right;margin-bottom:8px'>\
         <button class='refine-ai-btn' style='\
            background:#f0f0f0; border:1px solid #ccc; border-radius:4px; padding:4px 8px; \
            font-size:11px; cursor:pointer; color:#333; font-weight:500;'>\
            Disambiguate\
         </button></div>"
    );

    for clause in &sentence.clauses {
        html.push_str(&render_clause(clause, chunk_data));
    }
    html.push_str("</div>");
    html
}

fn render_clause(clause: &Clause, chunk_data: &mut Vec<ChunkData>) -> String {
    let color = clause.relation.color();
    let label = clause.relation.label();
    let mut html = format!(
        "<div style='border:1px solid {color};border-radius:4px;margin-bottom:10px;padding:6px 8px'>\
         <div style='font-size:10px;color:{color};margin-bottom:6px'>{label}</div>"
    );
    html.push_str(&render_chunk_group(&clause.predicate, "", "", chunk_data));
    if let Some(conn) = &clause.connective {
        let id = chunk_data.len();
        chunk_data.push((conn.clone(), None, None, None));
        html.push_str(&format!(
            "<div class='jong-row' data-chunk-id='{id}' style='margin-top:6px;font-size:14px;font-weight:600;color:{color};display:inline-block;padding:2px 4px'>{}</div>",
            conn.full
        ));
    }
    html.push_str("</div>");
    html
}

fn render_chunk_group(chunk: &Chunk, prefix: &str, branch: &str, chunk_data: &mut Vec<ChunkData>) -> String {
    let mut html = String::from("<div>");
    
    let mod_child_prefix = if prefix.is_empty() && branch.is_empty() {
        String::new()
    } else if branch == "┌── " {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };
    
    for (i, modifier) in chunk.modifiers.iter().enumerate() {
        let mod_is_first = i == 0;
        let mod_branch = if mod_is_first { "┌── " } else { "├── " };
        html.push_str(&render_modifier(modifier, &mod_child_prefix, mod_branch, chunk_data));
    }
    html.push_str(&render_row(
        &chunk.word,
        chunk.particle.as_ref(),
        chunk.particle_role.as_ref(),
        prefix,
        branch,
        chunk_data,
    ));
    html.push_str("</div>");
    html
}

fn render_modifier(modifier: &Modifier, prefix: &str, branch: &str, chunk_data: &mut Vec<ChunkData>) -> String {
    match modifier {
        Modifier::AdjectiveChunk(chunk) => render_chunk_group(chunk, prefix, branch, chunk_data),
        Modifier::AdverbChunk(chunk) => render_chunk_group(chunk, prefix, branch, chunk_data),
        Modifier::NounChunk(chunk) => render_chunk_group(chunk, prefix, branch, chunk_data),
        Modifier::Limitation(chunk) => render_chunk_group(chunk, prefix, branch, chunk_data),
        Modifier::Quotation(sentence) => {
            let mut html = String::new();
            for c in &sentence.clauses {
                html.push_str(&render_clause(c, chunk_data));
            }
            html
        },
        Modifier::Clause(clause) => {
            render_chunk_group(&clause.predicate, prefix, branch, chunk_data)
        }
    }
}

fn render_row(
    word: &ProcToken,
    particle: Option<&ProcToken>,
    role: Option<&ParticleRole>,
    prefix: &str,
    branch: &str,
    chunk_data: &mut Vec<ChunkData>,
) -> String {
    let id = chunk_data.len();
    chunk_data.push((word.clone(), particle.cloned(), role.cloned(), None));

    let (size, color, weight) = if prefix.is_empty() && branch.is_empty() {
        ("13px", "#000", "600")
    } else {
        ("12px", "#555", "400")
    };
    
    let mut html = format!(
        "<div class='jong-row' data-chunk-id='{id}' style='font-size:{size};line-height:1.2;padding:0 4px'>\
         <span style='font-family:monospace;white-space:pre;color:#888'>{}{}</span>\
         <span style='font-weight:{weight};color:{color}'>{}</span>",
        prefix, branch, word.full
    );
    if let Some(p) = particle {
        html.push_str(&format!(" <span style='color:{color}'>{}</span>", p.full));
    }
    if let Some(r) = role {
        let is_ambig = matches!(r, ParticleRole::Ambiguous(_));
        let class = if is_ambig { "ambiguous-badge" } else { "resolved-badge" };
        html.push_str(&format!(
            " <span class='{}' style='font-size:10px;color:#666;border:1px solid #ccc;border-radius:3px;padding:0 4px'>{}</span>",
            class,
            r.badge()
        ));
    }
    html.push_str("</div>");
    html
}

fn render_detail(word: &ProcToken, particle: Option<&ProcToken>, role: Option<&ParticleRole>, selected_def: Option<usize>) -> String {
    let mut html = String::from("<div style='font-size:12px;line-height:1.6'>");

    html.push_str(&format!(
        "<div style='font-size:16px;font-weight:600;margin-bottom:4px'>{}</div>",
        word.full
    ));

    let is_proper_noun = word.sub1 == PartOfSpeechSubcategory1::ProperNoun;
    match crate::jmdict::lookup_first_result(&word.base, word.pos, is_proper_noun) {
        Some(hit) => {
            let type_hint = match hit.source {
                crate::jmdict::DictSource::JMnedict => format!(" [{}]", hit.noun_type.label()),
                crate::jmdict::DictSource::JMdict => String::new(),
            };
            html.push_str(&format!(
                "<div><span style='color:#888'>Reading:</span> {}</div>",
                hit.kana
            ));
            html.push_str(&format!(
                "<div><span style='color:#888'>Base:</span> {}</div>",
                word.base
            ));
            html.push_str(&format!(
                "<div><span style='color:#888'>POS:</span> {:?}</div>",
                word.pos
            ));
            
            html.push_str(&format!("<div style='margin-top:4px'><strong>Definitions:</strong>{}</div>", type_hint));
            html.push_str("<div style='max-height:150px;overflow-y:auto;background:#fafafa;border:1px solid #eee;border-radius:4px;padding:8px 8px 8px 24px;margin-top:2px'>");
            html.push_str("<ol style='margin:0;padding:0;color:#333'>");
            
            // If there's a selected def, maybe show a toggle
            let is_resolved = selected_def.is_some();
            
            for (i, def) in hit.glosses.iter().enumerate() {
                let is_correct = selected_def.map(|s| s == i).unwrap_or(false);
                let (display, weight, opacity) = if is_resolved && !is_correct {
                    ("none", "400", "0.5")
                } else if is_correct {
                    ("list-item", "700", "1")
                } else {
                    ("list-item", "400", "1")
                };
                
                html.push_str(&format!(
                    "<li class='def-item' data-idx='{}' style='margin-bottom:4px;font-weight:{};opacity:{};display:{}'>{}</li>", 
                    i, weight, opacity, display, def
                ));
            }
            html.push_str("</ol>");
            
            if is_resolved {
                html.push_str("<div style='margin-top:8px;font-size:11px;text-align:center'>");
                html.push_str("<button onclick='\
                    let items = this.parentElement.parentElement.querySelectorAll(\".def-item\");\
                    let isHidden = items[0].style.display === \"none\" || items[1]?.style.display === \"none\";\
                    for (let i=0; i<items.length; i++) { \
                        items[i].style.display = \"list-item\"; \
                        if (isHidden) { items[i].style.opacity = items[i].style.fontWeight === \"700\" ? \"1\" : \"0.5\"; } \
                        else if (items[i].style.fontWeight !== \"700\") { items[i].style.display = \"none\"; } \
                    }\
                    this.innerText = isHidden ? \"Hide other definitions\" : \"Show other definitions\";\
                ' style='background:none;border:none;color:#4a9;cursor:pointer;text-decoration:underline'>Show other definitions</button>");
                html.push_str("</div>");
            }
            
            html.push_str("</div>");
        }
        None => {
            html.push_str(&format!(
                "<div><span style='color:#888'>Base:</span> {}</div>",
                word.base
            ));
            html.push_str(&format!(
                "<div><span style='color:#888'>POS:</span> {:?}</div>",
                word.pos
            ));
            html.push_str("<div style='color:#888'>no dictionary entry</div>");
        }
    }

    if let Some(p) = particle {
        html.push_str("<div style='margin-top:10px;border-top:1px solid #eee;padding-top:6px'>");
        html.push_str(&format!(
            "<div style='font-weight:600'>Particle: {}</div>",
            p.full
        ));
        match role {
            Some(ParticleRole::Ambiguous(candidates)) => {
                html.push_str(
                    "<div style='color:#888'>Role is ambiguous — candidates:</div>\
                     <ul style='margin:4px 0 4px 16px;padding:0'>",
                );
                for c in candidates {
                    html.push_str(&format!(
                        "<li><strong>{}</strong> — {}</li>",
                        c.badge(),
                        c.explanation()
                    ));
                }
                html.push_str("</ul>");
                html.push_str(
                    "<div style='color:#a07d2a;font-size:11px'>Run AI analysis to resolve (coming soon)</div>",
                );
            }
            Some(r) => {
                html.push_str(&format!(
                    "<div><span style='color:#888'>Role:</span> {} — {}</div>",
                    r.badge(),
                    r.explanation()
                ));
            }
            None => {
                html.push_str("<div style='color:#888'>Role: unknown</div>");
            }
        }
        html.push_str("</div>");
    }

    if let Some(conj) = &word.conjugation {
        let mut flags: Vec<&str> = Vec::new();
        if conj.negative {
            flags.push("Negative");
        }
        if conj.past {
            flags.push("Past");
        }
        if conj.teform {
            flags.push("Te-form");
        }
        if !flags.is_empty() {
            html.push_str(&format!(
                "<div style='margin-top:10px;border-top:1px solid #eee;padding-top:6px'>\
                 <div style='font-weight:600'>Conjugation</div><div>{}</div></div>",
                flags.join(", ")
            ));
        }
    }

    html.push_str("</div>");
    html
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
        let name = node.node_name();
        if name == "RT" || name == "RP" || name == "rt" || name == "rp" {
            return;
        }
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