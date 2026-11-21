//! Konfigürasyon yükleyici (config.toml, yoksa varsayılanlar).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerCfg,
    pub data: DataCfg,
    pub security: SecurityCfg,
    pub panel: PanelCfg,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerCfg {
    pub bind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
    pub cors_allow_all: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DataCfg {
    pub database: String,
    pub storage: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SecurityCfg {
    pub master_key_file: String,
    pub secret_max_inline_kb: u32,
    pub password_min_len: usize,
    pub session_ttl_hours: u64,
    pub token_default_ttl_days: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PanelCfg {
    pub dist: String,
}

impl Default for ServerCfg {
    fn default() -> Self {
        ServerCfg {
            bind: "127.0.0.1:8080".into(),
            public_url: None,
            cors_allow_all: false,
        }
    }
}

impl Default for DataCfg {
    fn default() -> Self {
        DataCfg {
            database: "data/registry.db".into(),
            storage: "data/storage".into(),
        }
    }
}

impl Default for SecurityCfg {
    fn default() -> Self {
        SecurityCfg {
            master_key_file: "data/master.key".into(),
            secret_max_inline_kb: 16,
            password_min_len: 10,
            session_ttl_hours: 12,
            token_default_ttl_days: 90,
        }
    }
}

impl Default for PanelCfg {
    fn default() -> Self {
        PanelCfg {
            dist: "apps/admin-web/dist".into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerCfg::default(),
            data: DataCfg::default(),
            security: SecurityCfg::default(),
            panel: PanelCfg::default(),
        }
    }
}

impl Config {
    /// `path` verilen konfigürasyonu okur; dosya yoksa varsayılanlarla gider.
    pub fn load(path: Option<&str>) -> anyhow::Result<Config> {
        let candidate = path.unwrap_or("config.toml");
        let raw = std::fs::read_to_string(candidate).ok();
        match raw {
            Some(s) => {
                let cfg: Config = toml::from_str(&s)?;
                Ok(cfg)
            }
            None => Ok(Config::default()),
        }
    }

    pub fn sqlite_url(&self) -> String {
        let db = &self.data.database;
        if db.starts_with("sqlite://") {
            db.clone()
        } else {
            format!("sqlite://{db}?mode=rwc")
        }
    }
}
