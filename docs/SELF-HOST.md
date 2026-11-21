# Sunucuyu kendin host et

Vardrop'ın sunucusu sizin makinenizde çalışır. İstemci (Vardrop)
GitHub Releases'ten iner ve sizin `https://...` adresinize bağlanır.
Bizim tarafımızda hesap, telemetri veya merkezi sunucu yoktur.

İki kurulum yolu: Docker (önerilen) veya ikili + systemd.

## Ne yedeklenir

Üçü birlikte, biri eksikse veri ya şifre çözülemez ya da dosyalar kaybolur:

| Dosya | Ne |
|---|---|
| `master.key` | AES-256-GCM anahtarı. Kayıp = secret'lar çöp |
| `registry.db` | SQLite kayıt defteri |
| `storage/` | binary / bundle dosyaları |

`config.toml` içinde gizli yoktur; yine de yedekleyin.

## 1) Docker

```bash
docker compose up -d --build
docker compose exec registry cat /data/initial-admin-password.txt
docker compose exec registry rm /data/initial-admin-password.txt
```

Panel: `http://127.0.0.1:8080` (yalnız bu makinenin loopback'i).

Yayınlanmış imaj (tag atıldıktan sonra):

```yaml
services:
  registry:
    image: ghcr.io/<org>/vardrop:latest
    # build satırını silin
```

GHCR paketi ilk push'tan sonra GitHub'da **public** yapılmalı; aksi halde `docker pull` anonim başarısız olur.

### TLS

`deploy/Caddyfile.docker` içindeki alan adını ve `docker-compose.yml` içindeki `DOMAIN` + `PUBLIC_URL` değerlerini değiştirin. DNS kaydı bu sunucuya baksın.

```bash
docker compose --profile tls up -d
```

`PUBLIC_URL=https://vars.example.com` vermezseniz panel çalışır ama istemciye yazacağınız adres sizin domain'inizdir; `public_url` bilgi amaçlıdır, yönlendirme yapmaz.

## 2) Release arşivi (Docker'sız)

GitHub Releases'tan `vardrop-server-<platform>` indirin. İçinde ikili, `panel/` ve `config.toml` vardır.

```bash
tar -xzf vardrop-server-linux-x64.tar.gz
cd vardrop
./vardrop-server init
./vardrop-server serve
```

Windows: zip'i açın, `vardrop-server.exe init` sonra `serve`. Çalışma dizini arşivin kökü olsun; yollar görecelidir.

`init` admin şifresini bir kez basar. Sonraki açılışlarda yalnız `serve`.

### systemd

```bash
sudo useradd --system --home /var/lib/vardrop --shell /usr/sbin/nologin vardrop
sudo mkdir -p /opt/vardrop /var/lib/vardrop /etc/vardrop
sudo cp vardrop-server /opt/vardrop/
sudo cp -a panel /opt/vardrop/panel
sudo cp deploy/config.systemd.example.toml /etc/vardrop/config.toml
# public_url ve gerekirse yolları düzenleyin
sudo chown -R vardrop:vardrop /var/lib/vardrop
sudo cp deploy/vardrop-server.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now vardrop-server
sudo journalctl -u vardrop-server -n 50
```

`init` henüz çalışmadıysa bir kez:

```bash
sudo -u vardrop vardrop-server init --config /etc/vardrop/config.toml
```

İkili PATH'te değilse tam yolu yazın. Şifre journal'a düşer; kaydedip log rotasyonuna güvenmeyin, hemen alın.

### Ters proxy

`deploy/Caddyfile` örnek. Nginx eşdeğeri:

```nginx
server {
    listen 443 ssl;
    server_name vars.example.com;
    # ssl_certificate ...
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto https;
        client_max_body_size 64m;
    }
}
```

Sunucuyu `0.0.0.0` ile TLS'siz internete açmayın. PAT ve oturum çerezi yoktur; Bearer token düz HTTP'de okunur.

## 3) İstemci bağlantısı

İstemci bağlantısı **PAT akışıyla** kurulur: panel → **Tokenlar** sayfasından
read kapsamlı PAT üretin (geçerlilik 7/30/90 gün… seçilebilir; token yalnız bir
kez gösterilir), sonra exe'nin kurulum ekranına **sunucu adresi + PAT** girin.
İstemci token'ı `GET /api/auth/me` ile doğrulayıp kaydeder. Token panelden
iptal edilebilir; son kullanım zamanı Tokenlar sayfasında görünür.

`cors_allow_all` prod'da `false` kalsın. Panel aynı origin'den servis edilir; ayrı porttaki vite yalnız geliştirme içindir.

## 4) İstemci dağıtımı

İstemci kurulum dosyaları **GitHub Releases**'ten iner (`Vardrop` → platform
paketi). Sunucu paketleri istemci içermez; dağıtım tek kanaldır.

1. Kullanıcı GitHub Releases'ten platform paketini indirir.
2. Kurulumdan sonra exe'nin kurulum ekranına sunucu adresini ve panelden
   ürettiği read PAT'i girer.

## Sınırlar

- Tek süreç, tek SQLite. Birden fazla `serve` aynı dosyaya yazmasın.
- Gövde limiti 64 MB.
- Master key rotasyonu yok. Anahtar değişirse eski secret çözülmez.
- Güncelleme: yeni imaj/ikili, volume veya `data/` durur. Migrasyon açılışta uygulanır.
