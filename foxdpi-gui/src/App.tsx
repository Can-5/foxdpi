import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Switch, SegmentedControl, Slider } from "./components/ui";

import { Power, PowerOff, Shield, Globe, Settings, Activity, ShieldOff, Zap, Filter, Clock, Database, Cpu, } from "lucide-react";

type DnsConfigPayload = {
  dns: {
    provider: "BuiltIn" | "Custom" | "None";
    servers: string[];
    encrypted: boolean;
    block_ads: boolean;
    block_pages: string[];
  };
};

function App() {
  const [status, setStatus] = useState({
    running: false,
    mode: "Default",
    dns_provider: "BuiltIn",
    adblock_enabled: false,
    speed_boost_enabled: false,
    block_pages_count: 3,
    techniques_count: 3,
    dns_prefetch: true,
    cache_bypass: true,
    dns_cache_ttl_seconds: 120,
    activate_delay_seconds: 3,
  });
  const [mode, setMode] = useState("Default");
  const [adblock, setAdblock] = useState(false);
  const [speed, setSpeed] = useState(false);
  const [dnsEncrypted, setDnsEncrypted] = useState(true);
  const [blockPages, setBlockPages] = useState<string[]>(["ads.example.com", "tracker.example.com", "analytics.example.com"]);
  const [techniques, setTechniques] = useState<string[]>(["dns-prefetch", "cache-bypass", "keep-alive"]);
  const [dnsPrefetch, setDnsPrefetch] = useState(true);
  const [cacheBypass, setCacheBypass] = useState(true);
  const [dnsCacheTtl, setDnsCacheTtl] = useState(120);
  const [activateDelay, setActivateDelay] = useState(3);
  const [configLoadedFromDisk, setConfigLoadedFromDisk] = useState(false);


  useEffect(() => {
    invoke<typeof status>("get_status").then(setStatus).catch((e) => {
      console.warn("şablon: status alınamadı, varsayılan kullanılıyor", e);
    });
  }, []);

  useEffect(() => {
    syncConfigToBackend();
  }, []);

  useEffect(() => {
    if (!configLoadedFromDisk) return;
    updateStatus();
  }, [configLoadedFromDisk]);

  useEffect(() => {
    const load = async () => {
      try {
        const loaded = await invoke<{
          bypass_mode: string;
          dns: { encrypted: boolean; block_pages: string[] };
          speed_boost: {
            techniques: string[];
            dns_prefetch: boolean;
            http_cache_bypass: boolean;
            dns_cache_ttl_seconds: number;
            activate_delay_seconds: number;
          };
        }>("load_config");
        setMode(loaded.bypass_mode as any);
        setDnsEncrypted(loaded.dns.encrypted);
        setBlockPages(loaded.dns.block_pages || []);
        setTechniques(loaded.speed_boost.techniques || []);
        setDnsPrefetch((loaded.speed_boost ?? {}).dns_prefetch ?? true);
        setCacheBypass((loaded.speed_boost ?? {}).http_cache_bypass ?? true);
        setDnsCacheTtl((loaded.speed_boost ?? {}).dns_cache_ttl_seconds ?? 120);
        setActivateDelay((loaded.speed_boost ?? {}).activate_delay_seconds ?? 3);
        setConfigLoadedFromDisk(true);
      } catch (e) {
        console.warn("şablon: config yükleme başarısız", e);
      }
    };
    load();
  }, []);

  useEffect(() => {
    updateStatus();
  }, [status, mode, adblock, speed, dnsEncrypted, blockPages, techniques, dnsPrefetch, cacheBypass, dnsCacheTtl, activateDelay]);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === " ") {
        e.preventDefault();
        status.running ? handleStop() : handleStart();
      } else if (e.key === "a" || e.key === "A") {
        e.preventDefault();
        handleToggleAdblock(!adblock);
      } else if (e.key === "h" || e.key === "H") {
        e.preventDefault();
        handleToggleSpeed(!speed);
      } else if (e.key === "d" || e.key === "D") {
        e.preventDefault();
        handleSetDnsEncrypted(!dnsEncrypted);
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [status.running, adblock, speed, dnsEncrypted]);

  const updateStatus = () => {
    setStatus((prev) => ({
      ...prev,
      running: status.running,
      mode,
      dns_provider: status.dns_provider,
      adblock_enabled: adblock,
      speed_boost_enabled: speed,
      block_pages_count: blockPages.length,
      techniques_count: techniques.length,
      dns_prefetch: dnsPrefetch,
      cache_bypass: cacheBypass,
      dns_cache_ttl_seconds: dnsCacheTtl,
      activate_delay_seconds: activateDelay,
    }));
  };


  void updateStatus;

  const buildDnsPayload = (extra?: Partial<DnsConfigPayload["dns"]>): DnsConfigPayload => ({
    dns: {
      provider: "BuiltIn",
      servers: [],
      encrypted: dnsEncrypted,
      block_ads: adblock,
      block_pages: blockPages,
      ...extra,
    },
  });

  const syncConfigToBackend = async () => {
    try {
      await invoke("set_config", {
        cfg: {
          bypass_mode: (mode as any),
          dns: buildDnsPayload().dns,
          adblock: { enabled: adblock, lists: [], custom_blockpages: blockPages },
          speed_boost: {
            enabled: speed,
            techniques,
            dns_prefetch: dnsPrefetch,
            http_cache_bypass: cacheBypass,
            dns_cache_ttl_seconds: dnsCacheTtl,
            activate_delay_seconds: activateDelay,
          },
          paths: { config_file: "", state_dir: "", log_dir: "", cache_dir: "" },
          telemetry: { enabled: false, endpoint: "" },
        } as any,
      });
    } catch (e) {
      console.warn("şablon: config senkronizasyonu başarısız", e);
    }
  };

  const handleStart = async () => {
    try {
      await invoke("start");
      await syncConfigToBackend();
      await invoke("save_config");
      updateStatus();
      setStatus((prev) => ({ ...prev, running: true }));
    } catch (e) {
      console.warn("şablon: start başlatılamadı", e);
    }
  };

  const handleStop = async () => {
    try {
      await invoke("stop");
      await syncConfigToBackend();
      await invoke("save_config");
      updateStatus();
      setStatus((prev) => ({ ...prev, running: false }));
    } catch (e) {
      console.warn("şablon: stop çalışmadı", e);
    }
  };

  const handleToggleAdblock = async (value: boolean) => {
    try {
      await invoke("toggle_adblock", { enabled: value });
      setAdblock(value);
      updateStatus();
    } catch (e) {
      console.warn("şablon: toggle_adblock", e);
    }
  };

  const handleToggleSpeed = async (value: boolean) => {
    try {
      await invoke("toggle_speed_boost", { enabled: value });
      setSpeed(value);
      updateStatus();
    } catch (e) {
      console.warn("şablon: toggle_speed_boost", e);
    }
  };

  const handleSetDnsEncrypted = async (value: boolean) => {
    setDnsEncrypted(value);
    try {
      await invoke("set_dns", buildDnsPayload({ encrypted: value }));
    } catch (e) {
      console.warn("şablon: dns şifreleme", e);
    }
  };

  const handleSetBlockPages = async (pages: string[]) => {
    setBlockPages(pages);
    try {
      await invoke("set_dns_blocked_pages", { pages });
    } catch (e) {
      console.warn("şablon: engellenen sayfalar", e);
    }
  };

  const handleSetTechniques = async (techs: string[]) => {
    setTechniques(techs);
    try {
      await invoke("set_techniques", { techniques: techs });
    } catch (e) {
      console.warn("şablon: teknikler", e);
    }
  };
  void handleSetTechniques;

  const handleSetDnsPrefetch = async (value: boolean) => {
    setDnsPrefetch(value);
    try {
      await invoke("set_dns_prefetch", { enabled: value });
    } catch (e) {
      console.warn("şablon: dns prefetch", e);
    }
  };

  const handleSetCacheBypass = async (value: boolean) => {
    setCacheBypass(value);
    try {
      await invoke("set_cache_bypass", { enabled: value });
    } catch (e) {
      console.warn("şablon: cache bypass", e);
    }
  };

  const handleSetDnsCacheTtl = async (value: number) => {
    setDnsCacheTtl(value);
    try {
      await invoke("set_dns_cache_ttl", { seconds: value });
    } catch (e) {
      console.warn("şablon: dns cache ttl", e);
    }
  };

  const handleSetActivateDelay = async (value: number) => {
    setActivateDelay(value);
    try {
      await invoke("set_activate_delay", { seconds: value });
    } catch (e) {
      console.warn("şablon: aktifasyon gecikmesi", e);
    }
  };

  const [telemetryEnabled, setTelemetryEnabled] = useState(false);
  const [telemetryEndpoint, setTelemetryEndpoint] = useState("");

  const handleToggleTelemetry = async (value: boolean) => {
    setTelemetryEnabled(value);
    try {
      await invoke("set_telemetry", { enabled: value, endpoint: telemetryEndpoint });
    } catch (e) {
      console.warn("şablon: telemetry", e);
    }
  };

  const handleTelemetryEndpointChange = async (value: string) => {
    setTelemetryEndpoint(value);
    try {
      await invoke("set_telemetry", { enabled: telemetryEnabled, endpoint: value });
    } catch (e) {
      console.warn("şablon: telemetry endpoint", e);
    }
  };

  return (
    <div className="min-h-screen flex flex-col">
      <header className="border-b border-fox-200 bg-white/70 backdrop-blur sticky top-0 z-10">
        <div className="mx-auto flex h-16 max-w-5xl items-center justify-between px-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-full bg-fox-600 text-white shadow-sm">
              <Globe className="h-5 w-5" aria-hidden="true" />
            </div>
            <div className="flex flex-col">
              <span className="font-semibold text-fox-900">FoxDPI</span>
              <span className="text-xs text-fox-500">kendi DPI bypass motorumuz</span>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <span className="hidden sm:inline-flex items-center gap-1 rounded-full bg-fox-100 px-3 py-1 text-sm text-fox-700">
              <span className="h-2 w-2 rounded-full bg-fox-500" />
              şablon
            </span>
            {configLoadedFromDisk && (
              <span className="rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800">
                Yapılandırma yüklendi
              </span>
            )}
          </div>
        </div>
      </header>

      <main className="flex flex-1 items-start justify-center px-4 py-8">
        <div className="w-full max-w-2xl space-y-6">
          {/* Genel durum */}
          <section className="rounded-2xl border border-fox-200 bg-white p-5 shadow-sm">
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Activity className="h-5 w-5 text-fox-600" aria-hidden="true" />
                <h2 className="text-lg font-semibold text-fox-900">Motor durum</h2>
              </div>
              <span className="rounded-full bg-fox-100 px-2 py-0.5 text-xs font-medium text-fox-700">
                şablon
              </span>
            </div>

            <div className="rounded-xl border border-fox-100 bg-fox-50 p-4">
              <div className="flex items-center justify-between gap-4">
                <div className="flex flex-col gap-1">
                  <span className="text-sm text-fox-500">Şu anki durum</span>
                  <span className="text-lg font-semibold text-fox-900">
                    {status.running ? "Çalışıyor" : "Durumuz kapalı"}
                  </span>
                </div>
                <div className="flex gap-3">
                  {status.running ? (
                    <button
                      onClick={handleStop}
                      className="inline-flex h-10 w-32 items-center justify-center gap-2 rounded-xl border border-red-200 bg-white px-4 text-sm font-medium text-red-700 transition hover:bg-red-50 focus-ring"
                    >
                      <PowerOff className="h-4 w-4" aria-hidden="true" />
                      Durdur
                    </button>
                  ) : (
                    <button
                      onClick={handleStart}
                      className="inline-flex h-10 w-32 items-center justify-center gap-2 rounded-xl border border-fox-200 bg-fox-600 px-4 text-sm font-medium text-white shadow-sm transition hover:bg-fox-700 focus-ring"
                    >
                      <Power className="h-4 w-4" aria-hidden="true" />
                      Başlat
                    </button>
                  )}
                </div>
              </div>
            </div>
          </section>

          {/* Bypass modu */}
          <section className="rounded-2xl border border-fox-200 bg-white p-5 shadow-sm">
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Shield className="h-5 w-5 text-fox-600" aria-hidden="true" />
                <h2 className="text-lg font-semibold text-fox-900">Bypass stratejisi</h2>
              </div>
              <span className="rounded-full bg-fox-100 px-2 py-0.5 text-xs font-medium text-fox-700">
                şablon
              </span>
            </div>

            <div className="rounded-xl border border-fox-100 bg-fox-50 p-4">
              <SegmentedControl
                value={mode}
                onValueChange={(value) => {
                  setMode(value);
                  invoke("set_bypass_mode", { mode: value as any }).catch(() => {});
                }}
                options={[
                  { value: "Default", label: "Varsayılan" },
                  { value: "Split", label: "Yarı parçalama" },
                  { value: "Desync", label: "Ayrıştırma" },
                  { value: "Decoy", label: "Decoy (sahte paket)" },
                  { value: "OOB", label: "Out-of-band bayt" },
                  { value: "HostScramble", label: "Host karıştırma" },
                ]}
                className="w-full"
              />
              <p className="mt-3 text-sm text-fox-500">
                Şu anki seçim: <span className="font-medium text-fox-700">{mode}</span>. Gerçek motor ayrı geliştirme.ekipleri tarafından tamamlanacaktır.
              </p>
            </div>
          </section>

          {/* Reklam engelleme */}
          <section className="rounded-2xl border border-fox-200 bg-white p-5 shadow-sm">
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <ShieldOff className="h-5 w-5 text-fox-600" aria-hidden="true" />
                <h2 className="text-lg font-semibold text-fox-900">Reklam engelleme</h2>
              </div>
              <span className="rounded-full bg-fox-100 px-2 py-0.5 text-xs font-medium text-fox-700">
                şablon
              </span>
            </div>

            <div className="rounded-xl border border-fox-100 bg-fox-50 p-4">
              <div className="flex items-center justify-between gap-4">
                <div className="flex flex-col gap-1">
                  <span className="text-sm text-fox-500">Reklam ve izleyici engelleme</span>
                  <span className="text-sm text-fox-900">DNS tabanlı engelleme listesiyle reklamları ve takipçi/izleyici servislerini engeller.</span>
                </div>
                <Switch
                  checked={adblock}
                  onCheckedChange={handleToggleAdblock}
                  className="translate-y-0.5"
                />
              </div>
              {adblock && (
                <div className="mt-4 rounded-lg border border-fox-100 bg-fox-50 p-4">
                  <div className="mb-2 flex items-center justify-between">
                    <span className="text-sm font-medium text-fox-900">Engellenen sayfalar</span>
                    <span className="text-sm text-fox-600">{blockPages.length} → {status.dns_provider}</span>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {blockPages.map((p) => (
                      <span key={p} className="inline-flex items-center gap-1 rounded-full bg-fox-100 px-2 py-1 text-xs text-fox-700">
                        <Filter className="h-3 w-3 text-fox-500" aria-hidden="true" />
                        {p}
                      </span>
                    ))}
                  </div>
                  <button
                    type="button"
                    onClick={() => handleSetBlockPages([])}
                    className="mt-3 rounded-lg border border-red-200 px-3 py-1 text-xs font-medium text-red-700 transition hover:bg-red-50 focus-ring"
                  >
                    Tüm listeyi temizle
                  </button>
                </div>
              )}
            </div>
          </section>

          {/* İnternet hızlandırma */}
          <section className="rounded-2xl border border-fox-200 bg-white p-5 shadow-sm">
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Zap className="h-5 w-5 text-fox-600" aria-hidden="true" />
                <h2 className="text-lg font-semibold text-fox-900">İnterneti hızlandırma</h2>
              </div>
              <span className="rounded-full bg-fox-100 px-2 py-0.5 text-xs font-medium text-fox-700">
                şablon
              </span>
            </div>

            <div className="rounded-xl border border-fox-100 bg-fox-50 p-4">
              <div className="flex items-center justify-between gap-4">
                <div className="flex flex-col gap-1">
                  <span className="text-sm text-fox-500">İnternet hızlandırma</span>
                  <span className="text-sm text-fox-900">DNS önbelleği, aktivasyon gecikmesi, HTTP cache bypass ve tekniklerle hızlanma denemesi.</span>
                </div>
                <Switch
                  checked={speed}
                  onCheckedChange={handleToggleSpeed}
                  className="translate-y-0.5"
                />
              </div>
              {speed && (
                <div className="mt-4 space-y-4">
                  <div className="rounded-lg border border-fox-100 bg-fox-50 p-4">
                    <div className="flex items-center justify-between gap-4">
                      <div className="flex items-center gap-2">
                        <Database className="h-4 w-4 text-fox-600" aria-hidden="true" />
                        <span className="text-sm font-medium text-fox-900">DNS önbellek TTL</span>
                      </div>
                      <span className="text-sm text-fox-700">{dnsCacheTtl}s</span>
                    </div>
                    <Slider
                      value={dnsCacheTtl}
                      onValueChange={handleSetDnsCacheTtl}
                      min={0}
                      max={600}
                      step={10}
                      label="Önbellek TTL (saniye)"
                    />
                  </div>

                  <div className="rounded-lg border border-fox-100 bg-fox-50 p-4">
                    <div className="flex items-center justify-between gap-4">
                      <div className="flex items-center gap-2">
                        <Clock className="h-4 w-4 text-fox-600" aria-hidden="true" />
                        <span className="text-sm font-medium text-fox-900">Aktifasyon gecikmesi</span>
                      </div>
                      <span className="text-sm text-fox-700">{activateDelay}s</span>
                    </div>
                    <Slider
                      value={activateDelay}
                      onValueChange={handleSetActivateDelay}
                      min={0}
                      max={30}
                      step={1}
                      label="Motor aktifasyon gecikmesi (saniye)"
                    />
                    <p className="mt-2 text-xs text-fox-500">Bağlantı kurulduktan sonra motorun aktif olması için beklenen gecikme.</p>
                  </div>
                  <div className="rounded-lg border border-fox-100 bg-fox-50 p-4">
                    <div className="flex items-center justify-between gap-4">
                      <div className="flex flex-col gap-1">
                        <span className="text-sm font-medium text-fox-900">DNS önborsa</span>
                        <span className="text-xs text-fox-500">DNS yanıtlarını yerel önborsada tutar.</span>
                      </div>
                      <Switch checked={dnsPrefetch} onCheckedChange={handleSetDnsPrefetch} className="translate-y-0.5" />
                    </div>
                    <div className="mt-3 flex items-center justify-between gap-4">
                      <div className="flex flex-col gap-1">
                        <span className="text-sm font-medium text-fox-900">HTTP cache bypass</span>
                        <span className="text-xs text-fox-500">Önborsalı DNS/HTTP trafiğini optimize eder.</span>
                      </div>
                      <Switch checked={cacheBypass} onCheckedChange={handleSetCacheBypass} className="translate-y-0.5" />
                    </div>
                    <div className="mt-3 flex flex-wrap gap-2">
                      {techniques.map((t) => (
                        <span key={t} className="inline-flex items-center gap-1 rounded-full bg-fox-100 px-2 py-1 text-xs text-fox-700">
                          <Cpu className="h-3 w-3 text-fox-500" aria-hidden="true" />
                          {t}
                        </span>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </section>

          {/* DNS / şifreleme */}
          <section className="rounded-2xl border border-fox-200 bg-white p-5 shadow-sm">
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Globe className="h-5 w-5 text-fox-600" aria-hidden="true" />
                <h2 className="text-lg font-semibold text-fox-900">DNS ve şifreleme</h2>
              </div>
              <span className="rounded-full bg-fox-100 px-2 py-0.5 text-xs font-medium text-fox-700">
                şablon
              </span>
            </div>

            <div className="rounded-xl border border-fox-100 bg-fox-50 p-4">
              <div className="flex items-center justify-between gap-4">
                <div className="flex flex-col gap-1">
                  <span className="text-sm text-fox-500">Şifreli DNS (DoH/DoT)</span>
                  <span className="text-sm text-fox-900">DNS sorgularını şifreleyerek orta threwkileri ve pollution'ı engeller.</span>
                </div>
                <Switch
                  checked={dnsEncrypted}
                  onCheckedChange={handleSetDnsEncrypted}
                  className="translate-y-0.5"
                />
              </div>
              <div className="mt-4 rounded-lg border border-fox-100 bg-fox-50 p-4">
                <div className="mb-3 flex items-center justify-between gap-4">
                  <div className="flex flex-col gap-1">
                    <span className="text-sm font-medium text-fox-900">Engellenen sayfalar (DNS/IP tabanlı)</span>
                    <span className="text-xs text-fox-500">{blockPages.length} hari → {status.dns_provider}</span>
                  </div>
                  <div className="flex gap-2">
                    {blockPages.map((p) => (
                      <span key={p} className="inline-flex items-center gap-1 rounded-full bg-fox-100 px-2 py-1 text-xs text-fox-700">
                        <Filter className="h-3 w-3 text-fox-500" aria-hidden="true" />
                        {p}
                      </span>
                    ))}
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  {["ads.example.com", "tracker.example.com", "analytics.example.com", "doubleclick.net", "facebook.com/trk"].map((candidate) => (
                    <button
                      key={candidate}
                      type="button"
                      onClick={() => {
                        handleSetBlockPages(
                          blockPages.includes(candidate)
                            ? blockPages.filter((x) => x !== candidate)
                            : [...blockPages, candidate],
                        );
                      }}
                      className={`inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs font-medium transition ${blockPages.includes(candidate)
                        ? "bg-fox-600 text-white shadow-sm ring-1 ring-fox-400"
                        : "bg-fox-100 text-fox-700 hover:bg-fox-200"
                        }`}
                    >
                      {candidate}
                    </button>
                  ))}
                </div>
                <p className="mt-3 text-xs text-fox-500">
                  Şablon aşamasında; gerçek DNS motoru, ad/kural listesi ve güncelleme ayrı yapılır.
                </p>
              </div>
            </div>
          </section>

          {/* Ayarlar / hakkında */}
          <section className="rounded-2xl border border-fox-200 bg-white p-5 shadow-sm">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Settings className="h-5 w-5 text-fox-600" aria-hidden="true" />
                <h2 className="text-lg font-semibold text-fox-900">Ayarlar ve bilgi</h2>
              </div>
              <span className="rounded-full bg-fox-100 px-2 py-0.5 text-xs font-medium text-fox-700">
                şablon
              </span>
            </div>
            <div className="mt-3 space-y-3">
              <div className="rounded-xl border border-fox-100 bg-fox-50 p-4 text-sm text-fox-900">
                <div className="mb-1 flex items-center gap-2">
                  <Shield className="h-4 w-4 text-fox-600" aria-hidden="true" />
                  <span className="font-medium text-fox-900">Shortcuts</span>
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs text-fox-500">
                  <div className="rounded-lg bg-fox-50 px-2 py-1"><kbd className="rounded bg-fox-100 px-1 text-fox-700">Space</kbd><span>Başlat/Durdur</span></div>
                  <div className="rounded-lg bg-fox-50 px-2 py-1"><kbd className="rounded bg-fox-100 px-1 text-fox-700">A</kbd><span>Reklam engelleme</span></div>
                  <div className="rounded-lg bg-fox-50 px-2 py-1"><kbd className="rounded bg-fox-100 px-1 text-fox-700">H</kbd><span>Hızlandırma</span></div>
                  <div className="rounded-lg bg-fox-100 px-2 py-1"><kbd className="rounded bg-fox-100 px-1 text-fox-700">D</kbd><span>DNS şifreleme</span></div>
                </div>
              </div>
              <div className="rounded-xl border border-fox-100 bg-fox-50 p-4 text-sm text-fox-500">
                FoxDPI — Linux dağıtımlarında çalışan, kendi DPI bypass motoru ile donatılmış grafiksel uygulamadır.
                Bu arayüz şablon aşamasında; gerçek motor, DNS motoru, reklam ve izleyici engelleme listeleri ve hızlandırma teknikleri ayrı geliştirme.ekipleri tarafından tamamlanacaktır.
              </div>

              <div className="rounded-xl border border-fox-100 bg-fox-50 p-4">
                <div className="mb-3 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Cpu className="h-4 w-4 text-fox-600" aria-hidden="true" />
                    <span className="text-sm font-medium text-fox-900">Telemetry</span>
                  </div>
                  <Switch checked={telemetryEnabled} onCheckedChange={handleToggleTelemetry} className="translate-y-0.5" />
                </div>
                <div className="mt-3 space-y-3">
                  <div className="flex flex-col gap-1">
                    <span className="text-sm text-fox-500">Telemetry uç noktası</span>
                    <input
                      type="text"
                      value={telemetryEndpoint}
                      onChange={(e) => handleTelemetryEndpointChange(e.target.value)}
                      placeholder="https://example.com/telemetry"
                      className="rounded-lg border border-fox-200 bg-white px-3 py-2 text-sm text-fox-900 shadow-sm outline-none focus:border-fox-400 focus:ring-1 focus:ring-fox-400"
                    />
                  </div>
                  <p className="text-xs text-fox-500">
                    Şablon aşamasında; gerçek telemetry gönderme ve 로그 elleştirme ayrı geliştirme.ekipleri tarafından tamamlanacaktır.
                  </p>
                </div>
              </div>
            </div>
          </section>
        </div>
      </main>

      <footer className="border-t border-fox-200 bg-white/70 backdrop-blur px-4 py-3 text-center text-xs text-fox-500">
        FoxDPI v0.1.0 şablon — kendi DPI bypass motorumuz. Tüm özellikler ayrı geliştirme.ekipleri tarafından tamamlanacaktır.
      </footer>
    </div>
  );
}

export default App;
