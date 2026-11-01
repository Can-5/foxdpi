use foxdpi_bypass::{
    apply_strategy, ClientHelloInfo, Config, DnsResolver, MatchKind, Rule, Strategy, SocksProxy,
};
use std::net::SocketAddr;

#[derive(clap::Parser)]
#[command(name = "foxdpi-bypass", about = "FoxDPI DPI bypass motoru (TLS record split / segment desync / decoy / OOB / host-scramble)")]
struct Cli {
    /// SOCKS5 proxy bind port
    #[arg(long, default_value_t = 1080)]
    port: u16,

    /// varsayılan bypass stratejisi
    #[arg(long, value_enum, default_value_t = clap::value_hint::ValueHint::ThatSortsLikeAnEnum)]
    mode: Option<Strategy>,

    /// domain kuralları (tekrarlanabilir)
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    rules: Option<Vec<String>>,

    /// DNS provide sint
    #[arg(long)]
    dns_provider: Option<String>,

    /// DNS enkripsiyonu
    #[arg(long, alias = "dns-encrypted")]
    dns_encrypted: bool,

    /// ad/resim engelleme listesi (virgülle bazlı)
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    block_pages: Option<Vec<String>>,

    /// config.yaml yol
    #[arg(long, alias = "config")]
    cfg_path: Option<String>,

    /// ileri düzlem (advanced mode) devreye al
    #[arg(long, alias = "advanced")]
    advanced: bool,

    /// SOCKS5 proxy başlat (varsayılan)
    #[arg(long)]
    run: bool,

    /// yapılandırmayı göster
    #[arg(long)]
    show: bool,

    /// DNS çözümleyiciyi test et (domain argümanı)
    #[arg(long)]
    dns_test: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    let mut cfg_path = cli.cfg_path.unwrap_or_else(|| "config.yaml".into());
    let path: std::path::PathBuf = if cfg_path.is_absolute() {
        cfg_path.clone().into()
    } else {
        let cwd = std::env::current_dir()?;
        cwd.join(cfg_path)
    };

    let mut config = Config::load(cfg_path.into())?;

    if let Some(m) = cli.mode {
        config.mode = m;
    }
    config.port = cli.port;
    config.advanced_mode = cli.advanced;
    config.enabled = true;

    if let Some(ref rp) = cli.rules {
        let mut rules = Vec::new();
        for spec in rp {
            let parts: Vec<&str> = spec.splitn(2, '=').collect();
            let domain = parts.first().unwrap_or(&"").to_string();
            if domain.is_empty() {
                continue;
            }
            let strategy = parts
                .get(1)
                .and_then(|s| s.parse::<Strategy>().ok())
                .unwrap_or(Strategy::Default);
            rules.push(Rule { domain, exempt: false, strategy: Some(strategy) });
        }
        config.rules = rules;
    }

    if let Some(ref prov) = cli.dns_provider {
        if let Ok(p) = prov.parse::<crate::DnsProvider>() {
            config.dns.provider = p;
        }
    }
    config.dns.encrypted = cli.dns_encrypted;
    if let Some(ref pages) = cli.block_pages {
        config.dns.block_pages = pages.clone();
    }

    // config kaydet sadece show değilse
    if !cli.show {
        let text = serde_yaml::to_string(&config)?;
        std::fs::write(&path, text)?;
    }

    if cli.show {
        println!("{}", serde_yaml::to_string(&config)?);
        return Ok(());
    }

    if let Some(domain) = cli.dns_test {
        let resolver = DnsResolver::new(config.dns.clone());
        match resolver.resolve(domain).await {
            Ok(ip) => println!("DNS: {} -> {}", domain, ip),
            Err(e) => eprintln!("DNS hata: {} -> {}", domain, e),
        }
        return Ok(());
    }

    if !cli.run {
        println!("FoxDPI bypass motoru hazır. --run ile başlatın.");
        println!("Port: {}, Mode: {}", config.port, config.mode);
        return Ok(());
    }

    println!("FoxDPI bypass başlatılıyor: 127.0.0.1:{}", cli.port);
    let proxy = SocksProxy::new(config.clone());
    proxy.run().await?;
    Ok(())
}

#[test]
fn test_rule_parse_basic() {
    let spec = "example.com=record-split";
    let parts: Vec<&str> = spec.splitn(2, '=').collect();
    assert_eq!(parts[0], "example.com");
    let strategy: Strategy = parts[1].parse().unwrap();
    assert_eq!(strategy, Strategy::RecordSplit);
}

#[test]
fn test_rule_parse_default() {
    let spec = "example.com";
    let parts: Vec<&str> = spec.splitn(2, '=').collect();
    let strategy: Strategy = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(Strategy::Default);
    assert_eq!(strategy, Strategy::Default);
}
