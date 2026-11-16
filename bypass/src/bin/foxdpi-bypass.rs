//! foxdpi-bypass — FoxDPI'nin kendi DPI/sansür atlatma motoru
//!
//! Kullanım:
//!   foxdpi-bypass                      # SOCKS5 proxy başlat (127.0.0.1:1080)
//!   foxdpi-bypass --mode record-split --port 10850
//!   foxdpi-bypass --rule youtube.com:record-split --rule banka.com.tr:exempt
//!   foxdpi-bypass --show                # etkin yapılandırmayı yazdır
//!   foxdpi-bypass --dns-test example.com

use clap::Parser;
use foxdpi_bypass::config::{AdBlockMode, Config, DnsProvider, Rule, Strategy};
use foxdpi_bypass::{DnsResolver, SocksProxy};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "foxdpi-bypass",
    version,
    about = "FoxDPI'nin kendi DPI bypass motoru — yerel SOCKS5 proxy + TLS ClientHello biçimlendirme",
    long_about = "Yerel 127.0.0.1 SOCKS5 proxy başlatır; tarayıcınızı/sisteminizi buraya proxy olarak verin.\nTLS ClientHello üzerinde record-split / segment-desync / disorder (fake-split) / decoy / OOB / Host-scramble uygulayarak SNI tabanlı sansürü aşar.\nDNS varsayılan olarak DoH (sertifika doğrulamalı, RFC 8484) ile çözülür; olmazsa TCP-53'e düşer.\ndecoy/disorder ham socket ister (root/CAP_NET_RAW); yoksa uyarı basılır ve sıradan akış sürer.\nReklam engelleme: --adblock off|nullroute|deny|sinkhole (sinkhole mini HTTP 204 sunucusu)."
)]
struct Args {
    /// Yapılandırma dosyası (varsayılan: ./config.yaml; yoksa yerleşik varsayılanlar)
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Dinlenecek port (127.0.0.1 üzerinden)
    #[arg(short, long, value_name = "PORT")]
    port: Option<u16>,

    /// Varsayılan strateji
    #[arg(short, long, value_enum, value_name = "STRATEGY")]
    mode: Option<Strategy>,

    /// Domain kuralı: DOMAIN[:strategy|:exempt]  (örn. --rule youtube.com:record-split --rule ornek.com.tr:exempt)
    /// Birden çok kez verilebilir; wildcard için *.ornek.com gibi yazın
    #[arg(long, value_name = "DOMAIN[:strategy|:exempt]")]
    rule: Vec<String>,

    /// DNS sağlayıcısı (doh varsayılan | builtin | custom | none)
    #[arg(long, value_name = "doh|builtin|custom|none")]
    dns: Option<String>,

    /// Özel DNS sunucuları (virgülle ayrık, örn. 1.1.1.1,8.8.8.8)
    #[arg(long, value_name = "SERVERS")]
    custom_dns: Option<String>,

    /// Reklam/izleyici engelleme modu: off|nullroute|deny|sinkhole
    #[arg(long, value_name = "MODE")]
    adblock: Option<AdBlockMode>,

    /// Reklam engelleme (kısayol): DNS seviyesinde 0.0.0.0'a yönlendirir (nullroute) + yerleşik liste
    #[arg(long)]
    block_ads: bool,

    /// Harici hosts-formatlı blocklist dosyası (her satır bir domain; # yorum)
    #[arg(long, value_name = "PATH")]
    blocklist_file: Option<PathBuf>,

    /// Özel DoH sunucusu (örn. https://1.1.1.1/dns-query); verilmezse builtin DoH havuzu + TCP-53 fallback
    #[arg(long, value_name = "URL")]
    doh_server: Option<String>,

    /// Raw decoy paketi için TTL (disorder/decoy stratejileri; root/CAP_NET_RAW gerekir)
    #[arg(long, value_name = "TTL")]
    decoy_ttl: Option<u8>,

    /// Auto-TTL: 1'den autottl_max'a kadar TTL'li sahte paketler gönder
    #[arg(long)]
    autottl: bool,

    /// Auto-TTL en yüksek TTL (varsayılan 6)
    #[arg(long, value_name = "N")]
    autottl_max: Option<u8>,

    /// Segment-desync parça gönderimleri arası gecikme (ms)
    #[arg(long, value_name = "MS")]
    desync_delay: Option<u64>,

    /// Kayıt bölümleme ofseti (byte); verilmezse SNI ofseti kullanılır
    #[arg(long, value_name = "OFFSET")]
    split_offset: Option<usize>,

    /// Sinkhole (204 No Content) sunucusu için port (varsayılan 8899)
    #[arg(long, value_name = "PORT")]
    sinkhole_port: Option<u16>,

    /// Etkin yapılandırmayı YAML olarak yazdır ve çık
    #[arg(long)]
    show: bool,

    /// DNS testi: verilen domain'i çöz ve yazdır, sonra çık
    #[arg(long, value_name = "DOMAIN")]
    dns_test: Option<String>,
}

fn load_or_default(path: &Option<PathBuf>) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    match path {
        Some(p) => {
            if !p.exists() {
                return Err(format!("yapılandırma dosyası bulunamadı: {}", p.display()).into());
            }
            let text = std::fs::read_to_string(p)?;
            Ok(serde_yaml::from_str(&text)?)
        }
        None => {
            let default_path = Path::new("config.yaml");
            if default_path.exists() {
                let text = std::fs::read_to_string(default_path)?;
                Ok(serde_yaml::from_str(&text)?)
            } else {
                Ok(Config::default())
            }
        }
    }
}

fn parse_rule(spec: &str) -> Result<Rule, String> {
    // DOMAIN[:STRATEGY|:exempt] — iki nokta üst üste'den sonra bilinmeyen bir
    // strateji adı gelirse kural reddedilir.
    match spec.rsplit_once(':') {
        Some((domain, tail)) if !domain.is_empty() => {
            let t = tail.trim().to_ascii_lowercase();
            if t == "exempt" || t == "none" || t == "bypass" {
                return Ok(Rule { domain: domain.to_string(), exempt: true, strategy: None });
            }
            match tail.parse::<Strategy>() {
                Ok(strategy) => Ok(Rule { domain: domain.to_string(), exempt: false, strategy: Some(strategy) }),
                Err(e) => Err(format!("kural '{}': {}", spec, e)),
            }
        }
        _ => Ok(Rule { domain: spec.to_string(), exempt: false, strategy: None }),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .init();

    let mut cfg = load_or_default(&args.config)?;

    if let Some(port) = args.port { cfg.port = port; }
    if let Some(mode) = args.mode { cfg.mode = mode; }
    for spec in &args.rule {
        match parse_rule(spec) {
            Ok(rule) => cfg.rules.push(rule),
            Err(e) => { eprintln!("uyarı: {}", e); }
        }
    }
    if let Some(dns) = &args.dns {
        cfg.dns.provider = dns.parse::<DnsProvider>().map_err(|e: String| e)?;
    }
    if let Some(servers) = &args.custom_dns {
        cfg.dns.provider = DnsProvider::Custom;
        cfg.dns.custom_servers = servers.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    }
    if args.block_ads {
        cfg.dns.block_ads = true;
        if cfg.dns.adblock == AdBlockMode::Off {
            cfg.dns.adblock = AdBlockMode::NullRoute;
        }
        cfg.dns.block_pages = foxdpi_bypass::dns::DEFAULT_AD_BLOCK_LIST.iter().map(|s| s.to_string()).collect();
    }
    if let Some(mode) = &args.adblock { cfg.dns.adblock = *mode; }
    if let Some(p) = &args.blocklist_file { cfg.dns.blocklist_file = Some(p.clone()); }
    if let Some(u) = &args.doh_server { cfg.dns.doh_server = Some(u.clone()); }
    if let Some(t) = args.decoy_ttl { cfg.decoy_ttl = t; }
    if args.autottl { cfg.autottl = true; }
    if let Some(n) = args.autottl_max { cfg.autottl_max = n; }
    if let Some(ms) = args.desync_delay { cfg.desync_delay_ms = ms; }
    if let Some(o) = args.split_offset { cfg.split_offset = Some(o); }
    if let Some(p) = args.sinkhole_port { cfg.dns.sinkhole_port = p; }

    // Sinkhole modu seçildiyse arka planda 204 sunucusunu başlat
    if cfg.dns.adblock == AdBlockMode::Sinkhole {
        let sp = cfg.dns.sinkhole_port;
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => { eprintln!("sinkhole runtime açılamadı: {}", e); return; }
            };
            rt.block_on(async move {
                if let Err(e) = foxdpi_bypass::sinkhole::Sinkhole::new(sp).run().await {
                    eprintln!("sinkhole durdu: {}", e);
                }
            });
        });
    }

    if args.show {
        print!("{}", serde_yaml::to_string(&cfg)?);
        return Ok(());
    }

    if let Some(domain) = &args.dns_test {
        return run_dns_test(cfg, domain.clone());
    }

    run_proxy(cfg)
}

fn run_dns_test(cfg: Config, domain: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let resolver = DnsResolver::new(cfg.dns.clone());
        match resolver.resolve(&domain).await {
            Ok(ip) => println!("{} → {}", domain, ip),
            Err(e) => { eprintln!("{} çözülemedi: {}", domain, e); std::process::exit(1); }
        }
    });
    Ok(())
}

fn run_proxy(cfg: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║  🦊 FoxDPI Bypass (kendi motorumuz)          ║");
    println!("╚══════════════════════════════════════════════╝");
    println!("  Proxy      : socks5://127.0.0.1:{}", cfg.port);
    println!("  Strateji   : {}", cfg.mode);
    println!("  DNS        : {:?} (DoH varsayılan)", cfg.dns.provider);
    println!("  Reklam engeli: {:?}", cfg.dns.adblock);
    println!("  Kurallar   : {}", cfg.rules.len());
    println!("  Tarayıcınızı 127.0.0.1:{} SOCKS5 proxy'ye ayarlayın. Çıkmak için Ctrl-C.", cfg.port);
    println!();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let proxy = SocksProxy::new(cfg);
        tokio::select! {
            res = proxy.run() => {
                if let Err(e) = res {
                    eprintln!("proxy durdu: {}", e);
                    std::process::exit(1);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n👋 Durduruluyor…");
            }
        }
    });
    Ok(())
}
