//! Cluster CRUD + kart listesi (Concept ekranı).

use crate::auth::middleware::{require_cluster_perm, require_perm, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::util::audit;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;use axum::{Json, Router};
use vardrop_core::{new_id, now_ms, ClusterCard, GroupChip};
use serde::Deserialize;
use sqlx::Row;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/clusters", get(list).post(create))
        .route("/api/clusters/{id}", get(get_one).patch(update).delete(delete_cluster))
}

#[derive(sqlx::FromRow)]
struct ClusterRow {
    id: String,
    name: String,
    slug: String,
    description: Option<String>,
    color: Option<String>,
    created_at: i64,
    groups_count: i64,
    entries_count: i64,
}

/// GET /api/clusters — Concept kartları (yalnız entry.view yetkisi olanlar)
pub async fn list(user: AuthUser, State(state): State<SharedState>) -> ApiResult<Json<Vec<ClusterCard>>> {
    let rows: Vec<ClusterRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.slug, c.description, c.color, c.created_at,
                (SELECT COUNT(*) FROM groups g WHERE g.cluster_id = c.id) AS groups_count,
                (SELECT COUNT(*) FROM entries e WHERE e.cluster_id = c.id) AS entries_count
         FROM clusters c ORDER BY c.name",
    )
    .fetch_all(&state.db)
    .await?;

    // RBAC: yalnız görme yetkisi olan cluster kartları döner.
    let rows: Vec<ClusterRow> = rows
        .into_iter()
        .filter(|r| user.perms.can_on(&r.id, "entry.view"))
        .collect();

    // grup chipleri: entry sayısıyla birlikte
    let g_rows = sqlx::query(
        "SELECT g.cluster_id, g.id, g.name,
                (SELECT COUNT(*) FROM entries e WHERE e.group_id = g.id) AS entry_count
         FROM groups g",
    )
    .fetch_all(&state.db)
    .await?;

    let mut by_cluster: std::collections::HashMap<String, Vec<GroupChip>> = Default::default();
    for r in &g_rows {
        let cid: String = r.get("cluster_id");
        by_cluster.entry(cid).or_default().push(GroupChip {
            id: r.get("id"),
            name: r.get("name"),
            entry_count: r.get("entry_count"),
        });
    }

    let mut out = Vec::new();
    for r in rows {
        let mut chips = by_cluster.remove(&r.id).unwrap_or_default();
        chips.sort_by(|a, b| b.entry_count.cmp(&a.entry_count).then(a.name.cmp(&b.name)));
        chips.truncate(5);
        out.push(ClusterCard {
            id: r.id,
            name: r.name,
            slug: r.slug,
            description: r.description,
            color: r.color,
            created_at: r.created_at,
            groups_count: r.groups_count,
            entries_count: r.entries_count,
            top_groups: chips,
        });
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct CreateClusterReq {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c.is_whitespace() || c == '-' || c == '_' || c == '/' {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        vardrop_core::new_id("cluster").replace('_', "-")
    } else {
        trimmed.to_string()
    }
}

async fn unique_slug(state: &SharedState, base: &str) -> ApiResult<String> {
    let mut candidate = base.to_string();
    let mut n = 1;
    loop {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM clusters WHERE slug = ?").bind(&candidate).fetch_optional(&state.db).await?;
        if exists.is_none() {
            return Ok(candidate);
        }
        n += 1;
        candidate = format!("{base}-{n}");
    }
}

/// POST /api/clusters
pub async fn create(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateClusterReq>,
) -> ApiResult<(StatusCode, Json<ClusterCard>)> {
    // cluster oluşturma global eylem: yalnız global cluster.manage
    require_perm(&user, "cluster.manage")?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad("cluster adı gerekli"));
    }
    let cid = new_id("c");
    let base = req.slug.map(|s| slugify(&s)).unwrap_or_else(|| slugify(name));
    let slug = unique_slug(&state, &base).await?;

    sqlx::query("INSERT INTO clusters (id, slug, name, description, color, created_by, created_at) VALUES (?,?,?,?,?,?,?)")
        .bind(&cid)
        .bind(&slug)
        .bind(name)
        .bind(req.description.clone())
        .bind(req.color.clone())
        .bind(&user.user.id)
        .bind(now_ms())
        .execute(&state.db)
        .await?;

    audit(&state.db, Some(user.uid()), "cluster.create", Some(&cid), None, None,
        Some(serde_json::json!({ "name": name, "slug": slug }))).await;

    Ok((
        StatusCode::CREATED,
        Json(ClusterCard {
            id: cid,
            name: name.to_string(),
            slug,
            description: req.description,
            color: req.color,
            created_at: now_ms(),
            groups_count: 0,
            entries_count: 0,
            top_groups: vec![],
        }),
    ))
}

/// GET /api/clusters/{id}
pub async fn get_one(user: AuthUser, State(state): State<SharedState>, Path(id): Path<String>) -> ApiResult<Json<ClusterCard>> {
    require_cluster_perm(&user, &id, "entry.view")?;
    let row: Option<ClusterRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.slug, c.description, c.color, c.created_at,
                (SELECT COUNT(*) FROM groups g WHERE g.cluster_id = c.id) AS groups_count,
                (SELECT COUNT(*) FROM entries e WHERE e.cluster_id = c.id) AS entries_count
         FROM clusters c WHERE c.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;
    let Some(r) = row else {
        return Err(ApiError::not_found("cluster bulunamadı"));
    };
    let chips: Vec<GroupChip> = sqlx::query(
        "SELECT g.id, g.name,
                (SELECT COUNT(*) FROM entries e WHERE e.group_id = g.id) AS entry_count
         FROM groups g WHERE g.cluster_id = ?",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r: sqlx::sqlite::SqliteRow| GroupChip { id: r.get("id"), name: r.get("name"), entry_count: r.get("entry_count") })
    .collect();

    Ok(Json(ClusterCard {
        id: r.id.clone(),
        name: r.name.clone(),
        slug: r.slug,
        description: r.description,
        color: r.color,
        created_at: r.created_at,
        groups_count: r.groups_count,
        entries_count: r.entries_count,
        top_groups: chips,
    }))
}

#[derive(Deserialize)]
pub struct PatchClusterReq {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

/// PATCH /api/clusters/{id}
pub async fn update(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<PatchClusterReq>,
) -> ApiResult<Json<serde_json::Value>> {
    require_cluster_perm(&user, &id, "cluster.manage")?;
    if let Some(n) = req.name.as_deref() {
        if n.trim().is_empty() {
            return Err(ApiError::bad("cluster adı boş olamaz"));
        }
    }
    sqlx::query(
        "UPDATE clusters SET name = COALESCE(?, name), description = COALESCE(?, description), color = COALESCE(?, color) WHERE id = ?",
    )
    .bind(req.name.clone())
    .bind(req.description.clone())
    .bind(req.color.clone())
    .bind(&id)
    .execute(&state.db)
    .await?;
    audit(&state.db, Some(user.uid()), "cluster.update", Some(&id), None, None,
        Some(serde_json::json!({ "name": req.name }))).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/clusters/{id} — cascade (gruplar + entries + versiyonlar)
pub async fn delete_cluster(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_cluster_perm(&user, &id, "cluster.manage")?;
    let entry_ids: Vec<String> =
        sqlx::query("SELECT id FROM entries WHERE cluster_id = ?").bind(&id).fetch_all(&state.db).await?
            .into_iter().map(|r: sqlx::sqlite::SqliteRow| r.get::<String, _>("id")).collect();
    for eid in &entry_ids {
        crate::storage::remove_entry_blobs(&state, eid).await;
    }
    sqlx::query("DELETE FROM clusters WHERE id = ?").bind(&id).execute(&state.db).await?;
    audit(&state.db, Some(user.uid()), "cluster.delete", Some(&id), None, None,
        Some(serde_json::json!({ "entries": entry_ids.len() }))).await;
    Ok(Json(serde_json::json!({ "ok": true, "deleted_entries": entry_ids.len() })))
}

/// Grup ağacı: GET /api/clusters/{id}/groups (clusters modülünde, entry sayacıyla)
pub async fn groups_tree(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<vardrop_core::GroupNode>>> {
    require_cluster_perm(&user, &id, "entry.view")?;
    // ağaç verisi groups.rs'ten geliyor
    let tree = crate::api::groups::tree_for_cluster(&user, &state, &id).await?;
    Ok(Json(tree))
}
