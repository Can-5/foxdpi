# foxdpi-bypass — Rust motoru icin ince Python baslaticisi
# Gercek DPI bypass motoru repo kokundeki `bypass/` altindaki Rust crate'idir
# (yerel SOCKS5 proxy + TLS ClientHello record-split / segment-desync / host-scramble).
# Bu paket sadece Rust binary'sini bulup argumanlariyla calistirir.

__version__ = "0.3.0"


def main(argv=None):
    import os
    import shutil
    import subprocess
    import sys

    if argv is None:
        argv = sys.argv[1:]

    here = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.environ.get("FOXDPI_RUST_BIN"),
        # repo duzeni: <root>/bypass/target/release/foxdpi-bypass
        os.path.join(here, "..", "..", "bypass", "target", "release", "foxdpi-bypass"),
        os.path.join(here, "..", "..", "..", "bypass", "target", "release", "foxdpi-bypass"),
        shutil.which("foxdpi-bypass-bin"),
    ]
    for cand in candidates:
        if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
            return subprocess.call([cand, *argv])

    print("[foxdpi-bypass] Rust motoru bulunamadi.", file=sys.stderr)
    print("Derleyin: (cd bypass && cargo build --release)", file=sys.stderr)
    print("veya FOXDPI_RUST_BIN=/yol/foxdpi-bypass ayarlayin.", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
