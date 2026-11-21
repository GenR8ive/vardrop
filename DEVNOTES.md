# DEVNOTES — tasarım notları ve bilinçli sapmalar

`docs/PLAN.md` yetkili mimarıdır; bu belge, v0.1 uygulamasındaki önemli kararları ve
planla arasındaki sapmaları kaydeder.

## 1. Arama — FTS5 yerine LIKE (sapma)

Plan SQLite **FTS5** oluyordu. v0.1 `LIKE %q%` kullanır (ad, açıklama, tags JSON metni) +
Rust tarafında sıralama (updated_at DESC). Nedenler:

- sqlx SQLite derlemesi FTS5'i ilave feature ile gerektirir; ilk kurulum sade kaldı.
- Kayıt sayısı on binlerce altında LIKE yeterli; popup gecikmesi ~1-2 ms.

Yükseltme yolu: migration `CREATE VIRTUAL TABLE entries_fts USING fts5(name, description, tags);`
+ trigger senkronu + `match` sorgusu (api/entries.rs `build_where` tek noktadan değişir).

## 2. Panel — rust-embed yerine ServeDir (sapma)

Plan derlemeye gömülü dist'i öngörüyordu; v0.1 server açılışta `panel.dist` klasörü varsa
`ServeDir + SPA fallback` servis eder. Panel ayrı yaşam döngüsüne (vite build) sahiptir.
Dağıtımda tek binary istenirse dist içeriğini `include_bytes!`/rust-embed ile gömöyle.

## 3. Kimlik, token, oturum

- Şifreler **argon2id**; tokenlar **sha256** hash ile `api_tokens`'da — ham token tek sefer gösterilir.
- Login oturumları ayrı tablo değil: `api_tokens` içinde `name = "session"`, TTL `security.session_ttl_hours`.
- Login panel + client aynı uç nokta; client için panel **Tokenlar** sayfasından üretilen PAT (`read` kapsamı).
- Bruteforce: IP başına 5 hatalı deneme → 60 sn kilit (in-memory, restart'ta sıfırlanır). `login.failed` audit'e yazılır.
- `acl_rules` tablosu şemada hazır ama **v0.1'de enforcement yok** — yetki global rol+kapsam ile çalışır.
  Eklenme noktası: `auth::middleware::require` (resource düzeyinde kontrol burada inşa edilir).

## 4. Secret saklama

- **AES-256-GCM**; format `nonce(12) || ciphertext+tag`. Anahtar 32 byte hex `data/master.key`.
- Inline eşiği `secret_max_inline_kb` (vars. 16 KB); üstü dosyaya yazılır (şifreli blob).
- Önizleme maskesi `••••••••`; **value okumak `secret.reveal` audit kaydı üretir**.
- Master key dönüşü (rotation) yok — gerekiyorsa: eski key ile decrypt + yeni key ile re-encrypt batch scripti (TODO).

## 5. Dosyalar / blob depolama

- Yol: `{storage}/{entry_id}/{sha256}` (şifreli olanlar `.enc` son ekli) — içerik adresli;
  aynı içerikli dosyalar versiyonlar arasında fiziksel tek kopyadır. Entry silinince klasörü silinir.
- `binary` = tek dosya per sürüm; orijinal dosya adı metadata olarak saklanmaz (bundle `file_path` saklar) — v0.1 sınırlaması.
- Multipart kuralla istemci kuralı: opsiyonel `path` alanı, her `file` alanından önce gelir (npm'siz client uyumu).
- Multipart field order: optional `path` field precedes each `file` field (planın kuralı korunur).

## 6. Versiyonlama

- Her güncelleme/dosya yükleme/restore **yeni versiyon satırı** üretir; restore kopyalama yoluyla ileri taşınır
  (eski değerler yeniden yazılır), sürüm silme/pruning yok.
- `entry_versions.value_hash` plaintext sha256 — değişiklik tarihçesi diff/teşhis için temel (UI diff yok, v0.1).

## 7. Client (Vardrop) sapmaları

- Kopyalama `navigator.clipboard` (WebView2) + fallback **arboard** (Rust komutu). Planın
  `tauri-plugin-clipboard-manager` yerine arboard — dış JS eklenti kurulumu beklemek istedik, ikisi de davranışı aynı.
- **Paste (otomatik yapıştırma) v0.1'de yok** — plan F4'te `enigo`/SendInput ile "focus sonra paste" senaryosu için;
  kısa ömürlü popup'ta odağın çalınması istenmediği için copy-only başlatıldı.
- Tauri identifier: `dev.vman.vardrop`. Ayar + PAT **OS anahtarlığında değil**:
  Windows `%LOCALAPPDATA%\vardrop` (eski `variable-filler` Local/Roaming yolları ilk okumada taşınır),
  macOS `~/Library/Application Support/vardrop`, Linux `$XDG_CONFIG_HOME` veya `~/.config/vardrop`.
  Unix'te dizin `0700`, dosya `0600`. keyring bağımlılığı kaldırıldı — WDAC/Credential Manager sapması duruyor;
  değerlendirme `SECURITY.md`.
- UI npm'siz vanilla JS (`window.__TAURI__`, `withGlobalTauri: true`); bundler eklenebilir.
- Bundle/binary içerik popup'tan kopyalanamaz (dosya tabanlı) — yönlendirme: panel.

## 8. Sunucu notları

- Varsayılan bind **127.0.0.1:8080**. `cors_allow_all` kod varsayılanı **false**;
  `config.example.toml` vite için `true`. Host şablonları `false`.
- Body limit 64 MB (`axum::extract::DefaultBodyLimit`) — multipart dosya yükleme üst sınırı.
- Audit `audit_log` için saklama (retention) kuralı yok: `user_id/username` denormalize (silinen kullanıcı kayıtları geride kalır, istenen davranış).
- Health: `GET /api/health` → `{status:"ok", version}`.

## 9. İstemci dağıtımı — GitHub Releases + PAT bağlantısı

İstemci (exe/dmg/deb/rpm/AppImage) **GitHub Releases**'ten iner: `release.yml`
client job'ı tauri-action ile paketleri doğrudan release'e yükler. Sunucu
paketleri (binary arşivi + ghcr imajı) istemci içermez — dağıtım tek kanaldır.

- İstemci bağlantısı **PAT akışıyla**: panel → Tokenlar → read PAT üret
  (geçerlilik 7/30/90/…/3650 gün, boş = süresiz; sunucu 1–3650 dışını 400 ile
  reddeder) → exe kurulum ekranına sunucu adresi + PAT → exe `GET /api/auth/me`
  ile doğrular, `server.txt` + `token.txt` yazar. "Unut" ikisini de siler.
- Secret okuma PIN kapısı (opsiyonel): `users.pin_hash` + `POST /api/auth/unlock`
  (10 dk pencere, 5 hatalı → 60 sn IP kilidi). Secret'lar yalnız pencerede okunur.
- Secret okumalar audit'te `secret.reveal`; PAT kullanımı `last_used_at`.
- Gelecek adayı: exe için otomatik güncelleme (tauri updater) — imzasız derleme
  gereği ertelendi.

## 10. Bilinen boşluklar (bilinçli, v0.2 adayları)

1. Panel: TS `tsc --noEmit` build'e bağlı değil (yalnız vite build) — package.json'a `tsc && vite build` yapılmalı.
2. `secret` içerikte admin "son 4 karakter" görüntüsü maskede mevcut düzenine göre `mask_preview(last4)`.
3. E-posta bildirimi ya da 2FA yok. HTTP auth yalnız `Authorization: Bearer`.
4. Blob GC: entry silinince klasör silinir; ancak başarısız multipart yarım kalmalar artık bırakabilir.
5. Bundle manifest endpoint'i `/api/entries/{id}/value` JSON döndürür (`?file_id=` ile dosya) — ayrı `/manifest` yok.
6. Rate limit / audit pagination sade (limit 500).
7. Tests: birim testler (crypto, maskeleme) mevcut; API entegrasyonu el ile `scripts/smoke.ps1`;
   HTTP entegrasyon testleri yazılmalı.
8. PAT akışı: token'lar DB'de kalıcı (api_tokens); istemci ayarları
   (`server.txt` + `token.txt`) kullanıcı profilinde düz metin — DPAPI/keyring
   sarması v0.2 adayı (SECURITY.md "Bilinen sınırlar").
