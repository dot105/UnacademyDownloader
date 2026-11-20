// =====================[ IMPORTS ]=====================
use std::ffi::{c_char, c_int, CStr, CString}; use std::path::PathBuf; use std::sync::Mutex; use once_cell::sync::Lazy;
// =====================[ LAST ERROR ]=====================
static LAST_ERROR: Lazy<Mutex<Option<CString>>> = Lazy::new(|| Mutex::new(None));
// =====================[ SET ERROR ]=====================
fn set_error(msg: String) { *LAST_ERROR.lock().unwrap() = CString::new(msg).ok() }
// =====================[ DOWNLOAD ]=====================
#[no_mangle]
pub extern "C" fn uadl_download(url: *const c_char, token: *const c_char, out_dir: *const c_char) -> c_int {
    if url.is_null() || token.is_null() || out_dir.is_null() { set_error("null pointer passed to uadl_download".into()); return -1 }
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().to_string();
    let token = unsafe { CStr::from_ptr(token) }.to_string_lossy().to_string();
    let out_dir = unsafe { CStr::from_ptr(out_dir) }.to_string_lossy().to_string();
    let args = crate::args_for_so(url, token, PathBuf::from(out_dir));
    match crate::run_with_args(args) { Ok(()) => 0, Err(e) => { set_error(format!("{e:#}")); -1 } }
}
// =====================[ LAST ERROR ]=====================
#[no_mangle]
pub extern "C" fn uadl_last_error() -> *const c_char { match LAST_ERROR.lock().unwrap().as_ref() { Some(s) => s.as_ptr(), None => std::ptr::null() } }
// =====================[ VERSION ]=====================
#[no_mangle]
pub extern "C" fn uadl_version() -> *const c_char { static V: &[u8] = b"uadl\0"; V.as_ptr() as *const c_char }
