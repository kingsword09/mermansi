//! Quadrant chart adapter — compact Cartesian terminal geometry.
//!
//! Valid normalized points are plotted inside a closed midpoint-cross box. Invalid coordinates,
//! collisions, labels that cannot fit, and all local/class style metadata remain explicit in
//! deterministic callouts rather than being truncated or silently dropped.

use crate::adapters::chart_primitives::{self, checked_chart_dimensions, ensure_entity_limit};
use crate::adapters::format_title;
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line, draw_vertical_line};
use crate::error::Result;
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::quadrant_chart::{
    QuadrantChartPointModel, QuadrantChartRenderModel, QuadrantChartStyles,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
struct PlotArea {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl PlotArea {
    fn right(self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(self) -> usize {
        self.y + self.height - 1
    }

    fn middle_x(self) -> usize {
        self.x + self.width / 2
    }

    fn middle_y(self) -> usize {
        self.y + self.height / 2
    }
}

#[derive(Clone, Debug)]
struct PointPlacement {
    note: Option<&'static str>,
}

type OverlapGroups = BTreeMap<(usize, usize), Vec<usize>>;

pub fn render_quadrant_chart(
    model: &QuadrantChartRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));
    ensure_entity_limit("quadrant points", model.points.len())?;

    let quadrant_labels = [
        sanitize_label_text(&model.quadrants.quadrant1_text),
        sanitize_label_text(&model.quadrants.quadrant2_text),
        sanitize_label_text(&model.quadrants.quadrant3_text),
        sanitize_label_text(&model.quadrants.quadrant4_text),
    ];
    let left_width =
        str_display_width(&quadrant_labels[1]).max(str_display_width(&quadrant_labels[2]));
    let right_width =
        str_display_width(&quadrant_labels[0]).max(str_display_width(&quadrant_labels[3]));
    let x_axis_width = str_display_width(&sanitize_label_text(&model.axes.x_axis_left_text))
        .saturating_add(str_display_width(&sanitize_label_text(
            &model.axes.x_axis_right_text,
        )))
        .saturating_add(3);
    let desired_width = 36usize
        .max(
            left_width
                .max(right_width)
                .saturating_mul(2)
                .saturating_add(5),
        )
        .max(x_axis_width.saturating_add(2))
        .min(48);
    let desired_height = (14 + model.points.len().min(8).div_ceil(2)).clamp(16, 24);
    let (chart_width, chart_height) =
        checked_chart_dimensions(opts, (20, 10), (desired_width, desired_height))?;
    let mut canvas = Canvas::new(chart_width, chart_height)?;
    let area = PlotArea {
        x: 1,
        y: 1,
        width: chart_width.saturating_sub(2).max(10),
        height: chart_height.saturating_sub(4).max(6),
    };

    draw_box(
        &mut canvas,
        area.x,
        area.y,
        area.width,
        area.height,
        opts.charset,
    )?;
    draw_horizontal_line(
        &mut canvas,
        area.middle_y(),
        area.x + 1,
        area.right() - 1,
        opts.charset,
    )?;
    draw_vertical_line(
        &mut canvas,
        area.middle_x(),
        area.y + 1,
        area.bottom() - 1,
        opts.charset,
    )?;
    canvas.set_text(area.middle_x(), area.middle_y(), "+")?;

    let (placements, overlap_groups) = place_points(&mut canvas, area, &model.points, opts)?;

    let mut label_callouts = Vec::new();
    let top_row = area.y + 1;
    let bottom_row = area.bottom() - 1;
    let left_start = area.x + 1;
    let left_end = area.middle_x().saturating_sub(1);
    let right_start = area.middle_x() + 1;
    let right_end = area.right().saturating_sub(1);
    for (key, text, start, end, row, align_right) in [
        (
            "Q1",
            &quadrant_labels[0],
            right_start,
            right_end,
            top_row,
            false,
        ),
        (
            "Q2",
            &quadrant_labels[1],
            left_start,
            left_end,
            top_row,
            true,
        ),
        (
            "Q3",
            &quadrant_labels[2],
            left_start,
            left_end,
            bottom_row,
            true,
        ),
        (
            "Q4",
            &quadrant_labels[3],
            right_start,
            right_end,
            bottom_row,
            false,
        ),
    ] {
        if !text.is_empty() && !place_full_label(&mut canvas, start, end, row, text, align_right)? {
            label_callouts.push((key, text.clone()));
        }
    }

    let x_axis_row = area.bottom() + 1;
    let y_bottom_row = area.bottom() + 2;
    let axis_labels = [
        ("x-left", sanitize_label_text(&model.axes.x_axis_left_text)),
        (
            "x-right",
            sanitize_label_text(&model.axes.x_axis_right_text),
        ),
        (
            "y-bottom",
            sanitize_label_text(&model.axes.y_axis_bottom_text),
        ),
        ("y-top", sanitize_label_text(&model.axes.y_axis_top_text)),
    ];
    if !axis_labels[0].1.is_empty()
        && !place_label_at(&mut canvas, area.x, x_axis_row, &axis_labels[0].1)?
    {
        label_callouts.push(axis_labels[0].clone());
    }
    let right_axis_start = area
        .right()
        .saturating_add(1)
        .saturating_sub(str_display_width(&axis_labels[1].1));
    if !axis_labels[1].1.is_empty()
        && !place_label_at(&mut canvas, right_axis_start, x_axis_row, &axis_labels[1].1)?
    {
        label_callouts.push(axis_labels[1].clone());
    }
    if !axis_labels[2].1.is_empty()
        && !place_label_at(&mut canvas, area.x, y_bottom_row, &axis_labels[2].1)?
    {
        label_callouts.push(axis_labels[2].clone());
    }
    if !axis_labels[3].1.is_empty()
        && !place_label_at(&mut canvas, area.x, area.y - 1, &axis_labels[3].1)?
    {
        label_callouts.push(axis_labels[3].clone());
    }

    out.push_str(&chart_primitives::render_cropped_canvas(&canvas));
    if !out.ends_with('\n') {
        out.push('\n');
    }

    if !label_callouts.is_empty() {
        out.push_str("\nLabels:\n");
        for (key, text) in &label_callouts {
            out.push_str(&format!("  {key}: {text}\n"));
        }
    }

    if !model.points.is_empty() {
        out.push_str("\nPoints:\n");
        for (index, (point, placement)) in model.points.iter().zip(&placements).enumerate() {
            let marker = chart_primitives::marker_char(index, opts.charset);
            let label = sanitize_label_text(&point.text);
            let class = point
                .class_name
                .as_deref()
                .map(sanitize_label_text)
                .filter(|class| !class.is_empty())
                .map(|class| format!(" [{class}]"))
                .unwrap_or_default();
            let styles = format_styles(&point.styles);
            out.push_str(&format!(
                "  {marker} {label} ({}, {}){class}\n",
                format_coordinate(point.x),
                format_coordinate(point.y),
            ));
            if !styles.is_empty() {
                out.push_str(&format!("    styles{styles}\n"));
            }
            if let Some(note) = placement.note {
                out.push_str(&format!("    status: {note}\n"));
            }
        }
    }

    if !model.classes.is_empty() {
        out.push_str("\nClasses:\n");
        for (name, styles) in &model.classes {
            let name = sanitize_label_text(name);
            out.push_str(&format!("  {name}{}\n", format_styles(styles)));
        }
    }

    if !overlap_groups.is_empty() {
        out.push_str("\nOverlapping points:\n");
        for ((column, row), indices) in overlap_groups {
            let points = indices
                .into_iter()
                .map(|index| {
                    format!(
                        "{} {}",
                        chart_primitives::marker_char(index, opts.charset),
                        sanitize_label_text(&model.points[index].text)
                    )
                })
                .collect::<Vec<_>>();
            out.push_str(&format!("  cell({column},{row}): {}\n", points.join(", ")));
        }
    }

    Ok(out)
}

fn place_points(
    canvas: &mut Canvas,
    area: PlotArea,
    points: &[QuadrantChartPointModel],
    opts: &MermansiOptions,
) -> Result<(Vec<PointPlacement>, OverlapGroups)> {
    let mut placements = Vec::with_capacity(points.len());
    let mut used = BTreeSet::new();
    let mut ideal_groups = BTreeMap::<(usize, usize), Vec<usize>>::new();

    for (index, point) in points.iter().enumerate() {
        let marker = chart_primitives::marker_char(index, opts.charset);
        if !point.x.is_finite() || !point.y.is_finite() {
            placements.push(PointPlacement {
                note: Some("invalid: non-finite coordinate"),
            });
            continue;
        }
        if !(0.0..=1.0).contains(&point.x) || !(0.0..=1.0).contains(&point.y) {
            placements.push(PointPlacement {
                note: Some("outside normalized range 0..1"),
            });
            continue;
        }

        let target = normalized_position(area, point.x, point.y);
        ideal_groups.entry(target).or_default().push(index);
        let position = if used.insert(target) {
            Some(target)
        } else {
            find_nearby_position(canvas, area, target, &used)
        };
        if let Some(position) = position {
            used.insert(position);
            canvas.set_text(position.0, position.1, marker)?;
            placements.push(PointPlacement {
                note: (position != target).then_some("shifted from overlapping coordinate"),
            });
        } else {
            placements.push(PointPlacement {
                note: Some("not placed: no free plot cell"),
            });
        }
    }

    ideal_groups.retain(|_, indices| indices.len() > 1);
    Ok((placements, ideal_groups))
}

fn normalized_position(area: PlotArea, x: f64, y: f64) -> (usize, usize) {
    let inner_left = area.x + 1;
    let inner_right = area.right() - 1;
    let inner_top = area.y + 1;
    let inner_bottom = area.bottom() - 1;
    let column = inner_left as f64 + x * (inner_right - inner_left) as f64;
    let row = inner_bottom as f64 - y * (inner_bottom - inner_top) as f64;
    (column.round() as usize, row.round() as usize)
}

fn find_nearby_position(
    canvas: &Canvas,
    area: PlotArea,
    target: (usize, usize),
    used: &BTreeSet<(usize, usize)>,
) -> Option<(usize, usize)> {
    let maximum_radius = area.width.max(area.height).min(8) as i64;
    for radius in 1..=maximum_radius {
        for delta_y in -radius..=radius {
            for delta_x in -radius..=radius {
                if delta_x.abs().max(delta_y.abs()) != radius {
                    continue;
                }
                let x = target.0 as i64 + delta_x;
                let y = target.1 as i64 + delta_y;
                if x <= area.x as i64
                    || x >= area.right() as i64
                    || y <= area.y as i64
                    || y >= area.bottom() as i64
                {
                    continue;
                }
                let position = (x as usize, y as usize);
                if !used.contains(&position) && plot_background_cell(canvas, position) {
                    return Some(position);
                }
            }
        }
    }
    None
}

fn plot_background_cell(canvas: &Canvas, position: (usize, usize)) -> bool {
    canvas
        .get_cell(position.0, position.1)
        .is_some_and(|cell| matches!(cell, "" | "─" | "│" | "+" | "-" | "|"))
        && canvas.continuation_owner(position.0, position.1).is_none()
}

fn place_full_label(
    canvas: &mut Canvas,
    start: usize,
    end: usize,
    row: usize,
    text: &str,
    align_right: bool,
) -> Result<bool> {
    if row >= canvas.height() || end < start {
        return Ok(false);
    }
    let width = str_display_width(text);
    let available = end - start + 1;
    if width == 0 || width > available {
        return Ok(false);
    }
    let column = if align_right { end + 1 - width } else { start };
    if !span_is_clear(canvas, column, row, width) {
        return Ok(false);
    }
    canvas.set_text(column, row, text)?;
    Ok(true)
}

fn place_label_at(canvas: &mut Canvas, column: usize, row: usize, text: &str) -> Result<bool> {
    let width = str_display_width(text);
    if text.is_empty()
        || row >= canvas.height()
        || column.saturating_add(width) > canvas.width()
        || !span_is_clear(canvas, column, row, width)
    {
        return Ok(false);
    }
    canvas.set_text(column, row, text)?;
    Ok(true)
}

fn span_is_clear(canvas: &Canvas, column: usize, row: usize, width: usize) -> bool {
    (column..column + width).all(|x| {
        canvas.get_cell(x, row).is_some_and(str::is_empty)
            && canvas.continuation_owner(x, row).is_none()
    })
}

fn format_styles(styles: &QuadrantChartStyles) -> String {
    let mut parts = Vec::new();
    if let Some(radius) = styles.radius {
        parts.push(format!("radius={radius}"));
    }
    if let Some(color) = &styles.color {
        parts.push(format!("color={}", sanitize_label_text(color)));
    }
    if let Some(color) = &styles.stroke_color {
        parts.push(format!("stroke-color={}", sanitize_label_text(color)));
    }
    if let Some(width) = &styles.stroke_width {
        parts.push(format!("stroke-width={}", sanitize_label_text(width)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" {{{}}}", parts.join(", "))
    }
}

fn format_coordinate(value: f64) -> String {
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
