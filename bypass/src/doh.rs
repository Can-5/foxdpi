//! DoH — DNS-over-HTTPS (RFC 8484) çözümlemesi.
//!
//! Sunucuyla TLS (webpki-roots CA'ları ile doğrulama) kurulur, POST
//! `application/dns-message` ile sorgu gider, cevap DNS wire formatında alınır
//! ve mevcut encode/parse rutinleriyle (dns.rs) açılır. Paletsiz ek
//! bağımlılık olmadan HTTP/1.1 üzerinden uygulanır.

use crate::dns::{encode_query, parse_response, DnsError};
use rustls::pki_types::ServerName;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsConnector;
use tracing::debug;

/// Anahtarlanmış DoH uç noktaları: (TLS SNI / Host, bağlanılacak IP, yol).
pub struct DohEndpoint {
    pub host: &'static str,
    pub ip: &'static str,
    pub path: &'static str,
}

pub const BUILTIN_DOH: &[DohEndpoint] = &[
    DohEndpoint { host: "cloudflare-dns.com", ip: "1.1.1.1", path: "/dns-query" },
    DohEndpoint { host: "dns.google", ip: "8.8.8.8", path: "/dns-query" },
    DohEndpoint { host: "dns.quad9.net", ip: "9.9.9.9", path: "/dns-query" },
];

fn tls_connector() -> TlsConnector {
    static CONNECTOR: OnceLock<TlsConnector> = OnceLock::new();
    CONNECTOR
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            for c in webpki_roots::TLS_SERVER_ROOTS.iter() {
                roots.roots.push(c.clone());
            }
            let config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            TlsConnector::from(Arc::new(config))
        })
        .clone()
}

fn rand_txid() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos() as u16).unwrap_or(0xbeef);
    nanos ^ (std::process::id() as u16) ^ 0x51ab
}

/// Tek bir DoH uç noktasına sorgu atar.
pub async fn query(domain: &str, endpoint: &DohEndpoint, qtype: u16) -> Result<IpAddr, DnsError> {
    let txid = rand_txid();
    let body = encode_query(domain, txid, qtype)?;

    let addr: SocketAddr = format!("{}:443", endpoint.ip)
        .parse()
        .map_err(|e| DnsError::ParseError(format!("DoH IP parse edilemedi: {}", e)))?;

    let tcp = tokio::time::timeout(Duration::from_secs(6), tokio::net::TcpStream::connect(addr))
        .await
        .map_err(|_| DnsError::Timeout)??;
    tcp.set_nodelay(true).ok();

    let server_name = ServerName::try_from(endpoint.host.to_string())
        .map_err(|e| DnsError::ParseError(format!("DoH sunucu adı geçersiz: {}", e)))?;
    let mut tls = tokio::time::timeout(Duration::from_secs(6), tls_connector().connect(server_name, tcp))
        .await
        .map_err(|_| DnsError::Timeout)??;

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/dns-message\r\nAccept: application/dns-message\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",
        path = endpoint.path,
        host = endpoint.host,
        len = body.len()
    );
    tls.write_all(request.as_bytes()).await?;
    tls.write_all(&body).await?;
    tls.flush().await?;

    // Cevabı oku (Connection: close → EOF'a kadar)
    let mut raw = Vec::with_capacity(4096);
    let _ = tokio::time::timeout(Duration::from_secs(6), tls.read_to_end(&mut raw)).await;

    // HTTP cevap başlıklarını gövdeden ayır
    let sep = find_subslice(&raw, b"\r\n\r\n").ok_or_else(|| DnsError::ParseError("DoH: HTTP başlıkları ayrıştırılamadı".into()))?;
    let head = String::from_utf8_lossy(&raw[..sep]).to_string();
    let mut body_bytes = raw[sep + 4..].to_vec();

    let status_line = head.lines().next().unwrap_or("");
    if !status_line.contains(" 200") {
        return Err(DnsError::ParseError(format!("DoH: beklenmeyen HTTP durumu: {}", status_line.trim())));
    }

    // Transfer-Encoding: chunked ise dechunk et
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        body_bytes = dechunk(&body_bytes)?;
    }

    debug!("DoH cevabı alındı ({} bayt) {} -> {}", body_bytes.len(), domain, endpoint.host);
    parse_response(&body_bytes, txid, qtype)
}

/// Kullanıcı tanımlı DoH uç noktası: "https://host/path" veya "host/path".
/// IPv4 SNI için aynı host kullanılamaz; buradan çözülemez — bu yüzden
/// özel uç noktalarda da host adıyla bağlanmaya çalışırız (sistem DNS'i devreye girer).
pub async fn query_custom(domain: &str, url: &str, qtype: u16) -> Result<IpAddr, DnsError> {
    let (host, path) = parse_doh_url(url)?;
    let txid = rand_txid();
    let body = encode_query(domain, txid, qtype)?;

    let addr = tokio::net::lookup_host(format!("{}:443", host))
        .await
        .map_err(|e| DnsError::ParseError(format!("DoH host çözümlenemedi ({}): {}", host, e)))?
        .next()
        .ok_or_else(|| DnsError::ParseError(format!("DoH host için kayıt yok: {}", host)))?;

    let tcp = tokio::time::timeout(Duration::from_secs(6), tokio::net::TcpStream::connect(addr))
        .await
        .map_err(|_| DnsError::Timeout)??;
    tcp.set_nodelay(true).ok();

    let server_name = ServerName::try_from(host.clone())
        .map_err(|e| DnsError::ParseError(format!("DoH sunucu adı geçersiz: {}", e)))?;
    let mut tls = tokio::time::timeout(Duration::from_secs(6), tls_connector().connect(server_name, tcp))
        .await
        .map_err(|_| DnsError::Timeout)??;

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/dns-message\r\nAccept: application/dns-message\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",
        path = path, host = host, len = body.len()
    );
    tls.write_all(request.as_bytes()).await?;
    tls.write_all(&body).await?;
    tls.flush().await?;

    let mut raw = Vec::with_capacity(4096);
    let _ = tokio::time::timeout(Duration::from_secs(6), tls.read_to_end(&mut raw)).await;

    let sep = find_subslice(&raw, b"\r\n\r\n").ok_or_else(|| DnsError::ParseError("DoH: HTTP başlıkları ayrıştırılamadı".into()))?;
    let head = String::from_utf8_lossy(&raw[..sep]).to_string();
    let mut body_bytes = raw[sep + 4..].to_vec();

    let status_line = head.lines().next().unwrap_or("");
    if !status_line.contains(" 200") {
        return Err(DnsError::ParseError(format!("DoH: beklenmeyen HTTP durumu: {}", status_line.trim())));
    }
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        body_bytes = dechunk(&body_bytes)?;
    }
    parse_response(&body_bytes, txid, qtype)
}

fn parse_doh_url(url: &str) -> Result<(String, String), DnsError> {
    let u = url.trim();
    let u = u.strip_prefix("https://").unwrap_or(u);
    match u.split_once('/') {
        Some((host, rest)) => Ok((host.to_string(), format!("/{}", rest))),
        None => Ok((u.to_string(), "/dns-query".to_string())),
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn dechunk(body: &[u8]) -> Result<Vec<u8>, DnsError> {
    let mut out = Vec::with_capacity(body.len());
    let mut pos = 0usize;
    while pos < body.len() {
        let line_end = body[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .map(|p| pos + p)
            .ok_or_else(|| DnsError::ParseError("chunked: satır sonu yok".into()))?;
        let size_str = std::str::from_utf8(&body[pos..line_end]).map_err(|_| DnsError::ParseError("chunked: boyut geçersiz".into()))?;
        let size = usize::from_str_radix(size_str.trim(), 16).map_err(|_| DnsError::ParseError(format!("chunked: boyut parse edilemedi: {}", size_str)))?;
        pos = line_end + 2;
        if size == 0 { break; }
        if pos + size > body.len() { return Err(DnsError::ParseError("chunked: gövde taşması".into())); }
        out.extend_from_slice(&body[pos..pos + size]);
        pos += size;
        if body[pos..].starts_with(b"\r\n") { pos += 2; }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dechunk() {
        let chunked = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
        assert_eq!(dechunk(chunked).unwrap(), b"Wikipedia");
    }

    #[test]
    fn test_parse_doh_url() {
        assert_eq!(parse_doh_url("https://dns.google/dns-query").unwrap(), ("dns.google".to_string(), "/dns-query".to_string()));
        assert_eq!(parse_doh_url("dns.example.com").unwrap(), ("dns.example.com".to_string(), "/dns-query".to_string()));
    }

    #[test]
    fn test_find_subslice() {
        assert_eq!(find_subslice(b"abc\r\n\r\ndef", b"\r\n\r\n"), Some(3));
        assert_eq!(find_subslice(b"abc", b"\r\n\r\n"), None);
    }
}
