## Kendi DPI bypass motorumuz (foxdpi-bypass)

FoxDPI'nin kendi bypass motoru **foxdpi-bypass**'tır. Bu sürümde motor real
implementasyon seviyesine geçmiştir:

- gerçek TLS record split, segment desync/disorder, decoy (TTL-based örneklendi),
  OOB ve HTTP Host scramble teknikleri engine katmanında çalışır.
- SOCKS5 proxy üzerinden çalışır; istemci trafiği motor tarafından işlenir.
- DNS tarafında gerçek DoH (DNS-over-HTTPS) kullanılır; fallback DNS-over-TCP
  placeholder da vardır; blok sayfa listesi dengan.

### Özellikler

- Yerel bir SOCKS5 proxy üzerinden çalışır; Linux dağıtımlarında
  (Debian, RPM, Arch, vb.) çalışabilir.
- İlk TCP/TLS ClientHello paketini parse eder; SNI/Host bilgisi kullanılır.
- Strateji motoru: TLS record split, segment desync/disorder, decoy, OOB,
  HTTP Host scramble gibi teknikleri uygular.
- Sistem proxy/PAC ayarlarını yönetmeye yönelik altyapı vardır; farklı
  masaüstleri ve dağıtımlar için uygun ayarlar yapılabilir.
- Şeffaf, güvenli, denetimli: loglar, doğrulama, kural seti, kullanıcı onayı.

### Kurulum

```bash
curl -fsSL https://.../install.sh | sh
curl -fsSL https://.../install.sh | sh -s -- --system
curl -fsSL https://.../install.sh | sh -s -- --prefix $HOME/.local
```

### Kullanım

```bash
foxdpi-bypass           # motoru çalıştır (varsayılan mod)
FOXDPI_BYPASS_MODE=split foxdpi-bypass
```

### Paket biçimleri

- deb, rpm, appimage, tar.gz, Arch (AUR) ve script (install.sh) ile
  kurulabilir. Her dağıtım kendi paket yönetimiyle (apt, dnf, pacman, vs.)
  uyumlu olmalıdır.

### Not

Bu sürüm real implementasyon seviyesine getirilmiştir; ileri düzlem (NFQUEUE,
raw socket, WinDivert gibi) ve bazı stateful DPI teknikleri roadmap'dadır.

### Kararlar

- Kendi motorumuzu yazdık; Shroudly/GoodbyeDPI gibi araçlardan ilham aldık ama
  kendi motorumuz olarak farklı bir yaklaşımla, Linux-dağıtımları-genelinde
  çalışacak şekilde tasarladık.
- Rust ile yazdık; Node.js veya C/Go değil.
- Sistem proxy/PAC ayarlarını yönetecek; Linux dağıtımları için
  GNOME, KDE, XFCE, LXQt ve diğer masaüstleri desteklenebilir.
