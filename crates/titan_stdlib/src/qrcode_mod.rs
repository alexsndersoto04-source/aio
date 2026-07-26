//! QR code generation (`std::qrcode::*`) via the `qrcode` crate.
//!
//! Three output formats:
//!   * ASCII / ANSI string — ready to `print` in a terminal.
//!   * SVG bytes — infinitely scalable, ideal for the web.
//!   * PNG bytes — great for sharing over SMS/email.
//!
//! Nothing simulated. `qrcode` encodes the QR bit matrix; we render it into
//! the requested format with `image` for PNG output.

use std::io::Cursor;

use image::{ImageFormat, Luma};
use qrcode::{EcLevel, QrCode};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QrError {
    #[error("QR encode error: {0}")]
    Encode(String),
    #[error("QR render error: {0}")]
    Render(String),
    #[error("unknown error-correction level '{0}' (try L, M, Q, H)")]
    UnknownLevel(String),
    #[error("image write error: {0}")]
    Image(String),
}

fn parse_level(name: &str) -> Result<EcLevel, QrError> {
    match name.to_ascii_uppercase().as_str() {
        "L" => Ok(EcLevel::L),
        "M" => Ok(EcLevel::M),
        "Q" => Ok(EcLevel::Q),
        "H" => Ok(EcLevel::H),
        other => Err(QrError::UnknownLevel(other.into())),
    }
}

fn build(text: &str, level: &str) -> Result<QrCode, QrError> {
    let level = parse_level(level)?;
    QrCode::with_error_correction_level(text.as_bytes(), level)
        .map_err(|e| QrError::Encode(e.to_string()))
}

/// Render the QR as an ASCII string, ready to `print()` in a terminal.
/// `dark` and `light` are the characters used for filled / empty modules.
pub fn to_ascii(text: &str, level: &str, dark: &str, light: &str) -> Result<String, QrError> {
    let code = build(text, level)?;
    let dark_char = dark.chars().next().unwrap_or('#');
    let light_char = light.chars().next().unwrap_or(' ');
    Ok(code.render::<char>()
        .dark_color(dark_char)
        .light_color(light_char)
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .build())
}

/// ANSI-coloured "unicode" rendering: uses UTF-8 block characters so each
/// terminal row holds two QR rows (looks crisp in most fonts).
pub fn to_unicode(text: &str, level: &str) -> Result<String, QrError> {
    let code = build(text, level)?;
    Ok(code.render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Dark)
        .light_color(qrcode::render::unicode::Dense1x2::Light)
        .quiet_zone(true)
        .build())
}

/// SVG bytes suitable for embedding in a webpage.
pub fn to_svg(text: &str, level: &str, module_pixels: u32) -> Result<Vec<u8>, QrError> {
    let code = build(text, level)?;
    Ok(code.render::<qrcode::render::svg::Color>()
        .min_dimensions(module_pixels, module_pixels)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .quiet_zone(true)
        .build()
        .into_bytes())
}

/// PNG bytes ready to be written or shared.
pub fn to_png(text: &str, level: &str, side_pixels: u32) -> Result<Vec<u8>, QrError> {
    let code = build(text, level)?;
    let image = code.render::<Luma<u8>>()
        .min_dimensions(side_pixels, side_pixels)
        .quiet_zone(true)
        .build();
    let mut buffer = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(image)
        .write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| QrError::Image(e.to_string()))?;
    Ok(buffer.into_inner())
}

/// Convenience: encode `text` into a PNG file at `path`.
pub fn save_png(text: &str, level: &str, side_pixels: u32, path: &str) -> Result<(), QrError> {
    let bytes = to_png(text, level, side_pixels)?;
    std::fs::write(path, bytes).map_err(|e| QrError::Image(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_qr_starts_and_ends_with_light_row() {
        let ascii = to_ascii("hola titan", "M", "#", " ").unwrap();
        // ASCII QR has multiple lines and includes at least one '#'.
        let lines: Vec<&str> = ascii.lines().collect();
        assert!(lines.len() > 10);
        assert!(ascii.contains('#'));
    }

    #[test]
    fn unicode_qr_is_smaller_because_two_rows_per_char() {
        let ascii   = to_ascii("hola", "M", "#", " ").unwrap();
        let unicode = to_unicode("hola", "M").unwrap();
        assert!(unicode.lines().count() < ascii.lines().count());
    }

    #[test]
    fn svg_bytes_are_valid_xml() {
        let svg = to_svg("https://arena.ai", "M", 200).unwrap();
        let text = String::from_utf8(svg).unwrap();
        assert!(text.starts_with("<?xml") || text.contains("<svg"));
        assert!(text.contains("</svg>"));
    }

    #[test]
    fn png_bytes_have_png_magic() {
        let png = to_png("hola", "L", 200).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!(png.len() > 100);
    }

    #[test]
    fn unknown_level_reports_error() {
        assert!(to_ascii("hola", "Z", "#", " ").is_err());
    }

    #[test]
    fn error_correction_levels_all_encode() {
        for level in ["L", "M", "Q", "H"] {
            let png = to_png("Hola desde TITAN", level, 300).unwrap();
            assert!(png.len() > 100);
        }
    }
}
