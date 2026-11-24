//! `GET /api/entries/{id}/thumbnail` — binary görseller için küçük JPEG.
//!
//! Üretilen thumbnail (şifresiz içerikte) storage'da sürüm bazlı cache'lenir;
//! secret içerikte cache yok ve okuma PIN/unlock kapısına tabidir.

use crate::api::entries::{fetch_current_version, fetch_entry};
use crate::auth::middleware::{require_cluster_perm, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::{images, storage, util::audit};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::Router;

pub fn router() -> Router<SharedState> {
    Router::new().route(
        "/api/entries/{id}/thumbnail",
        axum::routing::get(thumbnail),
    )
}

const THUMB_MAX: u32 = 160;

async fn thumbnail(
    user: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let entry = fetch_entry(&state.db, &id).await?;
    require_cluster_perm(&user, &entry.cluster_id, "entry.view")?;
    if entry.is_secret {
        // görsel secret'lar thumbnail üretmez (zaten maskeli satırda görünür)
        return Err(ApiError::not_found("bu entry görsel değil"));
    }
    if entry.kind != "binary" {
        return Err(ApiError::not_found("bu entry görsel değil"));
    }
    let vrow = fetch_current_version(&state.db, &entry).await?;
    let rel = vrow
        .blob_path
        .clone()
        .ok_or_else(|| ApiError::not_found("görsel içerik yok"))?;

    let cache_path = format!(
        "{}/{}/thumb-{}.jpg",
        state.cfg.data.storage, entry.id, entry.current_version
    );
    if !entry.is_secret {
        if let Ok(bytes) = tokio::fs::read(&cache_path).await {
            return Ok(jpeg_response(bytes));
        }
    }

    if entry.is_secret {
        crate::auth::require_secret_access(&state, &user).await?;
    }
    let bytes = storage::read_blob(&state, &rel, entry.is_secret).await?;
    if images::sniff_mime(&bytes).is_none() {
        return Err(ApiError::not_found("görsel değil"));
    }
    let thumb = images::make_thumbnail(&bytes, THUMB_MAX)?;

    if !entry.is_secret {
        if let Some(dir) = std::path::Path::new(&cache_path).parent() {
            if let Err(e) = tokio::fs::create_dir_all(dir).await {
                tracing::warn!("thumbnail dizini oluşturulamadı: {e}");
            }
        }
        if let Err(e) = tokio::fs::write(&cache_path, &thumb).await {
            tracing::warn!("thumbnail cache yazılamadı: {e}");
        }
    } else {
        audit(
            &state.db,
            Some(user.uid()),
            "secret.reveal",
            Some(&format!("{}/thumbnail", entry.name)),
            None,
            None,
            None,
        )
        .await;
    }

    Ok(jpeg_response(thumb))
}

fn jpeg_response(bytes: Vec<u8>) -> Response {
    Response::builder()
        .status(200)
        .header("content-type", "image/jpeg")
        .header("cache-control", "private, max-age=60")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))
        .expect("thumbnail yanıtı")
}
