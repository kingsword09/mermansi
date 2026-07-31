//! Treemap terminal geometry.
//!
//! A deterministic slice-and-dice layout draws every semantic node as a closed rectangle nested
//! inside its parent. Positive numeric values control sibling area; missing values use subtree
//! weight. Labels that cannot fit receive connected, deterministic callout identifiers.

use unicode_segmentation::UnicodeSegmentation;

use crate::adapters::chart_primitives::{checked_chart_dimensions, render_cropped_canvas};
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::treemap::{TreemapDiagramRenderModel, TreemapNodeRenderModel};
use serde_json::Value;

const MAX_TREEMAP_NODES: usize = 4_096;
const MAX_TREEMAP_DEPTH: usize = 64;
const MIN_CANVAS_WIDTH: usize = 28;
const MIN_CANVAS_HEIGHT: usize = 10;
const MIN_CHILD_WIDTH: usize = 4;
const MIN_CHILD_HEIGHT: usize = 3;

#[derive(Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

pub fn render_treemap(model: &TreemapDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let (node_count, depth) = validate_tree(&model.root)?;
    let root_children = children(&model.root);
    if node_count == 1 && model.root.name.trim().is_empty() && root_children.is_empty() {
        return Ok("(empty treemap)\n".to_owned());
    }

    let longest_label = longest_label(&model.root).clamp(1, 24);
    let top_level_count = root_children.len().max(1);
    let preferred_width = top_level_count
        .saturating_mul(longest_label.saturating_add(5))
        .saturating_add(2)
        .clamp(MIN_CANVAS_WIDTH, 96);
    let preferred_height = 10usize
        .saturating_add(depth.saturating_mul(5))
        .clamp(MIN_CANVAS_HEIGHT, 32);
    let (width, height) = checked_chart_dimensions(
        opts,
        (MIN_CANVAS_WIDTH, MIN_CANVAS_HEIGHT),
        (preferred_width, preferred_height),
    )?;

    let mut canvas = Canvas::new(width, height)?;
    let outer = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    draw_box(
        &mut canvas,
        outer.x,
        outer.y,
        outer.width,
        outer.height,
        opts.charset,
    )?;
    let mut callouts = Vec::new();
    let container_label = model
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .map(sanitize_label_text)
        .unwrap_or_else(|| {
            let root = sanitize_label_text(&model.root.name);
            if root.is_empty() {
                "Treemap".to_owned()
            } else {
                root
            }
        });
    write_label(&mut canvas, outer, &container_label, &mut callouts)?;
    let content = Rect {
        x: 1,
        y: 2,
        width: width - 2,
        height: height - 3,
    };
    if root_children.is_empty() {
        write_node_details(&mut canvas, outer, &model.root, &mut callouts)?;
    } else {
        layout_children(&mut canvas, root_children, content, 0, opts, &mut callouts)?;
    }

    let mut out = render_cropped_canvas(&canvas);
    append_callouts(&mut out, &callouts, opts.max_width);
    Ok(out)
}

fn layout_children(
    canvas: &mut Canvas,
    nodes: &[TreemapNodeRenderModel],
    rect: Rect,
    depth: usize,
    opts: &MermansiOptions,
    callouts: &mut Vec<String>,
) -> Result<()> {
    if nodes.is_empty() {
        return Ok(());
    }
    let split_width = rect.width >= rect.height.saturating_mul(2);
    let available = if split_width { rect.width } else { rect.height };
    let minimum = if split_width {
        MIN_CHILD_WIDTH
    } else {
        MIN_CHILD_HEIGHT
    };
    let weights = nodes.iter().map(node_weight).collect::<Vec<_>>();
    let spans = partition_spans(available, &weights, minimum).ok_or_else(|| {
        MermansiError::GeometryLayout {
            family: "treemap",
            message: format!(
                "{} siblings do not fit in a {}x{} parent rectangle",
                nodes.len(),
                rect.width,
                rect.height
            ),
        }
    })?;

    let mut cursor = if split_width { rect.x } else { rect.y };
    for (node, span) in nodes.iter().zip(spans) {
        let child = if split_width {
            Rect {
                x: cursor,
                y: rect.y,
                width: span,
                height: rect.height,
            }
        } else {
            Rect {
                x: rect.x,
                y: cursor,
                width: rect.width,
                height: span,
            }
        };
        draw_node(canvas, node, child, depth, opts, callouts)?;
        cursor += span;
    }
    Ok(())
}

fn draw_node(
    canvas: &mut Canvas,
    node: &TreemapNodeRenderModel,
    rect: Rect,
    depth: usize,
    opts: &MermansiOptions,
    callouts: &mut Vec<String>,
) -> Result<()> {
    draw_box(
        canvas,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        opts.charset,
    )?;
    write_node_details(canvas, rect, node, callouts)?;

    let child_nodes = children(node);
    if child_nodes.is_empty() {
        return Ok(());
    }
    if rect.width < MIN_CHILD_WIDTH + 2 || rect.height < MIN_CHILD_HEIGHT + 3 {
        return Err(MermansiError::GeometryLayout {
            family: "treemap",
            message: format!(
                "nested children of '{}' do not fit in their parent rectangle",
                sanitize_label_text(&node.name)
            ),
        });
    }
    layout_children(
        canvas,
        child_nodes,
        Rect {
            x: rect.x + 1,
            y: rect.y + 2,
            width: rect.width - 2,
            height: rect.height - 3,
        },
        depth + 1,
        opts,
        callouts,
    )
}

fn write_node_details(
    canvas: &mut Canvas,
    rect: Rect,
    node: &TreemapNodeRenderModel,
    callouts: &mut Vec<String>,
) -> Result<()> {
    let label = node_label(node);
    write_label(canvas, rect, &label, callouts)
}

fn write_label(
    canvas: &mut Canvas,
    rect: Rect,
    label: &str,
    callouts: &mut Vec<String>,
) -> Result<()> {
    let available = rect.width.saturating_sub(2);
    if available == 0 || rect.height < 3 {
        return Ok(());
    }
    if str_display_width(label) <= available {
        return canvas.set_text(rect.x + 1, rect.y + 1, label);
    }
    callouts.push(label.to_owned());
    let marker = format!("[{}]", callouts.len());
    if str_display_width(&marker) <= available {
        canvas.set_text(rect.x + 1, rect.y + 1, &marker)?;
    }
    Ok(())
}

fn append_callouts(out: &mut String, callouts: &[String], max_width: usize) {
    if callouts.is_empty() {
        return;
    }
    out.push_str("\nLabels:\n");
    let content_width = max_width.saturating_sub(6).max(1);
    for (index, label) in callouts.iter().enumerate() {
        for (line_index, line) in wrap_display(label, content_width).iter().enumerate() {
            if line_index == 0 {
                out.push_str(&format!("  [{}] {line}\n", index + 1));
            } else {
                out.push_str(&format!("      {line}\n"));
            }
        }
    }
}

fn partition_spans(total: usize, weights: &[f64], minimum: usize) -> Option<Vec<usize>> {
    let reserved = weights.len().checked_mul(minimum)?;
    let remaining = total.checked_sub(reserved)?;
    let weight_sum = weights.iter().sum::<f64>();
    let mut spans = Vec::with_capacity(weights.len());
    let mut distributed = 0usize;
    for (index, weight) in weights.iter().enumerate() {
        let extra = if index + 1 == weights.len() {
            remaining.saturating_sub(distributed)
        } else if weight_sum > 0.0 {
            (remaining as f64 * *weight / weight_sum).floor() as usize
        } else {
            remaining / weights.len()
        };
        spans.push(minimum + extra);
        distributed += extra;
    }
    Some(spans)
}

fn validate_tree(root: &TreemapNodeRenderModel) -> Result<(usize, usize)> {
    fn visit(node: &TreemapNodeRenderModel, depth: usize, count: &mut usize) -> Result<usize> {
        if depth > MAX_TREEMAP_DEPTH {
            return Err(MermansiError::RenderLimit {
                context: "treemap depth",
                requested: depth,
                limit: MAX_TREEMAP_DEPTH,
            });
        }
        *count = count.saturating_add(1);
        if *count > MAX_TREEMAP_NODES {
            return Err(MermansiError::RenderLimit {
                context: "treemap nodes",
                requested: *count,
                limit: MAX_TREEMAP_NODES,
            });
        }
        let mut maximum = depth;
        for child in children(node) {
            maximum = maximum.max(visit(child, depth + 1, count)?);
        }
        Ok(maximum)
    }

    let mut count = 0;
    let depth = visit(root, 0, &mut count)?;
    Ok((count, depth))
}

fn children(node: &TreemapNodeRenderModel) -> &[TreemapNodeRenderModel] {
    node.children.as_deref().unwrap_or_default()
}

fn node_weight(node: &TreemapNodeRenderModel) -> f64 {
    numeric_value(node.value.as_ref()).unwrap_or_else(|| {
        let total = children(node).iter().map(node_weight).sum::<f64>();
        if total > 0.0 { total } else { 1.0 }
    })
}

fn numeric_value(value: Option<&Value>) -> Option<f64> {
    let value = match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(number) => number.parse().ok(),
        _ => None,
    }?;
    (value.is_finite() && value > 0.0).then_some(value)
}

fn node_label(node: &TreemapNodeRenderModel) -> String {
    let name = sanitize_label_text(&node.name);
    let mut label = if name.is_empty() {
        "(unnamed)".to_owned()
    } else {
        name
    };
    if let Some(value) = &node.value {
        label.push_str(" = ");
        label.push_str(&format_value(value));
    }
    if let Some(class) = node.class_selector.as_deref() {
        let class = sanitize_label_text(class);
        if !class.is_empty() {
            label.push_str(" · class ");
            label.push_str(&class);
        }
    }
    if let Some(styles) = &node.css_compiled_styles
        && !styles.is_empty()
    {
        label.push_str(" · ");
        label.push_str(
            &styles
                .iter()
                .map(|style| sanitize_label_text(style))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    label
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(text) => sanitize_label_text(text),
        other => sanitize_label_text(&other.to_string()),
    }
}

fn longest_label(node: &TreemapNodeRenderModel) -> usize {
    children(node)
        .iter()
        .map(longest_label)
        .fold(str_display_width(&node_label(node)), usize::max)
}

fn wrap_display(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut used = 0usize;
    for grapheme in UnicodeSegmentation::graphemes(text, true) {
        let grapheme_width = str_display_width(grapheme).max(1);
        if used > 0 && used.saturating_add(grapheme_width) > width {
            lines.push(String::new());
            used = 0;
        }
        if let Some(line) = lines.last_mut() {
            line.push_str(grapheme);
        }
        used += grapheme_width;
    }
    lines
}
