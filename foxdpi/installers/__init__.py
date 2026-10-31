"""FoxDPI installer katmanı — paket biçimlerine göre stratejiler."""

from __future__ import annotations

import os
from abc import ABC, abstractmethod
from typing import Any

from foxdpi.distro import detect


class InstallerBase(ABC):
    """Her paket türü bu arayüze uygun olur."""

    kind: str = "base"

    @abstractmethod
    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        raise NotImplementedError

    @abstractmethod
    def remove(self, name: str, *, verbose: bool) -> None:
        raise NotImplementedError

    @abstractmethod
    def binary_path(self, name: str) -> str | None:
        raise NotImplementedError


_REGISTRY: dict[str, InstallerBase] = {}


def register(installer: InstallerBase) -> None:
    _REGISTRY[installer.kind] = installer


def resolve(pkg_hint: str | None, source: str, pm_info: dict[str, Any]) -> InstallerBase | None:
    """pkg_hint veya source uzantısından installer seç."""
    if pkg_hint:
        inst = _REGISTRY.get(pkg_hint)
        if inst is not None:
            return inst
    if source.lower().endswith(".deb"):
        return _REGISTRY.get("deb")
    if source.lower().endswith(".rpm"):
        return _REGISTRY.get("rpm")
    if source.lower().endswith((".tar.gz", ".tar.bz2", ".tar.zst", ".zip")):
        return _REGISTRY.get("tar")
    if "appimage" in source.lower():
        return _REGISTRY.get("appimage")
    if "://" in source:
        return _REGISTRY.get("remote")
    return _REGISTRY.get("script")


def by_name(name: str) -> InstallerBase | None:
    """İsimden yönetilen bir installer bul."""
    for inst in _REGISTRY.values():
        if getattr(inst, "handles_name", None) == name or name == inst.kind:
            return inst
    return None


def installed() -> list[dict[str, Any]]:
    return _list_installed()


def _list_installed() -> list[dict[str, Any]]:
    """Mevcut yönetilen paketler."""
    manifest_path = os.path.expanduser("~/.local/state/foxdpi/manifest.json")
    try:
        import json
        with open(manifest_path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
        return data if isinstance(data, list) else []
    except FileNotFoundError:
        return []


def binary_path(name: str) -> str | None:
    """Bir isme karşılık gelen ikili PATH içinden bulun."""
    import shutil
    if inst := by_name(name):
        p = inst.binary_path(name)
        if p and shutil.which(p):
            return p
    return shutil.which(name)


def _manifest_write(entries: list[dict[str, Any]]) -> None:
    import json
    path = os.path.expanduser("~/.local/state/foxdpi/manifest.json")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(entries, fh, indent=2)
