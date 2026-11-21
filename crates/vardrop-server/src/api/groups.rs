//! Grup işlemleri: ağaç, oluştur, tasi/yeniden adlandır, sil (cascade).

use crate::auth::middleware::{require, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::util::{audit, subtree_group_ids};
use axum::extract::{Path, Query, State};
use axum::routing::post;
use axum::{Json, Router};
use vardrop_core::{new_id, GroupNode, Scope};
use serde::Deserialize;
use sqlx::Row;

#[allow(dead_code)]
#[derive(Clone)]
pub struct GroupRow {
    pub id: String,
    pub cluster_id: String,
    pub parent_id: Option<String>,
    pub name: String,
}

pub async fn fetch_group(db: &sqlx::SqlitePool, id: &str) -> ApiResult<GroupRow> {
    let r = sqlx::query("SELECT id, cluster_id, parent_id, name FROM groups WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::not_found("grup bulunamadı"))?;
    Ok(GroupRow {
        id: r.get("id"),
        cluster_id: r.get("cluster_id"),
        parent_id: r.get("parent_id"),
        name: r.get("name"),
    })
}

/// POST /api/clusters/{id}/groups
pub async fn create_group(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(cluster_id): Path<String>,
    Json(req): Json<CreateGroupReq>,
) -> ApiResult<(axum::http::StatusCode, Json<GroupNode>)> {
    require(&user, Scope::Write)?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad("grup adı gerekli"));
    }
    if name.contains('/') {
        return Err(ApiError::bad("grup adı '/' içeremez"));
    }
    // parent bu cluster'a mi ait?
    let parent = match req.parent_id.as_deref() {
        Some(pid) => {
            let g = fetch_group(&state.db, pid).await?;
            if g.cluster_id != cluster_id {
                return Err(ApiError::bad("parent, cluster'a ait değil"));
            }
            Some(g)
        }
        None => None,
    };

    let dup: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM groups WHERE cluster_id = ? AND (parent_id IS ?) AND name = ?",
    )
    .bind(&cluster_id)
    .bind(parent.as_ref().map(|g| g.id.clone()))
    .bind(&name)
    .fetch_optional(&state.db)
    .await?;
    if dup.is_some() {
        return Err(ApiError::conflict("aynı yerde aynı adlı grup var"));
    }

    let gid = new_id("g");
    sqlx::query("INSERT INTO groups (id, cluster_id, parent_id, name) VALUES (?,?,?,?)")
        .bind(&gid)
        .bind(&cluster_id)
        .bind(parent.as_ref().map(|g| g.id.clone()))
        .bind(&name)
        .execute(&state.db)
        .await?;

    audit(&state.db, Some(user.uid()), "group.create", Some(&gid), None, None,
        Some(serde_json::json!({ "name": name, "parent": parent.as_ref().map(|g| g.id.clone()) }))).await;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(GroupNode {
            id: gid,
            cluster_id,
            parent_id: parent.map(|g| g.id),
            name,
            entry_count: 0,
            children: vec![],
        }),
    ))
}

/// Entry sayıları: group_id -> count
async fn counts_map(db: &sqlx::SqlitePool, cluster_id: &str) -> ApiResult<std::collections::HashMap<String, i64>> {
    let rows = sqlx::query("SELECT group_id, COUNT(*) AS cnt FROM entries WHERE cluster_id = ? GROUP BY group_id")
        .bind(cluster_id)
        .fetch_all(db)
        .await?;
    let mut m = std::collections::HashMap::new();
    for r in rows {
        m.insert(r.get::<String, _>("group_id"), r.get::<i64, _>("cnt"));
    }
    Ok(m)
}

/// Liste → ağaç düğümleri
pub async fn tree_for_cluster(
    _user: &AuthUser,
    state: &SharedState,
    cluster_id: &str,
) -> ApiResult<Vec<GroupNode>> {
    let rows = sqlx::query("SELECT id, cluster_id, parent_id, name FROM groups WHERE cluster_id = ? ORDER BY name")
        .bind(cluster_id)
        .fetch_all(&state.db)
        .await?;
    let counts = counts_map(&state.db, cluster_id).await?;

    let mut by_parent: std::collections::HashMap<Option<String>, Vec<String>> = Default::default();
    let mut nodes: std::collections::HashMap<String, GroupNode> = Default::default();
    for r in &rows {
        let id: String = r.get("id");
        let parent: Option<String> = r.get("parent_id");
        let name: String = r.get("name");
        let cid: String = r.get("cluster_id");
        nodes.insert(id.clone(), GroupNode {
            id: id.clone(),
            cluster_id: cid,
            parent_id: parent.clone(),
            name: name.clone(),
            entry_count: counts.get(&id).copied().unwrap_or(0),
            children: vec![],
        });
        by_parent.entry(parent.clone()).or_default().push(id);
    }

    fn assemble(
        parent: &Option<String>,
        by_parent: &std::collections::HashMap<Option<String>, Vec<String>>,
        nodes: &mut std::collections::HashMap<String, GroupNode>,
    ) -> Vec<GroupNode> {
        let ids = by_parent.get(parent).cloned().unwrap_or_default();
        ids.into_iter()
            .filter_map(|id| {
                let mut n = nodes.remove(&id)?;
                n.children = assemble(&Some(id), by_parent, nodes);
                Some(n)
            })
            .collect()
    }

    Ok(assemble(&None, &by_parent, &mut nodes))
}

#[derive(Deserialize)]
pub struct CreateGroupReq {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Deserialize)]
pub struct PatchGroupReq {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(serde::Deserialize, Default)]
pub struct DeleteOpts {
    #[serde(default)]
    pub cascade: bool,
}

/// PATCH /api/groups/{id}
pub async fn update_group(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<PatchGroupReq>,
) -> ApiResult<Json<serde_json::Value>> {
    require(&user, Scope::Write)?;
    let g = fetch_group(&state.db, &id).await?;

    if let Some(name) = req.name.as_deref() {
        let name = name.trim();
        if name.is_empty() || name.contains('/') {
            return Err(ApiError::bad("geçersiz grup adı"));
        }
        let dup: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM groups WHERE cluster_id = ? AND (parent_id IS ?) AND name = ? AND id != ?",
        )
        .bind(&g.cluster_id)
        .bind(g.parent_id.clone())
        .bind(name)
        .bind(&id)
        .fetch_optional(&state.db)
        .await?;
        if dup.is_some() {
            return Err(ApiError::conflict("aynı yerde aynı adlı grup var"));
        }
        sqlx::query("UPDATE groups SET name = ? WHERE id = ?").bind(name).bind(&id).execute(&state.db).await?;
    }

    if let Some(new_parent) = &req.parent_id {
        // kök: null'a dönememe — pass parent_id: null kök taşıma (desteklenir)
        if Some(id.clone()) == Some(new_parent.clone()) {
            return Err(ApiError::bad("grup kendisine taşınamaz"));
        }
        let np = fetch_group(&state.db, new_parent).await?;
        if np.cluster_id != g.cluster_id {
            return Err(ApiError::bad("hedef grup başka cluster'da"));
        }
        // döngü: hedef, taşınanın kendisi veya altı olamaz
        let ancestors = subtree_group_ids(&state.db, new_parent).await?;
        if ancestors.contains(&id) {
            return Err(ApiError::bad("grup kendi altına taşınamaz"));
        }
        sqlx::query("UPDATE groups SET parent_id = ? WHERE id = ?")
            .bind(Some(new_parent.clone()))
            .bind(&id)
            .execute(&state.db)
            .await?;
    }

    audit(&state.db, Some(user.uid()), "group.update", Some(&id), None, None,
        Some(serde_json::json!({ "name": req.name, "parent_id": req.parent_id }))).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/groups/{id}[?cascade=true]
pub async fn delete_group(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(opts): Query<DeleteOpts>,
) -> ApiResult<Json<serde_json::Value>> {
    require(&user, Scope::Write)?;
    let g = fetch_group(&state.db, &id).await?;

    let counts: (i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM groups WHERE parent_id = ?) AS children,
            (SELECT COUNT(*) FROM entries WHERE group_id = ?) AS entries",
    )
    .bind(&id)
    .bind(&id)
    .fetch_one(&state.db)
    .await?;

    if !opts.cascade && (counts.0 > 0 || counts.1 > 0) {
        return Err(ApiError::conflict(format!(
            "grupta {} alt grup, {} entry var; ?cascade=true ile sil",
            counts.0, counts.1
        )));
    }

    // alt ağaçki gruplarda entries
    let ids = subtree_group_ids(&state.db, &id).await?;
    for gid in &ids {
        let entries: Vec<sqlx::sqlite::SqliteRow> = sqlx::query("SELECT id FROM entries WHERE group_id = ?")
            .bind(gid)
            .fetch_all(&state.db)
            .await?;
        for e in entries {
            let eid: String = e.get("id");
            crate::storage::remove_entry_blobs(&state, &eid).await;
        }
        sqlx::query("DELETE FROM entries WHERE group_id = ?").bind(gid).execute(&state.db).await?;
    }
    for gid in &ids {
        sqlx::query("DELETE FROM groups WHERE id = ?").bind(gid).execute(&state.db).await?;
    }

    audit(&state.db, Some(user.uid()), "group.delete", Some(&id), None, None,
        Some(serde_json::json!({ "cluster": g.cluster_id, "groups": ids.len() }))).await;
    Ok(Json(serde_json::json!({ "ok": true, "groups_removed": ids.len() })))
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/groups/{id}", axum::routing::patch(update_group).delete(delete_group))
        .route("/api/clusters/{id}/groups", post(create_group).get(super::clusters::groups_tree))
}
