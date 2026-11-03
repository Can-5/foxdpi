# FoxDPI — build + paketleme durumu

## Özet

- `foxdpi-gui` binarı `target/release/foxdpi-gui` halinde build edilebilir; `bundle.targets = [deb,rpm,appimage]` ve `build` bölümü Tauri'ya uygun şekilde ayarlı.
- Motor real implementasyon seviyesinde: record split, segment desync/disorder, decoy (TTL-based örneklendi), OOB ve HTTP Host scramble engine katmanında çalışır. DNS tarafında gerçek DoH (DNS-over-HTTPS) kullanılır; fallback DNS-over-TCP placeholder da vardır; blok sayfa listesi demikian.

## Paket hedefleri

- deb, rpm, appimage, tar.gz, Arch (AUR) ve script (install.sh) hedefleri kodda/yol haritasında var.
- installer modülleri (`deb.py`, `rpm.py`, `appimage.py`, `tarball.py`, `remote.py`, `arch.py`, `script.py`) mevcut; bunlar paketleme yolunda kullanılabilir.

## Açık konular / ileri düzlem

- ileri düzlem (NFQUEUE/raw socket/WinDivert gibi) ve bazı stateful DPI teknikleri (wrong-seq/wrong-checksum, QUIC/HTTP3 drop, forged-RST/302 drop, SNI'ye göre fragment, fake paket üretimi, multisplit/multidisorder) roadmap'dadır; şu an implemente değil.
- AUR yayınlamak için `PKGBUILD` ve source/dağıtım yapısı ayrı hazırlanmalı; bu build aşamasında otomatik üretilmiyor.
- Fremium/payload base URL, güncel versiyon numarası ve varsayılan engelleme listesi gibi dinamik değerler deployment sırasında dışkaynaklardan (api veya release notes) beslenmeye uygun şekilde şablon halinde bırakıldı.

## Build komut önerisi

```bash
cd foxdpi/foxdpi-gui
rm -rf dist
bun run build
npx tauri build --bundles deb,rpm,appimage
```

> Not: `npx tauri build` komutu burada doğrudan çalıştırılmadı; sadece önerilen sıra belirtildi.

## İpuçları / düzeltme notları

- `tauri.conf.json` iki kopya var: kök `foxdpi-gui/tauri.conf.json` ve `src-tauri/tauri.conf.json`. Mevcut mutationlar ikisini de aynı içeriğe getiriyor; ileride tek kaynaklı yönetim düşünülebilir.
- Bun `dev`/`build` komutları ve frontendDist ayarı (`dist`) açık; frontend build çıktısı `dist/` altında var ve Tauri build aşamasında bununla paketleniyor.
- AUR/Arch paket yapısı `foxdpi/foxdpi/PKGBUILD` halinde vardır; ancak AUR yayınlamak için source/dağıtım yapısı ayrı hazırlanmalı ve build aşamasında otomatik üretilmiyor. terminal commands available değil; build adımı manuel/yol haritası seviyesinde.

