# =====================[ IMPORTS ]=====================
import asyncio; import ctypes; import os; import sys; from pathlib import Path
# =====================[ CONFIG ]=====================
LIB_CANDIDATES = [Path(__file__).resolve().parent / "libuadl.so", Path(__file__).resolve().parent / "target" / "release" / "libuadl.so", Path("/usr/local/lib/libuadl.so"), Path("/usr/lib/libuadl.so")]
DEFAULT_FPS = 12.0
# =====================[ FIND LIB ]=====================
def find_lib():
    for p in LIB_CANDIDATES:
        if p.is_file(): return p
    env = os.environ.get("UADL_SO")
    if env and Path(env).is_file(): return Path(env)
    return None
# =====================[ LOAD LIB ]=====================
_lib_cache = None
def load_lib():
    global _lib_cache
    if _lib_cache is not None: return _lib_cache
    path = find_lib()
    if not path: raise RuntimeError("libuadl.so not found. Build it with: cargo build --release")
    lib = ctypes.CDLL(str(path))
    lib.uadl_download_media.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_double]
    lib.uadl_download_media.restype = ctypes.c_int
    lib.uadl_last_error.restype = ctypes.c_char_p
    lib.uadl_version.restype = ctypes.c_char_p
    _lib_cache = lib
    return lib
# =====================[ DOWNLOAD ]=====================
async def download_media(webm_url: str, filename: str, out_dir: str, fps: float = DEFAULT_FPS) -> str:
    lib = await asyncio.to_thread(load_lib)
    def _call():
        rc = lib.uadl_download_media(webm_url.encode(), filename.encode(), out_dir.encode(), fps)
        if rc != 0:
            err = lib.uadl_last_error()
            msg = err.decode(errors="replace") if err else "unknown error"
            raise RuntimeError(msg)
        return str(Path(out_dir) / f"{filename}.mp4")
    return await asyncio.to_thread(_call)
# =====================[ MAIN ]=====================
async def main():
    if len(sys.argv) < 4:
        print("Usage: python uadl.py <webm_url> <filename> <out_dir> [fps]")
        print("  e.g. python uadl.py 'https://uamedia.uacdn.net/lesson-raw/J9E64LQ1DQIZGOUIJ76D/output.webm' 'Lecture 1' ./downloads 24")
        sys.exit(1)
    url = sys.argv[1]; filename = sys.argv[2]; out = sys.argv[3]; fps = float(sys.argv[4]) if len(sys.argv) > 4 else DEFAULT_FPS
    print(f"[INFO] Downloading {url} -> {out}/{filename}.mp4 @ {fps}fps")
    try:
        result = await download_media(url, filename, out, fps)
        print(f"[INFO] Done -> {result}")
    except Exception as e:
        print(f"[ERROR] {e}"); sys.exit(1)
# =====================[ RUN ]=====================
if __name__ == "__main__": asyncio.run(main())
