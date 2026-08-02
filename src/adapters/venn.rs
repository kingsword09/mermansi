//! Venn diagram adapter — genuine overlapping set geometry.
//!
//! Draws closed overlapping set ellipses, places every subset and text-node label inside its
//! associated region when space permits, and connects overflow labels to deterministic callouts.

use crate::adapters::chart_primitives::{self, checked_chart_dimensions, ensure_entity_limit};
use crate::adapters::format_title;
use crate::ansi::sanitize_label_text;
use crate::canvas::Canvas;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::venn::{VennDiagramRenderModel, VennSubsetRenderModel};
use std::collections::{BTreeMap, BTreeSet};

const LABEL_CLEARANCE_X: usize = 2;
const LABEL_CLEARANCE_Y: usize = 1;

#[derive(Clone, Debug)]
struct SetCircle {
    name: String,
    cx: i64,
    cy: i64,
    rx: i64,
    ry: i64,
}

#[derive(Clone, Debug)]
struct RegionLabel {
    sets: Vec<String>,
    text: String,
    order: usize,
}

#[derive(Clone, Debug)]
struct PendingCallout {
    marker: &'static str,
    text: String,
    anchor: (usize, usize),
    order: usize,
}

pub fn render_venn(model: &VennDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    ensure_entity_limit("venn subsets", model.subsets.len())?;
    ensure_entity_limit("venn text nodes", model.text_nodes.len())?;

    let mut seen_sets = BTreeSet::new();
    let singletons: Vec<_> = model
        .subsets
        .iter()
        .filter(|subset| {
            subset
                .sets
                .first()
                .is_some_and(|name| subset.sets.len() == 1 && seen_sets.insert(name.clone()))
        })
        .collect();
    if singletons.is_empty() {
        out.push_str("(empty venn diagram)\n");
        return Ok(out);
    }

    let preferred_height = match singletons.len() {
        1 => 18,
        2 => 24,
        3 => 30,
        _ => 32,
    };
    let preferred_width = (preferred_height * 2).min(64);
    let (chart_width, chart_height) =
        checked_chart_dimensions(opts, (20, 10), (preferred_width, preferred_height))?;
    let mut canvas = Canvas::new(chart_width, chart_height)?;
    let circles = build_set_circles(&singletons, chart_width, chart_height);
    let circle_lookup = circles
        .iter()
        .enumerate()
        .map(|(index, circle)| (circle.name.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let outline = match opts.charset {
        Charset::Unicode => "○",
        Charset::Ascii => "o",
    };
    for circle in &circles {
        draw_ellipse_outline(&mut canvas, circle, outline)?;
    }

    let intersection_separator = match opts.charset {
        Charset::Unicode => "∩",
        Charset::Ascii => "&",
    };
    let mut labels = collect_region_labels(model, intersection_separator);
    labels.sort_by(|left, right| {
        right
            .sets
            .len()
            .cmp(&left.sets.len())
            .then_with(|| left.order.cmp(&right.order))
    });

    let mut pending_callouts = Vec::new();
    let mut fallback_callouts = Vec::new();
    let mut callout_index = 0usize;
    for label in &labels {
        if label.text.is_empty()
            || place_region_label(&mut canvas, &circles, &circle_lookup, label)?
        {
            continue;
        }

        let marker = chart_primitives::marker_char(callout_index, opts.charset);
        if let Some(anchor) = find_region_point(&canvas, &circles, &circle_lookup, &label.sets) {
            canvas.set_text(anchor.0, anchor.1, marker)?;
            pending_callouts.push(PendingCallout {
                marker,
                text: label.text.clone(),
                anchor,
                order: label.order,
            });
        } else {
            place_region_marker(&mut canvas, &circles, &circle_lookup, &label.sets, marker)?;
            fallback_callouts.push((marker, label.text.clone()));
        }
        callout_index += 1;
    }
    fallback_callouts.extend(place_connected_callouts(
        &mut canvas,
        pending_callouts,
        opts.charset,
    )?);

    out.push_str(&chart_primitives::render_cropped_canvas(&canvas));
    if !out.ends_with('\n') {
        out.push('\n');
    }

    if !fallback_callouts.is_empty() {
        out.push_str("\nCallouts:\n");
        for (marker, label) in &fallback_callouts {
            out.push_str(&format!("  {marker} {label}\n"));
        }
    }

    if !model.style_entries.is_empty() {
        out.push_str("\nStyles:\n");
        for entry in &model.style_entries {
            let targets = entry.targets.join(",");
            let styles = entry
                .styles
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>();
            out.push_str(&format!("  {targets}: {}\n", styles.join(", ")));
        }
    }

    if model.subsets.iter().any(|subset| subset.sets.len() >= 2) {
        out.push_str("\nIntersections:\n");
        for subset in &model.subsets {
            if subset.sets.len() < 2 {
                continue;
            }
            let sets = subset.sets.join(intersection_separator);
            let label = subset
                .label
                .as_deref()
                .map(sanitize_label_text)
                .filter(|label| !label.is_empty())
                .map(|label| format!(": {label}"))
                .unwrap_or_default();
            out.push_str(&format!("  {{{sets}}} size={:.1}{label}\n", subset.size));
        }
    }

    if !model.text_nodes.is_empty() {
        out.push_str("\nText Nodes:\n");
        for node in &model.text_nodes {
            let sets = node.sets.join(intersection_separator);
            let id = sanitize_label_text(&node.id);
            let label = node
                .label
                .as_deref()
                .map(sanitize_label_text)
                .filter(|label| !label.is_empty())
                .map(|label| format!(": {label}"))
                .unwrap_or_default();
            out.push_str(&format!("  {{{sets}}} {id}{label}\n"));
        }
    }

    Ok(out)
}

fn collect_region_labels(model: &VennDiagramRenderModel, separator: &str) -> Vec<RegionLabel> {
    let mut labels = Vec::with_capacity(model.subsets.len() + model.text_nodes.len());
    for (order, subset) in model.subsets.iter().enumerate() {
        let default = subset.sets.join(separator);
        let text = sanitize_label_text(subset.label.as_deref().unwrap_or(&default));
        labels.push(RegionLabel {
            sets: subset.sets.clone(),
            text,
            order,
        });
    }
    let offset = labels.len();
    for (index, node) in model.text_nodes.iter().enumerate() {
        let id = sanitize_label_text(&node.id);
        let label = node
            .label
            .as_deref()
            .map(sanitize_label_text)
            .filter(|label| !label.is_empty());
        let text = match label {
            Some(label) if id.is_empty() => label,
            Some(label) if label == id => id,
            Some(label) => format!("{id}: {label}"),
            None => id,
        };
        labels.push(RegionLabel {
            sets: node.sets.clone(),
            text,
            order: offset + index,
        });
    }
    labels
}

fn build_set_circles(
    singletons: &[&VennSubsetRenderModel],
    width: usize,
    height: usize,
) -> Vec<SetCircle> {
    let base_ry = (height as f64 / 3.6).clamp(3.0, 6.0);
    let base_rx = base_ry * 2.0;
    let count = singletons.len();
    let mut drafts = singletons
        .iter()
        .enumerate()
        .map(|(index, subset)| {
            let (cx, cy) = relative_set_center(index, count, base_rx, base_ry);
            let factor = size_factor(subset.size);
            (
                subset.sets[0].clone(),
                cx,
                cy,
                base_rx * factor,
                base_ry * factor,
            )
        })
        .collect::<Vec<_>>();

    let min_x = drafts
        .iter()
        .map(|(_, cx, _, rx, _)| cx - rx)
        .fold(f64::INFINITY, f64::min);
    let max_x = drafts
        .iter()
        .map(|(_, cx, _, rx, _)| cx + rx)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = drafts
        .iter()
        .map(|(_, _, cy, _, ry)| cy - ry)
        .fold(f64::INFINITY, f64::min);
    let max_y = drafts
        .iter()
        .map(|(_, _, cy, _, ry)| cy + ry)
        .fold(f64::NEG_INFINITY, f64::max);
    let scale = ((width.saturating_sub(4)) as f64 / (max_x - min_x).max(1.0))
        .min((height.saturating_sub(2)) as f64 / (max_y - min_y).max(1.0))
        .min(1.0);
    for (_, cx, cy, rx, ry) in &mut drafts {
        *cx *= scale;
        *cy *= scale;
        *rx *= scale;
        *ry *= scale;
    }

    let rounded = drafts
        .into_iter()
        .map(|(name, cx, cy, rx, ry)| SetCircle {
            name,
            cx: cx.round() as i64,
            cy: cy.round() as i64,
            rx: (rx.round() as i64).max(3),
            ry: (ry.round() as i64).max(2),
        })
        .collect::<Vec<_>>();
    let geometry_min_x = rounded
        .iter()
        .map(|circle| circle.cx - circle.rx)
        .min()
        .unwrap_or(0);
    let geometry_min_y = rounded
        .iter()
        .map(|circle| circle.cy - circle.ry)
        .min()
        .unwrap_or(0);
    let geometry_max_y = rounded
        .iter()
        .map(|circle| circle.cy + circle.ry)
        .max()
        .unwrap_or(0);
    let geometry_height = (geometry_max_y - geometry_min_y + 1).max(1) as usize;
    // Keep the geometry left-aligned inside the allocation so overflow labels have a dedicated
    // right-side callout lane. The final renderer crops the unused margin.
    let shift_x = 1;
    let shift_y = height.saturating_sub(geometry_height) / 2;

    rounded
        .into_iter()
        .map(|mut circle| {
            circle.cx = circle.cx - geometry_min_x + shift_x as i64;
            circle.cy = circle.cy - geometry_min_y + shift_y as i64;
            circle
        })
        .collect()
}

fn relative_set_center(index: usize, count: usize, rx: f64, ry: f64) -> (f64, f64) {
    match count {
        0 | 1 => (0.0, 0.0),
        2 => {
            let direction = if index == 0 { -1.0 } else { 1.0 };
            (direction * rx * 0.55, 0.0)
        }
        3 => match index {
            0 => (0.0, -ry * 0.6),
            1 => (-rx * 0.48, ry * 0.35),
            _ => (rx * 0.48, ry * 0.35),
        },
        _ => {
            let angle = -std::f64::consts::FRAC_PI_2
                + (index as f64 / count as f64) * std::f64::consts::TAU;
            (rx * 0.65 * angle.cos(), ry * 0.65 * angle.sin())
        }
    }
}

fn size_factor(size: f64) -> f64 {
    if size <= 0.0 || !size.is_finite() {
        1.0
    } else {
        (size / 10.0).sqrt().clamp(0.65, 1.35)
    }
}

fn draw_ellipse_outline(canvas: &mut Canvas, circle: &SetCircle, glyph: &str) -> Result<()> {
    for dy in -circle.ry..=circle.ry {
        let ratio = dy as f64 / circle.ry as f64;
        let dx = (circle.rx as f64 * (1.0 - ratio * ratio).max(0.0).sqrt()).round() as i64;
        plot_ellipse_point(canvas, circle.cx - dx, circle.cy + dy, glyph)?;
        plot_ellipse_point(canvas, circle.cx + dx, circle.cy + dy, glyph)?;
    }
    for dx in -circle.rx..=circle.rx {
        let ratio = dx as f64 / circle.rx as f64;
        let dy = (circle.ry as f64 * (1.0 - ratio * ratio).max(0.0).sqrt()).round() as i64;
        plot_ellipse_point(canvas, circle.cx + dx, circle.cy - dy, glyph)?;
        plot_ellipse_point(canvas, circle.cx + dx, circle.cy + dy, glyph)?;
    }
    Ok(())
}

fn plot_ellipse_point(canvas: &mut Canvas, x: i64, y: i64, glyph: &str) -> Result<()> {
    if x >= 0 && y >= 0 && (x as usize) < canvas.width() && (y as usize) < canvas.height() {
        canvas.set_text(x as usize, y as usize, glyph)?;
    }
    Ok(())
}

fn place_region_label(
    canvas: &mut Canvas,
    circles: &[SetCircle],
    lookup: &BTreeMap<String, usize>,
    label: &RegionLabel,
) -> Result<bool> {
    let Some((x, y)) = find_region_span(canvas, circles, lookup, &label.sets, &label.text) else {
        return Ok(false);
    };
    canvas.set_text(x, y, &label.text)?;
    Ok(true)
}

fn find_region_span(
    canvas: &Canvas,
    circles: &[SetCircle],
    lookup: &BTreeMap<String, usize>,
    sets: &[String],
    text: &str,
) -> Option<(usize, usize)> {
    let width = str_display_width(text);
    if width == 0 || width > canvas.width() {
        return None;
    }
    let required = required_circle_indices(sets, lookup)?;
    let (target_x, target_y) = region_center(circles, &required, canvas);
    for exact in [true, false] {
        if !exact && required.len() <= 1 {
            continue;
        }
        let mut candidates = Vec::new();
        for y in 0..canvas.height() {
            for x in 0..=canvas.width() - width {
                if span_has_clear_margin(canvas, x, y, width)
                    && span_matches_region(circles, &required, x, y, width, exact)
                {
                    let center_x = x as i64 + width as i64 / 2;
                    let distance = (center_x - target_x).abs() + (y as i64 - target_y).abs();
                    candidates.push((distance, y, x));
                }
            }
        }
        candidates.sort_unstable();
        if let Some((_, y, x)) = candidates.first().copied() {
            return Some((x, y));
        }
    }
    None
}

fn place_connected_callouts(
    canvas: &mut Canvas,
    mut callouts: Vec<PendingCallout>,
    charset: Charset,
) -> Result<Vec<(&'static str, String)>> {
    if callouts.is_empty() {
        return Ok(Vec::new());
    }

    callouts.sort_by_key(|callout| (callout.anchor.1, callout.anchor.0, callout.order));
    let maximum_width = callouts
        .iter()
        .map(|callout| str_display_width(&format!("{}- {}", callout.marker, callout.text)))
        .max()
        .unwrap_or(0);
    let geometry_right = rightmost_occupied_column(canvas).unwrap_or(0);
    let callout_x = geometry_right.saturating_add(3);
    if callouts.len() > canvas.height() || callout_x.saturating_add(maximum_width) > canvas.width()
    {
        return Ok(callouts
            .into_iter()
            .map(|callout| (callout.marker, callout.text))
            .collect());
    }

    let mut rows = Vec::with_capacity(callouts.len());
    for (index, callout) in callouts.iter().enumerate() {
        let minimum_row = rows.last().map_or(index, |previous| previous + 1);
        let maximum_row = canvas.height() - (callouts.len() - index);
        rows.push(callout.anchor.1.clamp(minimum_row, maximum_row));
    }
    let all_labels_fit = callouts.iter().zip(&rows).all(|(callout, row)| {
        let text = format!("{}- {}", callout.marker, callout.text);
        span_is_clear(canvas, callout_x, *row, str_display_width(&text))
    });
    if !all_labels_fit {
        return Ok(callouts
            .into_iter()
            .map(|callout| (callout.marker, callout.text))
            .collect());
    }

    let connector = match charset {
        Charset::Unicode => "·",
        Charset::Ascii => ".",
    };
    for callout in &callouts {
        let leader_end = callout
            .anchor
            .0
            .saturating_add(3)
            .min(callout_x.saturating_sub(1));
        chart_primitives::draw_line(
            canvas,
            callout.anchor.0 as i64,
            callout.anchor.1 as i64,
            leader_end as i64,
            callout.anchor.1 as i64,
            connector,
        )?;
    }
    for (callout, row) in callouts.iter().zip(rows) {
        let separator = match charset {
            Charset::Unicode => "─",
            Charset::Ascii => "-",
        };
        let text = format!("{}{separator} {}", callout.marker, callout.text);
        canvas.set_text(callout_x, row, &text)?;
    }
    Ok(Vec::new())
}

fn place_region_marker(
    canvas: &mut Canvas,
    circles: &[SetCircle],
    lookup: &BTreeMap<String, usize>,
    sets: &[String],
    marker: &str,
) -> Result<()> {
    if let Some((x, y)) = find_region_point(canvas, circles, lookup, sets) {
        canvas.set_text(x, y, marker)?;
    }
    Ok(())
}

fn find_region_point(
    canvas: &Canvas,
    circles: &[SetCircle],
    lookup: &BTreeMap<String, usize>,
    sets: &[String],
) -> Option<(usize, usize)> {
    let required = required_circle_indices(sets, lookup)?;
    let (target_x, target_y) = region_center(circles, &required, canvas);
    for exact in [true, false] {
        if !exact && required.len() <= 1 {
            continue;
        }
        let mut candidates = Vec::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                if cell_is_clear(canvas, x, y)
                    && point_matches_region(circles, &required, x, y, exact, 0.78)
                {
                    let distance = (x as i64 - target_x).abs() + (y as i64 - target_y).abs();
                    candidates.push((distance, y, x));
                }
            }
        }
        candidates.sort_unstable();
        if let Some((_, y, x)) = candidates.first().copied() {
            return Some((x, y));
        }
    }
    None
}

fn required_circle_indices(
    sets: &[String],
    lookup: &BTreeMap<String, usize>,
) -> Option<Vec<usize>> {
    let mut required = sets
        .iter()
        .map(|set| lookup.get(set).copied())
        .collect::<Option<Vec<_>>>()?;
    required.sort_unstable();
    required.dedup();
    Some(required)
}

fn region_center(circles: &[SetCircle], required: &[usize], canvas: &Canvas) -> (i64, i64) {
    if required.is_empty() {
        return ((canvas.width() / 2) as i64, (canvas.height() / 2) as i64);
    }
    let x = required.iter().map(|index| circles[*index].cx).sum::<i64>() / required.len() as i64;
    let y = required.iter().map(|index| circles[*index].cy).sum::<i64>() / required.len() as i64;
    (x, y)
}

fn span_matches_region(
    circles: &[SetCircle],
    required: &[usize],
    x: usize,
    y: usize,
    width: usize,
    exact: bool,
) -> bool {
    (x..x + width).all(|column| point_matches_region(circles, required, column, y, exact, 0.82))
}

fn point_matches_region(
    circles: &[SetCircle],
    required: &[usize],
    x: usize,
    y: usize,
    exact: bool,
    inset: f64,
) -> bool {
    circles.iter().enumerate().all(|(index, circle)| {
        let inside = ellipse_contains(circle, x as f64, y as f64, inset);
        if required.binary_search(&index).is_ok() {
            inside
        } else {
            !exact || !ellipse_contains(circle, x as f64, y as f64, 0.92)
        }
    })
}

fn ellipse_contains(circle: &SetCircle, x: f64, y: f64, inset: f64) -> bool {
    let rx = (circle.rx as f64 * inset).max(1.0);
    let ry = (circle.ry as f64 * inset).max(1.0);
    let dx = (x - circle.cx as f64) / rx;
    let dy = (y - circle.cy as f64) / ry;
    dx * dx + dy * dy <= 1.0
}

fn span_is_clear(canvas: &Canvas, x: usize, y: usize, width: usize) -> bool {
    (x..x + width).all(|column| cell_is_clear(canvas, column, y))
}

fn span_has_clear_margin(canvas: &Canvas, x: usize, y: usize, width: usize) -> bool {
    let start_x = x.saturating_sub(LABEL_CLEARANCE_X);
    let end_x = x
        .saturating_add(width)
        .saturating_add(LABEL_CLEARANCE_X)
        .min(canvas.width());
    let start_y = y.saturating_sub(LABEL_CLEARANCE_Y);
    let end_y = y
        .saturating_add(LABEL_CLEARANCE_Y)
        .min(canvas.height().saturating_sub(1));
    (start_y..=end_y).all(|row| (start_x..end_x).all(|column| cell_is_clear(canvas, column, row)))
}

fn rightmost_occupied_column(canvas: &Canvas) -> Option<usize> {
    (0..canvas.width())
        .rev()
        .find(|column| (0..canvas.height()).any(|row| !cell_is_clear(canvas, *column, row)))
}

fn cell_is_clear(canvas: &Canvas, x: usize, y: usize) -> bool {
    canvas.get_cell(x, y).is_some_and(str::is_empty) && canvas.continuation_owner(x, y).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(name: &str, cx: i64, cy: i64) -> SetCircle {
        SetCircle {
            name: name.to_owned(),
            cx,
            cy,
            rx: 8,
            ry: 4,
        }
    }

    #[test]
    fn exact_region_membership_requires_every_requested_set() {
        let circles = vec![circle("A", 8, 5), circle("B", 14, 5)];
        let both = vec![0, 1];
        assert!(point_matches_region(&circles, &both, 11, 5, true, 0.8));
        assert!(!point_matches_region(&circles, &both, 5, 5, true, 0.8));

        let only_a = vec![0];
        assert!(point_matches_region(&circles, &only_a, 4, 5, true, 0.8));
        assert!(!point_matches_region(&circles, &only_a, 11, 5, true, 0.8));
    }

    #[test]
    fn cropped_canvas_removes_outer_blank_rows_and_indent() {
        let mut canvas = Canvas::new(20, 8).expect("canvas");
        canvas.set_text(5, 3, "A").expect("label");
        canvas.set_text(7, 4, "B").expect("label");
        assert_eq!(chart_primitives::render_cropped_canvas(&canvas), "A\n  B\n");
    }

    #[test]
    fn label_clearance_checks_neighboring_outline_rows() {
        let mut canvas = Canvas::new(20, 8).expect("canvas");
        canvas.set_text(8, 2, "○").expect("outline");

        assert!(!span_has_clear_margin(&canvas, 7, 3, 4));
        assert!(span_has_clear_margin(&canvas, 7, 5, 4));
    }
}
