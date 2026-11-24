//! RBAC yÃ¶netimi: roller, departmanlar, yetki atamalarÄ± (roles.manage kapÄ±sÄ±).
//!
//! Model: `grants` satÄ±rÄ± = (kullanÄ±cÄ±|departman) Ã— rol Ã— cluster (NULL = global).
//! Etkin yetki = bireysel grant'lar âˆª Ã¼yesi olunan departmanlarÄ±n grant'larÄ±.

use crate::auth::middleware::{require_perm, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::util::audit;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::Row;
use vardrop_core::{sanitize_permissions, DepartmentInfo, GrantInfo, PermsSummary, RoleDef};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/roles", get(list_roles).post(create_role))
        .route("/api/roles/{id}", axum::routing::patch(patch_role).delete(delete_role))
        .route("/api/departments", get(list_departments).post(create_department))
        .route("/api/departments/{id}", axum::routing::patch(patch_department).delete(delete_department))
        .route(
            "/api/departments/{id}/members",
            get(list_members).post(add_member),
        )
        .route("/api/departments/{id}/members/{user_id}", delete(remove_member))
        .route("/api/grants", get(list_grants).post(create_grant))
        .route("/api/grants/{id}", delete(delete_grant))
        .route("/api/auth/perms", get(my_perms))
}

fn row_role(r: &sqlx::sqlite::SqliteRow) -> RoleDef {
    let raw: String = r.get("permissions");
    RoleDef {
        id: r.get("id"),
        name: r.get("name"),
        description: r.get("description"),
        permissions: serde_json::from_str(&raw).unwrap_or_default(),
        is_system: r.get::<i64, _>("is_system") != 0,
        created_at: r.get("created_at"),
    }
}

// ---------------- roller ----------------

/// GET /api/roles
async fn list_roles(user: AuthUser, State(state): State<SharedState>) -> ApiResult<Json<Vec<RoleDef>>> {
    require_perm(&user, "roles.manage")?;
    let rows = sqlx::query(
        "SELECT id, name, description, permissions, is_system, created_at
         FROM roles ORDER BY is_system DESC, name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.iter().map(row_role).collect()))
}

#[derive(Deserialize)]
struct RoleReq {
    /// PATCH'te yok sayılır (rol adı sabit); create'de zorunlu.
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    permissions: Vec<String>,
}

/// POST /api/roles â€” yeni rol grubu.
async fn create_role(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<RoleReq>,
) -> ApiResult<(StatusCode, Json<RoleDef>)> {
    require_perm(&user, "roles.manage")?;
    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::bad("rol adÄ± 1â€“64 karakter olmalÄ±"));
    }
    let perms = sanitize_permissions(&req.permissions);
    if perms.is_empty() {
        return Err(ApiError::bad("en az bir geÃ§erli yetki seÃ§ilmeli"));
    }
    let dup: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM roles WHERE name = ?")
        .bind(name)
        .fetch_optional(&state.db)
        .await?;
    if dup.is_some() {
        return Err(ApiError::conflict("bu adlÄ± rol var"));
    }

    let id = vardrop_core::new_id("role");
    sqlx::query(
        "INSERT INTO roles (id, name, description, permissions, is_system, created_at) VALUES (?,?,?,?,0,?)",
    )
    .bind(&id)
    .bind(name)
    .bind(req.description.clone().unwrap_or_default())
    .bind(serde_json::to_string(&perms).unwrap_or_default())
    .bind(vardrop_core::now_ms())
    .execute(&state.db)
    .await?;

    audit(&state.db, Some(user.uid()), "role.create", Some(&id), None, None,
        Some(serde_json::json!({ "name": name, "permissions": perms }))).await;

    Ok((
        StatusCode::CREATED,
        Json(RoleDef {
            id,
            name: name.to_string(),
            description: req.description.unwrap_or_default(),
            permissions: perms,
            is_system: false,
            created_at: vardrop_core::now_ms(),
        }),
    ))
}

/// PATCH /api/roles/{id} â€” aÃ§Ä±klama + yetki seti (ad sabit).
async fn patch_role(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<RoleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&user, "roles.manage")?;
    let row = sqlx::query("SELECT id, is_system FROM roles WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("rol bulunamadÄ±"))?;
    let is_system: i64 = row.get("is_system");

    let mut perms = sanitize_permissions(&req.permissions);
    if is_system != 0 && id == "role_admin" {
        // admin sistem rolÃ¼ her zaman tam yetkili kalÄ±r (kilitlenme korumasÄ±)
        perms = vardrop_core::PERMISSIONS.iter().map(|p| p.to_string()).collect();
    }
    if perms.is_empty() {
        return Err(ApiError::bad("en az bir geÃ§erli yetki seÃ§ilmeli"));
    }

    sqlx::query("UPDATE roles SET description = ?, permissions = ? WHERE id = ?")
        .bind(req.description.clone().unwrap_or_default())
        .bind(serde_json::to_string(&perms).unwrap_or_default())
        .bind(&id)
        .execute(&state.db)
        .await?;

    audit(&state.db, Some(user.uid()), "role.update", Some(&id), None, None,
        Some(serde_json::json!({ "permissions": perms }))).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/roles/{id} â€” sistem rolleri silinemez; grant'lar cascade dÃ¼ÅŸer.
async fn delete_role(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&user, "roles.manage")?;
    let row = sqlx::query("SELECT is_system FROM roles WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("rol bulunamadÄ±"))?;
    if row.get::<i64, _>("is_system") != 0 {
        return Err(ApiError::forbidden("sistem rolÃ¼ silinemez"));
    }
    sqlx::query("DELETE FROM roles WHERE id = ?").bind(&id).execute(&state.db).await?;
    audit(&state.db, Some(user.uid()), "role.delete", Some(&id), None, None, None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------- departmanlar ----------------

/// GET /api/departments
async fn list_departments(
    user: AuthUser,
    State(state): State<SharedState>,
) -> ApiResult<Json<Vec<DepartmentInfo>>> {
    require_perm(&user, "roles.manage")?;
    let rows = sqlx::query(
        "SELECT d.id, d.name, d.description, d.created_at,
                (SELECT COUNT(*) FROM department_members m WHERE m.department_id = d.id) AS member_count
         FROM departments d ORDER BY d.name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows
        .into_iter()
        .map(|r| DepartmentInfo {
            id: r.get("id"),
            name: r.get("name"),
            description: r.get("description"),
            member_count: r.get("member_count"),
            created_at: r.get("created_at"),
        })
        .collect()))
}

#[derive(Deserialize)]
struct DepartmentReq {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

/// POST /api/departments
async fn create_department(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<DepartmentReq>,
) -> ApiResult<(StatusCode, Json<DepartmentInfo>)> {
    require_perm(&user, "roles.manage")?;
    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::bad("departman adÄ± 1â€“64 karakter olmalÄ±"));
    }
    let dup: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM departments WHERE name = ?")
        .bind(name)
        .fetch_optional(&state.db)
        .await?;
    if dup.is_some() {
        return Err(ApiError::conflict("bu adlÄ± departman var"));
    }
    let id = vardrop_core::new_id("dept");
    sqlx::query(
        "INSERT INTO departments (id, name, description, created_at) VALUES (?,?,?,?)",
    )
    .bind(&id)
    .bind(name)
    .bind(req.description.clone().unwrap_or_default())
    .bind(vardrop_core::now_ms())
    .execute(&state.db)
    .await?;
    audit(&state.db, Some(user.uid()), "department.create", Some(&id), None, None,
        Some(serde_json::json!({ "name": name }))).await;
    Ok((
        StatusCode::CREATED,
        Json(DepartmentInfo {
            id,
            name: name.to_string(),
            description: req.description.unwrap_or_default(),
            member_count: 0,
            created_at: vardrop_core::now_ms(),
        }),
    ))
}

/// PATCH /api/departments/{id}
async fn patch_department(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<DepartmentReq>,
) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&user, "roles.manage")?;
    let n = req.name.trim();
    if n.is_empty() || n.len() > 64 {
        return Err(ApiError::bad("departman adÄ± 1â€“64 karakter olmalÄ±"));
    }
    let res = sqlx::query("UPDATE departments SET name = ?, description = ? WHERE id = ?")
        .bind(n)
        .bind(req.description.clone().unwrap_or_default())
        .bind(&id)
        .execute(&state.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::not_found("departman bulunamadÄ±"));
    }
    audit(&state.db, Some(user.uid()), "department.update", Some(&id), None, None, None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/departments/{id} â€” Ã¼yelik + grant'lar cascade dÃ¼ÅŸer.
async fn delete_department(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&user, "roles.manage")?;
    sqlx::query("DELETE FROM departments WHERE id = ?").bind(&id).execute(&state.db).await?;
    audit(&state.db, Some(user.uid()), "department.delete", Some(&id), None, None, None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/departments/{id}/members â€” (id, username) Ã§iftleri.
async fn list_members(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    require_perm(&user, "roles.manage")?;
    let rows = sqlx::query(
        "SELECT u.id, u.username, u.display_name FROM department_members m
         JOIN users u ON u.id = m.user_id WHERE m.department_id = ? ORDER BY u.username",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows
        .into_iter()
        .map(|r| serde_json::json!({
            "id": r.get::<String, _>("id"),
            "username": r.get::<String, _>("username"),
            "display_name": r.get::<String, _>("display_name"),
        }))
        .collect()))
}

#[derive(Deserialize)]
struct MemberReq {
    user_id: String,
}

/// POST /api/departments/{id}/members
async fn add_member(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<MemberReq>,
) -> ApiResult<StatusCode> {
    require_perm(&user, "roles.manage")?;
    let dept: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM departments WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await?;
    if dept.is_none() {
        return Err(ApiError::not_found("departman bulunamadÄ±"));
    }
    let u: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM users WHERE id = ?")
        .bind(&req.user_id)
        .fetch_optional(&state.db)
        .await?;
    if u.is_none() {
        return Err(ApiError::not_found("kullanÄ±cÄ± bulunamadÄ±"));
    }
    sqlx::query("INSERT OR IGNORE INTO department_members (department_id, user_id) VALUES (?,?)")
        .bind(&id)
        .bind(&req.user_id)
        .execute(&state.db)
        .await?;
    audit(&state.db, Some(user.uid()), "department.member.add", Some(&id), None, None,
        Some(serde_json::json!({ "user_id": req.user_id }))).await;
    Ok(StatusCode::CREATED)
}

/// DELETE /api/departments/{id}/members/{user_id}
async fn remove_member(
    user: AuthUser,
    State(state): State<SharedState>,
    Path((id, user_id)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&user, "roles.manage")?;
    sqlx::query("DELETE FROM department_members WHERE department_id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user_id)
        .execute(&state.db)
        .await?;
    audit(&state.db, Some(user.uid()), "department.member.remove", Some(&id), None, None,
        Some(serde_json::json!({ "user_id": user_id }))).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------- atamalar (grants) ----------------

#[derive(Deserialize)]
struct GrantsQuery {
    subject_type: Option<String>,
    subject_id: Option<String>,
}

fn row_grant(r: &sqlx::sqlite::SqliteRow) -> GrantInfo {
    let raw: String = r.get("permissions");
    GrantInfo {
        id: r.get("id"),
        subject_type: r.get("subject_type"),
        subject_id: r.get("subject_id"),
        subject_name: r.get("subject_name"),
        role_id: r.get("role_id"),
        role_name: r.get("role_name"),
        cluster_id: r.get("cluster_id"),
        cluster_name: r.get("cluster_name"),
        permissions: serde_json::from_str(&raw).unwrap_or_default(),
        created_at: r.get("created_at"),
    }
}

/// GET /api/grants?subject_type=&subject_id= â€” filtresiz hepsi dÃ¶ner.
async fn list_grants(
    user: AuthUser,
    State(state): State<SharedState>,
    Query(q): Query<GrantsQuery>,
) -> ApiResult<Json<Vec<GrantInfo>>> {
    require_perm(&user, "roles.manage")?;

    let mut sql = String::from(
        "SELECT g.id, g.subject_type, g.subject_id, g.role_id, g.cluster_id, g.created_at,
                r.name AS role_name, r.permissions,
                COALESCE(c.name, '') AS cluster_name,
                CASE g.subject_type
                    WHEN 'user' THEN (SELECT username FROM users u WHERE u.id = g.subject_id)
                    ELSE (SELECT name FROM departments d WHERE d.id = g.subject_id)
                END AS subject_name
         FROM grants g
         JOIN roles r ON r.id = g.role_id
         LEFT JOIN clusters c ON c.id = g.cluster_id
         WHERE 1=1",
    );
    if let Some(st) = q.subject_type.as_deref() {
        if st == "user" || st == "department" {
            sql.push_str(" AND g.subject_type = ?");
        }
    }
    if q.subject_id.is_some() {
        sql.push_str(" AND g.subject_id = ?");
    }
    sql.push_str(" ORDER BY g.subject_type, g.subject_id, g.cluster_id IS NULL DESC, r.name");

    let mut query = sqlx::query(&sql);
    if let Some(st) = q.subject_type.as_deref() {
        if st == "user" || st == "department" {
            query = query.bind(st);
        }
    }
    if let Some(sid) = q.subject_id.as_deref() {
        query = query.bind(sid);
    }
    let rows = query.fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_grant).collect()))
}

#[derive(Deserialize)]
struct GrantReq {
    subject_type: String, // user | department
    subject_id: String,
    role_id: String,
    /// None/boÅŸ = global.
    #[serde(default)]
    cluster_id: Option<String>,
}

/// POST /api/grants â€” atama ekle.
async fn create_grant(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<GrantReq>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    require_perm(&user, "roles.manage")?;
    if !matches!(req.subject_type.as_str(), "user" | "department") {
        return Err(ApiError::bad("subject_type: user | department"));
    }
    let subj: Option<(i64,)> = match req.subject_type.as_str() {
        "user" => sqlx::query_as("SELECT 1 FROM users WHERE id = ?").bind(&req.subject_id).fetch_optional(&state.db).await?,
        _ => sqlx::query_as("SELECT 1 FROM departments WHERE id = ?").bind(&req.subject_id).fetch_optional(&state.db).await?,
    };
    if subj.is_none() {
        return Err(ApiError::bad("subject bulunamadÄ±"));
    }
    let role: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM roles WHERE id = ?")
        .bind(&req.role_id)
        .fetch_optional(&state.db)
        .await?;
    if role.is_none() {
        return Err(ApiError::bad("rol bulunamadÄ±"));
    }
    let cluster_id = req
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(cid) = cluster_id {
        let c: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM clusters WHERE id = ?")
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
        if c.is_none() {
            return Err(ApiError::bad("cluster bulunamadÄ±"));
        }
    }

    let res = sqlx::query(
        "INSERT INTO grants (subject_type, subject_id, role_id, cluster_id, created_at) VALUES (?,?,?,?,?)",
    )
    .bind(&req.subject_type)
    .bind(&req.subject_id)
    .bind(&req.role_id)
    .bind(cluster_id)
    .bind(vardrop_core::now_ms())
    .execute(&state.db)
    .await
    .map_err(|e| {
        // UNIQUE ihlali: aynÄ± atama zaten var
        if e.to_string().contains("UNIQUE") {
            ApiError::conflict("bu atama zaten var")
        } else {
            ApiError::from(e)
        }
    })?;

    audit(&state.db, Some(user.uid()), "grant.create", Some(&res.last_insert_rowid().to_string()), None, None,
        Some(serde_json::json!({
            "subject_type": req.subject_type, "subject_id": req.subject_id,
            "role_id": req.role_id, "cluster_id": cluster_id,
        }))).await;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true, "id": res.last_insert_rowid() }))))
}

/// DELETE /api/grants/{id}
async fn delete_grant(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&user, "roles.manage")?;
    sqlx::query("DELETE FROM grants WHERE id = ?").bind(id).execute(&state.db).await?;
    audit(&state.db, Some(user.uid()), "grant.delete", Some(&id.to_string()), None, None, None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------- kendi yetkilerim ----------------

/// GET /api/auth/perms â€” panel menÃ¼ kapÄ±sÄ±; herkes kendi Ã¶zetini okuyabilir.
async fn my_perms(user: AuthUser) -> Json<PermsSummary> {
    let mut clusters: std::collections::HashMap<String, Vec<String>> = Default::default();
    for (cid, set) in &user.perms.clusters {
        let mut list: Vec<String> = set.iter().cloned().collect();
        list.sort();
        clusters.insert(cid.clone(), list);
    }
    let mut global: Vec<String> = user.perms.global.iter().cloned().collect();
    global.sort();
    Json(PermsSummary { global, clusters })
}
