"""Sistem tespiti — dağıtım ailesi, paket yöneticileri, erişim ve varsayılan prefix."""

from __future__ import annotations

import os
import shutil
import subprocess
from typing import Any

try:
    import pygobject  # noqa: F401
except Exception:
    pygobject = None  # soft, sadece tespitte yardımcı olabilir


def _which(names: list[str]) -> str | None:
    for n in names:
        if shutil.which(n):
            return n
    return None


def detect() -> dict[str, Any]:
    """Mevcut sistem hakkında bilgi döndürür.

    Returns dict with keys:
      - id, name, family, version_id, pretty_name
      - pm_available: list of detected package manager command names
      - pm_primary: primary PM for this system (heuristic)
      - has_sudo: whether sudo appears usable
      - default_prefix: suggested install location (user vs system)
    """
    info: dict[str, Any] = {
        "id": None,
        "name": None,
        "family": None,
        "version_id": None,
        "pretty_name": None,
        "pm_available": [],
        "pm_primary": None,
        "has_sudo": False,
        "default_prefix": None,
    }

    # os-release
    try:
        with open("/etc/os-release", "r", encoding="utf-8") as fh:
            data: dict[str, str] = {}
            for line in fh:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" not in line:
                    continue
                key, _, val = line.partition("=")
                data[key.strip()] = val.strip().strip('"')
            info["id"] = data.get("ID")
            info["name"] = data.get("NAME")
            info["version_id"] = data.get("VERSION_ID")
            info["pretty_name"] = data.get("PRETTY_NAME")
            info["family"] = info["id"]
    except FileNotFoundError:
        pass

    # Paket yöneticileri tespiti
    candidates: list[tuple[str, list[str], str]] = [
        ("apt", ["apt-get", "apt"], "debian"),
        ("dnf", ["dnf"], "fedora/rhel"),
        ("yum", ["yum"], "rhel/centos"),
        ("pacman", ["pacman"], "arch"),
        ("zypper", ["zypper"], "suse"),
        ("apk", ["apk"], "alpine"),
        ("xbps", ["xbps-install"], "void"),
        ("eopkg", ["eopkg"], "solus"),
    ]
    for pm, cmds, family_hint in candidates:
        if _which(cmds):
            info["pm_available"].append(pm)
            if info["pm_primary"] is None:
                info["pm_primary"] = pm

    # AUR yardımcı programları (opsiyonel)
    if _which(["yay", "paru"]):
        info["pm_available"].append("aur_helper")

    # sudo var mı
    info["has_sudo"] = shutil.which("sudo") is not None or os.access("/usr/bin/sudo", os.X_OK)

    # Varsayılan prefix
    home_bin = os.path.expanduser("~/.local/bin")
    if os.access(home_bin, os.W_OK) or os.path.isdir(home_bin):
        info["default_prefix"] = "user"
    else:
        info["default_prefix"] = "system"

    if info["id"] and info["id"].lower() in ("arch", "cachyos", "artix", "manjaro", "endeavouros"):
        info["family"] = "arch"

    return info


def _run_privileged(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    """Sadece sudo gerektiren işlemler için konservatif yardımcı (test amaçlı)."""
    if shutil.which("sudo"):
        cmd = ["sudo", *cmd]
    return subprocess.run(cmd, check=False, capture_output=True, text=True)


def _system_pm_invoke(pm: str, args: list[str]) -> subprocess.CompletedProcess[str]:
    """Paket yöneticisine yönlendirilmiş çağrı (shell wrapper/consort değil)."""
    mapping = {
        "apt": ["apt-get"],
        "dnf": ["dnf"],
        "yum": ["yum"],
        "pacman": ["pacman"],
        "zypper": ["zypper"],
        "apk": ["apk"],
        "xbps": ["xbps-install"],
        "eopkg": ["eopkg"],
    }
    base = mapping.get(pm, [pm])
    return _run_privileged([*base, *args])
