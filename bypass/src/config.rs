//! Config — foxdpi-bypass yapılandırma struct'ları + YAML yükleme/yazma

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    Default,
    RecordSplit,
    SegmentDesync,
    Decoy,
    OOB,
    HostScramble,
    /// Fake low-TTL kopya + gerçek paket (TCP "disorder"; raw socket gerekir)
    Disorder,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strategy::Default => write!(f, "default"),
            Strategy::RecordSplit => write!(f, "record-split"),
            Strategy::SegmentDesync => write!(f, "segment-desync"),
            Strategy::Decoy => write!(f, "decoy"),
            Strategy::OOB => write!(f, "oob"),
            Strategy::HostScramble => write!(f, "host-scramble"),
            Strategy::Disorder => write!(f, "disorder"),
        }
    }
}

impl std::str::FromStr for Strategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "default" | "none" | "" => Ok(Strategy::Default),
            "split" | "record-split" | "record_split" | "tlsrec" => Ok(Strategy::RecordSplit),
            "desync" | "segment-desync" | "segment_desync" => Ok(Strategy::SegmentDesync),
            "disorder" | "fake-split" | "fakedisorder" => Ok(Strategy::Disorder),
            "decoy" | "decoy-only" => Ok(Strategy::Decoy),
            "oob" | "urgent" | "urgent-pointer" => Ok(Strategy::OOB),
            "host-scramble" | "host_scramble" | "host" | "hostcase" => Ok(Strategy::HostScramble),
            other => Err(format!("bilinmeyen strateji: '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DnsProvider {
    BuiltIn,
    Custom,
    None,
    /// DNS-over-HTTPS (en güvenli; sertifika doğrulamalı)
    Doh,
}

impl Default for DnsProvider {
    fn default() -> Self {
        DnsProvider::Doh
    }
}

impl std::str::FromStr for DnsProvider {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "builtin" | "built-in" | "built_in" | "default" | "tcp" => Ok(DnsProvider::BuiltIn),
            "custom" => Ok(DnsProvider::Custom),
            "none" | "off" => Ok(DnsProvider::None),
            "doh" | "dns-over-https" | "encrypted" => Ok(DnsProvider::Doh),
            other => Err(format!("bilinmeyen DNS sağlayıcısı: '{}'", other)),
        }
    }
}

/// Reklam/izleyici engelleme modu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum AdBlockMode {
    #[default]
    Off,
    /// DNS cevabı 0.0.0.0 (bu domain'lere hiç bağlanma)
    NullRoute,
    /// SOCKS5 CONNECT'i reddet (istemci bağlantı hatası alır)
    Deny,
    /// İstekleri yerel sinkhole'a (127.0.0.1:sinkhole_port) yönlendir; 204 döner
    Sinkhole,
}

impl std::fmt::Display for AdBlockMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdBlockMode::Off => write!(f, "off"),
            AdBlockMode::NullRoute => write!(f, "nullroute"),
            AdBlockMode::Deny => write!(f, "deny"),
            AdBlockMode::Sinkhole => write!(f, "sinkhole"),
        }
    }
}

impl std::str::FromStr for AdBlockMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "disabled" => Ok(AdBlockMode::Off),
            "nullroute" | "null" | "0.0.0.0" | "dns" => Ok(AdBlockMode::NullRoute),
            "deny" | "refuse" | "reject" => Ok(AdBlockMode::Deny),
            "sinkhole" | "local" => Ok(AdBlockMode::Sinkhole),
            other => Err(format!("bilinmeyen reklam engelleme modu: '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Rule {
    pub domain: String,
    #[serde(default)]
    pub exempt: bool,
    #[serde(default)]
    pub strategy: Option<Strategy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Exact,
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct MatchingRule {
    pub rule: Option<Rule>,
    pub kind: MatchKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub provider: DnsProvider,
    #[serde(default)]
    pub custom_servers: Vec<String>,
    /// Kullanıcı tanımlı DoH uç noktası (örn. "https://dns.example.com/dns-query");
    /// boşsa yerleşik uç noktalar (cloudflare-dns.com, dns.google, dns.quad9.net) denenir.
    #[serde(default)]
    pub doh_server: Option<String>,
    #[serde(default)]
    pub encrypted: bool,
    #[serde(default)]
    pub block_ads: bool,
    /// Reklam/izleyici engelleme modu (nullroute|deny|sinkhole|off)
    #[serde(default)]
    pub adblock: AdBlockMode,
    #[serde(default)]
    pub block_pages: Vec<String>,
    /// Harici hosts-benzeri liste dosyası (örn. StevenBlack hosts); her satırda
    /// "0.0.0.0 domain", "127.0.0.1 domain" veya tek başına "domain" desteklenir.
    #[serde(default)]
    pub blocklist_file: Option<PathBuf>,
    /// Sinkhole HTTP portu (127.0.0.1 üzerinde)
    #[serde(default = "default_sinkhole_port")]
    pub sinkhole_port: u16,
}

fn default_sinkhole_port() -> u16 {
    8899
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            provider: DnsProvider::Doh,
            custom_servers: Vec::new(),
            doh_server: None,
            encrypted: true,
            block_ads: false,
            adblock: AdBlockMode::Off,
            block_pages: Vec::new(),
            blocklist_file: None,
            sinkhole_port: default_sinkhole_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub mode: Strategy,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub dns: DnsConfig,
    pub port: u16,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub advanced_mode: bool,
    /// record-split bölme noktası (mutlak byte offset); yoksa SNI offset kullanılır
    #[serde(default)]
    pub split_offset: Option<usize>,
    /// Decoy/disorder sahte paket TTL'i (raw socket, Linux)
    #[serde(default = "default_decoy_ttl")]
    pub decoy_ttl: u8,
    /// Auto-TTL: 1..autottl_max aralığında artan TTL'li kopya gönder
    #[serde(default)]
    pub autottl: bool,
    #[serde(default = "default_autottl_max")]
    pub autottl_max: u8,
    /// segment-desync parçaları arasındaki gecikme (ms) — reassembly yarışı için
    #[serde(default)]
    pub desync_delay_ms: u64,
}

fn default_decoy_ttl() -> u8 {
    3
}

fn default_autottl_max() -> u8 {
    6
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Strategy::Default,
            rules: Vec::new(),
            dns: DnsConfig::default(),
            port: 1080,
            enabled: false,
            advanced_mode: false,
            split_offset: None,
            decoy_ttl: default_decoy_ttl(),
            autottl: false,
            autottl_max: default_autottl_max(),
            desync_delay_ms: 0,
        }
    }
}

pub fn write_default_config(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = Config::default();
    let yaml = serde_yaml::to_string(&cfg)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

pub fn load_config(path: &PathBuf) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    if !path.exists() {
        write_default_config(path)?;
    }
    let text = std::fs::read_to_string(path)?;
    let cfg: Config = serde_yaml::from_str(&text)?;
    Ok(cfg)
}
