//! Master key yönetimi + AES-256-GCM şifreleme (secret at-rest).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use rand::RngCore;

/// Master key dosyasından okur; yoksa üretir (32 byte hex).
pub fn load_or_create_master_key(path: &str) -> Result<[u8; 32]> {
    if let Ok(s) = std::fs::read_to_string(path) {
        let key = parse_key_file(s.trim())?;
        restrict_secret_file(path);
        return Ok(key);
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("dizin yaratılamadı: {parent:?}"))?;
    }
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    std::fs::write(path, hex::encode(key))
        .with_context(|| format!("master key yazılamadı: {path}"))?;
    restrict_secret_file(path);
    tracing::info!("yeni master key üretildi: {path}");
    Ok(key)
}

/// Unix'te secret dosyasını yalnız sahibi okusun (0600). Windows'ta profil ACL'i sınırdır.
fn restrict_secret_file(path: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn parse_key_file(s: &str) -> Result<[u8; 32]> {
    let trimmed = s.trim();
    if trimmed.len() != 64 {
        return Err(anyhow!("master key dosyası 64 hex karakter olmalı"));
    }
    let mut key = [0u8; 32];
    hex::decode_to_slice(trimmed, &mut key).map_err(|_| anyhow!("master key hex'i bozuk"))?;
    Ok(key)
}

/// Şifrele: çıkış = nonce(12) || ciphertext+tag.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    if plaintext.is_empty() {
        return Ok(Vec::new());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("aes init: {e}"))?;
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| anyhow!("şifreleme başarısız"))?;
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Şifreli blob'u çöz (nonce önde beklenir).
pub fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.is_empty() {
        return Ok(Vec::new());
    }
    if blob.len() < 13 {
        return Err(anyhow!("şifreli veri çok kısa"));
    }
    let (nonce_bytes, ct) = blob.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("aes: {e}"))?;
    cipher
        .decrypt(nonce, ct)
        .map_err(|_| anyhow!("şifre çözümlemesi başarısız (master key değişmiş olabilir)"))
}

pub fn random_token() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = [7u8; 32];
        let secret = b"SQL_CONNECTION_STRING";
        let enc = encrypt(&key, secret).unwrap();
        assert_ne!(enc, secret.to_vec());
        let dec = decrypt(&key, &enc).unwrap();
        assert_eq!(dec, secret.to_vec());
    }
}
