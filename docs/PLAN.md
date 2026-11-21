# Vardrop — Mimari Plan

İki bileşenli, merkezi kaynak (variable/prompt/skill/secret) yönetim sistemi.

- **Server (Registry)**: Tek makinede kurulur. Veriyi saklar, kümeler, arar, yönetir. Web paneli ile giriş yapılır.
- **Client (Vardrop)**: Kullanıcı makinesinde çalışır. Global kısayol ile çağrılan popup üzerinden server'da arama yapar, değeri aktif metin kutusuna taşır.

---

## 1. Üst Mimari

```
┌────────────────────────────────────┐          ┌─────────────────────────────────────┐
│  SERVER — "Registry"               │          │  CLIENT — "Vardrop"         │
│                                    │          │                                     │
│  axum REST API   ◄─────────────────┼──HTTPS──►│  tray daemon + global hotkey        │
│  Admin Panel (web, token auth)     │  JSON    │  overlay popup (caret/mouse yakını) │
│  SQLite (sqlx) + FTS5 arama        │          │  arama → seç → kopyala / yapıştır   │
│  Secret şifreleme (AES-256-GCM)    │          │  token: Windows Credential Manager  │
│  Audit log + ACL + versiyonlama    │          │                                     │
└────────────────────────────────────┘          └─────────────────────────────────────┘
```

- Server, domain veya IP üzerinden servis edilir. TLS için Caddy/Nginx reverse proxy (ölçek büyürse) ya da doğrudan `rustls` (basit).
- Client yalnızca API konuşur; veri formatını server belirler, client sunar (aşağıdaki katman tablosuyla birebir uyumlu).

### Kullanıcı katmanları → crate karşılıkları

| Katman | Sorumluluk | Nerede |
|---|---|---|
| Registry core | Saklama, arama, gruplama, versiyonlama | `vardrop-core` + `vardrop-server/db` |
| Content format | Text/JSON/binary/bundle tanımı, önizleme | `vardrop-core::content` |
| Security policy | Secret, izin, audit | `vardrop-core::security` + `vardrop-server/auth` |
| Client action | Kopyalama, dosyaya aktarma, render | `registry-client` (actions modülü) |
| Extension / handler | Türe özel davranış | `ContentHandler` trait'i, iki tarafta da |

---

## 2. Teknoloji Seçimleri

| Alan | Tercih | Gerekçe |
|---|---|---|
| Server dili | Rust, `axum` 0.8 | Workspace zaten Rust; tokio ekosistemi |
| DB | SQLite (`sqlx`) + FTS5 | Tek makine, tek dosya, yedekleme kolay; Postgres'i sonraya bırak |
| Panel | **Vite + React + TS + Tailwind** SPA | "Tatlı" arayüz: HMR ile hızlı geliştirme, hazır bileşen kütüphaneleri (shadcn/ui); axum dist'i statik servis eder |
| Şifre hash | `argon2` (argon2id) | Standart |
| Secret şifreleme | AES-256-GCM + master key | At-rest koruma |
| Client GUI | **Tauri 2** | Transparent/frameless/always-on-top pencere + HTML popup UI; global-shortcut plugin'i hazır |
| Clipboard | `arboard` | Windows'ta sağlam |
| Yazıştırma | `enigo` (SendInput Ctrl+V) | Opsiyonel otomatik yapıştırma |
| Token saklama | `keyring` | Windows Credential Manager |
| HTTP | `reqwest` | JSON + dosya indirme |

Not: Client saf Rust (winit + egui) alternatifi de var; UI geliştirme hızı ve popup görünümü için Tauri önerilir.

---

## 3. Alan Modeli (Registry core)

```
Cluster 1 ─< Group (ağaç, sınırsız derinlik) 1 ─< Entry 1 ─< EntryVersion
Entry ─< BundleFile (yalnız kind=bundle ise)
Cluster/Group/Entry ─< AclRule
Kullanıcı → ApiToken, AuditLog
```

- `Group.parent_id` nullable → kök grup. Döngü engeli: taşıma sırasında hedef, taşınanın kendisi veya altı olamaz.
- Tam yol hesabı: recursive CTE ile `path` üretilir (`Variables/LivaBudget/Production`). Aramada path indexlenir.
- Versiyon: her value yazımı `version + 1` üretir; `current_version` işaretçisi; eski sürüm restore edilebilir. Maksimum tutulan sürüm sayısı ayarlanabilir (default 20).

### Content tipleri (Content format katmanı)

```
kind: text | json | yaml | binary | bundle     (+ is_secret: bool)
```

| kind | Depolama | Önizleme | Client aksiyonları |
|---|---|---|---|
| text / yaml | inline (küçükse) veya dosya | düz metin | kopyala, yapıştır, dosyaya kaydet |
| json | inline, saklanırken parse-doğrula | biçimli, satır sayılı | kopyala, yapıştır |
| binary (tek dosya) | blob_path + sha256 + size | ikon + ad + boyut + hash | dosyaya kaydet, klasörde aç |
| bundle (dosya paketi) | manifest + çok dosya (ör. SKILL.md paketi) | dosya listesi | klasöre aç/kaydet |
| *is_secret=true* (her tipte) | AES-GCM şifreli | `••••••••` + son 4 karakter maskeli | kopyala (default), reveal ayrı izinli |

---

## 4. Veritabanı Şeması (SQLite)

```sql
CREATE TABLE users (
  id            TEXT PRIMARY KEY,
  username      TEXT NOT NULL UNIQUE,
  display_name  TEXT,
  password_hash TEXT NOT NULL,            -- argon2id
  role          TEXT NOT NULL DEFAULT 'viewer',  -- admin | editor | viewer
  is_active     INTEGER NOT NULL DEFAULT 1,
  created_at    INTEGER NOT NULL,
  last_login_at INTEGER
);

CREATE TABLE clusters (
  id          TEXT PRIMARY KEY,
  slug        TEXT NOT NULL UNIQUE,
  name        TEXT NOT NULL,
  description TEXT,
  color       TEXT,                        -- paneldeki kart rengi
  created_by  TEXT REFERENCES users(id),
  created_at  INTEGER NOT NULL
);

CREATE TABLE groups (
  id         TEXT PRIMARY KEY,
  cluster_id TEXT NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
  parent_id  TEXT REFERENCES groups(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  UNIQUE (cluster_id, parent_id, name)    -- parent_id NULL'lu kökler de tespit edilir
);

CREATE TABLE entries (
  id              TEXT PRIMARY KEY,
  cluster_id      TEXT NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
  group_id        TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  kind            TEXT NOT NULL,          -- text | json | yaml | binary | bundle
  is_secret       INTEGER NOT NULL DEFAULT 0,
  description     TEXT,
  tags            TEXT,                   -- JSON array
  current_version INTEGER NOT NULL DEFAULT 1,
  created_by      TEXT, created_at INTEGER NOT NULL,
  updated_by      TEXT, updated_at INTEGER NOT NULL,
  UNIQUE (cluster_id, group_id, name)
);

CREATE TABLE entry_versions (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  entry_id    TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
  version     INTEGER NOT NULL,
  value_text  TEXT,                       -- text/json/yaml küçük değerler
  value_enc   BLOB,                       -- secret/binary küçük değerler: AES-GCM
  blob_path   TEXT,                       -- büyük binary: storage yolu
  value_hash  TEXT,                       -- plaintext sha256
  size        INTEGER,
  created_by  TEXT, created_at INTEGER NOT NULL,
  UNIQUE (entry_id, version)
);

CREATE TABLE bundle_files (               -- kind=bundle için
  id         TEXT PRIMARY KEY,
  entry_id   TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
  version    INTEGER NOT NULL,
  file_path  TEXT NOT NULL,               -- paket içi göreli yol, ör. SKILL.md
  blob_path  TEXT,
  content_enc BLOB,                       -- küçük dosyalar inline
  size       INTEGER, sha256 TEXT,
  is_secret  INTEGER NOT NULL DEFAULT 0,
  UNIQUE (entry_id, version, file_path)
);

CREATE TABLE api_tokens (
  id          TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  token_hash  TEXT NOT NULL UNIQUE,       -- sha256(token), ham token saklanmaz
  scopes      TEXT NOT NULL,              -- ["read","search","reveal","write"]
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER,
  last_used_at INTEGER,
  revoked_at  INTEGER
);

CREATE TABLE acl_rules (                  -- ek yetki kuralları (cluster/group bazlı)
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  subject_type   TEXT NOT NULL,           -- user | role
  subject        TEXT NOT NULL,
  resource_type  TEXT NOT NULL,           -- cluster | group
  resource_id    TEXT NOT NULL,
  permission     TEXT NOT NULL,           -- view | use | edit | admin
  UNIQUE (subject_type, subject, resource_type, resource_id)
);

CREATE TABLE audit_log (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id    TEXT, username TEXT,
  action     TEXT NOT NULL,               -- login | login.failed | user.create |
                                          -- entry.create | entry.update | secret.reveal ...
  resource   TEXT,                        -- tam yol veya entity kalıcı kimliği
  ip         TEXT, user_agent TEXT,
  detail     TEXT,                        -- JSON
  created_at INTEGER NOT NULL
);

-- Arama: isim + yol + etiket + açıklama; secret değerler ASLA indexlenmez
CREATE VIRTUAL TABLE entries_fts USING fts5(name, path, tags, description);
```

Rol → izin matrisi (özel ACL kuralı girilmemişse geçerli varsayılan):

| Rol | Cluster/Grup/Entry görüntüle | Value kullan (kopyala/reveal) | Düzenle | Kullanıcı yönetimi |
|---|---|---|---|---|
| admin | ✓ | ✓ | ✓ | ✓ |
| editor | ✓ | ✓ | ✓ | — |
| viewer | ✓ | ✓ (reveal hariç) | — | — |

---

## 5. REST API (axum)

Auth: `POST /api/auth/login` → `{ token, user }`. Panel token'ı cookie'de tutar; client `Authorization: Bearer` gönderir.

```
# Auth & profil
POST   /api/auth/login                       { username, password } → { token, user }
GET    /api/auth/me
POST   /api/tokens                           { name, scopes, expires_at } → { token }   (PAT)
GET    /api/tokens                           kendi tokenları
DELETE /api/tokens/:id

# Kullanıcı yönetimi (yalnız admin) — panel üzerinden
GET    /api/users
POST   /api/users                            { username, password, role, display_name }
PATCH  /api/users/:id                        { role?, is_active?, password? }
DELETE /api/users/:id

# Cluster
GET    /api/clusters                         kart listesi + sayaçlar (groups, entries)
POST   /api/clusters
GET    /api/clusters/:id
PATCH  /api/clusters/:id
DELETE /api/clusters/:id

# Grup ağacı
GET    /api/clusters/:id/groups              → ağaç {id, name, children[], entry_count}
POST   /api/clusters/:id/groups              { name, parent_id? }
PATCH  /api/groups/:id                       { name?, parent_id? }  (move + yeniden adlandır; döngüye izin yok)
DELETE /api/groups/:id                       (boşsa veya ?recursive=true)

# Entry
GET    /api/entries?q=&cluster_id=&group_id=&kind=&tag=&secret=&limit=
POST   /api/entries                          { cluster_id, group_id, name, kind, is_secret, value }
GET    /api/entries/:id
PATCH  /api/entries/:id                      (value değişirse yeni version)
DELETE /api/entries/:id
GET    /api/entries/:id/versions
POST   /api/entries/:id/versions/:v/restore

# Value erişimi
GET    /api/entries/:id/value                content negotiation:
                                             text/json → JSON; binary → stream;
                                             secret → şifresi çözülmüş (reveal izni + audit)
POST   /api/entries/:id/files                (bundle/binary yükleme, multipart)
GET    /api/entries/:id/files/:fid           indirme
DELETE /api/entries/:id/files/:fid

# Audit (yalnız admin)
GET    /api/audit?user=&action=&from=&to=
```

### DTO örnekleri

```jsonc
// Erisim/liste satırı — secret maskeli
{
  "id": "e_sql_conn",
  "cluster": { "id": "c_variables", "name": "Variables" },
  "path": "Variables/LivaBudget/Production",
  "name": "SQL_CONNECTION_STRING",
  "kind": "text",
  "is_secret": true,
  "tags": ["db", "prod"],
  "version": 4,
  "updated_at": "2026-10-29T10:12:00Z",
  "preview": "••••••••7f2a"        // is_secret=false ise ilk 120 karakter
}

// Arama cevabı (client popup bunu kullanır)
{ "results": [/* EntrySummary[] */], "total": 18, "took_ms": 41 }
```

Arama: `?q=liva prod sql` → FTS5 match + path prefix boost. Client debounce 300 ms.

---

## 6. Server Paneli (Registry Admin)

Yapı: sol menü `Concept (Clusters)` · `Users` · `Audit`; üstte başlık + kullanıcı menüsü (şifre değiştir, çıkış).

**Panel = Vite SPA** (ayrı frontend projesi, server tarafından servis edilir):

- Stack: **Vite + React + TypeScript + Tailwind** (+ shadcn/ui bileşenleri).
- Prod: `vite build` → `dist/`; axum ya `rust-embed` ile binary'ye gömer (tek dosya deploy), ya `ServeDir` ile klasörden verir. Tüm bilinmeyen rotalar `index.html`'e düşer (SPA fallback), API `/api` altında ayrı.
- Dev: `vite dev` + `proxy: { "/api": "http://localhost:8443" }`.
- Route'lar: `/login`, `/clusters`, `/clusters/:id`, `/entries/:id`, `/users`, `/audit`.
- Auth: login'de token alınır (cookie veya localStorage), HTTP client 401'de otomatik `/login`'e yönlendirir.

Kullanıcı mock'unun birebir karşılığı — Concept ekranı:

```
┌─ Registry Admin ──────────────────────────────────────────────────┐
│  ⌂ Concept   Clusters                              admin ▾  Çıkış │
│                                                                   │
│  VARIABLES                                    [＋ New Cluster]    │
│  ┌ Variables — 3 groups · 18 entries ────────────────────────────┐ │
│  │  LivaBudget                                                   │ │
│  │    Development · Staging · Production                         │ │
│  │      ▸ Production (6)                                         │ │
│  │        SQL_CONNECTION_STRING   Secret · String   ••••••••7f2a  │ │
│  │        REDIS_URL              Text        redis://…           │ │
│  └───────────────────────────────────────────────────────────────┘ │
│  ┌ Prompts — 0 groups · 0 entries ────────────────────────────────┐ │
│  ┌ Agent Skills — 1 group · 2 entries ────────────────────────────┐ │
│  ┌ DevOps — 0 groups · 0 entries ─────────────────────────────────┐ │
└───────────────────────────────────────────────────────────────────┘
```

Ekran listesi:

1. **Login** — kullanıcı adı + şifre; hatalı deneme girişleri loglanır, 5 hatadan sonra IP 60 sn kilitli.
2. **Concept (Clusters)** — kart listesi; kartta grup chip'leri ve sayaçlar. Kart tıklaması → Cluster detay.
3. **Cluster detay** — sol: grup ağacı (sağ tıkla ekle/yeniden adlandır/sil/taşı), sağ: entry tablosu. Kolonlar: Ad · Kind rozeti (`Secret · String`, `JSON`, `Binary`, `Bundle`) · Versiyon · Güncellenme · Aksiyonlar.
4. **Entry düzenleyici** — kind seçici rozetler; `is_secret` toggle → önizleme maskelenecek şekilde davranış değişir; büyük editör (text/json), dosya yükleme (binary/bundle, çoklu dosya + göreli yol), tags, açıklama.
5. **Versiyonlar** — sürüm listesi (kim, ne zaman, hash, boyut), restore butonu.
6. **Users** — liste (aktif/pasif, rol, son giriş), `＋ New User`, rol seçimi, şifre reset (yalnız admin), aktif/pasif değiştirme; token oluşturma görünümü (kullanıcı kendi tokenı alır, ham token yalnız ilk gösterimde görünür).
7. **Audit** — filtreli tablo; `secret.reveal` kayıtlarında "kim, hangi entry, IP, zaman" her zaman tek satır.

Görsel dili: rozetler kind'a göre renk kodlu (Secret = kırmızımsı, JSON = mor, Text = gri, Bundle = turuncu); secret satırlarındaki önizleme maskeleme client'a da aynı şekilde yansır.

---

## 7. Client — Vardrop

### Akış

```
Global hotkey (örn. Ctrl+Alt+V)
   ↓
Overlay pencere açılır: aktif metin kutusunun üstü (yer yoksa altı), mouse/caret yakını
   ↓ (yaz, 300 ms debounce)
GET /api/entries?q=…&cluster_id=?   → maskeli sonuç listesi
   ↓ ↑ ile gezin, Enter ile seç
Aksiyon (kind + is_secret'e göre):
  Copy        → arboard clipboard'a value → pencereyi kapat         (default)
  Paste       → clipboard + hedefe Ctrl+V SendInput (enigo)
  Save_to     → binary/bundle: klasör seç, dosyaları indir
  Render      → json/md önizleme modalı
   ↓
Kapanış → hotkey/clipboard geçmişinde kalıntı bırakılmaz
```

### Pencere davranışı (Tauri)

- `transparent: true, decorated: false, always_on_top: true, skip_taskbar: true, focus: false`
- Görünüp odak alınmadan kaynak pencere kaybolmasın; pencere WS_EX_NOACTIVATE benzeri davranışla aktif pencereyi değişmeden bırakmalı (paste'in doğru pencereye gitmesi için kritik).
- Caret konumu: faz 1 — mouse pozisyonu; faz 2 — Win32 `GetGUIThreadInfo` caret rect; faz 3 — UI Automation ile gerçek focused element rect (Chrome/Electron/Terminal'lerde sağlıklı).
- Tray ikonu: açık/kapalı, ayarlar, çıkış; autostart (Windows Registry Run).

### Client ayarları (settings.json + keyring)

- `server_url` (domain / IP), kullanıcı şifresi ile login → PAT üret ve Credential Manager'a yaz (her açılışta yeniden sorulmaz).
- Hotkey (default `Ctrl+Alt+V`), default aksiyon `copy|paste`, cluster filtresi (yalnız "Variables" ya da tamamı). Secret'larda otomatik paste riskli (tuş olayı sistem log'larına düşebilir): ilk sürümde secret için otomatik paste kapalı, yalnız "kopyala" sunulur.

### Arama UX

```
┌ Vardrop — Variables ▾ ─────────────────────────────┐
│ 🔍 liva sql…                                               │
│ SQL_CONNECTION_STRING   Secret·String  …/LivaBudget/Production │
│ REDIS_URL               Text        …/LivaBudget/Production │
│ …                                                          │
│ ↑↓ gez · Enter kopyala · Shift+Enter yapıştır · Esc kapat  │
└────────────────────────────────────────────────────────────┘
```

- Sonuç satırı: ad (kalın), kind rozeti, tam yol (soluk).
- Boş sorguda en sık kullanılanlar (sıklık client'ta tutulur; secret'ler adıyla listelenir ama değeri gelmeden kopyalanmaz).
- Offline: son 5 dakikanın sonucu cache (secret'lar hariç); bağlantı gelince tazelenir.

---

## 8. Security Policy Katmanı

1. **Kimlik:** argon2id; min 10 karakter şifre; panel session cookie (HttpOnly, SameSite=Strict).
2. **Token:** rastgele 24+ byte, ham token yalnız ilk yanıtta görünür; sunucuda yalnız hash saklanır; scopes (`read`, `search`, `write`, `reveal`); süre bitişi ve son kullanım zamanı loglanır.
3. **Secret at-rest:** AES-256-GCM; master key `data/master.key` (dosya izni 0600) veya ADMIN belirtir çevre değişkeni; DB yedeklerinde secret kriptolu kalır.
4. **Transit:** HTTPS zorunlu (Caddy otomatik sertifika veya rustls).
5. **Reveal ayrı izin:** `viewer` değeri listeleyip kopyalar ama düz metin göremez; `reveal` uzantı izni gerekir. Her `GET /value` (`is_secret=true`) `secret.reveal` audit kaydı üretir.
6. **ACL:** rol varsayılanı + istenirse cluster/group bazlı ek kurallar (`acl_rules`).
7. **Login koruması:** IP bazlı rate limit (tower-governor), 5 hata → 60 sn; tüm başarısızlıklar audit'e.
8. **Backup:** gecelik `sqlite .backup` + storage klasörü zip; 14 gün saklama; restore prosedürü dokümanı.

---

## 9. Extension / Handler Katmanı

Türe özel davranış, kayıtlı handler'lar üzerinden:

```rust
pub trait ContentHandler {
    fn kind(&self) -> ContentKind;
    fn validate(&self, ctx: &Value) -> Result<()>;        // json parse örn.
    fn preview(&self, ctx: &Value, secret: bool) -> String; // server önizleme
    fn actions(&self, ctx: &ClientCtx) -> Vec<EntryAction>;  // client menüsü
}
```

- Varsayılanlar: `text`, `json`, `binary`, `bundle`.
- Örnek uzantı: `md-preview` (markdown render), `env-file` (birden çok KEY=VAL → hepsini clipboard'a), `url` (tarayıcıda aç).
- Server ve client ayrı registri tutabilir; ortak `vardrop-core` crate'inde paylaşılan davranışlar durur.

---

## 10. Konfigürasyon ve Çalıştırma

```toml
# server config.toml
[server]
bind      = "0.0.0.0:8443"
public_url = "https://registry.company.local:8443"

[data]
database = "data/registry.db"
storage  = "data/storage"          # büyük binary/bundle dosyaları, manifest.json dahil

[security]
master_key_file    = "data/master.key"
secret_max_inline_kb = 16          # üstündes DOSYALARI blob_path'e düşür
session_ttl        = "12h"
password_min_len   = 10

[backup]
schedule = "daily 03:00"
keep     = 14
```

- İlk kurulum: `vardrop-server init --admin-user admin` → rastgele admin şifresi üretir veya sorar; panel ilk loginla devam.
- Windows servis (NSSM) veya systemd; Docker tek binary'yle ama veri klasörü volume.
- Panel deploy: `apps/admin-web` → `pnpm build` → `dist/` server binary'ye embed edilir (tek artefakt) veya yanyana klasör olarak `ServeDir`.
- `/healthz` + `/metrics` (Prometheus formatı, opsiyonel).

## 11. Repo (Cargo Workspace)

```
variable-manager/
├─ Cargo.toml                    # workspace
├─ docs/PLAN.md
├─ crates/
│  ├─ vardrop-core/             # tipler (Cluster, Group, Entry, ContentKind, DTO)
│  │  ├─ src/lib.rs
│  │  ├─ src/content.rs          # ContentKind, preview/mask, SHA
│  │  ├─ src/security.rs         # rol-matris, maskViewModel
│  │  └─ tests/
│  ├─ vardrop-server/           # axum + sqlx (panel SPA'sını servis eder)
│  │  ├─ src/main.rs             # config, init CLI, serve, SPA fallback
│  │  ├─ src/db/  (migrations/, queries/)
│  │  ├─ src/api/ (auth.rs, users.rs, clusters.rs, groups.rs, entries.rs, values.rs, audit.rs)
│  │  ├─ src/auth/ (argon2, token, middleware, acl, rate_limit)
│  │  ├─ src/search/ (fts)
│  │  ├─ src/storage/ (blob, aes-gcm)
│  │  ├─ src/embed.rs / dist/    # panel build çıktısı (rust-embed) veya ServeDir
│  │  └─ migrations/
│  └─ registry-client/           # tauri 2 uygulama
│     ├─ src-tauri/src/          # (main.rs, hotkey.rs, overlay.rs, api.rs, actions/, settings.rs)
│     ├─ ui/                     # popup UI (html/css/ts veya vanilla)
│     └─ tauri.conf.json
├─ apps/
│  └─ admin-web/                 # Vite + React + TS + Tailwind panel SPA
│     ├─ package.json
│     ├─ vite.config.ts          # dev proxy: /api → localhost:8443
│     └─ src/ (pages/, components/, api/)
```

---

## 12. Faz Planı

| Faz | İçerik | Çıktı kriteri |
|---|---|---|
| **F0 — İskelet** | workspace + core tipler + DB migration şeması + `/healthz` | `cargo run` server ayağa, migration'lar uygulanır |
| **F1 — Server MVP** | login + PAT + cluster/group/entry CRUD + FTS arama + secret AES + panel — Vite SPA (Concept, cluster detay, entry edit, Users) | panelde "Variables/LivaBudget/Production/SQL_CONNECTION_STRING (Secret)" girilir, maskeli görünür |
| **F2 — Client MVP** | login/hotkey/overlay popup + arama + Copy/Paste | Notepad'de Ctrl+Alt+V → popup → seçim → metin kopyalanır/yapıştırılır |
| **F3 — Sertleştirme** | ACL kuralları, audit log ekranı, versiyon restore, HTTPS, rate limit, backup | audit kayıtları panelde görülür, restore çalışır |
| **F4 — Uzantılar** | bundle/binary yükleme-indirme, md/json render, UIA caret konumu, offline cache, ek handler'lar | SKILL.md paketi client'tan klasöre dahil edilir; caret konumu Chrome'da da doğru |

## 13. Açık Karar Noktaları

1. Panel framework: React (öneri) mı Svelte/Solid mi? Tasarım dili: Tailwind + shadcn/ui (öneri).
2. Client: Tauri (öneri) mı saf Rust (winit+egui) mı?
3. Master key: dosya (öneri) mı Windows DPAPI mi?
4. FTS indexlemede çok büyük JSON değerleri kırpılsın mı? (öneri: ilk 4 KB)
5. Otomatik paste (SendInput) default açık mı kapalı mı? (öneri: açık, ama secret için reveal izni istenir)
