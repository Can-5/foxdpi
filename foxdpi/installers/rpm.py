"""RPM installer stratejisi (dnf/yum/rpm)."""

from __future__ import annotations

import os
import subprocess
from typing import Any

from foxdpi.distro import detect, _system_pm_invoke
from foxdpi.installers import InstallerBase, _manifest_write, installed as _list_installed


class RpmInstaller(InstallerBase):
    kind = "rpm"

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        if not os.path.isfile(source):
            raise FileNotFoundError(f"RPM dosyası bulunamadı: {source}")
        name = name or os.path.splitext(os.path.basename(source))[0]
        info = detect()
        pm = info.get("pm_primary")
        if pm in ("dnf", "yum"):
            subprocess.run(["sudo", pm, "install", "-y", source], check=False)
        else:
            subprocess.run(["sudo", "rpm", "-i", source], check=False)
        _manifest_write([*self._current(), {"name": name, "source": source, "kind": "rpm"}])
        if verbose:
            print(f"[rpm] kuruldu: {name}")
        return {"name": name, "kind": "rpm", "source": source}

    def remove(self, name: str, *, verbose: bool) -> None:
        subprocess.run(["sudo", "rpm", "-e", name], check=False)
        current = self._current()
        _manifest_write([e for e in current if e.get("name") != name])
        if verbose:
            print(f"[rpm] kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        import shutil
        return shutil.which(name)

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "rpm"]


from foxdpi.installers import register as _register
_register(RpmInstaller())
