// =====================[ IMPORTS ]=====================
use anyhow::{anyhow, Result}; use serde_json::Value;
// =====================[ PLUGINS ]=====================
pub const P_CANVAS: &str = "cw"; pub const P_DOC: &str = "dcn"; pub const P_MAIN: &str = "mcn"; pub const P_POLL: &str = "pl"; pub const P_CHAT: &str = "ch";
// =====================[ CANVAS EVENTS ]=====================
pub const CW_DOWN: &str = "d"; pub const CW_MOVE: &str = "m"; pub const CW_UP: &str = "u"; pub const CW_POINTER: &str = "p"; pub const CW_ERASE_ALL: &str = "ea"; pub const CW_ZOOM: &str = "zm"; pub const CW_PAN: &str = "pn"; pub const CW_DELETE: &str = "dlos"; pub const CW_UNDO: &str = "un"; pub const CW_CREATE_IMAGE: &str = "cio"; pub const CW_TRANSFORM: &str = "tf"; pub const CW_PASTE: &str = "pdo2";
// =====================[ DOC EVENTS ]=====================
pub const DC_ADD_SLIDE: &str = "as"; pub const DC_SHOW_SLIDE: &str = "sc"; pub const DC_BG_COLOR: &str = "sbc"; pub const DC_COLOR: &str = "cc"; pub const DC_MODE: &str = "mc"; pub const DC_PEN_SIZE: &str = "pstc"; pub const DC_ERASER_SIZE: &str = "estc"; pub const DC_ROTATE: &str = "rs"; pub const DC_PLAY_GIF: &str = "pg"; pub const DC_SCREEN_SHARE: &str = "ss"; pub const DC_GREEN_SCREEN: &str = "gsrn";
// =====================[ MAIN EVENTS ]=====================
pub const MC_CLASS_END: &str = "cle"; pub const MC_CAMERA_SWITCH: &str = "cs"; pub const MC_EDUCATOR_ALIGN: &str = "el"; pub const MC_CONFIG: &str = "cfg";
// =====================[ EVENT STRUCT ]=====================
#[derive(Debug, Clone)] pub struct Event { pub p_time: i64, pub plugin: String, pub kind: String, pub payload: Value, pub canvas_id: Option<i64>, pub seq: i64 }
// =====================[ EVENT HELPERS ]=====================
impl Event {
    pub fn t(&self) -> f64 { self.p_time as f64 / 1_000_000.0 }
    pub fn f(&self, key: &str) -> Option<f64> { self.payload.get(key).and_then(as_f64) }
    pub fn i(&self, key: &str) -> Option<i64> { self.payload.get(key).and_then(as_i64) }
    pub fn s(&self, key: &str) -> Option<&str> { self.payload.get(key).and_then(|v| v.as_str()) }
    pub fn point(&self) -> Option<(f32, f32)> { let p = self.payload.get("p")?; let x = p.get("x").and_then(as_f64)?; let y = p.get("y").and_then(as_f64)?; Some((x as f32, y as f32)) }
}
// =====================[ NUMBER PARSERS ]=====================
pub fn as_f64(v: &Value) -> Option<f64> { match v { Value::Number(n) => n.as_f64(), Value::String(s) => s.parse::<f64>().ok(), _ => None } }
pub fn as_i64(v: &Value) -> Option<i64> { match v { Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)), Value::String(s) => s.parse::<i64>().ok().or_else(|| s.parse::<f64>().ok().map(|f| f as i64)), _ => None } }
// =====================[ EVENT PARSER ]=====================
fn parse_event(raw: &Value) -> Option<Event> {
    let body = raw.get("data")?; if !body.is_object() { return None; }
    let inner = body.get("data");
    let (payload, canvas_id) = match inner { Some(v) if v.is_object() && v.get("e").is_some() => (v, body.get("id").and_then(as_i64)), _ => (body, body.get("id").and_then(as_i64)) };
    let kind = payload.get("e")?.as_str()?.to_string();
    Some(Event { p_time: raw.get("p_time").and_then(as_i64).unwrap_or(0), plugin: raw.get("plugin").and_then(|v| v.as_str()).unwrap_or("").to_string(), kind, payload: payload.clone(), canvas_id, seq: raw.get("id").and_then(as_i64).unwrap_or(0) })
}
// =====================[ EVENT LOADER ]=====================
pub fn load_events(text: &str) -> Result<Vec<Event>> {
    let doc: Value = serde_json::from_str(text)?; let chunks = doc.as_array().ok_or_else(|| anyhow!("data.json is not an array"))?;
    let mut events = Vec::with_capacity(32_000);
    for chunk in chunks { match chunk.as_array() { Some(list) => events.extend(list.iter().filter_map(parse_event)), None => { if let Some(ev) = parse_event(chunk) { events.push(ev) } } } }
    events.sort_by_key(|e| (e.p_time, e.seq)); Ok(events)
}
// =====================[ CLASS INFO ]=====================
#[derive(Debug, Clone, Default)] pub struct ClassInfo { pub class_uid: String, pub media_uid: String, pub title: String, pub educator: String, pub duration: f64, pub webm_file: String, pub media_host: String, pub data_host: String, pub slides_pdf: Option<String> }
// =====================[ CLASS URLS ]=====================
impl ClassInfo {
    pub fn data_url(&self) -> String { format!("https://{}/lesson-raw/{}/data.json", self.data_host, self.media_uid) }
    pub fn media_url(&self) -> String { format!("https://{}/lesson-raw/{}/{}", self.media_host, self.media_uid, self.webm_file) }
}
