"""FoxDPI CLI – single entry point for all package/service operations."""

from __future__ import annotations

import importlib.metadata
import os
import sys
from typing import Any

import click
from rich.console import Console
from rich.table import Table

from foxdpi import __version__
from foxdpi.distro import detect
from foxdpi.installers import registry as installer_registry
from foxdpi.installers import installed as _installed_func


console = Console()


def _version_info() -> str:
    try:
        pkg = importlib.metadata.version("foxdpi")
    except importlib.metadata.PackageNotFoundError:
        pkg = "0.0.0 (development)"
    return pkg


def _add_common_options(func: Any) -> Any:
    func = click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")(func)
    func = click.option("--verbose", "-v", is_flag=True, help="Ayrıntılı günlük çıktısı")(func)
    return func


@click.group(context_settings={"help_option_names": ["-h", "--help"]})
@click.version_option(version=_version_info(), prog_name="foxdpi")
def main() -> None:
    """FoxDPI – tek komutla kurulsun, yönetilsin, izlenmesin."""
    pass


@main.command(name="doctor")
@_add_common_options
def doctor_cmd(quiet: bool, verbose: bool) -> None:
    """Sistem yanıtı: dağıtım, paket yöneticileri ve FoxDPI hazırlık durumu."""
    info = detect()
    table = Table(title=f"FoxDPI doctor  (foxdpi {__version__})")
    table.add_column("Özellik", style="cyan")
    table.add_column("Değer", style="green")

    table.add_row("Dağıtım ailesi", info.get("family") or "bilinmiyor")
    table.add_row("Dağıtım ID", info.get("id") or "bilinmiyor")
    table.add_row("Paket yöneticisi (öncelikli)", info.get("pm_primary") or "-")
    table.add_row("Kullanılabilecek yöneticiler", ", ".join(info.get("pm_available", [])) or "-")
    table.add_row("Root erişim", "evet" if info.get("has_sudo") else "hayır")
    table.add_row("Önerilen install yolü", info.get("default_prefix") or "-")

    console.print(table)

    if not quiet and not info.get("pm_available"):
        console.print("[yellow]Uyar⊗: Sistemde aktif paket yöneticisi bulunamadı.[/yellow]")


@main.command(name="install")
@click.argument("source", required=False)
@click.option("--name", "-n", help="Yönetim için kullanılabilecek işlem adı (varsayılan: kaynak basename)")
@click.option("--pkg", "-p", help="Paket türü zorlama: deb|rpm|tar|appimage|arch|script|remote")
@click.option("--checksum", "-c", help="SHA256 checksum (remote indirmede doğrulama için)")
@click.option("--checksum-url", help="Checksum dosyasının URL'i (remote)")
@click.option("--verify-gpg", is_flag=True, help="GPG imza doğrulamasını etkin yap (opsiyonel)")
@_add_common_options
def install_cmd(
    source: str | None,
    name: str | None,
    pkg: str | None,
    quiet: bool,
    verbose: bool,
    checksum: str | None,
    checksum_url: str | None,
    verify_gpg: bool,
) -> None:
    """Bir paket kaynağını sisteme kur.

    SOURCE olarak yerel yol, dosya yolu veya URL verilebilir.
    """
    if not source:
        click.echo(main.get_help(click.get_current_context()))
        sys.exit(0)

    pm_info = detect()
    installer = installer_registry.resolve(pkg, source, pm_info)
    if installer is None:
        console.print(f"[red]Desteklenmey⊗ paket veya kaynak: {source}[/red]")
        sys.exit(1)

    console.print(f"[cyan]FoxDPI[/cyan] kurulum stratejisi: {installer.kind}")
    try:
        result = installer.install(source, name=name, checksum=checksum, checksum_url=checksum_url,
                                  verify_gpg=verify_gpg, verbose=verbose)
        console.print(f"[green]✓[/green] {result.get('name','isim') if result else source}")
    except Exception as exc:
        console.print(f"[red]✗[/red] kurulum hatası: {exc}")
        sys.exit(1)


@main.command(name="remove")
@click.argument("name", required=True)
@_add_common_options
def remove_cmd(name: str, quiet: bool, verbose: bool) -> None:
    """Yönetilen bir paketi kaldır."""
    pm_info = detect()
    installer = installer_registry.by_name(name)
    if installer is None:
        console.print(f"[red]Yönetilen paket bulunamadı: {name}[/red]")
        sys.exit(1)
    try:
        installer.remove(name=name, verbose=verbose)
        console.print(f"[green]✓[/green] {name} kaldırıldı")
    except Exception as exc:
        console.print(f"[red]✗[/red] silme hatası: {exc}")
        sys.exit(1)


@main.command(name="list")
@_add_common_options
def list_cmd(quiet: bool, verbose: bool) -> None:
    """FoxDPI tarafından yönetilen paketleri listele."""
    entries = _installed_func()
    if not entries:
        console.print("[dim]Yönetilen paket yok.[/dim]")
        return
    table = Table(title="FoxDPI yönetilen paketler", title_style="cyan", show_lines=True)
    table.add_column("İsim", style="cyan")
    table.add_column("Kaynak", style="green")
    table.add_column(" Tür", style="yellow")
    for e in entries:
        table.add_row(e.get("name", ""), e.get("source", ""), e.get("kind", ""))
    console.print(table)


@main.command(name="version")
def version_cmd() -> None:
    """FoxDPI sürümünü göster."""
    console.print(f"foxdpi {__version__}")


@main.command(name="run")
@click.argument("binary", required=True)
@click.argument("args", nargs=-1)
@_add_common_options
def run_cmd(binary: str, args: tuple[str, ...], quiet: bool, verbose: bool) -> None:
    """Yönetilen bir ikiliyi PATH üzerinden çalıştır."""
    path = installer_registry.binary_path(binary)
    if path is None:
        console.print(f"[red]Binary bulunamadı (yönetilen): {binary}[/red]")
        sys.exit(1)
    import subprocess
    try:
        subprocess.run([path, *args], check=True)
    except subprocess.CalledProcessError as exc:
        sys.exit(exc.returncode)


@main.group(name="remote")
def remote_cmd() -> None:
    """Uzaktan kurulum pipeline'ı (curl-shroud tarzı)."""
    pass


@remote_cmd.command(name="install")
@click.argument("url", required=True)
@click.option("--checksum", "-c", help="SHA256")
@click.option("--checksum-url", help="checksum dosya URL'i")
@click.option("--name", "-n", help="Kurulacak paketin ismi")
@click.option("--verify-gpg", is_flag=True, help="GPG doğrulama")
@_add_common_options
def remote_install_cmd(
    url: str,
    checksum: str | None,
    checksum_url: str | None,
    name: str | None,
    verify_gpg: bool,
    quiet: bool,
    verbose: bool,
) -> None:
    """Remote URL'den paket kurulumu (indir → doğrula → kur)."""
    pm_info = detect()
    installer = installer_registry.resolve("remote", url, pm_info)
    if installer is None:
        console.print(f"[red]Remote kurulum desteklenmiyor veya URL geçersiz: {url}[/red]")
        sys.exit(1)
    try:
        result = installer.install(url, name=name, checksum=checksum, checksum_url=checksum_url,
                                  verify_gpg=verify_gpg, verbose=verbose)
        console.print(f"[green]✓[/green] {result.get('name', url) if result else url}")
    except Exception as exc:
        console.print(f"[red]✗[/red] remote kurulum hatası: {exc}")
        sys.exit(1)


if __name__ == "__main__":
    main()
