## Kendi DPI bypass motorumuz (foxdpi-bypass)

FoxDPI'nin kendi bypass motoru **foxdpi-bypass**'tır. Şu anki versiyon şablon aşamasında;
gerçek bypass motoru ayrı bir geliştirme.ekipler tarafından üretilecektir.

### Özellikler (tavsiye edilen yol)

- Yerel bir proxy (SOCKS5/HTTP) üzerinden çalışan, Linux dağıtımlarında
  (Debian, RPM, Arch, vb.) çalışabilecek küçük bir motor.
- İlk TCP/TLS ClientHello paketini inceleyip, SNI/Host bilgisini
  DPI'nin okumasız önce biçimlendirir (record split, segment desync, decoy…).
- Sistem proxy/PAC ayarlarını yönetir, farklı masaüstleri ve dağıtımlar
  için uygun ayarlar yapar.
- Şeffaf, güvenli, denetimli: loglar, doğrulama, kural seti, kullanıcı onayı.

### Kurulum (install.sh)

```bash
curl -fsSL https://.../install.sh | sh
curl -fsSL https://.../install.sh | sh -s -- --system
curl -fsSL https://.../install.sh | sh -s -- --prefix $HOME/.local
```

### Kullanım (şablon aşamasında)

```bash
foxdpi-bypass           # motoru çalıştır (varsayılan mod)
FOXDPI_BYPASS_MODE=split foxdpi-bypass
```

### Paket biçimleri

- deb, rpm, appimage, tar.gz, Arch (AUR) ve script (install.sh) ile
  kurulabilir. Her dağıtım kendi paket yönetimiyle (apt, dnf, pacman, vs.)
  uyumlu olmalıdır.

### Not

Bu motor şu an **şablon** aşamasında; gerçek bypass teknikleri,
Linux dağıtımlarına uygun paketleme ve güvenli daemon yönetimi ayrı
bir geliştirme.ekipleri tarafından tamamlanacaktır.

### Kararlar

- Kendi motorumuzu yazacağız; Shroudly/GoodbyeDPI gibi çalışan ama
  farklı bir, Linux-dağıtımları-genelinde çalışan ve her dağıtımda
  kurulabilen motor.
- Node.js + C yardımcı yazılımlarla (varsa) veya tamamen C/Go ile
  yazılabilir; şablon阶段ında Node.js ile başlıyoruz.
- Sistem proxy/PAC ayarlarını yönetecek; Linux dağıtımları için
  GNOME, KDE, XFCE, LXQt ve diğer masaüstleri desteklenmeli.
