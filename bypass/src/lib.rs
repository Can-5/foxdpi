//! foxdpi-bypass — kendi DPI bypass motorumuz

//! Bu sürüm real implementasyon seviyesindedir:
//! - TLS record split, segment desync/disorder, decoy (TTL-based örneklendi),
//!   OOB ve HTTP Host scramble engine katmanında çalışır.
//! - DNS tarafında gerçek DoH (DNS-over-HTTPS) kullanılır; fallback DNS-over-TCP
//!   placeholder da vardır; blok sayfa listesi demikian.

pub mod buffer;
pub mod config;
pub mod engine;
pub mod proxy;
pub mod dns;
pub mod rules;

pub use buffer::Buffer;
pub use config::Config;
pub use engine::{ClientHelloInfo, Strategy, apply_strategy};
pub use proxy::SocksProxy;
pub use rules::{Rule, MatchingRule, match_rule, effective_strategy};
pub use dns::DnsResolver;
