-- Vardrop — bağlantı kodları (istemci exe'sine gömülen imzalı JWT'ler)

CREATE TABLE connect_codes (
    jti        TEXT PRIMARY KEY,        -- JWT payload.jti; kodu tanımlar
    origin     TEXT NOT NULL,           -- gömülü sunucu adresi (scheme://host[:port])
    created_by TEXT,
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);

CREATE INDEX idx_connect_codes_created ON connect_codes(created_at DESC);
