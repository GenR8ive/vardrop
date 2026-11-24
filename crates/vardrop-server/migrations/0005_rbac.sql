-- RBAC: rol grupları, departmanlar, (kullanıcı|departman)×rol×cluster atamaları.

CREATE TABLE roles (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    permissions TEXT NOT NULL DEFAULT '[]',   -- JSON: ["cluster.view", ...]
    is_system   INTEGER NOT NULL DEFAULT 0,   -- sistem rolleri silinemez
    created_at  INTEGER NOT NULL
);

CREATE TABLE departments (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);

CREATE TABLE department_members (
    department_id TEXT NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE (department_id, user_id)
);

-- subject: 'user' | 'department'; cluster_id NULL = global.
CREATE TABLE grants (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_type TEXT NOT NULL,
    subject_id   TEXT NOT NULL,
    role_id      TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    cluster_id   TEXT REFERENCES clusters(id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    UNIQUE (subject_type, subject_id, role_id, cluster_id)
);
CREATE INDEX idx_grants_subject ON grants(subject_type, subject_id);
CREATE INDEX idx_grants_cluster  ON grants(cluster_id);
-- NULL cluster_id'yi benzersizlik kapsamına al (SQLite NULL'ları ayrı sayar):
CREATE UNIQUE INDEX ux_grants_global
    ON grants(subject_type, subject_id, role_id, COALESCE(cluster_id, ''));

-- Sistem rolleri. Zaman: migration anı (ms).
INSERT INTO roles (id, name, description, permissions, is_system, created_at) VALUES
 ('role_admin', 'admin', 'Tüm yetkiler (sistem rolü)',
  '["cluster.view","cluster.manage","entry.view","entry.create","entry.edit","entry.delete","secret.read","users.manage","roles.manage","audit.read"]',
  1, CAST(strftime('%s','now') AS INTEGER) * 1000),
 ('role_editor', 'editor', 'Cluster içeriğini okur/yazar, secret okur (sistem rolü)',
  '["cluster.view","entry.view","entry.create","entry.edit","entry.delete","secret.read"]',
  1, CAST(strftime('%s','now') AS INTEGER) * 1000),
 ('role_viewer', 'viewer', 'Yalnız okur; secret göremez (sistem rolü)',
  '["cluster.view","entry.view"]',
  1, CAST(strftime('%s','now') AS INTEGER) * 1000);

-- users.role -> global grant (geriye uyumluluk: davranış birebir korunur).
INSERT OR IGNORE INTO grants (subject_type, subject_id, role_id, cluster_id, created_at)
SELECT 'user', u.id,
       CASE u.role WHEN 'admin' THEN 'role_admin'
                   WHEN 'editor' THEN 'role_editor'
                   ELSE 'role_viewer' END,
       NULL, CAST(strftime('%s','now') AS INTEGER) * 1000
FROM users u;

-- acl_rules hiçbir kodda kullanılmadı; yerini grants aldı.
DROP TABLE IF EXISTS acl_rules;
