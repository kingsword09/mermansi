//! Gantt terminal geometry.
//!
//! Parsed task timestamps are mapped onto one bounded horizontal time axis. Each task receives a
//! proportional bar or milestone marker and a numbered semantic detail line.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::adapters::box_geometry::wrap_display;
use crate::adapters::chart_primitives::{fill_char, render_cropped_canvas};
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_vertical_line};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::gantt::{GanttDiagramRenderModel, GanttRenderTask};

const MAX_GANTT_TASKS: usize = 4_096;
const MIN_GANTT_WIDTH: usize = 32;
const MIN_PLOT_WIDTH: usize = 18;

pub fn render_gantt(model: &GanttDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    ensure_limit("gantt tasks", model.tasks.len(), MAX_GANTT_TASKS)?;
    if model.tasks.is_empty() {
        return render_empty_gantt(model, opts);
    }
    if opts.max_width < MIN_GANTT_WIDTH {
        return Err(MermansiError::RenderLimit {
            context: "gantt columns",
            requested: MIN_GANTT_WIDTH,
            limit: opts.max_width,
        });
    }

    let mut tasks = model.tasks.iter().enumerate().collect::<Vec<_>>();
    tasks.sort_by_key(|(index, task)| (task.order, *index));
    let minimum_time = tasks
        .iter()
        .map(|(_, task)| task.start_ms)
        .min()
        .unwrap_or(0);
    let maximum_time = tasks
        .iter()
        .map(|(_, task)| task.render_end_ms.unwrap_or(task.end_ms))
        .max()
        .unwrap_or(minimum_time)
        .max(minimum_time);

    let width = opts.max_width.min(120);
    let captions = tasks
        .iter()
        .enumerate()
        .map(|(index, (_, task))| {
            format!(
                "[{}] {} / {}",
                index + 1,
                nonempty(&task.section),
                nonempty(task.task.trim())
            )
        })
        .collect::<Vec<_>>();
    let desired_label_width = captions
        .iter()
        .map(|caption| str_display_width(caption))
        .max()
        .unwrap_or(5)
        .min(30);
    let label_width = desired_label_width
        .min(width.saturating_sub(MIN_PLOT_WIDTH + 1))
        .max(5);
    let plot_x = label_width + 1;
    let plot_width = width - plot_x;
    if plot_width < MIN_PLOT_WIDTH {
        return Err(MermansiError::RenderLimit {
            context: "gantt plot columns",
            requested: MIN_PLOT_WIDTH,
            limit: plot_width,
        });
    }

    let detail_lines = tasks
        .iter()
        .enumerate()
        .flat_map(|(index, (_, task))| {
            wrap_display(&task_detail(index + 1, task), width.saturating_sub(2))
        })
        .collect::<Vec<_>>();
    let plot_y = 3usize;
    let plot_height = tasks.len().saturating_add(2);
    let detail_y = plot_y + plot_height + 1;
    let height = detail_y.saturating_add(detail_lines.len()).max(7);
    if opts.max_height < height {
        return Err(MermansiError::RenderLimit {
            context: "gantt rows",
            requested: height,
            limit: opts.max_height,
        });
    }

    let mut canvas = Canvas::new(width, height)?;
    let title = model
        .title
        .as_deref()
        .map(sanitize_label_text)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "Gantt".to_owned());
    write_centered(&mut canvas, 0, 0, width, &title)?;
    let start_label = format_date(minimum_time);
    let end_label = format_date(maximum_time);
    canvas.set_text(plot_x, 2, &start_label)?;
    let end_x = width.saturating_sub(str_display_width(&end_label));
    if end_x > plot_x + str_display_width(&start_label) {
        canvas.set_text(end_x, 2, &end_label)?;
    }
    draw_box(
        &mut canvas,
        plot_x,
        plot_y,
        plot_width,
        plot_height,
        opts.charset,
    )?;
    for quarter in [1usize, 2, 3] {
        let x = plot_x + quarter * (plot_width - 1) / 4;
        draw_vertical_line(
            &mut canvas,
            x,
            plot_y,
            plot_y + plot_height - 1,
            opts.charset,
        )?;
    }

    let mut section_indices = HashMap::<&str, usize>::new();
    for (task_index, ((_, task), caption)) in tasks.iter().zip(&captions).enumerate() {
        let row_y = plot_y + 1 + task_index;
        let visible_caption = if str_display_width(caption) <= label_width {
            caption.clone()
        } else {
            format!("[{}]", task_index + 1)
        };
        canvas.set_text(0, row_y, &visible_caption)?;

        let next_section = section_indices.len();
        let section_index = *section_indices
            .entry(task.section.as_str())
            .or_insert(next_section);
        let inner_width = plot_width - 2;
        let start = scale_time(task.start_ms, minimum_time, maximum_time, inner_width - 1);
        let end_time = task.render_end_ms.unwrap_or(task.end_ms).max(task.start_ms);
        let end = scale_time(end_time, minimum_time, maximum_time, inner_width - 1).max(start);
        let x = plot_x + 1 + start;
        if task.milestone {
            canvas.set_char(
                x,
                row_y,
                if opts.charset == Charset::Unicode {
                    '◆'
                } else {
                    '*'
                },
            )?;
            continue;
        }
        let fill = task_fill(task, section_index, opts.charset);
        canvas.set_text(
            x,
            row_y,
            &fill.repeat(end.saturating_sub(start).saturating_add(1)),
        )?;
    }
    for (offset, line) in detail_lines.iter().enumerate() {
        canvas.set_text(1, detail_y + offset, line)?;
    }
    Ok(render_cropped_canvas(&canvas))
}

fn task_detail(index: usize, task: &GanttRenderTask) -> String {
    let mut parts = vec![
        format!(
            "[{index}] {} / {}",
            nonempty(&task.section),
            nonempty(task.task.trim())
        ),
        format!(
            "{} -> {}",
            format_date(task.start_ms),
            format_date(task.render_end_ms.unwrap_or(task.end_ms))
        ),
    ];
    if !task.id.trim().is_empty() {
        parts.push(format!("id={}", sanitize_label_text(task.id.trim())));
    }
    let mut flags = Vec::new();
    if task.milestone {
        flags.push("milestone");
    }
    if task.active {
        flags.push("active");
    }
    if task.done {
        flags.push("done");
    }
    if task.crit {
        flags.push("crit");
    }
    if task.vert {
        flags.push("vert");
    }
    if !flags.is_empty() {
        parts.push(flags.join(","));
    }
    if !task.classes.is_empty() {
        parts.push(format!("classes={}", task.classes.join(",")));
    }
    if !task.task_type.trim().is_empty() && task.task_type != task.section {
        parts.push(format!(
            "type={}",
            sanitize_label_text(task.task_type.trim())
        ));
    }
    parts.join("  ")
}

fn task_fill(task: &GanttRenderTask, section: usize, charset: Charset) -> &'static str {
    if charset == Charset::Ascii {
        if task.crit {
            "!"
        } else if task.active {
            "@"
        } else if task.done {
            "#"
        } else {
            fill_char(section, charset)
        }
    } else if task.crit {
        "▒"
    } else if task.active {
        "▓"
    } else if task.done {
        "█"
    } else {
        fill_char(section, charset)
    }
}

fn scale_time(value: i64, minimum: i64, maximum: i64, columns: usize) -> usize {
    let span = i128::from(maximum).saturating_sub(i128::from(minimum));
    if span <= 0 || columns == 0 {
        return 0;
    }
    let offset = i128::from(value)
        .saturating_sub(i128::from(minimum))
        .clamp(0, span);
    usize::try_from(offset.saturating_mul(columns as i128) / span)
        .unwrap_or(columns)
        .min(columns)
}

fn format_date(milliseconds: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(milliseconds).map_or_else(
        || milliseconds.to_string(),
        |date| {
            let local = merman_core::time::datetime_to_local_fixed(
                date.with_timezone(&merman_core::time::utc_fixed_offset()),
            );
            local.format("%Y-%m-%d").to_string()
        },
    )
}

fn render_empty_gantt(model: &GanttDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let width = opts.max_width.min(32);
    if width < 16 || opts.max_height < 3 {
        return Err(MermansiError::RenderLimit {
            context: "gantt empty card",
            requested: 16,
            limit: width.min(opts.max_height),
        });
    }
    let mut canvas = Canvas::new(width, 3)?;
    draw_box(&mut canvas, 0, 0, width, 3, opts.charset)?;
    let title = model.title.as_deref().unwrap_or("empty Gantt");
    write_centered(&mut canvas, 1, 1, width - 2, &nonempty(title))?;
    Ok(render_cropped_canvas(&canvas))
}

fn write_centered(
    canvas: &mut Canvas,
    left: usize,
    y: usize,
    width: usize,
    text: &str,
) -> Result<()> {
    let text = sanitize_label_text(text);
    let text_width = str_display_width(&text);
    if text_width > width {
        return Err(MermansiError::GeometryLayout {
            family: "gantt",
            message: "Gantt label exceeds its assigned width".to_owned(),
        });
    }
    canvas.set_text(left + (width - text_width) / 2, y, &text)
}

fn nonempty(value: &str) -> String {
    let value = sanitize_label_text(value).trim().to_owned();
    if value.is_empty() {
        "(unnamed)".to_owned()
    } else {
        value
    }
}

fn ensure_limit(context: &'static str, requested: usize, limit: usize) -> Result<()> {
    if requested > limit {
        return Err(MermansiError::RenderLimit {
            context,
            requested,
            limit,
        });
    }
    Ok(())
}
