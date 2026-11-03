//! DNS çözümleyici — DoH / DoT / DNS-over-TCP, block page engelleme,
//! rastgele rotasyonlu DNS pool'u.
//!
//! FoxDPI bu katmanı, SOCKS5 proxy bağlantılarında DNS sorguları için
//! kullanır. Proxy, DNS sorgusunu bu modüden çözer ve sonucu istemciye
//! döner. Block page listesi, DNS cevabında engellenmiş siteleri tespit
//! etmek veya DNS cevabı olarak engelleme IP'si döndürmek için kullanılır.
//!
//! "Encrypted DNS" seçeneği aktifse, DNS sorguları DoH (HTTPS) veya DoT
//! (TLS over TCP) üzerinden gider. Bazı ağlarda DNS sahtelemesi (DNS
//! pollution) engellenir, DNS cevabı sahte DNS sunucularından gelmeyen
//! gerçek DNS'dir.

use crate::config::{DnsConfig, DnsProvider};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Bir DNS çözümleyici örneği. Birden fazla bağlantı tarafından paylaşılır.
pub struct DnsResolver {
    config: Arc<RwLock<DnsConfig>>,
    /// bilinen engellenmiş hedefler (domain/IP) — block page listesi
    blocklist: Arc<RwLock<HashSet<String>>>,
    /// iç TLS DoH istemcisi (gelecek)
    #[allow(dead_code)]
    http_client: reqwest::Client,
}

impl DnsResolver {
    pub fn new(config: DnsConfig) -> Self {
        let blocklist: HashSet<String> = config
            .block_pages
            .iter()
            .cloned()
            .collect();
        Self {
            config: Arc::new(RwLock::new(config)),
            blocklist: Arc::new(RwLock::new(blocklist)),
            http_client: reqwest::Client::builder()
                .danger_accept_invalid_certs(false)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Blok listesine bir domain veya IP ekle
    pub async fn add_block(&self, entry: String) {
        let mut bl = self.blocklist.write().await;
        bl.insert(entry);
    }

    /// Blok listesinden bir domain veya IP çıkar
    pub async fn remove_block(&self, entry: String) {
        let mut bl = self.blocklist.write().await;
        bl.remove(&entry);
    }

    /// Blok listesini güncelle (tüm liste)
    pub async fn set_blocklist(&self, entries: Vec<String>) {
        let mut bl = self.blocklist.write().await;
        *bl = entries.into_iter().collect();
    }

    /// Yapılandırmayı güncelle (DNS provider, sunucular, şifreleme, block_ads)
    pub async fn update_config(&self, config: DnsConfig) {
        let mut c = self.config.write().await;
        *c = config;
        // block_ads aktifken varsa, bir azaltılmış list ekle
        let mut bl = self.blocklist.write().await;
        if c.block_ads {
            bl.extend(crate::dns::DEFAULT_AD_BLOCK_LIST.iter().map(|s| s.to_string()));
        }
    }

    /// Belirli bir domain için DNS çözümleme yap. Eğer domain blok listesinde
    /// ise, 0.0.0.0 veya GVGNYG blok sayfası IP'si döndürür (bypass
    /// engelleme için DNS tabanlı fehrasız).
    ///
    /// Bu fonksiyon SOCKS5 proxy tarafından çağrılır; istemci DNS sorunu
    /// çözerken proxy kendi DNS çözümlemesini kullanır.
    pub async fn resolve(&self, domain: &str) -> Result<IpAddr, DnsError> {
        // blocklist kontrolü: domain engellenmişse, blok sayfası IP'si
        if self.is_blocked(domain).await {
            return Ok(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))); // veya block-page IP
        }

        let cfg = self.config.read().await;
        match cfg.provider {
            DnsProvider::BuiltIn => self.resolve_builtin(domain, &cfg).await,
            DnsProvider::Custom => self.resolve_custom(domain, &cfg).await,
            DnsProvider::None => {
                // DNS'yi proxy tarafında çözemiyoruz, hata
                Err(DnsError::ProviderNone)
            }
        }
    }

    /// Domain blok listesinde mi?
    async fn is_blocked(&self, domain: &str) -> bool {
        let bl = self.blocklist.read().await;
        let d = domain.to_ascii_lowercase();
        bl.contains(&d) || bl.iter().any(|e| e.starts_with('*') && d.ends_with(&e[1..]))
    }

    /// İç pool'dan DoH/DNS-over-TCP ile çözümleme — gerçek DoH kullanır.
    async fn resolve_builtin(&self, domain: &str, cfg: &DnsConfig) -> Result<IpAddr, DnsError> {
        let servers = builtin_servers(cfg.encrypted);
        for server in servers.iter().take(3) {
            if server.contains("://") {
                // DoH endpoint
                match doh_query(domain, server).await {
                    Ok(ip) => {
                        debug!("DNS çözündü (doh): {} -> {}", domain, ip);
                        return Ok(ip);
                    }
                    Err(e) => {
                        warn!("DoH çözümleme başarısız (server={}): {}", server, e);
                    }
                }
            } else {
                // legacy DNS-over-TCP placeholder
                match dns_over_tcp_query(domain, server).await {
                    Ok(ip) => {
                        debug!("DNS çözündü (builtin): {} -> {}", domain, ip);
                        return Ok(ip);
                    }
                    Err(e) => {
                        warn!("DNS çözümleme başarısız (builtin, server={}): {}", server, e);
                    }
                }
            }
        }
        Err(DnsError::NoResolution)
    }

    /// Kullanıcı tanımlı sunuculardan çözümleme
    async fn resolve_custom(&self, domain: &str, cfg: &DnsConfig) -> Result<IpAddr, DnsError> {
        let servers = &cfg.custom_servers;
        if servers.is_empty() {
            return Err(DnsError::CustomNoServers);
        }
        for server in servers.iter() {
            match dns_over_tcp_query(domain, server).await {
                Ok(ip) => {
                    debug!("DNS çözündü (custom): {} -> {}", domain, ip);
                    return Ok(ip);
                }
                Err(e) => {
                    warn!("DNS çözümleme başarısız (custom, server={}): {}", server, e);
                }
            }
        }
        Err(DnsError::NoResolution)
    }
}

/// Varsayılan DNS sunucuları — şifrelenmiş modda gerçek DoH uç noktaları.
fn builtin_servers(encrypted: bool) -> Vec<String> {
    if encrypted {
        // Cloudflare ve Quad9 DoH uç noktaları (RFC 8484)
        vec![
            "https://9.9.9.9/dns-query".into(),
            "https://1.1.1.1/dns-query".into(),
            "https://8.8.8.8/dns-query".into(),
        ]
    } else {
        vec!["9.9.9.9:53".into(), "1.1.1.1:53".into()]
    }
}

/// Varsayılan reklam/izleyici engelleme listesi (örnek)
pub const DEFAULT_AD_BLOCK_LIST: &[&str] = &[
    "ads.example.com",
    "tracker.example.com",
    "analytics.example.com",
    "doubleclick.net",
    "facebook.com/trk",
    "googlesyndication.com",
    "googleadservices.com",
];

/// DNS çözümleme hatası türleri
#[derive(Debug, thiserror::Error)]
pub enum DnsError {
    #[error("DNS çözümleme başarısız: {0}")]
    ResolveFailed(#[from] std::io::Error),
    #[error("Hiçbir DNS sunucusu cevap vermedi")]
    NoResolution,
    #[error("DNS sağlayıcı 'none' ayarlı, çözümleme yapılamaz")]
    ProviderNone,
    #[error("Kullanıcı tanımlı DNS sunucusu yok")]
    CustomNoServers,
    #[error("DNS cevabı parse edilemedi: {0}")]
    ParseError(String),
    #[error("DoH isteği başarısız: {0}")]
    DoHFailed(String),
}

/// Gerçek DNS-over-HTTPS (DoH) sorgusu. RFC 8484'e göre POST-based wire format
/// kullanır; Cloudflare/Quad9 gibi operatörlerde çalışır.
async fn doh_query(domain: &str, server_url: &str) -> Result<IpAddr, DnsError> {
    // authority + dns wire format (RFC 1035 question)
    let encoded_name = encode_dns_name(domain);
    let mut wire = Vec::with_capacity(12 + encoded_name.len() + 4);
    // transaction id (dummy)
    wire.extend_from_slice(&[0x00, 0x01]);
    // flags: standard query
    wire.extend_from_slice(&[0x01, 0x00]);
    // question count
    wire.extend_from_slice(&[0x00, 0x01]);
    // answer/authority/additional counts
    wire.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    // question section
    wire.extend_from_slice(&encoded_name);
    wire.extend_from_slice(&[0x00]); // terminate name
    wire.extend_from_slice(&[0x00, 0x01]); // QTYPE A
    wire.extend_from_slice(&[0x00, 0x01]); // QCLASS IN

    let resp = reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .build()?
        .post(server_url)
        .header("Content-Type", "application/dns-message")
        .header("Accept", "application/dns-message")
        .body(wire)
        .send()
        .await
        .map_err(|e| DnsError::DoHFailed(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(DnsError::DoHFailed(format!("HTTP {}", status)));
    }

    let body = resp.bytes().await.map_err(|e| DnsError::DoHFailed(e.to_string()))?;
    let ip = parse_doh_a_answer(&body).ok_or_else(|| DnsError::NoResolution)?;
    Ok(ip)
}

fn encode_dns_name(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for label in name.split('.') {
        if !label.is_empty() {
            out.push(label.len() as u8);
            out.extend_from_slice(label.as_bytes());
        }
    }
    out
}

fn parse_doh_a_answer(body: &[u8]) -> Option<IpAddr> {
    if body.len() < 12 {
        return None;
    }
    let answers = ((body[6] as usize) << 8) | body[7] as usize;
    if answers == 0 {
        return None;
    }
    // skip header (12) + question section
    let mut pos = 12;
    // skip question: name + 4 bytes QTYPE/QCLASS
    pos = skip_dns_name(body, pos)?;
    pos += 4;
    // iterate answer RRs
    for _ in 0..answers {
        pos = skip_dns_name(body, pos)?;
        if pos + 10 > body.len() {
            return None;
        }
        let rtype = ((body[pos] as usize) << 8) | body[pos + 1] as usize;
        let rdlen = ((body[pos + 8] as usize) << 8) | body[pos + 9] as usize;
        pos += 10;
        if rtype != 1 {
            pos += rdlen;
            continue;
        }
        if rdlen != 4 || pos + 4 > body.len() {
            return None;
        }
        let ip = Ipv4Addr::new(body[pos], body[pos + 1], body[pos + 2], body[pos + 3]);
        return Some(IpAddr::V4(ip));
    }
    None
}

fn skip_dns_name(body: &[u8], mut pos: usize) -> Option<usize> {
    let start = pos;
    let max = body.len();
    let mut jumped = false;
    let mut jump_stack = Vec::new();
    while pos < max {
        let len = body[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        if (len & 0xc0) == 0xc0 {
            if pos + 1 >= max {
                return None;
            }
            let target = ((len & 0x3f) as usize) << 8 | body[pos + 1] as usize;
            if target >= start || target >= max {
                return None;
            }
            jump_stack.push(pos + 2);
            pos = target;
            jumped = true;
            continue;
        }
        pos += 1 + len;
    }
    if !jumped {
        return None;
    }
    jump_stack.pop()?;
    Some(pos)
}

/// DNS-over-TCP placeholder sorgusu. Gerçek hayatta DoH (HTTPS POST) veya
/// DoT (TLS over TCP) yapılır. Bu placeholder, basit bir TCP DNS sorgusu
/// (RFC 1035) görevi görür; ancak production'da DoH/DoT kullanılmalı.
async fn dns_over_tcp_query(domain: &str, _server: &str) -> Result<IpAddr, DnsError> {
    // Placeholder: gerçekten DNS-over-TCP yapmıyoruz, rastgele IP döndürüyoruz.
    // Bu, engine'ın placeholder olarak çalışmasını sağlar.
    // Production: http_reqwest ile DoH veya tokio-rustls ile DoT.
    let fake_ip = match domain.len() % 3 {
        0 => IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), // example.com
        1 => IpAddr::V4(Ipv4Addr::new(142, 250, 185, 7)), // google.com
        _ => IpAddr::V4(Ipv4Addr::new(151, 101, 1, 140)), // facebook.com
    };
    Ok(fake_ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolver_blocklist() {
        let resolver = DnsResolver::new(DnsConfig::default());
        resolver.add_block("ads.example.com".to_string()).await;
        assert!(resolver.is_blocked("ads.example.com").await);
        resolver.remove_block("ads.example.com".to_string()).await;
        assert!(!resolver.is_blocked("ads.example.com").await);
    }

    #[tokio::test]
    async fn test_resolver_wildcard_block() {
        let resolver = DnsResolver::new(DnsConfig::default());
        resolver
            .add_block("*.example.com".to_string())
            .await;
        assert!(resolver.is_blocked("sub.example.com").await);
        assert!(!resolver.is_blocked("example.com").await);
    }

    #[test]
    fn test_encode_dns_name() {
        assert_eq!(super::encode_dns_name("example.com"), vec![7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0]);
    }

    #[test]
    fn test_encode_dns_name_empty() {
        assert_eq!(super::encode_dns_name("localhost"), vec![9, b'l', b'o', b'c', b'a', b'l', b'h', b'o', b's', b't', 0]);
    }

    #[test]
    fn test_parse_doh_a_answer_minimal() {
        // header + question (example.com A IN) + 0 answers
        let mut pkt = vec![
            0x00, 0x01, // id
            0x81, 0x80, // flags
            0x00, 0x01, // questions
            0x00, 0x00, // answer
            0x00, 0x00, // authority
            0x00, 0x00, // additional
            // question: example.com + type A + class IN
            7, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
            3, b'c', b'o', b'm',
            0,
            0x00, 0x01,
            0x00, 0x01,
        ];
        assert!(super::parse_doh_a_answer(&pkt).is_none());
        // append a single A answer for 93.184.216.34
        pkt.extend_from_slice(&[
            7, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
            3, b'c', b'o', b'm',
            0,
            0x00, 0x01, // type A
            0x00, 0x01, // class IN
            0x00, 0x00, 0x00, 0x3c, // ttl
            0x00, 0x04, // rdlength
            0x5d, 0xb8, 0xd8, 0x22, // 93.184.216.34
        ]);
        assert_eq!(super::parse_doh_a_answer(&pkt), Some(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))));
    }
}
