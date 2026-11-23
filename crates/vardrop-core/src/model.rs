//! Wire.DTO'lar: SQL satırlarından üretilip API üzerinden taşınır.
//! Tüm zamanlar Unix epoch milisaniye (i64).

use serde::{Deserialize, Serialize};

// ---------- Kullanıcı & token ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub id: String,
    pub name: String,
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedToken {
    pub id: String,
    pub token: String,
}

// ---------- Cluster / Group ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub created_at: i64,
}

/// Concept ekranındaki kart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterCard {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub created_at: i64,
    pub groups_count: i64,
    pub entries_count: i64,
    pub top_groups: Vec<GroupChip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChip {
    pub id: String,
    pub name: String,
    pub entry_count: i64,
}

/// Sınırsız derinlikli grup ağacı düğümü.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupNode {
    pub id: String,
    pub cluster_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub entry_count: i64,
    pub children: Vec<GroupNode>,
}

/// Popup'ın boş-durum gezi görünümü: kluster + tam grup ağacı (alt ağaç sayıları dahil).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavCluster {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub entry_count: i64,
    pub roots: Vec<GroupNode>,
}

// ---------- Entry ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySummary {
    pub id: String,
    pub cluster_id: String,
    pub group_id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub is_secret: bool,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub current_version: i64,
    pub updated_at: i64,
    pub preview: String,
    /// binary + görsel içerik ise panel/client thumbnail gösterir.
    #[serde(default)]
    pub is_image: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub file_path: String,
    pub size: i64,
    pub sha256: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryDetail {
    pub id: String,
    pub cluster_id: String,
    pub group_id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub is_secret: bool,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub current_version: i64,
    pub created_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub files: Vec<FileInfo>,
    /// binary + görsel içerik ise panel/client thumbnail gösterir.
    #[serde(default)]
    pub is_image: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub created_at: i64,
    pub file_count: i64,
}

// ---------- Arama / audit / health ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<EntrySummary>,
    pub total: i64,
    pub took_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRow {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

