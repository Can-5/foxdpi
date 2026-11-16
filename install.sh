#!/usr/bin/env sh
# install.sh — FoxDPI kurulum scripti
# foxdpi-bypass, Rust motoru (bypass/target/release/foxdpi-bypass) olarak kurulur.
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

# foxdpi-bypass (Rust motoru)
# Öncelik: repo içinde derlenmiş ikili (<script>/bypass/target/release/foxdpi-bypass),
# yoksa release URL'sinden indirme denemesi; ikisi de başarısızsa kaynaktan derleme
# talimatı verilir.
FOXDPI_BYPASS_STATE="$STATE_DIR/foxdpi-bypass"
$SUDO mkdir -p "$FOXDPI_BYPASS_STATE"
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
LOCAL_BIN="$SCRIPT_DIR/bypass/target/release/foxdpi-bypass"
FOXDPI_BYPASS_URL="${FOXDPI_BYPASS_URL:-https://github.com/ilkaycan/foxdpi/releases/latest/download/foxdpi-bypass-x86_64}"
if [ -f "$LOCAL_BIN" ]; then
  $SUDO cp "$LOCAL_BIN" "$BIN_DIR/foxdpi-bypass"
  $SUDO chmod +x "$BIN_DIR/foxdpi-bypass"
  echo "==> foxdpi-bypass (Rust motoru) kuruldu: $BIN_DIR/foxdpi-bypass"
else
  echo "==> Yerel derlenmiş ikili yok; release URL deneniyor: $FOXDPI_BYPASS_URL"
  if command -v curl >/dev/null 2>&1 && curl -fsSL --max-time 60 "$FOXDPI_BYPASS_URL" -o "$FOXDPI_BYPASS_STATE/foxdpi-bypass" && [ -s "$FOXDPI_BYPASS_STATE/foxdpi-bypass" ]; then
    $SUDO cp "$FOXDPI_BYPASS_STATE/foxdpi-bypass" "$BIN_DIR/foxdpi-bypass"
    $SUDO chmod +x "$BIN_DIR/foxdpi-bypass"
    echo "==> foxdpi-bypass (Rust motoru) indirildi ve kuruldu: $BIN_DIR/foxdpi-bypass"
  else
    echo "==> foxdpi-bypass ikili dosyası kurulamadı; atlandı." >&2
    echo "    Kaynaktan derleyin: (cd bypass && cargo build --release)" >&2
    echo "    Ardından: cp bypass/target/release/foxdpi-bypass $BIN_DIR/foxdpi-bypass" >&2
  fi
fi

# foxdpi-gui (Python/tkinter grafiksel arayüz)
GUI_SRC="$SCRIPT_DIR/foxdpi-gui.py"
if [ -f "$GUI_SRC" ]; then
  if command -v python3 >/dev/null 2>&1 && python3 -c "import tkinter" 2>/dev/null; then
    $SUDO cp "$GUI_SRC" "$BIN_DIR/foxdpi-gui"
    $SUDO chmod +x "$BIN_DIR/foxdpi-gui"
    echo "==> foxdpi-gui kuruldu: $BIN_DIR/foxdpi-gui"
  else
    echo "==> Uyarı: python3/tkinter bulunamadı; foxdpi-gui atlandı." >&2
  fi
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

# sistem proxy ayarları (GNOME/KDE/XFCE/LXQt PAC entegrasyonu henüz yapılmadı)
SYSTEM_PROXY_CONF="$HOME/.config/foxdpi/90-foxdpi-proxy.conf"
mkdir -p "$(dirname "$SYSTEM_PROXY_CONF")"
if [ ! -f "$SYSTEM_PROXY_CONF" ]; then
  cat > "$SYSTEM_PROXY_CONF" <<'PROXY_EOF'
# foxdpi-bypass sistem proxy ayarları (GNOME/KDE PAC entegrasyonu henüz yapılmadı)
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
echo "  - foxdpi-gui             # grafiksel arayüz (Python/tkinter)"
echo "  - foxdpi-bypass --mode record-split --port 10850   # DPI bypass motorunu çalıştır"
echo "  - foxdpi install <dosya> # paket kur"
echo "  - foxdpi uninstall      # kurulu paketleri kaldır"
echo "  - foxdpi --help         # tüm komutlar"
