# foxdpi-gui — Tauri masaüstü GUI (KAYNAK, henüz derlenmiyor)

Bu dizin **Tauri v1/React** tabanlı arayüzün kaynağıdır. Şu anki durumu
dürüstçe belgeliyorum:

- **Bu makinede derlenemez**: `webkit2gtk4.1-devel` (ve 4.0) kurulu değil;
  kurulum için `sudo dnf install webkit2gtk4.1-devel` gerekir. Fedora 45
  üzerinde test edilmedi.
- **Motorla uyumsuz**: `src-tauri/src/main.rs`, var olmayan bir
  `foxdpi-bypass start` komutunu çağırıyor; gerçek CLI `-p/--port -m/--mode ...`
  alır. Fake toggle'lar (`speed_boost`, `dns_prefetch`, `telemetry`) motorda
  yoktur.
- Bu yüzden **çalışan GUI `../foxdpi-gui.py`'dır** (Python/tkinter, stdlib).
  Bu Tauri kaya, motor CLI'sine uyumlu hale getirilip webkit bağımlılığı
  kurulduktan sonra yeniden değerlendirilebilir; fake özellikler kaldırılmadan
  kullanıma alınmamalıdır.

Tauri'yi sağlamlaştırmak istenirse izlenecek yol:

1. `sudo dnf install webkit2gtk4.1-devel javascriptcoregtk4.1-devel rsvg2-devel`
2. `main.rs`'teki `start` komutunu gerçek argümanlara çevirin
   (`-p port -m mode --dns ...`), `stop` için sakladığınız child'a SIGTERM gönderin.
3. Fake toggle'ları (`speed_boost`, `telemetry`, `dns_prefetch`) UI ve Rust'tan kaldırın.
4. `cargo build` + gerçek ağ ile curl e2e doğrulayın.

Uygulama simgesi ve AppImage çıktısı `FoxDPI-x86_64.AppImage` bu dizinde hazır;
ancak yukarıdaki eksikler tamamlanmadan dağıtılmamalıdır.
