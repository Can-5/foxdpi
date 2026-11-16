"""Remote (URL) installer — indir, doğrula, hedef installer'a devir."""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tempfile
from typing import Any
from urllib.parse import urlparse

import click

from foxdpi.distro import detect
from foxdpi.installers import InstallerBase, _manifest_write, installed as _list_installed, resolve as _resolve


class RemoteInstaller(InstallerBase):
    kind = "remote"

    def __init__(self) -> None:
        self._cache_dir = os.path.expanduser("~/.cache/foxdpi/remote")

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        url = source
        parsed = urlparse(url)
        if not parsed.scheme or not parsed.netloc:
            raise ValueError(f"Geçerli bir URL gerekli: {url}")

        name = name or self._derive_name(url)
        os.makedirs(self._cache_dir, exist_ok=True)
        dest = os.path.join(self._cache_dir, os.path.basename(urlparse(url).path) or "remote-pkg")

        # indir
        subprocess.run(["curl", "-fsSL", "-o", dest, url], check=True)

        # checksum doğrula
        if checksum_url:
            checksum = self._fetch_checksum(checksum_url)
        if checksum:
            self._verify_checksum(dest, checksum)

        # hedef installer'a devir
        info = detect()
        installer = _resolve(None, dest, info)
        if installer is None:
            raise RuntimeError("İndirilen paket türü desteklenmiyor (devir edilecek installer bulunamadı)")
        result = installer.install(dest, name=name, checksum=None, checksum_url=None,
                                   verify_gpg=False, verbose=verbose)
        _manifest_write([*self._current(), {"name": result.get("name", name),
                                             "source": url, "kind": "remote"}])
        if verbose:
            click.echo(f"[remote] indirildi ve kuruldu: {url}")
        return result

    def remove(self, name: str, *, verbose: bool) -> None:
        current = self._current()
        next_entries = [e for e in current if e.get("name") != name]
        _manifest_write(next_entries)
        if verbose:
            click.echo(f"[remote] kaydı kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        return None

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "remote"]

    def _derive_name(self, url: str) -> str:
        base = os.path.basename(urlparse(url).path)
        name, _ = os.path.splitext(base)
        return name or "remote-pkg"

    def _fetch_checksum(self, checksum_url: str) -> str | None:
        proc = subprocess.run(["curl", "-fsSL", checksum_url], capture_output=True, text=True)
        if proc.returncode != 0:
            return None
        return proc.stdout.strip()

    def _verify_checksum(self, path: str, expected: str) -> None:
        h = hashlib.sha256()
        with open(path, "rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                h.update(chunk)
        if h.hexdigest() != expected.strip().lower():
            raise ValueError("remote checksum uyuşmazlığı")


from foxdpi.installers import register as _register
_register(RemoteInstaller())
