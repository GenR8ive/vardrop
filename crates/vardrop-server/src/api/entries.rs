//! Entry CRUD + versiyonlar + restore.

use crate::api::values::CreateEntryReq;
use crate::auth::middleware::{require, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::util::{audit, group_paths_of, parse_tags, subtree_group_ids};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use vardrop_core::{now_ms, EntryDetail, EntrySummary, FileInfo, Scope, VersionInfo};
use serde::Deserialize;
use sqlx::Row;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/entries", get(list).post(create))
        .route("/api/entries/{id}", axum::routing::patch(patch).delete(del).get(get_one))
        .route("/api/entries/{id}/versions", get(list_versions))
        .route("/api/entries/{id}/versions/{version}/restore", post(restore))
}

// ---------- satır yapıları ----------

#[allow(dead_code)] // bazı kolonlar SELECT eşlemesinde taşınır, okuyucu sonraki fazlarda
#[derive(sqlx::FromRow, Clone)]
pub(crate) struct EntryRow {
    pub id: String,
    pub cluster_id: String,
    pub group_id: String,
    pub name: String,
    pub kind: String,
    pub is_secret: bool,
    pub description: Option<String>,
    pub tags: String,
    pub current_version: i64,
    pub created_by: Option<String>,
    pub created_at: i64,
    pub updated_by: Option<String>,
    pub updated_at: i64,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Clone)]
pub(crate) struct VersionRow {
    pub id: i64,
    pub entry_id: String,
    pub version: i64,
    pub value_text: Option<String>,
    pub value_enc: Option<Vec<u8>>,
    pub blob_path: Option<String>,
    pub value_hash: Option<String>,
    pub size: Option<i64>,
    pub created_by: Option<String>,
    pub created_at: i64,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Clone)]
pub(crate) struct FileRow {
    pub id: String,
    pub entry_id: String,
    pub version: i64,
    pub file_path: String,
    pub blob_path: Option<String>,
    pub size: i64,
    pub sha256: String,
    pub is_secret: bool,
}

pub(crate) async fn fetch_entry(db: &sqlx::SqlitePool, id: &str) -> ApiResult<EntryRow> {
    sqlx::query_as::<_, EntryRow>(
        "SELECT id, cluster_id, group_id, name, kind, is_secret, description, tags, current_version,
                created_by, created_at, updated_by, updated_at
         FROM entries WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| ApiError::not_found("entry bulunamadı"))
}

pub(crate) async fn fetch_current_version(db: &sqlx::SqlitePool, entry: &EntryRow) -> ApiResult<VersionRow> {
    sqlx::query_as::<_, VersionRow>(
        "SELECT id, entry_id, version, value_text, value_enc, blob_path, value_hash, size, created_by, created_at
         FROM entry_versions WHERE entry_id = ? AND version = ?",
    )
    .bind(&entry.id)
    .bind(entry.current_version)
    .fetch_one(db)
    .await
    .map_err(|_| ApiError::internal("mevcut versiyon kaydı eksik"))
}

// ---------- detay ve önizleme ----------

pub(crate) async fn detail_of(
    _user: &AuthUser,
    state: &SharedState,
    id: &str,
) -> ApiResult<EntryDetail> {
    let entry = fetch_entry(&state.db, id).await?;
    let files = bundle_files_of(&state.db, &entry.id, entry.current_version).await?;
    let paths = group_paths_of(&state.db, &[entry.cluster_id.clone()]).await?;
    let path = paths.get(&entry.group_id).cloned().unwrap_or_default();

    Ok(EntryDetail {
        id: entry.id.clone(),
        cluster_id: entry.cluster_id.clone(),
        group_id: entry.group_id.clone(),
        path,
        name: entry.name.clone(),
        kind: entry.kind.clone(),
        is_secret: entry.is_secret,
        tags: parse_tags(&entry.tags),
        description: entry.description.clone(),
        current_version: entry.current_version,
        created_by: entry.created_by.clone(),
        created_at: entry.created_at,
        updated_at: entry.updated_at,
        files: files
            .into_iter()
            .map(|f| FileInfo {
                id: f.id,
                file_path: f.file_path,
                size: f.size,
                sha256: f.sha256,
                is_secret: f.is_secret,
            })
            .collect(),
    })
}

pub(crate) async fn bundle_files_of(
    db: &sqlx::SqlitePool,
    entry_id: &str,
    version: i64,
) -> ApiResult<Vec<FileRow>> {
    sqlx::query_as::<_, FileRow>(
        "SELECT id, entry_id, version, file_path, blob_path, size, sha256, is_secret
         FROM bundle_files WHERE entry_id = ? AND version = ? ORDER BY file_path",
    )
    .bind(entry_id)
    .bind(version)
    .fetch_all(db)
    .await
    .map_err(ApiError::from)
}

/// GET /api/entries/{id}
pub async fn get_one(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<EntryDetail>> {
    require(&user, Scope::Read)?;
    Ok(Json(detail_of(&user, &state, &id).await?))
}

// ---------- liste / arama ----------

#[derive(Deserialize, Default)]
pub struct ListParams {
    pub q: Option<String>,
    pub cluster_id: Option<String>,
    pub group_id: Option<String>,
    pub kind: Option<String>,
    pub secret: Option<bool>,
    pub tag: Option<String>,
    pub limit: Option<i64>,
}

pub(crate) enum SqlBind {
    S(String),
    B(bool),
}

/// WHERE cümlesi + bind listesi üretir (liste ve sayaç paylaşımlı).
pub(crate) fn build_where(p: &ListParams) -> ApiResult<(String, Vec<SqlBind>)> {
    // Not: SQL metni statik; tüm kullanıcı girdileri bind edilir.
    let mut sql = String::from(" WHERE 1=1");
    let mut binds: Vec<SqlBind> = Vec::new();

    if let Some(q) = p.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // isim + açıklama + etiket; grup/kluster adı eşleşmesi list() içinde
        // alt ağaç genişlemesiyle __QGROUP__ içine bağlanır (üst grup → alt itemlar dahil).
        sql.push_str(" AND (e.name LIKE ? OR e.description LIKE ? OR e.tags LIKE ? OR __QGROUP__)");
        let pat = format!("%{q}%");
        binds.push(SqlBind::S(pat.clone()));
        binds.push(SqlBind::S(pat.clone()));
        binds.push(SqlBind::S(pat));
    }
    if let Some(cid) = &p.cluster_id {
        sql.push_str(" AND e.cluster_id = ?");
        binds.push(SqlBind::S(cid.clone()));
    }
    if p.group_id.is_some() {
        // alt ağaç listesi çalışma zamanında sorgulanır; SQL yerini caller doldurur
        sql.push_str("__GROUP__");
    }
    if let Some(k) = &p.kind {
        if vardrop_core::ContentKind::parse(k).is_none() {
            return Err(ApiError::bad("geçersiz kind"));
        }
        sql.push_str(" AND e.kind = ?");
        binds.push(SqlBind::S(k.clone()));
    }
    if let Some(s) = p.secret {
        sql.push_str(" AND e.is_secret = ?");
        binds.push(SqlBind::B(s));
    }
    if let Some(t) = p.tag.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" AND e.tags LIKE ?");
        binds.push(SqlBind::S(format!("%\"{t}\"%")));
    }
    Ok((sql, binds))
}

async fn load_subtree(data_state: &SharedState, gid: &str) -> ApiResult<Vec<String>> {
    subtree_group_ids(&data_state.db, gid).await
}

/// GET /api/entries — arama (client popup bunu kullanır)
pub async fn list(
    user: AuthUser,
    State(state): State<SharedState>,
    Query(p): Query<ListParams>,
) -> ApiResult<Json<vardrop_core::SearchResponse>> {
    let started = std::time::Instant::now();
    require(&user, Scope::Read)?;

    // group_id filtresi alt ağaçla genişler: __GROUP__ yerine IN (...) eklenir
    let mut group_ids: Option<Vec<String>> = None;
    let (mut where_sql, mut binds) = build_where(&p)?;

    // __QGROUP__ genişlemesi: q grup/kluster adına eşliyorsan alt ağaçlar da dahil
    if where_sql.contains("__QGROUP__") {
        let q = p.q.as_deref().unwrap_or_default();
        let pat = format!("%{q}%");

        let cluster_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM clusters WHERE name LIKE ?")
                .bind(&pat)
                .fetch_all(&state.db)
                .await?
                .into_iter()
                .collect();

        let seeds: Vec<String> =
            sqlx::query_scalar("SELECT id FROM groups WHERE name LIKE ?")
                .bind(&pat)
                .fetch_all(&state.db)
                .await?
                .into_iter()
                .collect();
        let mut group_ids_q: std::collections::BTreeSet<String> = Default::default();
        for s in seeds {
            for id in load_subtree(&state, &s).await? {
                group_ids_q.insert(id);
            }
        }

        let mut parts: Vec<String> = Vec::new();
        let mut extra_binds: Vec<String> = Vec::new();
        if !cluster_ids.is_empty() {
            parts.push(format!(
                "e.cluster_id IN ({})",
                vec!["?"; cluster_ids.len()].join(",")
            ));
            extra_binds.extend(cluster_ids);
        }
        if !group_ids_q.is_empty() {
            let ids: Vec<String> = group_ids_q.into_iter().collect();
            parts.push(format!(
                "e.group_id IN ({})",
                vec!["?"; ids.len()].join(",")
            ));
            extra_binds.extend(ids);
        }
        let clause = if parts.is_empty() {
            "0".to_string()
        } else {
            parts.join(" OR ")
        };
        let idx = where_sql.find("__QGROUP__").unwrap();
        where_sql.replace_range(idx..idx + "__QGROUP__".len(), &clause);
        // bind sırası: q'nun 3 LIKE bind'inden hemen sonra
        for (i, id) in extra_binds.into_iter().enumerate() {
            binds.insert(3 + i, SqlBind::S(id));
        }
    }

    if where_sql.contains("__GROUP__") {
        let Some(gid) = &p.group_id else {
            return Err(ApiError::internal("group parametresi kayboldu"));
        };
        let ids = load_subtree(&state, gid).await?;
        let clause = if ids.is_empty() {
            " AND 0".to_string()
        } else {
            format!(
                " AND e.group_id IN ({})",
                vec!["?"; ids.len()].join(",")
            )
        };
        let idx = where_sql.find("__GROUP__").unwrap();
        where_sql.replace_range(idx..idx + "__GROUP__".len(), &clause);
        // bu bind grubun sonunda değil — placeholders sırayla; group ids'i id listesi olarak bind'liyoruz
        group_ids = Some(ids);
    }

    let limit = p.limit.unwrap_or(50).clamp(1, 200);

    // toplam
    const FROM_JOINED: &str = " FROM entries e JOIN groups g ON g.id = e.group_id JOIN clusters c ON c.id = e.cluster_id";
    let count_sql = format!("SELECT COUNT(*) AS c{FROM_JOINED}{where_sql}");
    let mut cq = sqlx::query(&count_sql);
    for b in &binds {
        cq = match b {
            SqlBind::S(s) => cq.bind(s.clone()),
            SqlBind::B(v) => cq.bind(*v),
        };
    }
    if let Some(ids) = &group_ids {
        for id in ids {
            cq = cq.bind(id.clone());
        }
    }
    let total: i64 = cq.fetch_one(&state.db).await?.get(0);

    // satırlar
    let select_sql = format!("SELECT e.id, e.cluster_id, e.group_id, e.name, e.kind, e.is_secret, e.description, e.tags, e.current_version, e.created_by, e.created_at, e.updated_by, e.updated_at{FROM_JOINED}{where_sql} ORDER BY e.updated_at DESC LIMIT ?");
    let mut q = sqlx::query_as::<_, EntryRow>(&select_sql);
    for b in &binds {
        q = match b {
            SqlBind::S(s) => q.bind(s.clone()),
            SqlBind::B(v) => q.bind(*v),
        };
    }
    if let Some(ids) = &group_ids {
        for id in ids {
            q = q.bind(id.clone());
        }
    }
    q = q.bind(limit);
    let rows = q.fetch_all(&state.db).await?;

    // cluster başına grup yolları
    let cluster_ids: Vec<String> = rows
        .iter()
        .map(|r| r.cluster_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let paths = group_paths_of(&state.db, &cluster_ids).await?;

    // her entry için güncel versiyon verisi (önizleme)
    let mut preview_map: std::collections::HashMap<String, VersionRow> = Default::default();
    for chunk in rows.chunks(90) {
        let ids: Vec<String> = chunk.iter().map(|r| r.id.clone()).collect();
        let in_clause = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT id, entry_id, version, value_text, value_enc, blob_path, value_hash, size, created_by, created_at
             FROM entry_versions WHERE entry_id IN ({in_clause})"
        );
        let mut q = sqlx::query_as::<_, VersionRow>(&sql);
        for id in &ids {
            q = q.bind(id.clone());
        }
        for v in q.fetch_all(&state.db).await? {
            preview_map.insert(v.entry_id.clone(), v);
        }
    }

    let admin = matches!(vardrop_core::Role::parse(&user.user.role), Some(vardrop_core::Role::Admin));
    let mut results = Vec::with_capacity(rows.len());
    for r in rows {
        let vrow = preview_map.get(&r.id);
        let file_count = if r.kind == "bundle" {
            Some(bundle_files_of(&state.db, &r.id, r.current_version).await?.len() as i64)
        } else {
            None
        };
        let preview = crate::api::values::compute_preview(&state, &r.kind, r.is_secret, admin, vrow, file_count).await;
        results.push(EntrySummary {
            id: r.id.clone(),
            cluster_id: r.cluster_id.clone(),
            group_id: r.group_id.clone(),
            path: paths.get(&r.group_id).cloned().unwrap_or_default(),
            name: r.name.clone(),
            kind: r.kind.clone(),
            is_secret: r.is_secret,
            tags: parse_tags(&r.tags),
            description: r.description.clone(),
            current_version: r.current_version,
            updated_at: r.updated_at,
            preview,
        });
    }
    Ok(Json(vardrop_core::SearchResponse {
        results,
        total,
        took_ms: started.elapsed().as_millis() as u64,
    }))
}

/// Query'e tip-uyumlu bind ekler (inline match kullanıldı; bu fn kalktı).

// ---------- oluştur / güncelle / sil ----------

/// POST /api/entries
pub async fn create(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateEntryReq>,
) -> ApiResult<(axum::http::StatusCode, Json<EntryDetail>)> {
    crate::api::values::create_entry(&state, &user, req).await
}

#[derive(Deserialize)]
pub struct PatchEntryReq {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub is_secret: Option<bool>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

/// PATCH /api/entries/{id}
pub async fn patch(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<PatchEntryReq>,
) -> ApiResult<Json<EntryDetail>> {
    require(&user, Scope::Write)?;
    let entry = fetch_entry(&state.db, &id).await?;

    if let Some(n) = req.name.as_deref() {
        let n = n.trim();
        if n.is_empty() {
            return Err(ApiError::bad("entry adı boş olamaz"));
        }
        let dup: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM entries WHERE cluster_id = ? AND group_id = ? AND name = ? AND id != ?",
        )
        .bind(&entry.cluster_id)
        .bind(&entry.group_id)
        .bind(n)
        .bind(&id)
        .fetch_optional(&state.db)
        .await?;
        if dup.is_some() {
            return Err(ApiError::conflict("aynı grupta aynı adlı entry var"));
        }
    }
    let mut kind_s = entry.kind.clone();
    if let Some(k) = req.kind.as_deref() {
        let ck = vardrop_core::ContentKind::parse(k).ok_or_else(|| ApiError::bad("geçersiz kind"))?;
        let old = vardrop_core::ContentKind::parse(&entry.kind).unwrap_or(vardrop_core::ContentKind::Text);
        let files_exist: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM bundle_files WHERE entry_id = ?").bind(&entry.id).fetch_one(&state.db).await?;
        if (old.is_file_based() || ck.is_file_based()) && (old != ck || files_exist.0 > 0) && old != ck {
            return Err(ApiError::bad("dosya tabanlı türler birbirine dönüştürülemez"));
        }
        kind_s = k.to_string();
    }
    if let Some(gid) = &req.group_id {
        let g = crate::api::groups::fetch_group(&state.db, gid).await?;
        if g.cluster_id != entry.cluster_id {
            return Err(ApiError::bad("hedef grup başka cluster'da"));
        }
    }
    if let Some(v) = &req.value {
        let k = vardrop_core::ContentKind::parse(&kind_s).unwrap_or(vardrop_core::ContentKind::Text);
        if k.is_file_based() {
            return Err(ApiError::bad("dosya tabanlı entry'de value alanı yok (dosya yükleyin)"));
        }
        if k == vardrop_core::ContentKind::Json {
            vardrop_core::content::validate_json(v).map_err(ApiError::bad)?;
        }
        let meta = crate::api::values::EntryStoreMeta {
            id: entry.id.clone(),
            name: req.name.clone().unwrap_or_else(|| entry.name.clone()),
            kind: k,
            is_secret: req.is_secret.unwrap_or(entry.is_secret),
        };
        crate::api::values::store_version(&state, &meta, entry.current_version + 1,
            &crate::api::values::StoredContent::Text(v.clone()), Some(&user)).await?;
    }

    sqlx::query(
        "UPDATE entries SET
            name = COALESCE(?, name),
            group_id = COALESCE(?, group_id),
            kind = COALESCE(?, kind),
            is_secret = COALESCE(?, is_secret),
            description = COALESCE(?, description),
            tags = COALESCE(?, tags),
            updated_by = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(req.name.clone())
    .bind(req.group_id.clone())
    .bind(req.kind.clone())
    .bind(req.is_secret)
    .bind(req.description.clone())
    .bind(req.tags.as_ref().map(|t| serde_json::json!(t).to_string()))
    .bind(user.user.id.clone())
    .bind(now_ms())
    .bind(&id)
    .execute(&state.db)
    .await?;

    audit(&state.db, Some(user.uid()), "entry.update", Some(&id), None, None,
        Some(serde_json::json!({ "name": req.name, "kind": req.kind, "secret": req.is_secret, "value": req.value.is_some() }))).await;

    Ok(Json(detail_of(&user, &state, &id).await?))
}

/// DELETE /api/entries/{id}
pub async fn del(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require(&user, Scope::Write)?;
    let entry = fetch_entry(&state.db, &id).await?;
    crate::storage::remove_entry_blobs(&state, &entry.id).await;
    sqlx::query("DELETE FROM entries WHERE id = ?").bind(&entry.id).execute(&state.db).await?;
    audit(&state.db, Some(user.uid()), "entry.delete", Some(&entry.id), None, None,
        Some(serde_json::json!({ "name": entry.name }))).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/entries/{id}/versions
pub async fn list_versions(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<VersionInfo>>> {
    require(&user, Scope::Read)?;
    fetch_entry(&state.db, &id).await?;
    let rows = sqlx::query(
        "SELECT v.version,
                v.size, v.value_hash, v.created_by, v.created_at,
                (SELECT COUNT(*) FROM bundle_files f WHERE f.entry_id = v.entry_id AND f.version = v.version) AS file_count
         FROM entry_versions v WHERE v.entry_id = ? ORDER BY v.version DESC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows
        .into_iter()
        .map(|r: sqlx::sqlite::SqliteRow| VersionInfo {
            version: r.get("version"),
            size: r.get("size"),
            value_hash: r.get("value_hash"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            file_count: r.get("file_count"),
        })
        .collect()))
}

/// POST /api/entries/{id}/versions/{version}/restore
pub async fn restore(
    user: AuthUser,
    State(state): State<SharedState>,
    Path((id, version)): Path<(String, i64)>,
) -> ApiResult<Json<EntryDetail>> {
    require(&user, Scope::Write)?;
    let entry = fetch_entry(&state.db, &id).await?;
    if version == entry.current_version {
        return Err(ApiError::bad("bu versiyon zaten güncel"));
    }
    let old: Option<VersionRow> =
        sqlx::query_as("SELECT id, entry_id, version, value_text, value_enc, blob_path, value_hash, size, created_by, created_at FROM entry_versions WHERE entry_id = ? AND version = ?")
            .bind(&id)
            .bind(version)
            .fetch_optional(&state.db)
            .await?;
    let Some(_old) = old else {
        return Err(ApiError::not_found("versiyon bulunamadı"));
    };
    let meta = crate::api::values::EntryStoreMeta {
        id: entry.id.clone(),
        name: entry.name.clone(),
        kind: vardrop_core::ContentKind::parse(&entry.kind).unwrap_or(vardrop_core::ContentKind::Text),
        is_secret: entry.is_secret,
    };
    crate::api::values::restore_version(&state, &meta, entry.current_version, version, Some(&user)).await?;
    audit(&state.db, Some(user.uid()), "entry.restore", Some(&id), None, None,
        Some(serde_json::json!({ "from": version }))).await;
    Ok(Json(detail_of(&user, &state, &id).await?))
}
