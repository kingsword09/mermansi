//! Radar chart adapter — genuine radial terminal geometry.
//!
//! Draws a shared center, N radial axis spokes, graticule rings or polygons, and connected
//! plotted curve vertices normalized from configured min/max. Preserves every axis label,
//! curve label/value, ticks, graticule choice, showLegend, and title.

use crate::adapters::chart_primitives::{
    self, MAX_CHART_ENTITIES, checked_chart_dimensions, ensure_entity_limit,
};
use crate::adapters::format_title;
use crate::ansi::sanitize_label_text;
use crate::canvas::Canvas;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::radar::RadarDiagramRenderModel;
use serde_json::Value;

pub fn render_radar(model: &RadarDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    ensure_entity_limit("radar axes", model.axes.len())?;
    ensure_entity_limit("radar curves", model.curves.len())?;

    if model.axes.is_empty() {
        out.push_str("(empty radar chart)\n");
        let _ = opts;
        return Ok(out);
    }

    let n_axes = model.axes.len();
    let (chart_w, chart_h) = checked_chart_dimensions(opts, (20, 10), (80, 50))?;

    let radius = (chart_h / 2).saturating_sub(2).max(3) as f64;
    let cx = (chart_w / 2) as f64;
    let cy = (chart_h / 2) as f64;

    let mut canvas = Canvas::new(chart_w, chart_h)?;

    // Graticule: concentric rings (circle) or polygons.
    let ticks = parse_ticks(&model.options.ticks).clamp(1, 10);
    let graticule_char = match opts.charset {
        Charset::Unicode => "·",
        Charset::Ascii => ".",
    };
    for tick in 1..=ticks {
        let r = radius * (tick as f64 / ticks as f64);
        if model.options.graticule == "polygon" {
            draw_graticule_polygon(&mut canvas, cx, cy, r, n_axes, graticule_char)?;
        } else {
            chart_primitives::draw_circle_outline(
                &mut canvas,
                cx as i64,
                cy as i64,
                r as i64,
                graticule_char,
            )?;
        }
    }

    // Axis spokes.
    let spoke_char = match opts.charset {
        Charset::Unicode => "│",
        Charset::Ascii => "|",
    };
    let axis_angles = compute_axis_angles(n_axes);
    for &angle in &axis_angles {
        let ex = cx + radius * angle.cos();
        let ey = cy + radius * angle.sin();
        chart_primitives::draw_line(
            &mut canvas,
            cx as i64,
            cy as i64,
            ex.round() as i64,
            ey.round() as i64,
            spoke_char,
        )?;
    }

    // Draw curves as connected polygons.
    let (data_min, data_max) = compute_data_range(model);
    let min_val = parse_optional_number(&model.options.min).unwrap_or(data_min);
    let max_val = model
        .options
        .max
        .as_ref()
        .and_then(parse_optional_number)
        .unwrap_or(data_max);
    let range = (max_val - min_val).max(1e-9);

    for (curve_idx, curve) in model.curves.iter().enumerate() {
        let marker = chart_primitives::marker_char(curve_idx, opts.charset);
        let line_char = match opts.charset {
            Charset::Unicode => "─",
            Charset::Ascii => "-",
        };

        let vertices: Vec<(f64, f64)> = curve
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let axis_idx = i % n_axes;
                let angle = *axis_angles.get(axis_idx)?;
                let raw = json_to_f64(entry)?;
                let normalized = ((raw - min_val) / range).clamp(0.0, 1.0);
                let r = radius * normalized;
                Some((cx + r * angle.cos(), cy + r * angle.sin()))
            })
            .collect();

        if vertices.len() >= 2 {
            for window in vertices.windows(2) {
                chart_primitives::draw_line(
                    &mut canvas,
                    window[0].0.round() as i64,
                    window[0].1.round() as i64,
                    window[1].0.round() as i64,
                    window[1].1.round() as i64,
                    line_char,
                )?;
            }
            // Close the polygon if 3+ vertices.
            if vertices.len() >= 3 {
                let first = vertices[0];
                let last = *vertices.last().unwrap();
                chart_primitives::draw_line(
                    &mut canvas,
                    last.0.round() as i64,
                    last.1.round() as i64,
                    first.0.round() as i64,
                    first.1.round() as i64,
                    line_char,
                )?;
            }
            for &(vx, vy) in &vertices {
                plot_marker(&mut canvas, vx, vy, marker)?;
            }
        }
    }

    // Place center marker.
    let center_char = match opts.charset {
        Charset::Unicode => "✛",
        Charset::Ascii => "+",
    };
    canvas.set_text(cx as usize, cy as usize, center_char)?;

    // Place axis labels at spoke ends.
    for (i, axis) in model.axes.iter().enumerate() {
        if let Some(&angle) = axis_angles.get(i) {
            let label_r = radius + 1.5;
            let lx = cx + label_r * angle.cos();
            let ly = cy + label_r * angle.sin();
            place_label(&mut canvas, lx, ly, &sanitize_label_text(&axis.label))?;
        }
    }

    let chart_text = canvas.render();
    out.push_str(&chart_text);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');

    // Legend with curve labels and values.
    if model.options.show_legend && !model.curves.is_empty() {
        out.push_str("Legend:\n");
        for (i, curve) in model.curves.iter().enumerate() {
            let marker = chart_primitives::marker_char(i, opts.charset);
            let label = sanitize_label_text(&curve.label);
            let values: Vec<String> = curve.entries.iter().map(format_json_value).collect();
            out.push_str(&format!("  {marker} {label}: {}\n", values.join(", ")));
        }
    }

    let _ = MAX_CHART_ENTITIES;
    Ok(out)
}

fn compute_axis_angles(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| -std::f64::consts::FRAC_PI_2 + (i as f64 / n as f64) * std::f64::consts::TAU)
        .collect()
}

fn draw_graticule_polygon(
    canvas: &mut Canvas,
    cx: f64,
    cy: f64,
    radius: f64,
    n_axes: usize,
    glyph: &str,
) -> Result<()> {
    let angles = compute_axis_angles(n_axes);
    let vertices: Vec<(i64, i64)> = angles
        .iter()
        .map(|&a| {
            (
                (cx + radius * a.cos()).round() as i64,
                (cy + radius * a.sin()).round() as i64,
            )
        })
        .collect();
    for window in vertices.windows(2) {
        chart_primitives::draw_line(
            canvas,
            window[0].0,
            window[0].1,
            window[1].0,
            window[1].1,
            glyph,
        )?;
    }
    if vertices.len() >= 3 {
        let first = vertices[0];
        let last = *vertices.last().unwrap();
        chart_primitives::draw_line(canvas, last.0, last.1, first.0, first.1, glyph)?;
    }
    Ok(())
}

fn plot_marker(canvas: &mut Canvas, x: f64, y: f64, glyph: &str) -> Result<()> {
    let ix = x.round() as i64;
    let iy = y.round() as i64;
    if ix >= 0 && iy >= 0 {
        let (ux, uy) = (ix as usize, iy as usize);
        if ux < canvas.width() && uy < canvas.height() {
            canvas.set_text(ux, uy, glyph)?;
        }
    }
    Ok(())
}

fn place_label(canvas: &mut Canvas, x: f64, y: f64, text: &str) -> Result<()> {
    let ix = x.round() as i64;
    let iy = y.round() as i64;
    if ix < 0 || iy < 0 {
        return Ok(());
    }
    let mut ux = ix as usize;
    let uy = iy as usize;
    let label_w = str_display_width(text);
    if ux + label_w > canvas.width() {
        ux = canvas.width().saturating_sub(label_w);
    }
    if uy < canvas.height() && ux < canvas.width() {
        canvas.set_text(ux, uy, text)?;
    }
    Ok(())
}

fn compute_data_range(model: &RadarDiagramRenderModel) -> (f64, f64) {
    let values: Vec<f64> = model
        .curves
        .iter()
        .flat_map(|c| c.entries.iter())
        .filter_map(json_to_f64)
        .collect();
    if values.is_empty() {
        return (0.0, 1.0);
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < 1e-9 {
        (min - 0.5, max + 0.5)
    } else {
        (min, max)
    }
}

fn parse_ticks(v: &Value) -> usize {
    v.as_u64().unwrap_or(5) as usize
}

fn parse_optional_number(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn json_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn format_json_value(v: &Value) -> String {
    match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                format!("{f:.2}")
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
