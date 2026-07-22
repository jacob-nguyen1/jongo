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

use crate::labels::{ClauseRelation, PartOfSpeechSubcategory1, ParticleRole};
use crate::grammar::ProcToken;
use crate::sentence::{Chunk, Clause, Modifier, Sentence};

const SENTENCE_DELIMITERS: [u16; 4] = ['.' as u16, '。' as u16, '\n' as u16, '…' as u16];
const MIN_WINDOW_WIDTH: f64 = 360.0;
const MIN_WINDOW_HEIGHT: f64 = 200.0;
const RESIZE_EDGE: f64 = 10.0;
const BASE_Z_INDEX: u32 = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResizeCorner {
    Nw,
    Ne,
    Sw,
    Se,
}

fn hit_resize_corner(el: &web_sys::HtmlElement, client_x: f64, client_y: f64) -> Option<ResizeCorner> {
    let rect = el.get_bounding_client_rect();
    let x = client_x - rect.left();
    let y = client_y - rect.top();
    let w = rect.width();
    let h = rect.height();
    let near_l = x <= RESIZE_EDGE;
    let near_r = x >= w - RESIZE_EDGE;
    let near_t = y <= RESIZE_EDGE;
    let near_b = y >= h - RESIZE_EDGE;
    match (near_l, near_r, near_t, near_b) {
        (true, _, true, _) => Some(ResizeCorner::Nw),
        (_, true, true, _) => Some(ResizeCorner::Ne),
        (true, _, _, true) => Some(ResizeCorner::Sw),
        (_, true, _, true) => Some(ResizeCorner::Se),
        _ => None,
    }
}

fn corner_cursor(corner: ResizeCorner) -> &'static str {
    match corner {
        ResizeCorner::Nw | ResizeCorner::Se => "nwse-resize",
        ResizeCorner::Ne | ResizeCorner::Sw => "nesw-resize",
    }
}

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

fn persist_dark_mode(on: bool) {
    let global = js_sys::global();
    let Ok(func_val) = js_sys::Reflect::get(&global, &JsValue::from_str("__jongo_set_dark_mode")) else {
        return;
    };
    let Ok(func) = func_val.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = func.call1(&JsValue::NULL, &JsValue::from_bool(on));
}

fn persist_setting(key: &str, val: &JsValue) {
    let global = js_sys::global();
    if let Ok(func_val) = js_sys::Reflect::get(&global, &JsValue::from_str("__jongo_save_setting")) {
        if let Ok(func) = func_val.dyn_into::<js_sys::Function>() {
            let _ = func.call2(&JsValue::NULL, &JsValue::from_str(key), val);
        }
    }
}

fn apply_theme(el: &web_sys::HtmlElement, dark: bool) {
    if dark {
        let _ = el.set_attribute("data-jong-dark", "1");
        let _ = el.style().remove_property("filter");
        let _ = el.style().set_property("background", "#3c3c3c");
        let _ = el.style().set_property("color", "#e8e8e8");
        let _ = el.style().set_property("border-color", "#666");
    } else {
        let _ = el.remove_attribute("data-jong-dark");
        let _ = el.style().remove_property("filter");
        let _ = el.style().set_property("background", "white");
        let _ = el.style().set_property("color", "black");
        let _ = el.style().set_property("border-color", "black");
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
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
    dark_mode: bool,
    furigana: bool,
    font_size: u32,
    prompt: Option<web_sys::HtmlElement>,
    analyses: Vec<AnalysisWindow>,
    next_id: u32,
    next_z: u32,
}

impl JongoController {
    fn new() -> Self {
        Self {
            mouse_x: 0.0,
            mouse_y: 0.0,
            enabled: true,
            dark_mode: false,
            furigana: true,
            font_size: 16,
            prompt: None,
            analyses: Vec::new(),
            next_id: 0,
            next_z: BASE_Z_INDEX,
        }
    }

    fn bring_to_front(&mut self, el: &web_sys::HtmlElement) {
        self.next_z += 1;
        let _ = el.style().set_property("z-index", &self.next_z.to_string());
    }

    fn apply_dark_mode_all(&self) {
        if let Some(prompt) = &self.prompt {
            apply_theme(prompt, self.dark_mode);
        }
        for a in &self.analyses {
            apply_theme(&a.element, self.dark_mode);
        }
    }

    fn set_dark_mode(&mut self, on: bool) {
        self.dark_mode = on;
        self.apply_dark_mode_all();
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

        element.set_inner_html("<button class='jong-prompt-btn' style='background:#fff;color:#333;border:1px solid #ccc;border-radius:4px;padding:2px 8px;font-size:12px;font-weight:500;cursor:pointer'>jong</button>");
        apply_theme(&element, self.dark_mode);

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
            .map(|s| render_structure(&s, sentence, &mut chunk_data))
            .unwrap_or_else(|| "<div>could not parse</div>".to_string());

        let chunk_data_rc = Rc::new(RefCell::new(chunk_data));

        let all_details = render_all_details(&chunk_data_rc.borrow());
        let analysis_body = format!(
            "<div class='jong-structure-scroll'><div class='jong-structure-inner'>{left}</div></div>\
             <div class='jong-detail'>{all_details}</div>"
        );
        let analysis_body_rc = Rc::new(analysis_body.clone());

        let html = format!(
            "<style>\
             .jong-row{{cursor:pointer;border-radius:3px;white-space:nowrap}}\
             .jong-row:hover{{background:#eef2f7}}\
             .jong-row-selected{{background:#dbeafe}}\
             .jong-top-bar{{display:flex;align-items:stretch;background:#f5f5f5;border-bottom:1px solid #e0e0e0;flex-shrink:0;height:32px}}\
             .jong-drag-handle{{flex:1;cursor:move;display:flex;align-items:center;padding:0 12px;user-select:none}}\
             .jong-legend{{background:#fefce8;color:#ca8a04;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 14px;display:flex;align-items:center;justify-content:center;transition:background 0.2s}}\
             .jong-legend:hover{{background:#fef9c3;color:#a16207}}\
             .jong-settings{{background:#f0f4f8;color:#3b82f6;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 14px;display:flex;align-items:center;justify-content:center;transition:background 0.2s}}\
             .jong-settings:hover{{background:#dbeafe;color:#2563eb}}\
             .jong-close{{background:#fef2f2;color:#ef4444;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 14px;display:flex;align-items:center;justify-content:center;transition:background 0.2s}}\
             .jong-close:hover{{background:#fee2e2;color:#dc2626}}\
             .jong-body{{display:flex;gap:16px;flex:1;min-height:0;padding:8px 8px 8px 0;box-sizing:border-box;overflow:hidden}}\
             .jong-structure-scroll{{direction:rtl;overflow-y:auto;overflow-x:hidden;flex:1;min-width:0;scrollbar-width:thin;scrollbar-color:#444 #e8e8e8;margin:0;user-select:none}}\
             .jong-structure-scroll::-webkit-scrollbar{{width:5px;height:5px}}\
             .jong-structure-scroll::-webkit-scrollbar-thumb{{background:#444;border-radius:0}}\
             .jong-structure-scroll::-webkit-scrollbar-track{{background:#e8e8e8}}\
             .jong-structure-inner{{direction:ltr;padding:0 8px 0 6px}}\
             .jong-detail{{flex:1;min-width:0;overflow-y:auto;border-left:1px solid #ddd;padding-left:12px;position:relative}}\
             .jong-muted{{color:#444}}\
             .jong-hint{{color:#555;font-size:11px}}\
             .jong-word-head{{font-weight:600;color:#111}}\
             .jong-word-mod{{font-weight:500;color:#111}}\
             .jong-tree-arm{{font-family:monospace;white-space:pre;color:#666}}\
             .jong-role-badge{{font-size:10px;color:#666;border:1px solid #ccc;border-radius:3px;padding:0 4px}}\
             .ambiguous-badge{{color:#b45309 !important;border-color:#f59e0b !important;background:#fffbeb !important;font-weight:600}}\
             .resolved-badge{{color:#15803d !important;border-color:#22c55e !important;background:#f0fdf4 !important;font-weight:600}}\
             .refine-ai-btn{{background:#f0f0f0;border:1px solid #ccc;border-radius:4px;padding:4px 8px;font-size:11px;cursor:pointer;color:#333;font-weight:500;flex-shrink:0}}\
             .jong-def-box{{max-height:150px;overflow-y:auto;background:#fafafa;border:1px solid #eee;border-radius:4px;padding:8px 8px 8px 24px;margin-top:2px}}\
             .jong-def-box ol{{margin:0;padding:0;color:#333}}\
             .jong-detail-section{{border-top:1px solid #eee}}\
             .jong-accordion{{border:1px solid #e0e0e0;border-radius:6px;margin-bottom:6px;overflow:hidden}}\
             .jong-accordion summary{{cursor:pointer;padding:6px 10px;font-size:13px;font-weight:600;background:#f8f9fa;list-style:none;display:flex;align-items:center;gap:6px;user-select:none}}\
             .jong-accordion summary::-webkit-details-marker{{display:none}}\
             .jong-accordion summary::before{{content:'▶';font-size:9px;transition:transform 0.15s;display:inline-block}}\
             .jong-accordion[open] summary::before{{transform:rotate(90deg)}}\
             .jong-accordion-body{{padding:6px 10px;font-size:12px;line-height:1.6}}\
             .jong-accordion-highlight{{border-color:#3b82f6;box-shadow:0 0 0 1px #3b82f6}}\
             .jong-panel{{flex:1;min-width:0;min-height:0;overflow-y:auto;padding:4px 12px 12px;font-size:12px;line-height:1.5;box-sizing:border-box;scrollbar-width:thin;scrollbar-color:#444 #e8e8e8}}\
             .jong-panel::-webkit-scrollbar{{width:5px}}\
             .jong-panel::-webkit-scrollbar-thumb{{background:#444;border-radius:0}}\
             .jong-panel::-webkit-scrollbar-track{{background:#e8e8e8}}\
             .jong-panel-section{{margin-top:14px}}\
             .jong-panel-section h3{{font-size:13px;margin:0 0 8px;border-bottom:1px solid #ddd;padding-bottom:4px}}\
             .jong-legend-row{{display:flex;gap:8px;align-items:flex-start;margin-bottom:6px}}\
             .jong-legend-badge{{flex-shrink:0;min-width:110px;font-size:10px;border:1px solid #ccc;border-radius:3px;padding:1px 6px;color:#444}}\
             .jong-settings-row{{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 12px;background:#f5f5f5;border-radius:8px}}\
             .jong-switch{{position:relative;display:inline-block;width:44px;height:24px;flex-shrink:0}}\
             .jong-switch input{{opacity:0;width:0;height:0}}\
             .jong-slider{{position:absolute;inset:0;background:#ccc;border-radius:24px;cursor:pointer;transition:background 0.2s}}\
             .jong-slider::before{{content:\"\";position:absolute;height:18px;width:18px;left:3px;bottom:3px;background:#fff;border-radius:50%;transition:transform 0.2s}}\
             .jong-switch input:checked + .jong-slider{{background:#4a9}}\
             .jong-switch input:checked + .jong-slider::before{{transform:translateX(20px)}}\
             .jong-back{{background:none;border:none;color:#4a9;cursor:pointer;font-size:12px;padding:0;margin-bottom:10px}}\
             [data-jong-dark=\"1\"] .jong-row:hover{{background:#4a4a4a}}\
             [data-jong-dark=\"1\"] .jong-row-selected{{background:#334155}}\
             [data-jong-dark=\"1\"] .jong-top-bar{{background:#2d2d2d;border-bottom-color:#444}}\
             [data-jong-dark=\"1\"] .jong-drag-handle{{color:#bbb}}\
             [data-jong-dark=\"1\"] .jong-legend{{background:#3b2d12;color:#facc15;border-left-color:#444}}\
             [data-jong-dark=\"1\"] .jong-legend:hover{{background:#543e17;color:#fde047}}\
             [data-jong-dark=\"1\"] .jong-settings{{background:#1e293b;color:#60a5fa;border-left-color:#444}}\
             [data-jong-dark=\"1\"] .jong-settings:hover{{background:#334155;color:#93c5fd}}\
             [data-jong-dark=\"1\"] .jong-close{{background:#451a1a;color:#f87171;border-left-color:#444}}\
             [data-jong-dark=\"1\"] .jong-close:hover{{background:#7f1d1d;color:#fca5a5}}\
             [data-jong-dark=\"1\"] .jong-detail{{border-left-color:#666}}\
             [data-jong-dark=\"1\"] .jong-muted{{color:#ddd}}\
             [data-jong-dark=\"1\"] .jong-hint{{color:#ccc}}\
             [data-jong-dark=\"1\"] .jong-word-head{{color:#f0f0f0}}\
             [data-jong-dark=\"1\"] .jong-word-mod{{color:#e0e0e0}}\
             [data-jong-dark=\"1\"] .jong-tree-arm{{color:#aaa}}\
             [data-jong-dark=\"1\"] .jong-role-badge{{color:#ccc;border-color:#777}}\
             [data-jong-dark=\"1\"] .refine-ai-btn{{background:#4a4a4a;border-color:#777;color:#e0e0e0}}\
             [data-jong-dark=\"1\"] .jong-def-box{{background:#333;border-color:#555}}\
             [data-jong-dark=\"1\"] .jong-def-box ol{{color:#e0e0e0}}\
             [data-jong-dark=\"1\"] .jong-detail-section{{border-top-color:#555}}\
             [data-jong-dark=\"1\"] .jong-accordion{{border-color:#555}}\
             [data-jong-dark=\"1\"] .jong-accordion summary{{background:#3a3a3a;color:#e0e0e0}}\
             [data-jong-dark=\"1\"] .jong-accordion-highlight{{border-color:#60a5fa;box-shadow:0 0 0 1px #60a5fa}}\
             [data-jong-dark=\"1\"] .jong-structure-scroll{{scrollbar-color:#888 #333}}\
             [data-jong-dark=\"1\"] .jong-structure-scroll::-webkit-scrollbar-thumb{{background:#888}}\
             [data-jong-dark=\"1\"] .jong-structure-scroll::-webkit-scrollbar-track{{background:#333}}\
             [data-jong-dark=\"1\"] .jong-panel{{scrollbar-color:#888 #333}}\
             [data-jong-dark=\"1\"] .jong-panel::-webkit-scrollbar-thumb{{background:#888}}\
             [data-jong-dark=\"1\"] .jong-panel::-webkit-scrollbar-track{{background:#333}}\
             [data-jong-dark=\"1\"] .jong-panel-section h3{{border-bottom-color:#666}}\
             [data-jong-dark=\"1\"] .jong-legend-badge{{border-color:#777;color:#ddd}}\
             [data-jong-dark=\"1\"] .jong-settings-row{{background:#4a4a4a}}\
             [data-jong-dark=\"1\"] .jong-slider{{background:#666}}\
             [data-jong-dark=\"1\"] .ambiguous-badge{{color:#fbbf24 !important;border-color:#d97706 !important;background:#451a03 !important}}\
             [data-jong-dark=\"1\"] .resolved-badge{{color:#4ade80 !important;border-color:#22c55e !important;background:#052e16 !important}}\
             </style>\
             <div class='jong-top-bar'>\
                <div class='jong-drag-handle'></div>\
                <button class='jong-legend' title='Legend'>\
                  <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'></circle><path d='M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3'></path><line x1='12' y1='17' x2='12.01' y2='17'></line></svg>\
                </button>\
                <button class='jong-settings' title='Settings'>\
                  <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='3'></circle><path d='M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z'></path></svg>\
                </button>\
                <button class='jong-close' title='Close'>\
                  <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><line x1='18' y1='6' x2='6' y2='18'></line><line x1='6' y1='6' x2='18' y2='18'></line></svg>\
                </button>\
              </div>\
              <div class='jong-body'>{analysis_body}</div>"
        );
        element.set_inner_html(&html);
        element.style().set_property("display", "flex").unwrap();
        element.style().set_property("flex-direction", "column").unwrap();
        element.style().set_property("padding", "0").unwrap();
        self.bring_to_front(&element);
        apply_theme(&element, self.dark_mode);

        // stop clicks inside from dismissing the prompt + bring to front
        let bring_el = element.clone();
        let stop_prop = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            e.stop_propagation();
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.bring_to_front(&bring_el);
                }
            });
        });
        element.add_event_listener_with_callback("mousedown", stop_prop.as_ref().unchecked_ref()).unwrap();
        stop_prop.forget();

        let stop_click = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
            e.stop_propagation();
        });
        element.add_event_listener_with_callback("click", stop_click.as_ref().unchecked_ref()).unwrap();
        stop_click.forget();

        // delegated click: chunk row -> detail panel (with toggle deselect)
        let detail_cb = {
            let container = element.clone();
            let chunk_data_for_click = chunk_data_rc.clone();
            let selected_id: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let Some(target) = e.target() else { return };
                let Ok(el) = target.dyn_into::<web_sys::Element>() else { return };
                let Ok(Some(row)) = el.closest("[data-chunk-id]") else { return };
                let Some(idx) = row
                    .get_attribute("data-chunk-id")
                    .and_then(|s| s.parse::<usize>().ok())
                else {
                    return;
                };
                let Ok(Some(detail_panel)) = container.query_selector(".jong-detail") else { return };

                // Clear previous selection highlight on tree rows
                let rows = container.query_selector_all(".jong-row-selected");
                if let Ok(list) = rows {
                    for i in 0..list.length() {
                        if let Some(node) = list.item(i) {
                            if let Ok(el) = node.dyn_into::<web_sys::Element>() {
                                let cls = el.get_attribute("class").unwrap_or_default();
                                let _ = el.set_attribute("class", &cls.replace(" jong-row-selected", ""));
                            }
                        }
                    }
                }

                // Clear accordion highlights
                let accords = container.query_selector_all(".jong-accordion-highlight");
                if let Ok(list) = accords {
                    for i in 0..list.length() {
                        if let Some(node) = list.item(i) {
                            if let Ok(el) = node.dyn_into::<web_sys::Element>() {
                                let cls = el.get_attribute("class").unwrap_or_default();
                                let _ = el.set_attribute("class", &cls.replace(" jong-accordion-highlight", ""));
                            }
                        }
                    }
                }

                // Toggle: if clicking the same row, deselect
                let mut sel = selected_id.borrow_mut();
                if *sel == Some(idx) {
                    *sel = None;
                    // Restore all details
                    let cd = chunk_data_for_click.borrow();
                    let all_html = render_all_details(&cd);
                    detail_panel.set_inner_html(&all_html);
                    return;
                }
                *sel = Some(idx);
                let cls = row.get_attribute("class").unwrap_or_default();
                let _ = row.set_attribute("class", &format!("{} jong-row-selected", cls));

                // Scroll to and highlight the matching panel
                let selector = format!("[data-detail-id='{}']", idx);
                if let Ok(Some(accordion)) = detail_panel.query_selector(&selector) {
                    let acc_cls = accordion.get_attribute("class").unwrap_or_default();
                    let _ = accordion.set_attribute("class", &format!("{} jong-accordion-highlight", acc_cls));
                    if let Ok(html_el) = accordion.dyn_into::<web_sys::HtmlElement>() {
                        if let Ok(detail_panel_html) = detail_panel.dyn_into::<web_sys::HtmlElement>() {
                            detail_panel_html.set_scroll_top(html_el.offset_top());
                        }
                    }
                }
            })
        };
        element.add_event_listener_with_callback("click", detail_cb.as_ref().unchecked_ref()).unwrap();
        detail_cb.forget();

        // AI button via delegation (survives panel swap / restore)
        if let Some(ast) = crate::sentence::build_sentence(grammar::analyze_sentence(sentence)) {
            let sentence_str = sentence.to_string();
            let context_str = context.to_string();
            let prompt = crate::llm::generate_prompt(&ast, &sentence_str, &context_str);
            let container = element.clone();
            let chunk_data_for_ai = chunk_data_rc.clone();

            let ai_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let Some(target) = e.target() else { return };
                let Ok(el) = target.dyn_into::<web_sys::Element>() else { return };
                let Ok(Some(btn)) = el.closest(".refine-ai-btn") else { return };

                let prompt = prompt.clone();
                let container = container.clone();
                let chunk_data_for_ai = chunk_data_for_ai.clone();
                let btn_html = btn.dyn_into::<web_sys::HtmlElement>().unwrap();

                wasm_bindgen_futures::spawn_local(async move {
                    btn_html.set_inner_text("Loading...");
                    btn_html.style().set_property("pointer-events", "none").unwrap();
                    btn_html.style().set_property("opacity", "0.5").unwrap();

                    let res = fetch_llm(&prompt).await;
                    if let Some(res_str) = res.as_string() {
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
                });
            });
            element.add_event_listener_with_callback("click", ai_cb.as_ref().unchecked_ref()).unwrap();
            ai_cb.forget();
        }

        // Panel swap: restore analysis body + wire Back on panels
        let wire_back_button = {
            let element_for_back = element.clone();
            let analysis_body_rc = analysis_body_rc.clone();
            Rc::new(move |body: web_sys::Element| {
                if let Ok(Some(back)) = body.query_selector(".jong-back") {
                    let element_for_back = element_for_back.clone();
                    let analysis_body_rc = analysis_body_rc.clone();
                    let back_cb = Closure::<dyn FnMut()>::new(move || {
                        if let Ok(Some(body)) = element_for_back.query_selector(".jong-body") {
                            body.set_inner_html(&analysis_body_rc);
                            if let Ok(html_body) = body.dyn_into::<web_sys::HtmlElement>() {
                                let _ = html_body.style().set_property("display", "flex");
                                let _ = html_body.style().set_property("flex-direction", "row");
                                let _ = html_body.style().set_property("overflow", "hidden");
                            }
                        }
                    });
                    back.add_event_listener_with_callback("click", back_cb.as_ref().unchecked_ref()).unwrap();
                    back_cb.forget();
                }
            })
        };

        // Settings button
        if let Ok(Some(settings_btn)) = element.query_selector(".jong-settings") {
            let element_settings = element.clone();
            let wire_back = wire_back_button.clone();
            let settings_cb = Closure::<dyn FnMut()>::new(move || {
                let (dark, furi, fsize) = CONTROLLER.with(|c| {
                    if let Ok(b) = c.try_borrow() {
                        (b.dark_mode, b.furigana, b.font_size)
                    } else {
                        (false, true, 16)
                    }
                });
                if let Ok(Some(body)) = element_settings.query_selector(".jong-body") {
                    body.set_inner_html(&render_settings_panel(dark, furi, fsize));
                    if let Ok(html_body) = body.clone().dyn_into::<web_sys::HtmlElement>() {
                        let _ = html_body.style().set_property("display", "flex");
                        let _ = html_body.style().set_property("flex-direction", "column");
                        let _ = html_body.style().set_property("overflow", "hidden");
                    }
                    wire_back(body.clone());

                    if let Ok(Some(toggle)) = body.query_selector("#jong-dark-toggle") {
                        let toggle_cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
                            let Some(target) = e.target() else { return };
                            let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() else { return };
                            let on = input.checked();
                            CONTROLLER.with(|c| {
                                if let Ok(mut ctrl) = c.try_borrow_mut() {
                                    ctrl.set_dark_mode(on);
                                }
                            });
                            persist_dark_mode(on);
                        });
                        toggle.add_event_listener_with_callback("change", toggle_cb.as_ref().unchecked_ref()).unwrap();
                        toggle_cb.forget();
                    }

                    if let Ok(Some(toggle)) = body.query_selector("#jong-furigana-toggle") {
                        let toggle_cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
                            let Some(target) = e.target() else { return };
                            let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() else { return };
                            let on = input.checked();
                            CONTROLLER.with(|c| {
                                if let Ok(mut ctrl) = c.try_borrow_mut() {
                                    ctrl.furigana = on;
                                }
                            });
                            persist_setting("furigana", &JsValue::from_bool(on));
                        });
                        toggle.add_event_listener_with_callback("change", toggle_cb.as_ref().unchecked_ref()).unwrap();
                        toggle_cb.forget();
                    }

                    if let Ok(Some(input_el)) = body.query_selector("#jong-font-size-input") {
                        let input_cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
                            let Some(target) = e.target() else { return };
                            let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() else { return };
                            if let Ok(val) = input.value().parse::<u32>() {
                                CONTROLLER.with(|c| {
                                    if let Ok(mut ctrl) = c.try_borrow_mut() {
                                        ctrl.font_size = val;
                                    }
                                });
                                persist_setting("fontSize", &JsValue::from_f64(val as f64));
                            }
                        });
                        input_el.add_event_listener_with_callback("change", input_cb.as_ref().unchecked_ref()).unwrap();
                        input_cb.forget();
                    }
                }
            });
            settings_btn.add_event_listener_with_callback("click", settings_cb.as_ref().unchecked_ref()).unwrap();
            settings_cb.forget();
        }

        // Legend button
        if let Ok(Some(legend_btn)) = element.query_selector(".jong-legend") {
            let element_legend = element.clone();
            let wire_back = wire_back_button.clone();
            let legend_cb = Closure::<dyn FnMut()>::new(move || {
                if let Ok(Some(body)) = element_legend.query_selector(".jong-body") {
                    body.set_inner_html(&render_legend_panel());
                    if let Ok(html_body) = body.clone().dyn_into::<web_sys::HtmlElement>() {
                        let _ = html_body.style().set_property("display", "flex");
                        let _ = html_body.style().set_property("flex-direction", "column");
                        let _ = html_body.style().set_property("overflow", "hidden");
                    }
                    wire_back(body);
                }
            });
            legend_btn.add_event_listener_with_callback("click", legend_cb.as_ref().unchecked_ref()).unwrap();
            legend_cb.forget();
        }

        // drag + corner resize (no visible handles)
        let dragging = Rc::new(RefCell::new(false));
        let resizing = Rc::new(RefCell::new(Option::<ResizeCorner>::None));
        let drag_offset_x = Rc::new(RefCell::new(0.0_f64));
        let drag_offset_y = Rc::new(RefCell::new(0.0_f64));
        // start_cx, start_cy, start_left, start_top, start_w, start_h (left/top in document coords)
        let resize_start = Rc::new(RefCell::new((0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64)));
        let element_drag = element.clone();
        let window_drag = window.clone();

        // Corner cursor feedback while hovering
        {
            let el = element.clone();
            let resizing_ref = Rc::clone(&resizing);
            let dragging_ref = Rc::clone(&dragging);
            let cursor_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                if resizing_ref.borrow().is_some() || *dragging_ref.borrow() {
                    return;
                }
                if let Some(corner) = hit_resize_corner(&el, e.client_x() as f64, e.client_y() as f64) {
                    let _ = el.style().set_property("cursor", corner_cursor(corner));
                } else {
                    let _ = el.style().set_property("cursor", "default");
                }
            });
            element
                .add_event_listener_with_callback("mousemove", cursor_cb.as_ref().unchecked_ref())
                .unwrap();
            cursor_cb.forget();
        }

        // mousedown on window: start corner resize if near a corner
        {
            let resizing_down = Rc::clone(&resizing);
            let dragging_down = Rc::clone(&dragging);
            let resize_start_down = Rc::clone(&resize_start);
            let element_down = element.clone();
            let window_down = window.clone();

            let resize_mousedown = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let Some(corner) = hit_resize_corner(&element_down, e.client_x() as f64, e.client_y() as f64) else {
                    return;
                };
                *dragging_down.borrow_mut() = false;
                *resizing_down.borrow_mut() = Some(corner);
                let rect = element_down.get_bounding_client_rect();
                let scroll_x = window_down.scroll_x().unwrap_or(0.0);
                let scroll_y = window_down.scroll_y().unwrap_or(0.0);
                *resize_start_down.borrow_mut() = (
                    e.client_x() as f64,
                    e.client_y() as f64,
                    rect.left() + scroll_x,
                    rect.top() + scroll_y,
                    rect.width(),
                    rect.height(),
                );
                let _ = element_down.style().set_property("cursor", corner_cursor(corner));
                e.prevent_default();
            });
            element
                .add_event_listener_with_callback("mousedown", resize_mousedown.as_ref().unchecked_ref())
                .unwrap();
            resize_mousedown.forget();
        }

        if let Ok(Some(handle)) = element.query_selector(".jong-drag-handle") {
            let dragging_down = Rc::clone(&dragging);
            let resizing_down = Rc::clone(&resizing);
            let offset_x_down = Rc::clone(&drag_offset_x);
            let offset_y_down = Rc::clone(&drag_offset_y);
            let element_down = element.clone();

            let drag_start = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                if resizing_down.borrow().is_some() {
                    return;
                }
                // Prefer corner resize over drag when near a corner of the handle
                if hit_resize_corner(&element_down, e.client_x() as f64, e.client_y() as f64).is_some() {
                    return;
                }
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
        let resizing_move = Rc::clone(&resizing);
        let offset_x_move = Rc::clone(&drag_offset_x);
        let offset_y_move = Rc::clone(&drag_offset_y);
        let resize_start_move = Rc::clone(&resize_start);
        let drag_move = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            if let Some(corner) = *resizing_move.borrow() {
                let (start_cx, start_cy, start_left, start_top, start_w, start_h) = *resize_start_move.borrow();
                let dx = e.client_x() as f64 - start_cx;
                let dy = e.client_y() as f64 - start_cy;
                let fixed_right = start_left + start_w;
                let fixed_bottom = start_top + start_h;

                let (new_left, new_top, new_w, new_h) = match corner {
                    ResizeCorner::Se => (
                        start_left,
                        start_top,
                        (start_w + dx).max(MIN_WINDOW_WIDTH),
                        (start_h + dy).max(MIN_WINDOW_HEIGHT),
                    ),
                    ResizeCorner::Sw => {
                        let new_w = (start_w - dx).max(MIN_WINDOW_WIDTH);
                        (
                            fixed_right - new_w,
                            start_top,
                            new_w,
                            (start_h + dy).max(MIN_WINDOW_HEIGHT),
                        )
                    }
                    ResizeCorner::Ne => {
                        let new_h = (start_h - dy).max(MIN_WINDOW_HEIGHT);
                        (
                            start_left,
                            fixed_bottom - new_h,
                            (start_w + dx).max(MIN_WINDOW_WIDTH),
                            new_h,
                        )
                    }
                    ResizeCorner::Nw => {
                        let new_w = (start_w - dx).max(MIN_WINDOW_WIDTH);
                        let new_h = (start_h - dy).max(MIN_WINDOW_HEIGHT);
                        (fixed_right - new_w, fixed_bottom - new_h, new_w, new_h)
                    }
                };

                element_drag.style().set_property("left", &format!("{new_left}px")).unwrap();
                element_drag.style().set_property("top", &format!("{new_top}px")).unwrap();
                element_drag.style().set_property("width", &format!("{new_w}px")).unwrap();
                element_drag.style().set_property("height", &format!("{new_h}px")).unwrap();
                return;
            }
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
        let resizing_up = Rc::clone(&resizing);
        let element_up = element.clone();
        let drag_end = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |_e: web_sys::MouseEvent| {
            *dragging_up.borrow_mut() = false;
            *resizing_up.borrow_mut() = None;
            let _ = element_up.style().set_property("cursor", "default");
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

#[wasm_bindgen]
pub fn set_dark_mode(on: bool) {
    CONTROLLER.with(|c| {
        if let Ok(mut ctrl) = c.try_borrow_mut() {
            ctrl.set_dark_mode(on);
        }
    });
}

#[wasm_bindgen]
pub fn set_furigana(on: bool) {
    CONTROLLER.with(|c| {
        if let Ok(mut ctrl) = c.try_borrow_mut() {
            ctrl.furigana = on;
        }
    });
}

#[wasm_bindgen]
pub fn set_font_size(size: u32) {
    CONTROLLER.with(|c| {
        if let Ok(mut ctrl) = c.try_borrow_mut() {
            ctrl.font_size = size;
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
                            entry.3 = Some(resolved_role.clone());
                        }
                        
                        // Update the badge in the DOM
                        if let Some(row) = container.query_selector(&format!("[data-chunk-id='{}']", idx)).unwrap() {
                            if let Some(badge) = row.query_selector(".ambiguous-badge").unwrap() {
                                badge.set_inner_html(resolved_role.badge());
                                badge.set_class_name("resolved-badge jong-role-badge");
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
                        entry.4 = Some(def_idx as usize);
                    }
                }
            }
        }
    }
    
    // Re-render the right sidebar to reflect the new disambiguated data
    if let Ok(Some(detail_panel)) = container.query_selector(".jong-detail") {
        let cd = chunk_data.borrow();
        let all_html = render_all_details(&cd);
        detail_panel.set_inner_html(&all_html);
        
        // Re-apply highlight and scroll if a row is currently selected
        if let Ok(Some(selected_row)) = container.query_selector(".jong-row-selected") {
            if let Some(idx_str) = selected_row.get_attribute("data-chunk-id") {
                let selector = format!("[data-detail-id='{}']", idx_str);
                if let Ok(Some(accordion)) = detail_panel.query_selector(&selector) {
                    let acc_cls = accordion.get_attribute("class").unwrap_or_default();
                    let _ = accordion.set_attribute("class", &format!("{} jong-accordion-highlight", acc_cls));
                    if let Ok(html_el) = accordion.dyn_into::<web_sys::HtmlElement>() {
                        if let Ok(detail_panel_html) = detail_panel.dyn_into::<web_sys::HtmlElement>() {
                            detail_panel_html.set_scroll_top(html_el.offset_top());
                        }
                    }
                }
            }
        }
    }
}

type ChunkData = (ProcToken, Option<ProcToken>, Option<ProcToken>, Option<ParticleRole>, Option<usize>);

fn render_structure(sentence: &Sentence, sentence_str: &str, chunk_data: &mut Vec<ChunkData>) -> String {
    let mut html = String::from("<div class='jong-structure' style='position:relative'>");
    
    let escaped = html_escape(sentence_str);
    html.push_str(&format!(
        "<div style='display:flex;align-items:flex-start;justify-content:space-between;gap:8px;margin-bottom:8px'>\
         <div style='font-size:13px;font-weight:600;line-height:1.4;flex:1;min-width:0;word-break:break-word'>{escaped}</div>\
         <button class='refine-ai-btn'>Disambiguate</button></div>"
    ));

    html.push_str("<div class='jong-tree-scroll-wrapper' style='overflow-x:auto; padding-bottom: 8px;'>");
    html.push_str("<div class='jong-tree-container' style='display:flex;flex-direction:column;width:max-content;min-width:100%'>");
    for clause in &sentence.clauses {
        html.push_str(&render_clause(clause, chunk_data));
    }
    html.push_str("</div></div>");
    html.push_str("</div>");
    html
}

fn render_settings_panel(dark_mode: bool, furigana: bool, font_size: u32) -> String {
    let checked_dark = if dark_mode { " checked" } else { "" };
    let checked_furi = if furigana { " checked" } else { "" };
    format!(
        "<div class='jong-panel'>\
           <button class='jong-back'>← Back</button>\
           <div style='font-size:14px;font-weight:600;margin-bottom:12px'>Settings</div>\
           <div class='jong-settings-row' style='margin-bottom:8px'>\
             <div>\
               <div style='font-weight:500'>Dark mode</div>\
               <div class='jong-hint' style='margin-top:2px'>Applies to Jongo windows only</div>\
             </div>\
             <label class='jong-switch' title='Toggle dark mode'>\
               <input type='checkbox' id='jong-dark-toggle'{checked_dark} />\
               <span class='jong-slider'></span>\
             </label>\
           </div>\
           <div class='jong-settings-row' style='margin-bottom:8px'>\
             <div>\
               <div style='font-weight:500'>Furigana</div>\
               <div class='jong-hint' style='margin-top:2px'>Show readings above kanji</div>\
             </div>\
             <label class='jong-switch' title='Toggle furigana'>\
               <input type='checkbox' id='jong-furigana-toggle'{checked_furi} />\
               <span class='jong-slider'></span>\
             </label>\
           </div>\
           <div class='jong-settings-row'>\
             <div>\
               <div style='font-weight:500'>Font Size</div>\
               <div class='jong-hint' style='margin-top:2px'>Base font size for structure view (px)</div>\
             </div>\
             <input type='number' id='jong-font-size-input' value='{font_size}' min='10' max='32' style='width:50px;padding:4px;border:1px solid #ccc;border-radius:4px' />\
           </div>\
         </div>"
    )
}

fn render_legend_panel() -> String {
    let mut html = String::from(
        "<div class='jong-panel'>\
           <button class='jong-back'>← Back</button>\
           <div style='font-size:14px;font-weight:600;margin-bottom:4px'>Legend</div>\
           <div class='jong-hint' style='margin-bottom:8px'>Reference for labels used in analysis</div>"
    );

    html.push_str("<div class='jong-panel-section'><h3>Particle roles</h3>");
    for role in ParticleRole::all() {
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='jong-legend-badge'>{}</span>\
               <span>{}</span>\
             </div>",
            role.badge(),
            role.explanation()
        ));
    }
    html.push_str("</div>");

    html.push_str("<div class='jong-panel-section'><h3>Clause relations</h3>");
    for rel in ClauseRelation::all() {
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='jong-legend-badge' style='border-color:{color};color:{color}'>{label}</span>\
               <span>{explanation}</span>\
             </div>",
            color = rel.color(),
            label = rel.label(),
            explanation = rel.explanation()
        ));
    }
    html.push_str("</div>");

    html.push_str("<div class='jong-panel-section'><h3>Verb conjugations</h3>");
    const CONJUGATIONS: &[(&str, &str)] = &[
        ("Negative", "Negates the action or state (〜ない)."),
        ("Past", "Marks completed or past tense (〜た / 〜だ)."),
        ("Continuous", "Ongoing or resulting state (〜ている)."),
        ("Te-form", "Connective form used for sequences and requests (〜て)."),
        ("Desiderative", "Expresses desire to do something (〜たい)."),
        ("Volitional", "Expresses intention or suggestion (〜よう / 〜う)."),
        ("Potential", "Ability or possibility (〜られる / できる)."),
        ("Causative", "Making or letting someone do something (〜させる)."),
        ("Conditional", "If / when condition (〜ば / 〜たら)."),
        ("Negative-imperative", "Command not to do something (〜な)."),
    ];
    for (label, explanation) in CONJUGATIONS {
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='jong-legend-badge'>{label}</span>\
               <span>{explanation}</span>\
             </div>"
        ));
    }
    html.push_str("</div></div>");
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
        chunk_data.push((conn.clone(), None, None, None, None));
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
        chunk.secondary_particle.as_ref(),
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

fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('\u{30a1}'..='\u{30f6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn has_kanji(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9faf}' | '\u{3400}'..='\u{4dbf}'))
}

fn format_word_html(word: &ProcToken, enable_furigana: bool) -> String {
    if enable_furigana && has_kanji(&word.full) {
        if let Some(r) = &word.reading {
            let h_reading = katakana_to_hiragana(r);
            if !h_reading.is_empty() && h_reading != word.full {
                return format!("<ruby>{}<rt>{}</rt></ruby>", word.full, h_reading);
            }
        }
    }
    word.full.clone()
}

fn render_row(
    word: &ProcToken,
    particle: Option<&ProcToken>,
    secondary_particle: Option<&ProcToken>,
    role: Option<&ParticleRole>,
    prefix: &str,
    branch: &str,
    chunk_data: &mut Vec<ChunkData>,
) -> String {
    let id = chunk_data.len();
    chunk_data.push((word.clone(), particle.cloned(), secondary_particle.cloned(), role.cloned(), None));

    let (enable_furi, base_font_size) = CONTROLLER.with(|c| {
        if let Ok(b) = c.try_borrow() {
            (b.furigana, b.font_size)
        } else {
            (true, 16)
        }
    });

    let (size, word_class) = if prefix.is_empty() && branch.is_empty() {
        (format!("{}px", base_font_size), "jong-word-head")
    } else {
        (format!("{}px", (base_font_size as f32 * 0.9) as u32), "jong-word-mod")
    };
    
    let word_html = format_word_html(word, enable_furi);

    let mut html = format!(
        "<div class='jong-row' data-chunk-id='{id}' style='font-size:{size};line-height:1.2;padding:0 4px'>\
         <span class='jong-tree-arm'>{}{}</span>\
         <span class='{word_class}'>{}</span>",
        prefix, branch, word_html
    );
    if let Some(p) = particle {
        let p_html = format_word_html(p, enable_furi);
        html.push_str(&format!(" <span class='{word_class}'>{}</span>", p_html));
    }
    if let Some(sp) = secondary_particle {
        let sp_html = format_word_html(sp, enable_furi);
        html.push_str(&format!(" <span class='{word_class}'>{}</span>", sp_html));
    }
    if let Some(r) = role {
        let is_ambig = matches!(r, ParticleRole::Ambiguous(_));
        let class = if is_ambig { "ambiguous-badge jong-role-badge" } else { "resolved-badge jong-role-badge" };
        html.push_str(&format!(
            " <span class='{}'>{}</span>",
            class,
            r.badge()
        ));
    }
    html.push_str("</div>");
    html
}

fn render_all_details(chunk_data: &[ChunkData]) -> String {
    let mut html = String::new();
    for (i, (word, particle, secondary_particle, role, selected_def)) in chunk_data.iter().enumerate() {
        let detail_body = render_detail(word, particle.as_ref(), secondary_particle.as_ref(), role.as_ref(), *selected_def);
        html.push_str(&format!(
            "<div class='jong-accordion' data-detail-id='{}'>\
             <div class='jong-accordion-body'>{}</div>\
             </div>",
            i, detail_body
        ));
    }
    html
}

fn render_detail(word: &ProcToken, particle: Option<&ProcToken>, secondary_particle: Option<&ProcToken>, role: Option<&ParticleRole>, selected_def: Option<usize>) -> String {
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
                "<div><span class='jong-muted'>Reading:</span> {}</div>",
                hit.kana
            ));
            html.push_str(&format!(
                "<div><span class='jong-muted'>Base:</span> {}</div>",
                word.base
            ));
            html.push_str(&format!(
                "<div><span class='jong-muted'>POS:</span> {:?}</div>",
                word.pos
            ));
            
            html.push_str(&format!("<div style='margin-top:4px'><strong>Definitions:</strong>{}</div>", type_hint));
            html.push_str("<div class='jong-def-box'>");
            html.push_str("<ol>");
            
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
                "<div><span class='jong-muted'>Base:</span> {}</div>",
                word.base
            ));
            html.push_str(&format!(
                "<div><span class='jong-muted'>POS:</span> {:?}</div>",
                word.pos
            ));
            html.push_str("<div class='jong-muted'>no dictionary entry</div>");
        }
    }

    if let Some(p) = particle {
        html.push_str("<div class='jong-detail-section' style='margin-top:10px;padding-top:6px'>");
        html.push_str(&format!(
            "<div style='font-weight:600;font-size:13px;margin-bottom:4px'>Particle: {}</div>",
            p.full
        ));
        match role {
            Some(ParticleRole::Ambiguous(candidates)) => {
                html.push_str(
                    "<div><strong>Role is ambiguous (candidates):</strong></div>\
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
            }
            Some(r) => {
                html.push_str(&format!(
                    "<div><strong>Role:</strong> {} — {}</div>",
                    r.badge(),
                    r.explanation()
                ));
            }
            None => {
                html.push_str("<div><strong>Role:</strong> unknown</div>");
            }
        }
        if let Some(sp) = secondary_particle {
            html.push_str(&format!(
                "<div style='margin-top:6px'><strong>Topicalizer:</strong> {}</div>",
                sp.full
            ));
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
                "<div class='jong-detail-section' style='margin-top:10px;padding-top:6px'>\
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