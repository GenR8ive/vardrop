//! API modüllerini birleştiren router.

pub mod access;
pub mod audit;
pub mod clusters;
pub mod entries;
pub mod groups;
pub mod nav;
pub mod thumbs;
pub mod users;
pub mod values;

use crate::auth;
use crate::state::SharedState;
use axum::routing::{delete, get, post};
use axum::Router;

pub fn router() -> Router<SharedState> {
    Router::new()
        // auth & tokenlar
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/unlock", post(auth::unlock))
        .route("/api/auth/pin", get(auth::pin_status).put(auth::set_pin))
        .route("/api/auth/tokens", get(auth::list_tokens).post(auth::create_token))
        .route("/api/auth/tokens/{id}", delete(auth::delete_token))
        // kullanıcı yönetimi (admin)
        .merge(users::router())
        // RBAC: roller, departmanlar, atamalar
        .merge(access::router())
        // cluster / group
        .merge(clusters::router())
        .merge(groups::router())
        // popup boş-durum gezi ağacı
        .merge(nav::router())
        // entry + içerik + audit
        .merge(entries::router())
        .merge(values::router())
        .merge(thumbs::router())
        .merge(audit::router())
}
