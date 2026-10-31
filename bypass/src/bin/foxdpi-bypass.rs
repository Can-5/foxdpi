use std::net::SocketAddr;
use anyhow::Result;
use clap::Parser;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn, error};

/// foxdpi-bypass — kendi DPI bypass motorumuz (şablon)
#[derive(Parser, Debug)]
#[command(name = "foxdpi-bypass", about = "FoxDPI'nin kendi DPI bypass motoru (şablon aşamasında)")]
struct Cli {
    /// Dinlenen port (öntanımlı: 10850)
    #[arg(short, long, default_value = "10850")]
    port: u16,

    /// Biçimlendirme strateji (şablon: default, split, desync, decoy)
    #[arg(long, default_value = "default")]
    mode: String,

    /// Log seviyesi (info, debug, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Parantezli debug (şablon: istekleri yazdır)
    #[arg(long, default_value_t = false)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Log seviyesi (taslak)
    tracing_subscriber::fmt()
        .with_env_filter(cli.log_level.clone())
        .init();

    info!("foxdpi-bypass şablon başlatıldı; mode={}", cli.mode);

    let addr: SocketAddr = format!("127.0.0.1:{}", cli.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("dinlemede: {}", addr);

    loop {
        let (mut socket, peer) = listener.accept().await?;
        info!("yeni bağlantı: {}", peer);

        let mode = cli.mode.clone();
        let verbose = cli.verbose;

        tokio::spawn(async move {
            if let Err(e) = handle_connection(&mut socket, peer, &mode, verbose).await {
                error!("bağlantı hatası ({}): {}", peer, e);
            }
        });
    }
}

async fn handle_connection(
    socket: &mut tokio::net::TcpStream,
    peer: SocketAddr,
    mode: &str,
    verbose: bool,
) -> Result<()> {
    // şablon: ilk baytları okup logla, gerçek implementasyonda
    // ClientHello parse + strateji uygula
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;
    if n == 0 {
        return Ok(()); // bağlantı kapandı
    }

    if verbose || mode != "default" {
        warn!("<- {} {} byte (mode={})", peer, n, mode);
    }

    // bu noktada:
    // - ilk TLS ClientHello'yı parse et
    // - SNI/Host pozisyonunu bul
    // - seçilen stratejiyi uygula (record split, segment desync, decoy, OOB, Host scramble)
    // - ileri kısımda connect ve forward

    // şablon forward: aynı veriyi geri gönder (dummy)
    socket.write_all(&buf[..n]).await?;
    info!("-> {} forward (şablon)", peer);

    Ok(())
}
