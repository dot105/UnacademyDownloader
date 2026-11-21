// =====================[ MODULES ]=====================
pub mod api;
pub mod assets;
pub mod bezier;
pub mod cam;
pub mod download;
pub mod export;
pub mod model;
pub mod render;
pub mod state;
pub mod yuv;
// =====================[ IMPORTS ]=====================
use std::ffi::{c_char, c_int, CStr, CString};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use anyhow::{bail, Result};
use clap::Parser;
use once_cell::sync::Lazy;
use assets::SlideCache;
// =====================[ ARGS ]=====================
#[derive(Parser, Debug, Clone)]
#[command(name = "uadl", about = "Download an Unacademy class as a single MP4 with the whiteboard rendered in", after_help = "example:\n  uadl https://unacademy.com/class/slug/ABCD1234 eyJhbGci...\n  uadl ABCD1234 \"$UNACADEMY_TOKEN\" -o ~/classes --fast")]
pub struct Args {
    pub url: String,
    pub token: Option<String>,
    #[arg(short, long, default_value = ".")]
    pub out: PathBuf,
    #[arg(short = 'c', long, default_value_t = 16)]
    pub connections: usize,
    #[arg(short = 'j', long, default_value_t = 0)]
    pub jobs: usize,
    #[arg(long, default_value_t = 12.0)]
    pub fps: f64,
    #[arg(long, default_value_t = 1280)]
    pub width: u32,
    #[arg(long, default_value_t = 28)]
    pub crf: u32,
    #[arg(long, default_value = "superfast")]
    pub preset: String,
    #[arg(long, conflicts_with = "best")]
    pub fast: bool,
    #[arg(long)]
    pub best: bool,
    #[arg(long)]
    pub no_pip: bool,
    #[arg(long)]
    pub no_zoom: bool,
    #[arg(long)]
    pub keep: bool,
    #[arg(long, default_value_t = 0.0)]
    pub start: f64,
    #[arg(long)]
    pub end: Option<f64>,
    #[arg(short, long)]
    pub quiet: bool,
}
// =====================[ REPORTER ]=====================
struct Reporter {
    tty: bool,
}
// =====================[ REPORTER IMPL ]=====================
impl Reporter {
    fn new(quiet: bool) -> Self {
        Reporter { tty: !quiet && std::io::stderr().is_terminal() }
    }
    fn say(&self, msg: &str) {
        eprintln!("{msg}");
    }
    fn line(&self, msg: &str) {
        if self.tty {
            eprint!("\r{msg}    ");
            let _ = std::io::stderr().flush();
        }
    }
    fn done(&self, msg: &str) {
        if self.tty {
            eprint!("\r{msg}    \n");
            let _ = std::io::stderr().flush();
        } else {
            eprintln!("{msg}");
        }
    }
}
// =====================[ TRIM NUM ]=====================
fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else { format!("{v}") }
}
// =====================[ SAFE NAME ]=====================
fn safe_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    let s = if cleaned.is_empty() { fallback.to_string() } else { cleaned };
    s.chars().take(150).collect()
}
// =====================[ ARGS FOR SO ]=====================
pub fn args_for_so(url: String, token: String, out: PathBuf) -> Args {
    Args { url, token: Some(token), out, connections: 16, jobs: 0, fps: 12.0, width: 1280, crf: 28, preset: "superfast".into(), fast: false, best: false, no_pip: false, no_zoom: false, keep: false, start: 0.0, end: None, quiet: true }
}
// =====================[ RUN WITH ARGS ]=====================
pub fn run_with_args(mut args: Args) -> Result<()> {
    if args.fast {
        args.crf = 30;
        if args.width == 1280 { args.width = 960; }
        if args.fps == 12.0 { args.fps = 8.0; }
    } else if args.best {
        args.preset = "veryfast".into();
        args.crf = 23;
    }
    let token = args.token.clone().or_else(|| std::env::var("UNACADEMY_TOKEN").ok()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let Some(token) = token else {
        bail!("no token: pass it as the second argument or set UNACADEMY_TOKEN.\nIt is the 'accessToken' cookie on unacademy.com.")
    };
    let rep = Reporter::new(args.quiet);
    export::require_ffmpeg()?;
    std::fs::create_dir_all(&args.out)?;
    let uid = api::parse_class_uid(&args.url)?;
    rep.say(&format!("Resolving {uid} ..."));
    let client = api::Client::new(&token);
    let info = client.class_info(&args.url)?;
    let stem = safe_name(format!("{} - {}", info.title, info.educator).trim_end_matches(" - "), &info.class_uid);
    let base = args.out.join(&stem);
    let final_mp4 = base.with_extension("mp4");
    rep.say(&format!("  {}", info.title));
    rep.say(&format!("  {}  |  {:.0} min  |  {}", info.educator, info.duration / 60.0, info.media_uid));
    let data_path = base.with_extension("events.json");
    let text = if data_path.is_file() {
        std::fs::read_to_string(&data_path)?
    } else {
        rep.say("Fetching board events ...");
        let t = client.get_text(&info.data_url())?;
        std::fs::write(&data_path, &t)?;
        t
    };
    let events = model::load_events(&text)?;
    rep.say(&format!("  {} events", events.len()));
    let webm = base.with_extension("webm");
    let media_url = info.media_url();
    let have = std::fs::metadata(&webm).map(|m| m.len()).unwrap_or(0);
    let remote = download::probe(&client.agent, &media_url).map(|(size, _)| size).unwrap_or(0);
    if have > 0 && have == remote {
        rep.say(&format!("Media already downloaded ({:.0} MB)", have as f64 / 1e6));
    } else {
        rep.say(&format!("Downloading media on {} connections ...", args.connections));
        let bytes = download::download(client.agent.clone(), &media_url, &webm, args.connections, |p| rep.line(&format!("  {}", download::bar(p))))?;
        rep.done(&format!("  downloaded {:.0} MB", bytes as f64 / 1e6));
    }
    let jobs = if args.jobs > 0 { args.jobs } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) };
    let height = ((args.width as f64 * 9.0 / 16.0).round() as u32) & !1;
    let opts = export::Options { fps: args.fps, width: args.width, height, crf: args.crf, preset: args.preset.clone(), pip: !args.no_pip, zoom: !args.no_zoom, start: args.start, end: args.end, ..Default::default() };
    rep.say(&format!("Rendering board at {}x{} @ {}fps on {jobs} threads ...", args.width, height, trim_num(args.fps)));
    let slides = SlideCache::new(args.width);
    let started = Instant::now();
    export::export(&events, &final_mp4, Some(&webm), &opts, info.duration, jobs, &slides, |done, total| {
        let secs = started.elapsed().as_secs_f64().max(1e-6);
        let rate = done as f64 / secs;
        let eta = if rate > 0.0 { ((total - done) as f64 / rate) as u64 } else { 0 };
        rep.line(&format!("  frame {done}/{total} ({:5.1}%)  {rate:6.1} fps  ETA {}m{:02}s", 100.0 * done as f64 / total as f64, eta / 60, eta % 60));
    })?;
    rep.done(&format!("  rendered in {:.0}s", started.elapsed().as_secs_f64()));
    if !args.keep {
        let _ = std::fs::remove_file(&webm);
        let _ = std::fs::remove_file(&data_path);
    }
    let size = std::fs::metadata(&final_mp4).map(|m| m.len()).unwrap_or(0);
    rep.say(&format!("\n-> {}  ({:.0} MB)", final_mp4.display(), size as f64 / 1e6));
    Ok(())
}
// =====================[ LAST ERROR ]=====================
static LAST_ERROR: Lazy<Mutex<Option<CString>>> = Lazy::new(|| Mutex::new(None));
// =====================[ SET ERROR ]=====================
fn set_error(msg: String) {
    *LAST_ERROR.lock().unwrap() = CString::new(msg).ok();
}
// =====================[ DOWNLOAD ]=====================
#[no_mangle]
pub extern "C" fn uadl_download(url: *const c_char, token: *const c_char, out_dir: *const c_char) -> c_int {
    if url.is_null() || token.is_null() || out_dir.is_null() {
        set_error("null pointer passed to uadl_download".into());
        return -1;
    }
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().to_string();
    let token = unsafe { CStr::from_ptr(token) }.to_string_lossy().to_string();
    let out_dir = unsafe { CStr::from_ptr(out_dir) }.to_string_lossy().to_string();
    let args = args_for_so(url, token, PathBuf::from(out_dir));
    match run_with_args(args) {
        Ok(()) => 0,
        Err(e) => {
            set_error(format!("{e:#}"));
            -1
        }
    }
}
// =====================[ LAST ERROR ]=====================
#[no_mangle]
pub extern "C" fn uadl_last_error() -> *const c_char {
    match LAST_ERROR.lock().unwrap().as_ref() {
        Some(s) => s.as_ptr(),
        None => std::ptr::null(),
    }
}
// =====================[ VERSION ]=====================
#[no_mangle]
pub extern "C" fn uadl_version() -> *const c_char {
    static V: &[u8] = b"uadl\0";
    V.as_ptr() as *const c_char
}
