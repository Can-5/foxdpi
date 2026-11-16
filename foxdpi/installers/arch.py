"""Arch / AUR yaklaşımı installer — varsa yay/paru/makepkg kullanır."""

from __future__ import annotations

import os
import subprocess
import tempfile
from typing import Any

import click

from foxdpi.distro import detect
from foxdpi.installers import InstallerBase, _manifest_write, installed as _list_installed


class ArchInstaller(InstallerBase):
    kind = "arch"

    def __init__(self) -> None:
        info = detect()
        self._pm = info.get("pm_primary")
        self._aur_helper = self._detect_aur_helper()

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        name = name or os.path.basename(source)
        if self._aur_helper:
            self._install_with_aur_helper(source, name, verbose)
        else:
            self._install_with_makepkg(source, name, verbose)
        _manifest_write([*self._current(), {"name": name, "source": source, "kind": "arch"}])
        if verbose:
            click.echo(f"[arch] kuruldu: {name}")
        return {"name": name, "kind": "arch", "source": source}

    def remove(self, name: str, *, verbose: bool) -> None:
        if self._aur_helper:
            subprocess.run([self._aur_helper, "-R", name], check=False)
        else:
            subprocess.run(["sudo", "pacman", "-Rns", name], check=False)
        current = self._current()
        _manifest_write([e for e in current if e.get("name") != name])
        if verbose:
            click.echo(f"[arch] kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        import shutil
        return shutil.which(name)

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "arch"]

    def _detect_aur_helper(self) -> str | None:
        for helper in ("yay", "paru"):
            if subprocess.run(["which", helper], capture_output=True).returncode == 0:
                return helper
        return None

    def _install_with_aur_helper(self, source: str, name: str, verbose: bool) -> None:
        if os.path.isdir(source) or (os.path.isfile(source) and source.endswith(".pkg.tar.zst")):
            subprocess.run([self._aur_helper, "-U", source], check=False)
        else:
            subprocess.run([self._aur_helper, "-S", source], check=False)

    def _install_with_makepkg(self, source: str, name: str, verbose: bool) -> None:
        click.echo("[arch] uyarı: AUR yardımcı programı bulunamadı; source içeren klasör bekleniyor (makepkg).")
        if not os.path.isdir(source):
            raise FileNotFoundError(f"PKGBUILD içeren dizin bekleniyor: {source}")
        with tempfile.TemporaryDirectory() as tmp:
            subprocess.run(["git", "clone", source, tmp], check=True, capture_output=True)
            subprocess.run(["makepkg", "-si", "--noconfirm"], cwd=tmp, check=False)


from foxdpi.installers import register as _register
_register(ArchInstaller())
