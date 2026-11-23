//! İçerik (value/dosya) depolama, servis etme, yüklerme; create_entry + versiyon mantığı.

use crate::api::entries::{bundle_files_of, detail_of, fetch_current_version, fetch_entry, FileRow, VersionRow};
use crate::auth::middleware::{require, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::{crypto, storage, util::audit};
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::Router;
use vardrop_core::{new_id, now_ms, FileInfo, Scope};
use serde::Deserialize;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/entries/{id}/value", axum::routing::get(get_value))
        .route("/api/entries/{id}/files", axum::routing::post(upload_file))
        .route(
            "/api/entries/{id}/files/{fid}",
            axum::routing::get(get_file).delete(remove_file),
        )
}

/// Metin tabanlı inline eşiği (1 MB).
const INLINE_TEXT_MAX: usize = 1_000_000;

#[derive(Debug)]
pub enum StoredContent {
    Text(String),
    Bytes(Vec<u8>),
}

impl From<String> for StoredContent {
    fn from(value: String) -> Self {
        StoredContent::Text(value)
    }
}

impl From<Vec<u8>> for StoredContent {
    fn from(value: Vec<u8>) -> Self {
        StoredContent::Bytes(value)
    }
}

pub enum LoadedContent {
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(serde::Serialize)]
struct ValueText {
    value: String,
}

#[derive(serde::Serialize)]
struct Manifest {
    version: i64,
    files: Vec<FileInfo>,
}

#[allow(dead_code)]
pub(crate) struct EntryStoreMeta {
    pub id: String,
    pub name: String,
    pub kind: vardrop_core::ContentKind,
    pub is_secret: bool,
}

// ---------- içerik yazar / okur ----------

pub(crate) async fn store_version(
    state: &SharedState,
    meta: &EntryStoreMeta,
    version: i64,
    content: &StoredContent,
    actor: Option<&AuthUser>,
) -> ApiResult<()> {
    let bytes: Vec<u8> = match content {
        StoredContent::Text(s) => s.as_bytes().to_vec(),
        StoredContent::Bytes(b) => b.clone(),
    };
    let hash = vardrop_core::content::sha256_hex(&bytes);
    let size = bytes.len() as i64;

    let mut value_text: Option<String> = None;
    let mut value_enc: Option<Vec<u8>> = None;
    let mut blob_path: Option<String> = None;

    if !meta.is_secret && matches!(content, StoredContent::Text(_)) && bytes.len() <= INLINE_TEXT_MAX {
        value_text = match content {
            StoredContent::Text(s) => Some(s.clone()),
            _ => None,
        };
    } else if meta.is_secret {
        let max_inline = state.cfg.security.secret_max_inline_kb as usize * 1024;
        if bytes.len() <= max_inline {
            value_enc = Some(crypto::encrypt(&state.key, &bytes)?);
        } else {
            blob_path = Some(storage::write_blob(state, &meta.id, &bytes, true).await?.0);
        }
    } else {
        blob_path = Some(storage::write_blob(state, &meta.id, &bytes, false).await?.0);
    }

    sqlx::query(
        "INSERT INTO entry_versions (entry_id, version, value_text, value_enc, blob_path, value_hash, size, created_by, created_at)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(&meta.id)
    .bind(version)
    .bind(value_text)
    .bind(value_enc)
    .bind(blob_path)
    .bind(hash)
    .bind(size)
    .bind(actor.map(|a| a.user.id.clone()))
    .bind(now_ms())
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE entries SET current_version = ?, updated_by = ?, updated_at = ? WHERE id = ?")
        .bind(version)
        .bind(actor.map(|a| a.user.id.clone()))
        .bind(now_ms())
        .bind(&meta.id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub(crate) async fn load_version(state: &SharedState, vr: &VersionRow, is_secret: bool) -> ApiResult<LoadedContent> {
    if let Some(t) = &vr.value_text {
        return Ok(LoadedContent::Text(t.clone()));
    }
    if let Some(enc) = &vr.value_enc {
        let plain = crypto::decrypt(&state.key, enc)?;
        if let Ok(s) = String::from_utf8(plain.clone()) {
            return Ok(LoadedContent::Text(s));
        }
        return Ok(LoadedContent::Bytes(plain));
    }
    if let Some(rel) = &vr.blob_path {
        let bytes = storage::read_blob(state, rel, is_secret).await?;
        if let Ok(s) = String::from_utf8(bytes.clone()) {
            return Ok(LoadedContent::Text(s));
        }
        return Ok(LoadedContent::Bytes(bytes));
    }
    Err(ApiError::internal("versiyon içeriği yok"))
}

/// Panellerin ve client'ın liste satırlarında gösterdiği önizleme.
pub(crate) async fn compute_preview(
    state: &SharedState,
    kind: &str,
    is_secret: bool,
    admin: bool,
    vr: Option<&VersionRow>,
    file_count: Option<i64>,
    is_image: bool,
) -> String {
    use vardrop_core::ContentKind as CK;
    let k = CK::parse(kind).unwrap_or(CK::Text);

    if is_secret {
        // secret: maske; yalnız admin'in inline değerde son 4 karakteri görebilir
        if admin {
            if let Some(v) = vr {
                if let Some(enc) = &v.value_enc {
                    if let Ok(LoadedContent::Text(t)) = load_version(state, v, is_secret).await {
                        let tail: String = t.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                        if !tail.is_empty() {
                            return vardrop_core::content::mask_preview(Some(&tail));
                        }
                    }
                    let _ = enc;
                }
            }
        }
        return vardrop_core::content::mask_preview(None);
    }

    match k {
        CK::Text | CK::Json | CK::Yaml => {
            if let Some(v) = vr {
                if let Some(t) = &v.value_text {
                    return vardrop_core::content::text_preview(t, 120);
                }
                if let Some(rel) = &v.blob_path {
                    if let Ok(bytes) = storage::read_blob_prefix(state, rel, false, 8192).await {
                        return vardrop_core::content::text_preview(&String::from_utf8_lossy(&bytes), 120);
                    }
                }
            }
            "—".into()
        }
        CK::Binary => {
            let size = vr.and_then(|v| v.size).unwrap_or(0);
            if is_image {
                format!("[görsel · {}]", vardrop_core::content::human_size(size))
            } else {
                format!("[dosya · {}]", vardrop_core::content::human_size(size))
            }
        }
        CK::Bundle => format!(
            "[paket · {} dosya]",
            file_count.unwrap_or(0)
        ),
    }
}

// ---------- create_entry (entries::create delegasyonu) ----------

#[derive(Deserialize)]
pub struct CreateEntryReq {
    pub cluster_id: String,
    pub group_id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub is_secret: Option<bool>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

pub(crate) async fn create_entry(
    state: &SharedState,
    user: &AuthUser,
    req: CreateEntryReq,
) -> ApiResult<(axum::http::StatusCode, axum::Json<vardrop_core::EntryDetail>)> {
    require(user, Scope::Write)?;
    let kind = vardrop_core::ContentKind::parse(&req.kind)
        .ok_or_else(|| ApiError::bad("geçersiz kind (text|json|yaml|binary|bundle)"))?;
    let name = req.name.trim().to_string();
    if name.is_empty() || name.contains('/') {
        return Err(ApiError::bad("geçersiz entry adı"));
    }

    let cluster: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM clusters WHERE id = ?")
        .bind(&req.cluster_id)
        .fetch_optional(&state.db)
        .await?;
    if cluster.is_none() {
        return Err(ApiError::bad("cluster bulunamadı"));
    }
    let group = crate::api::groups::fetch_group(&state.db, &req.group_id).await?;
    if group.cluster_id != req.cluster_id {
        return Err(ApiError::bad("grup, cluster'a ait değil"));
    }
    let dup: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM entries WHERE cluster_id = ? AND group_id = ? AND name = ?",
    )
    .bind(&req.cluster_id)
    .bind(&req.group_id)
    .bind(&name)
    .fetch_optional(&state.db)
    .await?;
    if dup.is_some() {
        return Err(ApiError::conflict("aynı grupta aynı adlı entry var"));
    }

    let is_secret = req.is_secret.unwrap_or(false);
    // değerin ilk sürümü (yalnız value-based türlerde)
    let content = match kind {
        vardrop_core::ContentKind::Binary | vardrop_core::ContentKind::Bundle => None,
        _ => Some(match &req.value {
            Some(v) if kind == vardrop_core::ContentKind::Json => {
                vardrop_core::content::validate_json(v).map_err(ApiError::bad)?;
                StoredContent::Text(v.clone())
            }
            Some(v) => StoredContent::Text(v.clone()),
            None => StoredContent::Text(String::new()),
        }),
    };

    let eid = new_id("e");
    sqlx::query(
        "INSERT INTO entries (id, cluster_id, group_id, name, kind, is_secret, description, tags, current_version, created_by, created_at, updated_by, updated_at)
         VALUES (?,?,?,?,?,?,?,COALESCE(?, '[]'), 1, ?, ?, ?, ?)",
    )
    .bind(&eid)
    .bind(&req.cluster_id)
    .bind(&req.group_id)
    .bind(&name)
    .bind(kind.as_str())
    .bind(is_secret)
    .bind(req.description.clone())
    .bind(req.tags.as_ref().map(|t| serde_json::json!(t).to_string()))
    .bind(user.user.id.clone())
    .bind(now_ms())
    .bind(user.user.id.clone())
    .bind(now_ms())
    .execute(&state.db)
    .await?;

    if let Some(c) = &content {
        let meta = EntryStoreMeta {
            id: eid.clone(),
            name: name.clone(),
            kind,
            is_secret,
        };
        store_version(state, &meta, 1, c, Some(user)).await?;
    }

    audit(&state.db, Some(user.uid()), "entry.create", Some(&eid), None, None,
        Some(serde_json::json!({ "name": name, "kind": kind.as_str(), "secret": is_secret }))).await;

    Ok((axum::http::StatusCode::CREATED, Json(detail_of(user, state, &eid).await?)))
}

// ---------- versiyon restore (bundle dahil) ----------

pub(crate) async fn restore_version(
    state: &SharedState,
    meta: &EntryStoreMeta,
    current: i64,
    from_version: i64,
    actor: Option<&AuthUser>,
) -> ApiResult<()> {
    let old: VersionRow = sqlx::query_as(
        "SELECT id, entry_id, version, value_text, value_enc, blob_path, value_hash, size, created_by, created_at
         FROM entry_versions WHERE entry_id = ? AND version = ?",
    )
    .bind(&meta.id)
    .bind(from_version)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("versiyon bulunamadı"))?;

    let new_version = current + 1;
    sqlx::query(
        "INSERT INTO entry_versions (entry_id, version, value_text, value_enc, blob_path, value_hash, size, created_by, created_at)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(&meta.id)
    .bind(new_version)
    .bind(old.value_text.clone())
    .bind(old.value_enc.clone())
    .bind(old.blob_path.clone())
    .bind(old.value_hash.clone())
    .bind(old.size)
    .bind(actor.map(|a| a.user.id.clone()))
    .bind(now_ms())
    .execute(&state.db)
    .await?;

    if meta.kind == vardrop_core::ContentKind::Bundle {
        let rows: Vec<FileRow> = bundle_files_of(&state.db, &meta.id, from_version).await?;
        for f in rows {
            sqlx::query(
                "INSERT OR IGNORE INTO bundle_files (id, entry_id, version, file_path, blob_path, size, sha256, is_secret)
                 VALUES (?,?,?,?,?,?,?,?)",
            )
            .bind(f.id)
            .bind(&meta.id)
            .bind(new_version)
            .bind(&f.file_path)
            .bind(&f.blob_path)
            .bind(f.size)
            .bind(&f.sha256)
            .bind(f.is_secret)
            .execute(&state.db)
            .await?;
        }
    }

    sqlx::query("UPDATE entries SET current_version = ?, updated_by = ?, updated_at = ? WHERE id = ?")
        .bind(new_version)
        .bind(actor.map(|a| a.user.id.clone()))
        .bind(now_ms())
        .bind(&meta.id)
        .execute(&state.db)
        .await?;
    Ok(())
}

// ---------- value / dosya endpoint'leri ----------

#[derive(Deserialize, Default)]
pub struct ValueQuery {
    pub file_id: Option<String>,
    pub download: Option<bool>,
}

fn bytes_response(bytes: Vec<u8>, filename: &str, download: bool) -> ApiResult<Response> {
    let mut b = Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream");
    if download {
        b = b.header("content-disposition", format!("attachment; filename=\"{filename}\""));
    }
    b.body(Body::from(bytes))
        .map_err(|e| ApiError::internal(format!("yanıt üretilemedi: {e}")))
}

/// GET /api/entries/{id}/value
pub async fn get_value(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<ValueQuery>,
) -> ApiResult<Response> {
    require(&user, Scope::Read)?;
    let entry = fetch_entry(&state.db, &id).await?;
    let kind = vardrop_core::ContentKind::parse(&entry.kind).unwrap_or(vardrop_core::ContentKind::Text);

    match kind {
        vardrop_core::ContentKind::Bundle => {
            match q.file_id {
                None => {
                    let files = bundle_files_of(&state.db, &entry.id, entry.current_version).await?;
                    Ok((
                        axum::http::StatusCode::OK,
                        Json(Manifest {
                            version: entry.current_version,
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
                        }),
                    )
                        .into_response())
                }
                Some(fid) => {
                    let f = fetch_file(&state.db, &entry.id, &fid).await?;
                    if f.is_secret {
                        crate::auth::require_secret_access(&state, &user).await?;
                    }
                    let bytes = storage::read_blob(&state, &f.blob_path.unwrap_or_default(), f.is_secret).await?;
                    audit(&state.db, Some(user.uid()),
                        if f.is_secret { "secret.reveal" } else { "file.read" },
                        Some(&format!("{}/{}", entry.name, f.file_path)), None, None, None).await;
                    bytes_response(bytes, &f.file_path, true)
                }
            }
        }
        vardrop_core::ContentKind::Binary => {
            let vrow = fetch_current_version(&state.db, &entry).await?;
            if entry.is_secret {
                crate::auth::require_secret_access(&state, &user).await?;
            }
            let rel = vrow.blob_path.ok_or_else(|| ApiError::internal("binary içerik yok"))?;
            let bytes = storage::read_blob(&state, &rel, entry.is_secret).await?;
            audit(&state.db, Some(user.uid()), "value.read", Some(&entry.name), None, None, None).await;
            bytes_response(bytes, &entry.name, q.download.unwrap_or(false))
        }
        _ => {
            let vrow = fetch_current_version(&state.db, &entry).await?;
            if entry.is_secret {
                crate::auth::require_secret_access(&state, &user).await?;
            }
            let loaded = load_version(&state, &vrow, entry.is_secret).await?;
            audit(&state.db, Some(user.uid()),
                if entry.is_secret { "secret.reveal" } else { "value.read" },
                Some(&entry.name), None, None, None).await;
            let s = match loaded {
                LoadedContent::Text(t) => t,
                LoadedContent::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
            };
            Ok((axum::http::StatusCode::OK, Json(ValueText { value: s })).into_response())
        }
    }
}

async fn fetch_file(db: &sqlx::SqlitePool, entry_id: &str, fid: &str) -> ApiResult<FileRow> {
    sqlx::query_as::<_, FileRow>(
        "SELECT id, entry_id, version, file_path, blob_path, size, sha256, is_secret
         FROM bundle_files WHERE entry_id = ? AND id = ?",
    )
    .bind(entry_id)
    .bind(fid)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| ApiError::not_found("dosya bulunamadı"))
}

fn sanitize_rel(name: &str) -> ApiResult<String> {
    let n = name.replace('\\', "/");
    let n = n.trim().trim_start_matches('/');
    if n.is_empty() {
        return Err(ApiError::bad("dosya adı boş"));
    }
    if n.split('/').any(|p| p == "..") || n.contains('\0') || n.len() > 255 {
        return Err(ApiError::bad("geçersiz dosya yolu"));
    }
    Ok(n.to_string())
}

fn sanitize_single(name: &str) -> ApiResult<String> {
    let s = sanitize_rel(name)?;
    Ok(s.split('/').last().unwrap_or("").to_string())
}

/// POST /api/entries/{id}/files (multipart: file[] + optional path[] önce)
pub async fn upload_file(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<axum::response::Json<vardrop_core::EntryDetail>> {
    require(&user, Scope::Write)?;
    let entry = fetch_entry(&state.db, &id).await?;
    let kind = vardrop_core::ContentKind::parse(&entry.kind).unwrap_or(vardrop_core::ContentKind::Text);
    if !kind.is_file_based() {
        return Err(ApiError::bad("dosya yüklemesi yalnız binary/bundle entry'lerde geçerli"));
    }

    let mut pending_path: Option<String> = None;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad(format!("multipart hatası: {e}")))?
    {
        if field.name() == Some("path") {
            pending_path = field.text().await.ok();
            continue;
        }
        if field.name() == Some("file") {
            let rel = pending_path
                .take()
                .or_else(|| field.file_name().map(|s| s.to_string()))
                .unwrap_or_else(|| "dosya".into());
            let data = field.bytes().await.map_err(|e| ApiError::bad(format!("dosya okunamadı: {e}")))?;
            let rel = match kind {
                vardrop_core::ContentKind::Binary => sanitize_single(&rel)?,
                _ => sanitize_rel(&rel)?,
            };
            files.push((rel, data.to_vec()));
        }
    }
    if files.is_empty() {
        return Err(ApiError::bad("dosya alanı yok"));
    }

    match kind {
        vardrop_core::ContentKind::Binary => {
            if files.len() != 1 {
                return Err(ApiError::bad("binary için tek dosya"));
            }
            let (rel, bytes) = files.remove(0);
            let meta = EntryStoreMeta {
                id: entry.id.clone(),
                name: entry.name.clone(),
                kind,
                is_secret: entry.is_secret,
            };
            // orijinal dosya adını saklamak için description'a not (v0.1 pratik e)
            store_version(&state, &meta, entry.current_version + 1, &StoredContent::Bytes(bytes), Some(&user)).await?;
            let _ = rel;
        }
        vardrop_core::ContentKind::Bundle => {
            bump_bundle(&state, &entry.id, entry.current_version, entry.is_secret, &files, Some(&user)).await?;
        }
        _ => unreachable!(),
    }

    audit(&state.db, Some(user.uid()), "entry.file.upload", Some(&entry.id), None, None,
        Some(serde_json::json!({ "files": files.len() }))).await;

    Ok(Json(detail_of(&user, &state, &id).await?))
}

async fn bump_bundle(
    state: &SharedState,
    entry_id: &str,
    current: i64,
    is_secret: bool,
    changes: &[(String, Vec<u8>)],
    actor: Option<&AuthUser>,
) -> ApiResult<()> {
    let new_version = current + 1;
    // mevcut sürümün dosyalarını yeni sürüme kopyala
    let copies: Vec<FileRow> = bundle_files_of(&state.db, entry_id, current).await?;
    for f in &copies {
        sqlx::query(
            "INSERT OR IGNORE INTO bundle_files (id, entry_id, version, file_path, blob_path, size, sha256, is_secret)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(&f.id)
        .bind(entry_id)
        .bind(new_version)
        .bind(&f.file_path)
        .bind(&f.blob_path)
        .bind(f.size)
        .bind(&f.sha256)
        .bind(f.is_secret)
        .execute(&state.db)
        .await?;
    }
    // değişiklikleri uygula (aynı yolsa eskisini değiştir)
    for (rel, bytes) in changes {
        sqlx::query("DELETE FROM bundle_files WHERE entry_id = ? AND version = ? AND file_path = ?")
            .bind(entry_id)
            .bind(new_version)
            .bind(rel)
            .execute(&state.db)
            .await?;
        let (bp, size, sha) = storage::write_blob(state, entry_id, bytes, is_secret).await?;
        sqlx::query(
            "INSERT INTO bundle_files (id, entry_id, version, file_path, blob_path, size, sha256, is_secret)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(new_id("f"))
        .bind(entry_id)
        .bind(new_version)
        .bind(rel)
        .bind(bp)
        .bind(size)
        .bind(sha)
        .bind(is_secret)
        .execute(&state.db)
        .await?;
    }
    sqlx::query("UPDATE entries SET current_version = ?, updated_at = ?, updated_by = ? WHERE id = ?")
        .bind(new_version)
        .bind(now_ms())
        .bind(actor.map(|a| a.user.id.clone()))
        .bind(entry_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// GET /api/entries/{id}/files/{fid}
pub async fn get_file(
    user: AuthUser,
    State(state): State<SharedState>,
    Path((id, fid)): Path<(String, String)>,
) -> ApiResult<Response> {
    require(&user, Scope::Read)?;
    let _ = fetch_entry(&state.db, &id).await?;
    let f = fetch_file(&state.db, &id, &fid).await?;
    if f.is_secret {
        crate::auth::require_secret_access(&state, &user).await?;
    }
    let bytes = storage::read_blob(&state, &f.blob_path.unwrap_or_default(), f.is_secret).await?;
    audit(&state.db, Some(user.uid()), "file.read", Some(&f.file_path), None, None, None).await;
    let name = f.file_path.split('/').last().unwrap_or(&f.file_path).to_string();
    bytes_response(bytes, &name, true)
}

/// DELETE /api/entries/{id}/files/{fid} — yeni versiyona düşürür
pub async fn remove_file(
    user: AuthUser,
    State(state): State<SharedState>,
    Path((id, fid)): Path<(String, String)>,
) -> ApiResult<axum::response::Json<vardrop_core::EntryDetail>> {
    require(&user, Scope::Write)?;
    let entry = fetch_entry(&state.db, &id).await?;
    let f = fetch_file(&state.db, &id, &fid).await?;
    if f.version != entry.current_version {
        return Err(ApiError::conflict("yalnız güncel sürümdeki dosya silinebilir"));
    }
    let new_version = entry.current_version + 1;
    let copies: Vec<FileRow> = bundle_files_of(&state.db, &entry.id, entry.current_version).await?;
    for c in &copies {
        if c.id == f.id {
            continue;
        }
        sqlx::query(
            "INSERT OR IGNORE INTO bundle_files (id, entry_id, version, file_path, blob_path, size, sha256, is_secret)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(&c.id)
        .bind(&entry.id)
        .bind(new_version)
        .bind(&c.file_path)
        .bind(&c.blob_path)
        .bind(c.size)
        .bind(&c.sha256)
        .bind(c.is_secret)
        .execute(&state.db)
        .await?;
    }
    sqlx::query("UPDATE entries SET current_version = ?, updated_at = ?, updated_by = ? WHERE id = ?")
        .bind(new_version)
        .bind(now_ms())
        .bind(user.user.id.clone())
        .bind(&entry.id)
        .execute(&state.db)
        .await?;
    audit(&state.db, Some(user.uid()), "entry.file.delete", Some(&entry.id), None, None,
        Some(serde_json::json!({ "file": f.file_path }))).await;
    Ok(Json(detail_of(&user, &state, &id).await?))
}
