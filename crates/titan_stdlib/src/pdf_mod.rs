//! PDF generation (`std::pdf::*`) powered by `printpdf` 0.7.
//!
//! Pure-Rust PDF writing built on `lopdf` + a minimal font parser
//! (`owned_ttf_parser`). We disable all `printpdf` default features so
//! no `azul-layout`, no `rust-fontconfig`, no HTML rendering, no C
//! deps — just the raw PDF machinery.
//!
//! ## API model
//!
//! Documents cross the `.titan` boundary as opaque `i64` handles kept
//! in a process-wide registry. Pages and layers are addressed by index
//! starting from 0. All coordinates are in millimetres, PDF-space
//! (Y grows upwards from the bottom-left).
//!
//! ## Example (Rust API)
//!
//! ```rust,ignore
//! use titan_stdlib::pdf_mod;
//! let doc = pdf_mod::new("Factura #123", 210.0, 297.0)?;    // A4 portrait
//! pdf_mod::add_text(doc, 0, 0, "Titan PDF demo", 24.0, 20.0, 270.0)?;
//! pdf_mod::add_line(doc, 0, 0, 20.0, 260.0, 190.0, 260.0, 0.5)?;
//! pdf_mod::add_rect(doc, 0, 0, 20.0, 230.0, 100.0, 20.0)?;
//! pdf_mod::save(doc, "factura.pdf")?;
//! pdf_mod::close(doc);
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Mutex, OnceLock};

use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument,
    PdfDocumentReference, PdfLayerIndex, PdfPageIndex, Point, Rgb,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("pdf error: {0}")]
    Backend(String),
    #[error("unknown pdf handle {0}")]
    UnknownHandle(i64),
    #[error("page index {0} out of range (document has {1} pages)")]
    BadPage(usize, usize),
    #[error("layer index {0} out of range")]
    BadLayer(usize),
    #[error("i/o error while writing PDF: {0}")]
    Io(#[from] std::io::Error),
}

fn map_err<E: std::fmt::Display>(e: E) -> PdfError { PdfError::Backend(e.to_string()) }

// ---- Per-document state ---------------------------------------------

struct DocState {
    doc:          PdfDocumentReference,
    pages:        Vec<(PdfPageIndex, Vec<PdfLayerIndex>)>,
    default_font: IndirectFontRef,
}

struct Registry {
    docs:    HashMap<(u64, i64), DocState>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { docs: HashMap::new(), next_id: 1 }))
}

fn handle_key(handle: i64) -> (u64, i64) { crate::native::runtime_handle_key(handle) }

fn insert(state: DocState) -> i64 {
    let mut reg = registry().lock().expect("pdf registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.docs.insert(handle_key(id), state);
    id
}

fn with<F, R>(handle: i64, action: F) -> Result<R, PdfError>
where F: FnOnce(&mut DocState) -> Result<R, PdfError> {
    let mut reg = registry().lock().expect("pdf registry poisoned");
    let state = reg.docs.get_mut(&handle_key(handle)).ok_or(PdfError::UnknownHandle(handle))?;
    action(state)
}

fn get_page_layer<'a>(
    s: &'a DocState, page_idx: usize, layer_idx: usize,
) -> Result<(PdfPageIndex, PdfLayerIndex), PdfError> {
    let n = s.pages.len();
    let (page_id, layers) = s.pages.get(page_idx).ok_or(PdfError::BadPage(page_idx, n))?;
    let layer_id = *layers.get(layer_idx).ok_or(PdfError::BadLayer(layer_idx))?;
    Ok((*page_id, layer_id))
}

// ---- Public API -----------------------------------------------------

/// Create a new PDF document with the given `title` and page 1 of the
/// given size (millimetres). Returns an opaque handle.
///
/// Common paper sizes:
/// * A4 portrait:    (210.0, 297.0)
/// * A4 landscape:   (297.0, 210.0)
/// * Letter portrait:(216.0, 279.0)
pub fn new(title: &str, width_mm: f64, height_mm: f64) -> Result<i64, PdfError> {
    let (doc, page1, layer1) = PdfDocument::new(title, Mm(width_mm), Mm(height_mm), "Layer 1");
    let font = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(map_err)?;
    Ok(insert(DocState {
        doc,
        pages: vec![(page1, vec![layer1])],
        default_font: font,
    }))
}

/// Append a blank page. Returns its page index (0-based).
pub fn add_page(handle: i64, width_mm: f64, height_mm: f64, layer_name: &str) -> Result<usize, PdfError> {
    with(handle, |s| {
        let (page, layer) = s.doc.add_page(Mm(width_mm), Mm(height_mm), layer_name);
        s.pages.push((page, vec![layer]));
        Ok(s.pages.len() - 1)
    })
}

/// Number of pages currently in the document.
pub fn page_count(handle: i64) -> Result<usize, PdfError> {
    with(handle, |s| Ok(s.pages.len()))
}

/// Write text at (x_mm, y_mm) with the given font size (pt).
/// Uses the document's default Helvetica.
pub fn add_text(handle: i64, page_idx: usize, layer_idx: usize,
                text: &str, font_size_pt: f64, x_mm: f64, y_mm: f64)
    -> Result<(), PdfError>
{
    with(handle, |s| {
        let (page, layer) = get_page_layer(s, page_idx, layer_idx)?;
        let l = s.doc.get_page(page).get_layer(layer);
        l.use_text(text, font_size_pt as f32, Mm(x_mm), Mm(y_mm), &s.default_font);
        Ok(())
    })
}

/// Set the fill *and* outline colour used by subsequent draw calls.
/// Components in `[0.0, 1.0]`.
pub fn set_color(handle: i64, page_idx: usize, layer_idx: usize,
                 r: f64, g: f64, b: f64) -> Result<(), PdfError>
{
    with(handle, |s| {
        let (page, layer) = get_page_layer(s, page_idx, layer_idx)?;
        let l = s.doc.get_page(page).get_layer(layer);
        let c = Color::Rgb(Rgb::new(r as f32, g as f32, b as f32, None));
        l.set_fill_color(c.clone());
        l.set_outline_color(c);
        Ok(())
    })
}

/// Draw a straight line from (x1, y1) to (x2, y2), thickness in pt.
pub fn add_line(handle: i64, page_idx: usize, layer_idx: usize,
                x1_mm: f64, y1_mm: f64, x2_mm: f64, y2_mm: f64,
                thickness_pt: f64) -> Result<(), PdfError>
{
    with(handle, |s| {
        let (page, layer) = get_page_layer(s, page_idx, layer_idx)?;
        let l = s.doc.get_page(page).get_layer(layer);
        l.set_outline_thickness(thickness_pt as f32);
        let points = vec![
            (Point::new(Mm(x1_mm), Mm(y1_mm)), false),
            (Point::new(Mm(x2_mm), Mm(y2_mm)), false),
        ];
        l.add_line(Line { points, is_closed: false });
        Ok(())
    })
}

/// Draw an axis-aligned rectangle. Uses a closed 4-point line, so the
/// active fill+outline colours both apply (set via `set_color`).
pub fn add_rect(handle: i64, page_idx: usize, layer_idx: usize,
                x_mm: f64, y_mm: f64, width_mm: f64, height_mm: f64)
    -> Result<(), PdfError>
{
    with(handle, |s| {
        let (page, layer) = get_page_layer(s, page_idx, layer_idx)?;
        let l = s.doc.get_page(page).get_layer(layer);
        let points = vec![
            (Point::new(Mm(x_mm),             Mm(y_mm)),             false),
            (Point::new(Mm(x_mm + width_mm),  Mm(y_mm)),             false),
            (Point::new(Mm(x_mm + width_mm),  Mm(y_mm + height_mm)), false),
            (Point::new(Mm(x_mm),             Mm(y_mm + height_mm)), false),
        ];
        l.add_line(Line { points, is_closed: true });
        Ok(())
    })
}

/// Serialize the document to `path` (does not remove it from the
/// registry — call `close()` after if you want to release memory).
pub fn save(handle: i64, path: &str) -> Result<(), PdfError> {
    with(handle, |s| {
        let doc = s.doc.clone();
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        doc.save(&mut writer).map_err(map_err)?;
        Ok(())
    })
}

/// Drop a document from the registry. Idempotent.
pub fn close(handle: i64) {
    if let Ok(mut reg) = registry().lock() { reg.docs.remove(&handle_key(handle)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(page_count(999_999), Err(PdfError::UnknownHandle(_))));
    }

    #[test]
    fn round_trip_writes_valid_pdf_header() {
        let dir = std::env::temp_dir();
        let path = dir.join("titan_pdf_test.pdf");
        let path_str = path.to_string_lossy().into_owned();

        let doc = new("Titan test", 210.0, 297.0).expect("new");
        assert_eq!(page_count(doc).unwrap(), 1);

        add_text(doc, 0, 0, "Hello from Titan", 18.0, 20.0, 270.0).unwrap();
        add_line(doc, 0, 0, 20.0, 260.0, 190.0, 260.0, 0.5).unwrap();
        set_color(doc, 0, 0, 0.2, 0.6, 0.8).unwrap();
        add_rect(doc, 0, 0, 20.0, 200.0, 50.0, 30.0).unwrap();

        let p2 = add_page(doc, 210.0, 297.0, "Layer 1").unwrap();
        assert_eq!(p2, 1);
        assert_eq!(page_count(doc).unwrap(), 2);
        add_text(doc, 1, 0, "Page two body", 14.0, 20.0, 270.0).unwrap();

        save(doc, &path_str).expect("save");
        close(doc);

        let bytes = std::fs::read(&path_str).expect("file exists");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 500);
        let _ = std::fs::remove_file(&path_str);
    }

    #[test]
    fn bad_page_index_errors_cleanly() {
        let doc = new("bad-page", 100.0, 100.0).unwrap();
        assert!(matches!(add_text(doc, 5, 0, "x", 10.0, 0.0, 0.0), Err(PdfError::BadPage(5, 1))));
        close(doc);
    }
}
