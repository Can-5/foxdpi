//! foxdpi-bypass — kendi DPI bypass motorumuz

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
