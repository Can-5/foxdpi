//! DNS çözümleyici — TCP üzerinden gerçek DNS sorgusu (RFC 1035 §4.2.2),
//! block page listesi ve builtin sunucu havuzu.
//!
//! SOCKS5 proxy, domain hedefli CONNECT isteklerinde kendi DNS
//! çözümlemesini kullanır. "Encrypted" seçeneği şu an Do53-over-TCP
//! (zamanlama/padding açısından UDP'ye göre daha dayanıklı) olarak uygulanır;
//! gerçek DoH/DoT sonraki sürümde eklenecek (bkz. README).

use crate::config::{AdBlockMode, DnsConfig, DnsProvider};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub struct DnsResolver {
    config: Arc<RwLock<DnsConfig>>,
    blocklist: Arc<RwLock<HashSet<String>>>,
}

impl DnsResolver {
    pub fn new(config: DnsConfig) -> Self {
        let mut blocklist: HashSet<String> = config.block_pages.iter().map(|s| s.to_ascii_lowercase()).collect();
        if let Some(path) = &config.blocklist_file {
            match load_blocklist_file(path) {
                Ok(entries) => {
                    info!("Reklam blocklist'i yüklendi: {} kayıt ({})", entries.len(), path.display());
                    blocklist.extend(entries);
                }
                Err(e) => warn!("blocklist dosyası okunamadı ({}): {}", path.display(), e),
            }
        }
        if config.block_ads {
            blocklist.extend(DEFAULT_AD_BLOCK_LIST.iter().map(|s| s.to_ascii_lowercase()));
        }
        Self { config: Arc::new(RwLock::new(config)), blocklist: Arc::new(RwLock::new(blocklist)) }
    }

    pub async fn add_block(&self, entry: String) {
        self.blocklist.write().await.insert(entry.to_ascii_lowercase());
    }

    pub async fn remove_block(&self, entry: String) {
        self.blocklist.write().await.remove(&entry.to_ascii_lowercase());
    }

    pub async fn set_blocklist(&self, entries: Vec<String>) {
        *self.blocklist.write().await = entries.into_iter().map(|s| s.to_ascii_lowercase()).collect();
    }

    pub async fn update_config(&self, config: DnsConfig) {
        *self.config.write().await = config;
    }

    pub async fn resolve(&self, domain: &str) -> Result<IpAddr, DnsError> {
        let domain = domain.trim_end_matches('.').to_string();
        let cfg = self.config.read().await.clone();

        // DNS seviyesinde engelleme yalnızca nullroute/block_ads modunda;
        // deny/sinkhole modlarında SOCKS5 katmanında karar verilir.
        if (cfg.adblock == AdBlockMode::NullRoute || cfg.block_ads) && self.is_blocked(&domain).await {
            return Ok(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        }

        match cfg.provider {
            DnsProvider::Doh => {
                // Önce sertifika doğrulamalı DoH; olmazsa TCP-53'e düşer.
                if let Some(url) = cfg.doh_server.as_deref() {
                    if let Ok(ip) = crate::doh::query_custom(&domain, url, 1).await {
                        debug!("DNS çözüldü (custom DoH {}): {} -> {}", url, domain, ip);
                        return Ok(ip);
                    }
                    warn!("custom DoH başarısız ({}), yerleşik DoH'ya düşülüyor", url);
                }
                for ep in crate::doh::BUILTIN_DOH {
                    match crate::doh::query(&domain, ep, 1).await {
                        Ok(ip) => { debug!("DNS çözüldü (DoH {}): {} -> {}", ep.host, domain, ip); return Ok(ip); }
                        Err(e) => warn!("DoH başarısız ({} @ {}): {}", domain, ep.host, e),
                    }
                }
                // DoH çalışmazsa TCP-53 fallback zinciri
                for server in builtin_servers() {
                    if let Ok(ip) = query_a(&domain, server).await {
                        warn!("DoH başarısız, TCP-53 fallback kullanıldı ({})", server);
                        return Ok(ip);
                    }
                }
                if let Some(ip) = resolve_aaaa_builtin(&domain).await { return Ok(ip); }
                Err(DnsError::NoResolution)
            }
            DnsProvider::BuiltIn => {
                for server in builtin_servers() {
                    match query_a(&domain, server).await {
                        Ok(ip) => { debug!("DNS çözüldü (builtin {}): {} -> {}", server, domain, ip); return Ok(ip); }
                        Err(e) => warn!("DNS başarısız ({} @ {}): {}", domain, server, e),
                    }
                }
                // IPv6 dene
                if let Some(ip) = resolve_aaaa_builtin(&domain).await { return Ok(ip); }
                Err(DnsError::NoResolution)
            }
            DnsProvider::Custom => {
                if cfg.custom_servers.is_empty() { return Err(DnsError::CustomNoServers); }
                for server in &cfg.custom_servers {
                    let addr = parse_server_addr(server).ok_or_else(|| DnsError::ParseError(format!("sunucu adresi geçersiz: {}", server)))?;
                    match query_a(&domain, addr).await {
                        Ok(ip) => { debug!("DNS çözüldü (custom {}): {} -> {}", addr, domain, ip); return Ok(ip); }
                        Err(e) => warn!("DNS başarısız ({} @ {}): {}", domain, addr, e),
                    }
                }
                Err(DnsError::NoResolution)
            }
            DnsProvider::None => Err(DnsError::ProviderNone),
        }
    }

    pub async fn is_blocked(&self, domain: &str) -> bool {
        let bl = self.blocklist.read().await;
        let d = domain.to_ascii_lowercase();
        bl.iter().any(|e| {
            if let Some(suffix) = e.strip_prefix('*') {
                suffix.len() > 1 && d.ends_with(suffix)
            } else {
                // Tam eşleşme veya alt-alan (ads.x.com, a.b.x.com vs.)
                d == *e
                    || (d.len() > e.len()
                        && d.as_bytes()[d.len() - e.len() - 1] == b'.'
                        && d.ends_with(e.as_str()))
            }
        })
    }
}

async fn resolve_aaaa_builtin(domain: &str) -> Option<IpAddr> {
    for server in builtin_servers() {
        if let Ok(ip) = query(domain, server, 28).await { return Some(ip); }
    }
    None
}

/// "1.1.1.1", "1.1.1.1:53", "[::1]:53" biçimlerini parse eder.
fn parse_server_addr(s: &str) -> Option<SocketAddr> {
    let s = s.trim();
    if let Ok(a) = s.parse::<SocketAddr>() { return Some(a); }
    if let Ok(ip) = s.parse::<IpAddr>() { return Some(SocketAddr::new(ip, 53)); }
    None
}

fn builtin_servers() -> Vec<SocketAddr> {
    ["1.1.1.1", "8.8.8.8", "9.9.9.9", "208.67.222.222"]
        .iter()
        .map(|s| SocketAddr::new(s.parse::<IpAddr>().unwrap(), 53))
        .collect()
}

/// A kaydı sorgula (qtype=1).
async fn query_a(domain: &str, server: SocketAddr) -> Result<IpAddr, DnsError> {
    query(domain, server, 1).await
}

/// Genel DNS sorgusu (TCP, length-prefixed).
async fn query(domain: &str, server: SocketAddr, qtype: u16) -> Result<IpAddr, DnsError> {
    let txid: u16 = rand_txid();
    let query_pkt = encode_query(domain, txid, qtype)?;

    let mut stream = tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(server))
        .await
        .map_err(|_| DnsError::Timeout)??;

    let len = (query_pkt.len() as u16).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&query_pkt).await?;
    stream.flush().await?;

    let mut len_buf = [0u8; 2];
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut len_buf)).await.map_err(|_| DnsError::Timeout)??;
    let resp_len = ((len_buf[0] as usize) << 8) | len_buf[1] as usize;
    let mut resp = vec![0u8; resp_len];
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut resp)).await.map_err(|_| DnsError::Timeout)??;

    parse_response(&resp, txid, qtype)
}

fn rand_txid() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos() as u16).unwrap_or(0x1234);
    nanos ^ (std::process::id() as u16) ^ 0x9e37
}

pub(crate) fn encode_query(domain: &str, txid: u16, qtype: u16) -> Result<Vec<u8>, DnsError> {
    let mut pkt = Vec::with_capacity(64);
    pkt.extend_from_slice(&txid.to_be_bytes());
    pkt.extend_from_slice(&[0x01, 0x00]); // flags: RD
    pkt.extend_from_slice(&[0x00, 0x01]); // QDCOUNT
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // AN/NS/AR
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(DnsError::ParseError(format!("geçersiz etiket: '{}'", label)));
        }
        pkt.push(label.len() as u8);
        pkt.extend_from_slice(label.as_bytes());
    }
    pkt.push(0x00); // terminator
    pkt.extend_from_slice(&qtype.to_be_bytes());
    pkt.extend_from_slice(&[0x00, 0x01]); // CLASS IN
    Ok(pkt)
}

pub(crate) fn parse_response(resp: &[u8], txid: u16, want_type: u16) -> Result<IpAddr, DnsError> {
    if resp.len() < 12 { return Err(DnsError::ParseError("kısa cevap".into())); }
    let r_txid = ((resp[0] as u16) << 8) | resp[1] as u16;
    if r_txid != txid { return Err(DnsError::ParseError("txid uyuşmazlığı".into())); }
    let flags = ((resp[2] as u16) << 8) | resp[3] as u16;
    if flags & 0x000f != 0 { return Err(DnsError::ParseError(format!("RCODE={}", flags & 0x000f))); }
    let qd = ((resp[4] as usize) << 8) | resp[5] as usize;
    let an = ((resp[6] as usize) << 8) | resp[7] as usize;

    let mut pos = 12usize;
    // Soru bölümünü atla
    for _ in 0..qd {
        pos = skip_name(resp, pos)?;
        pos += 4; // qtype + qclass
        if pos > resp.len() { return Err(DnsError::ParseError("soru taşması".into())); }
    }

    for _ in 0..an {
        pos = skip_name(resp, pos)?;
        if pos + 10 > resp.len() { return Err(DnsError::ParseError("cevap taşması".into())); }
        let rtype = ((resp[pos] as usize) << 8) | resp[pos + 1] as usize;
        let rdlen = ((resp[pos + 8] as usize) << 8) | resp[pos + 9] as usize;
        let rdata = pos + 10;
        if rdata + rdlen > resp.len() { return Err(DnsError::ParseError("rdata taşması".into())); }
        if rtype == want_type as usize {
            match (rtype, rdlen) {
                (1, 4) => return Ok(IpAddr::V4(Ipv4Addr::new(resp[rdata], resp[rdata+1], resp[rdata+2], resp[rdata+3]))),
                (28, 16) => {
                    let mut o = [0u8; 16];
                    o.copy_from_slice(&resp[rdata..rdata+16]);
                    return Ok(IpAddr::V6(Ipv6Addr::from(o)));
                }
                _ => {}
            }
        }
        pos = rdata + rdlen;
    }
    Err(DnsError::NoResolution)
}

/// DNS adını (sıkıştırılmış dahil) atlayarak sonraki pozisyonu döndürür.
fn skip_name(resp: &[u8], mut pos: usize) -> Result<usize, DnsError> {
    loop {
        if pos >= resp.len() { return Err(DnsError::ParseError("ad taşması".into())); }
        let len = resp[pos] as usize;
        if len == 0 { return Ok(pos + 1); }
        if len & 0xc0 == 0xc0 {
            return Ok(pos + 2); // pointer → 2 bayt
        }
        pos += 1 + len;
    }
}

/// Harici hosts-benzeri blocklist dosyasını oku ve domain kümesini döndürür.
/// Desteklenen satır biçimleri:
///   "0.0.0.0 domain", "127.0.0.1 domain", "domain", ".domain", "# yorum"
pub fn load_blocklist_file(path: &std::path::Path) -> Result<HashSet<String>, std::io::Error> {
    let text = std::fs::read_to_string(path)?;
    let mut out = HashSet::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut token: Option<&str> = None;
        for field in line.split_whitespace() {
            let f = field.trim();
            if f.eq_ignore_ascii_case("0.0.0.0") || f.eq_ignore_ascii_case("127.0.0.1")
                || f.eq_ignore_ascii_case("::") || f.eq_ignore_ascii_case("::1") || f.eq_ignore_ascii_case("255.255.255.255") {
                continue; // loopback/boş yönlendirme adresi — domain değil
            }
            token = Some(f);
        }
        if let Some(d) = token {
            let d = d.trim().trim_start_matches('*').trim_start_matches('.').to_ascii_lowercase();
            if d.contains('.') && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_') {
                out.insert(d);
            }
        }
    }
    Ok(out)
}

/// Varsayılan reklam/izleyici engelleme listesi
pub const DEFAULT_AD_BLOCK_LIST: &[&str] = &[
    "ads.example.com",
    "tracker.example.com",
    "analytics.example.com",
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "pagead2.googlesyndication.com",
    "adservice.google.com",
    "connect.facebook.net",
    "ads.twitter.com",
    "bat.bing.com",
    "scorecardresearch.com",
    "hotjar.com",
    "static.ads-twitter.com",
    "analytics.twitter.com",
    "pixel.facebook.com",
    "an.facebook.com",
    "ad.doubleclick.net",
    "adservice.google.com.tr",
    "syndication.twitter.com",
    "ads.youtube.com",
    "imasdk.googleapis.com",
    "ads.tiktok.com",
    "ads.linkedin.com",
    "adsrvr.org",
    "ads-api.twitter.com",
    "advertising.com",
    "amazon-adsystem.com",
    "moatads.com",
    "serving-sys.com",
    "criteo.com",
    "criteo.net",
    "outbrain.com",
    "taboola.com",
    "pubmatic.com",
    "rubiconproject.com",
    "openx.net",
    "adnxs.com",
    "contextweb.com",
    "bidswitch.net",
    "spotxchange.com",
    "yieldmo.com",
    "sharethrough.com",
    "smartadserver.com",
    "adform.net",
    "mopub.com",
    "adzerk.net",
];

#[derive(Debug, thiserror::Error)]
pub enum DnsError {
    #[error("DNS G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("DNS zaman aşımı")]
    Timeout,
    #[error("Hiçbir DNS sunucusu cevap vermedi")]
    NoResolution,
    #[error("DNS sağlayıcı 'none' ayarlı")]
    ProviderNone,
    #[error("Kullanıcı tanımlı DNS sunucusu yok")]
    CustomNoServers,
    #[error("DNS cevabı parse edilemedi: {0}")]
    ParseError(String),
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
        resolver.add_block("*.example.com".to_string()).await;
        assert!(resolver.is_blocked("sub.example.com").await);
        assert!(!resolver.is_blocked("example.com").await);
    }

    #[test]
    fn test_encode_query_roundtrip_shape() {
        let q = encode_query("ex.com", 0x1234, 1).unwrap();
        assert_eq!(&q[0..2], &[0x12, 0x34]);
        assert_eq!(&q[12..17], &[0x02, b'e', b'x', 0x03, b'c']);
    }

    #[test]
    fn test_skip_name_pointer() {
        // offset 12: isim "a.b" normal + pointer kullanımı simülasyonu
        let resp = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, b'a', 1, b'b', 0, 0];
        assert_eq!(skip_name(&resp, 12).unwrap(), 17);
    }

    #[test]
    fn test_load_blocklist_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("foxdpi-test-blocklist.txt");
        std::fs::write(&path, "# yorum\n0.0.0.0 Ads.Example.com\n127.0.0.1 track.example.net # satır sonu\nplain-domain.example.org\n\n::  v6host.example.com\n").unwrap();
        let out = load_blocklist_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(out.contains("ads.example.com"));
        assert!(out.contains("track.example.net"));
        assert!(out.contains("plain-domain.example.org"));
        assert!(out.contains("v6host.example.com"));
        assert_eq!(out.len(), 4);
    }
}
