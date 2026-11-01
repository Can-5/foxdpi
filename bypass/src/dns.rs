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

    /// İç pool'dan DoH/DNS-over-TCP ile çözümleme — şimdilik DNS-over-TCP
    /// placeholder'ı (gerçek DoH/DoT implementasyonu gelecek).
    async fn resolve_builtin(&self, domain: &str, cfg: &DnsConfig) -> Result<IpAddr, DnsError> {
        // varsayılan DNS sunucuları: rastgele seçilmiş pool
        let servers = builtin_servers(cfg.encrypted);
        // basit DNS-over-TCP placeholder: gerçekte DoH/DoT yapılır
        for server in servers.iter().take(3) {
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

/// Varsayılan DNS sunucuları (DoH/DoT/DNS-over-TCP placeholder'lari)
fn builtin_servers(encrypted: bool) -> Vec<String> {
    if encrypted {
        // DoH/DoT placeholder (gerçekte DoH URL veya DoT IP:port)
        vec![
            "9.9.9.9:53".into(), // Quad9
            "1.1.1.1:53".into(), // Cloudflare
            "8.8.8.8:53".into(), // Google
            "208.67.222.222:53".into(), // OpenDNS
        ]
    } else {
        vec!["8.8.8.8:53".into(), "1.1.1.1:53".into()]
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
}
