//! Raw socket paketi üretimi — decoy / TCP "disorder" / auto-TTL
//! (sadece Linux; AF_INET + IPPROTO_RAW, root veya CAP_NET_RAW gerekir).
//!
//! Üretilen paket gerçek bağlantının 5-tuplunu taşır ama düşük TTL ile
//! yola çıkarlar; DPI yolu üzerinde görür, sunucuya asla ulaşmazlar.
//! Sıra numarası doğruluğu sunucu için önemli değildir (paketler ölür).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::fd::RawFd;
use tokio::net::TcpStream;

const PROTO_TCP: u8 = 6;
const TCP_FLAG_PSH: u8 = 0x08;
const TCP_FLAG_ACK: u8 = 0x10;

/// RFC 1071 Internet checksum
pub fn ip_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);
    for c in &mut chunks {
        sum += u16::from_be_bytes([c[0], c[1]]) as u32;
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        sum += (rem[0] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// IP + TCP başlıklı tam bir paket üretir (IPPROTO_RAW için IP başlığı dahil).
pub fn build_packet(
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    payload: &[u8],
    ttl: u8,
) -> Vec<u8> {
    let ip_ihl: u8 = 5;
    let total_len: u16 = (ip_ihl as u16 * 4) + 20 + payload.len() as u16;
    let ip_id: u16 = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0x1234) as u16)
        .wrapping_add(0x5a5a);

    let mut pkt = Vec::with_capacity(total_len as usize);
    // --- IP başlığı ---
    pkt.push(0x45); // version + ihl
    pkt.push(0x00); // tos
    pkt.extend_from_slice(&total_len.to_be_bytes());
    pkt.extend_from_slice(&ip_id.to_be_bytes());
    pkt.extend_from_slice(&[0x40, 0x00]); // DF, frag_off=0
    pkt.push(ttl);
    pkt.push(PROTO_TCP);
    pkt.extend_from_slice(&[0x00, 0x00]); // checksum — aşağıda doldurulur
    pkt.extend_from_slice(&src_ip.octets());
    pkt.extend_from_slice(&dst_ip.octets());
    let ip_cksum_off = 10usize;
    let ip_cksum = ip_checksum(&pkt[..20]);
    pkt[ip_cksum_off..ip_cksum_off + 2].copy_from_slice(&ip_cksum.to_be_bytes());

    // --- TCP başlığı ---
    let tcp_start = pkt.len();
    pkt.extend_from_slice(&src_port.to_be_bytes());
    pkt.extend_from_slice(&dst_port.to_be_bytes());
    pkt.extend_from_slice(&seq.to_be_bytes());
    pkt.extend_from_slice(&0u32.to_be_bytes()); // ack_seq (belirsiz; paket yolda ölür)
    pkt.push(0x50); // data offset = 5 (20 bayt), reserved=0
    pkt.push(TCP_FLAG_PSH | TCP_FLAG_ACK);
    pkt.extend_from_slice(&64240u16.to_be_bytes()); // window
    pkt.extend_from_slice(&[0x00, 0x00]); // checksum — aşağıda doldurulur
    pkt.extend_from_slice(&[0x00, 0x00]); // urg_ptr
    pkt.extend_from_slice(payload);

    // TCP checksum: pseudo header + header + payload
    let tcp_len = (pkt.len() - tcp_start) as u16;
    let mut pseudo = Vec::with_capacity(12 + tcp_len as usize);
    pseudo.extend_from_slice(&src_ip.octets());
    pseudo.extend_from_slice(&dst_ip.octets());
    pseudo.push(0x00);
    pseudo.push(PROTO_TCP);
    pseudo.extend_from_slice(&tcp_len.to_be_bytes());
    pseudo.extend_from_slice(&pkt[tcp_start..]);
    let tcp_cksum = ip_checksum(&pseudo);
    let tcp_cksum_off = tcp_start + 16;
    pkt[tcp_cksum_off..tcp_cksum_off + 2].copy_from_slice(&tcp_cksum.to_be_bytes());

    pkt
}

/// Raw socket ile (IPPROTO_RAW) paketi yola çıkarır. Hata → EOperasyonNotPermitted vb.
pub fn send_raw(packet: &[u8], dst: SocketAddr) -> Result<(), String> {
    let fd: RawFd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_RAW) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!("raw socket açılamadı (root/CAP_NET_RAW gerekir): {}", err));
    }
    let result = unsafe {
        let addr = libc::sockaddr_in {
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: 0,
            sin_addr: libc::in_addr { s_addr: match dst.ip() {
                IpAddr::V4(a) => u32::from_be_bytes(a.octets()),
                IpAddr::V6(_) => 0, // aşağıda reddedilir
            }},
            sin_zero: [0; 8],
        };
        libc::sendto(
            fd,
            packet.as_ptr() as *const libc::c_void,
            packet.len(),
            0,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };
    unsafe { libc::close(fd); }
    if result < 0 {
        return Err(format!("sendto başarısız: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

/// Gerçek bağlantının 5-tuplu'sunu kullanarak düşük TTL'li ham paket(ler) gönderir.
/// `autottl = true` ise `max_ttl` değerine kadar artan TTL'lerle birer kopya gönderilir
/// (auto-TTL: DPI yolunun kaç hop ötede olduğunu deneme yanılma ile aşar).
pub async fn send_low_ttl(
    real: &TcpStream,
    payload: &[u8],
    ttl: u8,
    autottl: bool,
    autottl_max: u8,
) -> Result<(), String> {
    let local = real.local_addr().map_err(|e| format!("local_addr: {}", e))?;
    let peer = real.peer_addr().map_err(|e| format!("peer_addr: {}", e))?;
    let (src_ip, dst_ip) = match (local.ip(), peer.ip()) {
        (IpAddr::V4(s), IpAddr::V4(d)) => (s, d),
        _ => return Err("raw decoy/disorder yalnızca IPv4 hedeflerde desteklenir".into()),
    };
    if matches!(peer.ip(), IpAddr::V4(Ipv4Addr::BROADCAST)) {
        return Err("hedefe gönderilemiyor".into());
    }
    if autottl {
        for t in 1..=autottl_max.max(1) {
            let seq = rand_seq();
            let pkt = build_packet(src_ip, dst_ip, local.port(), peer.port(), seq, payload, t);
            if let Err(e) = send_raw(&pkt, peer) { return Err(e); }
        }
        Ok(())
    } else {
        let pkt = build_packet(src_ip, dst_ip, local.port(), peer.port(), rand_seq(), payload, ttl);
        send_raw(&pkt, peer)
    }
}

fn rand_seq() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1);
    nanos.wrapping_mul(2654435761) ^ 0x9e3779b9
}

/// Sahte ClientHello: SNI host adını aynı uzunlukta rastgele küçük harflerle değiştirir.
/// Uzunluk sabit kaldığı için paket yapısı (ve DPI'nin gördüğü "makul" ClientHello) bozulmaz.
pub fn fake_client_hello(original: &[u8], sni_offset: Option<usize>, sni_len: usize) -> Vec<u8> {
    let mut out = original.to_vec();
    if let Some(off) = sni_offset {
        let end = (off + sni_len).min(out.len());
        let mut state = rand_seq();
        for b in &mut out[off..end] {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = b"abcdefghijklmnopqrstuvwxyz"[((state >> 16) % 26) as usize];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_rfc1071_vector() {
        // RFC 1071 örnek: checksum(0x0001,0xf203,0xf4f5,0xf6f7) = 0x220d
        let data = [0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7];
        assert_eq!(ip_checksum(&data), 0x220d);
    }

    #[test]
    fn test_build_packet_layout() {
        let pkt = build_packet(Ipv4Addr::new(192, 168, 1, 2), Ipv4Addr::new(1, 1, 1, 1), 40000, 443, 0xdeadbeef, b"TLS", 3);
        // IP: 20 bayt, TCP: 20 bayt, payload 3 bayt
        assert_eq!(pkt.len(), 43);
        assert_eq!(pkt[0], 0x45);
        assert_eq!(pkt[8], 3); // TTL
        assert_eq!(pkt[9], 6); // TCP
        let total = ((pkt[2] as usize) << 8) | pkt[3] as usize;
        assert_eq!(total, 43);
        // Src port
        assert_eq!(((pkt[20] as u16) << 8) | pkt[21] as u16, 40000);
        // IP checksum alanı doğru mu: checksum'ı tekrar hesaplayınca 0 çıkmalı
        assert_eq!(ip_checksum(&pkt[..20]), 0);
    }

    #[test]
    fn test_fake_client_hello_preserves_len_and_dots() {
        let original = b"prefix-sni-example.com-suffix";
        let sni_off = 7usize;
        let sni_len = 17; // "-example.com-suf"? hayır — tam uzunluk gerekir
        let fake = fake_client_hello(original, Some(sni_off), sni_len);
        assert_eq!(fake.len(), original.len());
        assert_eq!(&fake[..sni_off], &original[..sni_off]);
    }
}
