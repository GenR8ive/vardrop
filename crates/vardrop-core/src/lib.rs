//! Vardrop — ortak tipler ve yardımcılar (server + client kullanır).

pub mod content;
pub mod model;
pub mod security;

pub use content::ContentKind;
pub use model::*;
pub use security::{Role, Scope};

/// Yeni kayıt kimliği: `<prefix>_<32 hex>` (uuid v4).
pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().simple())
}

/// Şu anki zaman: Unix epoch milisaniye.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
