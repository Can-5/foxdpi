"""Archive (tar.gz / tar.bz2 / tar.zst / zip) installer."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from typing import Any

from foxdpi.distro import detect
from foxdpi.installers import InstallerBase, _manifest_write, installed as _list_installed


ARCHIVE_EXTRACTORS = {
    ".tar.gz": ["tar", "xf"],
    ".tar.bz2": ["tar", "xf"],
    ".tar.zst": ["tar", "--zstd", "xf"],
    ".zip": ["unzip", "-o"],
}


class TarInstaller(InstallerBase):
    kind = "tar"

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        name = name or self._derive_name(source)
        dest = os.path.expanduser(f"~/.local/share/foxdpi/packages/{name}")
        os.makedirs(dest, exist_ok=True)

        # checksum doğrulaması (opsiyonel)
        if checksum:
            self._verify_checksum(source, checksum)

        if source.lower().endswith(".zip"):
            subprocess.run(["unzip", "-o", source, "-d", dest], check=True)
        else:
            ext = self._archive_ext(source)
            cmd = ARCHIVE_EXTRACTORS.get(ext, ["tar", "xf"])
            subprocess.run([*cmd, source, "-C", dest], check=True)

        _manifest_write([*self._current(), {"name": name, "source": source, "kind": "tar",
                                              "path": dest}])
        if verbose:
            print(f"[tar] çıkarıldı → {dest}")
        return {"name": name, "kind": "tar", "source": source, "path": dest}

    def remove(self, name: str, *, verbose: bool) -> None:
        current = self._current()
        entry = next((e for e in current if e.get("name") == name), None)
        if entry and entry.get("path"):
            shutil.rmtree(entry["path"], ignore_errors=True)
        _manifest_write([e for e in current if e.get("name") != name])
        if verbose:
            print(f"[tar] kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        for e in _list_installed():
            if e.get("name") == name and e.get("path"):
                for root, _, files in os.walk(e["path"]):
                    for f in files:
                        p = os.path.join(root, f)
                        if os.access(p, os.X_OK):
                            return p
        return None

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "tar"]

    def _derive_name(self, source: str) -> str:
        base = os.path.basename(source)
        for ext in (".tar.gz", ".tar.bz2", ".tar.zst", ".zip"):
            if base.lower().endswith(ext):
                return base[: -len(ext)]
        return os.path.splitext(base)[0]

    def _archive_ext(self, source: str) -> str:
        low = source.lower()
        for ext in (".tar.zst", ".tar.bz2", ".tar.gz", ".zip"):
            if low.endswith(ext):
                return ext
        return ".tar.gz"

    def _verify_checksum(self, path: str, expected: str) -> None:
        import hashlib
        h = hashlib.sha256()
        with open(path, "rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                h.update(chunk)
        if h.hexdigest() != expected.strip().lower():
            raise ValueError("checksum uyuşmazlığı")


from foxdpi.installers import register as _register
_register(TarInstaller())
