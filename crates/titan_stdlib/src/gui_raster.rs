//! Software rasterizer for the `std::gui` widget tree (Fase 2).
//!
//! Real pixels, zero fluff: the tree is drawn onto an RGBA8 buffer in
//! pure Rust with a built-in 8x8 bitmap font — no GPU, no display, no
//! OS window required, so it runs (and is pixel-tested) identically on
//! Android/Termux, Linux, Windows and macOS. The exact same buffer is
//! what the live-window backend (minifb) will present on screen in a
//! later slice of Fase 2: what the tests verify here is what users see.

use std::collections::HashMap;

use crate::gui::{self, Widget, WidgetType};

// ---------------- Theme (RGBA) -----------------------------------------

const BG: [u8; 4] = [24, 26, 32, 255];
const EDGE: [u8; 4] = [90, 96, 112, 255];
const TITLE_BG: [u8; 4] = [43, 47, 60, 255];
const TITLE_TEXT: [u8; 4] = [235, 238, 248, 255];
const BUTTON_BG: [u8; 4] = [70, 130, 190, 255];
const BUTTON_DOWN: [u8; 4] = [38, 76, 118, 255];
const BUTTON_EDGE: [u8; 4] = [155, 195, 235, 255];
const BUTTON_TEXT: [u8; 4] = [255, 255, 255, 255];
const LABEL_TEXT: [u8; 4] = [220, 224, 232, 255];

const GLYPH: i64 = 8;
const TITLE_BAR: i64 = 22;

// ---------------- Canvas ------------------------------------------------

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: usize, height: usize, bg: [u8; 4]) -> Self {
        let mut pixels = vec![0u8; width * height * 4];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bg);
        }
        Canvas {
            width,
            height,
            pixels,
        }
    }

    fn set(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let i = (y as usize * self.width + x as usize) * 4;
        self.pixels[i..i + 4].copy_from_slice(&c);
    }

    #[cfg(test)] // solo los tests inspeccionan pixeles individuales
    fn at(&self, x: i64, y: i64) -> [u8; 4] {
        let i = (y as usize * self.width + x as usize) * 4;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    fn fill_rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 4]) {
        if w <= 0 || h <= 0 {
            return;
        }
        for py in y..y + h {
            for px in x..x + w {
                self.set(px, py, c);
            }
        }
    }

    fn stroke_rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 4]) {
        if w <= 0 || h <= 0 {
            return;
        }
        for px in x..x + w {
            self.set(px, y, c);
            self.set(px, y + h - 1, c);
        }
        for py in y..y + h {
            self.set(x, py, c);
            self.set(x + w - 1, py, c);
        }
    }

    fn draw_glyph(&mut self, ch: u8, x: i64, y: i64, fg: [u8; 4]) {
        let rows = glyph(ch);
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    self.set(x + col as i64, y + row as i64, fg);
                }
            }
        }
    }

    fn draw_text(&mut self, s: &str, x: i64, y: i64, fg: [u8; 4]) {
        for (i, ch) in s.bytes().enumerate() {
            self.draw_glyph(ch.to_ascii_uppercase(), x + i as i64 * GLYPH, y, fg);
        }
    }
}

// ---------------- Renderer ----------------------------------------------

/// Render the widget tree rooted at `container_id` into an RGBA8 buffer.
/// Returns `(width, height, pixels)` or `None` when the id is unknown,
/// is not a container, or has absurd dimensions (> 4096 px per side).
pub fn render_rgba(container_id: i64) -> Option<(u32, u32, Vec<u8>)> {
    let widgets = gui::snapshot_widgets();
    let root = widgets.get(&container_id)?;
    if root.widget_type != WidgetType::Container {
        return None;
    }
    let (w, h) = (root.width, root.height);
    if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
        return None;
    }
    let mut canvas = Canvas::new(w as usize, h as usize, BG);
    draw_widget(&widgets, &mut canvas, root, 0, 0, 0);
    Some((w as u32, h as u32, canvas.pixels))
}

fn draw_widget(
    widgets: &HashMap<i64, Widget>,
    canvas: &mut Canvas,
    widget: &Widget,
    ox: i64,
    oy: i64,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    let x = ox + widget.x;
    let y = oy + widget.y;
    match widget.widget_type {
        WidgetType::Container => {
            canvas.fill_rect(x, y, widget.width, widget.height, BG);
            canvas.stroke_rect(x, y, widget.width, widget.height, EDGE);
            let bar = TITLE_BAR.min(widget.height.saturating_sub(2)).max(0);
            if bar > 0 {
                canvas.fill_rect(x + 1, y + 1, widget.width - 2, bar, TITLE_BG);
                canvas.fill_rect(x + 1, y + 1 + bar, widget.width - 2, 1, EDGE);
                canvas.draw_text(&widget.text, x + 6, y + 7, TITLE_TEXT);
            }
        }
        WidgetType::Button => {
            let fill = if widget.clicked {
                BUTTON_DOWN
            } else {
                BUTTON_BG
            };
            canvas.fill_rect(x, y, widget.width, widget.height, fill);
            canvas.stroke_rect(x, y, widget.width, widget.height, BUTTON_EDGE);
            let tw = GLYPH * widget.text.bytes().count() as i64;
            let tx = x + (widget.width - tw).max(0) / 2;
            let ty = y + (widget.height - GLYPH).max(0) / 2;
            canvas.draw_text(&widget.text, tx, ty, BUTTON_TEXT);
        }
        WidgetType::Label => {
            canvas.draw_text(&widget.text, x, y + 1, LABEL_TEXT);
        }
    }
    for child_id in &widget.children {
        if let Some(child) = widgets.get(child_id) {
            draw_widget(widgets, canvas, child, x, y, depth + 1);
        }
    }
}

// ---------------- Built-in 8x8 bitmap font (MSB = leftmost pixel) -------
//
// Covers space, digits, A-Z (lowercase is uppercased by `draw_text`)
// and the common punctuation set. Anything else falls back to '?'.

fn glyph(ch: u8) -> [u8; 8] {
    match ch {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'!' => [0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18, 0x00],
        b'"' => [0x24, 0x24, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'#' => [0x24, 0x7E, 0x24, 0x24, 0x24, 0x7E, 0x24, 0x00],
        b'%' => [0x62, 0x64, 0x08, 0x10, 0x20, 0x46, 0x86, 0x00],
        b'&' => [0x38, 0x4C, 0x38, 0x76, 0xDC, 0xCC, 0x76, 0x00],
        b'\'' => [0x18, 0x18, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
        b')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
        b'*' => [0x00, 0x24, 0x18, 0x7E, 0x18, 0x24, 0x00, 0x00],
        b'+' => [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00],
        b',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30],
        b'-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        b'/' => [0x06, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x60, 0x00],
        b'0' => [0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00],
        b'1' => [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
        b'2' => [0x3C, 0x66, 0x06, 0x0C, 0x30, 0x60, 0x7E, 0x00],
        b'3' => [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00],
        b'4' => [0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C, 0x1E, 0x00],
        b'5' => [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00],
        b'6' => [0x3C, 0x60, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00],
        b'7' => [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00],
        b'8' => [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
        b'9' => [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x3C, 0x00],
        b':' => [0x00, 0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00],
        b';' => [0x00, 0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x30],
        b'<' => [0x0C, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0C, 0x00],
        b'=' => [0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00],
        b'>' => [0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00],
        b'?' => [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x00, 0x18, 0x00],
        b'@' => [0x3C, 0x66, 0x6E, 0x6A, 0x6E, 0x60, 0x3E, 0x00],
        b'A' => [0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00],
        b'B' => [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00],
        b'C' => [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00],
        b'D' => [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00],
        b'E' => [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x7E, 0x00],
        b'F' => [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x00],
        b'G' => [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3E, 0x00],
        b'H' => [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
        b'I' => [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
        b'J' => [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00],
        b'K' => [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00],
        b'L' => [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00],
        b'M' => [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00],
        b'N' => [0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00],
        b'O' => [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'P' => [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00],
        b'Q' => [0x3C, 0x66, 0x66, 0x66, 0x6E, 0x6C, 0x36, 0x00],
        b'R' => [0x7C, 0x66, 0x66, 0x7C, 0x78, 0x6C, 0x66, 0x00],
        b'S' => [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00],
        b'T' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
        b'U' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'V' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
        b'W' => [0x63, 0x63, 0x6B, 0x6B, 0x7F, 0x77, 0x63, 0x00],
        b'X' => [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00],
        b'Y' => [0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x00],
        b'Z' => [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00],
        b'[' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
        b']' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
        b'_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
        _ => [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x00, 0x18, 0x00], // '?'
    }
}

// ---------------- Tests --------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// The GUI tree is one global registry and `cargo test` runs these on
    /// parallel threads — serialize behind one lock, like the other
    /// stateful modules from Fase 1.
    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn fresh_container(title: &str, w: i64, h: i64) -> i64 {
        assert!(gui::init());
        let id = gui::create_container(title, w, h);
        assert!(id > 0);
        id
    }

    #[test]
    fn unknown_or_non_container_ids_reject_cleanly() {
        let _guard = test_lock();
        assert!(render_rgba(9_999_999).is_none());
        assert!(gui::init());
        let root = gui::create_container("T", 10, 10);
        let btn = gui::add_button(root, "b", 0, 0, 5, 5);
        assert!(render_rgba(btn).is_none(), "only containers can render");
        let zero = gui::create_container("x", 0, 100);
        assert!(render_rgba(zero).is_none(), "degenerate width rejected");
    }

    #[test]
    fn background_border_and_title_bar_are_painted() {
        let _guard = test_lock();
        let id = fresh_container("QA", 100, 60);
        let (w, h, px) = render_rgba(id).unwrap();
        assert_eq!((w, h), (100, 60));
        assert_eq!(px.len(), 100 * 60 * 4);
        let canvas = px;
        let at = |x: i64, y: i64| {
            let i = (y as usize * 100 + x as usize) * 4;
            [canvas[i], canvas[i + 1], canvas[i + 2], canvas[i + 3]]
        };
        assert_eq!(at(50, 40), BG, "body below the title bar");
        assert_eq!(at(2, 12), TITLE_BG, "title bar covers the top strip");
        assert_eq!(at(99, 59), EDGE, "bottom-right edge");
        assert_eq!(at(50, 23), EDGE, "title separator line at row 23");
    }

    #[test]
    fn title_text_leaves_ink_but_only_when_titled() {
        let _guard = test_lock();
        let titled = fresh_container("A", 100, 60);
        let (_, _, with_title) = render_rgba(titled).unwrap();
        let untitled = fresh_container("", 100, 60);
        let (_, _, no_title) = render_rgba(untitled).unwrap();
        let ink = |px: &[u8]| {
            (0..100)
                .map(|x| {
                    let i = (7 * 100 + x) * 4;
                    [px[i], px[i + 1], px[i + 2], px[i + 3]]
                })
                .filter(|p| *p == TITLE_TEXT)
                .count()
        };
        assert!(ink(&with_title) > 0, "letter 'A' must ink the title row");
        assert_eq!(ink(&no_title), 0, "empty title inks nothing");
    }

    #[test]
    fn button_paint_and_pressed_state_change_pixels() {
        let _guard = test_lock();
        let root = fresh_container("P", 100, 60);
        let btn = gui::add_button(root, "GO", 10, 30, 60, 20);
        let (_, _, normal) = render_rgba(root).unwrap();
        let at = |px: &[u8], x: i64, y: i64| {
            let i = (y as usize * 100 + x as usize) * 4;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        assert_eq!(at(&normal, 40, 40), BUTTON_BG);
        assert_eq!(at(&normal, 10, 30), BUTTON_EDGE);
        assert!(gui::trigger_click(btn));
        let (_, _, pressed) = render_rgba(root).unwrap();
        assert_eq!(at(&pressed, 40, 40), BUTTON_DOWN, "pressed buttons darken");
    }

    #[test]
    fn button_label_is_centered_horizontally() {
        let _guard = test_lock();
        let root = fresh_container("C", 120, 60);
        // "AB" is 16 px wide in a 64 px button at x=10 -> text starts at 34.
        let _btn = gui::add_button(root, "AB", 10, 30, 64, 20);
        let (_, _, px) = render_rgba(root).unwrap();
        let at = |x: i64, y: i64| {
            let i = (y as usize * 120 + x as usize) * 4;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        // 'A' row 0 = 0x18 -> apex pixels at columns 3 and 4 of the glyph.
        assert_eq!(at(34 + 3, 36), BUTTON_TEXT, "glyph apex ink");
        assert_eq!(at(34, 36), BUTTON_BG, "left of the apex stays button fill");
    }

    #[test]
    fn glyph_a_has_apex_and_feet_like_a_real_letterform() {
        let mut canvas = Canvas::new(8, 8, BG);
        canvas.draw_glyph(b'A', 0, 0, BUTTON_TEXT);
        assert_eq!(canvas.at(3, 0), BUTTON_TEXT, "row0 0x18 apex");
        assert_eq!(canvas.at(4, 0), BUTTON_TEXT);
        assert_eq!(canvas.at(0, 0), BG, "no ink at the top-left corner");
        assert_eq!(canvas.at(1, 3), BUTTON_TEXT, "leg");
        assert_eq!(canvas.at(5, 3), BUTTON_TEXT, "other leg");
        assert_eq!(canvas.at(3, 7), BG, "last row is the padding row");
    }

    #[test]
    fn glyph_0_is_a_ring_with_a_hollow_center() {
        let mut canvas = Canvas::new(8, 8, BG);
        canvas.draw_glyph(b'0', 0, 0, BUTTON_TEXT);
        assert_eq!(canvas.at(3, 0), BUTTON_TEXT, "top of the ring: 0x3C");
        assert_eq!(canvas.at(1, 3), BUTTON_TEXT, "left of the ring");
        assert_eq!(canvas.at(3, 4), BG, "hollow center, like a real zero");
    }

    #[test]
    fn space_draws_nothing_and_lowercase_maps_to_uppercase() {
        let mut spaced = Canvas::new(8, 8, BG);
        spaced.draw_glyph(b' ', 0, 0, BUTTON_TEXT);
        assert!(spaced.pixels.chunks_exact(4).all(|p| p == BG.as_slice()));

        let mut lower = Canvas::new(8, 8, BG);
        lower.draw_text("a", 0, 0, BUTTON_TEXT);
        let mut upper = Canvas::new(8, 8, BG);
        upper.draw_text("A", 0, 0, BUTTON_TEXT);
        assert_eq!(lower.pixels, upper.pixels, "lowercase renders as uppercase");
    }

    #[test]
    fn unknown_glyphs_fall_back_to_question_mark() {
        let mut mystery = Canvas::new(8, 8, BG);
        mystery.draw_glyph(0xF1, 0, 0, BUTTON_TEXT); // 'ñ' is outside the set
        let mut question = Canvas::new(8, 8, BG);
        question.draw_glyph(b'?', 0, 0, BUTTON_TEXT);
        assert_eq!(mystery.pixels, question.pixels);
    }

    #[test]
    fn out_of_bounds_widgets_clip_without_panicking() {
        let _guard = test_lock();
        let root = fresh_container("X", 100, 60);
        let _ = gui::add_button(root, "EDGE", 90, 50, 60, 20); // overflows right & bottom
        let (w, h, px) = render_rgba(root).unwrap();
        assert_eq!((w, h), (100, 60));
        assert_eq!(px.len(), 100 * 60 * 4, "size intact after clipped paint");
    }

    #[test]
    fn render_is_deterministic() {
        let _guard = test_lock();
        let root = fresh_container("DET", 120, 80);
        let _ = gui::add_button(root, "RUN", 20, 40, 60, 24);
        let _ = gui::add_label(root, "V0.34", 20, 30);
        let first = render_rgba(root).unwrap();
        let second = render_rgba(root).unwrap();
        assert_eq!(first, second, "same tree, same pixels, every time");
    }

    #[test]
    fn child_widgets_paint_at_their_offsets_inside_the_container() {
        let _guard = test_lock();
        let root = fresh_container("OFF", 100, 80);
        let _ = gui::add_button(root, "O", 40, 50, 24, 16);
        let (_, _, px) = render_rgba(root).unwrap();
        let at = |x: i64, y: i64| {
            let i = (y as usize * 100 + x as usize) * 4;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        assert_eq!(
            at(52, 58),
            BUTTON_BG,
            "button painted at its (40,50) offset"
        );
        assert_eq!(at(20, 40), BG, "outside the button stays background");
    }
}
