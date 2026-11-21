# Releases

Kullanıcı iki şey indirir:

| Kim | Ne | Nereden |
|---|---|---|
| Sunucuyu host eden | Docker imajı veya `vardrop-server-*` | GHCR / GitHub Releases |
| İstemci kullanan | Vardrop kurulumu | **GitHub Releases** |

Kaynak derlemesi şart değil. Sunucu paketleri istemci içermez — dağıtım
tek kanaldır.

Kurulumdan sonra kullanıcı exe'nin kurulum ekranına sunucu adresini ve
panelden ürettiği **read PAT**'i girer (Tokenlar sayfası: ad + kapsam +
geçerlilik 1–3650 gün; token yalnız bir kez gösterilir). İstemci token'ı
`GET /api/auth/me` ile doğrular, geçerliyse kaydeder. Platform farkı yoktur.

`productName` kurulum dosya adındadır (`Vardrop`).

| Platform | Dosya |
|---|---|
| Windows x64 | `Vardrop_*_x64-setup.exe` |
| macOS Apple Silicon / Intel | `*.dmg` |
| Linux x64 / arm64 | `.deb` / `.rpm` / `.AppImage` |

Derlemeler **imzasızdır**. Windows SmartScreen ve macOS Gatekeeper uyarır. "Yine de çalıştır" kullanıcının kararı. İmzalı yayın için Apple Developer ID ve bir Windows kod imza sertifikası gerekir; bu depoda sır olarak durmaz, Actions secret eklenince workflow genişletilir.

## Sunucu dosyaları

- `vardrop-server-linux-x64.tar.gz`
- `vardrop-server-linux-arm64.tar.gz`
- `vardrop-server-windows-x64.zip`
- `vardrop-server-macos-arm64.tar.gz`
- `vardrop-server-macos-x64.tar.gz`
- yanında `.sha256`
- imaj: `ghcr.io/<org>/vardrop:<sürüm>` ve `:latest`

Linux ikilisi Ubuntu 22.04 üzerinde derlenir (glibc 2.35+). Daha eski dağıtımlar Docker kullansın.

`ubuntu-22.04-arm` yalnız **public** repoda vardır. Repo private ise o satırı workflow'dan çıkarın.

## Kesim

Sürüm `Cargo.toml` `[workspace.package] version` ve `apps/client/src-tauri/tauri.conf.json` `version` ile aynı olsun.

```bash
git tag v0.1.0
git push origin v0.1.0
```

`.github/workflows/release.yml` etikette çalışır: panel derlenir, server
arşivleri (binary + panel) kesilir, client paketleri tauri-action ile aynı
Release'e yüklenir, Docker imajı GHCR'ye gider.

İlk imajdan sonra paket görünürlüğünü public yapın (Packages → package settings).

## Yerel istemci derlemesi

```bash
cd apps/client
npm ci
npm run tauri build
```

Çıktı: `apps/client/src-tauri/target/release/bundle/`.
