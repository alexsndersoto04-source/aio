//! Charting (`std::plot::*`) powered by `plotters` 0.3 (pure Rust).
//!
//! We deliberately turn off `plotters`'s `ttf` / `font-kit` default so
//! no C dependency (freetype-sys, expat-sys, fontconfig) sneaks into
//! the Termux build. The **SVG backend** is used exclusively: SVG text
//! is rendered by whatever viewer opens the file (Firefox, GIMP, any
//! image viewer on Android), so charts stay crisp without shipping a
//! TTF file.
//!
//! Every function takes plain data (parallel `x`/`y` arrays of
//! floats, or plain string labels) plus a filesystem path and writes a
//! standalone `.svg` file. No handles, no state — just data in, file
//! out. Perfect fit for dashboards, quick reports, procfs snapshots.
//!
//! Combine with:
//! * `std::procfs::*` (Fase 8) — grafica RAM/CPU en el tiempo.
//! * `std::fs::read_bytes` — para meter el SVG en `respond_bytes`
//!   (Fase 11) y servirlo desde un endpoint web.
//! * `librsvg` (`pkg install librsvg`, entrega `rsvg-convert`) — si
//!   querés PNG: `rsvg-convert chart.svg -o chart.png`.
//!
//! ## Example
//!
//! ```rust,ignore
//! use titan_stdlib::plot_mod;
//! let xs = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
//! let ys = vec![0.0, 1.0, 4.0, 9.0, 16.0, 25.0];
//! plot_mod::line_svg("/tmp/parabola.svg", "y = x²", "x", "y", &xs, &ys)?;
//! ```

use std::path::Path;

use plotters::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotError {
    #[error("plot error: {0}")]
    Draw(String),
    #[error("plot input error: {0}")]
    Input(&'static str),
}

fn err<E: std::fmt::Display>(e: E) -> PlotError { PlotError::Draw(e.to_string()) }

/// Font family label passed to plotters. Because we disabled the `ttf`
/// feature, plotters emits `<text font-family="…">` and the viewer
/// picks a matching system font. Every SVG viewer honours this.
const FONT: &str = "sans-serif";

// Common cosmetic constants.
const WIDTH:  u32 = 900;
const HEIGHT: u32 = 500;

/// Compute (min, max, padded_min, padded_max) for a series so plots
/// have a small margin above and below the extremes.
fn range(values: &[f64]) -> (f64, f64) {
    if values.is_empty() { return (0.0, 1.0); }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in values { if v < lo { lo = v; } if v > hi { hi = v; } }
    if lo == hi { lo -= 0.5; hi += 0.5; }
    let pad = (hi - lo) * 0.05;
    (lo - pad, hi + pad)
}

/// Line chart (single series). Writes an SVG to `path`.
pub fn line_svg(
    path:   &str,
    title:  &str,
    x_axis: &str,
    y_axis: &str,
    xs:     &[f64],
    ys:     &[f64],
) -> Result<(), PlotError> {
    if xs.len() != ys.len() { return Err(PlotError::Input("xs and ys must have the same length")); }
    if xs.is_empty()        { return Err(PlotError::Input("need at least one point")); }

    let (x_lo, x_hi) = range(xs);
    let (y_lo, y_hi) = range(ys);

    let root = SVGBackend::new(Path::new(path), (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&WHITE).map_err(err)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, (FONT, 24))
        .margin(15)
        .x_label_area_size(45)
        .y_label_area_size(60)
        .build_cartesian_2d(x_lo..x_hi, y_lo..y_hi)
        .map_err(err)?;

    chart.configure_mesh()
        .x_desc(x_axis)
        .y_desc(y_axis)
        .label_style((FONT, 14))
        .axis_desc_style((FONT, 16))
        .draw().map_err(err)?;

    let series: Vec<(f64, f64)> = xs.iter().zip(ys.iter()).map(|(x, y)| (*x, *y)).collect();
    chart.draw_series(LineSeries::new(series.clone(), BLUE.stroke_width(2))).map_err(err)?;
    // Small dots on every data point so the reader can tell the samples apart.
    chart.draw_series(series.into_iter().map(|(x, y)| Circle::new((x, y), 3, BLUE.filled()))).map_err(err)?;

    root.present().map_err(err)?;
    Ok(())
}

/// Multi-series line chart. Each series is `(label, xs, ys)`. All
/// series must have matching xs.len() == ys.len(), but different
/// series can have different lengths. Writes SVG.
pub fn multi_line_svg(
    path:    &str,
    title:   &str,
    x_axis:  &str,
    y_axis:  &str,
    series:  &[(String, Vec<f64>, Vec<f64>)],
) -> Result<(), PlotError> {
    if series.is_empty() { return Err(PlotError::Input("need at least one series")); }
    for (_, xs, ys) in series {
        if xs.len() != ys.len() { return Err(PlotError::Input("each series xs/ys must match")); }
        if xs.is_empty()        { return Err(PlotError::Input("series cannot be empty")); }
    }

    let all_x: Vec<f64> = series.iter().flat_map(|(_, x, _)| x.iter().copied()).collect();
    let all_y: Vec<f64> = series.iter().flat_map(|(_, _, y)| y.iter().copied()).collect();
    let (x_lo, x_hi) = range(&all_x);
    let (y_lo, y_hi) = range(&all_y);

    let root = SVGBackend::new(Path::new(path), (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&WHITE).map_err(err)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, (FONT, 24))
        .margin(15)
        .x_label_area_size(45)
        .y_label_area_size(60)
        .build_cartesian_2d(x_lo..x_hi, y_lo..y_hi)
        .map_err(err)?;

    chart.configure_mesh()
        .x_desc(x_axis).y_desc(y_axis)
        .label_style((FONT, 14))
        .axis_desc_style((FONT, 16))
        .draw().map_err(err)?;

    // Deterministic palette so colours are stable across runs.
    let palette: [RGBColor; 8] = [
        RGBColor(31, 119, 180),  RGBColor(255, 127,  14),
        RGBColor(44, 160,  44),  RGBColor(214,  39,  40),
        RGBColor(148, 103, 189), RGBColor(140,  86,  75),
        RGBColor(227, 119, 194), RGBColor(127, 127, 127),
    ];

    for (i, (label, xs, ys)) in series.iter().enumerate() {
        let color = palette[i % palette.len()];
        let points: Vec<(f64, f64)> = xs.iter().zip(ys.iter()).map(|(x, y)| (*x, *y)).collect();
        chart.draw_series(LineSeries::new(points, color.stroke_width(2)))
            .map_err(err)?
            .label(label)
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 18, y)], color.stroke_width(2)));
    }

    chart.configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .label_font((FONT, 14))
        .draw().map_err(err)?;

    root.present().map_err(err)?;
    Ok(())
}

/// Bar chart from parallel `labels` / `values`. Writes SVG.
pub fn bar_svg(
    path:   &str,
    title:  &str,
    y_axis: &str,
    labels: &[String],
    values: &[f64],
) -> Result<(), PlotError> {
    if labels.len() != values.len() { return Err(PlotError::Input("labels and values must have the same length")); }
    if labels.is_empty()            { return Err(PlotError::Input("need at least one bar")); }

    let n = values.len();
    let (mut y_lo, y_hi) = range(values);
    if y_lo > 0.0 { y_lo = 0.0; }
    let y_hi = if y_hi <= 0.0 { 1.0 } else { y_hi };

    let root = SVGBackend::new(Path::new(path), (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&WHITE).map_err(err)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, (FONT, 24))
        .margin(15)
        .x_label_area_size(55)
        .y_label_area_size(60)
        .build_cartesian_2d(0f64..n as f64, y_lo..y_hi)
        .map_err(err)?;

    chart.configure_mesh()
        .y_desc(y_axis)
        .x_labels(n)
        .x_label_formatter(&|x| {
            let i = *x as usize;
            labels.get(i).cloned().unwrap_or_default()
        })
        .label_style((FONT, 14))
        .axis_desc_style((FONT, 16))
        .disable_x_mesh()
        .draw().map_err(err)?;

    let bar_color = RGBColor(31, 119, 180);
    chart.draw_series(values.iter().enumerate().map(|(i, &v)| {
        let x = i as f64;
        Rectangle::new([(x + 0.15, 0.0), (x + 0.85, v)], bar_color.filled())
    })).map_err(err)?;

    root.present().map_err(err)?;
    Ok(())
}

/// Scatter plot. Writes SVG.
pub fn scatter_svg(
    path:   &str,
    title:  &str,
    x_axis: &str,
    y_axis: &str,
    xs:     &[f64],
    ys:     &[f64],
) -> Result<(), PlotError> {
    if xs.len() != ys.len() { return Err(PlotError::Input("xs and ys must have the same length")); }
    if xs.is_empty()        { return Err(PlotError::Input("need at least one point")); }

    let (x_lo, x_hi) = range(xs);
    let (y_lo, y_hi) = range(ys);

    let root = SVGBackend::new(Path::new(path), (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&WHITE).map_err(err)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, (FONT, 24))
        .margin(15)
        .x_label_area_size(45)
        .y_label_area_size(60)
        .build_cartesian_2d(x_lo..x_hi, y_lo..y_hi)
        .map_err(err)?;

    chart.configure_mesh()
        .x_desc(x_axis).y_desc(y_axis)
        .label_style((FONT, 14))
        .axis_desc_style((FONT, 16))
        .draw().map_err(err)?;

    let dot = RGBColor(214, 39, 40);
    chart.draw_series(xs.iter().zip(ys.iter()).map(|(x, y)| Circle::new((*x, *y), 4, dot.filled())))
        .map_err(err)?;

    root.present().map_err(err)?;
    Ok(())
}

/// Histogram from a raw sample vector (auto-bins into `bins` buckets).
/// Writes SVG. `bins` is clamped to at least 1.
pub fn histogram_svg(
    path:   &str,
    title:  &str,
    x_axis: &str,
    values: &[f64],
    bins:   usize,
) -> Result<(), PlotError> {
    if values.is_empty() { return Err(PlotError::Input("need at least one value")); }
    let bins = bins.max(1);

    let (lo, hi) = range(values);
    let width = (hi - lo) / bins as f64;
    let mut counts = vec![0u64; bins];
    for &v in values {
        let mut idx = ((v - lo) / width) as usize;
        if idx >= bins { idx = bins - 1; }
        counts[idx] += 1;
    }
    let max_count = *counts.iter().max().unwrap_or(&1) as f64;

    let root = SVGBackend::new(Path::new(path), (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&WHITE).map_err(err)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, (FONT, 24))
        .margin(15)
        .x_label_area_size(45)
        .y_label_area_size(60)
        .build_cartesian_2d(lo..hi, 0f64..(max_count * 1.1))
        .map_err(err)?;

    chart.configure_mesh()
        .x_desc(x_axis).y_desc("count")
        .label_style((FONT, 14))
        .axis_desc_style((FONT, 16))
        .draw().map_err(err)?;

    let color = RGBColor(44, 160, 44);
    chart.draw_series(counts.iter().enumerate().map(|(i, &c)| {
        let x0 = lo + (i as f64) * width;
        let x1 = x0 + width;
        Rectangle::new([(x0, 0.0), (x1, c as f64)], color.filled())
    })).map_err(err)?;

    root.present().map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmp(name: &str) -> String {
        env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn line_writes_svg_file() {
        let out = tmp("titan_line_test.svg");
        let xs = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let ys = vec![0.0, 1.0, 4.0, 9.0, 16.0];
        line_svg(&out, "y = x²", "x", "y", &xs, &ys).expect("line ok");
        let meta = std::fs::metadata(&out).expect("file exists");
        assert!(meta.len() > 0);
        // Read first bytes to check it's a real SVG.
        let head = std::fs::read_to_string(&out).unwrap();
        assert!(head.starts_with("<?xml") || head.contains("<svg"));
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn bar_and_scatter_and_hist_are_svg() {
        let out1 = tmp("titan_bar_test.svg");
        let out2 = tmp("titan_scatter_test.svg");
        let out3 = tmp("titan_hist_test.svg");
        bar_svg(&out1, "Ventas", "€",
                &["ene".into(), "feb".into(), "mar".into()],
                &[100.0, 250.0, 175.0]).unwrap();
        scatter_svg(&out2, "Nube", "x", "y", &[0.0, 1.0, 2.0], &[1.0, 3.0, 2.0]).unwrap();
        histogram_svg(&out3, "Dist", "x", &[1.0, 1.0, 2.0, 3.0, 3.0, 3.0, 4.0], 4).unwrap();
        for p in [&out1, &out2, &out3] {
            let content = std::fs::read_to_string(p).unwrap();
            assert!(content.contains("<svg"), "not an SVG: {p}");
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn multi_line_supports_multiple_series() {
        let out = tmp("titan_multi_test.svg");
        let series = vec![
            ("a".into(), vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 4.0]),
            ("b".into(), vec![0.0, 1.0, 2.0], vec![0.0, 2.0, 1.0]),
        ];
        multi_line_svg(&out, "dos", "x", "y", &series).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("<svg"));
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn mismatched_lengths_are_rejected() {
        assert!(line_svg("/dev/null", "t", "x", "y", &[0.0], &[]).is_err());
        assert!(bar_svg("/dev/null", "t", "y", &["a".into()], &[]).is_err());
    }
}
