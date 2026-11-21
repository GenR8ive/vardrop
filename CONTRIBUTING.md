# Katkı

`data/`, `config.toml`, `master.key`, `*.db` commit edilmez. `.gitignore` buna göre.

```bash
cargo test --workspace
cargo run -p vardrop-server -- init
cargo run -p vardrop-server -- serve
```

Panel: `apps/admin-web` içinde `npm ci && npm run dev`.

İstemci: `apps/client` içinde `npm ci && npm run tauri dev`.

Sürüm kesimi: `docs/RELEASES.md`. `workspace.package.version` ile `tauri.conf.json` `version` aynı kalsın.

Lisans MIT. Başka lisans istenirse ilk public push'tan önce `LICENSE` değişsin; sonradan geri alınmaz.
