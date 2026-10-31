"""DEB installer stratejisi."""

from __future__ import annotations

import os
import subprocess
from typing import Any

from foxdpi.distro import _system_pm_invoke
from foxdpi.installers import InstallerBase, _manifest_write, _list_installed, register


class DebInstaller(InstallerBase):
    kind = "deb"

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        if not os.path.isfile(source):
            raise FileNotFoundError(f"DEB dosyası bulunamadı: {source}")
        name = name or os.path.splitext(os.path.basename(source))[0]
        # dpkg ile kur, sonra apt install -f ile bağımlılıkları çöz
        subprocess.run(["sudo", "dpkg", "-i", source], check=False)
        subprocess.run(["sudo", "apt-get", "install", "-f", "-y"], check=False)
        _manifest_write([*self._current(), {"name": name, "source": source, "kind": "deb"}])
        if verbose:
            print(f"[deb] kuruldu: {name}")
        return {"name": name, "kind": "deb", "source": source}

    def remove(self, name: str, *, verbose: bool) -> None:
        subprocess.run(["sudo", "dpkg", "-r", name], check=False)
        current = self._current()
        _manifest_write([e for e in current if e.get("name") != name])
        if verbose:
            print(f"[deb] kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        import shutil
        return shutil.which(name)

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "deb"]


register(DebInstaller())
