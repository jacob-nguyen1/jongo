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

use crate::labels::{ClauseRelation, PartOfSpeech, PartOfSpeechSubcategory1, ParticleRole};
use crate::grammar::ProcToken;
use crate::sentence::{Chunk, Clause, Modifier, Sentence};

const SENTENCE_DELIMITERS: [u16; 4] = ['.' as u16, '。' as u16, '\n' as u16, '…' as u16];
const MIN_WINDOW_WIDTH: f64 = 360.0;
// Raise the minimum default height so short popups are a bit taller by default
const MIN_WINDOW_HEIGHT: f64 = 280.0;
// Default and max sizes for the initial popup
const DEFAULT_WINDOW_WIDTH: f64 = 900.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 520.0;
const MAX_DEFAULT_WINDOW_WIDTH: f64 = 1200.0;
const MAX_DEFAULT_WINDOW_HEIGHT: f64 = 1000.0;
const RESIZE_EDGE: f64 = 10.0;
const BASE_Z_INDEX: u32 = 10_000;
const DEFAULT_FONT_SIZE: u32 = 20;
const MIN_FONT_SIZE: u32 = 12;
const MAX_FONT_SIZE: u32 = 32;

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

fn estimate_initial_window_size(sentence: &str, viewport_width: f64, viewport_height: f64) -> (f64, f64) {
    let length = sentence.chars().count().max(1) as f64;
    // Increase width sensitivity to account for nested clauses and tag pills
    let mut width = DEFAULT_WINDOW_WIDTH + (length / 60.0).floor() * 60.0;
    let mut height = DEFAULT_WINDOW_HEIGHT + (length / 20.0).floor() * 28.0;

    width = width.min(MAX_DEFAULT_WINDOW_WIDTH);
    height = height.min(MAX_DEFAULT_WINDOW_HEIGHT);

    // Allowed by viewport (leave a small margin)
    let max_width_allowed = (viewport_width - 40.0).max(120.0);
    let max_height_allowed = (viewport_height - 40.0).max(120.0);

    // Prefer the computed size, but do not exceed viewport; also prefer MIN_WINDOW_* when possible
    width = width.min(max_width_allowed);
    let lower_w = MIN_WINDOW_WIDTH.min(max_width_allowed);
    if width < lower_w {
        width = lower_w;
    }

    height = height.min(max_height_allowed);
    let lower_h = MIN_WINDOW_HEIGHT.min(max_height_allowed);
    if height < lower_h {
        height = lower_h;
    }

    (width, height)
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
        let _ = el.style().set_property("background", DARK_BASE);
        let _ = el.style().set_property("color", DARK_TEXT);
        let _ = el.style().set_property("border-color", DARK_BORDER_STRONG);
    } else {
        let _ = el.remove_attribute("data-jong-dark");
        let _ = el.style().remove_property("filter");
        let _ = el.style().set_property("background", "white");
        let _ = el.style().set_property("color", "black");
        let _ = el.style().set_property("border-color", "black");
    }
}

/// Theme the floating prompt host without the analysis-window box chrome.
fn apply_prompt_theme(el: &web_sys::HtmlElement, dark: bool) {
    if dark {
        let _ = el.set_attribute("data-jong-dark", "1");
    } else {
        let _ = el.remove_attribute("data-jong-dark");
    }
    let _ = el.style().set_property("background", "transparent");
    let _ = el.style().set_property("border", "none");
    let _ = el.style().set_property("padding", "0");
    let _ = el.style().remove_property("color");
}

const PROMPT_FADE_MS: i32 = 160;

fn fade_remove_prompt(el: web_sys::HtmlElement) {
    let _ = el.class_list().add_1("jong-prompt-leave");
    let Some(window) = web_sys::window() else {
        el.remove();
        return;
    };
    let cb = Closure::once(move || {
        el.remove();
    });
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        PROMPT_FADE_MS,
    );
    cb.forget();
}

fn prompt_host_html() -> &'static str {
    "\
<style>\
.jong-prompt-host{position:absolute;z-index:10000;pointer-events:auto;opacity:1;\
transform:translateX(-50%)}\
.jong-prompt-host.jong-prompt-leave{opacity:0;pointer-events:none;\
transform:translateX(-50%) translateY(4px) scale(0.96);\
transition:opacity .15s ease,transform .15s ease}\
.jong-prompt-host.jong-prompt-above.jong-prompt-leave{\
transform:translateX(-50%) translateY(-4px) scale(0.96)}\
.jong-prompt-btn{appearance:none;font-family:ui-sans-serif,system-ui,-apple-system,sans-serif;\
background:#1c1c1c;color:#f4f4f4;border:1px solid rgba(255,255,255,.08);\
border-radius:999px;padding:7px 16px;font-size:13px;font-weight:650;letter-spacing:.04em;\
line-height:1;cursor:pointer;box-shadow:0 4px 16px rgba(0,0,0,.2),0 1px 2px rgba(0,0,0,.12);\
transition:transform .12s ease,box-shadow .12s ease,background .12s ease,border-color .12s ease}\
.jong-prompt-btn:hover{transform:translateY(-1px);background:#2a2a2a;\
box-shadow:0 8px 22px rgba(0,0,0,.24),0 1px 2px rgba(0,0,0,.14)}\
.jong-prompt-btn:active{transform:translateY(0);box-shadow:0 2px 8px rgba(0,0,0,.18)}\
[data-jong-dark=\"1\"] .jong-prompt-btn{background:#ececec;color:#161616;border-color:rgba(0,0,0,.08);\
box-shadow:0 4px 16px rgba(0,0,0,.35),0 1px 2px rgba(0,0,0,.2)}\
[data-jong-dark=\"1\"] .jong-prompt-btn:hover{background:#fff;transform:translateY(-1px)}\
[data-jong-dark=\"1\"] .jong-prompt-btn:active{transform:translateY(0)}\
.jong-nat-seg{cursor:pointer;border-radius:3px;padding:0 2px;transition:background 0.1s}\
.jong-nat-seg:hover{background:#eef2f7}\
.jong-nat-seg-selected{background:#dbeafe}\
[data-jong-dark=\"1\"] .jong-nat-seg:hover{background:#2d333b}\
[data-jong-dark=\"1\"] .jong-nat-seg-selected{background:#3b4b61}\
</style>\
<button type='button' class='jong-prompt-btn' title='Analyze with Jongo'>jong</button>"
}

const DISAMBIGUATE_BTN_HTML: &str = "\
<button class='jong-disambiguate refine-ai-btn' type='button' title='Disambiguate with AI'>Disambiguate</button>";

const DISAMBIGUATE_SPINNER_HTML: &str = "\
<svg class='jong-disambiguate-spinner' width='14' height='14' viewBox='0 0 24 24' fill='none' aria-hidden='true'>\
<circle cx='12' cy='12' r='9' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-dasharray='42 14'/>\
</svg>";

// Dark palette — base #22242a, surfaces/borders/text derived from it
const DARK_BASE: &str = "#22242a";
const DARK_SURFACE: &str = "#2a2c34";
const DARK_SURFACE_HIGH: &str = "#32343e";
const DARK_SURFACE_HOVER: &str = "#3a3c48";
const DARK_BORDER: &str = "#3e404a";
const DARK_BORDER_STRONG: &str = "#4e505c";
const DARK_TEXT: &str = "#e8eaef";
const DARK_TEXT_SECONDARY: &str = "#d0d3db";
const DARK_TEXT_MUTED: &str = "#a8acb8";
const DARK_TEXT_DIM: &str = "#9094a0";
const DARK_SELECTED: &str = "#2e3648";
const DARK_SCROLL_THUMB: &str = "#6b6e7a";

fn scaled_font_px(base: u32, tier: &str) -> u32 {
    let factor: f32 = match tier {
        "mod" => 0.9,
        "title" | "detail-sub" => 0.8125,
        "legend-section" => 0.7,
        "detail" => 0.75,
        "detail-xs" => 0.6875,
        "section-label" | "badge" => 0.5,
        "reading" | "clause-label" => 0.55,
        "connective" => 0.9,
        _ => 1.0,
    };
    ((base as f32 * factor).round() as u32).max(8)
}

fn clause_relation_theme_class(relation: &ClauseRelation) -> &'static str {
    match relation {
        ClauseRelation::Main => "jong-clause-main",
        ClauseRelation::Modifier => "jong-clause-modifier",
        _ => "",
    }
}

fn legend_clause_badge_style(color: &str) -> String {
    format!("color:{color};background:{color}22;border:1px solid {color}55")
}

fn pos_badge_class(pos: PartOfSpeech) -> &'static str {
    match pos {
        PartOfSpeech::Noun => "jong-pos-noun",
        PartOfSpeech::Verb => "jong-pos-verb",
        PartOfSpeech::Adjective | PartOfSpeech::AdnominalAdjective => "jong-pos-adj",
        PartOfSpeech::Adverb => "jong-pos-adv",
        PartOfSpeech::AuxiliaryVerb => "jong-pos-aux",
        PartOfSpeech::Particle => "jong-pos-part",
        PartOfSpeech::Conjunction => "jong-pos-conj",
        PartOfSpeech::Interjection => "jong-pos-interj",
        _ => "jong-pos-neutral",
    }
}

#[derive(Clone, Copy)]
struct RenderContext {
    font_size: u32,
    furigana: bool,
    tooltips: bool,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            furigana: true,
            tooltips: true,
        }
    }
}

fn render_context_from_controller() -> RenderContext {
    CONTROLLER.with(|c| {
        if let Ok(b) = c.try_borrow() {
            RenderContext {
                font_size: b.font_size,
                furigana: true,
                tooltips: b.tooltips,
            }
        } else {
            RenderContext::default()
        }
    })
}

fn tip_attrs(ctx: &RenderContext, tip: &str) -> String {
    let escaped = html_escape(tip);
    if ctx.tooltips {
        format!(" data-tip=\"{escaped}\" title=\"{escaped}\"")
    } else {
        format!(" data-tip=\"{escaped}\"")
    }
}

fn conjugation_explanation(label: &str) -> &'static str {
    match label {
        "Negative" => "Negates the action or state (〜ない).",
        "Past" => "Marks completed or past tense (〜た / 〜だ).",
        "Continuous" => "Ongoing or resulting state (〜ている).",
        "Te-form" => "Connective form used for sequences and requests (〜て).",
        "Desiderative" => "Expresses desire to do something (〜たい).",
        "Volitional" => "Expresses intention or suggestion (〜よう / 〜う).",
        "Potential" => "Ability or possibility (〜られる / できる).",
        "Causative" => "Making or letting someone do something (〜させる).",
        "Conditional" => "If / when condition (〜ば / 〜たら).",
        "Negative-imperative" => "Command not to do something (〜な).",
        _ => "",
    }
}

fn role_tip(role: &ParticleRole) -> String {
    match role {
        ParticleRole::Ambiguous(candidates) => candidates
            .iter()
            .map(|c| format!("{}: {}", c.badge(), c.explanation()))
            .collect::<Vec<_>>()
            .join(" | "),
        _ => role.explanation().to_string(),
    }
}

fn push_verb_badges(html: &mut String, ctx: &RenderContext, word: &ProcToken) {
    let badge_px = scaled_font_px(ctx.font_size, "badge");
    if let Some(tag) = word.verb_print() {
        for label in tag.split(", ") {
            let tip = conjugation_explanation(label);
            html.push_str(&format!(
                " <span class='jong-verb-badge' data-font-tier='badge' style='font-size:{badge_px}px'{}>{}</span>",
                tip_attrs(ctx, tip),
                html_escape(label)
            ));
        }
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
    tooltips: bool,
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
            tooltips: true,
            font_size: DEFAULT_FONT_SIZE,
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
            apply_prompt_theme(prompt, self.dark_mode);
        }
        for a in &self.analyses {
            apply_theme(&a.element, self.dark_mode);
        }
    }

    fn set_dark_mode(&mut self, on: bool) {
        self.dark_mode = on;
        self.apply_dark_mode_all();
    }

    fn apply_font_size_all(&self) {
        let base = self.font_size;
        for a in &self.analyses {
            let Ok(nodes) = a.element.query_selector_all("[data-font-tier]") else { continue };
            for i in 0..nodes.length() {
                let Some(node) = nodes.item(i) else { continue };
                let Ok(el) = node.dyn_into::<web_sys::HtmlElement>() else { continue };
                let tier = el.get_attribute("data-font-tier").unwrap_or_default();
                let size = scaled_font_px(base, &tier);
                let _ = el.style().set_property("font-size", &format!("{size}px"));
            }
        }
    }

    fn apply_tooltips_all(&self) {
        for a in &self.analyses {
            let Ok(nodes) = a.element.query_selector_all("[data-tip]") else { continue };
            for i in 0..nodes.length() {
                let Some(node) = nodes.item(i) else { continue };
                let Ok(el) = node.dyn_into::<web_sys::Element>() else { continue };
                if self.tooltips {
                    if let Some(tip) = el.get_attribute("data-tip") {
                        let _ = el.set_attribute("title", &tip);
                    }
                } else {
                    let _ = el.remove_attribute("title");
                }
            }
        }
    }

    fn apply_furigana_all(&self) {
        for a in &self.analyses {
            if self.furigana {
                let _ = a.element.remove_attribute("data-jong-furi");
            } else {
                let _ = a.element.set_attribute("data-jong-furi", "0");
            }
        }
    }

    fn adjust_font_size(&mut self, delta: i32) {
        let new = (self.font_size as i32 + delta).clamp(MIN_FONT_SIZE as i32, MAX_FONT_SIZE as i32) as u32;
        if new == self.font_size {
            return;
        }
        self.font_size = new;
        persist_setting("fontSize", &JsValue::from_f64(new as f64));
        self.apply_font_size_all();
    }

    fn dismiss_prompt(&mut self) {
        if let Some(old) = self.prompt.take() {
            fade_remove_prompt(old);
        }
    }

    fn dismiss_prompt_immediate(&mut self) {
        if let Some(old) = self.prompt.take() {
            old.remove();
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
        if sentence_str.trim().is_empty() {
            return;
        }
        console::log_1(&format!("Sentence: {}", sentence_str).into());

        let element = document
            .create_element("div")
            .unwrap()
            .dyn_into::<web_sys::HtmlElement>()
            .unwrap();
        element.set_class_name("jong-prompt-host");

        let scroll_x = window.scroll_x().unwrap_or(0.0);
        let scroll_y = window.scroll_y().unwrap_or(0.0);
        let viewport_w = window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let viewport_h = window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(600.0);
        const BTN_EST_W: f64 = 72.0;
        const BTN_EST_H: f64 = 36.0;
        const GAP: f64 = 8.0;

        let space_below = viewport_h - rect.bottom();
        let space_above = rect.top();
        let place_above = space_below < BTN_EST_H + GAP && space_above > space_below;
        if place_above {
            let _ = element.class_list().add_1("jong-prompt-above");
        }

        let top = if place_above {
            rect.top() + scroll_y - BTN_EST_H - GAP
        } else {
            rect.bottom() + scroll_y + GAP
        };
        let caret_mid_x = rect.left() + rect.width() * 0.5;
        let left = (caret_mid_x + scroll_x)
            .clamp(scroll_x + BTN_EST_W * 0.5 + 8.0, scroll_x + viewport_w - BTN_EST_W * 0.5 - 8.0);

        element.style().set_property("position", "absolute").unwrap();
        element.style().set_property("top", &format!("{top}px")).unwrap();
        element.style().set_property("left", &format!("{left}px")).unwrap();
        element.set_inner_html(prompt_host_html());
        apply_prompt_theme(&element, self.dark_mode);

        // Replace previous prompt immediately so Shift+mousemove can track the caret
        self.dismiss_prompt_immediate();

        document.body().unwrap().append_child(&element).unwrap();

        // Clicks inside the host must not bubble to the document dismiss listener
        let stop_prop = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
            e.stop_propagation();
        });
        element
            .add_event_listener_with_callback("click", stop_prop.as_ref().unchecked_ref())
            .unwrap();
        stop_prop.forget();

        let sentence = sentence_str;
        let context = block_text;
        let btn = element.query_selector("button").unwrap().unwrap();

        let cb = Closure::<dyn FnMut()>::new(move || {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.analyze(&sentence, &context);
                    ctrl.dismiss_prompt_immediate();
                }
            });
        });

        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
            .unwrap();
        cb.forget();

        self.prompt = Some(element);
    }

    fn analyze(&mut self, sentence: &str, context: &str) {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        let prompt_rect = self.prompt.as_ref().unwrap().get_bounding_client_rect();
        let viewport_width = window.inner_width().unwrap_or(JsValue::from_f64(DEFAULT_WINDOW_WIDTH)).as_f64().unwrap_or(DEFAULT_WINDOW_WIDTH);
        let viewport_height = window.inner_height().unwrap_or(JsValue::from_f64(DEFAULT_WINDOW_HEIGHT)).as_f64().unwrap_or(DEFAULT_WINDOW_HEIGHT);
        let (default_width, default_height) = estimate_initial_window_size(sentence, viewport_width, viewport_height);

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
        element.style().set_property("width", &format!("{default_width}px")).unwrap();
        element.style().set_property("height", &format!("{default_height}px")).unwrap();
        element.style().set_property("box-sizing", "border-box").unwrap();
        element.style().set_property("overflow", "hidden").unwrap();
        if !self.furigana {
            let _ = element.set_attribute("data-jong-furi", "0");
        }
        let tokens = grammar::analyze_sentence(sentence);
        let mut chunk_data: Vec<ChunkData> = Vec::new();
        let ctx = RenderContext {
            font_size: self.font_size,
            furigana: true,  // Always render ruby; toggle via data-jong-furi + CSS
            tooltips: self.tooltips,
        };
        let left = match crate::sentence::build_sentence(tokens) {
            Some(s) => {
                let tree_html = render_structure_tree(&ctx, &s, &mut chunk_data);
                compute_surface_ranges(sentence, &mut chunk_data);
                let sentence_html = render_natural_sentence(&ctx, sentence, &chunk_data);
                format!("{sentence_html}{tree_html}")
            }
            None => "<div>could not parse</div>".to_string(),
        };

        let chunk_data_rc = Rc::new(RefCell::new(chunk_data));

        let all_details = render_all_details(&ctx, &chunk_data_rc.borrow());
        let analysis_body = format!(
            "<div class='jong-structure-scroll'><div class='jong-structure-inner'>{left}</div></div>\
             <div class='jong-detail'>{all_details}</div>"
        );
        let analysis_body_rc = Rc::new(analysis_body.clone());

        let html = format!(
            "<style>\
             .jong-row{{cursor:pointer;border-radius:3px;white-space:nowrap;display:flex;align-items:center;gap:6px}}\
.jong-tree-arm{{font-family:monospace;white-space:pre;color:#666;flex-shrink:0;align-self:center}}\
.jong-particle-inline{{color:#6b7280;font-weight:500;margin-left:2px}}\
[data-jong-dark=\"1\"] .jong-particle-inline{{color:{DARK_TEXT_MUTED}}}\
[data-jong-dark=\"1\"] .jong-candidate-label{{color:#fbbf24}}\
[data-jong-dark=\"1\"] .jong-candidate-or{{color:#6b7280}}\
[data-jong-dark=\"1\"] .jong-candidate-badge{{color:#fbbf24;border-color:#d97706;background:transparent}}\
.jong-candidate-group{{display:inline-flex;align-items:center;gap:5px;flex-wrap:wrap}}\
.jong-candidate-label{{color:#b45309;font-weight:600;text-transform:uppercase;letter-spacing:.04em}}\
.jong-candidate-or{{color:#9ca3af;font-style:italic}}\
.jong-candidate-badge{{color:#b45309;border:1px dashed #f59e0b;background:transparent;border-radius:999px;padding:2px 8px;font-weight:600}}\
             .jong-row:hover{{background:#eef2f7}}\
             .jong-row-selected{{background:#dbeafe}}\
.jong-nat-seg{{cursor:pointer;border-radius:3px;padding:0 2px;transition:background 0.1s}}\
.jong-nat-seg:hover{{background:#eef2f7}}\
.jong-nat-seg-selected{{background:#dbeafe}}\
             .jong-top-bar{{display:flex;align-items:stretch;background:#f5f5f5;border-bottom:1px solid #e0e0e0;flex-shrink:0;height:32px}}\
             .jong-drag-handle{{flex:1;cursor:move;display:flex;align-items:center;padding:0 12px;user-select:none}}\
             .jong-legend{{background:#fffef5;color:#ca8a04;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 14px;display:flex;align-items:center;justify-content:center;transition:background 0.2s}}\
             .jong-legend:hover{{background:#fefce8;color:#a16207}}\
             .jong-legend-active{{background:#fef9c3 !important;color:#a16207 !important}}\
             .jong-disambiguate{{background:#f5f3ff;color:#7c3aed;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 12px;display:flex;align-items:center;justify-content:center;flex-shrink:0;font-size:11px;font-weight:600;letter-spacing:.02em;transition:background 0.2s,color 0.2s,opacity 0.2s}}\
             .jong-disambiguate:hover{{background:#ede9fe;color:#6d28d9}}\
             .jong-disambiguate-loading{{pointer-events:none;opacity:0.85}}\
             .jong-disambiguate-spinner{{animation:jong-spin 0.8s linear infinite}}\
             @keyframes jong-spin{{to{{transform:rotate(360deg)}}}}\
             .jong-font-btn{{background:#f8fafc;color:#475569;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 10px;display:flex;align-items:center;justify-content:center;font-size:12px;font-weight:700;min-width:32px;transition:background 0.2s}}\
             .jong-font-btn:hover{{background:#e2e8f0;color:#1e293b}}\
             .jong-close{{background:#fef2f2;color:#ef4444;border:none;border-left:1px solid #e0e0e0;cursor:pointer;padding:0 14px;display:flex;align-items:center;justify-content:center;transition:background 0.2s}}\
             .jong-close:hover{{background:#fee2e2;color:#dc2626}}\
             .jong-body{{display:flex;gap:16px;flex:1;min-height:0;padding:8px 0;box-sizing:border-box;overflow:hidden}}\
             .jong-structure-scroll,.jong-detail,.jong-panel,.jong-def-box,.jong-staircase-box,.jong-tree-scroll-wrapper{{scrollbar-width:thin;scrollbar-color:#444 #e8e8e8}}\
             .jong-structure-scroll::-webkit-scrollbar,.jong-detail::-webkit-scrollbar,.jong-panel::-webkit-scrollbar,.jong-def-box::-webkit-scrollbar,.jong-staircase-box::-webkit-scrollbar,.jong-tree-scroll-wrapper::-webkit-scrollbar{{width:5px;height:5px}}\
             .jong-structure-scroll::-webkit-scrollbar-thumb,.jong-detail::-webkit-scrollbar-thumb,.jong-panel::-webkit-scrollbar-thumb,.jong-def-box::-webkit-scrollbar-thumb,.jong-staircase-box::-webkit-scrollbar-thumb,.jong-tree-scroll-wrapper::-webkit-scrollbar-thumb{{background:#444;border-radius:0}}\
             .jong-structure-scroll::-webkit-scrollbar-track,.jong-detail::-webkit-scrollbar-track,.jong-panel::-webkit-scrollbar-track,.jong-def-box::-webkit-scrollbar-track,.jong-staircase-box::-webkit-scrollbar-track,.jong-tree-scroll-wrapper::-webkit-scrollbar-track{{background:#e8e8e8}}\
             .jong-structure-scroll{{direction:rtl;overflow-y:auto;overflow-x:hidden;flex:1;min-width:0;margin:0;user-select:none}}\
             .jong-structure-inner{{direction:ltr;padding:0 8px 0 6px}}\
             .jong-detail{{flex:1;min-width:0;overflow-y:auto;border-left:2px solid #bbb;padding:0 8px 0 12px;position:relative}}\
             .jong-muted{{color:#444}}\
             .jong-hint{{color:#555;font-size:11px}}\
             .jong-word-head{{font-weight:600;color:#111}}\
             .jong-word-mod{{font-weight:500;color:#111}}\
             .jong-tree-arm{{font-family:monospace;white-space:pre;color:#666}}\
             .jong-role-badge{{color:#666;border:1px solid #ccc;border-radius:999px;padding:2px 8px}}\
             .ambiguous-badge{{color:#b45309 !important;border-color:#f59e0b !important;background:#fffbeb !important;font-weight:600}}\
             .resolved-badge{{color:#15803d !important;border-color:#22c55e !important;background:#f0fdf4 !important;font-weight:600}}\
             .jong-verb-badge{{color:#0369a1;border:1px solid #0ea5e9;background:#f0f9ff;border-radius:999px;padding:2px 8px;font-weight:600}}\
             .jong-def-box{{max-height:150px;overflow-y:auto;background:#fafafa;border:1px solid #eee;border-radius:4px;padding:8px 8px 8px 24px;margin-top:2px}}\
             .jong-def-box ol{{margin:0;padding:0;color:#333}}\
             .jong-card-header{{margin-bottom:2px}}\
             .jong-card-reading{{color:#888;line-height:1.2;margin-bottom:2px}}\
             .jong-card-headword-row{{display:flex;align-items:center;gap:8px;flex-wrap:wrap}}\
             .jong-card-headword{{font-weight:700;color:#111;line-height:1.2}}\
             .jong-card-base{{color:#888;margin-top:4px}}\
             .jong-pos-badge{{font-weight:700;text-transform:uppercase;letter-spacing:.04em;border-radius:999px;padding:2px 8px;flex-shrink:0;border:1px solid transparent}}\
             .jong-pos-noun{{color:#4338ca;background:#eef2ff;border-color:#c7d2fe}}\
             .jong-pos-verb{{color:#be123c;background:#fff1f2;border-color:#fecdd3}}\
             [data-jong-furi=\"0\"] rt{{display:none}}\
             .jong-pos-adj{{color:#7e22ce;background:#faf5ff;border-color:#e9d5ff}}\
             .jong-pos-adv{{color:#a21caf;background:#fdf4ff;border-color:#f5d0fe}}\
             .jong-pos-aux{{color:#6d28d9;background:#f5f3ff;border-color:#ddd6fe}}\
             .jong-pos-part{{color:#475569;background:#f8fafc;border-color:#cbd5e1}}\
             .jong-pos-conj{{color:#57534e;background:#fafaf9;border-color:#d6d3d1}}\
             .jong-pos-interj{{color:#db2777;background:#fdf2f8;border-color:#fbcfe8}}\
             .jong-pos-neutral{{color:#6b7280;background:#f9fafb;border-color:#e5e7eb}}\
             .jong-section-label{{font-weight:700;text-transform:uppercase;letter-spacing:.06em;color:#888;margin:0 0 6px}}\
             .jong-card-divider{{border:none;border-top:1px solid #eee;margin:10px 0}}\
             .jong-def-list{{list-style:none;margin:0;padding:0}}\
             .jong-def-item{{padding:4px 0 4px 10px;margin-bottom:2px;color:#555;border-left:2px solid transparent;line-height:1.4}}\
             .jong-def-item.selected{{font-weight:700;color:#111;border-left-color:#22c55e}}\
             .jong-def-toggle-btn{{background:none;border:none;color:#888;cursor:pointer;padding:0;font-family:inherit;font-weight:400;text-transform:none;letter-spacing:.02em;text-decoration:none;display:inline-flex;align-items:center;gap:4px;line-height:1}}\
             .jong-def-toggle-btn:hover{{color:#111}}\
             .jong-particle-row{{display:flex;align-items:center;gap:8px;flex-wrap:wrap}}\
             .jong-particle-chip{{display:inline-flex;align-items:center;justify-content:center;min-width:28px;padding:2px 8px;border:1px solid #ddd;border-radius:6px;font-weight:600;background:#fafafa}}\
             .jong-tag-row{{display:flex;flex-wrap:wrap;gap:6px}}\
             .jong-tree-scroll-wrapper{{overflow-x:auto;padding-bottom:8px}}\
             .jong-staircase-box{{max-height:150px;overflow-y:auto;border:1px solid #eee;border-radius:6px;padding:8px;margin-top:4px;background:#fafafa}}\
             .jong-staircase-step{{margin-bottom:4px;line-height:1.4}}\
             .jong-no-entry{{color:#888;font-style:italic;margin-top:4px}}\
             .jong-detail-section{{border-top:1px solid #eee}}\
             .jong-accordion{{border:2px solid #ccc;border-radius:6px;margin-bottom:6px;overflow:hidden;cursor:pointer}}\
             .jong-accordion summary{{cursor:pointer;padding:6px 10px;font-size:13px;font-weight:600;background:#f8f9fa;list-style:none;display:flex;align-items:center;gap:6px;user-select:none}}\
             .jong-accordion summary::-webkit-details-marker{{display:none}}\
             .jong-accordion summary::before{{content:'▶';font-size:9px;transition:transform 0.15s;display:inline-block}}\
             .jong-accordion[open] summary::before{{transform:rotate(90deg)}}\
             .jong-accordion-body{{padding:10px 12px;font-size:12px;line-height:1.6}}\
             .jong-accordion-highlight{{border-color:#3b82f6;box-shadow:0 0 0 1px #3b82f6}}\
             .jong-panel{{flex:1;min-width:0;min-height:0;overflow-y:auto;padding:4px 12px 12px;font-size:12px;line-height:1.5;box-sizing:border-box}}\
             .jong-panel-section{{margin-top:14px}}\
             .jong-panel-section h3{{margin:0 0 8px;border-bottom:1px solid #ddd;padding-bottom:4px}}\
             .jong-legend-section-title{{font-weight:600;color:#333}}\
             .jong-legend-row{{display:flex;gap:8px;align-items:center;margin-bottom:6px}}\
             .jong-legend-row .jong-role-badge,.jong-legend-row .jong-verb-badge,.jong-legend-row .jong-pos-badge{{flex-shrink:0}}\
             .jong-legend-clause-badge{{display:inline-flex;align-items:center;flex-shrink:0;min-width:96px;font-weight:600;padding:2px 8px;border-radius:4px;box-sizing:border-box}}\
             [data-jong-dark=\"1\"] .jong-row:hover{{background:{DARK_SURFACE_HIGH}}}\
             [data-jong-dark=\"1\"] .jong-row-selected{{background:{DARK_SELECTED}}}\
[data-jong-dark=\"1\"] .jong-nat-seg:hover{{background:{DARK_SURFACE_HIGH}}}\
[data-jong-dark=\"1\"] .jong-nat-seg-selected{{background:{DARK_SELECTED}}}\
             [data-jong-dark=\"1\"] .jong-top-bar{{background:{DARK_SURFACE};border-bottom-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-drag-handle{{color:{DARK_TEXT_MUTED}}}\
             [data-jong-dark=\"1\"] .jong-legend{{background:#352c14;color:#facc15;border-left-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-legend:hover{{background:#42361a;color:#fde047}}\
             [data-jong-dark=\"1\"] .jong-legend-active{{background:#4a3d1e !important;color:#fde047 !important}}\
             [data-jong-dark=\"1\"] .jong-disambiguate{{background:#2e1065;color:#c4b5fd;border-left-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-disambiguate:hover{{background:#3b0764;color:#ddd6fe}}\
             [data-jong-dark=\"1\"] .jong-font-btn{{background:{DARK_SURFACE_HIGH};color:{DARK_TEXT_SECONDARY};border-left-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-font-btn:hover{{background:{DARK_SURFACE_HOVER};color:{DARK_TEXT}}}\
             [data-jong-dark=\"1\"] .jong-close{{background:#3a2224;color:#f87171;border-left-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-close:hover{{background:#4f2b2e;color:#fca5a5}}\
             [data-jong-dark=\"1\"] .jong-detail{{border-left-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-muted{{color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-hint{{color:{DARK_TEXT_MUTED}}}\
             [data-jong-dark=\"1\"] .jong-word-head{{color:{DARK_TEXT}}}\
             [data-jong-dark=\"1\"] .jong-word-mod{{color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-tree-arm{{color:{DARK_TEXT_DIM}}}\
             [data-jong-dark=\"1\"] .jong-role-badge{{color:{DARK_TEXT_MUTED};border-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-def-box{{background:{DARK_SURFACE};border-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-def-box ol{{color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-card-reading{{color:{DARK_TEXT_DIM}}}\
             [data-jong-dark=\"1\"] .jong-card-headword{{color:{DARK_TEXT}}}\
             [data-jong-dark=\"1\"] .jong-card-base{{color:{DARK_TEXT_DIM}}}\
             [data-jong-dark=\"1\"] .jong-pos-noun{{color:#a5b4fc;background:#1e1b4b;border-color:#4338ca}}\
             [data-jong-dark=\"1\"] .jong-pos-verb{{color:#fda4af;background:#4c0519;border-color:#be123c}}\
             [data-jong-dark=\"1\"] .jong-pos-adj{{color:#d8b4fe;background:#3b0764;border-color:#7e22ce}}\
             [data-jong-dark=\"1\"] .jong-pos-adv{{color:#f0abfc;background:#4a044e;border-color:#a21caf}}\
             [data-jong-dark=\"1\"] .jong-pos-aux{{color:#c4b5fd;background:#2e1065;border-color:#6d28d9}}\
             [data-jong-dark=\"1\"] .jong-pos-part{{color:#cbd5e1;background:{DARK_SURFACE_HIGH};border-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-pos-conj{{color:#d6d3d1;background:#2c2a32;border-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-pos-interj{{color:#f9a8d4;background:#500724;border-color:#db2777}}\
             [data-jong-dark=\"1\"] .jong-pos-neutral{{color:#d1d5db;background:{DARK_SURFACE_HIGH};border-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-section-label{{color:{DARK_TEXT_DIM}}}\
             [data-jong-dark=\"1\"] .jong-card-divider{{border-top-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-def-item{{color:{DARK_TEXT_MUTED};border-left-color:transparent}}\
             [data-jong-dark=\"1\"] .jong-def-item.selected{{color:{DARK_TEXT};border-left-color:#4ade80}}\
             [data-jong-dark=\"1\"] .jong-def-toggle-btn{{color:#8b8f9a}}\
             [data-jong-dark=\"1\"] .jong-def-toggle-btn:hover{{color:#e8eaef}}\
             [data-jong-dark=\"1\"] .jong-particle-chip{{background:{DARK_SURFACE_HIGH};border-color:{DARK_BORDER_STRONG};color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-staircase-box{{background:{DARK_SURFACE};border-color:{DARK_BORDER};color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-no-entry{{color:{DARK_TEXT_DIM}}}\
             [data-jong-dark=\"1\"] .jong-detail-section{{border-top-color:{DARK_BORDER}}}\
             [data-jong-dark=\"1\"] .jong-accordion{{border-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-accordion summary{{background:{DARK_SURFACE};color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-accordion-highlight{{border-color:#60a5fa;box-shadow:0 0 0 1px #60a5fa}}\
             [data-jong-dark=\"1\"] .jong-structure-scroll,[data-jong-dark=\"1\"] .jong-detail,[data-jong-dark=\"1\"] .jong-panel,[data-jong-dark=\"1\"] .jong-def-box,[data-jong-dark=\"1\"] .jong-staircase-box,[data-jong-dark=\"1\"] .jong-tree-scroll-wrapper{{scrollbar-color:{DARK_SCROLL_THUMB} {DARK_BASE}}}\
             [data-jong-dark=\"1\"] .jong-structure-scroll::-webkit-scrollbar-thumb,[data-jong-dark=\"1\"] .jong-detail::-webkit-scrollbar-thumb,[data-jong-dark=\"1\"] .jong-panel::-webkit-scrollbar-thumb,[data-jong-dark=\"1\"] .jong-def-box::-webkit-scrollbar-thumb,[data-jong-dark=\"1\"] .jong-staircase-box::-webkit-scrollbar-thumb,[data-jong-dark=\"1\"] .jong-tree-scroll-wrapper::-webkit-scrollbar-thumb{{background:{DARK_SCROLL_THUMB}}}\
             [data-jong-dark=\"1\"] .jong-structure-scroll::-webkit-scrollbar-track,[data-jong-dark=\"1\"] .jong-detail::-webkit-scrollbar-track,[data-jong-dark=\"1\"] .jong-panel::-webkit-scrollbar-track,[data-jong-dark=\"1\"] .jong-def-box::-webkit-scrollbar-track,[data-jong-dark=\"1\"] .jong-staircase-box::-webkit-scrollbar-track,[data-jong-dark=\"1\"] .jong-tree-scroll-wrapper::-webkit-scrollbar-track{{background:{DARK_BASE}}}\
             [data-jong-dark=\"1\"] .jong-panel-section h3{{border-bottom-color:{DARK_BORDER_STRONG}}}\
             [data-jong-dark=\"1\"] .jong-legend-section-title{{color:{DARK_TEXT_SECONDARY}}}\
             [data-jong-dark=\"1\"] .jong-clause-box.jong-clause-main,[data-jong-dark=\"1\"] .jong-clause-box.jong-clause-modifier{{border-color:{DARK_TEXT} !important}}\
             [data-jong-dark=\"1\"] .jong-clause-label.jong-clause-main,[data-jong-dark=\"1\"] .jong-clause-label.jong-clause-modifier,[data-jong-dark=\"1\"] .jong-clause-connective.jong-clause-main,[data-jong-dark=\"1\"] .jong-clause-connective.jong-clause-modifier{{color:{DARK_TEXT} !important}}\
             [data-jong-dark=\"1\"] .jong-legend-clause-badge.jong-clause-main,[data-jong-dark=\"1\"] .jong-legend-clause-badge.jong-clause-modifier{{color:{DARK_TEXT} !important;background:{DARK_SURFACE_HIGH} !important;border-color:{DARK_BORDER_STRONG} !important}}\
             [data-jong-dark=\"1\"] .ambiguous-badge{{color:#fbbf24 !important;border-color:#d97706 !important;background:#451a03 !important}}\
             [data-jong-dark=\"1\"] .resolved-badge{{color:#4ade80 !important;border-color:#22c55e !important;background:#052e16 !important}}\
             [data-jong-dark=\"1\"] .jong-verb-badge{{color:#7dd3fc !important;border-color:#0284c7 !important;background:#082f49 !important}}\
             </style>\
             <div class='jong-top-bar'>\
                <div class='jong-drag-handle'></div>\
                {disambiguate_btn}\
                <button class='jong-legend' title='Legend (toggle)'>\
                  <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'></circle><path d='M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3'></path><line x1='12' y1='17' x2='12.01' y2='17'></line></svg>\
                </button>\
                <button class='jong-font-btn' data-font-delta='-1' title='Decrease text size' type='button'>A−</button>\
                <button class='jong-font-btn' data-font-delta='1' title='Increase text size' type='button'>A+</button>\
                <button class='jong-close' title='Close'>\
                  <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><line x1='18' y1='6' x2='6' y2='18'></line><line x1='6' y1='6' x2='18' y2='18'></line></svg>\
                </button>\
              </div>\
              <div class='jong-body'>{analysis_body}</div>",
            disambiguate_btn = DISAMBIGUATE_BTN_HTML,
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

        // Font size A− / A+
        let font_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
            let Some(target) = e.target() else { return };
            let Ok(el) = target.dyn_into::<web_sys::Element>() else { return };
            let Ok(Some(btn)) = el.closest("[data-font-delta]") else { return };
            let Some(delta_str) = btn.get_attribute("data-font-delta") else { return };
            let Ok(delta) = delta_str.parse::<i32>() else { return };
            e.stop_propagation();
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.adjust_font_size(delta);
                }
            });
        });
        element
            .add_event_listener_with_callback("click", font_cb.as_ref().unchecked_ref())
            .unwrap();
        font_cb.forget();

        // delegated click: chunk row / detail card <-> sync selection (with toggle deselect)
        let detail_cb = {
            let container = element.clone();
            let chunk_data_for_click = chunk_data_rc.clone();
            let selected_id: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let Some(target) = e.target() else { return };
                let Ok(el) = target.dyn_into::<web_sys::Element>() else { return };

                let idx = if let Ok(Some(accordion)) = el.closest(".jong-accordion") {
                    accordion
                        .get_attribute("data-detail-id")
                        .and_then(|s| s.parse::<usize>().ok())
                } else if let Ok(Some(row)) = el.closest("[data-chunk-id]") {
                    row.get_attribute("data-chunk-id")
                        .and_then(|s| s.parse::<usize>().ok())
                } else {
                    return;
                };
                let Some(idx) = idx else { return };

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

                // Clear natural sentence segment highlights
                let segs = container.query_selector_all(".jong-nat-seg-selected");
                if let Ok(list) = segs {
                    for i in 0..list.length() {
                        if let Some(node) = list.item(i) {
                            if let Ok(el) = node.dyn_into::<web_sys::Element>() {
                                let cls = el.get_attribute("class").unwrap_or_default();
                                let _ = el.set_attribute("class", &cls.replace(" jong-nat-seg-selected", ""));
                            }
                        }
                    }
                }

                // Clear natural sentence highlights
                let nat_segs = container.query_selector_all(".jong-nat-seg-selected");
                if let Ok(list) = nat_segs {
                    for i in 0..list.length() {
                        if let Some(node) = list.item(i) {
                            if let Ok(el) = node.dyn_into::<web_sys::Element>() {
                                let cls = el.get_attribute("class").unwrap_or_default();
                                let _ = el.set_attribute("class", &cls.replace(" jong-nat-seg-selected", ""));
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

                // Toggle: if clicking the same token, deselect
                let mut sel = selected_id.borrow_mut();
                if *sel == Some(idx) {
                    *sel = None;
                    let cd = chunk_data_for_click.borrow();
                    let ctx = render_context_from_controller();
                    let all_html = render_all_details(&ctx, &cd);
                    detail_panel.set_inner_html(&all_html);
                    return;
                }
                *sel = Some(idx);

                // Highlight tree row
                let row_selector = format!(".jong-row[data-chunk-id='{idx}']");
                if let Ok(Some(row)) = container.query_selector(&row_selector) {
                    let cls = row.get_attribute("class").unwrap_or_default();
                    let _ = row.set_attribute("class", &format!("{cls} jong-row-selected"));
                    if let Ok(row_html) = row.dyn_into::<web_sys::HtmlElement>() {
                        if let Ok(Some(scroll_container)) = container.query_selector(".jong-structure-scroll") {
                            if let Ok(scroll_el) = scroll_container.dyn_into::<web_sys::HtmlElement>() {
                                let container_rect = scroll_el.get_bounding_client_rect();
                                let row_rect = row_html.get_bounding_client_rect();
                                let current_scroll = scroll_el.scroll_top();
                                // Only scroll if row is outside the visible area
                                if row_rect.top() < container_rect.top() {
                                    let delta = (row_rect.top() - container_rect.top()).round() as i32;
                                    scroll_el.set_scroll_top(current_scroll + delta);
                                } else if row_rect.bottom() > container_rect.bottom() {
                                    let delta = (row_rect.bottom() - container_rect.bottom()).round() as i32;
                                    scroll_el.set_scroll_top(current_scroll + delta);
                                }
                            }
                        }
                    }
                }

                // Highlight natural sentence segment
                let seg_selector = format!(".jong-nat-seg[data-chunk-id='{idx}']");
                if let Ok(Some(seg)) = container.query_selector(&seg_selector) {
                    let cls = seg.get_attribute("class").unwrap_or_default();
                    let _ = seg.set_attribute("class", &format!("{cls} jong-nat-seg-selected"));
                }

                let nat_seg_selector = format!(".jong-nat-seg[data-chunk-id='{idx}']");
                if let Ok(Some(seg)) = container.query_selector(&nat_seg_selector) {
                    let cls = seg.get_attribute("class").unwrap_or_default();
                    let _ = seg.set_attribute("class", &format!("{cls} jong-nat-seg-selected"));
                }

                let accordion_selector = format!("[data-detail-id='{idx}']");
                if let Ok(Some(accordion)) = detail_panel.query_selector(&accordion_selector) {
                    let acc_cls = accordion.get_attribute("class").unwrap_or_default();
                    let _ = accordion.set_attribute("class", &format!("{acc_cls} jong-accordion-highlight"));
                    if let Ok(html_el) = accordion.dyn_into::<web_sys::HtmlElement>() {
                        if let Ok(detail_panel_html) = detail_panel.dyn_into::<web_sys::HtmlElement>() {
                            let panel_rect = detail_panel_html.get_bounding_client_rect();
                            let acc_rect = html_el.get_bounding_client_rect();
                            let current = detail_panel_html.scroll_top();
                            let delta = (acc_rect.top() - panel_rect.top()).round() as i32;
                            detail_panel_html.set_scroll_top(current + delta);
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
                    btn_html.set_class_name("jong-disambiguate refine-ai-btn jong-disambiguate-loading");
                    btn_html.set_inner_html(DISAMBIGUATE_SPINNER_HTML);

                    let res = fetch_llm(&prompt).await;
                    btn_html.set_class_name("jong-disambiguate refine-ai-btn");
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

        // Legend button toggles legend panel ↔ analysis
        if let Ok(Some(legend_btn)) = element.query_selector(".jong-legend") {
            let element_legend = element.clone();
            let analysis_body_rc = analysis_body_rc.clone();
            let legend_btn_for_cb = legend_btn.clone();
            let legend_cb = Closure::<dyn FnMut()>::new(move || {
                let Ok(Some(body)) = element_legend.query_selector(".jong-body") else { return };
                let on_legend = body.query_selector(".jong-panel").ok().flatten().is_some();
                if on_legend {
                    body.set_inner_html(&analysis_body_rc);
                    if let Ok(html_body) = body.dyn_into::<web_sys::HtmlElement>() {
                        let _ = html_body.style().set_property("display", "flex");
                        let _ = html_body.style().set_property("flex-direction", "row");
                        let _ = html_body.style().set_property("overflow", "hidden");
                    }
                    let _ = legend_btn_for_cb.class_list().remove_1("jong-legend-active");
                    CONTROLLER.with(|c| {
                        if let Ok(ctrl) = c.try_borrow() {
                            ctrl.apply_font_size_all();
                        }
                    });
                } else {
                    body.set_inner_html(&render_legend_panel(&render_context_from_controller()));
                    if let Ok(html_body) = body.dyn_into::<web_sys::HtmlElement>() {
                        let _ = html_body.style().set_property("display", "flex");
                        let _ = html_body.style().set_property("flex-direction", "column");
                        let _ = html_body.style().set_property("overflow", "hidden");
                    }
                    let _ = legend_btn_for_cb.class_list().add_1("jong-legend-active");
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

    // mouse click — outside click dismisses with fade
    let mouse_click_cb = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|_e: web_sys::MouseEvent| {
        CONTROLLER.with(|c| {
            if let Ok(mut ctrl) = c.try_borrow_mut() {
                ctrl.dismiss_prompt();
            }
        });
    });
    window
        .add_event_listener_with_callback("click", mouse_click_cb.as_ref().unchecked_ref())
        .unwrap();
    mouse_click_cb.forget();

    // key press
    let key_cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
        let key = e.key();
        if key == "Escape" {
            CONTROLLER.with(|c| {
                if let Ok(mut ctrl) = c.try_borrow_mut() {
                    ctrl.dismiss_prompt();
                }
            });
            return;
        }
        if key == "Shift" {
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
            ctrl.apply_furigana_all();
        }
    });
}

#[wasm_bindgen]
pub fn set_tooltips(on: bool) {
    CONTROLLER.with(|c| {
        if let Ok(mut ctrl) = c.try_borrow_mut() {
            ctrl.tooltips = on;
            ctrl.apply_tooltips_all();
        }
    });
}

#[wasm_bindgen]
pub fn set_font_size(size: u32) {
    CONTROLLER.with(|c| {
        if let Ok(mut ctrl) = c.try_borrow_mut() {
            ctrl.font_size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
            ctrl.apply_font_size_all();
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
                            entry.role = Some(resolved_role.clone());
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
                        entry.selected_def = Some(def_idx as usize);
                    }
                }
            }
        }
    }
    
    // Re-render the right sidebar to reflect the new disambiguated data
    if let Ok(Some(detail_panel)) = container.query_selector(".jong-detail") {
        let cd = chunk_data.borrow();
        let ctx = render_context_from_controller();
        let all_html = render_all_details(&ctx, &cd);
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
                            let panel_rect = detail_panel_html.get_bounding_client_rect();
                            let acc_rect = html_el.get_bounding_client_rect();
                            let current = detail_panel_html.scroll_top();
                            let delta = (acc_rect.top() - panel_rect.top()).round() as i32;
                            detail_panel_html.set_scroll_top(current + delta);
                        }
                    }
                }
            }
        }
    }
}

struct ChunkData {
    word: ProcToken,
    particle: Option<ProcToken>,
    secondary_particle: Option<ProcToken>,
    role: Option<ParticleRole>,
    selected_def: Option<usize>,
    surface_range: Option<(usize, usize)>,
}

fn compute_surface_ranges(sentence: &str, chunks: &mut [ChunkData]) {
    let chars: Vec<char> = sentence.chars().collect();
    let mut cursor = 0;

    for chunk in chunks {
        let mut surface = chunk.word.full.clone();
        if let Some(p) = &chunk.particle {
            surface.push_str(&p.full);
        }
        if let Some(sp) = &chunk.secondary_particle {
            surface.push_str(&sp.full);
        }

        let surface_chars: Vec<char> = surface.chars().collect();
        if surface_chars.is_empty() {
            continue;
        }

        let mut found_pos = None;
        for p in cursor..=chars.len().saturating_sub(surface_chars.len()) {
            if chars[p..p + surface_chars.len()] == surface_chars[..] {
                found_pos = Some(p);
                break;
            }
        }

        if let Some(p) = found_pos {
            chunk.surface_range = Some((p, p + surface_chars.len()));
            cursor = p + surface_chars.len();
        }
    }
}

fn render_natural_sentence(ctx: &RenderContext, sentence: &str, chunks: &[ChunkData]) -> String {
    let title_px = scaled_font_px(ctx.font_size, "title");
    let mut html = format!(
        "<div class='jong-sentence-text' data-font-tier='title' style='font-size:{title_px}px;font-weight:600;line-height:1.4;margin-bottom:8px;word-break:break-word'>"
    );

    let chars: Vec<char> = sentence.chars().collect();
    let mut cursor = 0;
    let mut sorted_chunks: Vec<(usize, &ChunkData)> = chunks
        .iter()
        .enumerate()
        .filter_map(|(id, c)| c.surface_range.map(|(start, _)| (id, c)))
        .collect();
    sorted_chunks.sort_by_key(|(id, c)| c.surface_range.unwrap().0);

    for (id, chunk) in sorted_chunks {
        let (start, end) = chunk.surface_range.unwrap();
        
        // Gap chars
        if cursor < start {
            let gap: String = chars[cursor..start].iter().collect();
            html.push_str(&html_escape(&gap));
            cursor = start;
        }

        // Segment
        let word_html = format_word_html(&chunk.word, ctx.furigana);
        let mut seg_content = word_html;
        if let Some(p) = &chunk.particle {
            seg_content.push_str(&format!(" {}", format_word_html(p, ctx.furigana)));
        }
        if let Some(sp) = &chunk.secondary_particle {
            seg_content.push_str(&format!(" {}", format_word_html(sp, ctx.furigana)));
        }

        html.push_str(&format!(
            "<span class='jong-nat-seg' data-chunk-id='{}'>{}</span>",
            id, seg_content
        ));
        cursor = end;
    }

    if cursor < chars.len() {
        let trailing: String = chars[cursor..].iter().collect();
        html.push_str(&html_escape(&trailing));
    }

    html.push_str("</div>");
    html
}

fn render_structure_tree(ctx: &RenderContext, sentence: &Sentence, chunk_data: &mut Vec<ChunkData>) -> String {
    let mut html = String::from("<div class='jong-structure' style='position:relative'>");
    html.push_str("<div class='jong-tree-scroll-wrapper'><div class='jong-tree-container' style='display:flex;flex-direction:column;width:max-content;min-width:100%'>");
    for clause in &sentence.clauses {
        html.push_str(&render_clause(ctx, clause, chunk_data));
    }
    html.push_str("</div></div>");
    html.push_str("</div>");
    html
}

fn render_legend_panel(ctx: &RenderContext) -> String {
    let title_px = scaled_font_px(ctx.font_size, "title");
    let hint_px = scaled_font_px(ctx.font_size, "detail-xs");
    let section_px = scaled_font_px(ctx.font_size, "legend-section");
    let badge_px = scaled_font_px(ctx.font_size, "badge");
    let body_px = scaled_font_px(ctx.font_size, "detail");
    let clause_px = scaled_font_px(ctx.font_size, "clause-label");

    let mut html = format!(
        "<div class='jong-panel' data-font-tier='detail' style='font-size:{body_px}px'>\
           <div data-font-tier='title' style='font-size:{title_px}px;font-weight:600;margin-bottom:4px'>Legend</div>\
           <div class='jong-hint' data-font-tier='detail-xs' style='font-size:{hint_px}px;margin-bottom:8px'>Reference for labels used in analysis</div>"
    );

    html.push_str(&format!(
        "<div class='jong-panel-section'><h3 class='jong-legend-section-title' data-font-tier='legend-section' style='font-size:{section_px}px'>Parts of speech</h3>"
    ));
    const POS_LEGEND: &[(PartOfSpeech, &str)] = &[
        (PartOfSpeech::Noun, "Nouns, pronouns, and proper nouns."),
        (PartOfSpeech::Verb, "Action and state verbs."),
        (PartOfSpeech::Adjective, "い-adjectives and な-adjectives."),
        (PartOfSpeech::Adverb, "Words that modify verbs or adjectives."),
        (PartOfSpeech::AuxiliaryVerb, "Helper verbs attached to main verbs."),
        (PartOfSpeech::Particle, "Grammatical particles (は, が, を, etc.)."),
        (PartOfSpeech::Conjunction, "Words that connect clauses or phrases."),
        (PartOfSpeech::Interjection, "Exclamations and interjections."),
        (PartOfSpeech::Others, "Other or unclassified word types."),
    ];
    let mut pos_entries: Vec<_> = POS_LEGEND.iter().copied().collect();
    pos_entries.sort_by(|a, b| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)));
    for (pos, explanation) in pos_entries {
        let pos_class = pos_badge_class(pos);
        let label = format!("{:?}", pos);
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='jong-pos-badge {pos_class}' data-font-tier='badge' style='font-size:{badge_px}px'>{label}</span>\
               <span data-font-tier='detail' style='font-size:{body_px}px'>{explanation}</span>\
             </div>"
        ));
    }
    html.push_str("</div>");

    html.push_str(&format!(
        "<div class='jong-panel-section'><h3 class='jong-legend-section-title' data-font-tier='legend-section' style='font-size:{section_px}px'>Particle roles</h3>"
    ));
    let mut particle_roles: Vec<(&str, &str, bool)> = ParticleRole::all()
        .iter()
        .map(|role| (role.badge(), role.explanation(), false))
        .collect();
    particle_roles.push((
        "Ambiguous",
        "Cannot be determined by rule-based parsing alone.",
        true,
    ));
    particle_roles.sort_by_key(|(badge, _, _)| *badge);
    for (badge, explanation, ambiguous) in particle_roles {
        let class = if ambiguous {
            "ambiguous-badge jong-role-badge"
        } else {
            "resolved-badge jong-role-badge"
        };
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='{class}' data-font-tier='badge' style='font-size:{badge_px}px'>{badge}</span>\
               <span data-font-tier='detail' style='font-size:{body_px}px'>{explanation}</span>\
             </div>"
        ));
    }
    html.push_str("</div>");

    html.push_str(&format!(
        "<div class='jong-panel-section'><h3 class='jong-legend-section-title' data-font-tier='legend-section' style='font-size:{section_px}px'>Clause relations</h3>"
    ));
    let mut clause_relations: Vec<_> = ClauseRelation::all().iter().collect();
    clause_relations.sort_by_key(|rel| rel.label());
    for rel in clause_relations {
        let color = rel.color();
        let rel_class = clause_relation_theme_class(rel);
        let badge_style = legend_clause_badge_style(color);
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='jong-legend-clause-badge {rel_class}' data-font-tier='clause-label' style='font-size:{clause_px}px;{badge_style}'>{}</span>\
               <span data-font-tier='detail' style='font-size:{body_px}px'>{}</span>\
             </div>",
            rel.label(),
            rel.explanation()
        ));
    }
    html.push_str("</div>");

    html.push_str(&format!(
        "<div class='jong-panel-section'><h3 class='jong-legend-section-title' data-font-tier='legend-section' style='font-size:{section_px}px'>Verb conjugations</h3>"
    ));
    const CONJUGATIONS: &[&str] = &[
        "Negative",
        "Past",
        "Continuous",
        "Te-form",
        "Desiderative",
        "Volitional",
        "Potential",
        "Causative",
        "Conditional",
        "Negative-imperative",
    ];
    let mut conjugations: Vec<&str> = CONJUGATIONS.to_vec();
    conjugations.sort();
    for label in conjugations {
        html.push_str(&format!(
            "<div class='jong-legend-row'>\
               <span class='jong-verb-badge' data-font-tier='badge' style='font-size:{badge_px}px'>{}</span>\
               <span data-font-tier='detail' style='font-size:{body_px}px'>{}</span>\
             </div>",
            label,
            conjugation_explanation(label)
        ));
    }
    html.push_str("</div></div>");
    html
}

fn render_clause(ctx: &RenderContext, clause: &Clause, chunk_data: &mut Vec<ChunkData>) -> String {
    let color = clause.relation.color();
    let label = clause.relation.label();
    let tip = clause.relation.explanation();
    let rel_class = clause_relation_theme_class(&clause.relation);
    let label_px = scaled_font_px(ctx.font_size, "clause-label");
    let conn_px = scaled_font_px(ctx.font_size, "connective");
    let mut html = format!(
        "<div class='jong-clause-box {rel_class}' style='border:1px solid {color};border-radius:4px;margin-bottom:10px;padding:6px 8px'>\
         <div class='jong-clause-label {rel_class}' data-font-tier='clause-label' style='font-size:{label_px}px;color:{color};margin-bottom:6px'{}>{label}</div>",
        tip_attrs(ctx, tip)
    );
    html.push_str(&render_chunk_group(ctx, &clause.predicate, "", "", chunk_data));
    if let Some(conn) = &clause.connective {
        let id = chunk_data.len();
        chunk_data.push(ChunkData {
            word: conn.clone(),
            particle: None,
            secondary_particle: None,
            role: None,
            selected_def: None,
            surface_range: None,
        });
        html.push_str(&format!(
            "<div class='jong-row jong-clause-connective {rel_class}' data-chunk-id='{id}' data-font-tier='connective' style='margin-top:6px;font-size:{conn_px}px;font-weight:600;color:{color};display:inline-block;padding:2px 4px'>{}</div>",
            conn.full
        ));
    }
    html.push_str("</div>");
    html
}

fn render_chunk_group(ctx: &RenderContext, chunk: &Chunk, prefix: &str, branch: &str, chunk_data: &mut Vec<ChunkData>) -> String {
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
        html.push_str(&render_modifier(ctx, modifier, &mod_child_prefix, mod_branch, chunk_data));
    }
    html.push_str(&render_row(
        ctx,
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

fn render_modifier(ctx: &RenderContext, modifier: &Modifier, prefix: &str, branch: &str, chunk_data: &mut Vec<ChunkData>) -> String {
    match modifier {
        Modifier::AdjectiveChunk(chunk) => render_chunk_group(ctx, chunk, prefix, branch, chunk_data),
        Modifier::AdverbChunk(chunk) => render_chunk_group(ctx, chunk, prefix, branch, chunk_data),
        Modifier::NounChunk(chunk) => render_chunk_group(ctx, chunk, prefix, branch, chunk_data),
        Modifier::Limitation(chunk) => render_chunk_group(ctx, chunk, prefix, branch, chunk_data),
        Modifier::Quotation(sentence) => {
            let mut html = String::new();
            for c in &sentence.clauses {
                html.push_str(&render_clause(ctx, c, chunk_data));
            }
            html
        },
        Modifier::Clause(clause) => {
            render_chunk_group(ctx, &clause.predicate, prefix, branch, chunk_data)
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

fn has_kana(s: &str) -> bool {
    s.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c))
}

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9faf}' | '\u{3400}'..='\u{4dbf}')
}

fn base_reading_for_furigana(word: &ProcToken) -> Option<String> {
    // Prefer token reading (inflected / context-aware) when available, otherwise consult dictionary base reading.
    if let Some(r) = &word.reading {
        let h = katakana_to_hiragana(r);
        if !h.is_empty() {
            return Some(h);
        }
    }

    let is_proper_noun = word.sub1 == crate::labels::PartOfSpeechSubcategory1::ProperNoun;
    if let Some(hit) = crate::jmdict::lookup_first_result(&word.base, word.pos, is_proper_noun) {
        if !hit.kana.is_empty() {
            return Some(katakana_to_hiragana(&hit.kana));
        }
    }
    None
}

fn strip_common_ending(mut s: String) -> String {
    // Unicode-safe removal of common dictionary endings to approximate verb/adjective stems
    // Prefer using strip_suffix so we don't slice by byte counts.
    if let Some(rest) = s.strip_suffix('る') {
        return rest.to_string();
    }
    if let Some(rest) = s.strip_suffix("ます") {
        return rest.to_string();
    }
    if let Some(rest) = s.strip_suffix('た') {
        return rest.to_string();
    }
    if let Some(rest) = s.strip_suffix('だ') {
        return rest.to_string();
    }
    s
}

fn try_render_multi_run(full_chars: &[char], h_reading: &str) -> Option<String> {
    // Split surface into alternating kanji / kana runs.
    let mut runs: Vec<(bool, String)> = Vec::new(); // (is_kanji, text)
    let mut i = 0;
    while i < full_chars.len() {
        let start = i;
        let is_k = is_kanji(full_chars[i]);
        while i < full_chars.len() && is_kanji(full_chars[i]) == is_k {
            i += 1;
        }
        let text: String = full_chars[start..i].iter().collect();
        runs.push((is_k, text));
    }

    let reading_chars: Vec<char> = h_reading.chars().collect();
    let mut r_cursor = 0usize;
    let mut out = String::new();

    for (idx, (is_k, text)) in runs.iter().enumerate() {
        if !is_k {
            // Kana run: must appear at reading cursor
            let kana_hira = katakana_to_hiragana(text);
            let kana_chars: Vec<char> = kana_hira.chars().collect();
            if r_cursor + kana_chars.len() > reading_chars.len() {
                return None;
            }
            if reading_chars[r_cursor..r_cursor + kana_chars.len()] != kana_chars[..] {
                return None;
            }
            out.push_str(&html_escape(text));
            r_cursor += kana_chars.len();
        } else {
            // Kanji run: reading extends until next kana landmark (or end of reading if last)
            let is_last = idx == runs.len() - 1;
            let kanji_reading: String = if is_last {
                reading_chars[r_cursor..].iter().collect()
            } else {
                // Peek at next run — must be kana — find where it starts in reading
                let (_, next_kana) = &runs[idx + 1];
                let next_hira = katakana_to_hiragana(next_kana);
                let next_chars: Vec<char> = next_hira.chars().collect();
                if next_chars.is_empty() {
                    return None;
                }
                // Find next_chars starting somewhere at or after r_cursor
                let mut found = None;
                if reading_chars.len() >= next_chars.len() {
                    for start in r_cursor..=reading_chars.len().saturating_sub(next_chars.len()) {
                        if reading_chars[start..start + next_chars.len()] == next_chars[..] {
                            found = Some(start);
                            break;
                        }
                    }
                }
                let end = found?;
                if end <= r_cursor {
                    return None; // Kanji run must have at least one char of reading
                }
                let slice: String = reading_chars[r_cursor..end].iter().collect();
                r_cursor = end;
                slice
            };

            if kanji_reading.is_empty() {
                return None;
            }
            if is_last {
                r_cursor = reading_chars.len();
            }
            out.push_str(&format!(
                "<ruby>{}<rt>{}</rt></ruby>",
                html_escape(text),
                html_escape(&kanji_reading)
            ));
        }
    }

    if r_cursor != reading_chars.len() {
        return None; // Reading not fully consumed → misalignment
    }

    Some(out)
}

fn format_word_html(word: &ProcToken, enable_furigana: bool) -> String {
    web_sys::console::log_1(&format!(
        "furigana input: full={:?} base={:?} reading={:?}",
        word.full, word.base, word.reading
    ).into());
    if !enable_furigana || !has_kanji(&word.full) {
        return word.full.clone();
    }

    let Some(reading) = &word.reading else {
        return word.full.clone();
    };
    let h_reading = katakana_to_hiragana(reading);
    if h_reading.is_empty() {
        return word.full.clone();
    }

    let full_chars: Vec<char> = word.full.chars().collect();

    // Split surface into: leading kana | contiguous kanji run | trailing kana.
    // If the surface has multiple kanji runs separated by kana, fall back to whole-surface ruby.
    let prefix_kana_len = full_chars.iter().take_while(|&&c| !is_kanji(c)).count();
    let kanji_run_len = full_chars[prefix_kana_len..]
        .iter()
        .take_while(|&&c| is_kanji(c))
        .count();
    if kanji_run_len == 0 {
        return word.full.clone();
    }
    let kanji_end = prefix_kana_len + kanji_run_len;

// Multi-run case: use kana landmarks to slice the reading.
    if full_chars[kanji_end..].iter().any(|&c| is_kanji(c)) {
        if let Some(rendered) = try_render_multi_run(&full_chars, &h_reading) {
            return rendered;
        }
        // Fallback: whole-surface ruby if landmarks don't align
        return format!(
            "<ruby>{}<rt>{}</rt></ruby>",
            html_escape(&word.full),
            html_escape(&h_reading)
        );
    }

    let prefix_kana: String = full_chars[..prefix_kana_len].iter().collect();
    let kanji_middle: String = full_chars[prefix_kana_len..kanji_end].iter().collect();
    let kana_tail: String = full_chars[kanji_end..].iter().collect();

    let prefix_kana_hira = katakana_to_hiragana(&prefix_kana);
    let kana_tail_hira = katakana_to_hiragana(&kana_tail);

    // Strip leading kana from the reading (must match).
    let h_chars: Vec<char> = h_reading.chars().collect();
    let prefix_chars: Vec<char> = prefix_kana_hira.chars().collect();
    if prefix_chars.len() > h_chars.len()
        || h_chars[..prefix_chars.len()] != prefix_chars[..]
    {
        // Reading doesn't start with the surface prefix kana — fall back.
        return format!(
            "<ruby>{}<rt>{}</rt></ruby>",
            html_escape(&word.full),
            html_escape(&h_reading)
        );
    }
    let reading_after_prefix = &h_chars[prefix_chars.len()..];

    // Find longest prefix of the tail kana that matches a suffix of the remaining reading.
    let tail_chars: Vec<char> = kana_tail_hira.chars().collect();
    let mut covered_len = 0usize;
    let max_len = tail_chars.len().min(reading_after_prefix.len().saturating_sub(1));
    for k in (1..=max_len).rev() {
        if tail_chars[..k] == reading_after_prefix[reading_after_prefix.len() - k..] {
            covered_len = k;
            break;
        }
    }

    let kanji_reading: String = reading_after_prefix[..reading_after_prefix.len() - covered_len]
        .iter()
        .collect();
    if kanji_reading.is_empty() {
        return word.full.clone();
    }

    let covered_oku: String = full_chars[kanji_end..kanji_end + covered_len].iter().collect();
    let remaining_inflection: String = full_chars[kanji_end + covered_len..].iter().collect();

    format!(
        "{}<ruby>{}<rt>{}</rt></ruby>{}{}",
        html_escape(&prefix_kana),
        html_escape(&kanji_middle),
        html_escape(&kanji_reading),
        html_escape(&covered_oku),
        html_escape(&remaining_inflection)
    )
}

fn render_row(
    ctx: &RenderContext,
    word: &ProcToken,
    particle: Option<&ProcToken>,
    secondary_particle: Option<&ProcToken>,
    role: Option<&ParticleRole>,
    prefix: &str,
    branch: &str,
    chunk_data: &mut Vec<ChunkData>,
) -> String {
    let id = chunk_data.len();
    chunk_data.push(ChunkData {
        word: word.clone(),
        particle: particle.cloned(),
        secondary_particle: secondary_particle.cloned(),
        role: role.cloned(),
        selected_def: None,
        surface_range: None,
    });

    let (tier, size, word_class) = if prefix.is_empty() && branch.is_empty() {
        ("head", format!("{}px", scaled_font_px(ctx.font_size, "head")), "jong-word-head")
    } else {
        (
            "mod",
            format!("{}px", scaled_font_px(ctx.font_size, "mod")),
            "jong-word-mod",
        )
    };

    let word_html = format_word_html(word, ctx.furigana);

    let mut html = format!(
        "<div class='jong-row' data-chunk-id='{id}' data-font-tier='{tier}' style='font-size:{size};line-height:1.2;padding:0 4px'>\
         <span class='jong-tree-arm'>{}{}</span>\
         <span class='{word_class}'>{}</span>",
        prefix, branch, word_html
    );
    if let Some(p) = particle {
        let p_html = format_word_html(p, ctx.furigana);
        html.push_str(&format!("<span class='jong-particle-inline'>{}</span>", p_html));
    }
    if let Some(sp) = secondary_particle {
        let sp_html = format_word_html(sp, ctx.furigana);
        html.push_str(&format!("<span class='jong-particle-inline'>{}</span>", sp_html));
    }
    if let Some(r) = role {
        let is_ambig = matches!(r, ParticleRole::Ambiguous(_));
        let class = if is_ambig { "ambiguous-badge jong-role-badge" } else { "resolved-badge jong-role-badge" };
        let tip = role_tip(r);
        let badge_px = scaled_font_px(ctx.font_size, "badge");
        html.push_str(&format!(
            " <span class='{class}' data-font-tier='badge' style='font-size:{badge_px}px'{}>{}</span>",
            tip_attrs(ctx, &tip),
            r.badge()
        ));
    }
    push_verb_badges(&mut html, ctx, word);
    html.push_str("</div>");
    html
}

fn render_all_details(ctx: &RenderContext, chunk_data: &[ChunkData]) -> String {
    let mut html = String::new();
    for (i, chunk) in chunk_data.iter().enumerate() {
        let detail_body = render_detail(
            ctx,
            &chunk.word,
            chunk.particle.as_ref(),
            chunk.secondary_particle.as_ref(),
            chunk.role.as_ref(),
            chunk.selected_def,
        );
        html.push_str(&format!(
            "<div class='jong-accordion' data-detail-id='{}'>\
             <div class='jong-accordion-body'>{}</div>\
             </div>",
            i, detail_body
        ));
    }
    html
}

fn render_detail(ctx: &RenderContext, word: &ProcToken, particle: Option<&ProcToken>, secondary_particle: Option<&ProcToken>, role: Option<&ParticleRole>, selected_def: Option<usize>) -> String {
    let body_px = scaled_font_px(ctx.font_size, "detail");
    let head_px = scaled_font_px(ctx.font_size, "detail-head");
    let xs_px = scaled_font_px(ctx.font_size, "detail-xs");
    let label_px = scaled_font_px(ctx.font_size, "section-label");
    let badge_px = scaled_font_px(ctx.font_size, "badge");
    let reading_px = scaled_font_px(ctx.font_size, "reading");
    let pos_label = format!("{:?}", word.pos);
    let pos_class = pos_badge_class(word.pos);

    let mut html = format!("<div class='jong-detail-card' data-font-tier='detail' style='font-size:{body_px}px;line-height:1.5'>");

    let is_proper_noun = word.sub1 == PartOfSpeechSubcategory1::ProperNoun;
    let dict_hit = crate::jmdict::lookup_first_result(&word.base, word.pos, is_proper_noun);

    // Header: reading, headword + POS pill + verb tags, optional base
    let reading = dict_hit.as_ref().map(|h| h.kana.as_str());
    html.push_str("<div class='jong-card-header'>");
    if has_kanji(&word.full) {
        if let Some(r) = reading {
            html.push_str(&format!(
                "<div class='jong-card-reading' data-font-tier='reading' style='font-size:{reading_px}px'>{}</div>",
                html_escape(r)
            ));
        }
    }
    html.push_str(&format!(
        "<div class='jong-card-headword-row'>\
         <div class='jong-card-headword' data-font-tier='detail-head' style='font-size:{head_px}px'>{}</div>\
         <span class='jong-pos-badge {pos_class}' data-font-tier='badge' style='font-size:{badge_px}px'>{}</span>",
        html_escape(&word.full),
        html_escape(&pos_label)
    ));
    push_verb_badges(&mut html, ctx, word);
    html.push_str("</div>");
    if word.base != word.full {
        html.push_str(&format!(
            "<div class='jong-card-base' data-font-tier='reading' style='font-size:{reading_px}px'>Base: {}</div>",
            html_escape(&word.base)
        ));
    }
    html.push_str("</div>");

    match dict_hit {
        Some(hit) => {
            let type_hint = match hit.source {
                crate::jmdict::DictSource::JMnedict => format!(" [{}]", hit.noun_type.label()),
                crate::jmdict::DictSource::JMdict => String::new(),
            };

            html.push_str("<div class='jong-card-divider'></div>");
            html.push_str("<div class='jong-def-section'>");

            let primary = selected_def.unwrap_or(0);
            let is_resolved = selected_def.is_some();

            let toggle_html = if is_resolved && hit.glosses.len() > 1 {
                let hidden_count = hit.glosses.len() - 1;
                format!(
                    "<button type='button' class='jong-def-toggle-btn' data-expanded='0' \
                     data-font-tier='detail-xs' style='font-size:{xs_px}px' onclick='\
                        let section = this.closest(\".jong-def-section\");\
                        let items = section.querySelectorAll(\".def-item\");\
                        let expanded = this.dataset.expanded === \"1\";\
                        for (let i = 0; i < items.length; i++) {{\
                            if (expanded) {{\
                                if (!items[i].classList.contains(\"selected\")) items[i].style.display = \"none\";\
                            }} else {{\
                                items[i].style.display = \"block\";\
                                items[i].style.opacity = items[i].classList.contains(\"selected\") ? \"1\" : \"0.5\";\
                            }}\
                        }}\
                        this.dataset.expanded = expanded ? \"0\" : \"1\";\
                        this.innerHTML = expanded ? \"▸ {hidden_count} more\" : \"▾ Hide extras\";\
                    '>▸ {hidden_count} more</button>"
                )
            } else {
                String::new()
            };

            html.push_str(&format!(
                "<div class='jong-section-label' data-font-tier='section-label' \
                 style='font-size:{label_px}px;display:flex;align-items:baseline;gap:10px'>\
                 <span>Definitions{type_hint}</span>{toggle_html}</div>",
                type_hint = type_hint,
                toggle_html = toggle_html,
            ));

            html.push_str("<ol class='jong-def-list'>");

            for (i, def) in hit.glosses.iter().enumerate() {
                let is_primary = i == primary;
                let (display, extra_class, opacity) = if is_resolved && !is_primary {
                    ("none", "", "0.5")
                } else if is_primary {
                    ("block", " selected", "1")
                } else {
                    ("block", "", "1")
                };

                html.push_str(&format!(
                    "<li class='def-item jong-def-item{extra_class}' data-idx='{i}' style='display:{display};opacity:{opacity}'>{def}</li>",
                ));
            }
            html.push_str("</ol>");

            html.push_str("</div>");
        }
        None => {
            html.push_str("<div class='jong-no-entry'>No dictionary entry</div>");
        }
    }

    if let Some(staircase) = &word.staircase {
        if staircase.len() > 1 {
            html.push_str("<div class='jong-card-divider'></div>");
            html.push_str(&format!(
                "<div class='jong-section-label' data-font-tier='section-label' style='font-size:{label_px}px'>Conjugation breakdown</div>"
            ));
            html.push_str("<div class='jong-staircase-box'>");
            for step in staircase {
                html.push_str(&format!(
                    "<div class='jong-staircase-step'>{} · {}</div>",
                    html_escape(&step.text),
                    html_escape(&step.description)
                ));
            }
            html.push_str("</div>");
        }
    }

    if let Some(p) = particle {
        html.push_str("<div class='jong-card-divider'></div>");
        html.push_str(&format!(
            "<div class='jong-section-label' data-font-tier='section-label' style='font-size:{label_px}px'>Particle</div>"
        ));
        html.push_str("<div class='jong-particle-row'>");
        html.push_str(&format!(
            "<span class='jong-particle-chip' data-font-tier='detail-head' style='font-size:{head_px}px'>{}</span>",
            html_escape(&p.full)
        ));
        match role {
            Some(ParticleRole::Ambiguous(candidates)) => {
                html.push_str("<span class='jong-candidate-group'>");
                html.push_str(&format!(
                    "<span class='jong-candidate-label' data-font-tier='detail-xs' style='font-size:{xs_px}px'>Unresolved</span>"
                ));
                for (i, c) in candidates.iter().enumerate() {
                    if i > 0 {
                        html.push_str(&format!(
                            "<span class='jong-candidate-or' data-font-tier='detail-xs' style='font-size:{xs_px}px'>or</span>"
                        ));
                    }
                    let tip = format!("{}: {}", c.badge(), c.explanation());
                    html.push_str(&format!(
                        "<span class='jong-candidate-badge' data-font-tier='badge' style='font-size:{badge_px}px'{}>{}</span>",
                        tip_attrs(ctx, &tip),
                        c.badge()
                    ));
                }
                html.push_str("</span>");
            }
            Some(r) => {
                let tip = role_tip(r);
                html.push_str(&format!(
                    "<span class='resolved-badge jong-role-badge' data-font-tier='badge' style='font-size:{badge_px}px'{}>{}</span>",
                    tip_attrs(ctx, &tip),
                    r.badge()
                ));
            }
            None => {
                html.push_str(&format!(
                    "<span class='jong-role-badge' data-font-tier='badge' style='font-size:{badge_px}px'>Unknown</span>"
                ));
            }
        }
        html.push_str("</div>");

        if let Some(sp) = secondary_particle {
            html.push_str("<div class='jong-card-divider'></div>");
            html.push_str(&format!(
                "<div class='jong-section-label' data-font-tier='section-label' style='font-size:{label_px}px'>Topicalizer</div>"
            ));
            html.push_str("<div class='jong-particle-row'>");
            html.push_str(&format!(
                "<span class='jong-particle-chip' data-font-tier='detail-head' style='font-size:{head_px}px'>{}</span>",
                html_escape(&sp.full)
            ));
            html.push_str("</div>");
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

// Expose a test-only wrapper so integration tests can call the internal formatter
pub fn test_format_word_html(word: &crate::grammar::ProcToken, enable: bool) -> String {
    format_word_html(word, enable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::ProcToken;
    use crate::labels::{PartOfSpeech, PartOfSpeechSubcategory1, PartOfSpeechSubcategory2};

    fn make_token(full: &str, base: &str, reading: Option<&str>, pos: PartOfSpeech, sub1: PartOfSpeechSubcategory1) -> ProcToken {
        ProcToken {
            full: full.to_string(),
            base: base.to_string(),
            pos,
            sub1,
            sub2: PartOfSpeechSubcategory2::X,
            conjugation: None,
            staircase: None,
            reading: reading.map(|s| s.to_string()),
        }
    }

    #[test]
    fn furigana_tabe_ta() {
        let tok = make_token("食べた", "食べる", Some("タベ"), PartOfSpeech::Verb, PartOfSpeechSubcategory1::Unbound);
        assert_eq!(format_word_html(&tok, true), "<ruby>食<rt>た</rt></ruby>べた");
    }

    #[test]
    fn furigana_waratta() {
        let tok = make_token("笑った", "笑う", Some("ワラッ"), PartOfSpeech::Verb, PartOfSpeechSubcategory1::Unbound);
        assert_eq!(format_word_html(&tok, true), "<ruby>笑<rt>わら</rt></ruby>った");
    }

    #[test]
    fn furigana_nonda() {
        let tok = make_token("飲んだ", "飲む", Some("ノン"), PartOfSpeech::Verb, PartOfSpeechSubcategory1::Unbound);
        assert_eq!(format_word_html(&tok, true), "<ruby>飲<rt>の</rt></ruby>んだ");
    }

    #[test]
    fn furigana_hito() {
        let tok = make_token("人", "人", Some("ヒト"), PartOfSpeech::Noun, PartOfSpeechSubcategory1::Unbound);
        assert_eq!(format_word_html(&tok, true), "<ruby>人<rt>ひと</rt></ruby>");
    } 

    #[test]
    fn furigana_onaka() {
        let tok = make_token("お腹", "お腹", Some("オナカ"), PartOfSpeech::Noun, PartOfSpeechSubcategory1::Unbound);
        assert_eq!(format_word_html(&tok, true), "お<ruby>腹<rt>なか</rt></ruby>");
    }

    #[test]
    fn estimate_initial_window_size_caps_and_grows() {
        let (width, height) = estimate_initial_window_size(&"あ".repeat(400), 2000.0, 1400.0);
        assert!(width <= MAX_DEFAULT_WINDOW_WIDTH);
        assert!(height <= MAX_DEFAULT_WINDOW_HEIGHT);
        assert!(height > DEFAULT_WINDOW_HEIGHT);
    }

    #[test]
    fn estimate_initial_window_size_respects_viewport() {
        let vw = 480.0;
        let vh = 260.0;
        let max_w = vw - 40.0;
        let max_h = vh - 40.0;
        let (width, height) = estimate_initial_window_size("短い文", vw, vh);
        assert!(width <= max_w);
        assert!(height <= max_h);
        assert!(width >= MIN_WINDOW_WIDTH.min(max_w));
        assert!(height >= MIN_WINDOW_HEIGHT.min(max_h));
    }

    #[test]
    fn render_detail_shows_staircase() {
        let tok = ProcToken {
            full: "食べた".to_string(),
            base: "食べる".to_string(),
            pos: PartOfSpeech::Verb,
            sub1: PartOfSpeechSubcategory1::Unbound,
            sub2: PartOfSpeechSubcategory2::X,
            conjugation: None,
            staircase: Some(vec![
                crate::grammar::StaircaseStep {
                    text: "食べる".to_string(),
                    description: "Plain form".to_string(),
                },
                crate::grammar::StaircaseStep {
                    text: "食べた".to_string(),
                    description: "Past".to_string(),
                },
            ]),
            reading: Some("タベ".to_string()),
        };

        let ctx = RenderContext::default();
        let html = render_detail(&ctx, &tok, None, None, None, None);
        assert!(html.contains("Conjugation breakdown"));
        assert!(html.contains("jong-staircase-box"));
        assert!(html.contains("食べた"));
        assert!(html.contains("Past"));
    }
}
