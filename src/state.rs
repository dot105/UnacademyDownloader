// =====================[ IMPORTS ]=====================
use std::collections::{HashMap, HashSet}; use crate::model::*; use crate::bezier::smooth_stroke;
// =====================[ CONSTANTS ]=====================
pub const MARKER: &str = "marker"; pub const HIGHLIGHTER: &str = "highlighter"; pub const ERASER: &str = "eraser"; pub const LASER: &str = "laser"; pub const DEFAULT_COLOR: &str = "#2C3844";
// =====================[ STROKE ]=====================
#[derive(Debug, Clone)] pub struct Stroke { pub id: i64, pub sid: String, pub oid: Option<i64>, pub mode: String, pub color: [u8; 3], pub width: f32, pub points: Vec<(f32, f32, i64)>, pub is_erase: bool, pub is_highlight: bool, pub is_temp_hl: bool, pub is_perm_hl: bool, pub t_start: i64, pub usl_url: Option<String>, pub dash: Vec<f32> }
// =====================[ STROKE HELPERS ]=====================
impl Stroke { pub fn is_erase(&self) -> bool { self.is_erase } pub fn is_highlight(&self) -> bool { self.is_highlight } pub fn is_temp_hl(&self) -> bool { self.is_temp_hl } }
// =====================[ CANVAS ]=====================
#[derive(Debug, Clone, Default)] pub struct Canvas { pub strokes: Vec<Stroke>, pub open: Option<Stroke>, pub zoom: f32, pub pan: (f32, f32), pub pointer: Option<(f32, f32)>, pub pointer_us: i64, pub revision: u64, pub rotation: f32, pub gif_url: String, pub screen_share: bool }
// =====================[ CANVAS HELPERS ]=====================
impl Canvas {
    pub fn new() -> Self { Canvas { zoom: 1.0, ..Default::default() } }
    pub fn touch(&mut self) { self.revision += 1 }
    pub fn pointer_alpha(&self, t_us: i64) -> f32 { const HOLD: f32 = 1.0; const FADE: f32 = 0.8; if self.pointer.is_none() { return 0.0 } let age = (t_us - self.pointer_us) as f32 / 1_000_000.0; if age <= HOLD { 1.0 } else if age >= HOLD + FADE { 0.0 } else { 1.0 - (age - HOLD) / FADE } }
    pub fn visible_strokes(&self) -> impl Iterator<Item = &Stroke> { self.strokes.iter().chain(self.open.iter().filter(|s| s.points.len() > 1)) }
}
// =====================[ SLIDE ]=====================
#[derive(Debug, Clone)] pub struct Slide { pub url: Option<String>, pub bg: Option<String>, pub background: [u8; 3], pub sid: String, pub t: i64 }
// =====================[ TRANSFORM ]=====================
#[derive(Debug, Clone, Default)] pub struct TransformEntry { pub dx: f32, pub dy: f32, pub sx: f32, pub sy: f32, pub angle: f32, pub px: f32, pub py: f32 }
// =====================[ POLL ]=====================
#[derive(Debug, Clone, Default)] pub struct Poll { pub t: i64, pub question: String, pub options: Vec<(String, f32, bool)>, pub is_open: bool }
// =====================[ REPLAY STATE ]=====================
pub struct ReplayState {
    pub slides: HashMap<i64, Slide>, pub canvases: HashMap<i64, Canvas>, pub current_slide: i64, pub color: [u8; 3], pub mode: String, pub pen_scale: f32, pub eraser_scale: f32, pub t_us: i64, pub applied: usize,
    pub slide_sid: String, pub bg_color: [u8; 3], pub bg_image_url: String, pub rotation: f32, pub gif_url: String, pub screen_share: bool, pub active_poll: Option<Poll>,
    pub completed: Vec<Stroke>, pub erased_up_to: i64, pub transform: HashMap<i64, TransformEntry>, pub temp_hl: Vec<Stroke>,
}
// =====================[ REPLAY STATE NEW ]=====================
impl Default for ReplayState { fn default() -> Self { Self::new() } }
// =====================[ REPLAY STATE IMPL ]=====================
impl ReplayState {
    pub fn new() -> Self { ReplayState { slides: HashMap::new(), canvases: HashMap::new(), current_slide: 0, color: hex_to_rgb(DEFAULT_COLOR), mode: MARKER.to_string(), pen_scale: 1.0, eraser_scale: 1.0, t_us: 0, applied: 0, slide_sid: "init".into(), bg_color: [17, 17, 24], bg_image_url: String::new(), rotation: 0.0, gif_url: String::new(), screen_share: false, active_poll: None, completed: Vec::new(), erased_up_to: -1, transform: HashMap::new(), temp_hl: Vec::new() } }
    pub fn canvas(&mut self, cid: i64) -> &mut Canvas { self.canvases.entry(cid).or_insert_with(Canvas::new) }
    pub fn active(&self) -> Option<&Canvas> { self.canvases.get(&self.current_slide) }
    pub fn active_slide(&self) -> Option<&Slide> { self.slides.get(&self.current_slide) }
}
// =====================[ TARGET CANVAS ]=====================
fn target_canvas(ev: &Event, current_slide: i64) -> i64 { ev.canvas_id.unwrap_or(current_slide) }
// =====================[ APPLY EVENT ]=====================
fn apply_event(state: &mut ReplayState, ev: &Event) {
    state.t_us = ev.p_time;
    match (ev.plugin.as_str(), ev.kind.as_str()) {
        (P_CANVAS, CW_DOWN) => pen_down(state, ev), (P_CANVAS, CW_MOVE) => pen_move(state, ev), (P_CANVAS, CW_UP) => pen_up(state, ev), (P_CANVAS, CW_POINTER) => pointer(state, ev),
        (P_CANVAS, CW_ERASE_ALL) => { let cid = target_canvas(ev, state.current_slide); let cv = state.canvas(cid); cv.strokes.clear(); cv.open = None; cv.touch(); state.erased_up_to = ev.p_time; }
        (P_CANVAS, CW_ZOOM) => { let cid = target_canvas(ev, state.current_slide); let v = ev.f("v").unwrap_or(1.0) as f32; let cv = state.canvas(cid); cv.zoom = v.max(1.0); cv.touch(); }
        (P_CANVAS, CW_PAN) => { let cid = target_canvas(ev, state.current_slide); if let Some(v) = ev.payload.get("v") { let x = v.get("x").and_then(as_f64).unwrap_or(0.0) as f32; let y = v.get("y").and_then(as_f64).unwrap_or(0.0) as f32; let cv = state.canvas(cid); cv.pan = (x, y); cv.touch(); } }
        (P_CANVAS, CW_DELETE) => { if let Some(ids) = ev.payload.get("ids").and_then(|v| v.as_array()) { let cid = target_canvas(ev, state.current_slide); let cv = state.canvas(cid); for id in ids { if let Some(oid) = as_i64(id) { cv.strokes.retain(|s| s.oid != Some(oid)) } } cv.touch(); } }
        (P_CANVAS, CW_TRANSFORM) => transform_event(state, ev),
        (P_DOC, DC_ADD_SLIDE) => { if let Some(i) = ev.i("i") { state.slides.insert(i, Slide { url: ev.s("u").map(str::to_string), bg: ev.s("bg").map(str::to_string), background: ev.s("bc").map(hex_to_rgb).unwrap_or([255, 255, 255]), sid: ev.s("uid").unwrap_or("").to_string(), t: ev.p_time }) } }
        (P_DOC, DC_SHOW_SLIDE) => { if let Some(s) = ev.i("s") { state.current_slide = s; if let Some(slide) = state.slides.get(&s) { state.slide_sid = slide.sid.clone(); state.bg_color = slide.background; state.bg_image_url = slide.bg.clone().unwrap_or_default(); state.rotation = 0.0; state.gif_url.clear() } } }
        (P_DOC, DC_BG_COLOR) => { if let Some(c) = ev.s("c").or_else(|| ev.s("bc")) { state.bg_color = hex_to_rgb(c) } if let Some(bg) = ev.s("bg") { state.bg_image_url = bg.to_string() } }
        (P_DOC, DC_COLOR) => { if let Some(c) = ev.s("c") { state.color = hex_to_rgb(c) } }
        (P_DOC, DC_MODE) => { if let Some(m) = ev.s("m") { state.mode = m.to_string() } }
        (P_DOC, DC_PEN_SIZE) => { if let Some(s) = ev.f("s") { state.pen_scale = s as f32 } }
        (P_DOC, DC_ERASER_SIZE) => { if let Some(s) = ev.f("s") { state.eraser_scale = s as f32 } }
        (P_DOC, DC_ROTATE) => { state.rotation = ev.f("v").unwrap_or(0.0) as f32 }
        (P_DOC, DC_PLAY_GIF) => { state.gif_url = ev.s("v").unwrap_or("").to_string() }
        (P_DOC, DC_SCREEN_SHARE) => { state.screen_share = ev.payload.get("v").and_then(|v| v.as_bool()).unwrap_or(false) }
        (P_MAIN, MC_CLASS_END) => {}
        (P_POLL, _) => poll_event(state, ev),
        _ => {}
    }
}
// =====================[ PEN DOWN ]=====================
fn pen_down(state: &mut ReplayState, ev: &Event) {
    let Some(pt) = ev.point() else { return }; let cid = target_canvas(ev, state.current_slide);
    let mode = ev.payload.get("mode").and_then(|v| v.as_str()).unwrap_or(&state.mode).to_string();
    let m = mode.to_lowercase();
    let is_temp_hl = m == LASER || m == HIGHLIGHTER;
    let is_perm_hl = m == "marker_highlighter" || m == "permanent_highlighter" || (m.contains("high") && m != "highlighter" && m != "laser");
    let is_erase = m == ERASER || m == "object-eraser";
    let width = if is_erase { state.eraser_scale } else { state.pen_scale };
    let color = if is_erase { [0, 0, 0] } else { state.color };
    let p_time = ev.p_time; let usl_url = ev.payload.get("v2").and_then(|v| v.as_str()).map(str::to_string);
    let cv = state.canvas(cid);
    cv.open = Some(Stroke { id: ev.canvas_id.unwrap_or(0), sid: state.slide_sid.clone(), oid: ev.payload.get("p").and_then(|p| p.get("oid")).and_then(as_i64), mode: mode.clone(), color, width, points: vec![(pt.0, pt.1, p_time)], is_erase, is_highlight: is_temp_hl || is_perm_hl, is_temp_hl, is_perm_hl, t_start: p_time, usl_url, dash: vec![] });
    cv.pointer = Some(pt); cv.pointer_us = p_time; cv.touch();
}
// =====================[ PEN MOVE ]=====================
fn pen_move(state: &mut ReplayState, ev: &Event) {
    let Some(pt) = ev.point() else { return }; let cid = target_canvas(ev, state.current_slide); let p_time = ev.p_time;
    let cv = state.canvas(cid); cv.pointer = Some(pt); cv.pointer_us = p_time;
    if let Some(open) = cv.open.as_mut() { open.points.push((pt.0, pt.1, p_time)); cv.revision += 1 }
}
// =====================[ PEN UP ]=====================
fn pen_up(state: &mut ReplayState, ev: &Event) {
    let pt = ev.point(); let cid = target_canvas(ev, state.current_slide); let p_time = ev.p_time;
    let cv = state.canvas(cid);
    if let Some(mut open) = cv.open.take() {
        if let Some(p) = pt { open.points.push((p.0, p.1, p_time)) }
        if open.points.len() == 1 { open.points.push(open.points[0]) }
        let pts3: Vec<(f64, f64, f64)> = open.points.iter().map(|p| (p.0 as f64, p.1 as f64, p.2 as f64)).collect();
        let smoothed = smooth_stroke(&pts3);
        open.points = smoothed.iter().map(|p| (p.0 as f32, p.1 as f32, p.2 as i64)).collect();
        if open.is_temp_hl { state.temp_hl.push(open.clone()) } else { state.completed.push(open.clone()); cv.strokes.push(open) }
        cv.touch();
    }
    if let Some(p) = pt { cv.pointer = Some(p); cv.pointer_us = p_time }
}
// =====================[ POINTER ]=====================
fn pointer(state: &mut ReplayState, ev: &Event) {
    let Some(pt) = ev.point() else { return }; let cid = target_canvas(ev, state.current_slide); let p_time = ev.p_time;
    let cv = state.canvas(cid); cv.pointer = Some(pt); cv.pointer_us = p_time;
}
// =====================[ TRANSFORM EVENT ]=====================
fn transform_event(state: &mut ReplayState, ev: &Event) {
    let Some(t) = ev.s("t") else { return };
    if t == "e" { state.transform.clear(); return }
    let ids: Vec<i64> = ev.payload.get("data").and_then(|d| d.get("ids")).and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(as_i64).collect()).unwrap_or_default();
    let d = ev.payload.get("data").cloned().unwrap_or_default();
    for oid in ids {
        let entry = state.transform.entry(oid).or_default();
        match t {
            "ts" => { entry.dx += d.get("dx").and_then(as_f64).unwrap_or(0.0) as f32; entry.dy += d.get("dy").and_then(as_f64).unwrap_or(0.0) as f32 }
            "r" => { entry.angle += d.get("dg").and_then(as_f64).unwrap_or(0.0) as f32; entry.px = d.get("cx").and_then(as_f64).unwrap_or(0.0) as f32; entry.py = d.get("cy").and_then(as_f64).unwrap_or(0.0) as f32 }
            "sc" | "sl" => { entry.sx = d.get("sx").and_then(as_f64).unwrap_or(1.0) as f32; entry.sy = d.get("sy").and_then(as_f64).unwrap_or(1.0) as f32 }
            _ => {}
        }
    }
}
// =====================[ POLL EVENT ]=====================
fn poll_event(state: &mut ReplayState, ev: &Event) {
    let d = ev.payload.get("data").cloned().unwrap_or_default();
    let question = d.get("q").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let opts = d.get("data").and_then(|v| v.as_array()).or_else(|| d.get("options").and_then(|v| v.as_array())).cloned().unwrap_or_default();
    let options = opts.iter().map(|o| { let text = if o.is_string() { o.as_str().unwrap_or("").to_string() } else { o.get("answer").or_else(|| o.get("text")).or_else(|| o.get("option")).and_then(|v| v.as_str()).unwrap_or("").to_string() }; let pct = o.get("percentage").or_else(|| o.get("pct")).and_then(as_f64).unwrap_or(0.0) as f32; let correct = o.get("isCorrect").or_else(|| o.get("correct")).and_then(|v| v.as_bool()).unwrap_or(false); (text, pct, correct) }).collect();
    state.active_poll = Some(Poll { t: ev.p_time, question, options, is_open: ev.kind == "opl" });
}
// =====================[ ADVANCE ]=====================
impl ReplayState {
    pub fn advance(&mut self, events: &[Event], t_us: i64) {
        while self.applied < events.len() && events[self.applied].p_time <= t_us { let ev = events[self.applied].clone(); apply_event(self, &ev); self.applied += 1 }
        self.t_us = t_us;
    }
}
// =====================[ BUILD AT ]=====================
pub fn build_at(events: &[Event], t_us: i64) -> ReplayState { let mut st = ReplayState::new(); st.advance(events, t_us); st }
// =====================[ HEX TO RGB ]=====================
pub fn hex_to_rgb(value: &str) -> [u8; 3] {
    let v = value.trim_start_matches('#'); let v = if v.len() == 3 { v.chars().flat_map(|c| [c, c]).collect::<String>() } else { v.to_string() };
    if v.len() != 6 { return [44, 56, 68] }
    let p = |i: usize| u8::from_str_radix(&v[i..i + 2], 16).unwrap_or(0); [p(0), p(2), p(4)]
}
