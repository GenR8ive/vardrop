//! Content format katmanı: içerik türleri, önizleme ve maskeleme.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Text,
    Json,
    Yaml,
    Binary,
    Bundle,
}

impl ContentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentKind::Text => "text",
            ContentKind::Json => "json",
            ContentKind::Yaml => "yaml",
            ContentKind::Binary => "binary",
            ContentKind::Bundle => "bundle",
        }
    }

    /// Panel rozetlerinde görünen ad.
    pub fn label(&self) -> &'static str {
        match self {
            ContentKind::Text => "Text",
            ContentKind::Json => "JSON",
            ContentKind::Yaml => "YAML",
            ContentKind::Binary => "Binary",
            ContentKind::Bundle => "Bundle",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "text" => ContentKind::Text,
            "json" => ContentKind::Json,
            "yaml" => ContentKind::Yaml,
            "binary" => ContentKind::Binary,
            "bundle" => ContentKind::Bundle,
            _ => return None,
        })
    }

    /// Dosya tabanlı türler (binary/bundle) value yerine dosyalarla yaşar.
    pub fn is_file_based(&self) -> bool {
        matches!(self, ContentKind::Binary | ContentKind::Bundle)
    }
}

/// JSON içeriği saklamadan önce doğrula.
pub fn validate_json(text: &str) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(text)
        .map(|_| ())
        .map_err(|e| format!("geçersiz JSON: {e}"))
}

/// Secret satırlarında gösterilen maske.
pub const SECRET_MASK: &str = "••••••••";

/// Secret önizleme: maske (+ sade admin görünümünde son 4 karakter).
pub fn mask_preview(last4: Option<&str>) -> String {
    match last4 {
        Some(l4) => format!("{SECRET_MASK}{l4}"),
        None => SECRET_MASK.to_string(),
    }
}

/// Düz metin önizleme: boşlukları sadeleştirip ilk `max` karakter.
pub fn text_preview(content: &str, max: usize) -> String {
    let cleaned = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= max {
        cleaned
    } else {
        let cut: String = cleaned.chars().take(max).collect();
        format!("{cut}…")
    }
}

 pub fn human_size(size: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let s = size as f64;
    if size < 1024 {
        format!("{size} B")
    } else if s < MB {
        format!("{:.1} KB", s / KB)
    } else if s < GB {
        format!("{:.1} MB", s / MB)
    } else {
        format!("{:.2} GB", s / GB)
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_parse() {
        assert_eq!(ContentKind::parse("json"), Some(ContentKind::Json));
        assert_eq!(ContentKind::parse("x"), None);
    }

    #[test]
    fn preview() {
        assert_eq!(text_preview("a\n  b   c", 10), "a b c");
        assert_eq!(mask_preview(Some("7f2a")), "••••••••7f2a");
    }

    #[test]
    fn json_ok() {
        assert!(validate_json("{}").is_ok());
        assert!(validate_json("nope").is_err());
    }
}
