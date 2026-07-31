//! Quadrant chart adapter — genuine Cartesian terminal geometry.
//!
//! Draws a closed plotting area with midpoint cross-axes, all four quadrant labels in their
//! actual regions, endpoint axis labels, and every point placed from its normalized x/y
//! coordinates with deterministic collision-safe markers.

use crate::adapters::chart_primitives::{
    self, MAX_CHART_ENTITIES, checked_chart_dimensions, ensure_entity_limit,
};
use crate::adapters::format_title;
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line, draw_vertical_line};
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::quadrant_chart::QuadrantChartRenderModel;

pub fn render_quadrant_chart(
    model: &QuadrantChartRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    ensure_entity_limit("quadrant points", model.points.len())?;

    // Compute chart dimensions within bounds.
    let (chart_w, chart_h) = checked_chart_dimensions(opts, (20, 10), (80, 50))?;

    let mut canvas = Canvas::new(chart_w, chart_h)?;

    let area_x = 4usize;
    let area_y = 2usize;
    let area_w = chart_w.saturating_sub(8).max(10);
    let area_h = chart_h.saturating_sub(4).max(6);
    let area_x2 = area_x + area_w - 1;
    let area_y2 = area_y + area_h - 1;
    let mid_x = area_x + area_w / 2;
    let mid_y = area_y + area_h / 2;

    // Draw closed plotting area border.
    draw_box(&mut canvas, area_x, area_y, area_w, area_h, opts.charset)?;

    // Draw midpoint cross-axes.
    let h_char = match opts.charset {
        Charset::Unicode => "─",
        Charset::Ascii => "-",
    };
    let v_char = match opts.charset {
        Charset::Unicode => "│",
        Charset::Ascii => "|",
    };
    draw_horizontal_line(&mut canvas, mid_y, area_x + 1, area_x2 - 1, opts.charset)?;
    draw_vertical_line(&mut canvas, mid_x, area_y + 1, area_y2 - 1, opts.charset)?;
    // Re-draw center cross with distinct chars
    canvas.set_text(mid_x, mid_y, "+")?;
    let _ = (h_char, v_char);

    // Place quadrant labels in their actual regions.
    // Q1 (top-right): quadrant1_text
    place_text_right(
        &mut canvas,
        mid_x + 1,
        area_y + 1,
        &model.quadrants.quadrant1_text,
    )?;
    // Q2 (top-left): quadrant2_text
    place_text_left(
        &mut canvas,
        mid_x - 1,
        area_y + 1,
        &model.quadrants.quadrant2_text,
    )?;
    // Q3 (bottom-left): quadrant3_text
    place_text_left(
        &mut canvas,
        mid_x - 1,
        area_y2 - 1,
        &model.quadrants.quadrant3_text,
    )?;
    // Q4 (bottom-right): quadrant4_text
    place_text_right(
        &mut canvas,
        mid_x + 1,
        area_y2 - 1,
        &model.quadrants.quadrant4_text,
    )?;

    // Place endpoint axis labels.
    // X-left (bottom-left corner area)
    place_axis_label(
        &mut canvas,
        area_x,
        area_y2 + 1,
        &model.axes.x_axis_left_text,
    )?;
    // X-right
    place_axis_label_right(
        &mut canvas,
        area_x2,
        area_y2 + 1,
        &model.axes.x_axis_right_text,
    )?;
    // Y-bottom (below bottom-left of area)
    place_axis_label(
        &mut canvas,
        area_x,
        area_y2 + 2,
        &model.axes.y_axis_bottom_text,
    )?;
    // Y-top (above top-left of area)
    place_axis_label(
        &mut canvas,
        area_x,
        area_y.saturating_sub(1),
        &model.axes.y_axis_top_text,
    )?;

    // Place points from normalized (x, y) coordinates.
    let mut collisions: Vec<(usize, String, f64, f64)> = Vec::new();
    for (i, point) in model.points.iter().enumerate() {
        let px = area_x as f64 + 1.0 + point.x * (area_w as f64 - 2.0);
        let py = area_y2 as f64 - point.y * (area_h as f64 - 2.0);
        let ix = px.round() as i64;
        let iy = py.round() as i64;

        if ix < 0 || iy < 0 {
            continue;
        }
        let (ux, uy) = (ix as usize, iy as usize);
        if ux < canvas.width() && uy < canvas.height() {
            // Check for collision.
            let occupied = canvas
                .get_cell(ux, uy)
                .is_some_and(|c| !c.is_empty() && c != "+");
            if occupied {
                // Try adjacent cells deterministically.
                let placed = try_place_nearby(&mut canvas, ux, uy, opts.charset, i)?;
                if !placed {
                    collisions.push((i + 1, sanitize_label_text(&point.text), point.x, point.y));
                }
            } else {
                let marker = chart_primitives::marker_char(i, opts.charset);
                canvas.set_text(ux, uy, marker)?;
            }
        }
    }

    let chart_text = canvas.render();
    out.push_str(&chart_text);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');

    // Point/class legend.
    if !model.points.is_empty() {
        out.push_str("Points:\n");
        for (i, point) in model.points.iter().enumerate() {
            let marker = chart_primitives::marker_char(i, opts.charset);
            let label = sanitize_label_text(&point.text);
            let class_str = point
                .class_name
                .as_deref()
                .map(|c| format!(" [{}]", c))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {marker} {label} ({:.2}, {:.2}){class_str}\n",
                point.x, point.y
            ));
        }
    }

    // Class definitions legend.
    if !model.classes.is_empty() {
        out.push_str("\nClasses:\n");
        for (name, styles) in &model.classes {
            out.push_str(&format!("  {name}"));
            if let Some(c) = &styles.color {
                out.push_str(&format!(" color={c}"));
            }
            if let Some(r) = styles.radius {
                out.push_str(&format!(" radius={r}"));
            }
            if let Some(sc) = &styles.stroke_color {
                out.push_str(&format!(" stroke-color={sc}"));
            }
            if let Some(sw) = &styles.stroke_width {
                out.push_str(&format!(" stroke-width={sw}"));
            }
            out.push('\n');
        }
    }

    // Collisions callout.
    if !collisions.is_empty() {
        out.push_str("\nOverlapping points:\n");
        for (num, label, x, y) in &collisions {
            out.push_str(&format!("  #{num} {label} ({x:.2}, {y:.2})\n"));
        }
    }

    let _ = MAX_CHART_ENTITIES;
    Ok(out)
}

fn place_text_right(canvas: &mut Canvas, x: usize, y: usize, text: &str) -> Result<()> {
    if y >= canvas.height() || x >= canvas.width() {
        return Ok(());
    }
    let label_w = str_display_width(text);
    let end = x + label_w;
    if end > canvas.width() {
        let truncated = truncate_to_width(text, canvas.width() - x);
        if !truncated.is_empty() {
            return canvas.set_text(x, y, &truncated);
        }
        return Ok(());
    }
    canvas.set_text(x, y, text)
}

fn place_text_left(canvas: &mut Canvas, x_right: usize, y: usize, text: &str) -> Result<()> {
    if y >= canvas.height() {
        return Ok(());
    }
    let label_w = str_display_width(text);
    if label_w == 0 {
        return Ok(());
    }
    let start = x_right.saturating_sub(label_w - 1);
    if start >= canvas.width() {
        return Ok(());
    }
    let end = start + label_w;
    if end > canvas.width() {
        let avail = canvas.width() - start;
        let truncated = truncate_to_width(text, avail);
        if !truncated.is_empty() {
            return canvas.set_text(start, y, &truncated);
        }
        return Ok(());
    }
    canvas.set_text(start, y, text)
}

fn place_axis_label(canvas: &mut Canvas, x: usize, y: usize, text: &str) -> Result<()> {
    if y >= canvas.height() || x >= canvas.width() || text.trim().is_empty() {
        return Ok(());
    }
    let label_w = str_display_width(text);
    let avail = canvas.width().saturating_sub(x);
    if label_w <= avail {
        canvas.set_text(x, y, text)
    } else {
        let truncated = truncate_to_width(text, avail);
        if !truncated.is_empty() {
            canvas.set_text(x, y, &truncated)
        } else {
            Ok(())
        }
    }
}

fn place_axis_label_right(canvas: &mut Canvas, x_right: usize, y: usize, text: &str) -> Result<()> {
    if y >= canvas.height() || text.trim().is_empty() {
        return Ok(());
    }
    let label_w = str_display_width(text);
    if label_w == 0 {
        return Ok(());
    }
    let start = x_right.saturating_sub(label_w - 1);
    if start >= canvas.width() {
        return Ok(());
    }
    canvas.set_text(start, y, text)
}

fn truncate_to_width(text: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let cw = if ch.is_control() {
            0
        } else {
            unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)
        };
        if width + cw > max_w {
            break;
        }
        result.push(ch);
        width += cw;
    }
    result
}

fn try_place_nearby(
    canvas: &mut Canvas,
    cx: usize,
    cy: usize,
    charset: Charset,
    idx: usize,
) -> Result<bool> {
    let marker = chart_primitives::marker_char(idx, charset);
    // Spiral search for nearest free cell.
    let offsets: [(i64, i64); 8] = [
        (0, -1),
        (1, 0),
        (0, 1),
        (-1, 0),
        (-1, -1),
        (1, -1),
        (1, 1),
        (-1, 1),
    ];
    for radius in 1..=5 {
        for &(dx, dy) in &offsets {
            let nx = cx as i64 + dx * radius;
            let ny = cy as i64 + dy * radius;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (ux, uy) = (nx as usize, ny as usize);
            if ux < canvas.width()
                && uy < canvas.height()
                && canvas.get_cell(ux, uy).is_none_or(|c| c.is_empty())
            {
                canvas.set_text(ux, uy, marker)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}
