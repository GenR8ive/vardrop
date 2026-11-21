//! Bearer token doğrulama extractor'ı + rol/kapsam yardımcıları.

use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use vardrop_core::{Role, Scope, User};
use sqlx::Row;

#[allow(dead_code)] // token_id ileride debug/header bilgisi için
pub struct AuthUser {
    pub user: User,
    pub scopes: Vec<Scope>,
    pub token_id: String,
}

impl FromRequestParts<SharedState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ApiError::unauthorized("kullanıcının token'ı gerekli (Authorization: Bearer)"))?;

        let hash = crate::auth::token_hash(token);
        let row = sqlx::query(
            "SELECT t.id AS token_id, t.scopes, t.expires_at, t.revoked_at,
                    u.id AS uid, u.username, u.display_name, u.role, u.is_active, u.created_at, u.last_login_at
             FROM api_tokens t
             JOIN users u ON u.id = t.user_id
             WHERE t.token_hash = ?",
        )
        .bind(hash)
        .fetch_optional(&state.db)
        .await
        .map_err(ApiError::from)?;

        let Some(r) = row else {
            return Err(ApiError::unauthorized("token geçersiz"));
        };

        let revoked: Option<i64> = r.get("revoked_at");
        let expires: Option<i64> = r.get("expires_at");
        let is_active: bool = r.get("is_active");
        let now = vardrop_core::now_ms();

        if revoked.is_some() {
            return Err(ApiError::unauthorized("token iptal edilmiş"));
        }
        if let Some(exp) = expires {
            if exp < now {
                return Err(ApiError::unauthorized("token süresi doldu"));
            }
        }
        if !is_active {
            return Err(ApiError::forbidden("kullanıcı pasif"));
        }

        let scopes_raw: String = r.get("scopes");
        let scopes: Vec<Scope> = serde_json::from_str::<Vec<String>>(&scopes_raw)
            .unwrap_or_default()
            .iter()
            .filter_map(|s| Scope::parse(s))
            .collect();

        let token_id: String = r.get("token_id");
        let user = user_from_row(&r, "uid");

        // last_used güncelle (en iyi çaba)
        let _ = sqlx::query("UPDATE api_tokens SET last_used_at = ? WHERE id = ?")
            .bind(now)
            .bind(&token_id)
            .execute(&state.db)
            .await;

        Ok(AuthUser { user, scopes, token_id })
    }
}

/// Satırdan User üretir. `uid_col` join sorgularında u.id kolonunun alias'ıdır.
pub(crate) fn user_from_row(r: &sqlx::sqlite::SqliteRow, uid_col: &str) -> User {
    User {
        id: r.get(uid_col),
        username: r.get("username"),
        display_name: r.get("display_name"),
        role: r.get("role"),
        is_active: r.get("is_active"),
        last_login_at: r.get("last_login_at"),
        created_at: r.get("created_at"),
    }
}

/// Kapsam kontrolü: tokenda yoksa 403.
pub fn require(auth: &AuthUser, scope: Scope) -> ApiResult<()> {
    if auth.scopes.contains(&scope) {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "gerekli kapsam: {}",
            scope.as_str()
        )))
    }
}

/// Admin kontrolü.
pub fn require_admin(auth: &AuthUser) -> ApiResult<()> {
    let role = Role::parse(&auth.user.role).unwrap_or(Role::Viewer);
    if role == Role::Admin && auth.scopes.contains(&Scope::Admin) {
        Ok(())
    } else {
        Err(ApiError::forbidden("bu işlem yalnız admin içindir"))
    }
}
