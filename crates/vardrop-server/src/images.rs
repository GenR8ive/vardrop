//! Görsel algılama + thumbnail üretimi.
//!
//! `binary` entry'lerin içeriği upload sırasında değişmez; türü magic byte'tan
//! anlık sniff edilir. Thumbnail isteğe bağlı üretilip storage'a cache'lenir
//! (`{entry}/{thumb-version}.jpg`) — şifreli içerikte cache kullanılmaz.

use crate::api::entries::{EntryRow, VersionRow};
use crate::error::{ApiError, ApiResult};
use crate::state::SharedState;
use crate::storage;

/// Desteklenen raster biçimleri (SVG bilinçli olarak dışarıda: raster decode yok).
pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"BM") {
        return Some("image/bmp");
    }
    None
}

/// Entry binary + görsel ise mime döner. Secret içerik asla işaretlenmez.
pub async fn image_mime_of_blob(
    state: &SharedState,
    entry: &EntryRow,
    vrow: Option<&VersionRow>,
) -> Option<&'static str> {
    if entry.kind != "binary" || entry.is_secret {
        return None;
    }
    let rel = vrow.and_then(|v| v.blob_path.as_deref())?;
    let head = storage::read_blob_head(state, rel, 32).await.ok()?;
    sniff_mime(&head)
}

/// Görseli en fazla `max` piksele sığdırıp JPEG'e çevirir.
pub fn make_thumbnail(bytes: &[u8], max: u32) -> ApiResult<Vec<u8>> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| ApiError::bad(format!("görsel çözülemedi: {e}")))?;
    let thumb = img.thumbnail(max, max).to_rgb8();
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82);
    enc.encode_image(&thumb)
        .map_err(|e| ApiError::internal(format!("thumbnail kodlanamadı: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_sniff() {
        let png = b"\x89PNG\r\n\x1a\n....";
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0];
        let webp = b"RIFF\x00\x00\x00\x00WEBP";
        assert_eq!(sniff_mime(png), Some("image/png"));
        assert_eq!(sniff_mime(&jpg), Some("image/jpeg"));
        assert_eq!(sniff_mime(webp), Some("image/webp"));
        assert_eq!(sniff_mime(b"BM\x00\x00"), Some("image/bmp"));
        assert_eq!(sniff_mime(b"merhaba dunya"), None);
        // kısa başlık — dizin taşmasın
        assert_eq!(sniff_mime(&[0xFF, 0xD8]), None);
    }

    #[test]
    fn thumb_png_sigar() {
        use image::{GenericImageView, ImageEncoder};
        // 200x100 düz PNG → en uzun kenarı 160'a inen JPEG
        let img = image::RgbImage::new(200, 100);
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(img.as_raw(), 200, 100, image::ExtendedColorType::Rgb8)
            .expect("png kodlanamadı");
        let thumb = make_thumbnail(&png, 160).expect("thumbnail");
        let decoded = image::load_from_memory(&thumb).expect("jpeg decode");
        assert_eq!(decoded.dimensions(), (160, 80));
    }
}
