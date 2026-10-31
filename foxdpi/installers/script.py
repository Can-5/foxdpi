"""Script installer — shell-installer.tarzı (curl-sh style) otomasyonu."""

from __future__ import annotations

import os
import subprocess
import tempfile
from typing import Any

import click

from foxdpi.distro import detect
from foxdpi.installers import InstallerBase, _manifest_write, _list_installed, register


class ScriptInstaller(InstallerBase):
    kind = "script"

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        name = name or "script-install"
        if source.startswith(("http://", "https://")):
            d = tempfile.mkdtemp(prefix="foxdpi-")
            path = os.path.join(d, "install.sh")
            subprocess.run(["curl", "-fsSL", "-o", path, source], check=True)
            if checksum:
                self._verify_checksum(path, checksum)
            return self._run_script(path, name, verbose)
        else:
            if not os.path.isfile(source):
                raise FileNotFoundError(f"Script dosyası bulunamadı: {source}")
            if checksum:
                self._verify_checksum(source, checksum)
            return self._run_script(source, name, verbose)

    def remove(self, name: str, *, verbose: bool) -> None:
        current = self._current()
        _manifest_write([e for e in current if e.get("name") != name])
        if verbose:
            click.echo(f"[script] kaydı kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        return None

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "script"]

    def _run_script(self, path: str, name: str, verbose: bool) -> dict[str, Any]:
        info = detect()
        prefix = info.get("default_prefix")
        env: dict[str, str] = {}
        if prefix == "user":
            env["FOXDPI_PREFIX"] = os.path.expanduser("~/.local")
        proc = subprocess.run(
            ["sh", path],
            env={**os.environ, **env},
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            click.echo(proc.stderr, err=True)
            raise RuntimeError(f"Script installer hatası (exit {proc.returncode})")
        if verbose:
            click.echo(proc.stdout)
        return {"name": name, "kind": "script", "source": path}

    def _verify_checksum(self, path: str, expected: str) -> None:
        import hashlib
        h = hashlib.sha256()
        with open(path, "rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                h.update(chunk)
        if h.hexdigest() != expected.strip().lower():
            raise ValueError("script checksum uyuşmazlığı")


register(ScriptInstaller())
