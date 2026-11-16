# foxdpi-bypass — FoxDPI'nin kendi DPI bypass motoru

Yerel SOCKS5 proxy olarak çalışır; ilk TLS ClientHello / HTTP Host baytlarını
parse edip SNI tabanlı sansürleri atlatacak biçimde paketleri yeniden düzenler.
Kendi DoH tabanlı DNS çözücümüzle domain hedeflerini çözer.

> **Dürüst durum raporu (2026-11):** Motor derlenir, 17 birim testi geçer ve
> yerel SOCKS5 proxy üzerinden uçtan uca doğrulanmıştır (HTTPS 200 e2e).
> decoy / disorder (fake-split) ham socket gerektirdiği için **root veya
> CAP_NET_RAW** ister; yoksa uyarı basılır ve sıradan akışla devam edilir.
>
> DNS varsayılan olarak **DoH (RFC 8484, sertifika doğrulamalı)** ile çözülür;
> DoH kullanılamazsa builtin DoH havuzu, o da olmazsa TCP-53'e düşer.

## Ne çalışıyor

- **SOCKS5 proxy** — no-auth, yalnızca CONNECT; IPv4 ve IPv6 hedefler
- **DoH DNS** (varsayılan) — RFC 8484, rustls + webpki-roots ile doğrulamalı TLS,
  HTTP/1.1 POST `/dns-query`. Builtin havuz: `cloudflare-dns.com` (1.1.1.1),
  `dns.google` (8.8.8.8), `dns.quad9.net` (9.9.9.9). Kullanıcı DoH: `--doh-server URL`.
  Olmazsa TCP-53 builtin (1.1.1.1 / 8.8.8.8 / 9.9.9.9 / 208.67.222.222) yedeği.
- **Reklam engelleme — 4 mod** (`--adblock off|nullroute|deny|sinkhole`):
  - `nullroute` — engelli domain 0.0.0.0'a çözülür (DNS seviyesi, tarayıcı bağlamaz)
  - `deny` — SOCKS5 aşamasında `0x05 connection refused` ile reddeder
  - `sinkhole` — engelli domain `127.0.0.1:8899` (mini HTTP 204 No Content sunucusu)
  - TLS'siz siteler (ör. düz HTTP reklam origin'leri) için 204; HTTPS reklamlarında
    bağlantı sinkhole'a yapılır, 204/TLS uyumsuzluğu istemci tarafında hata verir —
    yine de reklam yüklenmez.
- **Blocklist** — yerleşik ~40 izleyici/reklam domain'i (`--block-ads` → nullroute);
  alt-alanları da kapsar (`doubleclick.net` ⇒ `ads.doubleclick.net`);
  harici hosts-listesi: `--blocklist-file hosts.txt` (`#` yorumları atlanır)
- **TLS ClientHello parse** — absolute-offset SNI konumu (`engine::parse_first_bytes`)
- **record-split** — tek ClientHello kaydını SNI baytının önünde iki TLS kaydına böler
  (`--split-offset N` ile ofseti elle verebilirsiniz)
- **segment-desync** — yükü küçük TCP parçalarına ayırıp peş peşe yollar
  (TCP_NODELAY; `--desync-delay MS` ile parça arası gecikme)
- **disorder / fake-split** — gerçek baytların düşük-TTL kopyasını sahte paket
  olarak ham socket ile gönderir (routers'lar düşürür, DPI şaşırır; `--autottl`,
  `--autottl-max N`). **Root/CAP_NET_RAW gerekir.**
- **decoy** — sahte SNI'li (aynı uzunlukta) sahte ClientHello'yu düşük-TTL ile yollar;
  ardından gerçek ClientHello gider. **Root/CAP_NET_RAW gerekir.**
- **host-scramble** — düz HTTP `Host:` başlık adı ve değerinin harf case'ini ters çevirir
- **Kurallar** — `--rule domain[:strateji|:exempt]`, wildcard `*.ornek.com`
- **exit codes** — `--show` (YAML), `--dns-test <domain>`

## Ne çalışmıyor / sınırlamalar

- **decoy/disorder root ister** — ham socket yoksa EPERM alır, uyarı basılır ve
  sıradan (sahte paketsiz) akışla devam eder (bağlantı kopmaz).
- **OOB** — MSG_OOB urgent bayt gönderilir ama gerçek ağlarda bağlantı kopar;
  deneysel olarak duruyor, **üretimde kullanmayın**.
- **sinkhole 204 sadece HTTP** — HTTPS reklamları için 204 dönmez; bağlantı yine
  de gerçek sunucuya gitmez (ör. tarayıcı hata ekranı).
- **VPN-seviye bypass değil** — yalnızca TLS/HTTP'deki SNI ve Host tabanlı
  engellemelere karşı; IP bazlı engellerde işe yaramaz.
- **sistem proxy/PAC kurulumu yok** — tarayıcıyı veya `http_proxy`'i elle
  127.0.0.1:10850'ye yönlendirin.

## Derleme ve çalıştırma

```bash
cd bypass
cargo build --release          # çıktı: target/release/foxdpi-bypass
cargo test                     # 17/17 geçer (release'de de)

./target/release/foxdpi-bypass                       # 127.0.0.1:1080 (config.yaml varsa 10850)
./target/release/foxdpi-bypass --mode record-split --port 10850
./target/release/foxdpi-bypass --mode disorder --autottl      # root ister
./target/release/foxdpi-bypass --rule youtube.com:record-split --rule banka.com.tr:exempt
./target/release/foxdpi-bypass --block-ads                    # reklam engeli: nullroute + builtin liste
./target/release/foxdpi-bypass --adblock deny                 # SOCKS5 refused
./target/release/foxdpi-bypass --adblock sinkhole             # 127.0.0.1:8899 → HTTP 204
./target/release/foxdpi-bypass --blocklist-file ~/blocklist.txt --adblock nullroute
./target/release/foxdpi-bypass --dns-test example.com
./target/release/foxdpi-bypass --show
```

Tarayıcı: SOCKS5 proxy `127.0.0.1:10850` (domain resolve etme açık olmalı;
`curl --socks5-hostname` de aynı şeyi yapar).

```bash
curl --socks5-hostname 127.0.0.1:10850 https://example.com
```

## Config (`config.yaml`)

```yaml
mode: default            # default | record_split | segment_desync | host_scramble | oob | decoy | disorder
port: 10850
split_offset: null       # record-split ofseti (boyunca); null → SNI ofseti
decoy_ttl: 3
autottl: false
autottl_max: 6
desync_delay_ms: 0
rules:
  - domain: youtube.com
    strategy: record_split
  - domain: banka.com.tr
    exempt: true
dns:
  provider: doh          # doh (varsayılan) | builtin | custom | none
  custom_servers: []     # ["1.1.1.1", "8.8.8.8"] (builtin/custom TCP-53)
  doh_server: null       # örn. https://1.1.1.1/dns-query
  encrypted: true
  block_ads: false
  adblock: off           # off | nullroute | deny | sinkhole
  block_pages: []        # --block-ads ile yerleşik liste eklenir
  blocklist_file: null   # hosts-formatlı harici liste
  sinkhole_port: 8899
```

## Testler

17 birim testi: HTTP host parse, case-insensitive host, host-scramble (test beklentisi
`hOST: eXAMPLE.COM`), TLS ClientHello SNI absolute-offset, record-split roundtrip,
strateji default no-op, DNS encode/skip-name, blocklist wildcard + alt-alan eşleşme +
dosya yükleme, DoH dechunk/URL/parse yardımcıları, ham paket checksum (RFC 1071 vektörü),
paket düzeni, fake_client_hello determinizmi.

## Yol haritası

1. NFQUEUE/AF_PACKET tabanlı daha güçlü decoy (opsiyonel, root)
2. DoT (RFC 7858) çözücü + birim testli HTTPS sinkhole (204/301)
3. sistem proxy/PAC otomasyonu (GNOME/KDE/XFCE) ve foxdpi-gui entegrasyonu
4. paketleme (deb/rpm/AppImage) — `../Makefile` `bypass-rust` target'ı binary'i repo köküne kopyalar
