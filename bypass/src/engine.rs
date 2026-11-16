//! Engine — TLS ClientHello parse + strateji uygula.
//!
//! İlk veri parçasını parse eder (TLS ClientHello veya plain HTTP),
//! SNI'nin buf içindeki mutlak offset'ini döndürür ve istenen stratejiye
//! göre forward edilecek parçaları üretir.

use crate::config::{Rule, Strategy};
use crate::rules::effective_strategy;
use std::net::SocketAddr;

/// İlk baytlardan çıkarılan bağlantı bilgisi.
/// `sni_offset` her zaman **buf'ın başındaki mutlak byte konumu**dur.
#[derive(Debug, Clone)]
pub struct ClientHelloInfo {
    pub sni: Option<String>,
    pub host: Option<String>,
    pub sni_offset: Option<usize>,
    pub record_end: Option<usize>,
    pub first_n: usize,
}

pub fn parse_first_bytes(buf: &[u8]) -> ClientHelloInfo {
    if buf.is_empty() {
        return ClientHelloInfo { sni: None, host: None, sni_offset: None, record_end: None, first_n: 0 };
    }
    match buf[0] {
        0x16 => parse_tls_client_hello(buf),
        _ => parse_http_host(buf),
    }
}

fn empty_info(buf_len: usize) -> ClientHelloInfo {
    ClientHelloInfo { sni: None, host: None, sni_offset: None, record_end: None, first_n: buf_len }
}

/// TLS ClientHello'yu tek-kayıt varsayımıyla parse eder (pratikteki
/// ClientHello'ların tamamına yakını tek handshake record'u içindedir).
fn parse_tls_client_hello(buf: &[u8]) -> ClientHelloInfo {
    if buf.len() < 9 {
        return empty_info(buf.len());
    }
    let record_len = (((buf[3] as usize) << 8) | buf[4] as usize).min(buf.len().saturating_sub(5));
    let record_end = 5 + record_len;
    // buf tek kaydı kapsıyorsa record_end = gerçek son; yoksa buf sonu
    let record_end_abs = record_end.min(buf.len());

    // Handshake: [5]=type(0x01) [6..9]=len(3) [9..]=body
    if buf[5] != 0x01 {
        return ClientHelloInfo { record_end: Some(record_end_abs), ..empty_info(buf.len()) };
    }
    let hs_len = (((buf[6] as usize) << 16) | ((buf[7] as usize) << 8) | buf[8] as usize)
        .min(record_end_abs.saturating_sub(9));
    let hs_body_start = 9usize;
    let hs_body_end = (hs_body_start + hs_len).min(record_end_abs);
    let body = &buf[hs_body_start..hs_body_end];

    let sni = find_sni_absolute(buf, hs_body_start, body);

    ClientHelloInfo {
        sni: sni.as_ref().map(|(name, _)| name.clone()),
        host: sni.as_ref().map(|(name, _)| name.clone()),
        sni_offset: sni.map(|(_, off)| off),
        record_end: Some(record_end_abs),
        first_n: buf.len(),
    }
}

/// hs_body'yi tarayıp SNI (server_name) extension'ını ve adın buf-abs
/// offset'ini döndürür.
fn find_sni_absolute(buf: &[u8], body_start: usize, body: &[u8]) -> Option<(String, usize)> {
    // legacy_version(2) + random(32) = 34
    let mut pos = 34usize;
    if pos + 1 > body.len() { return None; }
    let sid_len = body[pos] as usize;
    pos += 1 + sid_len;
    if pos + 2 > body.len() { return None; }
    let cs_len = ((body[pos] as usize) << 8) | body[pos + 1] as usize;
    pos += 2 + cs_len;
    if pos >= body.len() { return None; }
    let cm_len = body[pos] as usize;
    pos += 1 + cm_len;
    if pos + 2 > body.len() { return None; }
    let ext_total = ((body[pos] as usize) << 8) | body[pos + 1] as usize;
    pos += 2;
    let ext_end = (pos + ext_total).min(body.len());

    while pos + 4 <= ext_end {
        let ext_type = ((body[pos] as usize) << 8) | body[pos + 1] as usize;
        let ext_len = ((body[pos + 2] as usize) << 8) | body[pos + 3] as usize;
        let data_start = pos + 4;
        let data_end = (data_start + ext_len).min(body.len());
        if data_start > body.len() { break; }

        if ext_type == 0x0000 && ext_len >= 5 && data_end <= body.len() {
            // server_name_list: list_len(2) | type(1) | name_len(2) | name
            let d = &body[data_start..data_end];
            if d.len() >= 5 {
                let name_len = ((d[3] as usize) << 8) | d[4] as usize;
                let name_start = data_start + 5;
                let name_end = name_start + name_len;
                if name_end <= body.len() {
                    let name = String::from_utf8_lossy(&buf[body_start + name_start..body_start + name_end]).into_owned();
                    // Adın mutlak offset'i (isim baytlarının başlangıcı)
                    let abs = body_start + name_start;
                    return Some((name, abs));
                }
            }
        }
        pos = data_end;
    }
    None
}

fn parse_http_host(buf: &[u8]) -> ClientHelloInfo {
    // "Host:" başlığını case-insensitive ara
    let windows_lower: Vec<u8> = buf.iter().map(|b| b.to_ascii_lowercase()).collect();
    if let Some(rel) = windows_lower.windows(5).position(|w| w == b"host:") {
        let start = rel + 5;
        let end = buf[start..]
            .iter()
            .position(|&b| b == b'\r' || b == b'\n')
            .map(|p| start + p)
            .unwrap_or(buf.len());
        let host = String::from_utf8_lossy(&buf[start..end]).trim().to_string();
        let host_opt = if host.is_empty() { None } else { Some(host) };
        return ClientHelloInfo {
            sni: host_opt.clone(),
            host: host_opt,
            sni_offset: None,
            record_end: None,
            first_n: buf.len(),
        };
    }
    empty_info(buf.len())
}

/// Segment desync: baytları parçalara böler (küçük TCP write'ları için).
pub fn apply_segment_desync(original: &[u8], chunk_sizes: &[usize]) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    for &size in chunk_sizes {
        if offset >= original.len() { break; }
        let end = (offset + size).min(original.len());
        chunks.push(original[offset..end].to_vec());
        offset = end;
    }
    if offset < original.len() {
        chunks.push(original[offset..].to_vec());
    }
    chunks
}

/// HTTP Host scramble: "Host:" başlık adının ve değerinin harf case'ini
/// ters çevirir (örn. "HOsT: eXaMPLE.cOM"). HTTP header adları
/// case-insensitive'dır; bazı DPI'ler düz "Host:" eşler.
pub fn apply_host_scramble(buf: &[u8]) -> Vec<u8> {
    let mut out = buf.to_vec();
    let lower: Vec<u8> = out.iter().map(|b| b.to_ascii_lowercase()).collect();
    if let Some(pos) = lower.windows(5).position(|w| w == b"host:") {
        let end = out[pos..]
            .iter()
            .position(|&b| b == b'\r' || b == b'\n')
            .map(|p| pos + p)
            .unwrap_or(out.len());
        for b in &mut out[pos..end] {
            if b.is_ascii_lowercase() { *b -= 32; } else if b.is_ascii_uppercase() { *b += 32; }
        }
    }
    out
}

/// TLS record split: tek handshake kaydını verilen mutlak offset'ten
/// iki ayrı TLS record'a böler.
pub fn apply_record_split(original: &[u8], split_abs: usize) -> Vec<Vec<u8>> {
    if original.len() < 10 {
        return vec![original.to_vec()];
    }
    let record_len = ((original[3] as usize) << 8) | original[4] as usize;
    let record_end = (5 + record_len).min(original.len());
    // Bölme noktası kaydın içinde ve handshake gövdesinde olmalı
    if split_abs < 9 || split_abs >= record_end {
        return vec![original.to_vec()];
    }
    let mut rec1 = Vec::with_capacity(split_abs + 1);
    rec1.extend_from_slice(&original[..3]); // content type + legacy version
    let len1 = (split_abs - 5) as u16;
    rec1.extend_from_slice(&len1.to_be_bytes());
    rec1.extend_from_slice(&original[5..split_abs]);

    let rest = &original[split_abs..record_end];
    let mut rec2 = Vec::with_capacity(5 + rest.len() + (original.len() - record_end));
    rec2.extend_from_slice(&original[..3]);
    let len2 = rest.len() as u16;
    rec2.extend_from_slice(&len2.to_be_bytes());
    rec2.extend_from_slice(rest);
    if record_end < original.len() {
        rec2.extend_from_slice(&original[record_end..]);
    }
    vec![rec1, rec2]
}

pub struct StrategyResult {
    pub mode: Strategy,
    pub parts: Vec<Vec<u8>>,
    pub oob_byte: Option<u8>,
    /// Raw socket ile düşük TTL'li gönderilecek sahte paket (decoy / disorder).
    /// Sunucuya ulaşmaz; DPI'nin reassembly kaydını kirletir.
    pub raw_fake: Option<Vec<u8>>,
}

/// Strateji uygulanmış, forward edilecek parçaları döndürür.
pub fn apply_strategy(
    info: &ClientHelloInfo,
    mode: Strategy,
    addr: SocketAddr,
    target: SocketAddr,
    rules: &[Rule],
    original: &[u8],
    split_offset: Option<usize>,
) -> StrategyResult {
    let _ = (addr, target); // şema gereği imzada tutuluyor
    let effective = effective_strategy(info, mode, rules);
    let is_tls = info.record_end.is_some() || original.first() == Some(&0x16);
    let plain = |effective: Strategy| StrategyResult { mode: effective, parts: vec![original.to_vec()], oob_byte: None, raw_fake: None };

    match effective {
        Strategy::Default => plain(effective),
        Strategy::RecordSplit => {
            // Record split sadece TLS ClientHello'ya uygulanır; HTTP/diğer
            // protokollere uygulanırsa isteği bozar (ilk 3 bayt + sahte
            // 2'şer baytlık "uzunluk" başlığına dönüşür).
            if !is_tls {
                return plain(Strategy::Default);
            }
            // Hedefli split: --split-offset verildiyse mutlak offset, yoksa SNI offset
            let split_point = split_offset.unwrap_or_else(|| info.sni_offset.unwrap_or(original.len() / 2));
            StrategyResult { mode: effective, parts: apply_record_split(original, split_point), oob_byte: None, raw_fake: None }
        }
        Strategy::SegmentDesync => {
            // İlk parçalar küçük: 2,4,8,16 bayt sonra kalan
            let mut chunks = Vec::new();
            let mut remaining = original.len();
            let mut cur = 2usize;
            while remaining > 0 {
                let take = cur.min(remaining);
                chunks.push(take);
                remaining -= take;
                cur = (cur * 2).min(256);
            }
            StrategyResult { mode: effective, parts: apply_segment_desync(original, &chunks), oob_byte: None, raw_fake: None }
        }
        Strategy::Decoy => {
            // Sahte ClientHello: SNI host'u aynı uzunlukta rastgele harflerle
            let sni_len = info.sni.as_ref().map(|s| s.len()).unwrap_or(0);
            let fake = crate::raw::fake_client_hello(original, info.sni_offset, sni_len);
            StrategyResult { mode: effective, parts: vec![original.to_vec()], oob_byte: None, raw_fake: Some(fake) }
        }
        Strategy::Disorder => {
            // TCP disorder: gerçek baytların düşük TTL'li bir kopyası yola çıkar;
            // DPI reassembly'si kopyayı "gerçek" sayar, sunucu kopyayı hiç görmez
            StrategyResult { mode: effective, parts: vec![original.to_vec()], oob_byte: None, raw_fake: Some(original.to_vec()) }
        }
        Strategy::OOB => {
            if !is_tls {
                // OOB (urgent pointer) ham HTTP gibi metin protokollere
                // uygulanırsa ilk baytı tehlikeye atar; sadece TLS'te kullan.
                return plain(Strategy::Default);
            }
            let oob = original.first().copied();
            let parts = match (oob, original.len()) {
                (Some(_), n) if n > 1 => vec![original[1..].to_vec()],
                _ => vec![original.to_vec()],
            };
            StrategyResult { mode: effective, parts, oob_byte: oob, raw_fake: None }
        }
        Strategy::HostScramble => {
            let modified = if is_tls {
                // TLS trafiğine dokunma
                vec![original.to_vec()]
            } else {
                vec![apply_host_scramble(original)]
            };
            StrategyResult { mode: effective, parts: modified, oob_byte: None, raw_fake: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn dummy_addrs() -> (SocketAddr, SocketAddr) {
        (
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10850),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443),
        )
    }

    #[test]
    fn test_parse_http_host() {
        let buf = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let info = parse_first_bytes(buf);
        assert_eq!(info.host.as_deref(), Some("example.com"));
    }

    #[test]
    fn test_http_host_case_insensitive() {
        let buf = b"GET / HTTP/1.1\r\nhOsT: Example.COM\r\n\r\n";
        let info = parse_first_bytes(buf);
        assert_eq!(info.host.as_deref(), Some("Example.COM"));
    }

    #[test]
    fn test_host_scramble() {
        let buf = b"Host: Example.com\r\n";
        let scrambled = apply_host_scramble(buf);
        assert_eq!(String::from_utf8_lossy(&scrambled), "hOST: eXAMPLE.COM\r\n");
    }

    /// Minimal ama geçerli bir TLS ClientHello (SNI = "ex.com")
    fn make_client_hello(sni: &str) -> Vec<u8> {
        let sni_bytes = sni.as_bytes();
        // server_name extension verisi: list_len(2) type(1) name_len(2) name
        let name_len = sni_bytes.len() as u16;
        let list_entry_len = 1 + 2 + name_len;
        let list_len = list_entry_len;
        let mut sni_ext = Vec::new();
        sni_ext.extend_from_slice(&list_len.to_be_bytes());
        sni_ext.push(0x00); // host_name
        sni_ext.extend_from_slice(&name_len.to_be_bytes());
        sni_ext.extend_from_slice(sni_bytes);

        let mut ext = Vec::new();
        ext.extend_from_slice(&0x0000u16.to_be_bytes()); // server_name
        ext.extend_from_slice(&(sni_ext.len() as u16).to_be_bytes());
        ext.extend_from_slice(&sni_ext);
        let ext_total = ext.len() as u16;

        let mut body = Vec::new();
        body.extend_from_slice(&[0x03, 0x03]); // legacy_version
        body.extend_from_slice(&[0xAB; 32]); // random
        body.push(0x00); // session_id len
        body.extend_from_slice(&[0x00, 0x02, 0x00, 0x2f]); // cipher suites (len + 1 suite)
        body.push(0x01); // compression methods len
        body.push(0x00); // null compression
        body.extend_from_slice(&ext_total.to_be_bytes());
        body.extend_from_slice(&ext);

        let mut hs = Vec::new();
        hs.push(0x01); // client_hello
        let hs_len = body.len() as u32;
        hs.extend_from_slice(&hs_len.to_be_bytes()[1..]);
        hs.extend_from_slice(&body);

        let mut rec = Vec::new();
        rec.push(0x16);
        rec.extend_from_slice(&[0x03, 0x01]); // record version
        let rec_len = hs.len() as u16;
        rec.extend_from_slice(&rec_len.to_be_bytes());
        rec.extend_from_slice(&hs);
        rec
    }

    #[test]
    fn test_parse_tls_sni_absolute_offset() {
        let buf = make_client_hello("ex.com");
        let info = parse_first_bytes(&buf);
        assert_eq!(info.sni.as_deref(), Some("ex.com"));
        let off = info.sni_offset.unwrap();
        assert_eq!(&buf[off..off + 6], b"ex.com");
    }

    #[test]
    fn test_record_split_roundtrip() {
        let buf = make_client_hello("ex.com");
        let info = parse_first_bytes(&buf);
        let parts = apply_record_split(&buf, info.sni_offset.unwrap());
        assert_eq!(parts.len(), 2);
        // İki kayıt da TLS handshake content type ile başlamalı
        assert_eq!(parts[0][0], 0x16);
        assert_eq!(parts[1][0], 0x16);
        // Birleştirildiğinde handshake gövdesi orijinalle aynı olmalı
        let reassembled: Vec<u8> = parts.iter().flat_map(|p| p[5..].iter().copied()).collect();
        assert_eq!(reassembled, buf[5..5 + (((buf[3] as usize) << 8) | buf[4] as usize)]);
    }

    #[test]
    fn test_strategy_default_no_change() {
        let buf = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let info = parse_first_bytes(buf);
        let (addr, target) = dummy_addrs();
        let result = apply_strategy(&info, Strategy::Default, addr, target, &[], buf, None);
        assert_eq!(result.parts.len(), 1);
        assert_eq!(result.parts[0], buf);
    }
}
