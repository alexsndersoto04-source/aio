//! Image processing (`std::image::*`) powered by the `image` crate.
//!
//! Real, non-simulated codecs for PNG, JPEG, WebP, BMP and GIF. Every helper
//! operates either on file paths (convenience) or on raw bytes (for pipelines
//! that already have the image in memory — e.g. downloaded via
//! `std::http_full` or read via `std::io`).
//!
//! Images are passed across the VM boundary as an opaque `i64` handle backed
//! by a process-wide registry, so `.titan` code can keep several images alive
//! at once without leaking Rust types into the language surface.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use image::{DynamicImage, ImageFormat, imageops::FilterType};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("image I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("unknown format '{0}' (try png, jpeg, webp, bmp, gif)")]
    UnknownFormat(String),
    #[error("unknown filter '{0}' (try nearest, triangle, catmullrom, gaussian, lanczos3)")]
    UnknownFilter(String),
    #[error("no image registered under handle {0}")]
    UnknownHandle(i64),
}

struct Registry { images: HashMap<i64, DynamicImage>, next_id: i64 }

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { images: HashMap::new(), next_id: 1 }))
}

fn insert(image: DynamicImage) -> i64 {
    let mut reg = registry().lock().expect("image registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.images.insert(id, image);
    id
}

fn with_image<F, R>(handle: i64, action: F) -> Result<R, ImageError>
where F: FnOnce(&DynamicImage) -> R {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?;
    Ok(action(image))
}

fn parse_format(name: &str) -> Result<ImageFormat, ImageError> {
    match name.to_ascii_lowercase().as_str() {
        "png"          => Ok(ImageFormat::Png),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        "webp"         => Ok(ImageFormat::WebP),
        "bmp"          => Ok(ImageFormat::Bmp),
        "gif"          => Ok(ImageFormat::Gif),
        other => Err(ImageError::UnknownFormat(other.into())),
    }
}

fn parse_filter(name: &str) -> Result<FilterType, ImageError> {
    match name.to_ascii_lowercase().as_str() {
        "nearest"    => Ok(FilterType::Nearest),
        "triangle"   => Ok(FilterType::Triangle),
        "catmullrom" => Ok(FilterType::CatmullRom),
        "gaussian"   => Ok(FilterType::Gaussian),
        "lanczos3"   => Ok(FilterType::Lanczos3),
        other => Err(ImageError::UnknownFilter(other.into())),
    }
}

// ---------------- Load / save ------------------------------------------

/// Load an image from a file path. Returns an opaque handle.
pub fn load(path: &str) -> Result<i64, ImageError> {
    Ok(insert(image::open(path)?))
}

/// Load an image from raw bytes. Returns an opaque handle.
pub fn load_bytes(bytes: &[u8]) -> Result<i64, ImageError> {
    Ok(insert(image::load_from_memory(bytes)?))
}

/// Save an image to a file. The format is inferred from the extension.
pub fn save(handle: i64, path: &str) -> Result<(), ImageError> {
    let path = path.to_string();
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?;
    image.save(Path::new(&path))?;
    Ok(())
}

/// Encode an image into bytes using an explicit format ("png", "jpeg", ...).
pub fn encode(handle: i64, format: &str) -> Result<Vec<u8>, ImageError> {
    let format = parse_format(format)?;
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?;
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, format)?;
    Ok(buffer.into_inner())
}

// ---------------- Metadata --------------------------------------------

pub fn width(handle: i64) -> Result<u32, ImageError> {
    with_image(handle, |image| image.width())
}
pub fn height(handle: i64) -> Result<u32, ImageError> {
    with_image(handle, |image| image.height())
}

/// Returns a short colour description: "L8", "La8", "Rgb8", "Rgba8", "Rgb16", ...
pub fn color_type(handle: i64) -> Result<String, ImageError> {
    with_image(handle, |image| format!("{:?}", image.color()))
}

// ---------------- Transforms (return new handles) ---------------------

pub fn resize(handle: i64, width: u32, height: u32, filter: &str) -> Result<i64, ImageError> {
    let filter = parse_filter(filter)?;
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.resize(width, height, filter)))
}

pub fn resize_exact(handle: i64, width: u32, height: u32, filter: &str) -> Result<i64, ImageError> {
    let filter = parse_filter(filter)?;
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.resize_exact(width, height, filter)))
}

pub fn thumbnail(handle: i64, width: u32, height: u32) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.thumbnail(width, height)))
}

pub fn crop(handle: i64, x: u32, y: u32, width: u32, height: u32) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let mut image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.crop(x, y, width, height)))
}

pub fn grayscale(handle: i64) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.grayscale()))
}

pub fn blur(handle: i64, sigma: f32) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.blur(sigma)))
}

pub fn brighten(handle: i64, value: i32) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.brighten(value)))
}

pub fn rotate90(handle: i64) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.rotate90()))
}

pub fn rotate180(handle: i64) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.rotate180()))
}

pub fn rotate270(handle: i64) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.rotate270()))
}

pub fn flip_horizontal(handle: i64) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.fliph()))
}

pub fn flip_vertical(handle: i64) -> Result<i64, ImageError> {
    let reg = registry().lock().expect("image registry poisoned");
    let image = reg.images.get(&handle).ok_or(ImageError::UnknownHandle(handle))?.clone();
    drop(reg);
    Ok(insert(image.flipv()))
}

/// Free the registry slot associated with `handle`. Idempotent.
pub fn close(handle: i64) {
    if let Ok(mut reg) = registry().lock() { reg.images.remove(&handle); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn make_test_image_bytes() -> Vec<u8> {
        // 4x4 red image.
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(4, 4, |_, _| Rgb([255, 0, 0]));
        let mut buffer = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img).write_to(&mut buffer, ImageFormat::Png).unwrap();
        buffer.into_inner()
    }

    #[test]
    fn load_bytes_reads_size_and_color() {
        let handle = load_bytes(&make_test_image_bytes()).unwrap();
        assert_eq!(width(handle).unwrap(), 4);
        assert_eq!(height(handle).unwrap(), 4);
        assert!(color_type(handle).unwrap().contains("Rgb"));
        close(handle);
    }

    #[test]
    fn resize_produces_new_handle_and_size() {
        let original = load_bytes(&make_test_image_bytes()).unwrap();
        let resized = resize_exact(original, 8, 8, "nearest").unwrap();
        assert_ne!(original, resized);
        assert_eq!(width(resized).unwrap(), 8);
        assert_eq!(height(resized).unwrap(), 8);
        close(original); close(resized);
    }

    #[test]
    fn transform_pipeline_round_trip() {
        let a = load_bytes(&make_test_image_bytes()).unwrap();
        let b = grayscale(a).unwrap();
        let c = brighten(b, 10).unwrap();
        let d = rotate90(c).unwrap();
        let e = flip_horizontal(d).unwrap();
        // Every step yields a distinct handle.
        for handle in [a, b, c, d, e] { assert!(width(handle).is_ok()); }
        for handle in [a, b, c, d, e] { close(handle); }
    }

    #[test]
    fn encode_round_trip_bytes() {
        let a = load_bytes(&make_test_image_bytes()).unwrap();
        let png = encode(a, "png").unwrap();
        assert!(png.len() > 8 && png.starts_with(b"\x89PNG"));
        let b = load_bytes(&png).unwrap();
        assert_eq!(width(b).unwrap(), 4);
        close(a); close(b);
    }

    #[test]
    fn parses_named_formats_and_filters() {
        assert!(matches!(parse_format("PNG").unwrap(), ImageFormat::Png));
        assert!(matches!(parse_format("jpg").unwrap(), ImageFormat::Jpeg));
        assert!(matches!(parse_filter("Lanczos3").unwrap(), FilterType::Lanczos3));
        assert!(parse_format("banana").is_err());
        assert!(parse_filter("magic").is_err());
    }

    #[test]
    fn unknown_handle_is_reported() {
        assert!(width(9_999_999).is_err());
    }
}
