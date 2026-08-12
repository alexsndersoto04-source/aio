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
use std::io::{BufRead, Cursor, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use image::{imageops::FilterType, DynamicImage, ImageFormat, ImageReader, Limits};
use thiserror::Error;

const MAX_IMAGE_HANDLES: usize = 64;
const MAX_IMAGE_DIMENSION: u32 = 8_192;
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_RUNTIME_IMAGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENCODED_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENCODED_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_PATH_BYTES: usize = 16 * 1024;
const MAX_BLUR_SIGMA: f32 = 100.0;
const MAX_CONCURRENT_IMAGE_OPERATIONS: usize = 4;
const MAX_TRANSIENT_IMAGE_BYTES: usize = 128 * 1024 * 1024;

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
    #[error("invalid RGBA buffer: {0}")]
    BadBuffer(String),
    #[error("no image registered under handle {0}")]
    UnknownHandle(i64),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("invalid image dimensions or transform parameters: {0}")]
    InvalidParameter(&'static str),
    #[error("image handle space exhausted")]
    HandleSpaceExhausted,
}

#[derive(Default)]
struct ImageOperationUsage {
    operations: usize,
    bytes: usize,
}

struct ImageOperationPermit {
    runtime_id: u64,
    bytes: usize,
}

impl Drop for ImageOperationPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(operation_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.operations = runtime.operations.saturating_sub(1);
            runtime.bytes = runtime.bytes.saturating_sub(self.bytes);
            if runtime.operations == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn operation_usage() -> &'static Mutex<HashMap<u64, ImageOperationUsage>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, ImageOperationUsage>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reserve_operation(bytes: usize) -> Result<ImageOperationPermit, ImageError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(operation_usage());
    let (operations, used_bytes) = usage
        .get(&runtime_id)
        .map_or((0, 0), |runtime| (runtime.operations, runtime.bytes));
    if operations >= MAX_CONCURRENT_IMAGE_OPERATIONS {
        return Err(ImageError::ResourceLimit {
            resource: "concurrent image operations",
            limit: MAX_CONCURRENT_IMAGE_OPERATIONS,
        });
    }
    if used_bytes.saturating_add(bytes) > MAX_TRANSIENT_IMAGE_BYTES {
        return Err(ImageError::ResourceLimit {
            resource: "transient image operation bytes",
            limit: MAX_TRANSIENT_IMAGE_BYTES,
        });
    }
    let runtime = usage.entry(runtime_id).or_default();
    runtime.operations += 1;
    runtime.bytes += bytes;
    Ok(ImageOperationPermit { runtime_id, bytes })
}

struct ImageEntry {
    image: Arc<DynamicImage>,
    bytes: usize,
}

struct Registry {
    images: HashMap<(u64, i64), ImageEntry>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            images: HashMap::new(),
            next_id: 1,
        })
    })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ImageError> {
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(ImageError::InvalidParameter(
            "width and height must be positive and within the dimension limit",
        ));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), ImageError> {
    if path.len() > MAX_IMAGE_PATH_BYTES {
        return Err(ImageError::ResourceLimit {
            resource: "image path bytes",
            limit: MAX_IMAGE_PATH_BYTES,
        });
    }
    Ok(())
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_BYTES as u64);
    limits
}

fn decode_reader<R: BufRead + Seek>(
    mut reader: ImageReader<R>,
) -> Result<DynamicImage, ImageError> {
    reader.limits(decode_limits());
    Ok(reader.decode()?)
}

fn validate_registry_capacity(
    active: usize,
    runtime_bytes: usize,
    image_bytes: usize,
) -> Result<(), ImageError> {
    if active >= MAX_IMAGE_HANDLES {
        return Err(ImageError::ResourceLimit {
            resource: "image handles",
            limit: MAX_IMAGE_HANDLES,
        });
    }
    if runtime_bytes.saturating_add(image_bytes) > MAX_RUNTIME_IMAGE_BYTES {
        return Err(ImageError::ResourceLimit {
            resource: "runtime decoded image bytes",
            limit: MAX_RUNTIME_IMAGE_BYTES,
        });
    }
    Ok(())
}

fn insert(image: DynamicImage) -> Result<i64, ImageError> {
    validate_dimensions(image.width(), image.height())?;
    let bytes = image.as_bytes().len();
    if bytes > MAX_IMAGE_BYTES {
        return Err(ImageError::ResourceLimit {
            resource: "decoded image bytes",
            limit: MAX_IMAGE_BYTES,
        });
    }
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = registry
        .images
        .keys()
        .filter(|(owner, _)| *owner == runtime_id)
        .count();
    let runtime_bytes = registry
        .images
        .iter()
        .filter(|((owner, _), _)| *owner == runtime_id)
        .try_fold(0usize, |total, (_, entry)| total.checked_add(entry.bytes))
        .unwrap_or(usize::MAX);
    validate_registry_capacity(active, runtime_bytes, bytes)?;
    let id = registry.next_id;
    registry.next_id = id.checked_add(1).ok_or(ImageError::HandleSpaceExhausted)?;
    registry.images.insert(
        (runtime_id, id),
        ImageEntry {
            image: Arc::new(image),
            bytes,
        },
    );
    Ok(id)
}

fn get_image(handle: i64) -> Result<Arc<DynamicImage>, ImageError> {
    crate::native::lock_recover(registry())
        .images
        .get(&handle_key(handle))
        .map(|entry| Arc::clone(&entry.image))
        .ok_or(ImageError::UnknownHandle(handle))
}

fn with_image<F, R>(handle: i64, action: F) -> Result<R, ImageError>
where
    F: FnOnce(&DynamicImage) -> R,
{
    let image = get_image(handle)?;
    Ok(action(&image))
}

fn parse_format(name: &str) -> Result<ImageFormat, ImageError> {
    match name.to_ascii_lowercase().as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        "webp" => Ok(ImageFormat::WebP),
        "bmp" => Ok(ImageFormat::Bmp),
        "gif" => Ok(ImageFormat::Gif),
        other => Err(ImageError::UnknownFormat(other.into())),
    }
}

struct BoundedCursor {
    inner: Cursor<Vec<u8>>,
    limit: usize,
}

impl BoundedCursor {
    fn new(limit: usize) -> Self {
        Self {
            inner: Cursor::new(Vec::new()),
            limit,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.inner.into_inner()
    }
}

impl Write for BoundedCursor {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let end = self
            .inner
            .position()
            .checked_add(buffer.len() as u64)
            .ok_or_else(|| std::io::Error::other("encoded image position overflow"))?;
        if end > self.limit as u64 {
            return Err(std::io::Error::other(format!(
                "encoded image exceeds byte limit {}",
                self.limit
            )));
        }
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Seek for BoundedCursor {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let position = self.inner.seek(position)?;
        if position > self.limit as u64 {
            return Err(std::io::Error::other(format!(
                "encoded image seek exceeds byte limit {}",
                self.limit
            )));
        }
        Ok(position)
    }
}

fn parse_filter(name: &str) -> Result<FilterType, ImageError> {
    match name.to_ascii_lowercase().as_str() {
        "nearest" => Ok(FilterType::Nearest),
        "triangle" => Ok(FilterType::Triangle),
        "catmullrom" => Ok(FilterType::CatmullRom),
        "gaussian" => Ok(FilterType::Gaussian),
        "lanczos3" => Ok(FilterType::Lanczos3),
        other => Err(ImageError::UnknownFilter(other.into())),
    }
}

// ---------------- Load / save ------------------------------------------

/// Load an image from a file path. Returns an opaque handle.
pub fn load(path: &str) -> Result<i64, ImageError> {
    validate_path(path)?;
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_ENCODED_INPUT_BYTES as u64 {
        return Err(ImageError::ResourceLimit {
            resource: "encoded image input bytes",
            limit: MAX_ENCODED_INPUT_BYTES,
        });
    }
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    insert(decode_reader(reader)?)
}

/// Load an image from raw bytes. Returns an opaque handle.
pub fn load_bytes(bytes: &[u8]) -> Result<i64, ImageError> {
    if bytes.len() > MAX_ENCODED_INPUT_BYTES {
        return Err(ImageError::ResourceLimit {
            resource: "encoded image input bytes",
            limit: MAX_ENCODED_INPUT_BYTES,
        });
    }
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    insert(decode_reader(reader)?)
}

/// Register a raw RGBA8 buffer as an image (Fase 2: the software GUI
/// rasterizer `std::gui::render` produces exactly this format) and
/// return its handle. `rgba.len()` must equal `width * height * 4`.
pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<i64, ImageError> {
    validate_dimensions(width, height)?;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ImageError::InvalidParameter("RGBA dimensions overflow"))?;
    if expected > MAX_IMAGE_BYTES {
        return Err(ImageError::ResourceLimit {
            resource: "decoded image bytes",
            limit: MAX_IMAGE_BYTES,
        });
    }
    let _permit = reserve_operation(expected)?;
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
        ImageError::BadBuffer(format!(
            "{width}x{height} needs {expected} bytes, got {}",
            rgba.len()
        ))
    })?;
    insert(DynamicImage::ImageRgba8(image))
}

/// Save an image to a file. The format is inferred from the extension.
pub fn save(handle: i64, path: &str) -> Result<(), ImageError> {
    validate_path(path)?;
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    image.save(Path::new(path))?;
    Ok(())
}

/// Encode an image into bytes using an explicit format ("png", "jpeg", ...).
pub fn encode(handle: i64, format: &str) -> Result<Vec<u8>, ImageError> {
    let format = parse_format(format)?;
    let _permit = reserve_operation(MAX_ENCODED_OUTPUT_BYTES)?;
    let image = get_image(handle)?;
    let mut buffer = BoundedCursor::new(MAX_ENCODED_OUTPUT_BYTES);
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
    validate_dimensions(width, height)?;
    let filter = parse_filter(filter)?;
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.resize(width, height, filter))
}

pub fn resize_exact(handle: i64, width: u32, height: u32, filter: &str) -> Result<i64, ImageError> {
    validate_dimensions(width, height)?;
    let filter = parse_filter(filter)?;
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.resize_exact(width, height, filter))
}

pub fn thumbnail(handle: i64, width: u32, height: u32) -> Result<i64, ImageError> {
    validate_dimensions(width, height)?;
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.thumbnail(width, height))
}

pub fn crop(handle: i64, x: u32, y: u32, width: u32, height: u32) -> Result<i64, ImageError> {
    validate_dimensions(width, height)?;
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    let right = x
        .checked_add(width)
        .ok_or(ImageError::InvalidParameter("crop coordinates overflow"))?;
    let bottom = y
        .checked_add(height)
        .ok_or(ImageError::InvalidParameter("crop coordinates overflow"))?;
    if right > image.width() || bottom > image.height() {
        return Err(ImageError::InvalidParameter(
            "crop is outside the source image",
        ));
    }
    insert(image.crop_imm(x, y, width, height))
}

pub fn grayscale(handle: i64) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.grayscale())
}

pub fn blur(handle: i64, sigma: f32) -> Result<i64, ImageError> {
    if !sigma.is_finite() || !(0.0..=MAX_BLUR_SIGMA).contains(&sigma) {
        return Err(ImageError::InvalidParameter(
            "blur sigma is outside the supported range",
        ));
    }
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.blur(sigma))
}

pub fn brighten(handle: i64, value: i32) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.brighten(value))
}

pub fn rotate90(handle: i64) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.rotate90())
}

pub fn rotate180(handle: i64) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.rotate180())
}

pub fn rotate270(handle: i64) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.rotate270())
}

pub fn flip_horizontal(handle: i64) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.fliph())
}

pub fn flip_vertical(handle: i64) -> Result<i64, ImageError> {
    let _permit = reserve_operation(MAX_IMAGE_BYTES)?;
    let image = get_image(handle)?;
    insert(image.flipv())
}

/// Free the registry slot associated with `handle`. Idempotent.
pub fn close(handle: i64) {
    crate::native::lock_recover(registry())
        .images
        .remove(&handle_key(handle));
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.images, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn make_test_image_bytes() -> Vec<u8> {
        // 4x4 red image.
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(4, 4, |_, _| Rgb([255, 0, 0]));
        let mut buffer = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut buffer, ImageFormat::Png)
            .unwrap();
        buffer.into_inner()
    }

    #[test]
    fn encoded_output_writer_stops_before_growth_past_limit() {
        let mut output = BoundedCursor::new(4);
        output.write_all(b"1234").unwrap();
        assert!(output.write_all(b"5").is_err());
        assert_eq!(output.into_inner(), b"1234");
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
        close(original);
        close(resized);
    }

    #[test]
    fn transform_pipeline_round_trip() {
        let a = load_bytes(&make_test_image_bytes()).unwrap();
        let b = grayscale(a).unwrap();
        let c = brighten(b, 10).unwrap();
        let d = rotate90(c).unwrap();
        let e = flip_horizontal(d).unwrap();
        // Every step yields a distinct handle.
        for handle in [a, b, c, d, e] {
            assert!(width(handle).is_ok());
        }
        for handle in [a, b, c, d, e] {
            close(handle);
        }
    }

    #[test]
    fn encode_round_trip_bytes() {
        let a = load_bytes(&make_test_image_bytes()).unwrap();
        let png = encode(a, "png").unwrap();
        assert!(png.len() > 8 && png.starts_with(b"\x89PNG"));
        let b = load_bytes(&png).unwrap();
        assert_eq!(width(b).unwrap(), 4);
        close(a);
        close(b);
    }

    #[test]
    fn parses_named_formats_and_filters() {
        assert!(matches!(parse_format("PNG").unwrap(), ImageFormat::Png));
        assert!(matches!(parse_format("jpg").unwrap(), ImageFormat::Jpeg));
        assert!(matches!(
            parse_filter("Lanczos3").unwrap(),
            FilterType::Lanczos3
        ));
        assert!(parse_format("banana").is_err());
        assert!(parse_filter("magic").is_err());
    }

    #[test]
    fn handles_dimensions_transforms_and_transient_memory_are_bounded() {
        assert!(matches!(
            from_rgba(0, 1, &[]),
            Err(ImageError::InvalidParameter(_))
        ));
        assert!(matches!(
            from_rgba(MAX_IMAGE_DIMENSION + 1, 1, &[]),
            Err(ImageError::InvalidParameter(_))
        ));
        assert!(matches!(
            validate_registry_capacity(0, MAX_RUNTIME_IMAGE_BYTES, 1),
            Err(ImageError::ResourceLimit {
                resource: "runtime decoded image bytes",
                ..
            })
        ));

        let runtime_id = 8_300_007;
        crate::native::with_runtime_context(runtime_id, || {
            let mut handles = (0..MAX_IMAGE_HANDLES)
                .map(|_| from_rgba(1, 1, &[0, 0, 0, 255]).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                from_rgba(1, 1, &[0, 0, 0, 255]),
                Err(ImageError::ResourceLimit {
                    resource: "image handles",
                    ..
                })
            ));
            assert!(matches!(
                crop(handles[0], 1, 0, 1, 1),
                Err(ImageError::InvalidParameter(_))
            ));
            assert!(matches!(
                blur(handles[0], f32::NAN),
                Err(ImageError::InvalidParameter(_))
            ));
            close(handles.pop().unwrap());
            handles.push(from_rgba(1, 1, &[0, 0, 0, 255]).unwrap());
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_IMAGE_HANDLES);

        crate::native::with_runtime_context(runtime_id, || {
            let first = reserve_operation(MAX_IMAGE_BYTES).unwrap();
            let second = reserve_operation(MAX_IMAGE_BYTES).unwrap();
            assert!(matches!(
                reserve_operation(1),
                Err(ImageError::ResourceLimit {
                    resource: "transient image operation bytes",
                    ..
                })
            ));
            drop((first, second));

            let permits = (0..MAX_CONCURRENT_IMAGE_OPERATIONS)
                .map(|_| reserve_operation(1).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_operation(1),
                Err(ImageError::ResourceLimit {
                    resource: "concurrent image operations",
                    ..
                })
            ));
            drop(permits);
        });
        assert!(!crate::native::lock_recover(operation_usage()).contains_key(&runtime_id));
    }

    #[test]
    fn unknown_handle_is_reported() {
        assert!(width(9_999_999).is_err());
    }
}
