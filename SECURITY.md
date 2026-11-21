# Güvenlik

Vardrop bir secret kasasıdır. Açık kaynak olması, host ettiğiniz
verinin bizim elimizden geçtiği anlamına gelmez: sunucu sizde, istemci
sizin makinenizde, ağ sizin TLS'iniz.

Güvenlik açığı bildirimi: public issue açmadan önce bakımcıya özel kanaldan
yazın. Henüz `SECURITY` adresi tanımlanmadıysa depoyu yayınlayan kişiye
doğrudan ulaşın. Düzeltme çıkmadan PoC'u herkese açmayın.

## Tehdit modeli (kısa)

Korunan: diskteki secret (AES-256-GCM, `master.key`), parola (argon2id),
API token (yalnız sha256 hash saklanır, ham değer bir kez gösterilir).

Korunmayan, sizin işiniz:

- `master.key` dosyasının kendisi. Uygulama anahtarı bir KMS'e sarmaz.
- TLS. Süreç düz HTTP konuşur; internete Caddy/Nginx ile çıkın.
- İstemci makinesinde aynı kullanıcı olarak çalışan kötü yazılım.
- Panel oturumu `localStorage` (`vm_token`). XSS veya paylaşılan tarayıcı profili okur.

## İstemci bağlantı bilgisi

Vardrop sunucu adresini ve PAT'i şöyle tutar:

| OS | Dizin |
|---|---|
| Windows | `%LOCALAPPDATA%\vardrop\` |
| macOS | `~/Library/Application Support/vardrop/` |
| Linux | `$XDG_CONFIG_HOME/vardrop` veya `~/.config/vardrop` |

Dosyalar: `server.txt`, `token.txt`. Unix'te dizin `0700`,
dosya `0600`. Windows'ta sınır, kullanıcı profilinin ACL'idir; DPAPI ile sarılmaz.

**PAT akışı**: istemci yalnız sunucu adresi + read kapsamlı PAT ile bağlanır.
Token'ın DB'de sha256 hash'i durur; panelden iptal edilebilir, geçerlilik süresi
seçilebilir (1–3650 gün), her kullanımda `last_used_at` güncellenir ve secret
okumalar audit'te `secret.reveal` olarak kayıt altındadır. Token ele geçerse
tek etkili savunma budur: iptal + audit görünürlüğü. Sunucu adresini gizlemek
bir güvenlik katmanı değildir (port taramasıyla bulunur); erişimi token zorunluluğu
(401), login bruteforce kilidi ve argon2 hash engeller, ağ dinlemesini TLS engeller.

**Secret PIN kapısı (opsiyonel)**: kullanıcı bir PIN belirleyebilir (Tokenlar
sayfası; argon2 hash'lenir, 4–64 karakter). Belirleyen için **secret okuma**
(text/json/yaml değeri, binary, secret paket dosyası) yalnız PIN ile açılan
10 dakikalık unlock penceresinde serbesttir (`POST /api/auth/unlock`,
token bazlı, RAM'de). Çalıntı token/PAT tek başına secret okuyamaz — ikinci
faktör. Kaba kuvvet: 5 hatalı PIN → IP 60 sn kilit; `auth.unlock.failed`
audit'te. PIN sunucuda doğrulanır (istemcide değil — atlanamaz). Sınır:
keylogger'lı makinede PIN de çalınır; makine güvenli değilse hiçbir istemci
tarafı katmanı kurtarmaz.

Eski kurulumlar `variable-filler` (Roaming veya Local) kullanıyordu; ilk açılışta ayar yenisine taşınır.
İlk okumada Local'e taşınır ve eski dosya silinir. Roaming profil / klasör
yönlendirme PAT'i dosya sunucusuna kopyalayabilirdi; bu yüzden Local.

OS anahtarlığı (keyring) bilerek yok. Kurumsal WDAC ortamında Windows
Credential Manager süreçler arası güvenilir değildi. Bağımlılık kaldırıldı.
Sonuç: PAT düz metin dosyadır. Aynı kullanıcıdaki yedek, disk imajı, başka
süreç okuyabilir.

PAT ayrıca webview'a gider. HTTP çağrıları Rust'ta değil, sayfanın `fetch`'inde.
`get_settings` ham token'ı JS'e verir. Yerel arayüz uzaktan HTML yüklemez;
sunucudan gelen isimler `esc()` ile basılır, renk yalnız `#hex` ise style'a girer.
CSP `connect-src` kullanıcı sunucusu bilinmediği için `http:` ve `https:` açıktır;
`script-src` Tauri köprüsü için `'unsafe-inline'` içerir. CSP token sınırını çizmez.
Webview'da çalışan her kod (veya aynı kullanıcıdaki bir debugger) PAT'i görür.
Bu, dosyadaki düz metinle aynı güven sınırıdır: **kullanıcı hesabı**.

Ne yapılmadı, sıradaki sertleştirme:

1. HTTP'yi Rust'a almak; webview'a token hiç vermemek.
2. Windows DPAPI / macOS Keychain / Linux secret portal ile dosyayı sarmak.
   WDAC sapması tekrar ölçülmeden keyring'e dönülmemeli.
3. İstemci sertifika sabitleme yok. Kötü vekil, kullanıcı `http://` veya sahte
   CA kabul ederse PAT'i görür. Yerel olmayan `http://` kurulum ekranında uyarılır, engellenmez (LAN).

`Unut` ve arama çubuğundaki `bağlantı` kayıtlı adresi ve PAT'i siler.
Sunucudaki token iptal edilmez; panelden de revoke edin.

## Sunucu

- Varsayılan bind `127.0.0.1`. Docker imajı konteyner içinde `0.0.0.0:8080` dinler, compose bunu host'ta yalnız `127.0.0.1:8080` yayınlar.
- `cors_allow_all` kod varsayılanı `false`. Geliştirme şablonunda `true` (vite).
- Login: IP başına 5 hata → 60 sn kilit, bellek içi, restart sıfırlar.
- Token TTL varsayılan 90 gün. İstemci için `read` kapsamı yeter.
