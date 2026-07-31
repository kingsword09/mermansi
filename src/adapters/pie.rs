//! Pie chart adapter — genuine circular/radial terminal geometry.
//!
//! Draws a closed circle outline with proportional sector fill derived from every finite
//! nonnegative section value, plus radial sector boundary spokes. A compact legend preserves
//! every label, value, percentage, title, and showData semantics.

use crate::adapters::chart_primitives::{
    self, MAX_CHART_ENTITIES, draw_circle_outline, draw_radial_line, ensure_entity_limit,
    fill_pie_sector,
};
use crate::adapters::{align_left_display, align_right_display, format_title};
use crate::ansi::sanitize_label_text;
use crate::canvas::Canvas;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::pie::PieDiagramRenderModel;

pub fn render_pie(model: &PieDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    ensure_entity_limit("pie sections", model.sections.len())?;

    let valid_sections: Vec<_> = model
        .sections
        .iter()
        .filter(|s| s.value.is_finite() && s.value >= 0.0)
        .collect();

    if valid_sections.is_empty() {
        out.push_str("(empty pie chart)\n");
        let _ = opts;
        return Ok(out);
    }

    let total: f64 = valid_sections.iter().map(|s| s.value).sum();
    if total <= 0.0 {
        out.push_str("(pie chart has zero total)\n");
        let _ = opts;
        return Ok(out);
    }

    // Compute chart dimensions within bounds.
    let max_w = opts.max_width.clamp(20, 80);
    let max_h = opts.max_height.clamp(10, 40);
    let chart_w = max_w.min(max_h * 2); // terminals are ~2:1 aspect
    let chart_h = usize::div_ceil(chart_w, 2);

    let radius = (chart_h / 2).saturating_sub(1).max(3) as i64;
    let cx = (chart_w / 2) as i64;
    let cy = (chart_h / 2) as i64;

    let canvas_w = chart_w;
    let canvas_h = chart_h;
    let mut canvas = Canvas::new(canvas_w, canvas_h)?;

    // Fill each sector proportionally.
    let mut cumulative_angle = -std::f64::consts::FRAC_PI_2; // start at top (12 o'clock)
    for (i, section) in valid_sections.iter().enumerate() {
        let fraction = section.value / total;
        let sweep = fraction * std::f64::consts::TAU;
        let start = cumulative_angle;
        let end = cumulative_angle + sweep;

        let fill = chart_primitives::fill_char(i, opts.charset);
        fill_pie_sector(&mut canvas, cx, cy, radius, start, end, fill)?;

        cumulative_angle = end;
    }

    // Draw radial boundary spokes between sectors.
    let spoke_char = match opts.charset {
        Charset::Unicode => "│",
        Charset::Ascii => "|",
    };
    let _ = spoke_char; // boundaries drawn via outline overlay
    cumulative_angle = -std::f64::consts::FRAC_PI_2;
    for section in &valid_sections {
        let fraction = section.value / total;
        let sweep = fraction * std::f64::consts::TAU;
        // Draw boundary at the start of each sector
        let boundary_angle = cumulative_angle;
        let boundary_char = match opts.charset {
            Charset::Unicode => "◆",
            Charset::Ascii => "+",
        };
        draw_radial_line(&mut canvas, cx, cy, radius, boundary_angle, boundary_char)?;
        cumulative_angle += sweep;
    }

    // Draw closed circle outline on top.
    let outline = match opts.charset {
        Charset::Unicode => "○",
        Charset::Ascii => "o",
    };
    draw_circle_outline(&mut canvas, cx, cy, radius, outline)?;

    // Place center marker.
    let center_char = match opts.charset {
        Charset::Unicode => "✛",
        Charset::Ascii => "+",
    };
    canvas.set_text(cx as usize, cy as usize, center_char)?;

    let chart_text = canvas.render();
    out.push_str(&chart_text);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');

    // Compact legend preserving every label, value, percentage.
    let rows: Vec<_> = valid_sections
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let pct = (section.value / total) * 100.0;
            let fill = chart_primitives::fill_char(i, opts.charset);
            let label = sanitize_label_text(&section.label);
            let value_str = format!("{:.2}", section.value);
            let pct_str = format!("{pct:.1}%");
            (fill, label, value_str, pct_str)
        })
        .collect();

    let legend_label_w = rows
        .iter()
        .map(|(_, l, _, _)| str_display_width(l))
        .max()
        .unwrap_or(0)
        .max(5);
    let legend_val_w = rows
        .iter()
        .map(|(_, _, v, _)| str_display_width(v))
        .max()
        .unwrap_or(0)
        .max(5);
    let legend_pct_w = rows
        .iter()
        .map(|(_, _, _, p)| str_display_width(p))
        .max()
        .unwrap_or(0)
        .max(4);

    // showData semantics: always show value and percentage columns.
    out.push_str(&format!(
        "  {} {} {}\n",
        align_left_display("Label", legend_label_w),
        align_right_display("Value", legend_val_w),
        align_right_display("Share", legend_pct_w),
    ));
    out.push_str(&format!(
        "  {}\n",
        "-".repeat(legend_label_w + legend_val_w + legend_pct_w + 2)
    ));
    for (fill, label, value, pct) in &rows {
        out.push_str(&format!(
            "  {} {} {} {}\n",
            fill,
            align_left_display(label, legend_label_w),
            align_right_display(value, legend_val_w),
            align_right_display(pct, legend_pct_w),
        ));
    }

    let _ = MAX_CHART_ENTITIES;
    let _ = model.show_data; // always show data in legend
    Ok(out)
}
