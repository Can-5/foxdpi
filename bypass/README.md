# foxdpi-bypass — kendi DPI bypass motorumuz

Bu dizin, FoxDPI'nin kendi DPI bypass motoru olarak geliştirilecek
**foxdpi-bypass**'in şablon ve mimari dökümantasyonudur.

## Misyon

Kendi bypass motorumuzu yazarak:
- Linux dağıtımlarında (Debian, RPM, Arch, vb.) çalışabilen,
- her dağıtımda kurulabilen (deb, rpm, appimage, tar.gz, AUR, script),
- kullanıcıya şeffaf, güvenli, denetlenebilir bir araç sunmak.

Shroudly ve GoodbyeDPI gibi araçlardan ilham alıyoruz ama
kendi motorumuz olarak farklı bir yaklaşımla, Linux-dağıtımları-genelinde
çalışacak şekilde tasarlayacağız.

## Mimari şablonu (taslak)

- **Motor katmanı**: düşük seviye TCP/TLS işleme, ilk ClientHello
  parse etme, SNI/Host pozisyonu belirleme.
- **Strateji motoru**: TLS record split, segment desync, decoy,
  HTTP Host scramble gibi teknikleri uygula.
- **Proxy/Forwarder katmanı**: yerel SOCKS5/HTTP proxy veya doğrudan
  TCP forwarder; istemci trafiğini motora yönlendir.
- **Sistem ayarlama katmanı**: Linux sistem proxy/PAC ayarlarını
  yönet, farklı masaüstleri (GNOME, KDE, XFCE, LXQt, vs.) için
  uygun ayarlar yap.
- **Paketlama/Dağıtım katmanı**: deb, rpm, appimage, tar.gz, AUR,
  script (install.sh) ile kurulabilir; her dağıtım kendi paket
  yönetimiyle uyumlu.
- **Güvenlik/Denetim katmanı**: loglar, doğrulama, kural seti,
  kullanıcı onayı, restore (çökmelerde/çıkışlarda eski ayarlar).

## Teknikler (örnek)

- TLS record split (RFC 8446 §5.1) — ClientHello'yu parçala,
  SNI/Host'ü DPI'nin okumasaz önce biçimlendir.
- TCP segment desync / disorder — isim parçalarını farklı segmentlerde
  gönder, filtre yeniden derleyemeyecek.
- Decoy (Linux) — serverin atığı ama filtrenin okuduğu ilk segment.
- Out-of-band (urgent pointer) — bazı filtrelerin görmediği bayt.
- HTTP Host scramble — Host başlığını yeniden biçimlendir.
- Encrypted DNS — DNS yanıtlarını doğrula, DNS pollution'a karşı.

## Linux dağıtımları desteği

- **deb**: Debian, Ubuntu, Pop!_OS ve benzerleri
- **rpm**: Fedora, RHEL, CentOS, openSUSE ve benzerleri
- **appimage**: dağıtım-bağımsız, her yerde çalışabilir
- **tar.gz**: manuel kurulum, her yerde
- **Arch (AUR/package)**: Arch, CachyOS, EndeavourOS ve benzerleri
- **script (install.sh)**: curl-sh tarzı, her yerde çalışır

Her dağıtım için uygun paket oluşturulacak; kullanıcıya kolay kurulum
ve denetim sunulacak.

## Güvenlik ve denetim

- Motor şeffaf olmalı; loglar, doğrulama, kural seti ve kullanıcı onayı ile çalışmalı.
- Sistem ayarlarını geri yükleyecek; çökme/çıkış durumunda eski ayarlar korunmalı.
- Kullanıcıya açık ve denetime açık olmalı; kapalı/kötü amaçlı kullanım önlenmelidir.
- Yasal ve etik sınırlar içinde kalınmalı; jurisdiction ve network politikaları göz önünde bulundurulmalıdır.

## Geliştirme aşamaları (tavsiye)

1. Şablon motoru — Node.js ile basit TCP sunucusu (bu dosya).
2. İlk ClientHello parser katmanı — TLS parse, SNI/Host pozisyonu.
3. Strateji motoru — record split, segment desync, decoy, OOB, Host scramble.
4. Proxy/forwarder — yerel SOCKS5/HTTP veya TCP forwarder.
5. Sistem ayarlama — Linux proxy/PAC, masaüstü entegrasyonu.
6. Paketleme — deb, rpm, appimage, tar.gz, AUR, script.
7. Güvenlik/denetim — log, doğrulama, kural, kullanıcı onayı, restore.

## Not

Bu dosya şablon aşamasında; gerçek motor ayrı bir geliştirme.ekipleri
tarafından tamamlanacaktır. Şablon, mimari kararları ve yol haritasını
yakalamak için burada tutulmuştur.
