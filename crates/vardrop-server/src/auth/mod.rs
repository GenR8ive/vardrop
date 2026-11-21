//! Authentication & token Yönetimi.

pub mod middleware;
pub mod password;

pub use middleware::AuthUser;

use crate::crypto::random_token;
use crate::error::{ApiError, ApiResult};
use crate::state::{Guard, SharedState};
use crate::util::audit;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use vardrop_core::{new_id, now_ms, LoginResponse, Role, User};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::net::SocketAddr;
use std::time::Instant;

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

pub fn token_hash(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

#[derive(Deserialize)]
pub struct CreateTokenReq {
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_days: Option<i64>,
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> ApiResult<Json<LoginResponse>> {
    let ip = addr.ip().to_string();

    // kilit kontrolü
    {
        let map = state.login_guard.lock().await;
        if let Some(g) = map.get(&ip) {
            if let Some(until) = g.locked_until {
                if Instant::now() < until {
                    return Err(ApiError::forbidden(
                        "çok fazla hatalı deneme; 60 sn bekleyin",
                    ));
                }
            }
        }
    }

    let row = sqlx::query(
        "SELECT id, username, display_name, role, is_active, created_at, last_login_at, password_hash
         FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(&state.db)
    .await?;

    let Some(urow) = row else {
        bump_failed(&state, &ip).await;
        audit_failed_login(&state, &req.username, &ip, &headers).await;
        return Err(ApiError::unauthorized("kullanıcı adı veya şifre hatalı"));
    };

    let hash: String = urow.get("password_hash");
    if !password::verify_password(&hash, &req.password) {
        bump_failed(&state, &ip).await;        audit_failed_login(&state, &req.username, &ip, &headers).await;
        return Err(ApiError::unauthorized("kullanıcı adı veya şifre hatalı"));
    }

    let is_active: bool = urow.get("is_active");
    if !is_active {
        return Err(ApiError::forbidden("hesap pasif durumda"));
    }

    let role_s: String = urow.get("role");
    let role = Role::parse(&role_s).unwrap_or(Role::Viewer);

    // oturum token'ı üret ve kaydet (PAT gibi ama süre kısa)
    let token = random_token();
    let expires = now_ms() + (state.cfg.security.session_ttl_hours as i64) * 3_600_000;
    let scopes: Vec<String> = role.allowed_scopes().iter().map(|s| s.as_str().to_string()).collect();
    let tid = new_id("t");
    sqlx::query(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, scopes, created_at, expires_at)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&tid)
    .bind(urow.get::<String, _>("id"))
    .bind("session")
    .bind(token_hash(&token))
    .bind(serde_json::to_string(&scopes).unwrap_or_default())
    .bind(now_ms())
    .bind(expires)
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(now_ms())
        .bind(urow.get::<String, _>("id"))
        .execute(&state.db)
        .await?;

    let uid = urow.get::<String, _>("id");
    let uname = urow.get::<String, _>("username");
    audit(
        &state.db,
        Some((uid.clone(), uname.clone())),
        "login",
        None,
        Some(&ip),
        header_ua(&headers),
        Some(serde_json::json!({ "scopes": scopes })),
    )
    .await;

    state.login_guard.lock().await.remove(&ip);

    Ok(Json(LoginResponse {
        token,
        user: middleware::user_from_row(&urow, "id"),
    }))
}

fn header_ua(headers: &HeaderMap) -> Option<&str> {
    headers.get("user-agent").and_then(|v| v.to_str().ok())
}

async fn audit_failed_login(state: &SharedState, username: &str, ip: &str, headers: &HeaderMap) {
    audit(
        &state.db,
        None,
        "login.failed",
        None,
        Some(ip),
        header_ua(headers),
        Some(serde_json::json!({ "username": username })),
    )
    .await;
}

async fn bump_failed(state: &SharedState, key: &str) {
    let mut map = state.login_guard.lock().await;
    let g = map.entry(key.to_string()).or_insert(Guard {
        fails: 0,
        locked_until: None,
    });
    g.fails += 1;
    if g.fails >= 5 {
        g.locked_until = Some(Instant::now() + std::time::Duration::from_secs(60));
        g.fails = 0;
    }
}

/// GET /api/auth/me
pub async fn me(user: AuthUser) -> Json<User> {
    Json(user.user)
}

/// GET /api/auth/tokens — kendi tokenları
pub async fn list_tokens(user: AuthUser, State(state): State<SharedState>) -> ApiResult<Json<Vec<vardrop_core::TokenInfo>>> {
    let rows = sqlx::query(
        "SELECT id, name, scopes, expires_at, last_used_at, created_at
         FROM api_tokens WHERE user_id = ? AND revoked_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(&user.user.id)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for r in rows {
        let scopes_raw: String = r.get("scopes");
        let scopes: Vec<String> = serde_json::from_str(&scopes_raw).unwrap_or_default();
        out.push(vardrop_core::TokenInfo {
            id: r.get("id"),
            name: r.get("name"),
            scopes,
            expires_at: r.get("expires_at"),
            last_used_at: r.get("last_used_at"),
            created_at: r.get("created_at"),
        });
    }
    Ok(Json(out))
}

/// POST /api/auth/tokens — yeni PAT
pub async fn create_token(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateTokenReq>,
) -> ApiResult<(StatusCode, Json<vardrop_core::CreatedToken>)> {
    let role = Role::parse(&user.user.role).unwrap_or(Role::Viewer);
    let allowed: Vec<String> = role
        .allowed_scopes()
        .iter()
        .map(|s| s.as_str().to_string())
        .collect();
    let scopes: Vec<String> = req
        .scopes
        .iter()
        .filter_map(|s| vardrop_core::Scope::parse(s))
        .map(|s| s.as_str().to_string())
        .filter(|s| allowed.contains(s))
        .collect();
    if scopes.is_empty() {
        return Err(ApiError::bad("kapsam seçilmedi (rolün izin verdiği: read/write/admin)"));
    }
    if req.name.trim().is_empty() {
        return Err(ApiError::bad("token adı gerekli"));
    }
    if let Some(d) = req.expires_days {
        if !(1..=3650).contains(&d) {
            return Err(ApiError::bad("geçerlilik 1–3650 gün arası olmalı (boş = süresiz)"));
        }
    }

    let token = random_token();
    let tid = new_id("t");
    let expires = req.expires_days.map(|d| now_ms() + (d as i64) * 86_400_000);
    sqlx::query(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, scopes, created_at, expires_at)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&tid)
    .bind(&user.user.id)
    .bind(req.name.trim())
    .bind(token_hash(&token))
    .bind(serde_json::to_string(&scopes).unwrap_or_default())
    .bind(now_ms())
    .bind(expires)
    .execute(&state.db)
    .await?;

    audit(
        &state.db,
        Some((user.user.id.clone(), user.user.username.clone())),
        "token.create",
        Some(&tid),
        None,
        None,
        Some(serde_json::json!({ "name": req.name, "scopes": scopes })),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(vardrop_core::CreatedToken { id: tid, token }),
    ))
}

/// DELETE /api/auth/tokens/{id}
pub async fn delete_token(
    user: AuthUser,
    State(state): State<SharedState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult<axum::Json<serde_json::Value>> {
    sqlx::query("UPDATE api_tokens SET revoked_at = ? WHERE id = ? AND user_id = ?")
        .bind(now_ms())
        .bind(&id)
        .bind(&user.user.id)
        .execute(&state.db)
        .await?;
    audit(
        &state.db,
        Some((user.user.id.clone(), user.user.username.clone())),
        "token.revoke",
        Some(&id),
        None,
        None,
        None,
    )
    .await;
    Ok(axum::Json(serde_json::json!({ "ok": true })))
}

// ---------------- secret PIN kapısı ----------------

/// Secret unlock penceresi (PIN doğrulanınca token bu kadar süre serbesttir).
const UNLOCK_SECS: u64 = 600;

/// Unlock map anahtarı: kullanıcı + token bazlı.
fn unlock_key(user: &AuthUser) -> String {
    format!("{}:{}", user.user.id, user.token_id)
}

/// Kullanıcının PIN hash'i (yoksa None).
async fn pin_hash_of(state: &SharedState, uid: &str) -> ApiResult<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT pin_hash FROM users WHERE id = ?")
            .bind(uid)
            .fetch_optional(&state.db)
            .await?;
    Ok(row
        .and_then(|r| r.0)
        .filter(|h: &String| !h.is_empty()))
}

/// Secret okuma kapısı: kullanıcı PIN belirlemişse token'ın unlock penceresi şart.
/// PIN belirlememiş kullanıcı için davranış değişmez (geriye uyumlu).
pub async fn require_secret_access(state: &SharedState, user: &AuthUser) -> ApiResult<()> {
    if pin_hash_of(state, &user.user.id).await?.is_none() {
        return Ok(());
    }
    let mut unlocks = state.unlocks.lock().await;
    let now = Instant::now();
    unlocks.retain(|_, exp| *exp > now);
    if unlocks.contains_key(&unlock_key(user)) {
        Ok(())
    } else {
        Err(ApiError::pin_required())
    }
}

#[derive(Deserialize)]
pub struct PinReq {
    pub pin: String,
}

/// POST /api/auth/unlock — PIN doğrular → 10 dk secret unlock penceresi açar.
/// Aynı kaba kuvvet perdesi: 5 hatalı → IP 60 sn kilit (login_guard ile aynı map).
pub async fn unlock(
    user: AuthUser,
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<PinReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let ip = addr.ip().to_string();
    let guard_key = format!("pin:{ip}");
    {
        let map = state.login_guard.lock().await;
        if let Some(g) = map.get(&guard_key) {
            if let Some(until) = g.locked_until {
                if Instant::now() < until {
                    return Err(ApiError::forbidden("çok fazla hatalı deneme; 60 sn bekleyin"));
                }
            }
        }
    }

    let Some(hash) = pin_hash_of(&state, &user.user.id).await? else {
        return Err(ApiError::bad("PIN belirlememişsiniz — önce PUT /api/auth/pin"));
    };
    if !password::verify_password(&hash, req.pin.trim()) {
        bump_failed(&state, &guard_key).await;
        audit(
            &state.db,
            Some(user.uid()),
            "auth.unlock.failed",
            None,
            Some(&ip),
            None,
            None,
        )
        .await;
        return Err(ApiError::unauthorized("PIN hatalı"));
    }

    state.login_guard.lock().await.remove(&guard_key);
    let mut unlocks = state.unlocks.lock().await;
    unlocks.insert(
        unlock_key(&user),
        Instant::now() + std::time::Duration::from_secs(UNLOCK_SECS),
    );
    audit(
        &state.db,
        Some(user.uid()),
        "auth.unlock",
        None,
        Some(&ip),
        None,
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true, "expires_in": UNLOCK_SECS })))
}

#[derive(Deserialize)]
pub struct SetPinReq {
    /// Mevcut PIN (değiştirme/kaldırma için gerekli; ilk kurulumda boş).
    #[serde(default)]
    pub current_pin: Option<String>,
    /// Yeni PIN; boş/None = PIN kaldırılır.
    #[serde(default)]
    pub new_pin: Option<String>,
}

/// GET /api/auth/pin — PIN durumunu döner (panel arayüzü için).
pub async fn pin_status(
    user: AuthUser,
    State(state): State<SharedState>,
) -> ApiResult<Json<serde_json::Value>> {
    let set = pin_hash_of(&state, &user.user.id).await?.is_some();
    Ok(Json(serde_json::json!({ "set": set })))
}

/// PUT /api/auth/pin — PIN belirle / değiştir / kaldır (kendi hesabı).
/// Değişiklikte kullanıcının tüm unlock pencereleri düşer.
pub async fn set_pin(
    user: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<SetPinReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let existing = pin_hash_of(&state, &user.user.id).await?;
    if let Some(hash) = &existing {
        let cur = req.current_pin.as_deref().map(str::trim).unwrap_or("");
        if !password::verify_password(hash, cur) {
            return Err(ApiError::unauthorized("mevcut PIN hatalı"));
        }
    }

    let new_pin = req
        .new_pin
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty());

    let action;
    match new_pin {
        Some(pin) => {
            if pin.len() < 4 || pin.len() > 64 {
                return Err(ApiError::bad("PIN 4–64 karakter olmalı"));
            }
            let hash = password::hash_password(pin)?;
            sqlx::query("UPDATE users SET pin_hash = ? WHERE id = ?")
                .bind(hash)
                .bind(&user.user.id)
                .execute(&state.db)
                .await?;
            action = "pin.set";
        }
        None => {
            sqlx::query("UPDATE users SET pin_hash = NULL WHERE id = ?")
                .bind(&user.user.id)
                .execute(&state.db)
                .await?;
            action = "pin.clear";
        }
    }

    // PIN değişti: bu kullanıcının tüm unlock pencereleri geçersiz.
    {
        let prefix = format!("{}:", user.user.id);
        let mut unlocks = state.unlocks.lock().await;
        unlocks.retain(|k, _| !k.starts_with(&prefix));
    }

    audit(
        &state.db,
        Some(user.uid()),
        action,
        None,
        None,
        None,
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

impl AuthUser {
    pub fn uid(&self) -> (String, String) {
        (self.user.id.clone(), self.user.username.clone())
    }
}
