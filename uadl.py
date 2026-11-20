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
    lib = ctypes.CDLL(str(path)); lib.uadl_download.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]; lib.uadl_download.restype = ctypes.c_int; lib.uadl_last_error.restype = ctypes.c_char_p; lib.uadl_version.restype = ctypes.c_char_p
    print(f"[INFO] Loaded {path} ({lib.uadl_version().decode()})"); return lib
# =====================[ MAIN ]=====================
def main():
    if len(sys.argv) < 2: print("Usage: python uadl.py <class_url_or_uid> [token] [out_dir]"); print("   or: UNACADEMY_TOKEN=... python uadl.py <url> [out_dir]"); sys.exit(1)
    url = sys.argv[1]; token = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith("-") else os.environ.get("UNACADEMY_TOKEN", ""); out = sys.argv[3] if len(sys.argv) > 3 else "."
    if not token: print("[ERROR] No token. Pass it as arg or set UNACADEMY_TOKEN."); sys.exit(1)
    lib = load_lib(); print(f"[INFO] Downloading {url} -> {out}"); rc = lib.uadl_download(url.encode(), token.encode(), out.encode())
    if rc != 0:
        err = lib.uadl_last_error(); msg = err.decode(errors="replace") if err else "unknown error"; print(f"[ERROR] {msg}"); sys.exit(1)
    print("[INFO] Done.")
# =====================[ RUN ]=====================
if __name__ == "__main__": main()
