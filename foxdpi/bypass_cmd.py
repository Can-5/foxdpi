"""FoxDPI bypass motoru CLI yönetimi — start/stop/configure/mode."""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import click
from rich.console import Console
from rich.table import Table

from foxdpi import __version__
from foxdpi.distro import detect

console = Console()


def _default_config_path() -> Path:
    """Varsayılan config yolu: ~/.foxdpi/config.yaml"""
    home = Path.home()
    return home / ".foxdpi" / "config.yaml"


def _ensure_config_exists(path: Path) -> Path:
    """Config yoksa varsayılanlı oluştur."""
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        default_content = (
            "# foxdpi-bypass varsayılan yapılandırma\n"
            "port: 10850\n"
            "mode: default\n"
            "verbose: false\n"
            "\n"
            "dns:\n"
            "  provider: built_in\n"
            "  custom_servers: []\n"
            "  encrypted: true\n"
            "  block_ads: false\n"
            "  block_pages: []\n"
            "\n"
            "rules: []\n"
        )
        path.write_text(default_content, encoding="utf-8")
        console.print(f"[cyan]Varsayılan config oluşturuldu: {path}[/cyan]")
    return path


def _run_bypass_cli(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    """foxdpi-bypass binary'sini çalıştır (olarak bulunursa)."""
    which_result = subprocess.run(["which", "foxdpi-bypass"], capture_output=True, text=True)
    if which_result.returncode == 0 and which_result.stdout.strip():
        binary = which_result.stdout.strip()
    else:
        # fallback: cargo install edilmemişse hata
        console.print("[red]foxdpi-bypass binary bulunamadı — önce kurulum yapılmalı[/red]")
        sys.exit(1)
    return subprocess.run([binary, *args], check=check, capture_output=True, text=True)


def _is_running(config_path: Path) -> bool:
    """Motor çalışıyor mu? (config.yaml'da running flag veya process check)"""
    # Basit process check: foxdpi-bypass process'ı var mı?
    try:
        proc = subprocess.run(
            ["pgrep", "-f", "foxdpi-bypass"],
            capture_output=True,
            text=True,
            check=False,
        )
        return proc.returncode == 0
    except Exception:
        return False


@click.group(name="bypass")
def bypass_cmd() -> None:
    """FoxDPI'nin kendi DPI bypass motoru yönetimi — başlat, durdur, yapılandır."""
    pass


@bypass_cmd.command(name="start")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu (varsayılan: ~/.foxdpi/config.yaml)")
@click.option("--mode", "-m", default=None, help="Varsayılan strateji: default|split|desync|decoy|oob|host-scramble")
@click.option("--port", "-p", default=None, help="Proxy port (öntanımlı: 10850)")
@click.option("--verbose", "-v", is_flag=True, help="Detaylı log")
@click.option("--foreground", is_flag=True, help="Foreground'ta çalıştır (arka planda değil)")
@click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")
def start_cmd(
    config: str | None,
    mode: str | None,
    port: str | None,
    verbose: bool,
    foreground: bool,
    quiet: bool,
) -> None:
    """FoxDPI bypass motorunu başlat (SOCKS5 proxy + DPI bypass)."""
    config_path = Path(config) if config else _default_config_path()
    _ensure_config_exists(config_path)

    if _is_running(config_path):
        if not quiet:
            console.print("[yellow]Motor zaten çalışıyor[/yellow]")
        return

    args = ["start"]
    if mode:
        args += ["--mode", mode]
    if port:
        args += ["--port", port]
    if verbose:
        args.append("--verbose")
    if foreground:
        args.append("--foreground")

    if quiet or foreground:
        # doğrudan çalıştır
        try:
            _run_bypass_cli(args, check=False)
        except Exception as e:
            console.print(f"[red]Başlatma hatası: {e}[/red]")
            sys.exit(1)
    else:
        # arka planda çalıştır (nohup / & veya systemdแนะนำ)
        try:
            # basit arka plan başlatması (test amaçlı; production'da systemd önerilir)
            subprocess.Popen(
                ["foxdpi-bypass", *args],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            console.print(f"[green]✓ Motor başlatıldı (config: {config_path})[/green]")
        except Exception as e:
            console.print(f"[red]Başlatma hatası: {e}[/red]")
            sys.exit(1)


@bypass_cmd.command(name="stop")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu")
@click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")
def stop_cmd(config: str | None, quiet: bool) -> None:
    """Çalışan FoxDPI bypass motorunu durdur."""
    config_path = Path(config) if config else _default_config_path()

    if not _is_running(config_path):
        if not quiet:
            console.print("[dim]Motor çalışmıyor[/dim]")
        return

    try:
        # foxdpi-bypass stop işlemi (kill veya api ile)
        proc = subprocess.run(
            ["pkill", "-f", "foxdpi-bypass"],
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode == 0:
            console.print("[green]✓ Motor durduruldu[/green]")
        else:
            console.print("[yellow]Motor bulunamadı veya durdurulamadı[/yellow]")
    except Exception as e:
        console.print(f"[red]Durdurma hatası: {e}[/red]")
        sys.exit(1)


@bypass_cmd.command(name="status")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu")
def status_cmd(config: str | None) -> None:
    """Motor durumunu göster (çalışıyor mu, hangi mode, hangi port)."""
    config_path = Path(config) if config else _default_config_path()

    running = _is_running(config_path)
    table = Table(title=f"FoxDPI bypass motoru durumu (foxdpi {__version__})")
    table.add_column("Özellik", style="cyan")
    table.add_column("Değer", style="green")

    table.add_row("Çalışıyor", "evet" if running else "hayır")
    table.add_row("Config dosyası", str(config_path))

    if config_path.exists():
        try:
            content = config_path.read_text(encoding="utf-8")
            # basit parse: port ve mode
            import yaml
            cfg = yaml.safe_load(content)
            port_val = cfg.get("port", "10850")
            mode_val = cfg.get("mode", "default")
            table.add_row("Port", str(port_val))
            table.add_row("Mode", str(mode_val))
            dns_section = cfg.get("dns", {})
            table.add_row("DNS sağlayıcı", str(dns_section.get("provider", "built_in")))
            table.add_row("Şifreli DNS", str(dns_section.get("encrypted", True)))
            table.add_row("Block pages", str(len(dns_section.get("block_pages", []))))
        except Exception:
            table.add_row("Yapılandırma", "okunamadı")

    console.print(table)

    if not running:
        console.print("[yellow]Motor çalışmıyor — 'foxdpi bypass start' ile başlatın[/yellow]")


@bypass_cmd.command(name="configure")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu")
@click.option("--mode", "-m", default=None, help="Strateji (default|split|desync|decoy|oob|host-scramble)")
@click.option("--port", "-p", default=None, help="Port")
@click.option("--block-page", "-b", multiple=True, help="Engellenmesini istediğiniz domain veya IP (tekrarlanabilir)")
@click.option("--unblock-page", default=None, help="Engellenmeden kaldırılacak domain/IP")
@click.option("--dns-provider", default=None, help="DNS sağlayıcı: built_in|custom|none")
@click.option("--dns-servers", default=None, help="Özel DNS sunucuları (virgülle ayrılmış,örn. 9.9.9.9:53,1.1.1.1:53)")
@click.option("--dns-encrypted/--no-dns-encrypted", default=None, help="Şifreli DNS (DoH/DoT) etkinleştirme/kapatma")
@click.option("--block-ads/--no-block-ads", default=None, help="Reklam engelleme listesini otomatik ekleme/kapatma")
@click.option("--verbose/--no-verbose", default=None, help="Detaylı log")
@click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")
def configure_cmd(
    config: str | None,
    mode: str | None,
    port: str | None,
    block_page: tuple[str, ...],
    unblock_page: str | None,
    dns_provider: str | None,
    dns_servers: str | None,
    dns_encrypted: bool | None,
    block_ads: bool | None,
    verbose: bool | None,
    quiet: bool,
) -> None:
    """FoxDPI bypass motoru yapılandırmasını düzenle."""
    config_path = Path(config) if config else _default_config_path()
    _ensure_config_exists(config_path)

    import yaml

    try:
        cfg = yaml.safe_load(config_path.read_text(encoding="utf-8")) or {}
    except Exception as e:
        console.print(f"[red]Config okuma hatası: {e}[/red]")
        sys.exit(1)

    # Varsayılan yapılandırmayı tamamla
    cfg.setdefault("port", 10850)
    cfg.setdefault("mode", "default")
    cfg.setdefault("verbose", False)
    cfg.setdefault("dns", {
        "provider": "built_in",
        "custom_servers": [],
        "encrypted": True,
        "block_ads": False,
        "block_pages": [],
    })
    cfg.setdefault("rules", [])

    # Her bir ayarı uygula
    if mode is not None:
        valid_modes = ["default", "split", "desync", "decoy", "oob", "host-scramble"]
        if mode not in valid_modes:
            console.print(f"[red]Geçersiz mode: {mode}. Geçerli: {', '.join(valid_modes)}[/red]")
            sys.exit(1)
        cfg["mode"] = mode
        if not quiet:
            console.print(f"[cyan]Mode güncellendi: {mode}[/cyan]")

    if port is not None:
        try:
            port_int = int(port)
            if not (1 <= port_int <= 65535):
                raise ValueError("Port 1-65535 arasında olmalı")
            cfg["port"] = port_int
        except ValueError as e:
            console.print(f"[red]Geçersiz port: {e}[/red]")
            sys.exit(1)
        if not quiet:
            console.print(f"[cyan]Port güncellendi: {port}[/cyan]")

    if block_page:
        block_pages = cfg["dns"].setdefault("block_pages", [])
        added = 0
        for page in block_page:
            page_lower = page.lower()
            if page_lower not in block_pages:
                block_pages.append(page_lower)
                added += 1
        if not quiet:
            console.print(f"[cyan]{added} yeni engellenen sayfa eklendi[/cyan]")

    if unblock_page:
        block_pages = cfg["dns"].get("block_pages", [])
        if unblock_page.lower() in block_pages:
            block_pages.remove(unblock_page.lower())
            if not quiet:
                console.print(f"[cyan]Engellenen sayfa kaldırıldı: {unblock_page}[/cyan]")
        else:
            if not quiet:
                console.print(f"[dim]Bulunamadı: {unblock_page}[/dim]")

    if dns_provider is not None:
        valid_providers = ["built_in", "custom", "none"]
        if dns_provider not in valid_providers:
            console.print(f"[red]Geçersiz DNS sağlayıcı: {dns_provider}. Geçerli: {', '.join(valid_providers)}[/red]")
            sys.exit(1)
        cfg["dns"]["provider"] = dns_provider
        if not quiet:
            console.print(f"[cyan]DNS sağlayıcı güncellendi: {dns_provider}[/cyan]")

    if dns_servers is not None:
        servers = [s.strip() for s in dns_servers.split(",") if s.strip()]
        cfg["dns"]["custom_servers"] = servers
        if not quiet:
            console.print(f"[cyan]{len(servers)} özel DNS sunucusu eklendi[/cyan]")

    if dns_encrypted is not None:
        cfg["dns"]["encrypted"] = dns_encrypted
        if not quiet:
            console.print(f"[cyan]Şifreli DNS {'etkin' if dns_encrypted else 'kapatıldı'}[/cyan]")

    if block_ads is not None:
        cfg["dns"]["block_ads"] = block_ads
        if not quiet:
            console.print(f"[cyan]Reklam engelleme {'etkin' if block_ads else 'kapatıldı'}[/cyan]")

    if verbose is not None:
        cfg["verbose"] = verbose
        if not quiet:
            console.print(f"[cyan]Verbose log {'etkin' if verbose else 'kapatıldı'}[/cyan]")

    # Config'i yaz
    try:
        config_path.write_text(yaml.safe_dump(cfg, sort_keys=False), encoding="utf-8")
        if not quiet:
            console.print(f"[green]✓ Yapılandırma kaydedildi: {config_path}[/green]")
    except Exception as e:
        console.print(f"[red]Yapılandırma kaydetme hatası: {e}[/red]")
        sys.exit(1)


@bypass_cmd.command(name="block-pages")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu")
@click.option("--add", "-a", multiple=True, help="Eklemek istediğiniz domain/IP (tekrarlanabilir)")
@click.option("--remove", "-r", multiple=True, help="Kaldırmak istediğiniz domain/IP (tekrarlanabilir)")
@click.option("--list", is_flag=True, help="Mevcut listayı göster")
@click.option("--clear", is_flag=True, help="Tüm listayı temizle")
@click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")
def block_pages_cmd(
    config: str | None,
    add: tuple[str, ...],
    remove: tuple[str, ...],
    list: bool,
    clear: bool,
    quiet: bool,
) -> None:
    """Engellenen domain/IP listasını yönet (reklam/izleyici/block-page liste)."""
    config_path = Path(config) if config else _default_config_path()
    _ensure_config_exists(config_path)

    import yaml

    try:
        cfg = yaml.safe_load(config_path.read_text(encoding="utf-8")) or {}
    except Exception as e:
        console.print(f"[red]Config okuma hatası: {e}[/red]")
        sys.exit(1)

    block_pages = cfg.setdefault("dns", {}).setdefault("block_pages", [])

    if clear:
        block_pages.clear()
        if not quiet:
            console.print("[cyan]Tüm engellenen list temizlendi[/cyan]")
    elif add:
        added = 0
        for page in add:
            page_lower = page.lower()
            if page_lower not in block_pages:
                block_pages.append(page_lower)
                added += 1
        if not quiet:
            console.print(f"[cyan]{added} yeni sayfa eklendi[/cyan]")
    elif remove:
        removed = 0
        for page in remove:
            page_lower = page.lower()
            if page_lower in block_pages:
                block_pages.remove(page_lower)
                removed += 1
        if not quiet:
            console.print(f"[cyan]{removed} sayfa kaldırıldı[/cyan]")

    # Listeyi göster veya kaydet
    if list or add or remove or clear:
        try:
            config_path.write_text(yaml.safe_dump(cfg, sort_keys=False), encoding="utf-8")
            if list or not quiet:
                table = Table(title="Engellenen sayfalar / block-page listesi")
                table.add_column("Domain / IP", style="cyan")
                if block_pages:
                    for page in sorted(block_pages):
                        table.add_row(page)
                else:
                    table.add_row("[dim]Hiçbiri[/dim]")
                console.print(table)
        except Exception as e:
            console.print(f"[red]Kaydetme hatası: {e}[/red]")
            sys.exit(1)
    else:
        if not quiet:
            console.print("[dim]Hiçbir değişiklik yapılmadı[/dim]")


@bypass_cmd.command(name="rules")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu")
@click.option("--add", "-a", nargs=2, help="Kural ekle: DOMAIN STRATEJİ (örn. example.com split)")
@click.option("--remove", "-r", help="Kaldırmak istediğiniz kuralın domain'i")
@click.option("--list", is_flag=True, help="Kuralları listele")
@click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")
def rules_cmd(
    config: str | None,
    add: tuple[str, str] | None,
    remove: str | None,
    list: bool,
    quiet: bool,
) -> None:
    """Domain bazlı bypass kuralını yönet (exempt veya özel strateji)."""
    config_path = Path(config) if config else _default_config_path()
    _ensure_config_exists(config_path)

    import yaml

    try:
        cfg = yaml.safe_load(config_path.read_text(encoding="utf-8")) or {}
    except Exception as e:
        console.print(f"[red]Config okuma hatası: {e}[/red]")
        sys.exit(1)

    rules = cfg.setdefault("rules", [])

    if add:
        domain, strategy = add
        valid_modes = ["default", "split", "desync", "decoy", "oob", "host-scramble"]
        if strategy not in valid_modes:
            console.print(f"[red]Geçersiz strateji: {strategy}. Geçerli: {', '.join(valid_modes)}[/red]")
            sys.exit(1)
        rules.append({"domain": domain, "strategy": strategy, "exempt": False})
        if not quiet:
            console.print(f"[cyan]Kural eklendi: {domain} → {strategy}[/cyan]")

    if remove:
        original_len = len(rules)
        rules = [r for r in rules if r.get("domain") != remove]
        if len(rules) < original_len:
            cfg["rules"] = rules
            if not quiet:
                console.print(f"[cyan]Kural kaldırıldı: {remove}[/cyan]")
        else:
            if not quiet:
                console.print(f"[dim]Bulunamadı: {remove}[/dim]")

    if list or add or remove:
        try:
            config_path.write_text(yaml.safe_dump(cfg, sort_keys=False), encoding="utf-8")
            if list or not quiet:
                table = Table(title="Bypass kuralları")
                table.add_column("Domain", style="cyan")
                table.add_column("Strateji", style="green")
                table.add_column("Exempt", style="yellow")
                if rules:
                    for r in rules:
                        table.add_row(r.get("domain", ""), r.get("strategy", "default"), str(r.get("exempt", False)))
                else:
                    table.add_row("[dim]Hiçbiri[/dim]")
                console.print(table)
        except Exception as e:
            console.print(f"[red]Kaydetme hatası: {e}[/red]")
            sys.exit(1)


@bypass_cmd.command(name="dns")
@click.option("--config", "-c", default=None, help="Yapılandırma dosyası yolu")
@click.option("--provider", help="DNS sağlayıcı: built_in|custom|none")
@click.option("--servers", help="Özel DNS sunucuları (virgülle ayrılmış,örn. 9.9.9.9:53,1.1.1.1:53)")
@click.option("--encrypted/--no-encrypted", help="Şifreli DNS etkin/kapat")
@click.option("--block-ads/--no-block-ads", help="Otomatik reklam engelleme listesi ekle/kapat")
@click.option("--list", is_flag=True, help="Mevcut DNS ayarlarını göster")
@click.option("--quiet", "-q", is_flag=True, help="Sadece hata çıktısını göster")
def dns_cmd(
    config: str | None,
    provider: str | None,
    servers: str | None,
    encrypted: bool | None,
    block_ads: bool | None,
    list: bool,
    quiet: bool,
) -> None:
    """DNS yapılandırmasını yönet (provider, sunucular, şifreleme, block-ads)."""
    config_path = Path(config) if config else _default_config_path()
    _ensure_config_exists(config_path)

    import yaml

    try:
        cfg = yaml.safe_load(config_path.read_text(encoding="utf-8")) or {}
    except Exception as e:
        console.print(f"[red]Config okuma hatası: {e}[/red]")
        sys.exit(1)

    dns_cfg = cfg.setdefault("dns", {
        "provider": "built_in",
        "custom_servers": [],
        "encrypted": True,
        "block_ads": False,
        "block_pages": [],
    })

    if provider:
        valid = ["built_in", "custom", "none"]
        if provider not in valid:
            console.print(f"[red]Geçersiz provider: {provider}. Geçerli: {', '.join(valid)}[/red]")
            sys.exit(1)
        dns_cfg["provider"] = provider
        if not quiet:
            console.print(f"[cyan]DNS sağlayıcı: {provider}[/cyan]")

    if servers:
        server_list = [s.strip() for s in servers.split(",") if s.strip()]
        dns_cfg["custom_servers"] = server_list
        if not quiet:
            console.print(f"[cyan]{len(server_list)} özel DNS sunucusu eklendi[/cyan]")

    if encrypted is not None:
        dns_cfg["encrypted"] = encrypted
        if not quiet:
            console.print(f"[cyan]Şifreli DNS {'etkin' if encrypted else 'kapatıldı'}[/cyan]")

    if block_ads is not None:
        dns_cfg["block_ads"] = block_ads
        if not quiet:
            console.print(f"[cyan]Reklam engelleme {'etkin' if block_ads else 'kapatıldı'}[/cyan]")

    if list or provider or servers or encrypted is not None or block_ads is not None:
        try:
            config_path.write_text(yaml.safe_dump(cfg, sort_keys=False), encoding="utf-8")
            if list or not quiet:
                table = Table(title="DNS yapılandırması")
                table.add_column("Özellik", style="cyan")
                table.add_column("Değer", style="green")
                table.add_row("Sağlayıcı", str(dns_cfg.get("provider", "built_in")))
                table.add_row("Şifreli DNS", str(dns_cfg.get("encrypted", True)))
                table.add_row("Reklam engelleme", str(dns_cfg.get("block_ads", False)))
                table.add_row("Özel sunucular", ", ".join(dns_cfg.get("custom_servers", [])) or "[dim]hiçbiri[/dim]")
                if dns_cfg.get("block_pages"):
                    table.add_row("Block pages", str(len(dns_cfg["block_pages"])))
                console.print(table)
        except Exception as e:
            console.print(f"[red]Kaydetme hatası: {e}[/red]")
            sys.exit(1)
