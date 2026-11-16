"""tar.gz installer stratejisi — arşivi çözer, exec'leri PATH'ye ekler."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from typing import Any

from foxdpi.distro import detect
from foxdpi.installers import InstallerBase, _manifest_write, installed as _list_installed, register


class TarInstaller(InstallerBase):
    kind = "tar"

    def install(self, source: str, *, name: str | None, checksum: str | None,
                checksum_url: str | None, verify_gpg: bool, verbose: bool) -> dict[str, Any]:
        if not os.path.isfile(source):
            raise FileNotFoundError(f"tar.gz dosyası bulunamadı: {source}")
        name = name or self._derive_name(source)
        # arşivi geçici dizine aç
        with tempfile.TemporaryDirectory(prefix="foxdpi-tar-") as tmp:
            # arşiv türünü tespit et
            cmd = ["tar", "-xf", source, "-C", tmp]
            proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
            if proc.returncode != 0:
                raise RuntimeError(f"tar faitını açma hatası: {proc.stderr or proc.stdout}")
            # içinde bir bin/ veya script/ klasörü var mı?
            extracted = self._find_executables(tmp)
            if not extracted:
                raise RuntimeError("tar archive içinde çalıştırılabilir bulunamadı")
            # ~/.local/share/foxdpi/tars/name/ dizinine kopyala
            app_dir = os.path.expanduser(f"~/.local/share/foxdpi/tars/{name}")
            os.makedirs(app_dir, exist_ok=True)
            for src_path, rel in extracted:
                dest = os.path.join(app_dir, rel)
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                shutil.copy2(src_path, dest)
                if os.access(dest, os.X_OK) or os.path.isfile(dest):
                    os.chmod(dest, 0o755)
            # PATH'e symlink ekle
            bin_dir = os.path.expanduser("~/.local/bin")
            os.makedirs(bin_dir, exist_ok=True)
            exe_names = [rel for _, rel in extracted if os.path.basename(rel) not in ("", ".", "..")]
            for exe_name in exe_names[:1]:  # ilk exec'ı PATH'e ekle
                link = os.path.join(bin_dir, name)
                if not os.path.exists(link) and exe_name:
                    os.symlink(os.path.join(app_dir, exe_name), link)
            _manifest_write([*self._current(), {
                "name": name,
                "source": source,
                "kind": "tar",
                "exec": os.path.join(bin_dir, name),
                "files": [os.path.join(app_dir, rel) for _, rel in extracted],
            }])
            if verbose:
                print(f"[tar] kuruldu ({len(extracted)} dosya): {name}")
            return {"name": name, "kind": "tar", "source": source, "exec": os.path.join(bin_dir, name)}

    def remove(self, name: str, *, verbose: bool) -> None:
        current = self._current()
        entry = next((e for e in current if e.get("name") == name and e.get("kind") == "tar"), None)
        if entry:
            # dosyaları sil
            for f in entry.get("files", []):
                if os.path.exists(f):
                    os.unlink(f)
                    os.rmdir(os.path.dirname(f), ignore_errors=True)
            # symlink'i sil
            bin_link = os.path.join(os.path.expanduser("~/.local/bin"), name)
            if os.path.islink(bin_link):
                os.unlink(bin_link)
        _manifest_write([e for e in current if not (e.get("name") == name and e.get("kind") == "tar")])
        if verbose:
            print(f"[tar] kaldırıldı: {name}")

    def binary_path(self, name: str) -> str | None:
        link = os.path.join(os.path.expanduser("~/.local/bin"), name)
        return link if os.path.exists(link) else None

    def _current(self) -> list[dict[str, Any]]:
        return [e for e in _list_installed() if e.get("kind") == "tar"]

    def _derive_name(self, source: str) -> str:
        base = os.path.basename(source)
        name, _ = os.path.splitext(base)
        if name.endswith(".tar"):
            name, _ = os.path.splitext(name)
        return name

    def _find_executables(self, root: str) -> list[tuple[str, str]]:
        """Arşiv içinde çalıştırılabilir dosyaları bul (exec biti olanlar)."""
        results = []
        for dirpath, dirnames, filenames in os.walk(root):
            for fname in filenames:
                full = os.path.join(dirpath, fname)
                if os.access(full, os.X_OK) or fname.endswith((".sh", ".py", ".rb", ".pl")):
                    rel = os.path.relpath(full, root)
                    results.append((full, rel))
        return results


register(TarInstaller())
