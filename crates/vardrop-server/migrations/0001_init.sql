-- Vardrop — başlangıç şeması

CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL DEFAULT '',
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'viewer', -- admin | editor | viewer
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    INTEGER NOT NULL,
    last_login_at INTEGER
);

CREATE TABLE clusters (
    id          TEXT PRIMARY KEY,
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT,
    created_by  TEXT,
    created_at  INTEGER NOT NULL
);

CREATE TABLE groups (
    id         TEXT PRIMARY KEY,
    cluster_id TEXT NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    parent_id  TEXT REFERENCES groups(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    UNIQUE (cluster_id, parent_id, name)
);

CREATE TABLE entries (
    id              TEXT PRIMARY KEY,
    cluster_id      TEXT NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    group_id        TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL,            -- text | json | yaml | binary | bundle
    is_secret       INTEGER NOT NULL DEFAULT 0,
    description     TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    current_version INTEGER NOT NULL DEFAULT 1,
    created_by      TEXT,
    created_at      INTEGER NOT NULL,
    updated_by      TEXT,
    updated_at      INTEGER NOT NULL,
    UNIQUE (cluster_id, group_id, name)
);

CREATE TABLE entry_versions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id    TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    version     INTEGER NOT NULL,
    value_text  TEXT,                        -- düz metin (inline)
    value_enc   BLOB,                        -- secret inline: nonce||AES-256-GCM
    blob_path   TEXT,                        -- dosya tabanlı içerik (secret ise şifreli)
    value_hash  TEXT,                        -- plaintext sha256
    size        INTEGER,
    created_by  TEXT,
    created_at  INTEGER NOT NULL,
    UNIQUE (entry_id, version)
);

CREATE TABLE bundle_files (
    id        TEXT PRIMARY KEY,
    entry_id  TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    version   INTEGER NOT NULL,
    file_path TEXT NOT NULL,                 -- paket içi göreli yol (ör. SKILL.md)
    blob_path TEXT,                          -- storage yolu (güvenli: {entry}/{sha256})
    size      INTEGER,
    sha256    TEXT,
    is_secret INTEGER NOT NULL DEFAULT 0,
    UNIQUE (entry_id, version, file_path)
);

CREATE TABLE api_tokens (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,       -- sha256(token); ham token saklanmaz
    scopes       TEXT NOT NULL,              -- ["read","write","admin"]
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER,
    last_used_at INTEGER,
    revoked_at   INTEGER
);

CREATE TABLE acl_rules (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_type  TEXT NOT NULL,             -- user | role
    subject       TEXT NOT NULL,
    resource_type TEXT NOT NULL,             -- cluster | group
    resource_id   TEXT NOT NULL,
    permission    TEXT NOT NULL,             -- view | use | edit | admin
    UNIQUE (subject_type, subject, resource_type, resource_id)
);

CREATE TABLE audit_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    TEXT,
    username   TEXT,
    action     TEXT NOT NULL,
    resource   TEXT,
    ip         TEXT,
    user_agent TEXT,
    detail     TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_groups_cluster   ON groups(cluster_id);
CREATE INDEX idx_entries_cluster  ON entries(cluster_id);
CREATE INDEX idx_entries_group    ON entries(group_id);
CREATE INDEX idx_ev_entry         ON entry_versions(entry_id);
CREATE INDEX idx_bundle_entry     ON bundle_files(entry_id, version);
CREATE INDEX idx_tokens_user      ON api_tokens(user_id);
CREATE INDEX idx_audit_created    ON audit_log(created_at DESC);
CREATE INDEX idx_audit_action     ON audit_log(action);
