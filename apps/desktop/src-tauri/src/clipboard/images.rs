use std::path::{Path, PathBuf};

const THUMB_MAX: u32 = 160;

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let expected = (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(4);
    if rgba.len() as u64 != expected {
        return Err(format!(
            "invalid RGBA buffer: got {} bytes, expected {expected} for {width}x{height}",
            rgba.len()
        ));
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out)
}

pub fn save_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    std::fs::write(path, encode_png(width, height, rgba)?).map_err(|e| e.to_string())
}

pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("PNG decode failed: {e}"))?;
    let rgba = img.to_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// Load a stored blob (PNG, or legacy raw RGBA if dimensions are known).
pub fn load_image_blob(path: &Path, dimensions: Option<(u32, u32)>) -> Result<DecodedImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read image blob: {e}"))?;
    if bytes.starts_with(b"\x89PNG") {
        return decode_png(&bytes);
    }
    if let Some((width, height)) = dimensions {
        let expected = (width as u64)
            .saturating_mul(height as u64)
            .saturating_mul(4);
        if bytes.len() as u64 == expected {
            return Ok(DecodedImage {
                width,
                height,
                rgba: bytes,
            });
        }
    }
    Err("Unsupported image format. Copy the image again to refresh this entry.".into())
}

pub fn parse_dimensions_label(label: &str) -> Option<(u32, u32)> {
    let normalized = label.replace('×', "x").replace('X', "x");
    let (w, h) = normalized.split_once('x')?;
    let width: u32 = w.trim().parse().ok()?;
    let height: u32 = h.trim().parse().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

pub fn make_thumbnail_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| "invalid RGBA buffer for thumbnail".to_string())?;
    let thumb = image::imageops::thumbnail(&img, THUMB_MAX, THUMB_MAX);
    encode_png(thumb.width(), thumb.height(), thumb.as_raw())
}

pub fn dimensions_label(width: u32, height: u32) -> String {
    format!("{width}×{height}")
}

pub fn image_filename_title(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Image")
        .to_string()
}

pub fn clipboard_image_title(text: Option<&str>, width: u32, height: u32) -> String {
    if let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) {
        if let Some(name) = image_name_from_clipboard_text(t) {
            return name;
        }
    }
    format!("Image {}", dimensions_label(width, height))
}

/// Filename from clipboard text (bare name or file path / file URL).
pub fn image_name_from_clipboard_text(text: &str) -> Option<String> {
    let path = parse_image_path(text)?;
    Some(image_filename_title(&path))
}

pub fn parse_image_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }
    let path_str = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    let path = PathBuf::from(path_str);
    if !is_image_path(&path) {
        return None;
    }
    Some(path)
}

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic"
            )
        })
        .unwrap_or(false)
}

pub fn load_image_file(path: &Path) -> Result<DecodedImage, String> {
    if !path.is_file() {
        return Err(format!("image file not found: {}", path.display()));
    }
    let img = image::open(path).map_err(|e| format!("open image: {e}"))?;
    let rgba = img.to_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

pub fn try_load_image_from_clipboard_text(text: &str) -> Option<DecodedImage> {
    let path = parse_image_path(text)?;
    load_image_file(&path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_roundtrip_preserves_pixels() {
        let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 128, 128, 128, 255];
        let png = encode_png(2, 2, &rgba).expect("encode");
        let decoded = decode_png(&png).expect("decode");
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.rgba, rgba);
    }

    #[test]
    fn parse_dimensions_label_accepts_multiply_sign() {
        assert_eq!(parse_dimensions_label("1920×1080"), Some((1920, 1080)));
    }
}
