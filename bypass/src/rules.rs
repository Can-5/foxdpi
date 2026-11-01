//! Kural eşleştirme — domain bazlı strateji ve exempt kuralları
use crate::config::{Rule, Strategy};
pub use crate::ClientHelloInfo;
pub use crate::config::MatchingRule;
pub use crate::config::MatchKind;

/// Kayıt listesinden, bir ClientHelloInfo için hangi kuralın geçerli olduğunu bul
pub fn match_rule(ch: &ClientHelloInfo, rules: &[Rule]) -> Option<MatchingRule> {
    let host = ch
        .host
        .as_deref()
        .or_else(|| ch.sni.as_deref())
        .map(|h| h.to_ascii_lowercase());
    let host_ref = host.as_deref()?;

    for rule in rules.iter() {
        let rule_domain = rule.domain.to_ascii_lowercase();
        if rule_domain == host_ref {
            return Some(MatchingRule {
                rule: Some(rule.clone()),
                kind: MatchKind::Exact,
            });
        }
        if rule_domain.starts_with('*') && rule_domain.len() > 1 {
            let suffix = &rule_domain[1..]; // "example.com" -> "*.example.com" pattern
            if host_ref.ends_with(suffix) {
                return Some(MatchingRule {
                    rule: Some(rule.clone()),
                    kind: MatchKind::Wildcard,
                });
            }
        }
    }
    None
}

/// Verilen bir ClientHelloInfo ve varsayılan mode'u al, kural override
/// uygulanmış etkin stratejiyi döndürür.
pub fn effective_strategy(ch: &ClientHelloInfo, default: Strategy, rules: &[Rule]) -> Strategy {
    match match_rule(ch, rules) {
        Some(mr) => {
            if let Some(r) = &mr.rule {
                if r.exempt {
                    // exempt = bypass yapılmaz, direkt forward
                    return Strategy::Default;
                }
                if let Some(s) = &r.strategy {
                    return *s;
                }
            }
        }
        None => {}
    }
    default
}
