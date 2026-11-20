// =====================[ IMPORTS ]=====================
use std::path::PathBuf; use std::process::Command; use anyhow::{bail, Context, Result}; use crate::export::Options;
// =====================[ CAM CONSTANTS ]=====================
pub const CAM_SCALE: f64 = 0.22; pub const CAM_BORDER: u32 = 2; pub const CAM_MARGIN: u32 = 18; pub const CAM_CORNER: &str = "br"; pub const CHROMA_KEY: bool = true;
// =====================[ CAM POSITION ]=====================
pub fn cam_position(width: u32, height: u32, scale: f64) -> (u32, u32, u32, u32) {
    let cam_w = ((width as f64 * scale) as u32 / 2 * 2).max(2);
    let cam_h = ((cam_w as f64 * 9.0 / 16.0) as u32 / 2 * 2).max(2);
    let margin = ((width as f64 / 71.0).round() as u32).max(2);
    let x = width.saturating_sub(cam_w + margin);
    let y = height.saturating_sub(cam_h + margin);
    (x, y, cam_w, cam_h)
}
// =====================[ OVERLAY FILTER ]=====================
pub fn overlay_filter(o: &Options) -> String {
    let width = ((o.width as f64 * o.pip_scale) as u32 / 2 * 2).max(2);
    let border = ((o.width as f64 / 640.0).round() as u32).max(1);
    let margin = ((o.width as f64 / 71.0).round() as u32).max(2);
    let x = if matches!(o.pip_corner.as_str(), "tr" | "br") { format!("W-w-{margin}") } else { margin.to_string() };
    let y = if matches!(o.pip_corner.as_str(), "bl" | "br") { format!("H-h-{margin}") } else { margin.to_string() };
    format!("[1:v]setpts=PTS-STARTPTS,scale={width}:-2,pad=iw+{}:ih+{}:{border}:{border}:white[pip];[0:v][pip]overlay={x}:{y}:shortest=0:eof_action=pass:repeatlast=1[v]", border * 2, border * 2)
}
// =====================[ CHROMA FILTER ]=====================
pub fn chroma_filter() -> String {
    "[1:v]colorkey=0x00FF00:0.3:0.2,format=yuva420p[ck];[0:v][ck]overlay=W-w-18:H-h-18:shortest=0:eof_action=pass:repeatlast=1[v]".to_string()
}
// =====================[ FFMPEG OVERLAY ARGS ]=====================
pub fn overlay_args(o: &Options, media: Option<&PathBuf>) -> Vec<String> {
    let mut args = Vec::new();
    if media.is_none() || !o.pip { return args }
    let filter = if CHROMA_KEY { chroma_filter() } else { overlay_filter(o) };
    args.push("-filter_complex".to_string()); args.push(filter); args.push("-map".to_string()); args.push("[v]".to_string()); args
}
// =====================[ CAM VERIFY ]=====================
pub fn verify_cam(media: &PathBuf) -> Result<()> {
    if !media.is_file() { bail!("cam media not found: {}", media.display()) }
    let out = Command::new("ffprobe").args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0"]).arg(media).output().context("running ffprobe")?;
    if !out.status.success() { bail!("ffprobe failed on {}", media.display()) }
    Ok(())
}
