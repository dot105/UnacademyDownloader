// =====================[ IMPORTS ]=====================
use std::collections::HashMap;
use crate::model::*;
use crate::bezier::smooth_stroke;
// =====================[ CONSTANTS ]=====================
pub const MARKER: &str = "marker";
pub const HIGHLIGHTER: &str = "highlighter";
pub const ERASER: &str = "eraser";
pub const LASER: &str = "laser";
pub const DEFAULT_COLOR: &str = "#2C3844";
// =====================[ STROKE ]=====================
#[derive(Debug, Clone)]
pub struct Stroke {
    pub id: i64,
    pub sid: String,
    pub oid: Option<i64>,
    pub mode: String,
    pub color: [u8; 3],
    pub width: f32,
    pub points: Vec<(f32, f32, i64)>,
    pub is_erase: bool,
    pub is_highlight: bool,
    pub is_temp_hl: bool,
    pub is_perm_hl: bool,
    pub t_start: i64,
    pub usl_url: Option<String>,
    pub dash: Vec<f32>,
}
// =====================[ CANVAS ]=====================
#[derive(Debug, Clone, Default)]
pub struct Canvas {
    pub strokes: Vec<Stroke>,
    pub open: Option<Stroke>,
    pub zoom: f32,
    pub pan: (f32, f32),
    pub pointer: Option<(f32, f32)>,
    pub pointer_us: i64,
    pub revision: u64,
    pub rotation: f32,
    pub gif_url: String,
    pub screen_share: bool,
}
// =====================[ CANVAS HELPERS ]=====================
impl Canvas {
    pub fn new() -> Self { Canvas { zoom: 1.0, ..Default::default() } }
    pub fn touch(&mut self) { self.revision += 1; }
    pub fn pointer_alpha(&self, t_us: i64) -> f32 {
        const HOLD: f32 = 1.0;
        const FADE: f32 = 0.8;
        if self.pointer.is_none() { return 0.0; }
        let age = (t_us - self.pointer_us) as f32 / 1_000_000.0;
        if age <= HOLD { 1.0 } else if age >= HOLD + FADE { 0.0 } else { 1.0 - (age - HOLD) / FADE }
    }
    pub fn visible_strokes(&self) -> impl Iterator<Item = &Stroke> {
        self.strokes.iter().chain(self.open.iter().filter(|s| s.points.len() > 1))
    }
}
// =====================[ SLIDE ]=====================
#[derive(Debug, Clone, Default)]
pub struct Slide {
    pub url: Option<String>,
    pub bg: Option<String>,
    pub background: [u8; 3],
    pub sid: String,
    pub t: i64,
    pub canvas_i: i64,
    pub page: i64,
}
// =====================[ TRANSFORM ]=====================
#[derive(Debug, Clone, Default)]
pub struct TransformEntry {
    pub dx: f32,
    pub dy: f32,
    pub sx: f32,
    pub sy: f32,
    pub angle: f32,
    pub px: f32,
    pub py: f32,
}
// =====================[ POLL ]=====================
#[derive(Debug, Clone, Default)]
pub struct Poll {
    pub t: i64,
    pub question: String,
    pub options: Vec<(String, f32, bool)>,
    pub is_open: bool,
}
// =====================[ REPLAY STATE ]=====================
pub struct ReplayState {
    pub slides: Vec<Slide>,
    pub slide_registry: HashMap<i64, usize>,
    pub pending_as: Vec<(i64, i64, Slide)>,
    pub canvases: HashMap<i64, Canvas>,
    pub current_slide: i64,
    pub color: [u8; 3],
    pub mode: String,
    pub pen_scale: f32,
    pub eraser_scale: f32,
    pub t_us: i64,
    pub applied: usize,
    pub slide_sid: String,
    pub bg_color: [u8; 3],
    pub bg_image_url: String,
    pub rotation: f32,
    pub gif_url: String,
    pub screen_share: bool,
    pub active_poll: Option<Poll>,
    pub completed: Vec<Stroke>,
    pub erased_up_to: i64,
    pub transform: HashMap<i64, TransformEntry>,
    pub temp_hl: Vec<Stroke>,
}
// =====================[ REPLAY STATE DEFAULT ]=====================
impl Default for ReplayState {
    fn default() -> Self { Self::new() }
}
// =====================[ REPLAY STATE IMPL ]=====================
impl ReplayState {
    pub fn new() -> Self {
        ReplayState {
            slides: vec![Slide { url: None, bg: None, background: [17, 17, 24], sid: "init".into(), t: 0, canvas_i: -1, page: -1 }],
            slide_registry: HashMap::new(),
            pending_as: Vec::new(),
            canvases: HashMap::new(),
            current_slide: 0,
            color: hex_to_rgb(DEFAULT_COLOR),
            mode: MARKER.to_string(),
            pen_scale: 1.0,
            eraser_scale: 1.0,
            t_us: 0,
            applied: 0,
            slide_sid: "init".into(),
            bg_color: [17, 17, 24],
            bg_image_url: String::new(),
            rotation: 0.0,
            gif_url: String::new(),
            screen_share: false,
            active_poll: None,
            completed: Vec::new(),
            erased_up_to: -1,
            transform: HashMap::new(),
            temp_hl: Vec::new(),
        }
    }
    pub fn canvas(&mut self, cid: i64) -> &mut Canvas { self.canvases.entry(cid).or_insert_with(Canvas::new) }
    pub fn active(&self) -> Option<&Canvas> { self.canvases.get(&self.current_slide) }
    pub fn active_slide(&self) -> Option<&Slide> { self.slides.get(self.current_slide as usize) }
}
// =====================[ TARGET CANVAS ]=====================
fn target_canvas(ev: &Event, current_slide: i64) -> i64 { ev.canvas_id.unwrap_or(current_slide) }
// =====================[ PAGE FROM URL ]=====================
fn page_from_url(url: &str) -> i64 {
    if let Some(idx) = url.find("page=") {
        let rest = &url[idx + 5..];
        let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return num.parse().unwrap_or(-1);
    }
    -1
}
// =====================[ FINALIZE SLIDES ]=====================
fn finalize_slides(state: &mut ReplayState) {
    if state.pending_as.is_empty() { return; }
    let mut pending = std::mem::take(&mut state.pending_as);
    pending.sort_by_key(|(t, seq, _)| (*t, *seq));
    let mut blocks: Vec<(i64, Vec<Slide>)> = Vec::new();
    for (_, _, slide) in pending {
        if let Some(last) = blocks.last_mut() {
            if last.0 == slide.canvas_i { last.1.push(slide); continue; }
        }
        blocks.push((slide.canvas_i, vec![slide]));
    }
    let mut final_list: Vec<Slide> = state.slides.clone();
    final_list.truncate(1);
    for (canvas_i, mut items) in blocks {
        items.sort_by(|a, b| {
            if a.page >= 0 && b.page >= 0 { a.page.cmp(&b.page) } else { a.t.cmp(&b.t) }
        });
        let splice_idx = if canvas_i < 0 { final_list.len() } else { canvas_i as usize };
        while final_list.len() < splice_idx {
            final_list.push(Slide::default());
        }
        for (offset, item) in items.into_iter().enumerate() {
            final_list.insert(splice_idx + offset, item);
        }
    }
    for (idx, slide) in final_list.iter().enumerate() {
        state.slide_registry.insert(idx as i64, idx);
        state.slide_registry.insert(slide.canvas_i, idx);
    }
    state.slides = final_list;
}
// =====================[ APPLY EVENT ]=====================
fn apply_event(state: &mut ReplayState, ev: &Event) {
    state.t_us = ev.p_time;
    match (ev.plugin.as_str(), ev.kind.as_str()) {
        (P_CANVAS, CW_DOWN) => pen_down(state, ev),
        (P_CANVAS, CW_MOVE) => pen_move(state, ev),
        (P_CANVAS, CW_UP) => pen_up(state, ev),
        (P_CANVAS, CW_POINTER) => pointer(state, ev),
        (P_CANVAS, CW_ERASE_ALL) => {
            let cid = target_canvas(ev, state.current_slide);
            let cv = state.canvas(cid);
            cv.strokes.clear();
            cv.open = None;
            cv.touch();
            state.erased_up_to = ev.p_time;
        }
        (P_CANVAS, CW_ZOOM) => {
            let cid = target_canvas(ev, state.current_slide);
            let v = ev.f("v").unwrap_or(1.0) as f32;
            let cv = state.canvas(cid);
            cv.zoom = v.max(1.0);
            cv.touch();
        }
        (P_CANVAS, CW_PAN) => {
            let cid = target_canvas(ev, state.current_slide);
            if let Some(v) = ev.payload.get("v") {
                let x = v.get("x").and_then(as_f64).unwrap_or(0.0) as f32;
                let y = v.get("y").and_then(as_f64).unwrap_or(0.0) as f32;
                let cv = state.canvas(cid);
                cv.pan = (x, y);
                cv.touch();
            }
        }
        (P_CANVAS, CW_DELETE) => {
            if let Some(ids) = ev.payload.get("ids").and_then(|v| v.as_array()) {
                let cid = target_canvas(ev, state.current_slide);
                let cv = state.canvas(cid);
                for id in ids {
                    if let Some(oid) = as_i64(id) { cv.strokes.retain(|s| s.oid != Some(oid)); }
                }
                cv.touch();
            }
        }
        (P_CANVAS, CW_TRANSFORM) => transform_event(state, ev),
        (P_DOC, DC_ADD_SLIDE) => {
            let url = ev.s("u").unwrap_or("").to_string();
            let bc = ev.s("bc").map(hex_to_rgb).unwrap_or([255, 255, 255]);
            let sid = ev.s("uid").unwrap_or("").to_string();
            let canvas_i = ev.i("i").unwrap_or(-1);
            let page = page_from_url(&url);
            let slide = Slide { url: Some(url), bg: None, background: bc, sid, t: ev.p_time, canvas_i, page };
            state.pending_as.push((ev.p_time, ev.seq, slide));
        }
        (P_DOC, DC_SHOW_SLIDE) => {
            if let Some(s) = ev.i("s") {
                if let Some(&pos) = state.slide_registry.get(&s) {
                    state.current_slide = pos as i64;
                } else {
                    state.current_slide = s;
                }
                if let Some(slide) = state.slides.get(state.current_slide as usize) {
                    state.slide_sid = slide.sid.clone();
                    state.bg_color = slide.background;
                    state.bg_image_url = slide.bg.clone().unwrap_or_default();
                    state.rotation = 0.0;
                    state.gif_url.clear();
                }
            } else if let Some(obj) = ev.payload.get("s") {
                if obj.is_object() {
                    let url = obj.get("u").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let bc = obj.get("bc").and_then(|v| v.as_str()).map(hex_to_rgb).unwrap_or([255, 255, 255]);
                    let sid = obj.get("uid").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let canvas_i = obj.get("i").and_then(as_i64).unwrap_or(-1);
                    let page = page_from_url(&url);
                    let slide = Slide { url: Some(url), bg: None, background: bc, sid, t: ev.p_time, canvas_i, page };
                    state.pending_as.push((ev.p_time, ev.seq, slide));
                }
            }
        }
        (P_DOC, DC_BG_COLOR) => {
            if let Some(c) = ev.s("c").or_else(|| ev.s("bc")) { state.bg_color = hex_to_rgb(c); }
            if let Some(bg) = ev.s("bg") { state.bg_image_url = bg.to_string(); }
        }
        (P_DOC, DC_COLOR) => { if let Some(c) = ev.s("c") { state.color = hex_to_rgb(c); } }
        (P_DOC, DC_MODE) => { if let Some(m) = ev.s("m") { state.mode = m.to_string(); } }
        (P_DOC, DC_PEN_SIZE) => { if let Some(s) = ev.f("s") { state.pen_scale = s as f32; } }
        (P_DOC, DC_ERASER_SIZE) => { if let Some(s) = ev.f("s") { state.eraser_scale = s as f32; } }
        (P_DOC, DC_ROTATE) => { state.rotation = ev.f("v").unwrap_or(0.0) as f32; }
        (P_DOC, DC_PLAY_GIF) => { state.gif_url = ev.s("v").unwrap_or("").to_string(); }
        (P_DOC, DC_SCREEN_SHARE) => { state.screen_share = ev.payload.get("v").and_then(|v| v.as_bool()).unwrap_or(false); }
        (P_MAIN, MC_CLASS_END) => {}
        (P_POLL, _) => poll_event(state, ev),
        _ => {}
    }
}
// =====================[ PEN DOWN ]=====================
fn pen_down(state: &mut ReplayState, ev: &Event) {
    let Some(pt) = ev.point() else { return; };
    let cid = target_canvas(ev, state.current_slide);
    let mode = ev.payload.get("mode").and_then(|v| v.as_str()).unwrap_or(&state.mode).to_string();
    let m = mode.to_lowercase();
    let is_temp_hl = m == LASER || m == HIGHLIGHTER;
    let is_perm_hl = m == "marker_highlighter" || m == "permanent_highlighter" || (m.contains("high") && m != "highlighter" && m != "laser");
    let is_erase = m == ERASER || m == "object-eraser";
    let width = if is_erase { state.eraser_scale } else { state.pen_scale };
    let color = if is_erase { [0, 0, 0] } else { state.color };
    let p_time = ev.p_time;
    let usl_url = ev.payload.get("v2").and_then(|v| v.as_str()).map(str::to_string);
    let sid = state.slide_sid.clone();
    let oid = ev.payload.get("p").and_then(|p| p.get("oid")).and_then(as_i64);
    let cv = state.canvas(cid);
    cv.open = Some(Stroke {
        id: ev.canvas_id.unwrap_or(0), sid, oid, mode, color, width,
        points: vec![(pt.0, pt.1, p_time)], is_erase,
        is_highlight: is_temp_hl || is_perm_hl, is_temp_hl, is_perm_hl,
        t_start: p_time, usl_url, dash: vec![],
    });
    cv.pointer = Some(pt);
    cv.pointer_us = p_time;
    cv.touch();
}
// =====================[ PEN MOVE ]=====================
fn pen_move(state: &mut ReplayState, ev: &Event) {
    let Some(pt) = ev.point() else { return; };
    let cid = target_canvas(ev, state.current_slide);
    let p_time = ev.p_time;
    let cv = state.canvas(cid);
    cv.pointer = Some(pt);
    cv.pointer_us = p_time;
    if let Some(open) = cv.open.as_mut() {
        open.points.push((pt.0, pt.1, p_time));
        cv.revision += 1;
    }
}
// =====================[ PEN UP ]=====================
fn pen_up(state: &mut ReplayState, ev: &Event) {
    let pt = ev.point();
    let cid = target_canvas(ev, state.current_slide);
    let p_time = ev.p_time;
    let cv = state.canvas(cid);
    if let Some(mut open) = cv.open.take() {
        if let Some(p) = pt { open.points.push((p.0, p.1, p_time)); }
        if open.points.len() == 1 { open.points.push(open.points[0]); }
        let pts3: Vec<(f64, f64, f64)> = open.points.iter().map(|p| (p.0 as f64, p.1 as f64, p.2 as f64)).collect();
        let smoothed = smooth_stroke(&pts3);
        open.points = smoothed.iter().map(|p| (p.0 as f32, p.1 as f32, p.2 as i64)).collect();
        if open.is_temp_hl {
            cv.touch();
            state.temp_hl.push(open);
        } else {
            cv.strokes.push(open.clone());
            cv.touch();
            state.completed.push(open);
        }
    }
    if let Some(p) = pt {
        let cv = state.canvas(cid);
        cv.pointer = Some(p);
        cv.pointer_us = p_time;
    }
}
// =====================[ POINTER ]=====================
fn pointer(state: &mut ReplayState, ev: &Event) {
    let Some(pt) = ev.point() else { return; };
    let cid = target_canvas(ev, state.current_slide);
    let p_time = ev.p_time;
    let cv = state.canvas(cid);
    cv.pointer = Some(pt);
    cv.pointer_us = p_time;
}
// =====================[ TRANSFORM EVENT ]=====================
fn transform_event(state: &mut ReplayState, ev: &Event) {
    let Some(t) = ev.s("t") else { return; };
    if t == "e" { state.transform.clear(); return; }
    let ids: Vec<i64> = ev.payload.get("data").and_then(|d| d.get("ids")).and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(as_i64).collect()).unwrap_or_default();
    let d = ev.payload.get("data").cloned().unwrap_or_default();
    for oid in ids {
        let entry = state.transform.entry(oid).or_default();
        match t {
            "ts" => {
                entry.dx += d.get("dx").and_then(as_f64).unwrap_or(0.0) as f32;
                entry.dy += d.get("dy").and_then(as_f64).unwrap_or(0.0) as f32;
            }
            "r" => {
                entry.angle += d.get("dg").and_then(as_f64).unwrap_or(0.0) as f32;
                entry.px = d.get("cx").and_then(as_f64).unwrap_or(0.0) as f32;
                entry.py = d.get("cy").and_then(as_f64).unwrap_or(0.0) as f32;
            }
            "sc" | "sl" => {
                entry.sx = d.get("sx").and_then(as_f64).unwrap_or(1.0) as f32;
                entry.sy = d.get("sy").and_then(as_f64).unwrap_or(1.0) as f32;
            }
            _ => {}
        }
    }
}
// =====================[ POLL EVENT ]=====================
fn poll_event(state: &mut ReplayState, ev: &Event) {
    let d = ev.payload.get("data").cloned().unwrap_or_default();
    let question = d.get("q").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let opts = d.get("data").and_then(|v| v.as_array()).or_else(|| d.get("options").and_then(|v| v.as_array())).cloned().unwrap_or_default();
    let options = opts.iter().map(|o| {
        let text = if o.is_string() { o.as_str().unwrap_or("").to_string() } else { o.get("answer").or_else(|| o.get("text")).or_else(|| o.get("option")).and_then(|v| v.as_str()).unwrap_or("").to_string() };
        let pct = o.get("percentage").or_else(|| o.get("pct")).and_then(as_f64).unwrap_or(0.0) as f32;
        let correct = o.get("isCorrect").or_else(|| o.get("correct")).and_then(|v| v.as_bool()).unwrap_or(false);
        (text, pct, correct)
    }).collect();
    state.active_poll = Some(Poll { t: ev.p_time, question, options, is_open: ev.kind == "opl" });
}
// =====================[ ADVANCE ]=====================
impl ReplayState {
    pub fn advance(&mut self, events: &[Event], t_us: i64) {
        while self.applied < events.len() && events[self.applied].p_time <= t_us {
            let ev = events[self.applied].clone();
            apply_event(self, &ev);
            self.applied += 1;
        }
        finalize_slides(self);
        self.t_us = t_us;
    }
}
// =====================[ BUILD AT ]=====================
pub fn build_at(events: &[Event], t_us: i64) -> ReplayState {
    let mut st = ReplayState::new();
    st.advance(events, t_us);
    st
}
// =====================[ HEX TO RGB ]=====================
pub fn hex_to_rgb(value: &str) -> [u8; 3] {
    let v = value.trim_start_matches('#');
    let v = if v.len() == 3 { v.chars().flat_map(|c| [c, c]).collect::<String>() } else { v.to_string() };
    if v.len() != 6 { return [44, 56, 68]; }
    let p = |i: usize| u8::from_str_radix(&v[i..i + 2], 16).unwrap_or(0);
    [p(0), p(2), p(4)]
}
// =====================[ POINTER STREAM ]=====================
#[derive(Debug, Clone)]
pub struct PointerPoint { pub t: i64, pub x: f32, pub y: f32 }
// =====================[ SNAPSHOT ]=====================
#[derive(Debug, Clone)]
pub struct Snapshot { pub t: i64, pub ev_idx: usize, pub sid: String, pub slide_url: String, pub bg_color: [u8; 3], pub bg_image_url: String, pub color: [u8; 3], pub mode: String, pub pen_scale: f32, pub eraser_scale: f32, pub pan: (f32, f32), pub zoom: f32, pub rotation: f32, pub gif_url: String, pub screen_share: bool }
// =====================[ TIMELINE ]=====================
pub struct Timeline { pub events: Vec<Event>, pub pointer_stream: Vec<PointerPoint>, pub snapshots: Vec<Snapshot>, pub max_t: i64, pub record_start_ms: i64 }
// =====================[ TIMELINE BUILD ]=====================
impl Timeline {
    pub fn build(events: Vec<Event>) -> Self {
        let pointer_stream: Vec<PointerPoint> = events.iter()
            .filter(|e| matches!(e.kind.as_str(), CW_DOWN | CW_MOVE | CW_UP | CW_POINTER) && e.plugin == P_CANVAS)
            .filter_map(|e| { let p = e.point()?; Some(PointerPoint { t: e.p_time, x: p.0, y: p.1 }) })
            .collect();
        let max_t = events.last().map(|e| e.p_time).unwrap_or(0);
        let mut record_start_ms = -1i64;
        for ev in &events {
            if ev.plugin == P_MAIN && ev.kind == "cp" {
                if let Some(ct) = ev.payload.get("ct").and_then(as_i64) {
                    if ct > 0 { record_start_ms = ct - (ev.p_time / 1000); break; }
                }
            }
        }
        let mut snapshots = Vec::new();
        let mut state = Snapshot {
            t: 0, ev_idx: 0, sid: "init".into(), slide_url: String::new(),
            bg_color: [17, 17, 24], bg_image_url: String::new(),
            color: hex_to_rgb(DEFAULT_COLOR), mode: MARKER.into(),
            pen_scale: 1.0, eraser_scale: 1.0, pan: (0.0, 0.0), zoom: 1.0,
            rotation: 0.0, gif_url: String::new(), screen_share: false,
        };
        let mut next_snap = 0i64;
        for (i, ev) in events.iter().enumerate() {
            while ev.p_time >= next_snap && next_snap <= max_t + 10_000_000 {
                snapshots.push(Snapshot { t: next_snap, ev_idx: i, ..state.clone() });
                next_snap += 10_000_000;
            }
            match (ev.plugin.as_str(), ev.kind.as_str()) {
                (P_DOC, DC_SHOW_SLIDE) => {
                    if let Some(_s) = ev.i("s") {
                        state.sid = ev.s("uid").unwrap_or("").to_string();
                        state.slide_url = ev.s("u").unwrap_or("").to_string();
                        state.bg_color = ev.s("bc").map(hex_to_rgb).unwrap_or([255, 255, 255]);
                        state.bg_image_url = ev.s("bg").unwrap_or("").to_string();
                        state.rotation = 0.0;
                        state.gif_url.clear();
                    }
                }
                (P_DOC, DC_BG_COLOR) => {
                    if let Some(c) = ev.s("c").or_else(|| ev.s("bc")) { state.bg_color = hex_to_rgb(c); }
                    if let Some(bg) = ev.s("bg") { state.bg_image_url = bg.to_string(); }
                }
                (P_DOC, DC_COLOR) => { if let Some(c) = ev.s("c") { state.color = hex_to_rgb(c); } }
                (P_DOC, DC_MODE) => { if let Some(m) = ev.s("m") { state.mode = m.to_string(); } }
                (P_DOC, DC_PEN_SIZE) => { if let Some(s) = ev.f("s") { state.pen_scale = s as f32; } }
                (P_DOC, DC_ERASER_SIZE) => { if let Some(s) = ev.f("s") { state.eraser_scale = s as f32; } }
                (P_DOC, DC_ROTATE) => { state.rotation = ev.f("v").unwrap_or(0.0) as f32; }
                (P_DOC, DC_PLAY_GIF) => { state.gif_url = ev.s("v").unwrap_or("").to_string(); }
                (P_DOC, DC_SCREEN_SHARE) => { state.screen_share = ev.payload.get("v").and_then(|v| v.as_bool()).unwrap_or(false); }
                (P_CANVAS, CW_ZOOM) => { state.zoom = ev.f("v").unwrap_or(1.0) as f32; }
                (P_CANVAS, CW_PAN) => {
                    if let Some(v) = ev.payload.get("v") {
                        state.pan = (v.get("x").and_then(as_f64).unwrap_or(0.0) as f32, v.get("y").and_then(as_f64).unwrap_or(0.0) as f32);
                    }
                }
                _ => {}
            }
        }
        if snapshots.is_empty() { snapshots.push(Snapshot { t: 0, ev_idx: 0, ..state }); }
        Timeline { events, pointer_stream, snapshots, max_t, record_start_ms }
    }
}
// =====================[ POINTER INTERP ]=====================
pub fn interpolate_pointer(stream: &[PointerPoint], target_us: i64, hide_us: i64) -> Option<(f32, f32)> {
    if stream.is_empty() { return None; }
    let mut lo = 0usize;
    let mut hi = stream.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if stream[mid].t < target_us { lo = mid + 1; } else { hi = mid; }
    }
    if lo == 0 {
        return if target_us - stream[0].t <= hide_us { Some((stream[0].x, stream[0].y)) } else { None };
    }
    let p0 = &stream[lo - 1];
    if target_us - p0.t > hide_us { return None; }
    if lo >= stream.len() { return Some((p0.x, p0.y)); }
    let p1 = &stream[lo];
    if p1.t == p0.t { return Some((p0.x, p0.y)); }
    let ratio = (target_us - p0.t) as f32 / (p1.t - p0.t) as f32;
    Some((p0.x + (p1.x - p0.x) * ratio, p0.y + (p1.y - p0.y) * ratio))
}
// =====================[ DO SEEK ]=====================
pub fn do_seek(timeline: &Timeline, target_us: i64) -> ReplayState {
    let snap_idx = ((target_us.max(0) / 10_000_000) as usize).min(timeline.snapshots.len().saturating_sub(1));
    let snap = &timeline.snapshots[snap_idx];
    let mut state = ReplayState::new();
    state.slide_sid = snap.sid.clone();
    state.bg_color = snap.bg_color;
    state.bg_image_url = snap.bg_image_url.clone();
    state.color = snap.color;
    state.mode = snap.mode.clone();
    state.pen_scale = snap.pen_scale;
    state.eraser_scale = snap.eraser_scale;
    state.rotation = snap.rotation;
    state.gif_url = snap.gif_url.clone();
    state.screen_share = snap.screen_share;
    for i in snap.ev_idx..timeline.events.len() {
        let ev = &timeline.events[i];
        if ev.p_time > target_us { break; }
        apply_event(&mut state, ev);
    }
    finalize_slides(&mut state);
    let cur_canvas = state.canvases.get(&state.current_slide).cloned();
    if let Some(mut c) = cur_canvas {
        c.zoom = snap.zoom;
        c.pan = snap.pan;
        state.canvases.insert(state.current_slide, c);
    }
    state.t_us = target_us;
    state.applied = timeline.events.iter().position(|e| e.p_time > target_us).unwrap_or(timeline.events.len());
    state
}
// =====================[ TICK DRAW ]=====================
pub fn tick_draw(state: &mut ReplayState, events: &[Event], target_us: i64) -> bool {
    let mut needs_redraw = false;
    while state.applied < events.len() {
        let ev = events[state.applied].clone();
        if ev.p_time > target_us { break; }
        apply_event(state, &ev);
        state.applied += 1;
    }
    finalize_slides(state);
    state.t_us = target_us;
    if let Some(cv) = state.canvases.get(&state.current_slide) {
        if cv.revision != 0 { needs_redraw = true; }
    }
    needs_redraw
}
// =====================[ VIEWPORT CLAMP ]=====================
pub fn clamp_viewport(zoom: &mut f32, pan: &mut (f32, f32)) {
    if *zoom < 1.0 { *zoom = 1.0; }
    if *zoom > 5.0 { *zoom = 5.0; }
    if pan.0 > 0.0 { pan.0 = 0.0; }
    if pan.1 > 0.0 { pan.1 = 0.0; }
    if pan.0 < 1.0 - *zoom { pan.0 = 1.0 - *zoom; }
    if pan.1 < 1.0 - *zoom { pan.1 = 1.0 - *zoom; }
}
// =====================[ PROJECT POINT ]=====================
pub fn project_point(x: f32, y: f32, zoom: f32, pan: (f32, f32), w: f32, h: f32) -> (f32, f32) {
    (x * zoom + pan.0 * w, y * zoom + pan.1 * h)
}
// =====================[ TRANSFORM OF ]=====================
pub fn transform_of(zoom: f32, pan: (f32, f32)) -> (f32, f32, f32) {
    if zoom <= 1.0 + 1e-6 { return (1.0, 0.0, 0.0); }
    let limit = 0.5 * (zoom - 1.0);
    let clamp = |v: f32| v.clamp(-limit, limit);
    (zoom, clamp(pan.0 * zoom), clamp(pan.1 * zoom))
}
