# Vardrop

Kendi sunucunuzda çalışan değişken / secret kaydı. Rust **vardrop-server**
(axum + SQLite + yönetim paneli) ve **Vardrop** (Tauri 2 masaüstü istemci).

Biz host etmiyoruz. Sunucuyu siz kurarsınız. İstemciyi platformunuza göre
GitHub Releases'tan indirirsiniz. Telemetri yok. Lisans: [MIT](LICENSE).

```
variable-manager/
├─ docs/SELF-HOST.md         # sunucu kurulumu (Docker, systemd, TLS)
├─ docs/RELEASES.md          # release kesimi ve indirilecek dosyalar
├─ SECURITY.md               # tehdit modeli, istemci PAT saklama
├─ deploy/                   # Caddy, systemd, Docker config
├─ crates/vardrop-server/   # HTTP sunucusu
├─ apps/admin-web/           # panel (sunucu ile aynı origin)
└─ apps/client/              # Vardrop
```

## İstemci (kullanan kişi)

**GitHub Releases** → `Vardrop` → platform paketi:

| Platform | Dosya |
|---|---|
| Windows x64 | `Vardrop_*_x64-setup.exe` |
| macOS Apple Silicon / Intel | `*.dmg` |
| Linux | `.deb` / `.rpm` / `.AppImage` |

Kurulumdan sonra istemcinin kurulum ekranına **sunucu adresini** ve panelden
ürettiğiniz **read PAT**'i girin (panel → Tokenlar; token yalnız bir kez
gösterilir). `Ctrl+Alt+V` aramayı açar, **Enter** panoya kopyalar,
**Shift+Enter** odaktaki uygulamaya yapıştırır (paste-after-focus).
Görsel entry'lerde küçük thumbnail görünür; Enter görseli panoya resim olarak koyar.

Panel ayrıca kurulabilir **PWA**: tarayıcıda "Uygulamayı yükle" ile masaüstüne
kurulur — bakma/yazma için web, kopyalama/yapıştırma için Vardrop istemcisi.

## Sunucu (host eden)

Önerilen yol Docker. Ayrıntı ve systemd: [docs/SELF-HOST.md](docs/SELF-HOST.md).

```bash
docker compose up -d --build
docker compose exec registry cat /data/initial-admin-password.txt
```

Panel `http://127.0.0.1:8080`. İnternete yalnız TLS arkasından açın
(`docker compose --profile tls up -d` veya Caddy/Nginx).

Release arşivi (`vardrop-server-*`) ikili + paneli birlikte getirir;
Node veya Rust kurmadan `vardrop-server init` sonra `serve`.

Yedek: `master.key` + veritabanı + `storage/`. Anahtar kaybı = secret'lar çözülemez.

## Model

- **Cluster** → **Group** (ağaç) → **Entry**.
- Türler: `text` / `json` / `yaml` · `binary` (görseller otomatik thumbnail alır) · `bundle`.
- Her değişiklik yeni sürüm. Secret'lar diskte AES-256-GCM (`master.key`).
- Roller: admin · editor · viewer. Token kapsamları: read · write · admin.
- Kritik işlemler audit log'a yazılır.

## Geliştirme

```powershell
cargo run -p vardrop-server -- init
cargo run -p vardrop-server -- serve
```

`serve` config yolu: `--config`, yoksa `VARDROP_CONFIG`, yoksa `./config.toml`.

```powershell
cd apps/admin-web
npm ci
npm run dev          # :5173, /api → :8080
npm run build        # dist/; serve bunu panel.dist'ten verir
```

```powershell
cd apps/client
npm ci
npm run tauri dev
npm run tauri build
```

Geliştirme şablonu `config.example.toml` (`cors_allow_all = true`, vite için).
Host şablonu `config.host.example.toml`. Kod, config dosyası yokken CORS'u kapalı tutar.

## Uyarılar

- `data/`, `config.toml`, `master.key`, `*.db` git'e girmez.
- Trafik düz HTTP'dir. LAN ötesi kullanımda ters proxy + TLS şart.
- İstemci PAT'i OS anahtarlığında değil, kullanıcı profilinde düz dosyada tutar.
  Değerlendirme: [SECURITY.md](SECURITY.md).

Tasarım sapmaları: [DEVNOTES.md](DEVNOTES.md).
