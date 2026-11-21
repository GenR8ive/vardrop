#!/bin/sh
set -eu

CONFIG_SRC="${VARDROP_CONFIG:-/etc/vardrop/config.toml}"
CONFIG="/tmp/vardrop-config.toml"
cp "$CONFIG_SRC" "$CONFIG"
chmod 644 "$CONFIG"

if [ -n "${BIND:-}" ]; then
  sed -i "s|^bind *=.*|bind = \"${BIND}\"|" "$CONFIG"
fi
if [ -n "${PUBLIC_URL:-}" ]; then
  sed -i "s|^public_url *=.*|public_url = \"${PUBLIC_URL}\"|" "$CONFIG"
fi

mkdir -p /data/storage
chown -R vardrop:vardrop /data
chmod 700 /data

export VARDROP_ADMIN_PASSWORD_FILE="${VARDROP_ADMIN_PASSWORD_FILE:-/data/initial-admin-password.txt}"

if [ ! -s /data/registry.db ]; then
  echo "[entrypoint] veritabanı yok — init"
  runuser -u vardrop -- vardrop-server init --config "$CONFIG" --admin-user "${ADMIN_USER:-admin}"
  if [ -f /data/initial-admin-password.txt ]; then
    chmod 600 /data/initial-admin-password.txt
    echo "[entrypoint] admin şifresi: /data/initial-admin-password.txt (okuyup silin)"
  fi
fi

exec runuser -u vardrop -- vardrop-server serve --config "$CONFIG"
