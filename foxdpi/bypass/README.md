# foxdpi-bypass (Python baslaticisi)

FoxDPI'nin kendi DPI bypass motoru **Rust crate'idir** ve repo kokundeki
`bypass/` dizinindedir (yerel SOCKS5 proxy + TLS ClientHello biçimlendirme).

Bu Python paketi **ince bir baslaticidir**: Rust binary'sini bulur
(`FOXDPI_RUST_BIN` veya `bypass/target/release/foxdpi-bypass`) ve argumanlarıyla
çalıştırır. Gerçek bypass mantığı burada değildir.

## Rust motorunu derleme

```bash
cd bypass && cargo build --release    # çıktı: bypass/target/release/foxdpi-bypass
cargo test                            # 10/10
```

## Calistirma

```bash
./bypass/target/release/foxdpi-bypass --mode record-split --port 10850
# veya Python baslaticisi uzerinden:
FOXDPI_RUST_BIN=$PWD/bypass/target/release/foxdpi-bypass python -c "from foxdpi_bypass import main; main()"
```

Tarayicinizi 127.0.0.1:10850 SOCKS5 proxy'sine yonlendirin (domain cozumlemesi acik).

## Dürüst sinirlar

- decoy raw socket gerektirir; `--mode decoy` verilirse uyariyla record-split'e dusulur
- OOB (MSG_OOB) deneyseldir, gercek aglarda kopma yapar
- DNS, TCP 53'e gider; DoH/DoT henüz yok
- Sistem proxy/PAC otomasyonu yok; elle proxy ayari gerekir
- Bu Python paketi setuptools paketleme olarak hatalidir (klasor adinda tire var);
  kurulum icin Rust binary'sini `make bypass-rust` ile repo kokune kopyalayin
