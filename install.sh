#!/usr/bin/env sh
# install.sh — FoxDPI kurulum scripti
# Kullanım:
#   curl -fsSL https://.../install.sh | sh
#   curl -fsSL https://.../install.sh | sh -s -- --system
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

# Basit argüman parserı
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

# Gerekli araçlar
REQUIRED_TOOLS="curl tar grep sed"
for tool in $REQUIRED_TOOLS; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Hata: '$tool' bulunamadı. Lütfen öncelikle kurun." >&2
    exit 1
  fi
done

# Dizinleri oluştur
for dir in "$BIN_DIR" "$STATE_DIR" "$CACHE_DIR"; do
  $SUDO mkdir -p "$dir"
done

# FoxDPI ikilisini indir (bu örnekte yerel dosya üzerinden kurulum),
# gerçek deploymentta GitHub releases / kendi depolarından ~/.cache/foxdpi/altına indirilir.
echo "==> FoxDPI motorlarını hazırlıyorum…"

# Örnek: foxdpi-bypass ikilisi (Node.js ile çalıştırılabilir, tek dosya).
# Gerçek buildta bu adım foxdpi-bypass'i derler / pakeder.
FOXDPI_BYPASS="$STATE_DIR/foxdpi-bypass/index.js"
$SUDO mkdir -p "$(dirname "$FOXDPI_BYPASS")"

if [ ! -f "$FOXDPI_BYPASS" ]; then
  # placeholder: gerçek kurulumta bu dosya FoxDPI depolarından indirilir.
  : > "$FOXDPI_BYPASS"
fi

# Shim / wrapper: foxdpi-bypass komutunu $BIN_DIR ya koy.
WRAPPER="$BIN_DIR/foxdpi-bypass"
if [ "$SYSTEM" -eq 1 ] || [ -w "$BIN_DIR" ]; then
  cat > "$WRAPPER" <<'WRAPPER_EOF'
#!/usr/bin/env node
/**
 * foxdpi-bypass wrapper — FoxDPI'nin kendi DPI bypass motoru
 * Gerçek motor FoxDPI depolarından ~/.share/foxdpi/foxdpi-bypass/index.js veya
 * benzeri bir konumdan yüklenir.
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
  console.error("Kurulumı tamamlayın veya FOXDPI_ENGINE_DIR'ini doğru ayarlayın.");
  process.exit(1);
}

// Motoru yükle ve çalıştır
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

# FoxDPI CLI (Python)
echo "==> FoxDPI CLI (Python) paketi kuruluyor…"
if [ -f "$STATE_DIR/foxdpi/foxdpi" ]; then
  CLI_SRC="$STATE_DIR/foxdpi/foxdpi"
  CLI_LINK="$BIN_DIR/foxdpi"
  if [ -x "$CLI_SRC" ]; then
    $SUDO ln -sf "$CLI_SRC" "$CLI_LINK"
    echo "==> foxdpi -> $CLI_LINK"
  fi
fi

echo "==> Kurulum tamamlandı."
if [ "$SYSTEM" -eq 1 ]; then
  echo "Not: sistem genelinde kuruldu. foxdpi-bypass çalıştırmak için sudo gerekebilir."
else
  echo "Not: kullanıcı düzeyinde kuruldu. PATH'e $BIN_DIR eklemeyi unutmayın."
fi

echo "Sonraki adımlar:"
echo "  - foxdpi doctor          # sistem kontrolü"
echo "  - foxdpi bypass start    # DPI bypass motorunu çalıştır"
echo "  - foxdpi install <dosya> # paket kur"
echo "  - foxdpi uninstall      # kurulu paketleri kaldır"
echo "  - foxdpi --help         # tüm komutlar"
