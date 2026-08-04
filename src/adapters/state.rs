//! State terminal geometry.
//!
//! Simple models reuse `merman-ascii`. Composite and otherwise unsupported models use the shared
//! bounded box router so transition labels cannot overwrite group borders or state labels.

use std::collections::{HashMap, HashSet};

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxGroup, BoxLayout, BoxNode, BoxNodeShape,
    EdgeLegend, EdgeMarker, EdgeStyle,
};
use crate::adapters::{nonempty_or, to_ascii_options};
use crate::ansi::{sanitize_label_text, strip_ansi};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use serde_json::Value;

const MAX_STATE_NODES: usize = 4_096;
const MAX_STATE_EDGES: usize = 8_192;

pub fn render_state(model: &StateDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    ensure_state_limits(model)?;
    if requires_native_geometry(model) {
        return render_composite_state(model, opts);
    }

    let geometry_model = state_geometry_model(model);
    let output = match merman_ascii::render_state(&geometry_model, &to_ascii_options(opts)) {
        Ok(output) => output,
        Err(merman_ascii::AsciiError::UnsupportedFeature { .. }) => {
            return render_composite_state(model, opts);
        }
        Err(error) => return Err(error.into()),
    };
    if opts.charset == Charset::Ascii {
        return Ok(output);
    }

    Ok(close_pseudostate_borders(output))
}

fn state_geometry_model(model: &StateDiagramRenderModel) -> StateDiagramRenderModel {
    let mut geometry = model.clone();
    geometry.acc_title = geometry.acc_title.as_deref().map(sanitize_label_text);
    geometry.acc_descr = geometry.acc_descr.as_deref().map(sanitize_label_text);
    for node in &mut geometry.nodes {
        if let Some(label) = &mut node.label {
            sanitize_label_value(label);
        }
        if let Some(description) = &mut node.description {
            for line in description {
                *line = sanitize_label_text(line);
            }
        }
    }
    for edge in &mut geometry.edges {
        edge.label = sanitize_label_text(&edge.label);
    }
    geometry
}

fn sanitize_label_value(value: &mut Value) {
    match value {
        Value::String(text) => *text = sanitize_label_text(text),
        Value::Array(values) => values.iter_mut().for_each(sanitize_label_value),
        Value::Object(values) => values.values_mut().for_each(sanitize_label_value),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn requires_native_geometry(model: &StateDiagramRenderModel) -> bool {
    model.nodes.iter().any(|node| node.is_group)
}

fn ensure_state_limits(model: &StateDiagramRenderModel) -> Result<()> {
    for (context, requested, limit) in [
        ("state nodes", model.nodes.len(), MAX_STATE_NODES),
        ("state edges", model.edges.len(), MAX_STATE_EDGES),
    ] {
        if requested > limit {
            return Err(MermansiError::RenderLimit {
                context,
                requested,
                limit,
            });
        }
    }
    Ok(())
}

fn render_composite_state(
    model: &StateDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let child_counts = model.nodes.iter().fold(HashMap::new(), |mut counts, node| {
        if let Some(parent) = node.parent_id.as_ref() {
            *counts.entry(parent.as_str()).or_insert(0usize) += 1;
        }
        counts
    });
    let note_parent_by_child = model
        .nodes
        .iter()
        .filter(|node| node.shape == "note")
        .filter_map(|node| {
            node.parent_id
                .as_deref()
                .map(|parent| (node.id.as_str(), parent))
        })
        .collect::<HashMap<_, _>>();
    let group_ids = model
        .nodes
        .iter()
        .filter(|node| {
            node.is_group
                && node.shape != "noteGroup"
                && child_counts.get(node.id.as_str()).copied().unwrap_or(0) > 0
        })
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let direction = BoxDirection::from_str(&model.direction);
    let (from_side, to_side) = direction.edge_sides();

    let mut groups = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| group_ids.contains(node.id.as_str()))
        .map(|(order, node)| BoxGroup {
            id: node.id.clone(),
            lines: group_label(node),
            parent: visible_parent(node, &group_ids),
            columns: group_columns(
                node.dir
                    .as_deref()
                    .map(BoxDirection::from_str)
                    .unwrap_or(direction),
            ),
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let mut nodes = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            !group_ids.contains(node.id.as_str())
                && !note_parent_by_child.contains_key(node.id.as_str())
        })
        .map(|(order, node)| BoxNode {
            id: node.id.clone(),
            lines: state_node_lines(node, opts.charset),
            dividers: Vec::new(),
            parent: visible_parent(node, &group_ids),
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let node_shapes = model
        .nodes
        .iter()
        .filter(|node| {
            !group_ids.contains(node.id.as_str())
                && !note_parent_by_child.contains_key(node.id.as_str())
        })
        .map(|node| (node.id.clone(), state_node_shape(node)))
        .collect::<HashMap<_, _>>();
    let edges = model
        .edges
        .iter()
        .map(|edge| BoxEdge {
            from: note_parent_by_child
                .get(edge.start.as_str())
                .copied()
                .unwrap_or(&edge.start)
                .to_owned(),
            to: note_parent_by_child
                .get(edge.end.as_str())
                .copied()
                .unwrap_or(&edge.end)
                .to_owned(),
            label: sanitize_label_text(&edge.label),
            marker_start: EdgeMarker::None,
            marker_end: if is_note_edge(edge) {
                EdgeMarker::None
            } else {
                EdgeMarker::Arrow
            },
            style: if is_note_edge(edge) {
                EdgeStyle::Dotted
            } else {
                EdgeStyle::Solid
            },
            from_side: Some(from_side),
            to_side: Some(to_side),
        })
        .collect::<Vec<_>>();
    let mut rank_entities = nodes.clone();
    rank_entities.extend(groups.iter().map(|group| BoxNode {
        id: group.id.clone(),
        lines: Vec::new(),
        dividers: Vec::new(),
        parent: group.parent.clone(),
        span: group.span,
        order: group.order,
    }));
    let ranks = box_geometry::directed_ranks(&rank_entities, &edges);
    let max_rank = ranks.values().copied().max().unwrap_or(0);
    let stride = model.nodes.len().saturating_add(1);
    for (source_order, node) in nodes.iter_mut().enumerate() {
        let rank = ranks.get(&node.id).copied().unwrap_or(max_rank);
        let rank = if matches!(direction, BoxDirection::Bt | BoxDirection::Rl) {
            max_rank.saturating_sub(rank)
        } else {
            rank
        };
        node.order = rank.saturating_mul(stride).saturating_add(source_order);
    }
    for group in &mut groups {
        let descendant_rank = nodes
            .iter()
            .filter(|node| node.parent.as_deref() == Some(group.id.as_str()))
            .filter_map(|node| ranks.get(&node.id))
            .copied()
            .min();
        if let Some(rank) = descendant_rank {
            group.order = rank.saturating_mul(stride).saturating_add(group.order);
        }
    }

    let mut diagram = BoxDiagram {
        family: "state",
        title: model.acc_title.clone(),
        nodes,
        groups,
        spacers: Vec::new(),
        edges,
        columns: None,
        layout: BoxLayout::Layered { direction, ranks },
        edge_legend: EdgeLegend::None,
    };
    let mut output = match box_geometry::render_with_node_shapes(&diagram, opts, &node_shapes) {
        Err(MermansiError::RenderLimit {
            context: "box geometry columns",
            ..
        }) => {
            diagram.layout = BoxLayout::Packed;
            box_geometry::render_with_node_shapes(&diagram, opts, &node_shapes)?
        }
        result => result?,
    };
    append_transition_details(&mut output, model, opts.max_width);
    Ok(output)
}

fn visible_parent(node: &StateDiagramRenderNode, group_ids: &HashSet<&str>) -> Option<String> {
    node.parent_id
        .as_ref()
        .filter(|parent| group_ids.contains(parent.as_str()))
        .cloned()
}

fn group_columns(direction: BoxDirection) -> Option<usize> {
    matches!(direction, BoxDirection::Tb | BoxDirection::Bt).then_some(1)
}

fn group_label(node: &StateDiagramRenderNode) -> Vec<String> {
    if node.shape == "divider" {
        return Vec::new();
    }
    vec![state_label(node)]
}

fn state_node_lines(node: &StateDiagramRenderNode, charset: Charset) -> Vec<String> {
    match (node.shape.as_str(), charset) {
        ("stateStart", Charset::Unicode) => vec!["●".to_owned()],
        ("stateStart", Charset::Ascii) => vec!["*".to_owned()],
        ("stateEnd", Charset::Unicode) => vec!["◎".to_owned()],
        ("stateEnd", Charset::Ascii) => vec!["@".to_owned()],
        ("fork" | "join", Charset::Unicode) => vec!["━━━━━━━━".to_owned()],
        ("fork" | "join", Charset::Ascii) => vec!["========".to_owned()],
        ("choice", Charset::Unicode) => vec!["◇".to_owned()],
        ("choice", Charset::Ascii) => vec!["<>".to_owned()],
        ("divider", _) => vec!["[concurrent region]".to_owned()],
        _ => state_label(node)
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    }
}

fn state_node_shape(node: &StateDiagramRenderNode) -> BoxNodeShape {
    match node.shape.as_str() {
        "rect" | "rectWithTitle" | "roundedWithTitle" | "note" | "noteGroup" => {
            BoxNodeShape::Rounded
        }
        "choice" => BoxNodeShape::Decision,
        _ => BoxNodeShape::Rectangle,
    }
}

fn state_label(node: &StateDiagramRenderNode) -> String {
    let mut lines = Vec::new();
    if let Some(label) = node.label.as_ref() {
        append_value_lines(&mut lines, label);
    }
    if let Some(description) = node.description.as_ref() {
        lines.extend(
            description
                .iter()
                .map(|line| sanitize_label_text(line))
                .filter(|line| !line.trim().is_empty()),
        );
    }
    lines.dedup();
    if lines.is_empty() {
        nonempty_or(&sanitize_label_text(&node.id), "(unnamed state)")
    } else {
        lines.join("\n")
    }
}

fn append_value_lines(lines: &mut Vec<String>, value: &Value) {
    match value {
        Value::String(text) => {
            let text = sanitize_label_text(text);
            if !text.trim().is_empty() {
                lines.push(text);
            }
        }
        Value::Array(values) => {
            for value in values {
                append_value_lines(lines, value);
            }
        }
        Value::Null => {}
        other => lines.push(sanitize_label_text(&other.to_string())),
    }
}

fn append_transition_details(
    output: &mut String,
    model: &StateDiagramRenderModel,
    max_width: usize,
) {
    let labels = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), transition_endpoint(node)))
        .collect::<HashMap<_, _>>();
    let labeled = model
        .edges
        .iter()
        .filter(|edge| !edge.label.trim().is_empty())
        .collect::<Vec<_>>();
    if labeled.is_empty() {
        return;
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("\nTransitions\n");
    for edge in labeled {
        let from = labels
            .get(edge.start.as_str())
            .cloned()
            .unwrap_or_else(|| sanitize_label_text(&edge.start));
        let to = labels
            .get(edge.end.as_str())
            .cloned()
            .unwrap_or_else(|| sanitize_label_text(&edge.end));
        let text = format!("{from} --> {to}: {}", sanitize_label_text(&edge.label));
        for line in box_geometry::wrap_words(&text, max_width.saturating_sub(2)) {
            output.push_str("  ");
            output.push_str(&line);
            output.push('\n');
        }
    }
}

fn transition_endpoint(node: &StateDiagramRenderNode) -> String {
    match node.shape.as_str() {
        "stateStart" => "[start]".to_owned(),
        "stateEnd" => "[end]".to_owned(),
        "fork" => "[fork]".to_owned(),
        "join" => "[join]".to_owned(),
        "choice" => "[choice]".to_owned(),
        "divider" => "[concurrent region]".to_owned(),
        _ => state_label(node),
    }
}

fn is_note_edge(edge: &StateDiagramRenderEdge) -> bool {
    edge.classes
        .split_whitespace()
        .any(|class| class == "note-edge")
}

fn close_pseudostate_borders(output: String) -> String {
    let has_trailing_newline = output.ends_with('\n');
    let mut lines = output.lines().map(str::to_owned).collect::<Vec<_>>();
    let pseudo_rows = lines
        .iter()
        .enumerate()
        .filter_map(|(row, line)| {
            let visible = strip_ansi(line);
            (visible.contains('●') || visible.contains('◎')).then_some(row)
        })
        .collect::<Vec<_>>();

    for row in pseudo_rows {
        if row == 0 || row + 1 >= lines.len() {
            continue;
        }
        let top = strip_ansi(&lines[row - 1]);
        let bottom = strip_ansi(&lines[row + 1]);
        if top.contains('╭') && top.contains('╮') && bottom.contains('╰') && bottom.contains('╯')
        {
            lines[row - 1] = lines[row - 1].replace('╭', "┌").replace('╮', "┐");
            lines[row + 1] = lines[row + 1].replace('╰', "└").replace('╯', "┘");
        }
    }

    let mut closed = lines.join("\n");
    if has_trailing_newline {
        closed.push('\n');
    }
    closed
}
