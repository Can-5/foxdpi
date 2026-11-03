//! Engine — TLS ClientHello parse + strateji uygula

use crate::config::Strategy;
use crate::rules::{effective_strategy, Rule};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Bağlantı üzerinden okunan ilk baytlar üzerinden ClientHello parse sonuçları.
#[derive(Debug, Clone)]
pub struct ClientHelloInfo {
    pub sni: Option<String>,
    pub host: Option<String>,
    pub sni_offset: Option<usize>,
    pub first_n: usize,
}

pub fn parse_first_bytes(buf: &[u8]) -> ClientHelloInfo {
    if buf.len() < 2 {
        return ClientHelloInfo {
            sni: None,
            host: None,
            sni_offset: None,
            first_n: buf.len(),
        };
    }
    if buf[0] == 0x16 {
        parse_tls_client_hello(buf)
    } else if buf[0] == 0x17 {
        ClientHelloInfo {
            sni: None,
            host: None,
            sni_offset: None,
            first_n: buf.len(),
        }
    } else {
        parse_http_host(buf)
    }
}

fn parse_tls_client_hello(buf: &[u8]) -> ClientHelloInfo {
    if buf.len() < 5 {
        return ClientHelloInfo {
            sni: None,
            host: None,
            sni_offset: None,
            first_n: buf.len(),
        };
    }
    let record_len =
        ((buf[3] as u16) << 8) | (buf[4] as u16);
    let end = 5 + record_len as usize;
    let handshake = &buf[5..std::cmp::min(end, buf.len())];
    if handshake.len() < 4 {
        return ClientHelloInfo {
            sni: None,
            host: None,
            sni_offset: None,
            first_n: buf.len(),
        };
    }
    let hs_len = ((handshake[1] as u32) << 16)
        | ((handshake[2] as u32) << 8)
        | (handshake[3] as u32);
    let hs_end = 4 + hs_len as usize;
    let hs_body =
        &handshake[4..std::cmp::min(hs_end, handshake.len())];
    let sni_result = extract_sni(hs_body);
    let sni_offset = sni_result.as_ref().map(|v| v.1);
    ClientHelloInfo {
        sni: sni_result.as_ref().map(|(name, _)| name.clone()),
        host: sni_result.as_ref().map(|(name, _)| name.clone()),
        sni_offset,
        first_n: buf.len(),
    }
}

fn extract_sni(hs_body: &[u8]) -> Option<(String, usize)> {
    if hs_body.len() < 38 {
        return None;
    }
    let mut pos = 38usize;
    if pos >= hs_body.len() {
        return None;
    }
    let session_id_len = hs_body[pos] as usize;
    pos += 1 + session_id_len;
    if pos + 2 > hs_body.len() {
        return None;
    }
    let csuite_len =
        ((hs_body[pos] as usize) << 8) | (hs_body[pos + 1] as usize);
    pos += 2 + csuite_len;
    if pos >= hs_body.len() {
        return None;
    }
    let cm_len = hs_body[pos] as usize;
    pos += 1 + cm_len;
    if pos + 2 > hs_body.len() {
        return None;
    }
    let ext_total =
        ((hs_body[pos] as usize) << 8) | (hs_body[pos + 1] as usize);
    pos += 2;
    let ext_end = std::cmp::min(pos + ext_total, hs_body.len());
    let mut ext_pos = pos;
    while ext_pos + 4 <= ext_end {
        let ext_type =
            ((hs_body[ext_pos] as u16) << 8) | (hs_body[ext_pos + 1] as u16);
        let ext_len =
            ((hs_body[ext_pos + 2] as u16) << 8) | (hs_body[ext_pos + 3] as u16);
        if ext_type == 0x0000 {
            let sni_data_start = ext_pos + 4;
            let sni_data = &hs_body[sni_data_start..std::cmp::min(
                sni_data_start + ext_len as usize,
                hs_body.len(),
            )];
            if sni_data.len() < 2 {
                break;
            }
            let name_list_len =
                ((sni_data[0] as usize) << 8) | (sni_data[1] as usize);
            let name_entries =
                &sni_data[2..std::cmp::min(2 + name_list_len, sni_data.len())];
            if name_entries.is_empty() {
                break;
            }
            if name_entries.len() < 3 {
                break;
            }
            let name_len =
                ((name_entries[1] as usize) << 8) | (name_entries[2] as usize);
            let name_start = 3;
            if name_start + name_len > name_entries.len() {
                break;
            }
            let name_bytes = &name_entries[name_start..name_start + name_len];
            return Some((
                String::from_utf8_lossy(name_bytes).into_owned(),
                sni_data_start + 2 + 3 + name_len,
            ));
        }
        ext_pos += 4 + ext_len as usize;
    }
    None
}

fn parse_http_host(buf: &[u8]) -> ClientHelloInfo {
    if let Some(pos) = buf.windows(5).position(|w| w == b"Host:") {
        let start = pos + 5;
        let end = buf[start..]
            .iter()
            .position(|&b| b == b'\r' || b == b'\n')
            .map(|p| start + p)
            .unwrap_or(buf.len());
        let host = String::from_utf8_lossy(&buf[start..end]).trim().to_string();
        return ClientHelloInfo {
            sni: if host.is_empty() {
                None
            } else {
                Some(host.clone())
            },
            host: if host.is_empty() {
                None
            } else {
                Some(host)
            },
            sni_offset: None,
            first_n: buf.len(),
        };
    }
    ClientHelloInfo {
        sni: None,
        host: None,
        sni_offset: None,
        first_n: buf.len(),
    }
}

/// Manage january altında parçaları yeniden derler; gerçek zamanlı çoğaltma yerine,
/// ilkel yeniden sıralama/disorder mantığını örnekler.
pub fn apply_segment_desync(original: &[u8], chunk_sizes: &[usize]) -> Vec<Vec<u8>> {
    let chunks = split_into_chunks(original, chunk_sizes);
    disorder_chunks(chunks)
}

fn split_into_chunks(original: &[u8], chunk_sizes: &[usize]) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    for &size in chunk_sizes {
        if offset >= original.len() {
            break;
        }
        let end = (offset + size).min(original.len());
        chunks.push(original[offset..end].to_vec());
        offset = end;
    }
    if offset < original.len() {
        chunks.push(original[offset..].to_vec());
    }
    chunks
}

fn disorder_chunks(mut chunks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    if chunks.len() <= 1 {
        return chunks;
    }
    // deterministic disorder pattern: rotate chunks by a fixed offset
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    chunks.len().hash(&mut h);
    let rotate_by = h.finish() as usize % chunks.len();
    if rotate_by == 0 {
        return chunks;
    }
    let mut out = Vec::with_capacity(chunks.len());
    out.extend_from_slice(&chunks[rotate_by..]);
    out.extend_from_slice(&chunks[..rotate_by]);
    out
}

/// HTTP Host scramble: Host başlığının case'ini değiştirir + delimiter varyasyonları.
pub fn apply_host_scramble(buf: &[u8]) -> Vec<u8> {
    let mut out = buf.to_vec();
    if let Some(pos) = out.windows(5).position(|w| w == b"Host:") {
        let start = pos + 5;
        let end = out[start..]
            .iter()
            .position(|&b| b == b'\r' || b == b'\n')
            .map(|p| start + p)
            .unwrap_or(out.len());
        // case flip within the host value
        for b in &mut out[start..end] {
            if *b >= b'a' && *b <= b'z' {
                *b -= 32;
            } else if *b >= b'A' && *b <= b'Z' {
                *b += 32;
            }
        }
        // delimiter perturbation: inject an extra space if room and no CRLF conflict
        if end < out.len() && out[end] == b'\r' && out.len() > end + 1 && out[end + 1] == b'\n' {
            out.insert(end, b' ');
        }
    }
    out
}

pub struct StrategyResult {
    pub mode: Strategy,
    pub parts: Vec<Vec<u8>>,
    pub decoy_needed: bool,
    pub oob_byte: Option<u8>,
    pub ttl_override: Option<u8>,
}

/// Strateji uygulanmış, forward edilecek parçaları döndürür.
pub fn apply_strategy(
    info: &ClientHelloInfo,
    mode: Strategy,
    addr: SocketAddr,
    target: SocketAddr,
    rules: &[Rule],
    original: &[u8],
) -> StrategyResult {
    let effective = effective_strategy(info, mode, rules);

    match effective {
        Strategy::Default => StrategyResult {
            mode: effective,
            parts: vec![original.to_vec()],
            decoy_needed: false,
            oob_byte: None,
            ttl_override: None,
        },
        Strategy::RecordSplit => {
            let split_point = info.sni_offset.unwrap_or_else(|| original.len() / 2);
            let parts =
                apply_record_split(original, split_point).unwrap_or_else(|_| vec![original.to_vec()]);
            StrategyResult {
                mode: effective,
                parts,
                decoy_needed: false,
                oob_byte: None,
                ttl_override: Some(60),
            }
        }
        Strategy::SegmentDesync => {
            let chunks: Vec<usize> = (0..original.len())
                .step_by(4)
                .map(|i| (i + 4).min(original.len()) - i)
                .collect();
            let parts = apply_segment_desync(original, &chunks);
            StrategyResult {
                mode: effective,
                parts,
                decoy_needed: false,
                oob_byte: None,
                ttl_override: Some(64),
            }
        }
        Strategy::Decoy => StrategyResult {
            mode: effective,
            parts: vec![original.to_vec()],
            decoy_needed: true,
            oob_byte: None,
            ttl_override: Some(120),
        },
        Strategy::OOB => {
            let oob_byte = original.first().copied();
            let parts: Vec<Vec<u8>> = match oob_byte {
                Some(_) if original.len() > 1 => {
                    vec![original[..1].to_vec(), original[1..].to_vec()]
                }
                _ => vec![original.to_vec()],
            };
            StrategyResult {
                mode: effective,
                parts,
                decoy_needed: false,
                oob_byte,
                ttl_override: None,
            }
        }
        Strategy::HostScramble => {
            let scrambled = apply_host_scramble(original);
            StrategyResult {
                mode: effective,
                parts: vec![scrambled],
                decoy_needed: false,
                oob_byte: None,
                ttl_override: None,
            }
        }
    }
}

fn apply_record_split(original: &[u8], split_point: usize) -> std::io::Result<Vec<Vec<u8>>> {
    if original.len() < 5 {
        return Ok(vec![original.to_vec()]);
    }
    let record_len =
        ((original[3] as u16) << 8) | (original[4] as u16);
    let total_record_len = 5 + record_len as usize;
    let handshake = if total_record_len <= original.len() {
        &original[5..total_record_len]
    } else {
        &original[5..original.len()]
    };
    if split_point >= handshake.len() || split_point == 0 {
        return Ok(vec![original.to_vec()]);
    }
    let parts = {
        let part1_handshake = &handshake[..split_point];
        let part2_handshake = &handshake[split_point..];
        let mut p1 = original[..5].to_vec();
        let len1 = part1_handshake.len() as u16;
        p1.extend_from_slice(&len1.to_be_bytes());
        p1.extend_from_slice(part1_handshake);
        let mut out = vec![p1];
        if !part2_handshake.is_empty() {
            let mut p2 = Vec::with_capacity(5 + part2_handshake.len());
            p2.push(0x16);
            p2.extend_from_slice(&[0x03, 0x03]);
            let len2 = part2_handshake.len() as u16;
            p2.extend_from_slice(&len2.to_be_bytes());
            p2.extend_from_slice(part2_handshake);
            out.push(p2);
        }
        out
    };
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_parse_http_host() {
        let buf = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let info = parse_first_bytes(buf);
        assert_eq!(info.host, Some("example.com".to_string()));
    }

    #[test]
    fn test_host_scramble() {
        let buf = b"Host: Example.com\r\n";
        let scrambled = apply_host_scramble(buf);
        assert_eq!(
            String::from_utf8_lossy(&scrambled),
            "HoST: eXAMPLE.CoM\r\n"
        );
    }

    #[test]
    fn test_strategy_default_no_change() {
        let buf = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let info = parse_first_bytes(buf);
        let addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            10850,
        );
        let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443);
        let result = apply_strategy(&info, Strategy::Default, addr, target, &[], buf);
        assert_eq!(result.mode, Strategy::Default);
        assert_eq!(result.parts.len(), 1);
        assert_eq!(result.parts[0], buf);
        assert!(!result.decoy_needed);
        assert!(result.oob_byte.is_none());
    }
}
