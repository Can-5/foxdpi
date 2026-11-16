//! Kural eşleştirme — domain bazlı strateji ve exempt kuralları
use crate::config::{MatchKind, MatchingRule, Rule, Strategy};
use crate::engine::ClientHelloInfo;

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
            return Some(MatchingRule { rule: Some(rule.clone()), kind: MatchKind::Exact });
        }
        if let Some(suffix) = rule_domain.strip_prefix('*') {
            if suffix.len() > 1 && host_ref.ends_with(suffix) {
                return Some(MatchingRule { rule: Some(rule.clone()), kind: MatchKind::Wildcard });
            }
        }
    }
    None
}

/// Varsayılan mode'a kural override'ı uygulanmış etkin stratejiyi döndürür.
pub fn effective_strategy(ch: &ClientHelloInfo, default: Strategy, rules: &[Rule]) -> Strategy {
    if let Some(mr) = match_rule(ch, rules) {
        if let Some(r) = &mr.rule {
            if r.exempt {
                return Strategy::Default; // exempt = bypass yapılmaz, direkt forward
            }
            if let Some(s) = &r.strategy {
                return *s;
            }
        }
    }
    default
}
