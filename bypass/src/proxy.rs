//! SOCKS5 proxy — istemciler buraya bağlanır (tarayıcı/sistem proxy ayarı);
//! ilk TLS ClientHello / HTTP Host parse edilir, strateji uygulanır ve
//! trafik gerçek sunucuya forward edilir.
//!
//! TCP-level düz bir proxy'dir. decoy / disorder (fake-split) stratejileri
//! raw socket ile düşük-TTL sahte paket gönderir; raw socket için root veya
//! CAP_NET_RAW gerekir — yoksa EPERM hatası alınır, uyarı basılır ve
//! gerçek akış normal gönderilir (bağlantı kopmaz).
//!
//! Reklam engelleme (AdBlockMode):
//!   nullroute — DNS çözümleyici 0.0.0.0 döndürür (bkz. dns.rs)
//!   deny      — SOCKS5 aşamasında 0x05 (connection refused) ile reddeder
//!   sinkhole  — 127.0.0.1:sinkhole_port (204 No Content dönen mini HTTP sunucu)

use crate::config::{AdBlockMode, Config, Strategy};
use crate::dns::{DnsError, DnsResolver};
use crate::engine::{apply_strategy, parse_first_bytes};
use crate::rules::effective_strategy;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::AsRawFd;
use std::sync::Arc;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub struct SocksProxy {
    config: Arc<RwLock<Config>>,
    dns: Arc<DnsResolver>,
    listen_addr: SocketAddr,
}

impl SocksProxy {
    pub fn new(config: Config) -> Self {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), config.port);
        let dns = DnsResolver::new(config.dns.clone());
        Self { config: Arc::new(RwLock::new(config)), dns: Arc::new(dns), listen_addr: addr }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        info!("FoxDPI SOCKS5 proxy dinleniyor: {}", self.listen_addr);

        loop {
            let (socket, peer) = listener.accept().await?;
            debug!("Yeni SOCKS5 bağlantısı: {}", peer);
            let cfg = self.config.clone();
            let dns = self.dns.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, peer, cfg, dns).await {
                    debug!("Bağlantı kapandı ({}): {}", peer, e);
                }
            });
        }
    }

    pub async fn update_config(&self, new_config: Config) {
        let mut cfg = self.config.write().await;
        *cfg = new_config;
        self.dns.update_config(cfg.dns.clone()).await;
    }
}

/// Domain + SNI/Host bilgisine göre reklam engeli kontrolü.
/// Deny ise `true` döner (çağıran bağlantıyı reddeder).
async fn adblock_denied(dns: &DnsResolver, cfg: &Config, host: &str) -> bool {
    cfg.dns.adblock == AdBlockMode::Deny && dns.is_blocked(host).await
}

async fn handle_connection(
    mut socket: TcpStream,
    peer: SocketAddr,
    config: Arc<RwLock<Config>>,
    dns: Arc<DnsResolver>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // --- SOCKS5 greeting ---
    let mut hdr = [0u8; 2];
    socket.read_exact(&mut hdr).await?;
    if hdr[0] != 0x05 {
        return Err("SOCKS5 dışında bağlantı kabul edilmiyor (proxy sadece SOCKS5 konuşur)".into());
    }
    let nmethods = hdr[1] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;
    socket.write_all(&[0x05, 0x00]).await?; // no-auth

    // --- SOCKS5 request ---
    let mut req = [0u8; 4];
    socket.read_exact(&mut req).await?;
    let (cmd, atyp) = (req[1], req[3]);

    let cfg = config.read().await.clone();

    let target: SocketAddr = match atyp {
        0x01 => {
            let mut b = [0u8; 4];
            socket.read_exact(&mut b).await?;
            let port = read_port(&mut socket).await?;
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(b[0], b[1], b[2], b[3])), port)
        }
        0x03 => {
            let mut len_b = [0u8; 1];
            socket.read_exact(&mut len_b).await?;
            let mut name = vec![0u8; len_b[0] as usize];
            socket.read_exact(&mut name).await?;
            let domain = String::from_utf8_lossy(&name).to_string();
            let port = read_port(&mut socket).await?;

            // Reklam engeli: deny → reddet; sinkhole → yerel sunucu
            if adblock_denied(&dns, &cfg, &domain).await {
                write_reply(&mut socket, 0x05).await?; // connection refused
                info!("adblock deny: {} (SOCKS5 refused)", domain);
                return Err(format!("adblock deny: {}", domain).into());
            }
            if cfg.dns.adblock == AdBlockMode::Sinkhole && dns.is_blocked(&domain).await {
                let sp = cfg.dns.sinkhole_port;
                info!("adblock sinkhole: {} -> 127.0.0.1:{}", domain, sp);
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), sp)
            } else {
                let ip = match dns.resolve(&domain).await {
                    Ok(ip) => ip,
                    Err(DnsError::ProviderNone) => {
                        write_reply(&mut socket, 0x08).await?;
                        return Err("DNS sağlayıcı 'none'; domain çözümlenemiyor".into());
                    }
                    Err(e) => {
                        write_reply(&mut socket, 0x04).await?; // host unreachable
                        return Err(format!("DNS çözümleme başarısız ({}): {}", domain, e).into());
                    }
                };
                SocketAddr::new(ip, port)
            }
        }
        0x04 => {
            let mut b = [0u8; 16];
            socket.read_exact(&mut b).await?;
            let port = read_port(&mut socket).await?;
            SocketAddr::new(IpAddr::V6(Ipv6Addr::from(b)), port)
        }
        _ => {
            write_reply(&mut socket, 0x01).await?; // general failure
            return Err("bilinmeyen adres tipi".into());
        }
    };

    if cmd != 0x01 {
        write_reply(&mut socket, 0x07).await?; // command not supported
        return Err("sadece CONNECT desteklenir (UDP/BIND yok)".into());
    }

    // --- Hedefe bağlan ---
    let mut remote = match tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(target)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => { write_reply(&mut socket, 0x05).await?; return Err(format!("hedef bağlantısı başarısız ({}): {}", target, e).into()); }
        Err(_) => { write_reply(&mut socket, 0x06).await?; return Err(format!("hedef bağlantı zaman aşımı ({})", target).into()); }
    };
    remote.set_nodelay(true)?;
    socket.set_nodelay(true).ok();
    write_reply(&mut socket, 0x00).await?; // success

    info!("SOCKS5 CONNECT: {} -> {}", peer, target);

    // --- İlk istemci verisini oku, strateji uygula ---
    let mut buf = vec![0u8; 16384];
    let n = match socket.read(&mut buf).await {
        Ok(0) => return Ok(()),
        Ok(n) => n,
        Err(e) => return Err(e.into()),
    };
    buf.truncate(n);

    let info = parse_first_bytes(&buf);

    // Reklam engeli (SNI/Host üzerinden, CONNECT sonrası): deny → bağlantıyı kes
    let host_key = info.sni.clone().or_else(|| info.host.clone());
    if let Some(h) = &host_key {
        if adblock_denied(&dns, &cfg, h).await {
            info!("adblock deny (SNI/Host): {}", h);
            return Err(format!("adblock deny: {}", h).into());
        }
    }

    let strategy = effective_strategy(&info, cfg.mode, &cfg.rules);

    let result = apply_strategy(&info, strategy, peer, target, &cfg.rules, &buf, cfg.split_offset);
    debug!("strateji={} parçalar={}", result.mode, result.parts.len());

    // Raw socket: düşük-TTL sahte paket (decoy / disorder). Root yoksa sessiz düş.
    if let Some(fake) = &result.raw_fake {
        if let Err(e) = crate::raw::send_low_ttl(&remote, fake, cfg.decoy_ttl, cfg.autottl, cfg.autottl_max).await {
            warn!("raw socket gönderilemedi ({}); sıradan akış sürüyor — root/CAP_NET_RAW gerekli", e);
        }
    }

    // OOB: ilk baytı urgent pointer ile gönder
    if let Some(oob) = result.oob_byte {
        let fd = remote.as_raw_fd();
        let one = [oob];
        let sent = unsafe { libc::send(fd, one.as_ptr() as *const libc::c_void, 1, libc::MSG_OOB) };
        if sent != 1 {
            warn!("OOB gönderimi başarısız (fd={}); bayt normal akışta gönderiliyor", fd);
            remote.write_all(&one).await?;
        }
    }

    // Parça parça gönder; --desync-delay ms arayla bekle (segment-desync için)
    for (i, part) in result.parts.iter().enumerate() {
        if result.mode == Strategy::SegmentDesync && i > 0 && cfg.desync_delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(cfg.desync_delay_ms)).await;
        }
        remote.write_all(part).await?;
    }
    remote.flush().await?;

    // --- Kalan trafiği çift yönlü kopyala ---
    let mut socket = socket;
    let _ = copy_bidirectional(&mut socket, &mut remote).await;
    Ok(())
}

/// SOCKS5 hata/başarı cevabı (IPv4-mapped zero addr).
async fn write_reply(socket: &mut TcpStream, rep: u8) -> std::io::Result<()> {
    let resp = [0x05, rep, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
    socket.write_all(&resp).await
}

async fn read_port(socket: &mut TcpStream) -> std::io::Result<u16> {
    let mut port_buf = [0u8; 2];
    socket.read_exact(&mut port_buf).await?;
    Ok(((port_buf[0] as u16) << 8) | port_buf[1] as u16)
}
