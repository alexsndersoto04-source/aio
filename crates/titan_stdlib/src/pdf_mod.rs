//! Bounded PDF generation (`std::pdf::*`) powered by `printpdf` 0.7.
//!
//! `printpdf` keeps documents in `Rc<RefCell<_>>`, so its live document type
//! cannot safely be placed in a process-wide registry or shared by VM tasks.
//! TITAN instead stores a bounded, thread-safe display list and materialises a
//! real `printpdf` document only while saving a snapshot. This also gives us
//! precise limits before asking the PDF backend to allocate.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::{BuiltinFont, Color, Line, Mm, PdfDocument, Point, Polygon, Rgb};
use thiserror::Error;

const MAX_DOCUMENTS_PER_RUNTIME: usize = 8;
const MAX_PAGES_PER_DOCUMENT: usize = 256;
const MAX_PAGES_PER_RUNTIME: usize = 512;
const MAX_COMMANDS_PER_DOCUMENT: usize = 16_000;
const MAX_COMMANDS_PER_RUNTIME: usize = 32_000;
const MAX_DOCUMENT_LOGICAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_RUNTIME_LOGICAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_SERIALIZED_PDF_BYTES: usize = 64 * 1024 * 1024;
const MAX_TITLE_BYTES: usize = 4 * 1024;
const MAX_LAYER_NAME_BYTES: usize = 4 * 1024;
const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 16 * 1024;
const MAX_CONCURRENT_OPERATIONS: usize = 4;
const MAX_CONCURRENT_SAVES_PER_RUNTIME: usize = 1;
const MAX_CONCURRENT_SAVES_GLOBAL: usize = 2;
const DOCUMENT_OVERHEAD: usize = 1024;
const PAGE_OVERHEAD: usize = 512;
const COMMAND_OVERHEAD: usize = 512;
const MIN_PAGE_MM: f64 = 1.0;
const MAX_PAGE_MM: f64 = 5_000.0;
const MAX_COORDINATE_MM: f64 = 10_000.0;
const MIN_FONT_SIZE_PT: f64 = 0.1;
const MAX_FONT_SIZE_PT: f64 = 1_000.0;
const MIN_LINE_THICKNESS_PT: f64 = 0.01;
const MAX_LINE_THICKNESS_PT: f64 = 1_000.0;
const TEMP_FILE_ATTEMPTS: usize = 32;

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("PDF backend error: {0}")]
    Backend(String),
    #[error("unknown PDF handle {0}")]
    UnknownHandle(i64),
    #[error("PDF handle {0} is closing or closed")]
    Closed(i64),
    #[error("page index {0} out of range (document has {1} pages)")]
    BadPage(usize, usize),
    #[error("layer index {0} out of range (each page currently has one layer)")]
    BadLayer(usize),
    #[error("invalid PDF argument: {0}")]
    InvalidArgument(&'static str),
    #[error("character {0:?} is not representable by the built-in PDF font")]
    UnsupportedText(char),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("PDF handle space exhausted")]
    HandleSpaceExhausted,
    #[error("PDF runtime ownership ended while a document was being created")]
    RuntimeClosed,
    #[error("i/o error while writing PDF: {0}")]
    Io(#[from] std::io::Error),
}

fn map_backend<E: std::fmt::Display>(error: E) -> PdfError {
    PdfError::Backend(error.to_string())
}

#[derive(Clone)]
enum DrawCommand {
    Text {
        text: String,
        font_size_pt: f32,
        x_mm: f32,
        y_mm: f32,
    },
    Color {
        red: f32,
        green: f32,
        blue: f32,
    },
    Line {
        x1_mm: f32,
        y1_mm: f32,
        x2_mm: f32,
        y2_mm: f32,
        thickness_pt: f32,
    },
    Rect {
        x_mm: f32,
        y_mm: f32,
        width_mm: f32,
        height_mm: f32,
    },
}

#[derive(Clone)]
struct PageState {
    width_mm: f32,
    height_mm: f32,
    layer_name: String,
    commands: Vec<DrawCommand>,
}

#[derive(Clone)]
struct DocState {
    owner: u64,
    title: String,
    pages: Vec<PageState>,
    logical_bytes: usize,
    command_count: usize,
    closed: bool,
}

type SharedDocument = Arc<Mutex<DocState>>;

struct Registry {
    docs: HashMap<(u64, i64), SharedDocument>,
    reserved: HashMap<u64, usize>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            docs: HashMap::new(),
            reserved: HashMap::new(),
            next_id: 1,
        })
    })
}

#[derive(Default)]
struct RuntimeUsage {
    logical_bytes: usize,
    pages: usize,
    commands: usize,
    active_operations: usize,
}

fn runtime_usage() -> &'static Mutex<HashMap<u64, RuntimeUsage>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, RuntimeUsage>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn usage_is_empty(usage: &RuntimeUsage) -> bool {
    usage.logical_bytes == 0
        && usage.pages == 0
        && usage.commands == 0
        && usage.active_operations == 0
}

struct OperationPermit {
    runtime_id: u64,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(runtime_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.active_operations = runtime.active_operations.saturating_sub(1);
            if usage_is_empty(runtime) {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn reserve_operation() -> Result<OperationPermit, PdfError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(runtime_usage());
    let runtime = usage.entry(runtime_id).or_default();
    if runtime.active_operations >= MAX_CONCURRENT_OPERATIONS {
        return Err(PdfError::ResourceLimit {
            resource: "concurrent PDF operations",
            limit: MAX_CONCURRENT_OPERATIONS,
        });
    }
    runtime.active_operations += 1;
    Ok(OperationPermit { runtime_id })
}

#[derive(Default)]
struct SaveUsage {
    active_global: usize,
    active_by_runtime: HashMap<u64, usize>,
}

fn save_usage() -> &'static Mutex<SaveUsage> {
    static USAGE: OnceLock<Mutex<SaveUsage>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(SaveUsage::default()))
}

struct SavePermit {
    runtime_id: u64,
}

impl Drop for SavePermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(save_usage());
        usage.active_global = usage.active_global.saturating_sub(1);
        if let Some(active) = usage.active_by_runtime.get_mut(&self.runtime_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                usage.active_by_runtime.remove(&self.runtime_id);
            }
        }
    }
}

fn reserve_save() -> Result<SavePermit, PdfError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(save_usage());
    let runtime_active = usage
        .active_by_runtime
        .get(&runtime_id)
        .copied()
        .unwrap_or(0);
    if runtime_active >= MAX_CONCURRENT_SAVES_PER_RUNTIME {
        return Err(PdfError::ResourceLimit {
            resource: "concurrent PDF saves per runtime",
            limit: MAX_CONCURRENT_SAVES_PER_RUNTIME,
        });
    }
    if usage.active_global >= MAX_CONCURRENT_SAVES_GLOBAL {
        return Err(PdfError::ResourceLimit {
            resource: "concurrent PDF saves",
            limit: MAX_CONCURRENT_SAVES_GLOBAL,
        });
    }
    usage.active_global += 1;
    *usage.active_by_runtime.entry(runtime_id).or_default() += 1;
    Ok(SavePermit { runtime_id })
}

fn active_documents(registry: &Registry, runtime_id: u64) -> usize {
    registry
        .docs
        .keys()
        .filter(|(owner, _)| *owner == runtime_id)
        .count()
}

fn release_handle_reservation(registry: &mut Registry, runtime_id: u64) {
    if let Some(reserved) = registry.reserved.get_mut(&runtime_id) {
        *reserved = reserved.saturating_sub(1);
        if *reserved == 0 {
            registry.reserved.remove(&runtime_id);
        }
    }
}

fn check_runtime_growth(
    runtime: &RuntimeUsage,
    bytes: usize,
    pages: usize,
    commands: usize,
) -> Result<(usize, usize, usize), PdfError> {
    let new_bytes = runtime
        .logical_bytes
        .checked_add(bytes)
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF runtime logical bytes",
            limit: MAX_RUNTIME_LOGICAL_BYTES,
        })?;
    if new_bytes > MAX_RUNTIME_LOGICAL_BYTES {
        return Err(PdfError::ResourceLimit {
            resource: "PDF runtime logical bytes",
            limit: MAX_RUNTIME_LOGICAL_BYTES,
        });
    }
    let new_pages = runtime.pages.checked_add(pages).ok_or(PdfError::ResourceLimit {
        resource: "PDF runtime pages",
        limit: MAX_PAGES_PER_RUNTIME,
    })?;
    if new_pages > MAX_PAGES_PER_RUNTIME {
        return Err(PdfError::ResourceLimit {
            resource: "PDF runtime pages",
            limit: MAX_PAGES_PER_RUNTIME,
        });
    }
    let new_commands = runtime
        .commands
        .checked_add(commands)
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF runtime drawing commands",
            limit: MAX_COMMANDS_PER_RUNTIME,
        })?;
    if new_commands > MAX_COMMANDS_PER_RUNTIME {
        return Err(PdfError::ResourceLimit {
            resource: "PDF runtime drawing commands",
            limit: MAX_COMMANDS_PER_RUNTIME,
        });
    }
    Ok((new_bytes, new_pages, new_commands))
}

struct DocumentReservation {
    runtime_id: u64,
    bytes: usize,
    pages: usize,
    committed: bool,
}

fn reserve_document(bytes: usize, pages: usize) -> Result<DocumentReservation, PdfError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = active_documents(&registry, runtime_id);
    let reserved = registry.reserved.get(&runtime_id).copied().unwrap_or(0);
    if active.saturating_add(reserved) >= MAX_DOCUMENTS_PER_RUNTIME {
        return Err(PdfError::ResourceLimit {
            resource: "PDF document handles",
            limit: MAX_DOCUMENTS_PER_RUNTIME,
        });
    }

    let mut usage = crate::native::lock_recover(runtime_usage());
    let runtime = usage.entry(runtime_id).or_default();
    let (new_bytes, new_pages, new_commands) = check_runtime_growth(runtime, bytes, pages, 0)?;
    runtime.logical_bytes = new_bytes;
    runtime.pages = new_pages;
    runtime.commands = new_commands;
    *registry.reserved.entry(runtime_id).or_default() += 1;
    Ok(DocumentReservation {
        runtime_id,
        bytes,
        pages,
        committed: false,
    })
}

impl DocumentReservation {
    fn commit(mut self, state: DocState) -> Result<i64, PdfError> {
        let mut registry = crate::native::lock_recover(registry());
        if registry
            .reserved
            .get(&self.runtime_id)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return Err(PdfError::RuntimeClosed);
        }
        let id = registry.next_id;
        registry.next_id = id.checked_add(1).ok_or(PdfError::HandleSpaceExhausted)?;
        release_handle_reservation(&mut registry, self.runtime_id);
        registry
            .docs
            .insert((self.runtime_id, id), Arc::new(Mutex::new(state)));
        self.committed = true;
        Ok(id)
    }
}

impl Drop for DocumentReservation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        {
            let mut registry = crate::native::lock_recover(registry());
            release_handle_reservation(&mut registry, self.runtime_id);
        }
        release_runtime_storage(self.runtime_id, self.bytes, self.pages, 0);
    }
}

fn release_runtime_storage(runtime_id: u64, bytes: usize, pages: usize, commands: usize) {
    let mut usage = crate::native::lock_recover(runtime_usage());
    if let Some(runtime) = usage.get_mut(&runtime_id) {
        runtime.logical_bytes = runtime.logical_bytes.saturating_sub(bytes);
        runtime.pages = runtime.pages.saturating_sub(pages);
        runtime.commands = runtime.commands.saturating_sub(commands);
        if usage_is_empty(runtime) {
            usage.remove(&runtime_id);
        }
    }
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

fn get_document(handle: i64) -> Result<SharedDocument, PdfError> {
    let registry = crate::native::lock_recover(registry());
    registry
        .docs
        .get(&handle_key(handle))
        .cloned()
        .ok_or(PdfError::UnknownHandle(handle))
}

fn ensure_open(state: &DocState, handle: i64) -> Result<(), PdfError> {
    if state.closed {
        Err(PdfError::Closed(handle))
    } else {
        Ok(())
    }
}

fn validate_size(
    value: &str,
    resource: &'static str,
    limit: usize,
) -> Result<(), PdfError> {
    if value.len() > limit {
        return Err(PdfError::ResourceLimit { resource, limit });
    }
    Ok(())
}

fn is_windows_1252(character: char) -> bool {
    matches!(character, '\u{20}'..='\u{7e}' | '\u{a0}'..='\u{ff}')
        || matches!(
            character,
            '\u{20ac}'
                | '\u{201a}'
                | '\u{0192}'
                | '\u{201e}'
                | '\u{2026}'
                | '\u{2020}'
                | '\u{2021}'
                | '\u{02c6}'
                | '\u{2030}'
                | '\u{0160}'
                | '\u{2039}'
                | '\u{0152}'
                | '\u{017d}'
                | '\u{2018}'
                | '\u{2019}'
                | '\u{201c}'
                | '\u{201d}'
                | '\u{2022}'
                | '\u{2013}'
                | '\u{2014}'
                | '\u{02dc}'
                | '\u{2122}'
                | '\u{0161}'
                | '\u{203a}'
                | '\u{0153}'
                | '\u{017e}'
                | '\u{0178}'
        )
}

fn validate_text(value: &str, resource: &'static str, limit: usize) -> Result<(), PdfError> {
    validate_size(value, resource, limit)?;
    for character in value.chars() {
        if !is_windows_1252(character) {
            return Err(PdfError::UnsupportedText(character));
        }
    }
    Ok(())
}

fn validate_page_size(value: f64) -> Result<f32, PdfError> {
    if !value.is_finite() || !(MIN_PAGE_MM..=MAX_PAGE_MM).contains(&value) {
        return Err(PdfError::InvalidArgument(
            "page dimensions must be finite and between 1 and 5000 mm",
        ));
    }
    Ok(value as f32)
}

fn validate_coordinate(value: f64) -> Result<f32, PdfError> {
    if !value.is_finite() || value.abs() > MAX_COORDINATE_MM {
        return Err(PdfError::InvalidArgument(
            "coordinates must be finite and within +/-10000 mm",
        ));
    }
    Ok(value as f32)
}

fn validate_font_size(value: f64) -> Result<f32, PdfError> {
    if !value.is_finite() || !(MIN_FONT_SIZE_PT..=MAX_FONT_SIZE_PT).contains(&value) {
        return Err(PdfError::InvalidArgument(
            "font size must be finite and between 0.1 and 1000 pt",
        ));
    }
    Ok(value as f32)
}

fn validate_thickness(value: f64) -> Result<f32, PdfError> {
    if !value.is_finite()
        || !(MIN_LINE_THICKNESS_PT..=MAX_LINE_THICKNESS_PT).contains(&value)
    {
        return Err(PdfError::InvalidArgument(
            "line thickness must be finite and between 0.01 and 1000 pt",
        ));
    }
    Ok(value as f32)
}

fn validate_color(value: f64) -> Result<f32, PdfError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(PdfError::InvalidArgument(
            "color components must be finite and between 0 and 1",
        ));
    }
    Ok(value as f32)
}

fn validate_positive_extent(value: f64) -> Result<f32, PdfError> {
    if !value.is_finite() || value <= 0.0 || value > MAX_PAGE_MM {
        return Err(PdfError::InvalidArgument(
            "rectangle dimensions must be finite, positive, and at most 5000 mm",
        ));
    }
    Ok(value as f32)
}

fn page_mut<'a>(
    state: &'a mut DocState,
    handle: i64,
    page_idx: usize,
    layer_idx: usize,
) -> Result<&'a mut PageState, PdfError> {
    ensure_open(state, handle)?;
    let page_count = state.pages.len();
    let page = state
        .pages
        .get_mut(page_idx)
        .ok_or(PdfError::BadPage(page_idx, page_count))?;
    if layer_idx != 0 {
        return Err(PdfError::BadLayer(layer_idx));
    }
    Ok(page)
}

fn reserve_growth(
    state: &mut DocState,
    bytes: usize,
    pages: usize,
    commands: usize,
) -> Result<(), PdfError> {
    let document_bytes = state
        .logical_bytes
        .checked_add(bytes)
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF document logical bytes",
            limit: MAX_DOCUMENT_LOGICAL_BYTES,
        })?;
    if document_bytes > MAX_DOCUMENT_LOGICAL_BYTES {
        return Err(PdfError::ResourceLimit {
            resource: "PDF document logical bytes",
            limit: MAX_DOCUMENT_LOGICAL_BYTES,
        });
    }
    let document_pages = state.pages.len().checked_add(pages).ok_or(
        PdfError::ResourceLimit {
            resource: "PDF document pages",
            limit: MAX_PAGES_PER_DOCUMENT,
        },
    )?;
    if document_pages > MAX_PAGES_PER_DOCUMENT {
        return Err(PdfError::ResourceLimit {
            resource: "PDF document pages",
            limit: MAX_PAGES_PER_DOCUMENT,
        });
    }
    let document_commands = state
        .command_count
        .checked_add(commands)
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF document drawing commands",
            limit: MAX_COMMANDS_PER_DOCUMENT,
        })?;
    if document_commands > MAX_COMMANDS_PER_DOCUMENT {
        return Err(PdfError::ResourceLimit {
            resource: "PDF document drawing commands",
            limit: MAX_COMMANDS_PER_DOCUMENT,
        });
    }

    let mut usage = crate::native::lock_recover(runtime_usage());
    let runtime = usage.entry(state.owner).or_default();
    let (runtime_bytes, runtime_pages, runtime_commands) =
        check_runtime_growth(runtime, bytes, pages, commands)?;
    runtime.logical_bytes = runtime_bytes;
    runtime.pages = runtime_pages;
    runtime.commands = runtime_commands;
    state.logical_bytes = document_bytes;
    state.command_count = document_commands;
    Ok(())
}

fn append_command(
    handle: i64,
    page_idx: usize,
    layer_idx: usize,
    command: DrawCommand,
    text_bytes: usize,
) -> Result<(), PdfError> {
    let state = get_document(handle)?;
    let mut state = crate::native::lock_recover(&state);
    // Validate the page before charging the command quota.
    let _ = page_mut(&mut state, handle, page_idx, layer_idx)?;
    let bytes = COMMAND_OVERHEAD
        .checked_add(text_bytes)
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF document logical bytes",
            limit: MAX_DOCUMENT_LOGICAL_BYTES,
        })?;
    reserve_growth(&mut state, bytes, 0, 1)?;
    page_mut(&mut state, handle, page_idx, layer_idx)?
        .commands
        .push(command);
    Ok(())
}

/// Create a PDF display list with one page and one layer.
pub fn new(title: &str, width_mm: f64, height_mm: f64) -> Result<i64, PdfError> {
    let _permit = reserve_operation()?;
    validate_text(title, "PDF title bytes", MAX_TITLE_BYTES)?;
    let width_mm = validate_page_size(width_mm)?;
    let height_mm = validate_page_size(height_mm)?;
    let layer_name = "Layer 1".to_string();
    let logical_bytes = DOCUMENT_OVERHEAD
        .checked_add(title.len())
        .and_then(|value| value.checked_add(PAGE_OVERHEAD))
        .and_then(|value| value.checked_add(layer_name.len()))
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF document logical bytes",
            limit: MAX_DOCUMENT_LOGICAL_BYTES,
        })?;
    let reservation = reserve_document(logical_bytes, 1)?;
    let owner = crate::native::current_runtime_id();
    reservation.commit(DocState {
        owner,
        title: title.to_string(),
        pages: vec![PageState {
            width_mm,
            height_mm,
            layer_name,
            commands: Vec::new(),
        }],
        logical_bytes,
        command_count: 0,
        closed: false,
    })
}

/// Append a blank page. Returns its zero-based page index.
pub fn add_page(
    handle: i64,
    width_mm: f64,
    height_mm: f64,
    layer_name: &str,
) -> Result<usize, PdfError> {
    let _permit = reserve_operation()?;
    validate_text(
        layer_name,
        "PDF layer name bytes",
        MAX_LAYER_NAME_BYTES,
    )?;
    let width_mm = validate_page_size(width_mm)?;
    let height_mm = validate_page_size(height_mm)?;
    let bytes = PAGE_OVERHEAD
        .checked_add(layer_name.len())
        .ok_or(PdfError::ResourceLimit {
            resource: "PDF document logical bytes",
            limit: MAX_DOCUMENT_LOGICAL_BYTES,
        })?;
    let state = get_document(handle)?;
    let mut state = crate::native::lock_recover(&state);
    ensure_open(&state, handle)?;
    reserve_growth(&mut state, bytes, 1, 0)?;
    state.pages.push(PageState {
        width_mm,
        height_mm,
        layer_name: layer_name.to_string(),
        commands: Vec::new(),
    });
    Ok(state.pages.len() - 1)
}

pub fn page_count(handle: i64) -> Result<usize, PdfError> {
    let _permit = reserve_operation()?;
    let state = get_document(handle)?;
    let state = crate::native::lock_recover(&state);
    ensure_open(&state, handle)?;
    Ok(state.pages.len())
}

pub fn add_text(
    handle: i64,
    page_idx: usize,
    layer_idx: usize,
    text: &str,
    font_size_pt: f64,
    x_mm: f64,
    y_mm: f64,
) -> Result<(), PdfError> {
    let _permit = reserve_operation()?;
    validate_text(text, "PDF text bytes", MAX_TEXT_BYTES)?;
    let command = DrawCommand::Text {
        text: text.to_string(),
        font_size_pt: validate_font_size(font_size_pt)?,
        x_mm: validate_coordinate(x_mm)?,
        y_mm: validate_coordinate(y_mm)?,
    };
    append_command(handle, page_idx, layer_idx, command, text.len())
}

pub fn set_color(
    handle: i64,
    page_idx: usize,
    layer_idx: usize,
    red: f64,
    green: f64,
    blue: f64,
) -> Result<(), PdfError> {
    let _permit = reserve_operation()?;
    let command = DrawCommand::Color {
        red: validate_color(red)?,
        green: validate_color(green)?,
        blue: validate_color(blue)?,
    };
    append_command(handle, page_idx, layer_idx, command, 0)
}

pub fn add_line(
    handle: i64,
    page_idx: usize,
    layer_idx: usize,
    x1_mm: f64,
    y1_mm: f64,
    x2_mm: f64,
    y2_mm: f64,
    thickness_pt: f64,
) -> Result<(), PdfError> {
    let _permit = reserve_operation()?;
    let command = DrawCommand::Line {
        x1_mm: validate_coordinate(x1_mm)?,
        y1_mm: validate_coordinate(y1_mm)?,
        x2_mm: validate_coordinate(x2_mm)?,
        y2_mm: validate_coordinate(y2_mm)?,
        thickness_pt: validate_thickness(thickness_pt)?,
    };
    append_command(handle, page_idx, layer_idx, command, 0)
}

pub fn add_rect(
    handle: i64,
    page_idx: usize,
    layer_idx: usize,
    x_mm: f64,
    y_mm: f64,
    width_mm: f64,
    height_mm: f64,
) -> Result<(), PdfError> {
    let _permit = reserve_operation()?;
    let x_end = x_mm + width_mm;
    let y_end = y_mm + height_mm;
    let command = DrawCommand::Rect {
        x_mm: validate_coordinate(x_mm)?,
        y_mm: validate_coordinate(y_mm)?,
        width_mm: validate_positive_extent(width_mm)?,
        height_mm: validate_positive_extent(height_mm)?,
    };
    // Validate the calculated endpoints as well as the individual values.
    validate_coordinate(x_end)?;
    validate_coordinate(y_end)?;
    append_command(handle, page_idx, layer_idx, command, 0)
}

fn render_snapshot(snapshot: DocState) -> Result<Vec<u8>, PdfError> {
    let first_page = snapshot
        .pages
        .first()
        .ok_or(PdfError::InvalidArgument("PDF document has no pages"))?;
    let (document, first_page_id, first_layer_id) = PdfDocument::new(
        &snapshot.title,
        Mm(first_page.width_mm),
        Mm(first_page.height_mm),
        &first_page.layer_name,
    );
    let font = document
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(map_backend)?;
    let mut backend_pages = Vec::with_capacity(snapshot.pages.len());
    backend_pages.push((first_page_id, first_layer_id));
    for page in snapshot.pages.iter().skip(1) {
        backend_pages.push(document.add_page(
            Mm(page.width_mm),
            Mm(page.height_mm),
            &page.layer_name,
        ));
    }

    for (page, (page_id, layer_id)) in snapshot.pages.iter().zip(backend_pages) {
        let layer = document.get_page(page_id).get_layer(layer_id);
        for command in &page.commands {
            match command {
                DrawCommand::Text {
                    text,
                    font_size_pt,
                    x_mm,
                    y_mm,
                } => layer.use_text(
                    text,
                    *font_size_pt,
                    Mm(*x_mm),
                    Mm(*y_mm),
                    &font,
                ),
                DrawCommand::Color { red, green, blue } => {
                    let color = Color::Rgb(Rgb::new(*red, *green, *blue, None));
                    layer.set_fill_color(color.clone());
                    layer.set_outline_color(color);
                }
                DrawCommand::Line {
                    x1_mm,
                    y1_mm,
                    x2_mm,
                    y2_mm,
                    thickness_pt,
                } => {
                    layer.set_outline_thickness(*thickness_pt);
                    layer.add_line(Line {
                        points: vec![
                            (Point::new(Mm(*x1_mm), Mm(*y1_mm)), false),
                            (Point::new(Mm(*x2_mm), Mm(*y2_mm)), false),
                        ],
                        is_closed: false,
                    });
                }
                DrawCommand::Rect {
                    x_mm,
                    y_mm,
                    width_mm,
                    height_mm,
                } => {
                    layer.add_polygon(Polygon {
                        rings: vec![vec![
                            (Point::new(Mm(*x_mm), Mm(*y_mm)), false),
                            (Point::new(Mm(*x_mm + *width_mm), Mm(*y_mm)), false),
                            (
                                Point::new(Mm(*x_mm + *width_mm), Mm(*y_mm + *height_mm)),
                                false,
                            ),
                            (Point::new(Mm(*x_mm), Mm(*y_mm + *height_mm)), false),
                        ]],
                        mode: PaintMode::FillStroke,
                        winding_order: WindingOrder::NonZero,
                    });
                }
            }
        }
    }

    let bytes = document.save_to_bytes().map_err(map_backend)?;
    if bytes.len() > MAX_SERIALIZED_PDF_BYTES {
        return Err(PdfError::ResourceLimit {
            resource: "serialized PDF bytes",
            limit: MAX_SERIALIZED_PDF_BYTES,
        });
    }
    Ok(bytes)
}

struct TempOutput {
    path: PathBuf,
    keep: bool,
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn create_temp_output(target: &Path) -> Result<(File, TempOutput), PdfError> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .ok_or(PdfError::InvalidArgument("PDF output path has no file name"))?
        .to_string_lossy();
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);
    for _ in 0..TEMP_FILE_ATTEMPTS {
        let id = NEXT_TEMP_ID
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
            .map_err(|_| {
                PdfError::Io(std::io::Error::other(
                    "temporary PDF identifier space exhausted",
                ))
            })?;
        let path = parent.join(format!(
            ".{file_name}.titan-pdf-{}-{id}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                return Ok((file, TempOutput { path, keep: false }));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(PdfError::Io(error)),
        }
    }
    Err(PdfError::Io(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "could not allocate a unique temporary PDF output",
    )))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), PdfError> {
    let (mut file, mut temporary) = create_temp_output(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary.path, path)?;
    temporary.keep = true;
    Ok(())
}

/// Save a bounded snapshot to `path` without holding the registry or document
/// lock during backend generation or filesystem I/O.
pub fn save(handle: i64, path: &str) -> Result<(), PdfError> {
    let _operation = reserve_operation()?;
    let _save = reserve_save()?;
    validate_size(path, "PDF output path bytes", MAX_PATH_BYTES)?;
    if path.is_empty() {
        return Err(PdfError::InvalidArgument("PDF output path is empty"));
    }
    let state = get_document(handle)?;
    let snapshot = {
        let state = crate::native::lock_recover(&state);
        ensure_open(&state, handle)?;
        state.clone()
    };
    let bytes = render_snapshot(snapshot)?;
    atomic_write(Path::new(path), &bytes)
}

fn close_state(state: SharedDocument) -> usize {
    let mut state = crate::native::lock_recover(&state);
    if state.closed {
        return 0;
    }
    state.closed = true;
    release_runtime_storage(
        state.owner,
        state.logical_bytes,
        state.pages.len(),
        state.command_count,
    );
    1
}

/// Drop a document. Idempotent. A save that already captured its bounded
/// snapshot may finish, but no later operation can find this handle.
pub fn close(handle: i64) {
    let state = {
        let mut registry = crate::native::lock_recover(registry());
        registry.docs.remove(&handle_key(handle))
    };
    if let Some(state) = state {
        close_state(state);
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let states = {
        let mut registry = crate::native::lock_recover(registry());
        let states = registry
            .docs
            .iter()
            .filter(|((owner, _), _)| *owner == runtime_id)
            .map(|(_, state)| Arc::clone(state))
            .collect::<Vec<_>>();
        registry
            .docs
            .retain(|(owner, _), _| *owner != runtime_id);
        registry.reserved.remove(&runtime_id);
        states
    };
    states.into_iter().map(close_state).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_RUNTIME: AtomicU64 = AtomicU64::new(30_000);

    fn in_test_runtime<R>(test: impl FnOnce(u64) -> R) -> R {
        let runtime_id = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
        crate::native::with_runtime_context(runtime_id, || test(runtime_id))
    }

    fn temp_path(tag: &str, runtime_id: u64) -> PathBuf {
        std::env::temp_dir().join(format!(
            "titan-pdf-{tag}-{}-{runtime_id}.pdf",
            std::process::id()
        ))
    }

    fn save_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn round_trip_writes_a_parseable_pdf_with_real_content() {
        let _save_guard = crate::native::lock_recover(save_test_lock());
        in_test_runtime(|runtime_id| {
            let path = temp_path("round-trip", runtime_id);
            let path_string = path.to_string_lossy().into_owned();
            let document = new("Titan test", 210.0, 297.0).unwrap();
            add_text(
                document,
                0,
                0,
                "Hola desde TITAN",
                18.0,
                20.0,
                270.0,
            )
            .unwrap();
            add_line(document, 0, 0, 20.0, 260.0, 190.0, 260.0, 0.5).unwrap();
            set_color(document, 0, 0, 0.2, 0.6, 0.8).unwrap();
            add_rect(document, 0, 0, 20.0, 200.0, 50.0, 30.0).unwrap();
            assert_eq!(add_page(document, 210.0, 297.0, "Detalles").unwrap(), 1);
            add_text(document, 1, 0, "Página dos", 14.0, 20.0, 270.0).unwrap();
            assert_eq!(page_count(document).unwrap(), 2);

            save(document, &path_string).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(bytes.starts_with(b"%PDF-"));
            let parsed = printpdf::lopdf::Document::load(&path).unwrap();
            assert_eq!(parsed.get_pages().len(), 2);

            close(document);
            assert!(matches!(
                page_count(document),
                Err(PdfError::UnknownHandle(_))
            ));
            std::fs::remove_file(path).unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn dimensions_coordinates_colors_and_text_are_validated() {
        in_test_runtime(|runtime_id| {
            assert!(matches!(
                new("bad", f64::NAN, 100.0),
                Err(PdfError::InvalidArgument(_))
            ));
            assert!(matches!(
                new("emoji 😀", 100.0, 100.0),
                Err(PdfError::UnsupportedText('😀'))
            ));
            let document = new("valid", 100.0, 100.0).unwrap();
            assert!(matches!(
                set_color(document, 0, 0, 1.1, 0.0, 0.0),
                Err(PdfError::InvalidArgument(_))
            ));
            assert!(matches!(
                add_line(document, 0, 0, 0.0, 0.0, f64::INFINITY, 1.0, 1.0),
                Err(PdfError::InvalidArgument(_))
            ));
            assert!(matches!(
                add_rect(document, 0, 0, 9_000.0, 0.0, 2_000.0, 1.0),
                Err(PdfError::InvalidArgument(_))
            ));
            assert!(matches!(
                add_text(document, 0, 0, "line\nbreak", 12.0, 0.0, 0.0),
                Err(PdfError::UnsupportedText('\n'))
            ));
            assert!(matches!(
                add_text(document, 0, 0, "x", 0.0, 0.0, 0.0),
                Err(PdfError::InvalidArgument(_))
            ));
            close(document);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn bad_page_and_layer_do_not_consume_command_quota() {
        in_test_runtime(|runtime_id| {
            let document = new("indexes", 100.0, 100.0).unwrap();
            let before = crate::native::lock_recover(runtime_usage())
                .get(&runtime_id)
                .unwrap()
                .commands;
            assert!(matches!(
                add_text(document, 5, 0, "x", 10.0, 0.0, 0.0),
                Err(PdfError::BadPage(5, 1))
            ));
            assert!(matches!(
                add_text(document, 0, 2, "x", 10.0, 0.0, 0.0),
                Err(PdfError::BadLayer(2))
            ));
            assert_eq!(
                crate::native::lock_recover(runtime_usage())
                    .get(&runtime_id)
                    .unwrap()
                    .commands,
                before
            );
            close(document);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn handles_pages_commands_and_operations_are_bounded_and_recover() {
        in_test_runtime(|runtime_id| {
            let documents = (0..MAX_DOCUMENTS_PER_RUNTIME)
                .map(|index| new(&format!("doc {index}"), 100.0, 100.0).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                new("one too many", 100.0, 100.0),
                Err(PdfError::ResourceLimit {
                    resource: "PDF document handles",
                    ..
                })
            ));
            close(documents[0]);
            let replacement = new("replacement", 100.0, 100.0).unwrap();

            for index in 1..MAX_PAGES_PER_DOCUMENT {
                assert_eq!(
                    add_page(replacement, 100.0, 100.0, "Layer").unwrap(),
                    index
                );
            }
            assert!(matches!(
                add_page(replacement, 100.0, 100.0, "Layer"),
                Err(PdfError::ResourceLimit {
                    resource: "PDF document pages",
                    ..
                })
            ));
            for _ in 0..(MAX_PAGES_PER_RUNTIME - MAX_PAGES_PER_DOCUMENT - 7) {
                add_page(documents[1], 100.0, 100.0, "Layer").unwrap();
            }
            assert!(matches!(
                add_page(documents[1], 100.0, 100.0, "Layer"),
                Err(PdfError::ResourceLimit {
                    resource: "PDF runtime pages",
                    ..
                })
            ));

            let permits = (0..MAX_CONCURRENT_OPERATIONS)
                .map(|_| reserve_operation().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_operation(),
                Err(PdfError::ResourceLimit {
                    resource: "concurrent PDF operations",
                    ..
                })
            ));
            drop(permits);

            for document in documents.into_iter().skip(1) {
                close(document);
            }
            close(replacement);
            assert_eq!(cleanup_runtime(runtime_id), 0);
            assert!(!crate::native::lock_recover(runtime_usage()).contains_key(&runtime_id));
        });
    }

    #[test]
    fn command_and_logical_byte_limits_are_enforced() {
        in_test_runtime(|runtime_id| {
            let first = new("commands one", 100.0, 100.0).unwrap();
            for _ in 0..MAX_COMMANDS_PER_DOCUMENT {
                set_color(first, 0, 0, 0.0, 0.0, 0.0).unwrap();
            }
            assert!(matches!(
                set_color(first, 0, 0, 0.0, 0.0, 0.0),
                Err(PdfError::ResourceLimit {
                    resource: "PDF document drawing commands",
                    ..
                })
            ));
            let second = new("commands two", 100.0, 100.0).unwrap();
            for _ in 0..MAX_COMMANDS_PER_DOCUMENT {
                set_color(second, 0, 0, 0.0, 0.0, 0.0).unwrap();
            }
            let third = new("commands three", 100.0, 100.0).unwrap();
            assert!(matches!(
                set_color(third, 0, 0, 0.0, 0.0, 0.0),
                Err(PdfError::ResourceLimit {
                    resource: "PDF runtime drawing commands",
                    ..
                })
            ));
            close(first);
            close(second);
            close(third);

            let document = new("bytes", 100.0, 100.0).unwrap();
            let oversized = "x".repeat(MAX_TEXT_BYTES + 1);
            assert!(matches!(
                add_text(document, 0, 0, &oversized, 10.0, 0.0, 0.0),
                Err(PdfError::ResourceLimit {
                    resource: "PDF text bytes",
                    ..
                })
            ));
            close(document);

            let chunk = "x".repeat(MAX_TEXT_BYTES);
            let first = new("bytes one", 100.0, 100.0).unwrap();
            let per_command = COMMAND_OVERHEAD + chunk.len();
            let initial_bytes = crate::native::lock_recover(&get_document(first).unwrap())
                .logical_bytes;
            let chunks_per_document =
                (MAX_DOCUMENT_LOGICAL_BYTES - initial_bytes) / per_command;
            for _ in 0..chunks_per_document {
                add_text(first, 0, 0, &chunk, 10.0, 0.0, 0.0).unwrap();
            }
            assert!(matches!(
                add_text(first, 0, 0, &chunk, 10.0, 0.0, 0.0),
                Err(PdfError::ResourceLimit {
                    resource: "PDF document logical bytes",
                    ..
                })
            ));

            let second = new("bytes two", 100.0, 100.0).unwrap();
            for _ in 0..chunks_per_document {
                add_text(second, 0, 0, &chunk, 10.0, 0.0, 0.0).unwrap();
            }
            let third = new("bytes three", 100.0, 100.0).unwrap();
            while add_text(third, 0, 0, &chunk, 10.0, 0.0, 0.0).is_ok() {}
            assert!(matches!(
                add_text(third, 0, 0, &chunk, 10.0, 0.0, 0.0),
                Err(PdfError::ResourceLimit {
                    resource: "PDF runtime logical bytes",
                    ..
                })
            ));
            close(first);
            close(second);
            close(third);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn saves_and_global_save_work_are_quota_bounded() {
        let _save_guard = crate::native::lock_recover(save_test_lock());
        in_test_runtime(|runtime_id| {
            let permit = reserve_save().unwrap();
            assert!(matches!(
                reserve_save(),
                Err(PdfError::ResourceLimit {
                    resource: "concurrent PDF saves per runtime",
                    ..
                })
            ));
            drop(permit);

            let first = reserve_save().unwrap();
            let second_runtime = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
            let second = crate::native::with_runtime_context(second_runtime, reserve_save).unwrap();
            let third_runtime = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
            let third = crate::native::with_runtime_context(third_runtime, reserve_save);
            assert!(matches!(
                third,
                Err(PdfError::ResourceLimit {
                    resource: "concurrent PDF saves",
                    ..
                })
            ));
            drop(first);
            drop(second);

            let save_usage = crate::native::lock_recover(save_usage());
            assert_eq!(save_usage.active_global, 0);
            assert!(save_usage.active_by_runtime.is_empty());
            drop(save_usage);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn save_is_atomic_and_cleans_temporary_files_on_failure() {
        let _save_guard = crate::native::lock_recover(save_test_lock());
        in_test_runtime(|runtime_id| {
            let document = new("atomic", 100.0, 100.0).unwrap();
            let path = temp_path("atomic", runtime_id);
            let path_string = path.to_string_lossy().into_owned();
            std::fs::write(&path, b"old contents").unwrap();
            save(document, &path_string).unwrap();
            assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF-"));

            let directory = std::env::temp_dir().join(format!(
                "titan-pdf-target-directory-{}-{runtime_id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            let directory_string = directory.to_string_lossy().into_owned();
            assert!(save(document, &directory_string).is_err());
            let prefix = format!(".{}.titan-pdf-", directory.file_name().unwrap().to_string_lossy());
            assert!(!directory
                .parent()
                .unwrap()
                .read_dir()
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix)));

            close(document);
            std::fs::remove_file(path).unwrap();
            std::fs::remove_dir(directory).unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }

    #[test]
    fn handles_are_runtime_owned_and_cleanup_releases_usage() {
        in_test_runtime(|runtime_id| {
            let document = new("owned", 100.0, 100.0).unwrap();
            add_text(document, 0, 0, "content", 12.0, 1.0, 1.0).unwrap();
            let other_runtime = NEXT_TEST_RUNTIME.fetch_add(1, Ordering::Relaxed);
            crate::native::with_runtime_context(other_runtime, || {
                assert!(matches!(
                    page_count(document),
                    Err(PdfError::UnknownHandle(_))
                ));
                assert_eq!(cleanup_runtime(other_runtime), 0);
            });
            assert_eq!(cleanup_runtime(runtime_id), 1);
            assert!(!crate::native::lock_recover(runtime_usage()).contains_key(&runtime_id));
        });
    }

    #[test]
    fn cleanup_invalidates_inflight_document_reservation() {
        in_test_runtime(|runtime_id| {
            let bytes = DOCUMENT_OVERHEAD + PAGE_OVERHEAD + "Layer 1".len();
            let reservation = reserve_document(bytes, 1).unwrap();
            assert_eq!(cleanup_runtime(runtime_id), 0);
            let state = DocState {
                owner: runtime_id,
                title: "late".into(),
                pages: vec![PageState {
                    width_mm: 100.0,
                    height_mm: 100.0,
                    layer_name: "Layer 1".into(),
                    commands: Vec::new(),
                }],
                logical_bytes: bytes,
                command_count: 0,
                closed: false,
            };
            assert!(matches!(
                reservation.commit(state),
                Err(PdfError::RuntimeClosed)
            ));
            assert!(!crate::native::lock_recover(runtime_usage()).contains_key(&runtime_id));
        });
    }

    #[test]
    fn unknown_handle_and_path_limits_report_typed_errors() {
        in_test_runtime(|runtime_id| {
            assert!(matches!(
                page_count(999_999),
                Err(PdfError::UnknownHandle(_))
            ));
            let document = new("path", 100.0, 100.0).unwrap();
            let oversized_path = "x".repeat(MAX_PATH_BYTES + 1);
            assert!(matches!(
                save(document, &oversized_path),
                Err(PdfError::ResourceLimit {
                    resource: "PDF output path bytes",
                    ..
                })
            ));
            close(document);
            assert_eq!(cleanup_runtime(runtime_id), 0);
        });
    }
}
