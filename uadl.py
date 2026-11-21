# =====================[ IMPORTS ]=====================
import ctypes; import os; import sys; from pathlib import Path
# =====================[ CONFIG ]=====================
LIB_CANDIDATES = [Path(__file__).resolve().parent / "libuadl.so", Path(__file__).resolve().parent / "target" / "release" / "libuadl.so", Path("/usr/local/lib/libuadl.so"), Path("/usr/lib/libuadl.so")]
# =====================[ FIND LIB ]=====================
def find_lib():
    for p in LIB_CANDIDATES:
        if p.is_file(): return p
    env = os.environ.get("UADL_SO")
    if env and Path(env).is_file(): return Path(env)
    return None
# =====================[ LOAD LIB ]=====================
def load_lib():
    path = find_lib()
    if not path: print("[ERROR] libuadl.so not found. Build it with: cargo build --release"); sys.exit(1)
    lib = ctypes.CDLL(str(path)); lib.uadl_download_media.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]; lib.uadl_download_media.restype = ctypes.c_int; lib.uadl_last_error.restype = ctypes.c_char_p; lib.uadl_version.restype = ctypes.c_char_p
    print(f"[INFO] Loaded {path} ({lib.uadl_version().decode()})"); return lib
# =====================[ MAIN ]=====================
def main():
    if len(sys.argv) < 4: print("Usage: python uadl.py <webm_url> <filename> <out_dir>"); print("  e.g. python uadl.py 'https://uamedia.uacdn.net/lesson-raw/J9E64LQ1DQIZGOUIJ76D/output.webm' 'Lecture 1' ./downloads"); sys.exit(1)
    url = sys.argv[1]; filename = sys.argv[2]; out = sys.argv[3]
    lib = load_lib(); print(f"[INFO] Downloading {url} -> {out}/{filename}.mp4"); rc = lib.uadl_download_media(url.encode(), filename.encode(), out.encode())
    if rc != 0:
        err = lib.uadl_last_error(); msg = err.decode(errors="replace") if err else "unknown error"; print(f"[ERROR] {msg}"); sys.exit(1)
    print("[INFO] Done.")
# =====================[ RUN ]=====================
if __name__ == "__main__": main()
