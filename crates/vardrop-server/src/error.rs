//! API hata tipi — tüm handler'lar Result<_, ApiError> döner.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("yetkisiz: {0}")]
    Unauthorized(String),
    #[error("yasak: {0}")]
    Forbidden(String),
    #[error("PIN gerekli — secret okuma için POST /api/auth/unlock")]
    PinRequired,
    #[error("bulunamadı: {0}")]
    NotFound(String),
    #[error("çakışma: {0}")]
    Conflict(String),
    #[error("iç hata")]
    Internal(String),
}

impl ApiError {
    pub fn bad(msg: impl Into<String>) -> Self {
        ApiError::BadRequest(msg.into())
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        ApiError::Unauthorized(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        ApiError::Forbidden(msg.into())
    }

    /// Secret okuma PIN kapısı: istemci bu kodu görünce PIN ister.
    pub fn pin_required() -> Self {
        ApiError::PinRequired
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        ApiError::NotFound(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        ApiError::Conflict(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        ApiError::Internal(msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg, code) = match &self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone(), None),
            ApiError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone(), None),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone(), None),
            ApiError::PinRequired => (
                StatusCode::FORBIDDEN,
                "PIN gerekli — secret okuma için PIN girin".into(),
                Some("pin_required"),
            ),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone(), None),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m.clone(), None),
            ApiError::Internal(m) => {
                tracing::error!(hata = %m, "iç hata");
                (StatusCode::INTERNAL_SERVER_ERROR, m.clone(), None)
            }
        };
        let mut body = json!({ "error": msg });
        if let Some(c) = code {
            body["code"] = json!(c);
        }
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => ApiError::not_found("kayıt bulunamadı"),
            other => ApiError::internal(format!("db: {other}")),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::internal(e.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
