//! Kullanıcı yönetimi (yalnız admin): liste, oluştur, düzenle, sil.

use crate::auth::middleware::{require_admin, AuthUser};
use crate::auth::password;
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::util::{audit, valid_username};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use vardrop_core::{new_id, now_ms, User};
use serde::Deserialize;

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    username: String,
    display_name: String,
    role: String,
    is_active: bool,
    last_login_at: Option<i64>,
    created_at: i64,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        User {
            id: r.id,
            username: r.username,
            display_name: r.display_name,
            role: r.role,
            is_active: r.is_active,
            last_login_at: r.last_login_at,
            created_at: r.created_at,
        }
    }
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/users", get(list).post(create))
        .route("/api/users/{id}", patch(update).delete(delete_user))
}

const USER_COLS: &str = "id, username, display_name, role, is_active, created_at, last_login_at";

/// GET /api/users
pub async fn list(user: AuthUser, State(state): State<SharedState>) -> ApiResult<Json<Vec<User>>> {
    require_admin(&user)?;
    let rows = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLS} FROM users ORDER BY username"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(User::from).collect()))
}

#[derive(Deserialize)]
pub struct CreateUserReq {
    pub username: String,
    pub password: String,
    pub role: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// POST /api/users
pub async fn create(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateUserReq>,
) -> ApiResult<(StatusCode, Json<User>)> {
    require_admin(&user)?;
    let role = vardrop_core::Role::parse(&req.role)
        .ok_or_else(|| ApiError::bad("rol admin|editor|viewer olmalı"))?;
    valid_username(&req.username)?;
    if req.password.len() < state.cfg.security.password_min_len {
        return Err(ApiError::bad(format!(
            "şifre en az {} karakter olmalı",
            state.cfg.security.password_min_len
        )));
    }
    let exists: Option<UserRow> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(&req.username)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    if exists.is_some() {
        return Err(ApiError::conflict("kullanıcı adı alınmış"));
    }

    let hash = password::hash_password(&req.password)?;
    let uid = new_id("u");
    sqlx::query(
        "INSERT INTO users (id, username, display_name, password_hash, role, is_active, created_at)
         VALUES (?,?,?,?,?,1,?)",
    )
    .bind(&uid)
    .bind(&req.username)
    .bind(req.display_name.clone().unwrap_or_default())
    .bind(hash)
    .bind(role.as_str())
    .bind(now_ms())
    .execute(&state.db)
    .await?;

    audit(
        &state.db,
        Some(user.uid()),
        "user.create",
        Some(&uid),
        None,
        None,
        Some(serde_json::json!({ "username": req.username, "role": role.as_str() })),
    )
    .await;

    let row = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLS} FROM users WHERE id = ?"
    ))
    .bind(&uid)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

#[derive(Deserialize)]
pub struct PatchUserReq {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// PATCH /api/users/{id}
pub async fn update(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<PatchUserReq>,
) -> ApiResult<Json<User>> {
    require_admin(&user)?;
    let target: Option<UserRow> =
        sqlx::query_as(&format!("SELECT {USER_COLS} FROM users WHERE id = ?"))
            .bind(&id)
            .fetch_optional(&state.db)
            .await?;
    let Some(target) = target else {
        return Err(ApiError::not_found("kullanıcı bulunamadı"));
    };
    let _ = target;

    if let Some(role) = req.role.as_deref() {
        if vardrop_core::Role::parse(role).is_none() {
            return Err(ApiError::bad("rol admin|editor|viewer olmalı"));
        }
    }
    if let Some(pass) = req.password.as_deref() {
        if pass.len() < state.cfg.security.password_min_len {
            return Err(ApiError::bad(format!(
                "şifre en az {} karakter olmalı",
                state.cfg.security.password_min_len
            )));
        }
    }

    // son admin olmanın böçülmesi
    admin_change_guard(&state, &id, req.role.as_deref(), req.is_active).await?;

    let hash = match req.password.as_deref() {
        Some(p) => Some(password::hash_password(p)?),
        None => None,
    };

    sqlx::query(
        "UPDATE users SET
            role = COALESCE(?, role),
            is_active = COALESCE(?, is_active),
            display_name = COALESCE(?, display_name),
            password_hash = COALESCE(?, password_hash)
         WHERE id = ?",
    )
    .bind(req.role.clone())
    .bind(req.is_active)
    .bind(req.display_name.clone())
    .bind(hash)
    .bind(&id)
    .execute(&state.db)
    .await?;

    audit(
        &state.db,
        Some(user.uid()),
        "user.update",
        Some(&id),
        None,
        None,
        Some(serde_json::json!({
            "role": req.role, "is_active": req.is_active,
            "reset_password": req.password.is_some()
        })),
    )
    .await;

    let row = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLS} FROM users WHERE id = ?"
    ))
    .bind(&id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row.into()))
}

/// DELETE /api/users/{id}
pub async fn delete_user(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&user)?;
    if id == user.user.id {
        return Err(ApiError::bad("kendi hesabını silemezsin"));
    }
    let target: Option<UserRow> =
        sqlx::query_as(&format!("SELECT {USER_COLS} FROM users WHERE id = ?"))
            .bind(&id)
            .fetch_optional(&state.db)
            .await?;
    let Some(t) = target else {
        return Err(ApiError::not_found("kullanıcı bulunamadı"));
    };
    if t.role == "admin" {
        admin_change_guard(&state, &id, None, Some(false)).await?;
    }
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    audit(&state.db, Some(user.uid()), "user.delete", Some(&id), None, None,
        Some(serde_json::json!({ "username": t.username }))).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Sistemde aktif kalan en az bir admin olmalı.
async fn admin_change_guard(
    state: &SharedState,
    id: &str,
    new_role: Option<&str>,
    new_active: Option<bool>,
) -> ApiResult<()> {
    let target_row: Option<UserRow> =
        sqlx::query_as(&format!("SELECT {USER_COLS} FROM users WHERE id = ?"))
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some(t) = target_row else {
        return Err(ApiError::not_found("kullanıcı bulunamadı"));
    };
    let is_admin = t.role == "admin";
    let will_be_admin = match new_role {
        Some(r) => r == "admin",
        None => is_admin,
    };
    let will_be_active = new_active.unwrap_or(t.is_active);

    if is_admin && (!will_be_admin || !will_be_active) {
        let admins: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1 AND id != ?",
        )
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        if admins.0 == 0 {
            return Err(ApiError::conflict(
                "sistemde yalnız kalacak son admin değiştirilemez",
            ));
        }
    }
    Ok(())
}
