//! XY chart terminal geometry.
//!
//! The upstream renderer remains the default. Models whose bars would overlap, grow from the
//! axis minimum instead of zero, or disappear on a degenerate range use the bounded native
//! grouped-bar path below.

use crate::adapters::box_geometry::{self, wrap_display};
use crate::adapters::chart_primitives::{self, ensure_entity_limit};
use crate::adapters::to_ascii_options;
use crate::ansi::{AnsiEncoder, AnsiRole, sanitize_label_text};
use crate::canvas::{Canvas, draw_horizontal_line, draw_vertical_line};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::xychart::{
    XyChartAxisRenderModel, XyChartDiagramRenderModel, XyChartPlotRenderModel, XyChartPlotType,
};

const DEFAULT_CATEGORY_BAND_WIDTH: usize = 3;
const AXIS_AND_TICK_BUDGET: usize = 16;
const NATIVE_PLOT_HEIGHT: usize = 11;
const MIN_NATIVE_PLOT_WIDTH: usize = 12;

#[derive(Clone, Copy, Debug)]
struct ValueRange {
    min: f64,
    max: f64,
}

impl ValueRange {
    fn span(self) -> f64 {
        self.max - self.min
    }

    fn baseline(self) -> f64 {
        0.0f64.clamp(self.min, self.max)
    }

    fn normalized(self, value: f64) -> f64 {
        ((value.clamp(self.min, self.max) - self.min) / self.span()).clamp(0.0, 1.0)
    }
}

pub fn render_xychart(model: &XyChartDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    ensure_entity_limit(
        "xychart values",
        model.plots.iter().fold(model.plots.len(), |count, plot| {
            count.saturating_add(plot.values.len())
        }),
    )?;
    if model.plots.is_empty() {
        return Ok("(empty XY chart)\n".to_owned());
    }

    if requires_native_geometry(model) {
        return render_native(model, opts);
    }

    let geometry_model = xychart_geometry_model(model);
    let mut ascii_options = to_ascii_options(opts);
    if let XyChartAxisRenderModel::Band { categories, .. } = &geometry_model.x_axis {
        let category_count = categories.len().max(
            geometry_model
                .plots
                .iter()
                .map(|plot| plot.values.len())
                .max()
                .unwrap_or_default(),
        );
        let available = opts.max_width.saturating_sub(AXIS_AND_TICK_BUDGET);
        if let Some(maximum) = available
            .saturating_sub(category_count.saturating_sub(1))
            .checked_div(category_count)
        {
            let desired = categories
                .iter()
                .map(|category| str_display_width(category))
                .max()
                .unwrap_or_default()
                .max(DEFAULT_CATEGORY_BAND_WIDTH);
            ascii_options =
                ascii_options.with_xychart_category_band_width(desired.min(maximum.max(1)));
        }
    }

    let output = merman_ascii::render_xychart(&geometry_model, &ascii_options)?;
    if output.trim().is_empty() {
        Ok("(empty XY chart)\n".to_owned())
    } else {
        Ok(output)
    }
}

fn xychart_geometry_model(model: &XyChartDiagramRenderModel) -> XyChartDiagramRenderModel {
    let mut geometry = model.clone();
    geometry.title = geometry.title.as_deref().map(sanitize_label_text);
    geometry.acc_title = geometry.acc_title.as_deref().map(sanitize_label_text);
    geometry.acc_descr = geometry.acc_descr.as_deref().map(sanitize_label_text);
    for axis in [&mut geometry.x_axis, &mut geometry.y_axis] {
        match axis {
            XyChartAxisRenderModel::Band { title, categories } => {
                *title = sanitize_label_text(title);
                for category in categories {
                    *category = sanitize_label_text(category);
                }
            }
            XyChartAxisRenderModel::Linear { title, .. } => {
                *title = sanitize_label_text(title);
            }
        }
    }
    for plot in &mut geometry.plots {
        plot.title = plot.title.as_deref().map(sanitize_label_text);
        for (label, _) in &mut plot.data {
            *label = sanitize_label_text(label);
        }
    }
    geometry
}

fn requires_native_geometry(model: &XyChartDiagramRenderModel) -> bool {
    if model
        .plots
        .iter()
        .flat_map(|plot| &plot.values)
        .any(|value| !value.is_finite())
    {
        return true;
    }
    let bars = model
        .plots
        .iter()
        .filter(|plot| plot.plot_type == XyChartPlotType::Bar)
        .collect::<Vec<_>>();
    if bars.is_empty() {
        return false;
    }
    if bars.len() > 1
        || bars
            .iter()
            .flat_map(|plot| &plot.values)
            .any(|value| *value < 0.0)
    {
        return true;
    }
    matches!(
        model.y_axis,
        XyChartAxisRenderModel::Linear {
            min: Some(min),
            max: Some(max),
            ..
        } if (max - min).abs() <= f64::EPSILON
    )
}

fn render_native(model: &XyChartDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let categories = category_labels(model);
    if categories.is_empty() {
        return Ok("(empty XY chart)\n".to_owned());
    }
    let range = value_range(model);
    let bar_count = model
        .plots
        .iter()
        .filter(|plot| plot.plot_type == XyChartPlotType::Bar)
        .count();

    let mut output = String::new();
    append_header(&mut output, model, opts.max_width);
    append_legend(&mut output, model, opts);

    let horizontal = model.orientation.eq_ignore_ascii_case("horizontal");
    let chart = if horizontal {
        render_horizontal(model, &categories, range, opts)?
    } else {
        match render_vertical(model, &categories, range, bar_count, opts) {
            Ok(chart) => chart,
            Err(MermansiError::RenderLimit {
                context: "xychart grouped columns",
                ..
            }) => render_horizontal(model, &categories, range, opts)?,
            Err(error) => return Err(error),
        }
    };
    output.push_str(&chart);
    append_axis_titles(&mut output, model, opts.max_width);
    append_values(&mut output, model, &categories, opts);
    Ok(output)
}

fn render_vertical(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    range: ValueRange,
    bar_count: usize,
    opts: &MermansiOptions,
) -> Result<String> {
    let labels = tick_labels(range);
    let show_y_labels = model.display.y_axis.show_label;
    let gutter = if show_y_labels {
        labels
            .iter()
            .map(|label| str_display_width(label))
            .max()
            .unwrap_or(0)
    } else {
        0
    };
    let axis_x = gutter + usize::from(show_y_labels);
    let plot_start = axis_x + usize::from(model.display.y_axis.show_axis_line);
    let available = opts.max_width.saturating_sub(plot_start);
    let minimum = categories.len().saturating_mul(bar_count.max(1));
    if available < minimum.max(MIN_NATIVE_PLOT_WIDTH.min(opts.max_width)) {
        return Err(MermansiError::RenderLimit {
            context: "xychart grouped columns",
            requested: plot_start.saturating_add(minimum),
            limit: opts.max_width,
        });
    }

    let desired_band = bar_count.saturating_mul(2).saturating_add(2).max(3);
    let desired_width = categories.len().saturating_mul(desired_band).min(available);
    let plot_width = desired_width.max(minimum);
    let width = plot_start.saturating_add(plot_width);
    let plot_height = NATIVE_PLOT_HEIGHT.min(opts.max_height.saturating_sub(2));
    if plot_height < 3 {
        return Err(MermansiError::RenderLimit {
            context: "xychart rows",
            requested: 5,
            limit: opts.max_height,
        });
    }
    let category_row = plot_height + 1;
    let mut canvas = Canvas::new(width, category_row + 1)?;

    let zero_y = vertical_coordinate(range.baseline(), range, plot_height);
    let encoder = AnsiEncoder::new(opts.color_mode);
    for (series_index, plot) in model
        .plots
        .iter()
        .enumerate()
        .filter(|(_, plot)| plot.plot_type == XyChartPlotType::Line)
    {
        draw_vertical_line_series(
            &mut canvas,
            plot,
            series_index,
            categories.len(),
            plot_start,
            plot_width,
            plot_height,
            range,
            opts,
            &encoder,
        )?;
    }

    let mut bar_ordinal = 0usize;
    let band_width = plot_width / categories.len();
    let bar_width = if band_width >= bar_count.saturating_mul(2) {
        2
    } else {
        1
    };
    let group_width = bar_count.saturating_mul(bar_width);
    for (series_index, plot) in model
        .plots
        .iter()
        .enumerate()
        .filter(|(_, plot)| plot.plot_type == XyChartPlotType::Bar)
    {
        for (category_index, value) in plot.values.iter().copied().enumerate() {
            if category_index >= categories.len()
                || !value.is_finite()
                || (value - range.baseline()).abs() <= f64::EPSILON
            {
                continue;
            }
            let band_start = plot_start + category_index * band_width;
            let group_start = band_start + band_width.saturating_sub(group_width) / 2;
            let x = group_start + bar_ordinal * bar_width;
            let value_y = vertical_coordinate(value, range, plot_height);
            let (top, bottom) = ordered(value_y, zero_y);
            for y in top..=bottom {
                for offset in 0..bar_width {
                    paint_series_cell(
                        &mut canvas,
                        x + offset,
                        y,
                        chart_primitives::fill_char(series_index, opts.charset),
                        series_index,
                        &encoder,
                    )?;
                }
            }
        }
        bar_ordinal += 1;
    }

    if model.display.y_axis.show_axis_line {
        draw_vertical_line(&mut canvas, axis_x, 0, plot_height - 1, opts.charset)?;
    }
    if model.display.x_axis.show_axis_line {
        draw_horizontal_line(
            &mut canvas,
            zero_y,
            axis_x,
            width.saturating_sub(1),
            opts.charset,
        )?;
    }
    if show_y_labels {
        write_tick_label(&mut canvas, axis_x, 0, &labels[0])?;
        write_tick_label(&mut canvas, axis_x, zero_y, &labels[1])?;
        write_tick_label(&mut canvas, axis_x, plot_height - 1, &labels[2])?;
    }
    if model.display.x_axis.show_label {
        write_category_labels(
            &mut canvas,
            categories,
            plot_start,
            plot_width,
            category_row,
        )?;
    }
    Ok(canvas.render())
}

#[allow(clippy::too_many_arguments)]
fn draw_vertical_line_series(
    canvas: &mut Canvas,
    plot: &XyChartPlotRenderModel,
    series_index: usize,
    category_count: usize,
    plot_start: usize,
    plot_width: usize,
    plot_height: usize,
    range: ValueRange,
    opts: &MermansiOptions,
    encoder: &AnsiEncoder,
) -> Result<()> {
    let band_width = plot_width / category_count;
    let points = plot
        .values
        .iter()
        .copied()
        .take(category_count)
        .enumerate()
        .map(|(index, value)| {
            value.is_finite().then(|| {
                (
                    plot_start + index * band_width + band_width / 2,
                    vertical_coordinate(value, range, plot_height),
                )
            })
        })
        .collect::<Vec<_>>();
    let glyph = chart_primitives::marker_char(series_index, opts.charset);
    for pair in points.windows(2) {
        if let [Some(from), Some(to)] = pair {
            chart_primitives::draw_line(
                canvas,
                from.0 as i64,
                from.1 as i64,
                to.0 as i64,
                to.1 as i64,
                glyph,
            )?;
        }
    }
    for (x, y) in points.into_iter().flatten() {
        paint_series_cell(canvas, x, y, glyph, series_index, encoder)?;
    }
    Ok(())
}

fn render_horizontal(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    range: ValueRange,
    opts: &MermansiOptions,
) -> Result<String> {
    let bar_count = model
        .plots
        .iter()
        .filter(|plot| plot.plot_type == XyChartPlotType::Bar)
        .count();
    let has_line = model
        .plots
        .iter()
        .any(|plot| plot.plot_type == XyChartPlotType::Line);
    let line_row = bar_count;
    let label_limit = opts.max_width.saturating_sub(MIN_NATIVE_PLOT_WIDTH + 2);
    let label_width = if model.display.x_axis.show_label {
        categories
            .iter()
            .map(|label| str_display_width(label))
            .max()
            .unwrap_or(0)
            .min(label_limit)
    } else {
        0
    };
    let plot_start = label_width + usize::from(label_width > 0);
    let plot_width = opts.max_width.saturating_sub(plot_start);
    if plot_width < MIN_NATIVE_PLOT_WIDTH {
        return Err(MermansiError::RenderLimit {
            context: "xychart horizontal columns",
            requested: plot_start.saturating_add(MIN_NATIVE_PLOT_WIDTH),
            limit: opts.max_width,
        });
    }
    let group_height = bar_count.saturating_add(usize::from(has_line)).max(1);
    let plot_rows = categories
        .len()
        .saturating_mul(group_height)
        .saturating_sub(1);
    let axis_label_row = plot_rows + 1;
    if axis_label_row + 1 > opts.max_height {
        return Err(MermansiError::RenderLimit {
            context: "xychart horizontal rows",
            requested: axis_label_row + 1,
            limit: opts.max_height,
        });
    }
    let mut canvas = Canvas::new(opts.max_width, axis_label_row + 1)?;
    let zero_x = horizontal_coordinate(range.baseline(), range, plot_start, plot_width);
    let encoder = AnsiEncoder::new(opts.color_mode);

    let mut bar_ordinal = 0usize;
    for (series_index, plot) in model.plots.iter().enumerate() {
        match plot.plot_type {
            XyChartPlotType::Bar => {
                for (category_index, value) in plot.values.iter().copied().enumerate() {
                    if category_index >= categories.len()
                        || !value.is_finite()
                        || (value - range.baseline()).abs() <= f64::EPSILON
                    {
                        continue;
                    }
                    let y = category_index * group_height + bar_ordinal;
                    let value_x = horizontal_coordinate(value, range, plot_start, plot_width);
                    let (left, right) = ordered(value_x, zero_x);
                    for x in left..=right {
                        paint_series_cell(
                            &mut canvas,
                            x,
                            y,
                            chart_primitives::fill_char(series_index, opts.charset),
                            series_index,
                            &encoder,
                        )?;
                    }
                }
                bar_ordinal += 1;
            }
            XyChartPlotType::Line => {
                let points = plot
                    .values
                    .iter()
                    .copied()
                    .take(categories.len())
                    .enumerate()
                    .map(|(index, value)| {
                        value.is_finite().then(|| {
                            (
                                horizontal_coordinate(value, range, plot_start, plot_width),
                                index * group_height + line_row,
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                let glyph = chart_primitives::marker_char(series_index, opts.charset);
                for pair in points.windows(2) {
                    if let [Some(from), Some(to)] = pair {
                        chart_primitives::draw_line(
                            &mut canvas,
                            from.0 as i64,
                            from.1 as i64,
                            to.0 as i64,
                            to.1 as i64,
                            glyph,
                        )?;
                    }
                }
                for (x, y) in points.into_iter().flatten() {
                    paint_series_cell(&mut canvas, x, y, glyph, series_index, &encoder)?;
                }
            }
        }
    }

    if model.display.y_axis.show_axis_line {
        draw_vertical_line(
            &mut canvas,
            zero_x,
            0,
            plot_rows.saturating_sub(1),
            opts.charset,
        )?;
    }
    if model.display.x_axis.show_label {
        for (index, category) in categories.iter().enumerate() {
            let fitted = fit_label(category, label_width);
            let y = index * group_height;
            let x = label_width.saturating_sub(str_display_width(&fitted));
            canvas.set_text(x, y, &fitted)?;
        }
    }
    if model.display.y_axis.show_label {
        let min = format_number(range.min);
        let zero = format_number(range.baseline());
        let max = format_number(range.max);
        canvas.set_text(plot_start, axis_label_row, &fit_label(&min, plot_width))?;
        let zero_start = zero_x.saturating_sub(str_display_width(&zero) / 2);
        let min_end = plot_start.saturating_add(str_display_width(&min));
        let zero_end = zero_start.saturating_add(str_display_width(&zero));
        let max_start = opts.max_width.saturating_sub(str_display_width(&max));
        if zero_start > min_end && zero_end < max_start {
            canvas.set_text(zero_start, axis_label_row, &zero)?;
        }
        canvas.set_text(max_start, axis_label_row, &max)?;
    }
    Ok(canvas.render())
}

fn paint_series_cell(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    glyph: &str,
    series_index: usize,
    encoder: &AnsiEncoder,
) -> Result<()> {
    canvas.set_styled_text(
        x,
        y,
        glyph,
        encoder.prefix(AnsiRole::ChartSeries(series_index as u8)),
        encoder.suffix(),
    )
}

fn append_header(output: &mut String, model: &XyChartDiagramRenderModel, width: usize) {
    if !model.display.show_title {
        return;
    }
    if let Some(title) = model
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
    {
        for line in wrap_display(&sanitize_label_text(title), width) {
            output.push_str(&line);
            output.push('\n');
        }
    }
}

fn append_legend(output: &mut String, model: &XyChartDiagramRenderModel, opts: &MermansiOptions) {
    if model.plots.len() <= 1 {
        return;
    }
    let encoder = AnsiEncoder::new(opts.color_mode);
    let mut bar_index = 0usize;
    let mut line_index = 0usize;
    let mut entries = Vec::new();
    for (series_index, plot) in model.plots.iter().enumerate() {
        let label = series_label(plot, &mut bar_index, &mut line_index);
        let glyph = match plot.plot_type {
            XyChartPlotType::Bar => chart_primitives::fill_char(series_index, opts.charset),
            XyChartPlotType::Line => chart_primitives::marker_char(series_index, opts.charset),
        };
        entries.push(format!(
            "{} {label}",
            encoder.paint(AnsiRole::ChartSeries(series_index as u8), glyph)
        ));
    }
    let legend = entries.join("  ");
    for line in wrap_display(&legend, opts.max_width) {
        output.push_str(&line);
        output.push('\n');
    }
}

fn append_axis_titles(output: &mut String, model: &XyChartDiagramRenderModel, width: usize) {
    for (prefix, enabled, title) in [
        (
            "x: ",
            model.display.x_axis.show_title,
            axis_title(&model.x_axis),
        ),
        (
            "y: ",
            model.display.y_axis.show_title,
            axis_title(&model.y_axis),
        ),
    ] {
        if !enabled || title.trim().is_empty() {
            continue;
        }
        for line in wrap_display(&format!("{prefix}{}", sanitize_label_text(title)), width) {
            output.push_str(&line);
            output.push('\n');
        }
    }
}

fn append_values(
    output: &mut String,
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    opts: &MermansiOptions,
) {
    output.push_str("\nSeries values\n");
    let encoder = AnsiEncoder::new(opts.color_mode);
    let mut bar_index = 0usize;
    let mut line_index = 0usize;
    for (series_index, plot) in model.plots.iter().enumerate() {
        let label = series_label(plot, &mut bar_index, &mut line_index);
        let marker = match plot.plot_type {
            XyChartPlotType::Bar => chart_primitives::fill_char(series_index, opts.charset),
            XyChartPlotType::Line => chart_primitives::marker_char(series_index, opts.charset),
        };
        let values = plot
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let category = categories
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| (index + 1).to_string());
                format!(
                    "{}={}",
                    sanitize_label_text(&category),
                    format_number(*value)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let line = format!("{marker} {label}: {values}");
        for (index, wrapped) in box_geometry::wrap_words(&line, opts.max_width.saturating_sub(2))
            .into_iter()
            .enumerate()
        {
            output.push_str("  ");
            if index == 0 && opts.color_mode != crate::options::ColorMode::Plain {
                let styled = wrapped.replacen(
                    marker,
                    &encoder.paint(AnsiRole::ChartSeries(series_index as u8), marker),
                    1,
                );
                output.push_str(&styled);
            } else {
                output.push_str(&wrapped);
            }
            output.push('\n');
        }
    }
}

fn category_labels(model: &XyChartDiagramRenderModel) -> Vec<String> {
    let count = model
        .plots
        .iter()
        .map(|plot| plot.values.len())
        .max()
        .unwrap_or(0);
    match &model.x_axis {
        XyChartAxisRenderModel::Band { categories, .. } => {
            let mut labels = categories
                .iter()
                .map(|label| sanitize_label_text(label))
                .collect::<Vec<_>>();
            labels.extend((labels.len()..count).map(|index| (index + 1).to_string()));
            labels
        }
        XyChartAxisRenderModel::Linear { min, max, .. } => {
            linear_labels(min.unwrap_or(1.0), max.unwrap_or(count as f64), count)
        }
    }
}

fn linear_labels(min: f64, max: f64, count: usize) -> Vec<String> {
    match count {
        0 => Vec::new(),
        1 => vec![format_number(min)],
        _ => {
            let step = (max - min) / (count - 1) as f64;
            (0..count)
                .map(|index| format_number(min + step * index as f64))
                .collect()
        }
    }
}

fn value_range(model: &XyChartDiagramRenderModel) -> ValueRange {
    let values = model
        .plots
        .iter()
        .flat_map(|plot| plot.values.iter())
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let data_min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let data_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let has_bars = model
        .plots
        .iter()
        .any(|plot| plot.plot_type == XyChartPlotType::Bar);
    let (axis_min, axis_max) = match model.y_axis {
        XyChartAxisRenderModel::Linear { min, max, .. } => (min, max),
        XyChartAxisRenderModel::Band { .. } => (None, None),
    };
    let axis_min = axis_min.filter(|value| value.is_finite());
    let axis_max = axis_max.filter(|value| value.is_finite());
    let default_min = if data_min.is_finite() {
        if has_bars {
            data_min.min(0.0)
        } else {
            data_min
        }
    } else {
        0.0
    };
    let default_max = if data_max.is_finite() { data_max } else { 1.0 };
    let mut min = axis_min.unwrap_or(default_min);
    let mut max = axis_max.unwrap_or(default_max);
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if (max - min).abs() <= f64::EPSILON {
        if has_bars && max > 0.0 {
            min = 0.0;
        } else if has_bars && min < 0.0 {
            max = 0.0;
        } else {
            let padding = min.abs().max(1.0) * 0.5;
            min -= padding;
            max += padding;
        }
    }
    ValueRange { min, max }
}

fn tick_labels(range: ValueRange) -> [String; 3] {
    [
        format_number(range.max),
        format_number(range.baseline()),
        format_number(range.min),
    ]
}

fn vertical_coordinate(value: f64, range: ValueRange, height: usize) -> usize {
    let level = (range.normalized(value) * height.saturating_sub(1) as f64).round() as usize;
    height.saturating_sub(1).saturating_sub(level)
}

fn horizontal_coordinate(value: f64, range: ValueRange, start: usize, width: usize) -> usize {
    start + (range.normalized(value) * width.saturating_sub(1) as f64).round() as usize
}

fn write_tick_label(canvas: &mut Canvas, axis_x: usize, row: usize, label: &str) -> Result<()> {
    let width = str_display_width(label);
    canvas.set_text(axis_x.saturating_sub(width + 1), row, label)
}

fn write_category_labels(
    canvas: &mut Canvas,
    categories: &[String],
    start: usize,
    width: usize,
    row: usize,
) -> Result<()> {
    let band_width = width / categories.len();
    for (index, category) in categories.iter().enumerate() {
        let fitted = fit_label(category, band_width);
        let label_width = str_display_width(&fitted);
        let x = start + index * band_width + band_width.saturating_sub(label_width) / 2;
        canvas.set_text(x, row, &fitted)?;
    }
    Ok(())
}

fn fit_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    wrap_display(&sanitize_label_text(label), width)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn series_label(
    plot: &XyChartPlotRenderModel,
    bar_index: &mut usize,
    line_index: &mut usize,
) -> String {
    let fallback = match plot.plot_type {
        XyChartPlotType::Bar => {
            *bar_index += 1;
            format!("Bar {}", *bar_index)
        }
        XyChartPlotType::Line => {
            *line_index += 1;
            format!("Line {}", *line_index)
        }
    };
    plot.title
        .as_deref()
        .map(sanitize_label_text)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(fallback)
}

fn axis_title(axis: &XyChartAxisRenderModel) -> &str {
    match axis {
        XyChartAxisRenderModel::Band { title, .. }
        | XyChartAxisRenderModel::Linear { title, .. } => title,
    }
}

fn format_number(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() <= 1e-9 {
        format!("{rounded:.0}")
    } else {
        value.to_string()
    }
}

fn ordered(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}
