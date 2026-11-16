//! Sinkhole — engellenen reklam/izleyici domain'lerine bağlanan istemcileri
//! yerel bir mini HTTP sunucuya yönlendirir; boş 204 No Content döner.
//! (HTTPS'e dokunamaz: TLS handshake bu sunucu tarafından tamamlanamaz;
//! tarayıcı bağlantı hatası görür. HTTP istekleri temiz 204 alır.)

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, info};

pub struct Sinkhole {
    port: u16,
    stats: Arc<tokio::sync::Mutex<u64>>,
}

impl Sinkhole {
    pub fn new(port: u16) -> Self {
        Self { port, stats: Arc::new(tokio::sync::Mutex::new(0)) }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(("127.0.0.1", self.port)).await?;
        info!("Reklam sinkhole dinleniyor: 127.0.0.1:{} (204 No Content)", self.port);
        loop {
            let (mut sock, peer) = listener.accept().await?;
            let stats = self.stats.clone();
            tokio::spawn(async move {
                // İsteği oku (en fazla 8KB, sonsuza kadar bekleme)
                let mut buf = vec![0u8; 8192];
                let mut read = 0usize;
                loop {
                    match tokio::time::timeout(std::time::Duration::from_secs(2), sock.read(&mut buf[read..])).await {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => {
                            read += n;
                            if read >= 4 && buf[..read].windows(4).any(|w| w == b"\r\n\r\n") { break; }
                            if read == buf.len() { break; }
                        }
                        _ => break,
                    }
                }
                let response = b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = sock.write_all(response).await;
                let mut guard = stats.lock().await;
                *guard += 1;
                debug!("sinkhole: 204 dönüşü (toplam {} istek, peer {})", *guard, peer);
            });
        }
    }
}


