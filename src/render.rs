// =====================[ IMPORTS ]=====================
use std::collections::HashMap;
use tiny_skia::{BlendMode, Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Stroke as SkStroke, Transform};
use crate::assets::SlideCache;
use crate::state::{Canvas, ReplayState, Stroke, transform_of, project_point};
use crate::yuv::{self, YuvFrame};
// =====================[ CONSTANTS ]=====================
pub const BASE_WIDTH: f32 = 1080.0;
pub const MARKER_PX: f32 = 2.0;
pub const HIGHLIGHT_PX: f32 = 10.0;
pub const ERASER_PX: f32 = 12.0;
pub const HIGHLIGHT_ALPHA: f32 = 0.35;
pub const POINTER_COLOR: [u8; 3] = [255, 40, 40];
pub const POINTER_GLOW: [u8; 3] = [255, 64, 64];
pub const POINTER_HIDE_US: i64 = 2_000_000;
// =====================[ THEME ]=====================
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub width: u32,
    pub height: u32,
    pub show_pointer: bool,
    pub apply_zoom: bool,
}
// =====================[ THEME NEW ]=====================
impl Theme {
    pub fn new(width: u32, height: u32) -> Self { Theme { width, height, show_pointer: true, apply_zoom: true } }
    pub fn scale(&self) -> f32 { self.width as f32 / BASE_WIDTH }
}
// =====================[ FRAME KEY ]=====================
#[derive(PartialEq, Clone)]
struct FrameKey {
    slide: i64,
    revision: u64,
    transform: (u32, u32, u32),
    url: Option<String>,
    rotation: u32,
    gif: String,
    screen_share: bool,
}
// =====================[ BOARD RENDERER ]=====================
pub struct BoardRenderer {
    pub theme: Theme,
    slides: SlideCache,
    backgrounds: HashMap<String, Pixmap>,
    cached: Option<(FrameKey, Pixmap)>,
    scratch: Pixmap,
    board_yuv: YuvFrame,
    out_yuv: YuvFrame,
    dirty: Option<(u32, u32, u32, u32)>,
}
// =====================[ BOARD RENDERER NEW ]=====================
impl BoardRenderer {
    pub fn new(theme: Theme, slides: SlideCache) -> Self {
        BoardRenderer {
            scratch: Pixmap::new(theme.width, theme.height).expect("pixmap"),
            board_yuv: YuvFrame::new(theme.width, theme.height),
            out_yuv: YuvFrame::new(theme.width, theme.height),
            dirty: None,
            theme,
            slides,
            backgrounds: HashMap::new(),
            cached: None,
        }
    }
}
// =====================[ BACKGROUND ]=====================
impl BoardRenderer {
    fn background(&mut self, state: &ReplayState) -> Pixmap {
        let (w, h) = (self.theme.width, self.theme.height);
        let slide = state.active_slide();
        if let Some(url) = slide.and_then(|s| s.url.as_deref()) {
            if !self.backgrounds.contains_key(url) {
                if let Some(img) = self.slides.get(url, w, h) {
                    self.backgrounds.insert(url.to_string(), img);
                }
            }
            if let Some(img) = self.backgrounds.get(url) {
                return img.clone();
            }
        }
        let bg = slide.map(|s| s.background).unwrap_or(state.bg_color);
        let mut p = Pixmap::new(w, h).expect("pixmap");
        p.fill(Color::from_rgba8(bg[0], bg[1], bg[2], 255));
        p
    }
}
// =====================[ DRAW DOT ]=====================
fn draw_dot(target: &mut Pixmap, x: f32, y: f32, stroke: &Stroke, scale: f32, w: f32, h: f32) {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    let line_width = if stroke.is_erase {
        (stroke.width + 10.0) * scale
    } else if stroke.is_highlight {
        stroke.width * scale * 5.0
    } else {
        stroke.width * scale * 3.0
    };
    let radius = (line_width / 2.0).max(1.0);
    if stroke.is_erase {
        paint.set_color(Color::TRANSPARENT);
        paint.blend_mode = BlendMode::Clear;
    } else if stroke.is_highlight {
        paint.set_color(Color::from_rgba8(stroke.color[0], stroke.color[1], stroke.color[2], (255.0 * HIGHLIGHT_ALPHA) as u8));
        paint.blend_mode = BlendMode::SourceOver;
    } else {
        paint.set_color(Color::from_rgba8(stroke.color[0], stroke.color[1], stroke.color[2], 255));
        paint.blend_mode = BlendMode::SourceOver;
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(x * w, y * h, radius);
    if let Some(path) = pb.finish() {
        target.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}
// =====================[ DRAW CURVE ]=====================
fn draw_curve(target: &mut Pixmap, last_x: f32, last_y: f32, mid_x: f32, mid_y: f32, x: f32, y: f32, stroke: &Stroke, scale: f32, w: f32, h: f32) {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    let line_width = if stroke.is_erase {
        (stroke.width + 10.0) * scale
    } else if stroke.is_highlight {
        stroke.width * scale * 5.0
    } else {
        stroke.width * scale * 3.0
    };
    if stroke.is_erase {
        paint.set_color(Color::TRANSPARENT);
        paint.blend_mode = BlendMode::Clear;
    } else if stroke.is_highlight {
        paint.set_color(Color::from_rgba8(stroke.color[0], stroke.color[1], stroke.color[2], (255.0 * HIGHLIGHT_ALPHA) as u8));
        paint.blend_mode = BlendMode::SourceOver;
    } else {
        paint.set_color(Color::from_rgba8(stroke.color[0], stroke.color[1], stroke.color[2], 255));
        paint.blend_mode = BlendMode::SourceOver;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(mid_x * w, mid_y * h);
    pb.quad_to(last_x * w, last_y * h, x * w, y * h);
    if let Some(path) = pb.finish() {
        let sk = SkStroke { width: line_width.max(1.0), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
        target.stroke_path(&path, &paint, &sk, Transform::identity(), None);
    }
}
// =====================[ DRAW STROKE ]=====================
fn draw_stroke(target: &mut Pixmap, stroke: &Stroke, target_us: i64, scale: f32, w: f32, h: f32, hl_target: &mut Pixmap) {
    let pts: Vec<(f32, f32, i64)> = stroke.points.iter().filter(|p| p.2 <= target_us).cloned().collect();
    if pts.is_empty() { return; }
    let dest = if stroke.is_highlight && !stroke.is_erase { hl_target } else { target };
    let first = pts[0];
    if pts.len() == 1 {
        draw_dot(dest, first.0, first.1, stroke, scale, w, h);
        return;
    }
    if !stroke.is_highlight && !stroke.is_erase {
        draw_dot(dest, first.0, first.1, stroke, scale, w, h);
    }
    let mut last_x = first.0;
    let mut last_y = first.1;
    let mut mid_x = last_x;
    let mut mid_y = last_y;
    for i in 1..pts.len() {
        let (nx, ny, _) = pts[i];
        let (mx, my) = if i == pts.len() - 1 { (nx, ny) } else { ((last_x + nx) / 2.0, (last_y + ny) / 2.0) };
        draw_curve(dest, last_x, last_y, mid_x, mid_y, mx, my, stroke, scale, w, h);
        last_x = nx;
        last_y = ny;
        mid_x = mx;
        mid_y = my;
    }
}
// =====================[ DRAW POINTER ]=====================
fn draw_pointer(frame: &mut Pixmap, state: &ReplayState, canvas: &Canvas, theme: &Theme) -> Option<(u32, u32, u32, u32)> {
    if !theme.show_pointer { return None; }
    let alpha = canvas.pointer_alpha(state.t_us);
    if alpha <= 0.02 { return None; }
    let Some((mut x, mut y)) = canvas.pointer else { return None; };
    if theme.apply_zoom {
        let (px, py) = project_point(x, y, canvas.zoom, canvas.pan, 1.0, 1.0);
        x = px;
        y = py;
    }
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) { return None; }
    let cx = x * theme.width as f32;
    let cy = y * theme.height as f32;
    let r = (9.0 * theme.scale()).max(3.0);
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r * 1.9);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(POINTER_GLOW[0], POINTER_GLOW[1], POINTER_GLOW[2], (0.20 * alpha * 255.0) as u8));
        paint.anti_alias = true;
        frame.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(POINTER_COLOR[0], POINTER_COLOR[1], POINTER_COLOR[2], (0.75 * alpha * 255.0) as u8));
        paint.anti_alias = true;
        frame.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
    let pad = r * 2.2;
    Some(((cx - pad).max(0.0) as u32, (cy - pad).max(0.0) as u32, (cx + pad).min(theme.width as f32) as u32, (cy + pad).min(theme.height as f32) as u32))
}
// =====================[ APPLY ZOOM ]=====================
fn apply_zoom(board: &Pixmap, canvas: &Canvas, theme: &Theme) -> Option<Pixmap> {
    let (s, tx, ty) = transform_of(canvas.zoom, canvas.pan);
    if !theme.apply_zoom || (s <= 1.0 + 1e-6 && tx == 0.0 && ty == 0.0) { return None; }
    let (w, h) = (theme.width as f32, theme.height as f32);
    let mut out = Pixmap::new(theme.width, theme.height)?;
    let t = Transform::from_row(s, 0.0, 0.0, s, (0.5 - 0.5 * s + tx) * w, (0.5 - 0.5 * s + ty) * h);
    let mut paint = PixmapPaint::default();
    paint.quality = tiny_skia::FilterQuality::Bilinear;
    out.draw_pixmap(0, 0, board.as_ref(), &paint, t, None);
    Some(out)
}
// =====================[ RESTORE REGION ]=====================
fn restore(scratch: &mut Pixmap, board: &Pixmap, rect: (u32, u32, u32, u32), width: u32) {
    let (x0, y0, x1, y1) = rect;
    let w = width as usize;
    let dst = scratch.data_mut();
    let src = board.data();
    for y in y0 as usize..y1 as usize {
        let a = (y * w + x0 as usize) * 4;
        let b = (y * w + x1 as usize) * 4;
        if b <= src.len() { dst[a..b].copy_from_slice(&src[a..b]); }
    }
}
// =====================[ RENDER ]=====================
impl BoardRenderer {
    pub fn render(&mut self, state: &ReplayState) -> &YuvFrame {
        let empty = Canvas { zoom: 1.0, ..Default::default() };
        let canvas = state.active().unwrap_or(&empty).clone();
        let (s, tx, ty) = transform_of(canvas.zoom, canvas.pan);
        let key = FrameKey {
            slide: state.current_slide,
            revision: canvas.revision,
            transform: (s.to_bits(), tx.to_bits(), ty.to_bits()),
            url: state.active_slide().and_then(|s| s.url.clone()),
            rotation: canvas.rotation.to_bits(),
            gif: state.gif_url.clone(),
            screen_share: state.screen_share,
        };
        let hit = matches!(&self.cached, Some((k, _)) if *k == key);
        if !hit {
            let mut board = self.background(state);
            let w = self.theme.width as f32;
            let h = self.theme.height as f32;
            let scale = self.theme.scale();
            let mut hl_layer = Pixmap::new(self.theme.width, self.theme.height).expect("pixmap");
            let mut draw_layer = Pixmap::new(self.theme.width, self.theme.height).expect("pixmap");
            draw_layer.fill(Color::TRANSPARENT);
            for stroke in &canvas.strokes {
                if stroke.t_start > state.t_us { continue; }
                if stroke.t_start <= state.erased_up_to { continue; }
                draw_stroke(&mut draw_layer, stroke, state.t_us, scale, w, h, &mut hl_layer);
            }
            if let Some(open) = &canvas.open {
                if open.points.len() > 1 {
                    draw_stroke(&mut draw_layer, open, state.t_us, scale, w, h, &mut hl_layer);
                }
            }
            let mut paint = PixmapPaint::default();
            paint.quality = tiny_skia::FilterQuality::Nearest;
            board.draw_pixmap(0, 0, hl_layer.as_ref(), &paint, Transform::identity(), None);
            board.draw_pixmap(0, 0, draw_layer.as_ref(), &paint, Transform::identity(), None);
            if let Some(zoomed) = apply_zoom(&board, &canvas, &self.theme) {
                board = zoomed;
            }
            yuv::convert(&board.as_ref(), &mut self.board_yuv);
            self.scratch.data_mut().copy_from_slice(board.data());
            self.cached = Some((key, board));
            self.out_yuv.copy_from(&self.board_yuv);
            self.dirty = None;
        }
        let Self { cached, scratch, theme, out_yuv, dirty, .. } = self;
        let board = &cached.as_ref().expect("just populated").1;
        if let Some(rect) = dirty.take() {
            restore(scratch, board, rect, theme.width);
            yuv::convert_region(&scratch.as_ref(), out_yuv, rect);
        }
        let canvas_now = state.active().cloned().unwrap_or_default();
        if let Some(rect) = draw_pointer(scratch, state, &canvas_now, theme) {
            yuv::convert_region(&scratch.as_ref(), out_yuv, rect);
            *dirty = Some(rect);
        }
        out_yuv
    }
}
// =====================[ RGBA TO RGB ]=====================
pub fn rgba_to_rgb(src: &PixmapRef, out: &mut Vec<u8>) {
    out.clear();
    out.reserve(src.width() as usize * src.height() as usize * 3);
    for px in src.pixels() {
        let a = px.alpha();
        if a == 255 {
            out.extend_from_slice(&[px.red(), px.green(), px.blue()]);
        } else if a == 0 {
            out.extend_from_slice(&[0, 0, 0]);
        } else {
            let f = 255.0 / a as f32;
            out.extend_from_slice(&[(px.red() as f32 * f).min(255.0) as u8, (px.green() as f32 * f).min(255.0) as u8, (px.blue() as f32 * f).min(255.0) as u8]);
        }
    }
}
// =====================[ BLANK ]=====================
pub fn blank(width: u32, height: u32) -> Vec<u8> { vec![255u8; (width * height * 3) as usize] }
