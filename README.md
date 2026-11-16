## Kendi DPI bypass motorumuz (foxdpi-bypass)

FoxDPI'nin kendi bypass motoru **foxdpi-bypass**'tır (Rust). Yerel SOCKS5 proxy
olarak çalışır; TLS ClientHello üzerinde SNI konumunu bulup record-split ve
segment-desync uygulayarak SNI tabanlı sansürleri atlatır. Motor şu an
**çalışır ve güçlendirilmiş durumda** (derlenir, 17/17 test geçer, curl ile
uçtan uca doğrulandı; **DoH varsayılan DNS** — rustls + webpki-roots;
**4 reklam engelleme modu** — nullroute / deny / sinkhole / off; disorder ve
autottl ham socket ister); dürüst sınırları aşağıda ve `bypass/README.md`'de
yazılıdır.

### Özellikler (tavsiye edilen yol)

- Yerel bir proxy (SOCKS5/HTTP) üzerinden çalışan, Linux dağıtımlarında
  (Debian, RPM, Arch, vb.) çalışabilecek küçük bir motor.
- İlk TCP/TLS ClientHello paketini inceleyip, SNI/Host bilgisini
  DPI'nin okumasından önce biçimlendirir (record split, segment desync, decoy…).
- Sistem proxy/PAC ayarlarını yönetir, farklı masaüstleri ve dağıtımlar
  için uygun ayarlar yapar.
- Şeffaf, güvenli, denetimli: loglar, doğrulama, kural seti, kullanıcı onayı.

### Kurulum (install.sh)

```bash
curl -fsSL https://.../install.sh | sh
curl -fsSL https://.../install.sh | sh -s -- --system
curl -fsSL https://.../install.sh | sh -s -- --prefix $HOME/.local
```

### Kullanım

```bash
cd bypass && cargo build --release
./target/release/foxdpi-bypass --mode record-split --port 10850   # 127.0.0.1 SOCKS5
# tarayıcı/curl proxy'sini 127.0.0.1:10850 yapın
curl --socks5-hostname 127.0.0.1:10850 https://example.com
foxdpi-bypass --show            # etkin yapılandırma
foxdpi-bypass --dns-test example.com           # DoH ile çözer
foxdpi-bypass --adblock sinkhole               # 4 mod: off|nullroute|deny|sinkhole
foxdpi-bypass --blocklist-file /path/hosts.txt # harici liste
foxdpi-bypass --decoy-ttl 3 --autottl          # ham socket (root/CAP_NET_RAW)
```

### Grafiksel arayüz (foxdpi-gui)

Çalışan GUI **`foxdpi-gui.py`**'dır (Python 3 + tkinter, stdlib — ekstra bağımlılık yok;
KDE/Wayland'de XWayland üzerinden açılır):

```bash
foxdpi-gui           # kuruluysa (install.sh) ya da
python3 foxdpi-gui.py
```

Sekmeler: **Genel** (strateji, port, DNS/DoH, reklam engelleme modu, `--block-ads`
listesi, `--blocklist-file`, sinkhole portu, kural editörü — wildcard + exempt) ve
**Gelişmiş** (split-offset, desync-delay, decoy-ttl, autottl). Başlat/Durdur, canlı
log penceresi (`~/.config/foxdpi/foxdpi-gui.log`), "Bağlantı Testi" (curl ile
discord.com/app'e HTTP kodu), "Discord'u Aç" (ayrı Firefox profili ile), ayarlar
`~/.config/foxdpi/gui-settings.json`. Motor zaten açıksa (örn. 1080) GUI onu
"harici" olarak algılar ve yeniden başlatmadan bağlanır.

### Paket biçimleri

- deb, rpm, appimage, tar.gz, Arch (AUR) ve script (install.sh) ile
  kurulabilir. Her dağıtım kendi paket yönetimiyle (apt, dnf, pacman, vs.)
  uyumlu olmalıdır.

### Not

Motor şu an **userspace seviyesinde** çalışır. DNS varsayılan olarak **DoH**'dur
(rustls + webpki-roots, builtin: cloudflare-dns.com/dns.google/dns.quad9.net;
TCP-53'e düşer; kendi sunucunuz: `--doh-server https://dns.example.com/dns-query`).
**disorder/decoy** ve **autottl** gerçek ham socket gerektirir (Linux, root veya
CAP_NET_RAW); yetmezse uyarı verir ve akış sıradan devam eder. **Reklam engeli**:
`--adblock` ile 4 mod (`off`, `nullroute` = DNS 0.0.0.0, `deny` = SOCKS5 reddi
0x05, `sinkhole` = yerel 204 uç noktası); sinkhole yalnızca düz HTTP'yi 204 yapar,
HTTPS'te bağlantı kurulur ama içerik gelmez. `--blocklist-file` ile hosts-benzeri
harici liste (yorumlar/boş/IP satırları atlanır) desteklenir. Sistem proxy/PAC
otomasyonu henüz yapılmadı. Yol haritası ve dürüst bölüm için `bypass/README.md`.

### Kararlar

- Kendi motorumuzu yazacağız; Shroudly/GoodbyeDPI gibi çalışan ama
  farklı bir, Linux-dağıtımları-genelinde çalışan ve her dağıtımda
  kurulabilen motor. (Karar verildi: Rust, `bypass/` — çalışır durumda.)
- Sistem proxy/PAC ayarlarını yönetecek; Linux dağıtımları için
  GNOME, KDE, XFCE, LXQt ve diğer masaüstleri desteklenmeli (henüz yapılmadı).
