//! Uygulama durumu: pool, config, master key, login kilitleme.

use crate::config::Config;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub cfg: Config,
    pub key: [u8; 32],
    /// IP -> login deneme durumu (bruteforce koruması).
    pub login_guard: Mutex<HashMap<String, Guard>>,
    /// "{uid}:{token_id}" -> secret unlock penceresi bitişi (PIN akışı).
    pub unlocks: Mutex<HashMap<String, Instant>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Guard {
    pub fails: u32,
    pub locked_until: Option<Instant>,
}

pub type SharedState = Arc<AppState>;
