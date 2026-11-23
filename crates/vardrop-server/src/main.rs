//! vardrop-server — giriş noktası: `init` ve `serve` komutları.

mod api;
mod auth;
mod config;
mod crypto;
mod error;
mod images;
mod state;
mod storage;
mod util;

use anyhow::{Context as _, Result};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use state::{AppState, SharedState};
use std::net::SocketAddr;
use std::str::FromStr;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("serve");
    let arg = |name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        args.get(i + 1).cloned()
    };

    let config = resolve_config_arg(arg("--config"));
    match cmd {
        "init" => init(&config, arg("--admin-user")).await,
        "reset-password" => {
            // `reset-password [kullanıcı] [--config dosya]` — ilk pozisyonel arg kullanıcı adı.
            let user = args.iter().skip(1).find(|a| !a.starts_with('-')).cloned();
            reset_password(&config, user).await
        }
        "serve" => serve(&config).await,
        "version" | "--version" | "-V" => {
            println!("vardrop-server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("bilinmeyen komut: {other}");
            print_usage();
            Ok(())
        }
    }
}

fn print_usage() {
    eprintln!(
        "kullanım:\n  \
         vardrop-server init  [--config dosya] [--admin-user admin]\n  \
         vardrop-server reset-password [kullanıcı] [--config dosya]\n  \
         vardrop-server serve [--config dosya]\n  \
         vardrop-server version\n\n\
         config yolu: --config, yoksa VARDROP_CONFIG, yoksa ./config.toml"
    );
}

/// `--config` boş değilse onu, değilse `VARDROP_CONFIG` ortam değişkenini döner.
fn resolve_config_arg(explicit: Option<String>) -> String {
    if let Some(p) = explicit {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return p;
        }
    }
    std::env::var("VARDROP_CONFIG")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "config.toml".into())
}

fn default_config_template() -> &'static str {
    // Release arşivi `panel/` ile gelir; kaynak ağacında bu klasör yok.
    if std::path::Path::new("panel").is_dir() {
        include_str!("../../../config.host.example.toml")
    } else {
        include_str!("../../../config.example.toml")
    }
}

// ---------------- init ----------------

async fn init(cfg_path: &str, admin_user: Option<String>) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .try_init()
        .ok();

    let admin_user = admin_user.unwrap_or_else(|| "admin".into());
    if !std::path::Path::new(cfg_path).exists() {
        if let Some(parent) = std::path::Path::new(cfg_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| format!("config dizini: {parent:?}"))?;
            }
        }
        std::fs::write(cfg_path, default_config_template())
            .with_context(|| format!("config yazılamadı: {cfg_path}"))?;
        println!("[init] varsayılan config yazıldı: {cfg_path}");
    }
    let cfg = config::Config::load(Some(cfg_path))?;

    let data_dir = std::path::Path::new(&cfg.data.database)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&cfg.data.storage)?;

    let key = crypto::load_or_create_master_key(&cfg.security.master_key_file)?;
    let pool = open_db(&cfg).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    println!("[init] veritabanı hazır: {}", cfg.data.database);
    // İlk admin kullanıcısı (yoksa) — rastgele şifre üret, yalnızca bir kez göster.
    let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
        .fetch_one(&pool)
        .await?;
    if exists.0 == 0 {
        let password = random_password(16);
        let hash = auth::password::hash_password(&password)?;
        sqlx::query("INSERT INTO users (id, username, display_name, password_hash, role, is_active, created_at) VALUES (?,?,?,?,?,1,?)")
            .bind(vardrop_core::new_id("u"))
            .bind(&admin_user)
            .bind("Administrator")
            .bind(hash)
            .bind("admin")
            .bind(vardrop_core::now_ms())
            .execute(&pool)
            .await?;
        println!("[init] admin kullanıcısı yaratıldı: {admin_user}");
        println!("[init] ŞİFRE (yalnız bir kez gösterilir, hemen kaydedin): {password}");
        if let Ok(path) = std::env::var("VARDROP_ADMIN_PASSWORD_FILE") {
            let path = path.trim();
            if !path.is_empty() {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                std::fs::write(path, &password)
                    .with_context(|| format!("admin şifre dosyası yazılamadı: {path}"))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
                }
                println!("[init] şifre dosyası: {path} (okuyup silin)");
            }
        }
    } else {
        println!("[init] zaten admin mevcut — yeni kullanıcı oluşturulmadı");
    }
    if std::path::Path::new(&cfg.panel.dist).is_dir() {
        println!("[init] panel hazır: {}", cfg.panel.dist);
    } else {
        println!("[init] panel yok ({}) — kaynak ağacında: apps/admin-web içinde npm install && npm run build", cfg.panel.dist);
    }
    println!("[init] başlatma: vardrop-server serve --config {cfg_path}");
    let _ = key;
    Ok(())
}

// ---------------- reset-password ----------------

/// Şifresi kaybolan kullanıcının şifresini sıfırlar: yeni rastgele şifre üretir,
/// argon2 hash'ini günceller ve şifreyi yalnız bir kez gösterir.
async fn reset_password(cfg_path: &str, user: Option<String>) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .try_init()
        .ok();

    let username = user.unwrap_or_else(|| "admin".into());
    let cfg = config::Config::load(Some(cfg_path))?;
    if !std::path::Path::new(&cfg.data.database).exists() {
        anyhow::bail!("veritabanı yok: {} — önce `init` çalıştırın", cfg.data.database);
    }
    let pool = open_db(&cfg).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let password = random_password(16);
    let hash = auth::password::hash_password(&password)?;
    let res = sqlx::query("UPDATE users SET password_hash = ? WHERE username = ?")
        .bind(hash)
        .bind(&username)
        .execute(&pool)
        .await?;
    if res.rows_affected() == 0 {
        anyhow::bail!("kullanıcı yok: {username}");
    }

    println!("[reset] {username} şifresi güncellendi");
    println!("[reset] YENİ ŞİFRE (yalnız bir kez gösterilir, hemen kaydedin): {password}");
    write_password_file(&password)?;
    Ok(())
}

/// `VARDROP_ADMIN_PASSWORD_FILE` tanımlıysa şifreyi oraya yazar (okunup silinmelidir).
fn write_password_file(password: &str) -> Result<()> {
    if let Ok(path) = std::env::var("VARDROP_ADMIN_PASSWORD_FILE") {
        let path = path.trim();
        if !path.is_empty() {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(path, password)
                .with_context(|| format!("admin şifre dosyası yazılamadı: {path}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }
            println!("[init] şifre dosyası: {path} (okuyup silin)");
        }
    }
    Ok(())
}

fn random_password(len: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789!@#$%^&*-";
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..len).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
}

// ---------------- serve ----------------

async fn serve(cfg_path: &str) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .try_init()
        .ok();

    let cfg = config::Config::load(Some(cfg_path))?;
    tracing::info!("config: {cfg_path}");
    std::fs::create_dir_all(std_path_parent(&cfg.data.database))?;
    std::fs::create_dir_all(&cfg.data.storage)?;

    let key = crypto::load_or_create_master_key(&cfg.security.master_key_file)?;
    let pool = open_db(&cfg).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let state: SharedState = std::sync::Arc::new(AppState {
        db: pool,
        cfg: cfg.clone(),
        key,
        login_guard: tokio::sync::Mutex::new(Default::default()),
        unlocks: tokio::sync::Mutex::new(Default::default()),
    });

    let app = build_router(state, &cfg);
    let addr = cfg.server.bind.clone();
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind başarısız: {addr}"))?;
    tracing::info!("vardrop-server {} dinlemede ({addr})", env!("CARGO_PKG_VERSION"));
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

fn std_path_parent(p: &str) -> std::path::PathBuf {
    std::path::Path::new(p)
        .parent()
        .map(|x| x.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

async fn open_db(cfg: &config::Config) -> Result<sqlx::SqlitePool> {
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&cfg.sqlite_url())?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

fn build_router(state: SharedState, cfg: &config::Config) -> Router {
    let app = Router::new()
        .route("/api/health", get(health))
        .merge(api::router())
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let app = if cfg.server.cors_allow_all {
        app.layer(CorsLayer::permissive())
    } else {
        app
    };

    let dist = std::path::PathBuf::from(&cfg.panel.dist);
    if dist.is_dir() {
        let spa = ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html")));
        app.fallback_service(spa)
    } else {
        app.fallback(no_panel)
    }
}

async fn health() -> Response {
    (
        StatusCode::OK,
        axum::Json(vardrop_core::HealthResponse {
            status: "ok".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }),
    )
        .into_response()
}

async fn no_panel() -> Response {
    (
        StatusCode::NOT_FOUND,
        Html("<html><body style=\"font-family: sans-serif\"><h1>Registry API</h1><p>Panel bulunamadı. <code>panel.dist</code> yolunu kontrol edin. Kaynak ağacında: <code>apps/admin-web</code> içinde <code>npm install &amp;&amp; npm run build</code>.</p></body></html>"),
    )
        .into_response()
}
