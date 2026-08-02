//! Radar chart adapter — compact layered radial terminal geometry.
//!
//! Graticule, spokes, curves, markers, and labels are drawn as explicit layers so data curves
//! remain visible at crossings. Configured ticks and total curve entries are bounded before
//! geometry allocation.

use crate::adapters::chart_primitives::{self, checked_chart_dimensions, ensure_entity_limit};
use crate::adapters::{detail_separator, format_title};
use crate::ansi::sanitize_label_text;
use crate::canvas::Canvas;
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::radar::RadarDiagramRenderModel;
use serde_json::Value;

const MAX_RADAR_TICKS: usize = 18;

pub fn render_radar(model: &RadarDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    ensure_entity_limit("radar axes", model.axes.len())?;
    ensure_entity_limit("radar curves", model.curves.len())?;
    let entry_count = model.curves.iter().try_fold(0usize, |total, curve| {
        total
            .checked_add(curve.entries.len())
            .ok_or(MermansiError::RenderLimit {
                context: "radar curve entries",
                requested: usize::MAX,
                limit: chart_primitives::MAX_CHART_ENTITIES,
            })
    })?;
    ensure_entity_limit("radar curve entries", entry_count)?;

    if model.axes.is_empty() {
        out.push_str("(empty radar chart)\n");
        return Ok(out);
    }

    let ticks = parse_ticks(&model.options.ticks)?;
    let preferred_height = ticks.saturating_add(3).saturating_mul(2).clamp(14, 40);
    let longest_axis = model
        .axes
        .iter()
        .map(axis_label)
        .map(|label| str_display_width(&label))
        .max()
        .unwrap_or(0);
    let preferred_width = (preferred_height * 2)
        .max(longest_axis.saturating_mul(2).saturating_add(8))
        .min(80);
    let (chart_width, chart_height) =
        checked_chart_dimensions(opts, (20, 10), (preferred_width, preferred_height))?;
    let radius = (chart_height / 2).saturating_sub(2).max(3) as f64;
    let tick_capacity = radius.floor() as usize;
    if ticks > tick_capacity {
        return Err(MermansiError::RenderLimit {
            context: "radar ticks",
            requested: ticks,
            limit: tick_capacity,
        });
    }

    let center_x = (chart_width / 2) as f64;
    let center_y = (chart_height / 2) as f64;
    let axis_angles = compute_axis_angles(model.axes.len());
    let mut canvas = Canvas::new(chart_width, chart_height)?;

    let graticule = match opts.charset {
        Charset::Unicode => "·",
        Charset::Ascii => ".",
    };
    for tick in 1..=ticks {
        let ring_radius = radius * (tick as f64 / ticks as f64);
        if model.options.graticule == "polygon" {
            draw_graticule_polygon(
                &mut canvas,
                center_x,
                center_y,
                ring_radius,
                &axis_angles,
                graticule,
            )?;
        } else {
            draw_sparse_ring(
                &mut canvas,
                center_x,
                center_y,
                ring_radius,
                model.axes.len(),
                graticule,
            )?;
        }
    }

    let spoke = match opts.charset {
        Charset::Unicode => "┊",
        Charset::Ascii => ":",
    };
    for angle in &axis_angles {
        let (endpoint_x, endpoint_y) =
            chart_primitives::radial_point(center_x, center_y, radius, *angle);
        chart_primitives::draw_line_over(
            &mut canvas,
            center_x.round() as i64,
            center_y.round() as i64,
            endpoint_x.round() as i64,
            endpoint_y.round() as i64,
            spoke,
        )?;
    }

    let (minimum, maximum) = resolve_data_range(model)?;
    let range = maximum - minimum;
    for (curve_index, curve) in model.curves.iter().enumerate() {
        let marker = chart_primitives::marker_char(curve_index, opts.charset);
        let line = curve_line_char(curve_index, opts.charset);
        let vertices = curve
            .entries
            .iter()
            .enumerate()
            .filter_map(|(entry_index, entry)| {
                let value = json_to_f64(entry).filter(|value| value.is_finite())?;
                let angle = axis_angles[entry_index % axis_angles.len()];
                let normalized = ((value - minimum) / range).clamp(0.0, 1.0);
                let vertex_radius = radius * normalized;
                Some(chart_primitives::radial_point(
                    center_x,
                    center_y,
                    vertex_radius,
                    angle,
                ))
            })
            .collect::<Vec<_>>();

        for window in vertices.windows(2) {
            chart_primitives::draw_line_over(
                &mut canvas,
                window[0].0.round() as i64,
                window[0].1.round() as i64,
                window[1].0.round() as i64,
                window[1].1.round() as i64,
                line,
            )?;
        }
        if vertices.len() >= 3 {
            let first = vertices[0];
            let last = vertices[vertices.len() - 1];
            chart_primitives::draw_line_over(
                &mut canvas,
                last.0.round() as i64,
                last.1.round() as i64,
                first.0.round() as i64,
                first.1.round() as i64,
                line,
            )?;
        }
        for (x, y) in vertices {
            plot_marker(&mut canvas, x, y, marker)?;
        }
    }

    let center = match opts.charset {
        Charset::Unicode => "✛",
        Charset::Ascii => "+",
    };
    canvas.set_text(center_x as usize, center_y as usize, center)?;
    for (axis, angle) in model.axes.iter().zip(&axis_angles) {
        let label_radius = radius + 1.5;
        let (label_x, label_y) =
            chart_primitives::radial_point(center_x, center_y, label_radius, *angle);
        place_axis_label(&mut canvas, label_x, label_y, *angle, &axis_label(axis))?;
    }

    out.push_str(&chart_primitives::render_cropped_canvas(&canvas));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let separator = detail_separator(opts.charset);
    out.push_str(&format!(
        "\nScale: {}..{}{separator}ticks={ticks}{separator}{}\n",
        format_number(minimum),
        format_number(maximum),
        model.options.graticule
    ));
    let axes = model.axes.iter().map(axis_label).collect::<Vec<_>>();
    out.push_str(&format!("Axes: {}\n", axes.join(separator)));

    if model.options.show_legend && !model.curves.is_empty() {
        out.push_str("Legend:\n");
        for (index, curve) in model.curves.iter().enumerate() {
            let marker = chart_primitives::marker_char(index, opts.charset);
            let label = sanitize_label_text(&curve.label);
            let values = curve
                .entries
                .iter()
                .map(format_json_value)
                .collect::<Vec<_>>();
            out.push_str(&format!("  {marker} {label}: {}\n", values.join(", ")));
        }
    }

    Ok(out)
}

fn axis_label(axis: &merman_core::diagrams::radar::RadarRenderAxis) -> String {
    let label = sanitize_label_text(&axis.label);
    if label.trim().is_empty() {
        sanitize_label_text(&axis.name)
    } else {
        label
    }
}

fn compute_axis_angles(count: usize) -> Vec<f64> {
    (0..count)
        .map(|index| {
            -std::f64::consts::FRAC_PI_2 + (index as f64 / count as f64) * std::f64::consts::TAU
        })
        .collect()
}

fn draw_sparse_ring(
    canvas: &mut Canvas,
    center_x: f64,
    center_y: f64,
    radius: f64,
    axis_count: usize,
    glyph: &str,
) -> Result<()> {
    let samples = axis_count.saturating_mul(4).clamp(12, 48);
    for sample in 0..samples {
        let angle = sample as f64 / samples as f64 * std::f64::consts::TAU;
        let (x, y) = chart_primitives::radial_point(center_x, center_y, radius, angle);
        plot_grid_point(canvas, x.round() as i64, y.round() as i64, glyph)?;
    }
    Ok(())
}

fn draw_graticule_polygon(
    canvas: &mut Canvas,
    center_x: f64,
    center_y: f64,
    radius: f64,
    angles: &[f64],
    glyph: &str,
) -> Result<()> {
    let vertices = angles
        .iter()
        .map(|angle| {
            let (x, y) = chart_primitives::radial_point(center_x, center_y, radius, *angle);
            (x.round() as i64, y.round() as i64)
        })
        .collect::<Vec<_>>();
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
        chart_primitives::draw_line(
            canvas,
            vertices[vertices.len() - 1].0,
            vertices[vertices.len() - 1].1,
            vertices[0].0,
            vertices[0].1,
            glyph,
        )?;
    }
    Ok(())
}

fn plot_grid_point(canvas: &mut Canvas, x: i64, y: i64, glyph: &str) -> Result<()> {
    if x >= 0 && y >= 0 {
        let (x, y) = (x as usize, y as usize);
        if x < canvas.width()
            && y < canvas.height()
            && canvas.get_cell(x, y).is_some_and(str::is_empty)
            && canvas.continuation_owner(x, y).is_none()
        {
            canvas.set_text(x, y, glyph)?;
        }
    }
    Ok(())
}

fn plot_marker(canvas: &mut Canvas, x: f64, y: f64, glyph: &str) -> Result<()> {
    let x = x.round() as i64;
    let y = y.round() as i64;
    if x >= 0 && y >= 0 && (x as usize) < canvas.width() && (y as usize) < canvas.height() {
        canvas.set_text(x as usize, y as usize, glyph)?;
    }
    Ok(())
}

fn place_axis_label(canvas: &mut Canvas, x: f64, y: f64, angle: f64, text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let text = truncate_to_width(text, canvas.width());
    let width = str_display_width(&text);
    let mut x = x.round() as i64;
    if angle.cos() < -0.25 {
        x -= width as i64;
    } else if angle.cos().abs() <= 0.25 {
        x -= width as i64 / 2;
    }
    let y = (y.round() as i64).clamp(0, canvas.height().saturating_sub(1) as i64);
    let x = x.clamp(0, canvas.width().saturating_sub(width) as i64) as usize;
    canvas.set_text(x, y as usize, &text)
}

fn truncate_to_width(text: &str, width: usize) -> String {
    let mut output = String::new();
    let mut used = 0usize;
    for character in text.chars() {
        let character_width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if used.saturating_add(character_width) > width {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output
}

fn resolve_data_range(model: &RadarDiagramRenderModel) -> Result<(f64, f64)> {
    let (data_minimum, data_maximum) = compute_data_range(model);
    let minimum = configured_number(&model.options.min, "min", data_minimum)?;
    let maximum = match &model.options.max {
        Some(value) => configured_number(value, "max", data_maximum)?,
        None => data_maximum,
    };
    if maximum <= minimum {
        return Err(MermansiError::GeometryLayout {
            family: "radar",
            message: format!("max ({maximum}) must be greater than min ({minimum})"),
        });
    }
    Ok((minimum, maximum))
}

fn compute_data_range(model: &RadarDiagramRenderModel) -> (f64, f64) {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in model
        .curves
        .iter()
        .flat_map(|curve| &curve.entries)
        .filter_map(json_to_f64)
        .filter(|value| value.is_finite())
    {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    if !minimum.is_finite() || !maximum.is_finite() {
        (0.0, 1.0)
    } else if (maximum - minimum).abs() < 1e-9 {
        (minimum - 0.5, maximum + 0.5)
    } else {
        (minimum, maximum)
    }
}

fn configured_number(value: &Value, name: &'static str, fallback: f64) -> Result<f64> {
    if value.is_null() {
        return Ok(fallback);
    }
    let number = json_to_f64(value)
        .filter(|number| number.is_finite())
        .ok_or_else(|| MermansiError::GeometryLayout {
            family: "radar",
            message: format!("{name} must be a finite number"),
        })?;
    Ok(number)
}

fn parse_ticks(value: &Value) -> Result<usize> {
    let Some(raw) = value.as_u64() else {
        return Err(MermansiError::GeometryLayout {
            family: "radar",
            message: "ticks must be a positive integer".to_owned(),
        });
    };
    let requested = usize::try_from(raw).unwrap_or(usize::MAX);
    if requested == 0 {
        return Err(MermansiError::GeometryLayout {
            family: "radar",
            message: "ticks must be greater than zero".to_owned(),
        });
    }
    if requested > MAX_RADAR_TICKS {
        return Err(MermansiError::RenderLimit {
            context: "radar ticks",
            requested,
            limit: MAX_RADAR_TICKS,
        });
    }
    Ok(requested)
}

fn json_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn curve_line_char(index: usize, charset: Charset) -> &'static str {
    const UNICODE: &[&str] = &["─", "═", "┄", "┈"];
    const ASCII: &[&str] = &["-", "=", "~", ":"];
    match charset {
        Charset::Unicode => UNICODE[index % UNICODE.len()],
        Charset::Ascii => ASCII[index % ASCII.len()],
    }
}

fn format_number(value: f64) -> String {
    format!("{value:.2}")
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::Number(number) => number
            .as_f64()
            .map_or_else(|| number.to_string(), format_number),
        Value::String(text) => sanitize_label_text(text),
        other => sanitize_label_text(&other.to_string()),
    }
}
