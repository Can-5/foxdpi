#!/usr/bin/env python3
"""FoxDPI — kendi DPI/sansür atlama motoru için grafiksel arayüz.

foxdpi-bypass (Rust) SOCKS5 proxy'sini başlatır/durdurur, mod/DNS/reklam
engelleme ayarlarını yönetir, bağlantı testi yapar ve Discord'u proxy'li
açar. Yalnızca Python standart kütüphanesi (tkinter) kullanır.

Çalıştırma:
    foxdpi-gui                 # arayüzü aç

Notlar:
  - Tauri kaynak ağacı `foxdpi-gui/` ayrıdır; webkit2gtk-devel gerektirir
    (Fedora: sudo dnf install webkit2gtk4.1-devel) ve bu makinede derlenmez.
  - disorder/decoy/autottl ham socket ister (root/CAP_NET_RAW); motor EPERM
    alırsa uyarı ile sıradan akışa düşer.
"""

__version__ = "0.3.0"

import json
import os
import queue
import re
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import tkinter as tk
from tkinter import ttk, filedialog, messagebox

CONFIG_DIR = Path(os.path.expanduser("~/.config/foxdpi"))
SETTINGS_FILE = CONFIG_DIR / "gui-settings.json"
LOG_FILE = CONFIG_DIR / "foxdpi-gui.log"
DISCORD_PROFILE = CONFIG_DIR / "firefox-profile"

STRATEGIES = [
    ("record-split", "Record Split (önerilen)"),
    ("segment-desync", "Segment Desync"),
    ("disorder", "Disorder (root gerekir)"),
    ("decoy", "Decoy (root gerekir)"),
    ("host-scramble", "Host Scramble"),
    ("oob", "OOB (deneysel)"),
    ("default", "Varsayılan (bypass kapalı)"),
]
DNS_MODES = [("doh", "DoH (güvenli, varsayılan)"), ("builtin", "Builtin (TCP-53)"),
             ("custom", "Özel sunucular"), ("none", "Kapalı")]
ADBLOCK_MODES = [("off", "Kapalı"), ("nullroute", "NullRoute (0.0.0.0)"),
                 ("deny", "Bağlantıyı Reddet"), ("sinkhole", "Sinkhole (204)")]

DEFAULT_SETTINGS = {
    "port": 1080,
    "mode": "record-split",
    "dns": "doh",
    "doh_server": "",
    "custom_dns": "",
    "block_ads": False,
    "adblock": "off",
    "blocklist_file": "",
    "sinkhole_port": 8899,
    "split_offset": "",
    "desync_delay": 0,
    "decoy_ttl": 3,
    "autottl": False,
    "autottl_max": 6,
    "rules": [],
}


# --- renkler (midnight + canlı vurgular) ---
BG = "#141422"
PANEL = "#1e1e2e"
PANEL2 = "#262638"
FG = "#e8e6f0"
MUTED = "#8b8ba3"
ACCENT = "#f97316"   # fox orange
GREEN = "#22c55e"
RED = "#ef4444"
BLUE = "#3b82f6"
BLURPLE = "#5865f2"  # discord
YELLOW = "#facc15"


def find_engine():
    candidates = [
        os.environ.get("FOXDPI_BYPASS_BIN", ""),
        str(Path.home() / "bin/foxdpi-bypass"),
        str(Path.home() / ".local/bin/foxdpi-bypass"),
        str(Path.cwd() / "foxdpi-bypass"),
        str(Path.cwd() / "bypass/target/release/foxdpi-bypass"),
    ]
    for c in candidates:
        if c and os.path.isfile(c) and os.access(c, os.X_OK):
            return c
    return shutil.which("foxdpi-bypass")


def engine_version(path):
    try:
        out = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=5)
        return out.stdout.strip() or out.stderr.strip()
    except Exception as e:
        return f"(sürüm okunamadı: {e})"


def external_running():
    try:
        out = subprocess.run(["pgrep", "-x", "foxdpi-bypass"], capture_output=True, text=True, timeout=5)
        pids = [int(p) for p in out.stdout.split() if p.isdigit()]
        return pids[0] if pids else None
    except Exception:
        return None


class FoxDPIGui(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title(f"FoxDPI {__version__}")
        self.geometry("880x760")
        self.minsize(820, 640)

        self.engine = find_engine()
        self.child = None
        self.log_queue = queue.Queue()
        self.settings = self._load_settings()
        self.started_at = None

        self.engine_ver = engine_version(self.engine) if self.engine else None

        self._build_ui()
        self._log(f"motor: {self.engine or 'BULUNAMADI'} ({self.engine_ver or '-'})")
        self._poll()
        pid = external_running()
        if pid:
            self._log(f"uyarı: foxdpi-bypass zaten sistemde çalışıyor (pid {pid})")

    # ---------- ayarlar ----------
    def _load_settings(self):
        try:
            data = json.loads(SETTINGS_FILE.read_text(encoding="utf-8"))
            cfg = DEFAULT_SETTINGS.copy()
            cfg.update({k: v for k, v in data.items() if k in cfg})
            return cfg
        except Exception:
            return DEFAULT_SETTINGS.copy()

    def _save_settings(self):
        self.settings.update(self._collect_ui())
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        SETTINGS_FILE.write_text(json.dumps(self.settings, indent=2, ensure_ascii=False), encoding="utf-8")

    def _collect_ui(self):
        return {
            "port": self.port_var.get(),
            "mode": self.mode_var.get(),
            "dns": self.dns_var.get(),
            "doh_server": self.doh_var.get().strip(),
            "custom_dns": self.custom_dns_var.get().strip(),
            "block_ads": self.block_ads_var.get(),
            "adblock": self.adblock_var.get(),
            "blocklist_file": self.blocklist_var.get().strip(),
            "sinkhole_port": self.sinkhole_port_var.get(),
            "split_offset": self.split_offset_var.get().strip(),
            "desync_delay": self.desync_delay_var.get(),
            "decoy_ttl": self.decoy_ttl_var.get(),
            "autottl": self.autottl_var.get(),
            "autottl_max": self.autottl_max_var.get(),
            "rules": list(self.rules_list.get(0, tk.END)),
        }

    # ---------- UI ----------
    # ---------- tema ----------
    def _apply_theme(self):
        style = ttk.Style(self)
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass
        self.configure(background=BG)
        style.configure("Root.TFrame", background=BG)
        style.configure("TFrame", background=PANEL)
        style.configure("TLabel", background=PANEL, foreground=FG)
        style.configure("TNotebook", background=BG, borderwidth=0, tabmargins=[6, 6, 6, 0])
        style.configure("TNotebook.Tab", background=PANEL2, foreground=FG, padding=[18, 8],
                        font=("", 10, "bold"))
        style.map("TNotebook.Tab",
                  background=[("selected", ACCENT)],
                  foreground=[("selected", "#141422")])
        style.configure("TLabelframe", background=PANEL, bordercolor=PANEL2, relief="flat")
        style.configure("TLabelframe.Label", background=PANEL, foreground=ACCENT,
                        font=("", 9, "bold"))
        style.configure("TSeparator", background=PANEL2)
        for name, bg, active in (("FoxGo", GREEN, "#4ade80"),
                                 ("FoxStop", RED, "#f87171"),
                                 ("FoxTest", BLUE, "#60a5fa"),
                                 ("FoxDiscord", BLURPLE, "#7985f5")):
            style.configure(f"{name}.TButton", background=bg,
                            foreground="#062b11" if name == "FoxGo" else "#ffffff",
                            font=("", 10, "bold"), padding=[14, 6], borderwidth=0)
            style.map(f"{name}.TButton",
                      background=[("active", active), ("disabled", PANEL2)],
                      foreground=[("disabled", MUTED)])
        style.configure("TButton", background=PANEL2, foreground=FG, padding=[10, 5], borderwidth=0)
        style.map("TButton", background=[("active", "#33334d")])
        style.configure("TCombobox", fieldbackground=PANEL2, background=PANEL2,
                        foreground=FG, arrowcolor=FG, padding=4)
        style.map("TCombobox",
                  fieldbackground=[("readonly", PANEL2)],
                  foreground=[("readonly", FG)])
        style.configure("TEntry", fieldbackground=PANEL2, foreground=FG,
                        insertcolor=FG, padding=4)
        style.configure("TSpinbox", fieldbackground=PANEL2, background=PANEL2,
                        foreground=FG, arrowcolor=FG)
        style.configure("TCheckbutton", background=PANEL, foreground=FG)
        style.map("TCheckbutton", background=[("active", PANEL)])
        style.configure("TScrollbar", background=PANEL2, troughcolor=BG, arrowcolor=MUTED)

    def _build_ui(self):
        s = self.settings
        self._apply_theme()
        root = ttk.Frame(self, padding=10, style="Root.TFrame")
        root.pack(fill=tk.BOTH, expand=True)
        root.columnconfigure(0, weight=1)
        root.rowconfigure(4, weight=1)

        hdr = tk.Frame(root, bg=ACCENT, padx=16, pady=12)
        hdr.grid(row=0, column=0, sticky="ew")
        tk.Label(hdr, text="🦊 FoxDPI", bg=ACCENT, fg="#141422",
                 font=("", 22, "bold")).pack(side=tk.LEFT)
        tk.Label(hdr, text=" kendi DPI/sansür atlatma motorumuz", bg=ACCENT,
                 fg="#3a1c04", font=("", 10, "bold")).pack(side=tk.LEFT, padx=8)
        self.engine_lbl = tk.Label(hdr, text="", bg=ACCENT, fg="#3a1c04", font=("", 8))
        self.engine_lbl.pack(side=tk.RIGHT, padx=4)

        nb = ttk.Notebook(root)
        nb.grid(row=1, column=0, sticky="ew", pady=(8, 4))

        basic = ttk.Frame(nb, padding=10)
        advanced = ttk.Frame(nb, padding=10)
        nb.add(basic, text="Genel")
        nb.add(advanced, text="Gelişmiş")
        for i in range(4):
            basic.columnconfigure(i, weight=1)

        # --- Genel sekmesi ---
        ttk.Label(basic, text="Strateji").grid(row=0, column=0, sticky="w")
        self.mode_var = tk.StringVar(value=s["mode"])
        mode_box = ttk.Combobox(basic, textvariable=self.mode_var, values=[m for m, _ in STRATEGIES],
                                state="readonly", width=18)
        mode_box.grid(row=1, column=0, sticky="ew", padx=(0, 8))
        self.mode_hint = ttk.Label(basic, text="", font=("", 8))
        self.mode_hint.grid(row=2, column=0, sticky="w")
        self.mode_var.trace_add("write", lambda *_: self._update_mode_hint())

        ttk.Label(basic, text="Port").grid(row=0, column=1, sticky="w")
        self.port_var = tk.IntVar(value=s["port"])
        ttk.Spinbox(basic, from_=1024, to=65535, textvariable=self.port_var, width=8).grid(row=1, column=1, sticky="w", padx=(0, 8))

        ttk.Label(basic, text="DNS").grid(row=0, column=2, sticky="w")
        self.dns_var = tk.StringVar(value=s["dns"])
        ttk.Combobox(basic, textvariable=self.dns_var, values=[d for d, _ in DNS_MODES],
                     state="readonly", width=10).grid(row=1, column=2, sticky="ew", padx=(0, 8))

        ttk.Label(basic, text="DoH sunucu (opsiyonel)").grid(row=3, column=0, sticky="w", columnspan=2, pady=(8, 0))
        self.doh_var = tk.StringVar(value=s["doh_server"])
        ttk.Entry(basic, textvariable=self.doh_var).grid(row=4, column=0, columnspan=2, sticky="ew", padx=(0, 8))

        ttk.Label(basic, text="Özel DNS sunucuları (virgülle)").grid(row=3, column=2, sticky="w", columnspan=2)
        self.custom_dns_var = tk.StringVar(value=s["custom_dns"])
        ttk.Entry(basic, textvariable=self.custom_dns_var).grid(row=4, column=2, columnspan=2, sticky="ew")

        ttk.Separator(basic, orient="horizontal").grid(row=5, column=0, columnspan=4, sticky="ew", pady=10)

        ttk.Label(basic, text="Reklam engelleme").grid(row=6, column=0, sticky="w")
        self.adblock_var = tk.StringVar(value=s["adblock"])
        ttk.Combobox(basic, textvariable=self.adblock_var, values=[a for a, _ in ADBLOCK_MODES],
                     state="readonly", width=14).grid(row=7, column=0, sticky="ew", padx=(0, 8))
        self.block_ads_var = tk.BooleanVar(value=s["block_ads"])
        ttk.Checkbutton(basic, text="--block-ads (builtin liste + NullRoute)", variable=self.block_ads_var).grid(
            row=7, column=1, columnspan=3, sticky="w")

        ttk.Label(basic, text="Blocklist dosyası (hosts formatı)").grid(row=8, column=0, columnspan=2, sticky="w", pady=(8, 0))
        bf = ttk.Frame(basic)
        bf.grid(row=9, column=0, columnspan=3, sticky="ew")
        self.blocklist_var = tk.StringVar(value=s["blocklist_file"])
        ttk.Entry(bf, textvariable=self.blocklist_var).pack(side=tk.LEFT, fill=tk.X, expand=True)
        ttk.Button(bf, text="Seç…", command=self._pick_blocklist).pack(side=tk.LEFT, padx=(6, 0))
        ttk.Label(basic, text="Sinkhole portu").grid(row=8, column=3, sticky="w", pady=(8, 0))
        self.sinkhole_port_var = tk.IntVar(value=s["sinkhole_port"])
        ttk.Spinbox(basic, from_=1024, to=65535, textvariable=self.sinkhole_port_var, width=8).grid(row=9, column=3, sticky="w")

        rf = ttk.LabelFrame(basic, text="Kurallar (--rule DOMAIN[:strategy|:exempt])", padding=8)
        rf.grid(row=10, column=0, columnspan=4, sticky="ew", pady=(10, 0))
        rf.columnconfigure(0, weight=1)
        self.rules_list = tk.Listbox(rf, height=3, bg=PANEL2, fg=FG, selectbackground=ACCENT,
                                      selectforeground="#141422", borderwidth=0,
                                      highlightthickness=1, highlightbackground=PANEL2,
                                      highlightcolor=ACCENT, activestyle="none")
        self.rules_list.grid(row=0, column=0, rowspan=2, sticky="ew")
        for r in s["rules"]:
            self.rules_list.insert(tk.END, r)
        self.rule_entry = ttk.Entry(rf)
        self.rule_entry.grid(row=0, column=1, sticky="ew", padx=(8, 0))
        self.rule_entry.insert(0, "ornek.com:record-split")
        bf2 = ttk.Frame(rf)
        bf2.grid(row=1, column=1, sticky="w", padx=(8, 0), pady=(4, 0))
        ttk.Button(bf2, text="Ekle", command=self._add_rule).pack(side=tk.LEFT)
        ttk.Button(bf2, text="Sil", command=self._del_rule).pack(side=tk.LEFT, padx=(6, 0))

        # --- Gelişmiş sekmesi ---
        ttk.Label(advanced, text="Split offset (boş = SNI'ya göre otomatik)").grid(row=0, column=0, sticky="w")
        self.split_offset_var = tk.StringVar(value=s["split_offset"])
        ttk.Entry(advanced, textvariable=self.split_offset_var, width=10).grid(row=0, column=1, sticky="w")

        ttk.Label(advanced, text="Segment desync gecikmesi (ms)").grid(row=1, column=0, sticky="w", pady=(8, 0))
        self.desync_delay_var = tk.IntVar(value=s["desync_delay"])
        ttk.Spinbox(advanced, from_=0, to=1000, textvariable=self.desync_delay_var, width=8).grid(row=1, column=1, sticky="w", pady=(8, 0))

        ttk.Label(advanced, text="Decoy / Disorder TTL").grid(row=2, column=0, sticky="w", pady=(8, 0))
        self.decoy_ttl_var = tk.IntVar(value=s["decoy_ttl"])
        ttk.Spinbox(advanced, from_=1, to=64, textvariable=self.decoy_ttl_var, width=8).grid(row=2, column=1, sticky="w", pady=(8, 0))

        self.autottl_var = tk.BooleanVar(value=s["autottl"])
        ttk.Checkbutton(advanced, text="AutoTTL (artan TTL kopyaları)", variable=self.autottl_var).grid(row=3, column=0, columnspan=2, sticky="w", pady=(8, 0))
        ttk.Label(advanced, text="AutoTTL maks").grid(row=4, column=0, sticky="w", pady=(8, 0))
        self.autottl_max_var = tk.IntVar(value=s["autottl_max"])
        ttk.Spinbox(advanced, from_=1, to=64, textvariable=self.autottl_max_var, width=8).grid(row=4, column=1, sticky="w", pady=(8, 0))
        ttk.Label(advanced, text="Not: disorder/decoy/autottl ham socket ister; root olmadan motor uyarı verip devam eder.",
                  font=("", 8), foreground="#fbbf24").grid(row=5, column=0, columnspan=3, sticky="w", pady=(12, 0))

        # --- Start/Stop + durum ---
        ctl = ttk.Frame(root)
        ctl.grid(row=2, column=0, sticky="ew", pady=(4, 4))
        self.start_btn = ttk.Button(ctl, text="▶  Başlat", command=self.start_engine, style="FoxGo.TButton")
        self.start_btn.pack(side=tk.LEFT, ipadx=12, ipady=5)
        self.stop_btn = ttk.Button(ctl, text="■  Durdur", command=self.stop_engine, state="disabled", style="FoxStop.TButton")
        self.stop_btn.pack(side=tk.LEFT, padx=(8, 0), ipadx=12, ipady=5)
        self.test_btn = ttk.Button(ctl, text="🔗 Bağlantı Testi", command=self.connection_test, style="FoxTest.TButton")
        self.test_btn.pack(side=tk.LEFT, padx=(8, 0), ipady=4)
        self.discord_btn = ttk.Button(ctl, text="🎮 Discord'u Aç", command=self.launch_discord, style="FoxDiscord.TButton")
        self.discord_btn.pack(side=tk.LEFT, padx=(8, 0), ipady=4)
        self.status_lbl = tk.Label(ctl, text="○ çalışmıyor", bg=PANEL, fg=RED, font=("", 10, "bold"))
        self.status_lbl.pack(side=tk.RIGHT)

        logf = ttk.LabelFrame(root, text="Motor günlüğü", padding=6)
        logf.grid(row=4, column=0, sticky="nsew", pady=(4, 0))
        logf.columnconfigure(0, weight=1)
        logf.rowconfigure(0, weight=1)
        self.log_text = tk.Text(logf, height=12, state="disabled", wrap="word",
                                font=("monospace", 9), bg="#101018", fg="#a6e3a1",
                                insertbackground="#a6e3a1", borderwidth=0,
                                highlightthickness=1, highlightbackground=PANEL2,
                                highlightcolor=ACCENT, padx=6, pady=4)
        self.log_text.tag_configure("warn", foreground="#facc15")
        self.log_text.tag_configure("error", foreground="#f87171")
        self.log_text.tag_configure("info", foreground="#7dd3fc")
        sb = ttk.Scrollbar(logf, command=self.log_text.yview)
        self.log_text.configure(yscrollcommand=sb.set)
        self.log_text.grid(row=0, column=0, sticky="nsew")
        sb.grid(row=0, column=1, sticky="ns")

        foot = tk.Label(root, text=f"FoxDPI {__version__} — kendi DPI bypass motorumuz · SOCKS5 127.0.0.1 · kapanışta GUI'nin başlattığı motor kapatılır",
                        bg=BG, font=("", 8), fg=MUTED)
        foot.grid(row=5, column=0, sticky="ew", pady=(4, 0))
        self.protocol("WM_DELETE_WINDOW", self.on_close)
        self._update_mode_hint()

    def _update_mode_hint(self):
        hints = {
            "record-split": "Önerilen: TLS ClientHello'yu SNI'dan iki kayda böler",
            "segment-desync": "ClientHello'yu küçük parçalara yazar (TCP_NODELAY)",
            "disorder": "Düşük TTL sahte kopya + gerçek sıra (ham socket/root)",
            "decoy": "Sahte SNI'lı düşük TTL paket (ham socket/root)",
            "host-scramble": "HTTP Host başlığı case'ini karıştırır",
            "oob": "İlk baytı urgent pointer (ağlarda çoğunlukla RST alır)",
            "default": "Bypass kapalı, düz forward",
        }
        self.mode_hint.configure(text=hints.get(self.mode_var.get(), ""))

    def _pick_blocklist(self):
        path = filedialog.askopenfilename(title="Blocklist dosyası seç",
                                          filetypes=[("Blocklist/hosts", "*.txt *.hosts *.list *.conf"), ("Tüm dosyalar", "*.*")])
        if path:
            self.blocklist_var.set(path)

    def _add_rule(self):
        spec = self.rule_entry.get().strip()
        if not spec:
            return
        if not re.match(r"^[A-Za-z0-9*.\-]+(:(record-split|segment-desync|disorder|decoy|host-scramble|oob|default|exempt|none|bypass))?$", spec):
            messagebox.showwarning("Kural biçimi", "Örnek: youtube.com:record-split, *.ornek.com:exempt veya ornek.com.tr (stratejisiz)")
            return
        self.rules_list.insert(tk.END, spec)
        self.rule_entry.delete(0, tk.END)

    def _del_rule(self):
        for i in reversed(self.rules_list.curselection()):
            self.rules_list.delete(i)

    # ---------- motor kontrolü ----------
    def _engine_args(self):
        a = [self.engine, "-p", str(self.port_var.get()), "-m", self.mode_var.get(), "--dns", self.dns_var.get()]
        if self.dns_var.get() == "doh" and self.doh_var.get().strip():
            a += ["--doh-server", self.doh_var.get().strip()]
        if self.dns_var.get() == "custom" and self.custom_dns_var.get().strip():
            a += ["--custom-dns", self.custom_dns_var.get().strip()]
        if self.block_ads_var.get():
            a.append("--block-ads")
        if self.adblock_var.get() != "off":
            a += ["--adblock", self.adblock_var.get()]
            if self.adblock_var.get() == "sinkhole":
                a += ["--sinkhole-port", str(self.sinkhole_port_var.get())]
        if self.blocklist_var.get().strip():
            bf = self.blocklist_var.get().strip()
            if not os.path.isfile(bf):
                raise FileNotFoundError(f"blocklist dosyası bulunamadı: {bf}")
            a += ["--blocklist-file", bf]
        if self.split_offset_var.get().strip():
            a += ["--split-offset", self.split_offset_var.get().strip()]
        if self.desync_delay_var.get() > 0:
            a += ["--desync-delay", str(self.desync_delay_var.get())]
        a += ["--decoy-ttl", str(self.decoy_ttl_var.get())]
        if self.autottl_var.get():
            a += ["--autottl", "--autottl-max", str(self.autottl_max_var.get())]
        for r in self.rules_list.get(0, tk.END):
            a += ["--rule", r]
        return a

    def start_engine(self):
        if self.engine is None:
            messagebox.showerror("Motor bulunamadı", "foxdpi-bypass bulunamadı.\nÖnce derleyin: cd bypass && cargo build --release\nya da install.sh ile kurun.")
            return
        if self.child is not None and self.child.poll() is None:
            return
        pid = external_running()
        if pid:
            if messagebox.askyesno("Engine çalışıyor", f"foxdpi-bypass sistemde zaten çalışıyor (pid {pid}).\nDurdurup bu ayarlarla yeniden başlatılsın mı?"):
                subprocess.run(["pkill", "-x", "foxdpi-bypass"])
                time.sleep(0.3)
            else:
                return
        try:
            args = self._engine_args()
        except FileNotFoundError as e:
            messagebox.showerror("Dosya bulunamadı", str(e))
            return
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        self._log("başlatılıyor: " + " ".join(args))
        with open(LOG_FILE, "ab") as logf:
            logf.write(("\n=== foxdpi-gui start %s ===\n" % time.strftime("%Y-%m-%d %H:%M:%S")).encode())
        self.child = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                      stdin=subprocess.DEVNULL, start_new_session=True)
        self._start_log_reader(self.child.stdout)
        self.started_at = time.time()
        self._save_settings()
        self._update_state()

    def _start_log_reader(self, stream):
        def reader():
            try:
                for raw in iter(stream.readline, b""):
                    line = raw.decode("utf-8", errors="replace").rstrip()
                    if line:
                        self.log_queue.put(line)
                        with open(LOG_FILE, "a", encoding="utf-8") as f:
                            f.write(line + "\n")
            except Exception as e:
                self.log_queue.put(f"(log okuyucu kapandı: {e})")
        threading.Thread(target=reader, daemon=True).start()

    def stop_engine(self):
        if self.child is not None and self.child.poll() is None:
            self.child.terminate()
            try:
                self.child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.child.kill()
            self._log("motor durduruldu (SIGTERM)")
        else:
            r = subprocess.run(["pkill", "-x", "foxdpi-bypass"])
            self._log("motor pkill ile durduruldu" if r.returncode == 0 else "çalışan motor bulunamadı")
        self.child = None
        self.started_at = None
        self._update_state()

    def connection_test(self):
        if shutil.which("curl") is None:
            messagebox.showerror("curl yok", "Bağlantı testi curl gerektirir.")
            return
        port = self.port_var.get()
        self._log(f"test: curl --socks5-hostname 127.0.0.1:{port} https://discord.com/app")
        try:
            out = subprocess.run(
                ["curl", "--socks5-hostname", f"127.0.0.1:{port}", "-s", "-o", "/dev/null",
                 "-w", "%{http_code}", "--max-time", "15", "https://discord.com/app"],
                capture_output=True, text=True, timeout=20)
            code = out.stdout.strip() or "000"
            ok = code.startswith(("2", "3"))
            self._log(f"test sonucu: HTTP {code}")
            messagebox.showinfo("Bağlantı Testi", f"discord.com → HTTP {code}\n{'✓ Proxy üzerinden erişim var' if ok else '✗ Erişim yok (motor çalışıyor mu? doğru mod seçili mi?)'}")
        except subprocess.TimeoutExpired:
            self._log("test: zaman aşımı (15s)")
            messagebox.showwarning("Bağlantı Testi", "Zaman aşımı — motor çalışıyor mu?")

    def launch_discord(self):
        ff = shutil.which("firefox") or ("/usr/bin/firefox" if os.path.isfile("/usr/bin/firefox") else None)
        if not ff:
            messagebox.showerror("Firefox yok", "Discord'u açabilmek için Firefox kurulu olmalı. Alternatif: tarayıcınızın SOCKS5 proxy'sini 127.0.0.1:PORT yapın.")
            return
        port = self.port_var.get()
        DISCORD_PROFILE.mkdir(parents=True, exist_ok=True)
        (DISCORD_PROFILE / "user.js").write_text(
            f'user_pref("network.proxy.type", 1);\n'
            f'user_pref("network.proxy.socks", "127.0.0.1");\n'
            f'user_pref("network.proxy.socks_port", {port});\n'
            f'user_pref("network.proxy.socks_remote_dns", true);\n'
            f'user_pref("network.proxy.no_proxies_on", "");\n',
            encoding="utf-8")
        subprocess.Popen([ff, "--no-remote", "-profile", str(DISCORD_PROFILE), "https://discord.com/app"],
                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                         start_new_session=True)
        self._log(f"discord açıldı (firefox, proxy 127.0.0.1:{port})")

    # ---------- durum/döngü ----------
    def _log(self, line):
        self.log_text.configure(state="normal")
        low = line.lower()
        if "error" in low or "hata" in low or "bulunamadı" in low:
            tag = "error"
        elif "warn" in low or "uyarı" in low:
            tag = "warn"
        elif "doh" in low or "çözüldü" in low:
            tag = "info"
        else:
            tag = None
        self.log_text.insert(tk.END, time.strftime("%H:%M:%S ") + line + "\n", tag)
        self.log_text.see(tk.END)
        self.log_text.configure(state="disabled")

    def _poll(self):
        try:
            while True:
                self._log(self.log_queue.get_nowait())
        except queue.Empty:
            pass
        self._update_state()
        self.after(500, self._poll)

    def _update_state(self):
        running = (self.child is not None and self.child.poll() is None) or external_running() is not None
        if running:
            uptime = ""
            if self.started_at:
                uptime = f" · {int(time.time() - self.started_at)}s"
            self.status_lbl.configure(text=f"● çalışıyor{uptime}", fg="#4ade80")
            self.start_btn.configure(state="disabled")
            self.stop_btn.configure(state="normal")
        else:
            if self.child is not None and self.child.poll() is not None:
                self.child = None  # motor kendi kendine çıktı
            self.status_lbl.configure(text="○ çalışmıyor", fg="#f87171")
            self.start_btn.configure(state="normal")
            self.stop_btn.configure(state="disabled")
        if self.engine:
            self.engine_lbl.configure(text=f"motor: {self.engine}  ({self.engine_ver})", fg="#3a1c04")
        else:
            self.engine_lbl.configure(text="motor BULUNAMADI — önce kurun", fg="#7f1d1d")

    def on_close(self):
        if self.child is not None and self.child.poll() is None:
            if messagebox.askyesno("Çıkış", "GUI'nin başlattığı motor da kapatılsın mı?"):
                self.stop_engine()
            elif self.child is not None and self.child.poll() is None:
                return
        self._save_settings()
        self.destroy()


if __name__ == "__main__":
    app = FoxDPIGui()
    app.mainloop()
