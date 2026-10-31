#!/usr/bin/env sh
# install.sh — FoxDPI kurulum scripti (şablon aşamasında)
# Kullanım:
#   curl -fsSL https://.../install.sh | sh
#   curl -fsSL https://.../install.sh | sh -s -- --system
#   curl -fsSL https://.../install.sh | sh -s -- --prefix $HOME/.local
set -e

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
STATE_DIR="$PREFIX/share/foxdpi"
CACHE_DIR="$HOME/.cache/foxdpi"

usage() {
  printf '%s\n' \
    "Kullanım: install.sh [seçenekler]" \
    "Seçenekler:" \
    "  --system          Sistem genelinde kur (sudo gerektirir, /usr/local/bin)" \
    "  --prefix PREFIX   Özel kurulum dizini (öntanımlı: ~/.local)" \
    "  --help            Bu mesajı göster"
  exit 0
}

SYSTEM=0
while [ $# -gt 0 ]; do
  case "$1" in
    --system) SYSTEM=1; shift ;;
    --prefix) PREFIX="$2"; shift 2 ;;
    --help) usage ;;
    *) echo "Bilinmeyen seçenek: $1" >&2; exit 1 ;;
  esac
done

if [ "$SYSTEM" -eq 1 ]; then
  BIN_DIR="/usr/local/bin"
  STATE_DIR="/usr/local/share/foxdpi"
  : "${SUDO:=sudo}"
else
  SUDO=""
  BIN_DIR="$PREFIX/bin"
  STATE_DIR="$PREFIX/share/foxdpi"
fi

REQUIRED_TOOLS="curl tar grep sed"
for tool in $REQUIRED_TOOLS; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Hata: '$tool' bulunamadı. Kurulumu tamamlamak için öncelikle kurun." >&2
    exit 1
  fi
done

for dir in "$BIN_DIR" "$STATE_DIR" "$CACHE_DIR"; do
  $SUDO mkdir -p "$dir"
done

echo "==> FoxDPI motorlarını hazırlıyorum…"

# foxdpi-bypass (Rust binary veya Node.js şablon wrapper)
FOXDPI_BYPASS_STATE="$STATE_DIR/foxdpi-bypass"
$SUDO mkdir -p "$FOXDPI_BYPASS_STATE"

if [ ! -f "$FOXDPI_BYPASS_STATE/index.js" ]; then
  # şablon Node.js dosyasını kopyala (gerçek buildda binary veya dosya indirilecek)
  if [ -f "foxdpi/bypass/index.js" ]; then
    $SUDO cp "foxdpi/bypass/index.js" "$FOXDPI_BYPASS_STATE/index.js"
    $SUDO chmod +x "$FOXDPI_BYPASS_STATE/index.js"
    echo "==> foxdpi-bypass şablon Node.js dosyası kopylandı"
  else
    : > "$FOXDPI_BYPASS_STATE/index.js"
    echo "==> foxdpi-bypass şablon placeholder oluşturuldu"
  fi
fi

# foxdpi-bypass wrapper
WRAPPER="$BIN_DIR/foxdpi-bypass"
if [ "$SYSTEM" -eq 1 ] || [ -w "$BIN_DIR" ]; then
  cat > "$WRAPPER" <<'WRAPPER_EOF'
#!/usr/bin/env node
/**
 * foxdpi-bypass wrapper — FoxDPI'nin kendi DPI bypass motoru (şablon)
 * Gerçek motor ~/.local/share/foxdpi/foxdpi-bypass/index.js veya
 * şu andaki buildda ~/.local/share/foxdpi/foxdpi-bypass/ altındaki dosyalar.
 */
"use strict";
const path = require("path");
const fs = require("fs");

const engineDir = process.env.FOXDPI_ENGINE_DIR || path.join(
  process.env.HOME || process.env.USERPROFILE || ".",
  ".local", "share", "foxdpi", "foxdpi-bypass"
);

let engineFile = path.join(engineDir, "index.js");
if (!fs.existsSync(engineFile) && fs.existsSync(path.join(engineDir, "foxdpi-bypass", "index.js"))) {
  engineFile = path.join(engineDir, "foxdpi-bypass", "index.js");
}

if (!fs.existsSync(engineFile)) {
  console.error("foxdpi-bypass motor dosyası bulunamadı:", engineFile);
  console.error("Kurulumu tamamlayın veya FOXDPI_ENGINE_DIR'ini doğru ayarlayın.");
  process.exit(1);
}

const engine = require(engineFile);
if (typeof engine === "function") {
  engine(process.argv.slice(2));
} else if (engine && typeof engine.start === "function") {
  engine.start(process.argv.slice(2));
} else {
  console.error("foxdpi-bypass motoru çalıştırılabilir değil");
  process.exit(1);
}
WRAPPER_EOF
  $SUDO chmod +x "$WRAPPER"
  echo "==> foxdpi-bypass wrapper yazıldı: $WRAPPER"
else
  echo "Uyarı: $BIN_DIR yazılabilir değil, wrapper oluşturulamadı." >&2
fi

# FoxDPI CLI (Python) paketi kurulumu
echo "==> FoxDPI CLI (Python) paketi kuruluyor…"
if [ -f "$STATE_DIR/foxdpi/foxdpi" ]; then
  CLI_SRC="$STATE_DIR/foxdpi/foxdpi"
  CLI_LINK="$BIN_DIR/foxdpi"
  if [ -x "$CLI_SRC" ]; then
    $SUDO ln -sf "$CLI_SRC" "$CLI_LINK"
    echo "==> foxdpi -> $CLI_LINK"
  fi
fi

# şablon sistem proxy ayarları (gerçek implementasyonda GNOME/KDE/XFCE/LXQt)
SYSTEM_PROXY_CONF="$HOME/.config/foxdpi/90-foxdpi-proxy.conf"
mkdir -p "$(dirname "$SYSTEM_PROXY_CONF")"
if [ ! -f "$SYSTEM_PROXY_CONF" ]; then
  cat > "$SYSTEM_PROXY_CONF" <<'PROXY_EOF'
# foxdpi-bypass şablon sistem proxy ayarları
# Bu dosya gerçek implementasyonda GNOME/KDE/XFCE/LXQt proxy/PAC ayarlarını
# yönetecek. Şu an sadece placeholder.
export http_proxy="http://127.0.0.1:10850"
export https_proxy="http://127.0.0.1:10850"
export all_proxy="socks5://127.0.0.1:10850"
PROXY_EOF
  echo "==> şablon sistem proxy dosyası oluşturuldu: $SYSTEM_PROXY_CONF"
fi

echo "==> Kurulum tamamlandı."
if [ "$SYSTEM" -eq 1 ]; then
  echo "Not: sistem genelinde kuruldu; foxdpi-bypass çalıştırmak için sudo gerekebilir."
else
  echo "Not: kullanıcı düzeyinde kuruldu; PATH'e $BIN_DIR eklemeyi unutmayın."
fi

echo "Sonraki adımlar:"
echo "  - foxdpi doctor          # sistem kontrolü"
echo "  - foxdpi bypass start    # DPI bypass motorunu çalıştır (şablon)"
echo "  - foxdpi install <dosya> # paket kur"
echo "  - foxdpi uninstall      # kurulu paketleri kaldır"
echo "  - foxdpi --help         # tüm komutlar"
