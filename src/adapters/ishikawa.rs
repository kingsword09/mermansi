//! Ishikawa (fishbone) terminal geometry.
//!
//! Main causes alternate above and below a compact horizontal spine. Each cause has a near-45°
//! diagonal bone; labels sit to its left and connect through short horizontal branches. The effect
//! appears exactly once in the closed head box.

use crate::adapters::chart_primitives::{draw_line, render_cropped_canvas};
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::ishikawa::{IshikawaDiagramRenderModel, IshikawaNodeRenderModel};

const MAX_ISHIKAWA_NODES: usize = 4_096;
const MAX_ISHIKAWA_DEPTH: usize = 64;
const MIN_SLOT_WIDTH: usize = 12;
const MAX_SLOT_WIDTH: usize = 24;
const MIN_EFFECT_WIDTH: usize = 12;
const MAX_EFFECT_WIDTH: usize = 34;
const EFFECT_GAP_WIDTH: usize = 3;

#[derive(Clone)]
struct Descendant {
    text: String,
    depth: usize,
}

pub fn render_ishikawa(
    model: &IshikawaDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let Some(root) = &model.root else {
        return Ok("(empty ishikawa diagram)\n".to_owned());
    };
    validate_tree(root)?;

    let main_causes = &root.children;
    let descendants = main_causes
        .iter()
        .map(|cause| {
            let mut flattened = Vec::new();
            flatten_descendants(cause, 1, &mut flattened);
            flattened
        })
        .collect::<Vec<_>>();
    let maximum_descendants = descendants.iter().map(Vec::len).max().unwrap_or(0);
    let longest_cause = main_causes
        .iter()
        .map(longest_cause_label)
        .max()
        .unwrap_or(0);
    let preferred_slot_width = longest_cause
        .saturating_add(7)
        .clamp(MIN_SLOT_WIDTH, MAX_SLOT_WIDTH);
    let effect = sanitize_label_text(&root.text);
    let preferred_effect_width = str_display_width(&effect)
        .saturating_add(4)
        .clamp(MIN_EFFECT_WIDTH, MAX_EFFECT_WIDTH);
    let branch_count = main_causes.len().max(1);
    let minimum_slots_width =
        branch_count
            .checked_mul(MIN_SLOT_WIDTH)
            .ok_or(MermansiError::RenderLimit {
                context: "ishikawa columns",
                requested: usize::MAX,
                limit: opts.max_width,
            })?;
    let effect_budget = opts
        .max_width
        .saturating_sub(minimum_slots_width)
        .saturating_sub(EFFECT_GAP_WIDTH);
    let effect_width = preferred_effect_width.min(effect_budget.max(MIN_EFFECT_WIDTH));
    let fixed_width = effect_width.saturating_add(EFFECT_GAP_WIDTH);
    let slot_width = compressed_slot_width(
        preferred_slot_width,
        branch_count,
        fixed_width,
        opts.max_width,
    );
    let width = branch_count
        .checked_mul(slot_width)
        .and_then(|value| value.checked_add(fixed_width))
        .ok_or(MermansiError::RenderLimit {
            context: "ishikawa columns",
            requested: usize::MAX,
            limit: opts.max_width,
        })?;
    let distinct_title = model
        .title
        .as_deref()
        .map(sanitize_label_text)
        .filter(|title| !title.is_empty() && *title != effect);
    let title_rows = usize::from(distinct_title.is_some()) * 2;
    let branch_rows = maximum_descendants.saturating_add(4).max(5);
    let height = title_rows
        .checked_add(branch_rows.saturating_mul(2).saturating_add(1))
        .ok_or(MermansiError::RenderLimit {
            context: "ishikawa rows",
            requested: usize::MAX,
            limit: opts.max_height,
        })?;
    ensure_dimension("ishikawa columns", width, opts.max_width)?;
    ensure_dimension("ishikawa rows", height, opts.max_height)?;

    let mut canvas = Canvas::new(width, height)?;
    if let Some(title) = distinct_title {
        let x = width.saturating_sub(str_display_width(&title)) / 2;
        canvas.set_text(x, 0, &title)?;
    }
    let spine_y = title_rows + branch_rows;
    let effect_x = width - effect_width;
    draw_horizontal_line(&mut canvas, spine_y, 1, effect_x, opts.charset)?;
    draw_box(
        &mut canvas,
        effect_x,
        spine_y - 1,
        effect_width,
        3,
        opts.charset,
    )?;

    let mut callouts = Vec::new();
    place_label(
        &mut canvas,
        effect_x + 1,
        spine_y,
        effect_width - 2,
        &effect,
        &mut callouts,
    )?;

    for (index, cause) in main_causes.iter().enumerate() {
        let above = index % 2 == 0;
        let slot_x = 1 + index * slot_width;
        let attach_x = slot_x + slot_width - 2;
        let endpoint_y = if above { title_rows + 1 } else { height - 2 };
        let diagonal_span = endpoint_y
            .abs_diff(spine_y)
            .min(slot_width.saturating_sub(4));
        let endpoint_x = attach_x.saturating_sub(diagonal_span);
        let diagonal = match (opts.charset, above) {
            (Charset::Unicode, true) => "╲",
            (Charset::Unicode, false) => "╱",
            (Charset::Ascii, true) => "\\",
            (Charset::Ascii, false) => "/",
        };
        draw_line(
            &mut canvas,
            endpoint_x as i64,
            endpoint_y as i64,
            attach_x as i64,
            spine_y as i64,
            diagonal,
        )?;
        let joint = match opts.charset {
            Charset::Unicode => "◆",
            Charset::Ascii => "*",
        };
        canvas.set_text(attach_x, spine_y, joint)?;
        place_connected_label(
            &mut canvas,
            slot_x,
            endpoint_x,
            endpoint_y,
            1,
            &sanitize_label_text(&cause.text),
            opts.charset,
            &mut callouts,
        )?;

        for (descendant_index, descendant) in descendants[index].iter().enumerate() {
            let row = if above {
                endpoint_y + descendant_index + 1
            } else {
                endpoint_y.saturating_sub(descendant_index + 1)
            };
            if row == spine_y || row >= height {
                return Err(MermansiError::GeometryLayout {
                    family: "ishikawa",
                    message: "cause hierarchy does not fit between the branch label and spine"
                        .to_owned(),
                });
            }
            let bone_x = interpolate_x(endpoint_x, endpoint_y, attach_x, spine_y, row);
            place_connected_label(
                &mut canvas,
                slot_x,
                bone_x,
                row,
                descendant.depth,
                &descendant.text,
                opts.charset,
                &mut callouts,
            )?;
        }
    }

    let mut out = render_cropped_canvas(&canvas);
    append_callouts(&mut out, &callouts);
    Ok(out)
}

fn place_label(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    width: usize,
    text: &str,
    callouts: &mut Vec<String>,
) -> Result<()> {
    let text = if text.trim().is_empty() {
        "(unnamed)".to_owned()
    } else {
        text.to_owned()
    };
    if str_display_width(&text) <= width {
        return canvas.set_text(x, y, &text);
    }
    callouts.push(text);
    let marker = format!("[{}]", callouts.len());
    if str_display_width(&marker) <= width {
        canvas.set_text(x, y, &marker)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn place_connected_label(
    canvas: &mut Canvas,
    slot_x: usize,
    bone_x: usize,
    y: usize,
    depth: usize,
    text: &str,
    charset: Charset,
    callouts: &mut Vec<String>,
) -> Result<()> {
    let text = if text.trim().is_empty() {
        "(unnamed)".to_owned()
    } else {
        text.to_owned()
    };
    let connector = 1usize.saturating_add(depth.saturating_sub(1).saturating_mul(2));
    let available = bone_x.saturating_sub(slot_x).saturating_sub(connector);
    let label = if str_display_width(&text) <= available {
        text
    } else {
        callouts.push(text);
        format!("[{}]", callouts.len())
    };
    let label_width = str_display_width(&label);
    if label_width > available {
        return Ok(());
    }
    let label_x = bone_x - connector - label_width;
    canvas.set_text(label_x, y, &label)?;
    let connector_start = label_x + label_width;
    let connector_end = bone_x.saturating_sub(1);
    if connector_start <= connector_end {
        draw_horizontal_line(canvas, y, connector_start, connector_end, charset)?;
    }
    Ok(())
}

fn append_callouts(out: &mut String, callouts: &[String]) {
    if callouts.is_empty() {
        return;
    }
    out.push_str("\nLabels:\n");
    for (index, label) in callouts.iter().enumerate() {
        out.push_str(&format!("  [{}] {label}\n", index + 1));
    }
}

fn flatten_descendants(node: &IshikawaNodeRenderModel, depth: usize, out: &mut Vec<Descendant>) {
    for child in &node.children {
        out.push(Descendant {
            text: sanitize_label_text(&child.text),
            depth,
        });
        flatten_descendants(child, depth + 1, out);
    }
}

fn longest_cause_label(node: &IshikawaNodeRenderModel) -> usize {
    node.children.iter().map(longest_cause_label).fold(
        str_display_width(&sanitize_label_text(&node.text)),
        usize::max,
    )
}

fn compressed_slot_width(
    preferred: usize,
    branch_count: usize,
    fixed_width: usize,
    max_width: usize,
) -> usize {
    let available_per_branch = max_width
        .saturating_sub(fixed_width)
        .checked_div(branch_count)
        .unwrap_or(0);
    preferred.min(available_per_branch.max(MIN_SLOT_WIDTH))
}

fn interpolate_x(start_x: usize, start_y: usize, end_x: usize, end_y: usize, y: usize) -> usize {
    let distance = end_y.abs_diff(start_y).max(1);
    let offset = y.abs_diff(start_y).min(distance);
    start_x + end_x.saturating_sub(start_x) * offset / distance
}

fn validate_tree(root: &IshikawaNodeRenderModel) -> Result<()> {
    fn visit(node: &IshikawaNodeRenderModel, depth: usize, count: &mut usize) -> Result<()> {
        if depth > MAX_ISHIKAWA_DEPTH {
            return Err(MermansiError::RenderLimit {
                context: "ishikawa depth",
                requested: depth,
                limit: MAX_ISHIKAWA_DEPTH,
            });
        }
        *count = count.saturating_add(1);
        if *count > MAX_ISHIKAWA_NODES {
            return Err(MermansiError::RenderLimit {
                context: "ishikawa nodes",
                requested: *count,
                limit: MAX_ISHIKAWA_NODES,
            });
        }
        for child in &node.children {
            visit(child, depth + 1, count)?;
        }
        Ok(())
    }

    let mut count = 0;
    visit(root, 0, &mut count)
}

fn ensure_dimension(context: &'static str, requested: usize, limit: usize) -> Result<()> {
    if requested > limit {
        return Err(MermansiError::RenderLimit {
            context,
            requested,
            limit,
        });
    }
    Ok(())
}
