//! Config — foxdpi-bypass yapılandırma struct'ları + YAML yükleme/yazma

use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    Default,
    RecordSplit,
    SegmentDesync,
    Decoy,
    OOB,
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
        match s.trim().to_lowercase().as_str() {
            "default" | "none" | "" => Ok(Strategy::Default),
            "split" | "record-split" | "record_split" | "tlsrec" => Ok(Strategy::RecordSplit),
            "desync" | "segment-desync" | "segment_desync" | "disorder" => Ok(Strategy::SegmentDesync),
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
}

impl Default for DnsProvider {
    fn default() -> Self {
        DnsProvider::BuiltIn
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
    #[serde(default)]
    pub encrypted: bool,
    #[serde(default)]
    pub block_ads: bool,
    #[serde(default)]
    pub block_pages: Vec<String>,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            provider: DnsProvider::BuiltIn,
            custom_servers: Vec::new(),
            encrypted: false,
            block_ads: false,
            block_pages: Vec::new(),
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
