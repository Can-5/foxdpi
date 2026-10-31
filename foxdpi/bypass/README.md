# foxdpi-bypass (Python şablon paketi)

Bu paket şablon aşamasında; gerçek FoxDPI bypass motoru ayrı bir
geliştirme.ekipleri tarafından tamamlanacaktır.

Şu an sadece sürüm ve CLI entry sağlar.

## Kurulum

```bash
pip install foxdpi-bypass
```

veya kaynakçadan:

```bash
git clone https://github.com/ilkaycan/foxdpi
cd foxdpi/foxdpi/bypass
pip install -e .
```

## Kullanım (şablon)

```bash
foxdpi-bypass
```

Şu an sadece şablon placeholder çıktısı verir; gerçek motor
foxdpi-bypass Rust binary'si veya Node.js şablonu ile çalıştırılmalıdır.

## Linux dağıtımları desteği

- **deb**: Debian, Ubuntu, Pop!_OS ve benzerleri
- **rpm**: Fedora, RHEL, CentOS, openSUSE ve benzerleri
- **appimage**: dağıtım-bağımsız, her yerde çalışabilir
- **tar.gz**: manuel kurulum, her yerde
- **Arch (AUR/package)**: Arch, CachyOS, EndeavourOS ve benzerleri
- **script (install.sh)**: curl-sh tarzı, her yerde çalışır

Her dağıtım için uygun paket oluşturulacak; kullanıcıya kolay kurulum
ve denetim sunulacak.

## Not

Bu paket şablon aşamasında; gerçek implementasyon ayrı bir
geliştirme.ekipleri tarafından tamamlanacaktır.
