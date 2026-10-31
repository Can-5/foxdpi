//! foxdpi-bypass — kendi DPI bypass motorumuz (şablon kütüphanesi)
//!
//! Bu kütüphane şu an şablon aşamasında; gerçek bypass motoru
//! ayrı bir geliştirme.ekipleri tarafından tamamlanacaktır.
//!
//! Şablon olarak aşağıdaki placeholderları içerir:
//! - strateji seçimi (default, split, desync, decoy, oob, host_scramble)
//! - TLS ClientHello parse placeholder (gelecek)

use serde::{Deserialize, Serialize};

/// Desteklenen strateji türleri (şablon)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    /// Varsayılan (şablon; gerçekte none veya direkt tunnel)
    Default,
    /// TLS record split (RFC 8446 §5.1)
    RecordSplit,
    /// TCP segment desync/disorder
    SegmentDesync,
    /// Decoy (Linux) — filtrenin okuduğu ilk segment
    Decoy,
    /// Out-of-band (urgent pointer) bayt
    OOB,
    /// HTTP Host scramble
    HostScramble,
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
        }
    }
}

impl std::str::FromStr for Strategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" | "none" => Ok(Strategy::Default),
            "split" | "record-split" | "record_split" => Ok(Strategy::RecordSplit),
            "desync" | "segment-desync" | "segment_desync" => Ok(Strategy::SegmentDesync),
            "decoy" => Ok(Strategy::Decoy),
            "oob" | "urgent" => Ok(Strategy::OOB),
            "host-scramble" | "host_scramble" | "host" => Ok(Strategy::HostScramble),
            other => Err(format!("bilinmeyen strateji: {}", other)),
        }
    }
}

/// Şablon motoru yapılandırması
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub mode: Strategy,
    pub verbose: bool,
    pub dns: DnsConfig,
    pub rules: Vec<Rule>,
}

/// Şablon DNS yapılandırması
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub provider: DnsProvider,
    pub custom_servers: Vec<String>,
    pub encrypted: bool,
    pub block_ads: bool,
    /// bilinen block page listesi (şablon)
    #[serde(default)]
    pub block_pages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsProvider {
    ShroudlyBuiltIn,
    Custom,
}

/// Şablon kuralı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub domain: String,
    #[serde(default)]
    pub exempt: bool,
    #[serde(default)]
    pub strategy: Option<Strategy>,
}

/// Şablon bir ClientHello parse sonucu (placeholder)
#[derive(Debug, Clone)]
pub struct ClientHelloInfo {
    pub sni: Option<String>,
    pub host: Option<String>,
    pub raw_position: Option<usize>,
}

/// Şablon parse fonksiyonu (placeholder; gerçek implementasyonda
/// TLS record parse + SNI/Host pozisyonu bulunur)
pub fn parse_client_hello(_buf: &[u8]) -> ClientHelloInfo {
    ClientHelloInfo {
        sni: None,
        host: None,
        raw_position: None,
    }
}

/// Şablon strateji uygula (placeholder)
pub async fn apply_strategy(_info: &ClientHelloInfo, _mode: Strategy) -> Option<Vec<u8>> {
    None
}
