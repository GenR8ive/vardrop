//! Blob (dosya) depolama: storage root altında `{entry_id}/{sha256}.enc|.raw`.
//! Secret içerikler yazılmadan önce şifrelenir; hash plaintext üzerinden alınır.

use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;

fn entry_dir(state: &SharedState, entry_id: &str) -> String {
    format!("{}/{}", state.cfg.data.storage, entry_id)
}

/// Blob'u kaydeder; göreli yol, boyut ve hash döner.
pub async fn write_blob(
    state: &SharedState,
    entry_id: &str,
    data: &[u8],
    encrypted: bool,
) -> ApiResult<(String, i64, String)> {
    if data.is_empty() {
        return Err(ApiError::bad("boş dosya"));
    }
    let sha = vardrop_core::content::sha256_hex(data);
    let dir = entry_dir(state, entry_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ApiError::internal(format!("storage dizin hatası: {e}")))?;
    let (fname, bytes) = if encrypted {
        (
            format!("{sha}.enc"),
            crate::crypto::encrypt(&state.key, data)?,
        )
    } else {
        (sha.clone(), data.to_vec())
    };
    let rel = format!("{entry_id}/{fname}");
    let full = format!("{}/{}", state.cfg.data.storage, rel);
    tokio::fs::write(&full, bytes)
        .await
        .map_err(|e| ApiError::internal(format!("blob yazılamadı: {e}")))?;
    Ok((rel, data.len() as i64, sha))
}

/// Blob'u okur; şifreliyse çözerek döner.
pub async fn read_blob(state: &SharedState, rel: &str, encrypted: bool) -> ApiResult<Vec<u8>> {
    let full = format!("{}/{}", state.cfg.data.storage, rel);
    let bytes = tokio::fs::read(&full)
        .await
        .map_err(|e| ApiError::not_found(format!("blob okunamadı: {e}")))?;
    if encrypted {
        Ok(crate::crypto::decrypt(&state.key, &bytes)?)
    } else {
        Ok(bytes)
    }
}

/// Önizleme için ilk `cap` byte'ı okur (büyük dosyaları yüklemez).
pub async fn read_blob_prefix(
    state: &SharedState,
    rel: &str,
    encrypted: bool,
    cap: usize,
) -> ApiResult<Vec<u8>> {
    let full = format!("{}/{}", state.cfg.data.storage, rel);
    let mut bytes = tokio::fs::read(&full)
        .await
        .map_err(|e| ApiError::not_found(format!("blob okunamadı: {e}")))?;
    if encrypted && !bytes.is_empty() {
        bytes = crate::crypto::decrypt(&state.key, &bytes)?;
    }
    bytes.truncate(cap);
    Ok(bytes)
}

/// Entry silinince blob klasörünü temizler (artan dosyalar — sessizce).
pub async fn remove_entry_blobs(state: &SharedState, entry_id: &str) {
    let dir = entry_dir(state, entry_id);
    match tokio::fs::remove_dir_all(&dir).await {
        Ok(_) => {}
        // klasör hiç oluşmamışsa (dosyasız entry) sessiz geç
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!("blob klasörü silinemedi: {dir} ({e})"),
    }
}
