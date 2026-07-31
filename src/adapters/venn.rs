//! Venn diagram adapter — genuine overlapping set geometry.
//!
//! Draws closed overlapping circle outlines for semantic singleton sets, places
//! intersection/subset and text labels in or connected to their associated regions, and
//! preserves sizes, labels, styles, and arbitrary parsed subsets.

use crate::adapters::chart_primitives::{
    self, MAX_CHART_ENTITIES, checked_chart_dimensions, ensure_entity_limit,
};
use crate::adapters::format_title;
use crate::ansi::sanitize_label_text;
use crate::canvas::Canvas;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::venn::VennDiagramRenderModel;

pub fn render_venn(model: &VennDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    ensure_entity_limit("venn subsets", model.subsets.len())?;
    ensure_entity_limit("venn text nodes", model.text_nodes.len())?;

    // Identify singleton sets (the base sets).
    let singletons: Vec<_> = model.subsets.iter().filter(|s| s.sets.len() == 1).collect();

    if singletons.is_empty() {
        out.push_str("(empty venn diagram)\n");
        let _ = opts;
        return Ok(out);
    }

    let (chart_w, chart_h) = checked_chart_dimensions(opts, (20, 10), (80, 50))?;

    let mut canvas = Canvas::new(chart_w, chart_h)?;

    let n_sets = singletons.len();
    let base_radius = (chart_h as f64 / 3.5).max(4.0);
    let cx = (chart_w as f64 / 2.0).round() as i64;
    let cy = (chart_h as f64 / 2.0).round() as i64;

    // Compute circle centers for each singleton set with controlled overlap.
    let set_centers = compute_set_positions(cx, cy, base_radius, n_sets);

    // Build set name → center lookup.
    let mut set_name_to_center: std::collections::BTreeMap<String, (i64, i64)> =
        std::collections::BTreeMap::new();
    for (i, subset) in singletons.iter().enumerate() {
        if let Some(set_name) = subset.sets.first() {
            let (sx, sy) = set_centers[i];
            set_name_to_center
                .entry(set_name.clone())
                .or_insert((sx, sy));
        }
    }

    // Draw all set circle outlines FIRST (before labels, so labels aren't overwritten).
    let outline_char = match opts.charset {
        Charset::Unicode => "○",
        Charset::Ascii => "o",
    };
    for (i, subset) in singletons.iter().enumerate() {
        let (sx, sy) = set_centers[i];
        let r = scale_radius(base_radius, subset.size);
        chart_primitives::draw_circle_outline(&mut canvas, sx, sy, r as i64, outline_char)?;
    }

    // Now place set labels inside their own-only regions.
    for (i, subset) in singletons.iter().enumerate() {
        let (sx, sy) = set_centers[i];
        let r = scale_radius(base_radius, subset.size);
        let label_text = sanitize_label_text(
            subset
                .label
                .as_deref()
                .unwrap_or(subset.sets.first().unwrap_or(&String::new())),
        );
        let lx = sx;
        let ly = sy - (r * 0.5).round() as i64;
        place_label_safe(&mut canvas, lx, ly, &label_text)?;
    }

    let intersection_sep = match opts.charset {
        Charset::Unicode => "∩",
        Charset::Ascii => "&",
    };

    // Place intersection/subset labels at centroid of involved set centers.
    for subset in &model.subsets {
        if subset.sets.len() < 2 {
            continue;
        }
        let centers: Vec<(i64, i64)> = subset
            .sets
            .iter()
            .filter_map(|s| set_name_to_center.get(s).copied())
            .collect();
        if centers.is_empty() {
            continue;
        }
        let sum_x: i64 = centers.iter().map(|c| c.0).sum();
        let sum_y: i64 = centers.iter().map(|c| c.1).sum();
        let centroid = (sum_x / centers.len() as i64, sum_y / centers.len() as i64);

        let label_text = sanitize_label_text(
            subset
                .label
                .as_deref()
                .unwrap_or(&subset.sets.join(intersection_sep)),
        );
        if !label_text.is_empty() {
            place_label_safe(&mut canvas, centroid.0, centroid.1, &label_text)?;
        }
    }

    // Place text_nodes in the region determined by their sets.
    for (i, node) in model.text_nodes.iter().enumerate() {
        let centers: Vec<(i64, i64)> = node
            .sets
            .iter()
            .filter_map(|s| set_name_to_center.get(s).copied())
            .collect();
        let (px, py) = if centers.is_empty() {
            (cx, cy)
        } else if centers.len() == 1 {
            let (sx, sy) = centers[0];
            (sx, sy + (base_radius * 0.3) as i64)
        } else {
            let sum_x: i64 = centers.iter().map(|c| c.0).sum();
            let sum_y: i64 = centers.iter().map(|c| c.1).sum();
            (
                sum_x / centers.len() as i64,
                sum_y / centers.len() as i64 + i as i64,
            )
        };

        let marker = chart_primitives::marker_char(i, opts.charset);
        place_marker_safe(&mut canvas, px, py, marker)?;
    }

    let chart_text = canvas.render();
    out.push_str(&chart_text);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');

    // Styles legend.
    if !model.style_entries.is_empty() {
        out.push_str("Styles:\n");
        for entry in &model.style_entries {
            let targets = entry.targets.join(",");
            let styles: Vec<String> = entry
                .styles
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            out.push_str(&format!("  {targets}: {}\n", styles.join(", ")));
        }
        out.push('\n');
    }

    // Intersection summary (preserving all parsed subsets).
    if model.subsets.iter().any(|s| s.sets.len() >= 2) {
        out.push_str("Intersections:\n");
        for subset in &model.subsets {
            if subset.sets.len() < 2 {
                continue;
            }
            let sets_str = subset.sets.join(intersection_sep);
            let label = subset
                .label
                .as_deref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {{{sets_str}}} size={:.1}{label}\n",
                subset.size
            ));
        }
        out.push('\n');
    }

    // Text nodes summary (preserving every text node).
    if !model.text_nodes.is_empty() {
        out.push_str("Text Nodes:\n");
        for node in &model.text_nodes {
            let sets_str = node.sets.join(intersection_sep);
            let label = node
                .label
                .as_deref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {{{sets_str}}} {}{label}\n",
                sanitize_label_text(&node.id)
            ));
        }
    }

    let _ = MAX_CHART_ENTITIES;
    Ok(out)
}

fn compute_set_positions(cx: i64, cy: i64, radius: f64, n: usize) -> Vec<(i64, i64)> {
    let offset = (radius * 0.6).round() as i64;
    match n {
        0 => Vec::new(),
        1 => vec![(cx, cy)],
        2 => vec![(cx - offset, cy), (cx + offset, cy)],
        3 => {
            let r = offset;
            (0..3)
                .map(|i| {
                    let angle =
                        -std::f64::consts::FRAC_PI_2 + (i as f64 / 3.0) * std::f64::consts::TAU;
                    (
                        cx + (r as f64 * angle.cos()).round() as i64,
                        cy + (r as f64 * angle.sin()).round() as i64,
                    )
                })
                .collect()
        }
        _ => {
            let r = offset.max(3);
            (0..n)
                .map(|i| {
                    let angle = (i as f64 / n as f64) * std::f64::consts::TAU;
                    (
                        cx + (r as f64 * angle.cos()).round() as i64,
                        cy + (r as f64 * angle.sin()).round() as i64,
                    )
                })
                .collect()
        }
    }
}

fn scale_radius(base: f64, size: f64) -> f64 {
    if size <= 0.0 || !size.is_finite() {
        return base;
    }
    let factor = (size / 10.0).sqrt().clamp(0.5, 2.0);
    (base * factor).max(3.0)
}

fn place_label_safe(canvas: &mut Canvas, x: i64, y: i64, text: &str) -> Result<()> {
    if x < 0 || y < 0 || text.is_empty() {
        return Ok(());
    }
    let ux = x as usize;
    let uy = y as usize;
    let label_w = str_display_width(text);
    let adjusted_x = if ux + label_w > canvas.width() {
        canvas.width().saturating_sub(label_w)
    } else {
        ux
    };
    if uy < canvas.height() && adjusted_x < canvas.width() {
        canvas.set_text(adjusted_x, uy, text)?;
    }
    Ok(())
}

fn place_marker_safe(canvas: &mut Canvas, x: i64, y: i64, glyph: &str) -> Result<()> {
    if x < 0 || y < 0 {
        return Ok(());
    }
    let (ux, uy) = (x as usize, y as usize);
    if ux < canvas.width() && uy < canvas.height() {
        canvas.set_text(ux, uy, glyph)?;
    }
    Ok(())
}
