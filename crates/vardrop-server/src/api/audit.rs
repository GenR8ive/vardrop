//! Audit listesi (yalnız admin).

use crate::auth::middleware::{require_admin, AuthUser};
use crate::error::ApiResult;
use crate::state::SharedState;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use vardrop_core::AuditRow;
use serde::Deserialize;
use sqlx::Row;

pub fn router() -> Router<SharedState> {
    Router::new().route("/api/audit", get(list))
}

#[derive(Deserialize, Default)]
pub struct AuditParams {
    pub action: Option<String>,
    pub username: Option<String>,
    pub user_id: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub limit: Option<i64>,
}

/// GET /api/audit
pub async fn list(
    user: AuthUser,
    State(state): State<SharedState>,
    Query(p): Query<AuditParams>,
) -> ApiResult<Json<Vec<AuditRow>>> {
    require_admin(&user)?;

    let mut sql = String::from(
        "SELECT id, user_id, username, action, resource, ip, detail, created_at FROM audit_log WHERE 1=1",
    );
    if p.action.is_some() {
        sql.push_str(" AND action = ?");
    }
    if p.username.is_some() {
        sql.push_str(" AND username = ?");
    }
    if p.user_id.is_some() {
        sql.push_str(" AND user_id = ?");
    }
    if p.from.is_some() {
        sql.push_str(" AND created_at >= ?");
    }
    if p.to.is_some() {
        sql.push_str(" AND created_at <= ?");
    }
    let limit = p.limit.unwrap_or(100).clamp(1, 500);
    sql.push_str(&format!(" ORDER BY id DESC LIMIT {limit}"));

    let mut q = sqlx::query(&sql);
    if let Some(a) = &p.action {
        q = q.bind(a.clone());
    }
    if let Some(u) = &p.username {
        q = q.bind(u.clone());
    }
    if let Some(u) = &p.user_id {
        q = q.bind(u.clone());
    }
    if let Some(f) = p.from {
        q = q.bind(f);
    }
    if let Some(t) = p.to {
        q = q.bind(t);
    }

    let rows = q.fetch_all(&state.db).await?;
    Ok(Json(rows
        .into_iter()
        .map(|r: sqlx::sqlite::SqliteRow| AuditRow {
            id: r.get("id"),
            user_id: r.get("user_id"),
            username: r.get("username"),
            action: r.get("action"),
            resource: r.get("resource"),
            ip: r.get("ip"),
            detail: r.get("detail"),
            created_at: r.get("created_at"),
        })
        .collect()))
}
