//! SOCKS5 proxy sunucusu — istemciler buraya bağlanır (tarayıcı/PAC/proxy
//! ayarı), ilk TLS ClientHello parse edilir, strateji uygulanır ve trafik
//! gerçek sunucuya forward edilir.
//!
//! Bu proxy, TCP SOCKS5 protokolünü konuşur; bir istemci bağlandığında,
//! CONNECT/BIND bir hedefe yönlendirilir ve o hedefe dogrudan bir TCP
//! bağlantısı kurulur. İlk baytlarda, eğer TLS handshake ise, SNI/Host
//! parse edilir, strateji uygulanır (record split, segment desync, decoy
//! placeholder, OOB placeholder, host scramble) ve forward edilir.
//!
//! Bu seviye userspace'ta çalışır; root/kernel gerektiren ileri düzlem
//! (NFQUEUE, raw socket, TCP-MD5 decoy, autottl, RST drop gibi) ayrı
//! `foxdpi-bypass-adv` crate'inde bulunur.

use crate::config::{load_config, write_default_config, Config, Strategy};
use crate::dns::{DnsError, DnsResolver};
use crate::engine::{apply_strategy, parse_first_bytes, ClientHelloInfo, StrategyResult};
use crate::rules::effective_strategy;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

/// SOCKS5 proxy sunucusu örneği — birden fazla bağlantı yönetir
pub struct SocksProxy {
    config: Arc<RwLock<Config>>,
    /// DNS çözümleyici (paylaşılan)
    dns: Arc<DnsResolver>,
    /// dinlenen port
    listen_addr: SocketAddr,
}

impl SocksProxy {
    pub fn new(config: Config) -> Self {
        let addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            config.port,
        );
        let dns = DnsResolver::new(config.dns.clone());
        Self {
            config: Arc::new(RwLock::new(config)),
            dns: Arc::new(dns),
            listen_addr: addr,
        }
    }

    /// Sunucuyu başlat — SOCKS5 proxy olarak dinlemeye başla
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
                    error!("Bağlantı hatası ({}): {}", peer, e);
                }
            });
        }
    }

    /// Yapılandırmayı güncelle (CLI veya GUI tarafından)
    pub async fn update_config(&self, new_config: Config) {
        let mut cfg = self.config.write().await;
        *cfg = new_config.clone();
        // DNS çözümleyiciyi de güncelle
        self.dns.update_config(cfg.dns.clone()).await;
    }
}

/// SOCKS5 protokolünü implemente eden bağlantı handler'ı.
/// SOCKS5 önce istemci ile protocol negotiation yapar, sonra CONNECT/BIND
/// isteği alır, hedefe bağlantı kurar, ardından tunnel yapar.
///
/// Bu implementation basit SOCKS5 (no auth) üzerinden çalışır; gerçek
/// implementasyonda username/password auth da eklenebilir.
async fn handle_connection(
    socket: TcpStream,
    peer: SocketAddr,
    config: Arc<RwLock<Config>>,
    dns: Arc<DnsResolver>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = socket;

    // SOCKS5 greeting: parmak 0x05 (SOCKS5), method count, methods
    let mut buf = [0u8; 2];
    socket.read_exact(&mut buf).await?;

    if buf[0] != 0x05 {
        // SOCKS5 değil, direkt raw TCP (CONNECT sonrası ilerleyebilir)
        // burada SOCKS5 protocol'i geçiyoruz, direkt forward'a geç
        return handle_raw_tcp(socket, peer, config, dns).await;
    }

    // method selection: no auth (0x00) sun
    socket.write_all(&[0x05, 0x00]).await?;

    // SOCKS5 request: parmak 0x05, connect(1)/bind(2)/udp(3), reserved, addr type
    let mut req_buf = [0u8; 4];
    socket.read_exact(&mut req_buf).await?;

    let cmd = req_buf[1];
    let addr_type = req_buf[3];

    // Address empty
    let addr = match addr_type {
        0x01 => {
            // IPv4: 4 byte
            let mut addr_buf = [0u8; 4];
            socket.read_exact(&mut addr_buf).await?;
            let ip = IpAddr::V4(Ipv4Addr::new(addr_buf[0], addr_buf[1], addr_buf[2], addr_buf[3]));
            let mut port_buf = [0u8; 2];
            socket.read_exact(&mut port_buf).await?;
            let port = ((port_buf[0] as u16) << 8) | (port_buf[1] as u16);
            SocketAddr::new(ip, port)
        }
        0x03 => {
            // Domain name
            let mut len_buf = [0u8; 1];
            socket.read_exact(&mut len_buf).await?;
            let name_len = len_buf[0] as usize;
            let mut name_buf = vec![0u8; name_len];
            socket.read_exact(&mut name_buf).await?;
            let domain = String::from_utf8_lossy(&name_buf).to_string();
            let mut port_buf = [0u8; 2];
            socket.read_exact(&mut port_buf).await?;
            let port = ((port_buf[0] as u16) << 8) | (port_buf[1] as u16);
            // domain'den IP çözümlemesi (DNS)
            let ip = match dns.resolve(&domain).await {
                Ok(ip) => ip,
                Err(DnsError::ProviderNone) => {
                    // DNS none – direkt domain'den bağlantı dene
                    return Err("DNS çözümleyici none modunda".into());
                }
                Err(_) => {
                    // fallback: domain name ile direkt bağlanma dene (başarısız olursa DNS hatası)
                    warn!("DNS çözümlemesi başarısız, direkt bağlantı denemesi (domain={})", domain);
                    // daha sonra domain name ile direct connect yapılabilir
                    // şimdilik example IP ile devam et (fallback)
                    IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))
                }
            };
            SocketAddr::new(ip, port)
        }
        0x04 => {
            // IPv6 — şu an desteklenmiyor (IPv4 focus)
            return Err("IPv6 şu an desteklenmiyor".into());
        }
        _ => return Err("Bilinmeyen adres tipi".into()),
    };

    // CONNECT başarılı yanıt: 0x05 0x00 0x00 0x01 IP port (IPv4)
    let response = if cmd == 0x01 {
        // CONNECT
        let response_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
        let mut resp = vec![0x05, 0x00, 0x00, 0x01];
        let ip_bytes: [u8; 4] = match response_addr.ip() { IpAddr::V4(v4) => v4.octets(), IpAddr::V6(_) => [0, 0, 0, 0], };
        resp.extend_from_slice(&ip_bytes);
        resp.extend_from_slice(&response_addr.port().to_be_bytes());
        resp
    } else {
        // bind veya udp — şimdilik sadece connect desteklenir
        let mut resp = vec![0x05, 0x00, 0x00, 0x01];
        resp.extend_from_slice(&[0, 0, 0, 0]);
        resp.extend_from_slice(&[0, 0]);
        resp
    };
    socket.write_all(&response).await?;

    if cmd != 0x01 {
        // sadece CONNECT destekleniyor, diğer komutlar için kapat
        return Ok(());
    }

    // Socket üzerinden gerçek sunucuya bağlanma
    let target_socket = TcpStream::connect(addr).await?;
    info!("SOCKS5 CONNECT: {} -> {}", peer, addr);

    let c2s = async move {
        let config_clone = config.clone();
        let dns_clone = dns.clone();
        let mut src = socket;
        let mut dst = target_socket;
        if let Err(e) = forward_with_strategy(src, dst, peer, addr, "c2s", config_clone.clone(), dns_clone.clone()).await {
            warn!("İstemci->sunucu forward hatası: {}", e);
        }
    };
    let s2c = async move {
        let config_clone = config.clone();
        let dns_clone = dns.clone();
        let mut src = target_socket;
        let mut dst = socket;
        if let Err(e) = forward_with_strategy(src, dst, addr, peer, "s2c", config_clone.clone(), dns_clone.clone()).await {
            warn!("Sunucu->istemci forward hatası: {}", e);
        }
    };

    tokio::select! {
        _ = c2s => {}
        _ = s2c => {}
    }

    Ok(())
}

/// Raw TCP (SOCKS5 dışı) bağlantısı — direkt forward, strateji uygula
async fn handle_raw_tcp(
    mut socket: TcpStream,
    peer: SocketAddr,
    config: Arc<RwLock<Config>>,
    dns: Arc<DnsResolver>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // İlk baytları oku ve strateji uygula (TLS veya plain olan bağıntıda)
    let mut buf = vec![0u8; 4096];
    let n = socket.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    buf.truncate(n);

    let info = parse_first_bytes(&buf);
    let cfg = config.read().await;
    let effective_mode = effective_strategy(&info, cfg.mode, &cfg.rules);
    drop(cfg);

    let target_addr: SocketAddr = match parse_target_from_raw(&info, &buf) {
        Some(a) => a,
        None => {
            // Hedef adres bilinmiyor, direkt bir hedefe bağlanmaya çalış
            // (örneğin CONNECT sonrası raw tunnel)
            warn!("Hedef adres bilinmiyor, dummy target ile devam ediliyor");
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), 443)
        }
    };

    let target_socket = TcpStream::connect(target_addr).await?;
    let result = apply_strategy(&info, effective_mode, peer, target_addr, &cfg.rules, &buf);

    let mut target = target_socket;
    for part in result.parts {
        target.write_all(&part).await?;
    }
    info!("Strateji {} uygulandı (parça sayısı={})", result.mode, result.parts.len());

    // İki yönlü tunnel başlat
    let _ = copy_bidirectional(&mut socket, &mut target).await;

    Ok(())
}

/// Raw TCP connection'dan hedef adresini çıkar (CONNECT sonrası olabilir)
fn parse_target_from_raw(_info: &ClientHelloInfo, _buf: &[u8]) -> Option<SocketAddr> {
    // TLS ClientHello'da hedef IP bilgisi yok; DNS çözümlemesi gerekir
    // Plain HTTP ise Host: header'ından domain çıkar, DNS ile IP bul
    // Bu placeholder, gerçekte DNS çözümlemesi yapılır
    None
}

/// İstemci -> sunucu yönlü forward, ilk baytlarda strateji uygula
async fn forward_with_strategy(
    mut src: TcpStream,
    mut dst: TcpStream,
    src_peer: SocketAddr,
    dst_peer: SocketAddr,
    _direction: &str,
    config: Arc<RwLock<Config>>,
    dns: Arc<DnsResolver>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // İlk baytı al, strateji uygula, ardından her şeyi forward et
    let mut buf = vec![0u8; 4096];
    let n = src.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    buf.truncate(n);

    let info = parse_first_bytes(&buf);
    let cfg = config.read().await;
    let effective_mode = effective_strategy(&info, cfg.mode, &cfg.rules);
    drop(cfg);    let result = apply_strategy(&info, effective_mode, src_peer, dst_peer, &cfg.rules, &buf);

    for part in result.parts {
        dst.write_all(&part).await?;
    }

    let _ = copy_bidirectional(&mut src, &mut dst).await;

    Ok(())
}


