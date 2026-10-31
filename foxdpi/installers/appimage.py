"""AppImage installer — ikiliyi çalıştırılabilir yapar, PATH'e ekler."""

from __future__ import annotations

import os
import subprocess
from typing import Any

from foxdpi.distro import detect
from foxdpi.installers import InstallerBase, _manifest_write, installed as _list_installed


class AppImageInstaller(InstallerBase):
    kind = "appimage"

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        if not os.path.isfile(source):
            raise FileNotFoundError(f"AppImage dosyası bulunamadı: {source}")
        name = name or self._derive_name(source)
        app_dir = os.path.expanduser(f"~/.local/share/foxdpi/appimages/{name}")
        os.makedirs(app_dir, exist_ok=True)
        dest = os.path.join(app_dir, os.path.basename(source))
        shutil_copy = __import__("shutil").copy2(source, dest)
        subprocess.run(["chmod", "+x", dest], check=True)
        bin_dir = os.path.expanduser("~/.local/bin")
        os.makedirs(bin_dir, exist_ok=True)
        link = os.path.join(bin_dir, name)
        if not os.path.exists(link):
            os.symlink(dest, link)
        _manifest_write([*self._current(), {"name": name, "source": source, "kind": "appimage",
                                              "exec": dest}])
        if verbose:
            print(f"[appimage] hazır: {link}")
        return {"name": name, "kind": "appimage", "source": source, "exec": dest}

    def remove(self, name: str, *, verbose: bool) -> None:
        current = self._current()
        entry = next((e for e in current if e.get("name") == name), None)
        if entry:
            exec_ = entry.get("exec")
            if exec_ and os.path.exists(exec_):
                os.unlink(exec_)
                os.rmdir(os.path.dirname(exec_), ignore_errors=True)
            bin_link = os.path.join(os.path.expanduser("~/.local/bin"), name)
            if os.path.islink(bin_link):
                os.unlink(bin_link)
        _manifest_write([e for e in current if e.get("name") != name])
        if verbose:
            print(f"[appimage] kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        link = os.path.join(os.path.expanduser("~/.local/bin"), name)
        return link if os.path.exists(link) else None

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "appimage"]

    def _derive_name(self, source: str) -> str:
        base = os.path.basename(source)
        name, _ = os.path.splitext(base)
        return name


from foxdpi.installers import register as _register
_register(AppImageInstaller())
