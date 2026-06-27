//! Image normalization shared across the worker and API.
//!
//! Several downstream consumers (vision models, img2vid, face-swap) choke on formats
//! like WebP. We decode whatever was uploaded and re-encode to PNG before handing it on.

/// Decode any supported image (png/jpeg/webp/gif) and re-encode as PNG.
/// Returns `None` if the bytes can't be decoded as an image.
pub fn to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some(buf.into_inner())
}

/// Normalize to PNG, falling back to the original bytes if decoding fails
/// (so formats our build can't decode still reach a consumer that might handle them).
pub fn to_png_or_original(bytes: Vec<u8>) -> Vec<u8> {
    match to_png(&bytes) {
        Some(png) => png,
        None => bytes,
    }
}
