//! Ortak yardımcılar: audit kaydı, doğrulama, dinamik IN listeleri.

use crate::error::{ApiError, ApiResult};
use sqlx::Row;

/// Audit log'a satır yazar; hatalar sessizce loglanır (akışı bozmamalı).
pub async fn audit(
    db: &sqlx::SqlitePool,
    actor: Option<(String, String)>, // (user_id, username)
    action: &str,
    resource: Option<&str>,
    ip: Option<&str>,
    user_agent: Option<&str>,
    detail: Option<serde_json::Value>,
) {
    let (uid, uname) = match actor {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    };
    let result = sqlx::query(
        "INSERT INTO audit_log (user_id, username, action, resource, ip, user_agent, detail, created_at)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(uid)
    .bind(uname)
    .bind(action)
    .bind(resource)
    .bind(ip)
    .bind(user_agent)
    .bind(detail.map(|d| d.to_string()))
    .bind(vardrop_core::now_ms())
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!("audit yazılamadı: {e}");
    }
}

/// Kullanıcı adı kontrolü: 3+ karakter, harf/rakam/._-
pub fn valid_username(name: &str) -> ApiResult<()> {
    let ok = name.len() >= 3
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::bad("kullanıcı adı: 3+ karakter (harf, rakam, _ . -)"))
    }
}

/// tags JSON kolonunu parse eder; bozuk veri boş listeye döner.
pub fn parse_tags(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

/// groups tablosunun tam yol haritasını üretir: group_id -> "Root/Alt/Grup" (cluster bazlı).
pub async fn group_paths_of(
    db: &sqlx::SqlitePool,
    cluster_ids: &[String],
) -> ApiResult<std::collections::HashMap<String, String>> {
    let mut paths = std::collections::HashMap::new();
    for cid in cluster_ids {
        let rows = sqlx::query("SELECT id, parent_id, name FROM groups WHERE cluster_id = ?")
            .bind(cid)
            .fetch_all(db)
            .await?;
        // builder: id -> (parent, name)
        let mut builders: std::collections::HashMap<String, (Option<String>, String)> =
            std::collections::HashMap::new();
        for r in &rows {
            let id: String = r.get("id");
            let parent: Option<String> = r.get("parent_id");
            let name: String = r.get("name");
            builders.insert(id, (parent, name));
        }
        for r in &rows {
            let id: String = r.get("id");
            let mut chain = Vec::new();
            let mut cur = Some(id.clone());
            let mut hop_guard = 0;
            while let Some(gid) = cur {
                let Some((parent, name)) = builders.get(&gid) else { break };
                chain.push(name.clone());
                cur = parent.clone();
                hop_guard += 1;
                if hop_guard > 128 {
                    break; // döngü koruması
                }
            }
            chain.reverse();
            paths.insert(id, chain.join("/"));
        }
    }
    Ok(paths)
}

/// Verilen grubun altında kalan tüm grup kimlikleri (kendisi dahil).
pub async fn subtree_group_ids(db: &sqlx::SqlitePool, root: &str) -> ApiResult<Vec<String>> {
    let rows = sqlx::query(
        "WITH RECURSIVE sub(id) AS (
            SELECT id FROM groups WHERE id = ?1
            UNION ALL
            SELECT g.id FROM groups g JOIN sub ON g.parent_id = sub.id
         )
         SELECT id FROM sub",
    )
    .bind(root)
    .fetch_all(db)
    .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("id")).collect())
}
