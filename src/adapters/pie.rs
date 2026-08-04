//! Pie chart adapter — compact circular/radial terminal geometry.
//!
//! Finite nonnegative values determine proportional sectors. Invalid values never affect sector
//! weights, but remain visible as explicitly excluded rows instead of disappearing.

use crate::adapters::chart_primitives::{
    self, checked_chart_dimensions, draw_circle_outline, draw_radial_line, ensure_entity_limit,
    fill_pie_sector,
};
use crate::adapters::{align_left_display, align_right_display, format_title};
use crate::ansi::sanitize_label_text;
use crate::canvas::Canvas;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::pie::{PieDiagramRenderModel, PieRenderSection};

#[derive(Debug)]
struct LegendRow {
    marker: &'static str,
    label: String,
    value: String,
    share: String,
}

pub fn render_pie(model: &PieDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title, opts.max_width));
    ensure_entity_limit("pie sections", model.sections.len())?;

    if model.sections.is_empty() {
        out.push_str("(empty pie chart)\n");
        return Ok(out);
    }

    let plottable = model
        .sections
        .iter()
        .filter(|section| section.value.is_finite() && section.value >= 0.0)
        .collect::<Vec<_>>();
    let excluded = model
        .sections
        .iter()
        .filter(|section| !section.value.is_finite() || section.value < 0.0)
        .collect::<Vec<_>>();
    let maximum_value = plottable
        .iter()
        .map(|section| section.value)
        .fold(0.0_f64, f64::max);
    let scaled_total = if maximum_value > 0.0 {
        plottable
            .iter()
            .map(|section| section.value / maximum_value)
            .sum::<f64>()
    } else {
        0.0
    };

    if scaled_total > 0.0 {
        render_chart(&mut out, &plottable, maximum_value, scaled_total, opts)?;
    } else if plottable.is_empty() {
        out.push_str("(pie chart has no plottable values)\n\n");
    } else {
        out.push_str("(pie chart has zero total; no sectors plotted)\n\n");
    }

    let rows = plottable
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let share = section_share(section, maximum_value, scaled_total);
            LegendRow {
                marker: chart_primitives::fill_char(index, opts.charset),
                label: sanitize_label_text(&section.label),
                value: format_value(section.value),
                share: format!("{:.1}%", share * 100.0),
            }
        })
        .collect::<Vec<_>>();
    if !rows.is_empty() {
        render_legend(&mut out, &rows, model.show_data);
    }

    if !excluded.is_empty() {
        let marker = match opts.charset {
            Charset::Unicode => "×",
            Charset::Ascii => "!",
        };
        out.push_str("\nExcluded (not plotted):\n");
        for section in excluded {
            let label = sanitize_label_text(&section.label);
            out.push_str(&format!(
                "  {marker} {label} = {} ({})\n",
                format_value(section.value),
                excluded_reason(section.value)
            ));
        }
    }

    Ok(out)
}

fn render_chart(
    out: &mut String,
    sections: &[&PieRenderSection],
    maximum_value: f64,
    scaled_total: f64,
    opts: &MermansiOptions,
) -> Result<()> {
    let longest_label = sections
        .iter()
        .map(|section| str_display_width(&sanitize_label_text(&section.label)))
        .max()
        .unwrap_or(0);
    let preferred_width = (20 + sections.len().min(12))
        .max(longest_label.saturating_add(8))
        .min(36);
    let preferred_height = preferred_width.div_ceil(2);
    let (chart_width, chart_height) =
        checked_chart_dimensions(opts, (20, 10), (preferred_width, preferred_height))?;
    let radius = (chart_height / 2).saturating_sub(1).max(3) as i64;
    let center_x = (chart_width / 2) as i64;
    let center_y = (chart_height / 2) as i64;
    let mut canvas = Canvas::new(chart_width, chart_height)?;

    let mut angle = -std::f64::consts::FRAC_PI_2;
    let mut boundary_angles = Vec::with_capacity(sections.len());
    for (index, section) in sections.iter().enumerate() {
        let fraction = section_share(section, maximum_value, scaled_total);
        let end = angle + fraction * std::f64::consts::TAU;
        fill_pie_sector(
            &mut canvas,
            center_x,
            center_y,
            radius,
            angle,
            end,
            chart_primitives::fill_char(index, opts.charset),
        )?;
        boundary_angles.push(angle);
        angle = end;
    }

    let leader = match opts.charset {
        Charset::Unicode => "·",
        Charset::Ascii => ".",
    };
    for boundary in &boundary_angles {
        draw_radial_line(&mut canvas, center_x, center_y, radius, *boundary, leader)?;
    }

    let outline = match opts.charset {
        Charset::Unicode => "○",
        Charset::Ascii => "o",
    };
    draw_circle_outline(&mut canvas, center_x, center_y, radius, outline)?;
    let boundary_marker = match opts.charset {
        Charset::Unicode => "◆",
        Charset::Ascii => "+",
    };
    for boundary in boundary_angles {
        let (endpoint_x, endpoint_y) = chart_primitives::radial_point(
            center_x as f64,
            center_y as f64,
            radius as f64,
            boundary,
        );
        let endpoint_x = endpoint_x.round() as i64;
        let endpoint_y = endpoint_y.round() as i64;
        if endpoint_x >= 0
            && endpoint_y >= 0
            && (endpoint_x as usize) < canvas.width()
            && (endpoint_y as usize) < canvas.height()
        {
            canvas.set_text(endpoint_x as usize, endpoint_y as usize, boundary_marker)?;
        }
    }
    let center = match opts.charset {
        Charset::Unicode => "✛",
        Charset::Ascii => "+",
    };
    canvas.set_text(center_x as usize, center_y as usize, center)?;

    out.push_str(&chart_primitives::render_cropped_canvas(&canvas));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    Ok(())
}

fn section_share(section: &PieRenderSection, maximum_value: f64, scaled_total: f64) -> f64 {
    if maximum_value <= 0.0 || scaled_total <= 0.0 {
        0.0
    } else {
        (section.value / maximum_value) / scaled_total
    }
}

fn render_legend(out: &mut String, rows: &[LegendRow], show_data: bool) {
    let label_width = rows
        .iter()
        .map(|row| str_display_width(&row.label))
        .max()
        .unwrap_or(0)
        .max(str_display_width("Label"));
    let share_width = rows
        .iter()
        .map(|row| str_display_width(&row.share))
        .max()
        .unwrap_or(0)
        .max(str_display_width("Share"));

    if show_data {
        let value_width = rows
            .iter()
            .map(|row| str_display_width(&row.value))
            .max()
            .unwrap_or(0)
            .max(str_display_width("Value"));
        out.push_str(&format!(
            "    {} {} {}\n",
            align_left_display("Label", label_width),
            align_right_display("Value", value_width),
            align_right_display("Share", share_width),
        ));
        out.push_str(&format!(
            "  {}\n",
            "-".repeat(label_width + value_width + share_width + 4)
        ));
        for row in rows {
            out.push_str(&format!(
                "  {} {} {} {}\n",
                row.marker,
                align_left_display(&row.label, label_width),
                align_right_display(&row.value, value_width),
                align_right_display(&row.share, share_width),
            ));
        }
    } else {
        out.push_str(&format!(
            "    {} {}\n",
            align_left_display("Label", label_width),
            align_right_display("Share", share_width),
        ));
        out.push_str(&format!(
            "  {}\n",
            "-".repeat(label_width + share_width + 3)
        ));
        for row in rows {
            out.push_str(&format!(
                "  {} {} {}\n",
                row.marker,
                align_left_display(&row.label, label_width),
                align_right_display(&row.share, share_width),
            ));
        }
    }
}

fn format_value(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "inf".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-inf".to_owned()
    } else {
        format!("{value:.2}")
    }
}

fn excluded_reason(value: f64) -> &'static str {
    if value.is_nan() {
        "not a number"
    } else if value.is_infinite() {
        "not finite"
    } else {
        "negative"
    }
}
