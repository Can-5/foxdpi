//! foxdpi-bypass — kendi DPI bypass motorumuz

pub mod buffer;
pub mod config;
pub mod dns;
pub mod doh;
pub mod engine;
pub mod proxy;
pub mod raw;
pub mod rules;
pub mod sinkhole;

pub use buffer::Buffer;
pub use config::{AdBlockMode, Config, DnsProvider, MatchKind, MatchingRule, Rule, Strategy};
pub use dns::DnsResolver;
pub use engine::{apply_strategy, ClientHelloInfo};
pub use proxy::SocksProxy;
pub use rules::{effective_strategy, match_rule};
pub use sinkhole::Sinkhole;
