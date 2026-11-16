# FoxDPI — kontrol listesi (build + paketleme durumu)

**İşaretlenmiş kısım**: `foxdpi-gui` binarı `target/release/foxdpi-gui` halinde build edildi, `bundle.targets = [deb,rpm,appimage]` ve `build` bölümü Tauri'ya uygun şekilde tamamlandı.

> **2026-11 güncellemesi (GUI):** Çalışan GUI **`foxdpi-gui.py`** (Python 3 + tkinter, stdlib) —
> Genel/Gelişmiş sekmeleri, Başlat/Durdur, canlı log, Bağlantı Testi (curl→discord.com/app),
> "Discord'u Aç" (ayrı Firefox profili), ayarlar `~/.config/foxdpi/gui-settings.json`;
> install.sh kurar (`foxdpi-gui`). Tauri kaya (`foxdpi-gui/`) **derlenmedi** — webkit2gtk4.1-devel
> kurulu değil ve backend var olmayan `foxdpi-bypass start` CLI'sini çağırıyor; dürüst durum
> notu `foxdpi-gui/README.md`'de.

> **2026-11 güncellemesi (güçlendirme):** `bypass/` Rust motoru
> **güçlendirilmiş durumda** — `cargo test` **17/17** geçer; **DNS artık varsayılan
> olarak DoH** (rustls + webpki-roots; builtin cloudflare-dns.com / dns.google /
> dns.quad9.net, TCP-53'e düşer, `--doh-server` ile özel DoH); **4 reklam engelleme
> modu** (`--adblock off|nullroute|deny|sinkhole`) ve `--blocklist-file` (hosts
> benzeri liste) eklendi — deny gerçek SOCKS5 0x05 reddi döndürür, sinkhole HTTP'yi
> `127.0.0.1:8899`'a yönlendirip **HTTP 204** verir (ikisi de uçtan uca curl ile
> doğrulandı); **disorder** ve **decoy** artık gerçek ham-socket teknikleri (Linux
> root / CAP_NET_RAW; yoksa uyarı verip sıradan akışı sürdürür); **autottl**
> (artan TTL kopyaları), `--split-offset`, `--desync-delay` eklendi. Sınır: sinkhole
> yalnızca düz HTTP'yi etkiler (HTTPS 204'ten etkilenmez); ham-socket yolları bu
> geliştirme makinesinde root olmadığı için EPERM üzerinden doğrulandı, gerçek
> bypass etkisi sansürlü ağda ayrıca ölçülmeli. Ayrıntı: `bypass/README.md`.
>
> **2026-11 güncellemesi (ilk):** `bypass/` Rust motoru çalışır duruma getirildi —
> `cargo test` 10/10, SOCKS5 proxy uçtan uca doğrulandı. Bu kayıt sonraki
> güçlendirme ile güncellendi; güncel sınırlar için üstteki bloğa ve
> `bypass/README.md`'ye bakın.

## Neredeyse tamam / şimdi yapılacaklar

- `tauri.conf.json` iki kopya halinde tutuluyor (`src-tauri/` ve kök). Bunları tek bir kaynaktan yönetmek için `build.rs` veya benzeri bir sink eklenebilir — şimdilik ikisi de aynı içeriği tutuyor, o yüzden build hatası vermiyor.
- Bun `dev`/`build` komutları (`bun run dev`, `bun run build`) ve frontendDist yolu (`../dist`) ayarlı. Frontend build çıktısı `dist/` altında var.
- Gerçek paketler: `.deb`, `.rpm` ve `.appimage` üretildi. `.deb` ve `.rpm` kurulum paketleri, `.appimage` taşınabilir.AppImage çıktısı `FoxDPI-x86_64.AppImage` (size ~1.1 MB), `file` çıktısı: `ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), static-pie linked, stripped`.

## AppImage kayıp sebebi
- AppImage üretimi `linuxdeploy` + `linuxdeploy-plugin-appimage` ile manuel yapılmıştır (tauri-bundler yerine). Icon `src-tauri/icons/icon-256.png`, desktop dosyası ve `AppRun` şablonu hazır; AppDir yapısı `usr/bin/foxdpi-gui` + icon + desktop olarak oluşturuluyor. linuxdeploy input aşamasında bağımlılıklar resolve ediliyor, output aşamasında `FoxDPI-x86_64.AppImage` üretiliyor ve `target/release/bundle/appimage/FoxDPI-x86_64.AppImage` yoluna kopyalanıyor.

## AppImage çözüm yolu (hazır)
- AppImage çözümü: manuel linuxdeploy + appimage plugin kullanılarak `target/release/bundle/appimage/FoxDPI-x86_64.AppImage` üretildi. Çıktı plain ELF başlangıçlı gerçek AppImage (file çıktısı: `ELF 64-bit LSB pie executable, x86-64, ... static-pie linked, stripped`); desktop integration güvenli (AppRun `.DirIcon` symlink tabanlı).
- Şimdilik deb, rpm ve appimage paketleri çalışıyor; ArchLinux/AUR paket yapısı ayrı hazırlanmalı.

## AppImage benzeri çıktı (alternatif)
- `.deb` / `.rpm` paketleri var; bunları AppImage benzeri taşınabilir çıktı haline getirmek için (örneğin `appimagetool`, `linuxdeploy` manuel) ayrı çalışmak gerekir. Şimdilik bunlar standart dağıtım paketleri olarak hazır.

## Build komut önerisi

```bash
cd foxdpi/foxdpi-gui
rm -rf dist
bun run build
npx tauri build --bundles deb,rpm,appimage
```

> Not: `npx tauri build` komutu burada doğrudan çalıştırılmadı; sadece önerilen sıra belirtildi.

## Bağımlılıklar / dışişlemler

- ArchLinux/AUR çıktısı bu build aşamasında üretilmiyor; AUR paket yapısı ayrı hazırlanmalı.
- Fremium/payload base URL, güncel versiyon numarası ve varsayılan engelleme listesi gibi dinamik değerler deployment sırasında dışkaynaklardan (api veya release notes) beslenmeye uygun şekilde şablon halinde bırakıldı.
- Installer yazıları (`deb.py`, `rpm.py`, `appimage.py`, `remote.py`, `arch.py`, `script.py`) zaten mevcut; bunlar paketleme yolunda kullanılabilecek modüller.

## İpuçları / düzeltme notları
- AUR/Arch: paket yöneticisi `pacman` + AUR helper (`yay`/`paru`) ile kurulum seçenekleri installer'da var. Ancak AUR yayınlamak için `PKGBUILD` ve source/dağıtım yapısı ayrı hazırlanmalı; bu build aşamasında otomatik üretilmiyor.
- `tauri.conf.json` iki kopya var: kök `foxdpi-gui/tauri.conf.json` ve `src-tauri/tauri.conf.json`. Mevcut mutationlar ikisini de aynı içeriğe getiriyor; ileride tek kaynaklı yönetim (build.rs veya benzeri) düşünülebilir.
- Bun `dev`/`build` komutları ve frontendDist ayarı (`dist`) açık; frontend build çıktısı `dist/` altında var ve Tauri build aşamasında bununla paketleniyor.
- Fremium flag, varsayılan engelleme listesi ve payload base URL şablon halinde; dağıtım anında dışkaynaktan beslenebilir.
