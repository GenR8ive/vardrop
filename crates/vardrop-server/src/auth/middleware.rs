//! Bearer token doğrulama extractor'ı + yetki (RBAC) yardımcıları.

use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use vardrop_core::{Scope, User};
use sqlx::Row;
use std::collections::{HashMap, HashSet};

#[allow(dead_code)] // token_id ileride debug/header bilgisi için
pub struct AuthUser {
    pub user: User,
    pub scopes: Vec<Scope>,
    pub token_id: String,
    /// RBAC: etkin yetkiler (kendi + departman grant'ları).
    pub perms: Perms,
}

/// Kullanıcının etkin yetkileri: global küme + cluster başına kümeler.
#[derive(Debug, Default, Clone)]
pub struct Perms {
    pub global: HashSet<String>,
    pub clusters: HashMap<String, HashSet<String>>,
}

impl Perms {
    /// Global yetki (users.manage gibi yalnız-global anahtarlar dahil).
    pub fn can(&self, p: &str) -> bool {
        self.global.contains(p)
    }

    /// Cluster'a özgü yetki: global küme ∪ o cluster'ın kümesi.
    pub fn can_on(&self, cluster_id: &str, p: &str) -> bool {
        self.can(p)
            || self
                .clusters
                .get(cluster_id)
                .map(|s| s.contains(p))
                .unwrap_or(false)
    }
}

/// Kullanıcının grant'larını (bireysel + departman) yükleyip Perms üretir.
pub async fn load_perms(db: &sqlx::SqlitePool, uid: &str) -> Perms {
    let mut perms = Perms::default();
    let rows = sqlx::query(
        "SELECT g.cluster_id, r.permissions
         FROM grants g JOIN roles r ON r.id = g.role_id
         WHERE (g.subject_type = 'user' AND g.subject_id = ?1)
            OR (g.subject_type = 'department' AND g.subject_id IN
                (SELECT department_id FROM department_members WHERE user_id = ?1))",
    )
    .bind(uid)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for r in rows {
        let cluster_id: Option<String> = r.get("cluster_id");
        let raw: String = r.get("permissions");
        let list: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
        let target = match cluster_id.as_deref() {
            None | Some("") => &mut perms.global,
            Some(cid) => perms.clusters.entry(cid.to_string()).or_default(),
        };
        for p in list {
            if vardrop_core::valid_permission(&p) {
                target.insert(p);
            }
        }
    }
    perms
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

        let perms = load_perms(&state.db, &user.id).await;

        Ok(AuthUser { user, scopes, token_id, perms })
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

/// Global yetki kontrolü (users.manage, roles.manage, audit.read gibi).
pub fn require_perm(auth: &AuthUser, perm: &str) -> ApiResult<()> {
    if auth.perms.can(perm) {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!("gerekli yetki: {perm}")))
    }
}

/// Cluster'a özgü yetki kontrolü: global küme ∪ cluster kümesi.
pub fn require_cluster_perm(auth: &AuthUser, cluster_id: &str, perm: &str) -> ApiResult<()> {
    if auth.perms.can_on(cluster_id, perm) {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "bu cluster için gerekli yetki: {perm}"
        )))
    }
}

/// Admin kontrolü (geriye uyumlu; eski çağrı noktaları).
pub fn require_admin(auth: &AuthUser) -> ApiResult<()> {
    require_perm(auth, "users.manage")
}
