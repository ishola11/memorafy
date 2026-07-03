use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub enum CapturedContent {
    Text(String),
    Url(String),
    Code(String),
    #[allow(dead_code)]
    Image {
        path: String,
        size: i64,
        thumbnail_path: Option<String>,
    },
}

pub fn classify(text: &str) -> (&'static str, CapturedContent) {
    let trimmed = text.trim();
    if crate::search::is_url(trimmed) {
        return ("url", CapturedContent::Url(trimmed.to_string()));
    }
    if crate::search::looks_like_code(trimmed) {
        return ("code", CapturedContent::Code(trimmed.to_string()));
    }
    ("text", CapturedContent::Text(trimmed.to_string()))
}

pub fn hash_content(content_type: &str, text: Option<&str>, blob_path: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content_type.as_bytes());
    if let Some(t) = text {
        hasher.update(t.as_bytes());
    }
    if let Some(p) = blob_path {
        hasher.update(p.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Content hash for clipboard images. Hashes the actual pixel bytes — two
/// different screenshots at the same resolution must not dedupe as one.
pub fn hash_image(width: usize, height: usize, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"image");
    hasher.update(width.to_le_bytes());
    hasher.update(height.to_le_bytes());
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::hash_image;

    #[test]
    fn same_dimensions_different_pixels_hash_differently() {
        let a = hash_image(2, 2, &[0, 0, 0, 255]);
        let b = hash_image(2, 2, &[255, 255, 255, 255]);
        assert_ne!(a, b);
    }

    #[test]
    fn identical_images_hash_identically() {
        let a = hash_image(2, 2, &[1, 2, 3, 4]);
        let b = hash_image(2, 2, &[1, 2, 3, 4]);
        assert_eq!(a, b);
    }
}
