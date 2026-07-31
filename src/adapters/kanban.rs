//! Kanban board terminal geometry.

use std::collections::HashMap;

use crate::adapters::box_geometry::{self, BoxDiagram, BoxGroup, BoxLayout, BoxNode};
use crate::adapters::{detail_separator, nonempty_or};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};

const MAX_KANBAN_NODES: usize = 4_096;

pub fn render_kanban(model: &KanbanDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    if model.nodes.len() > MAX_KANBAN_NODES {
        return Err(MermansiError::RenderLimit {
            context: "kanban nodes",
            requested: model.nodes.len(),
            limit: MAX_KANBAN_NODES,
        });
    }
    if model.nodes.is_empty() {
        return Ok("Kanban\n\n(empty board)\n".to_owned());
    }

    let mut group_ids = HashMap::<&str, String>::new();
    for (index, node) in model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.is_group)
    {
        group_ids
            .entry(node.id.as_str())
            .or_insert_with(|| geometry_id(index));
    }
    let groups = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.is_group)
        .map(|(order, node)| BoxGroup {
            id: geometry_id(order),
            lines: node_lines(node, opts.charset),
            parent: None,
            columns: Some(1),
            span: 1,
            order,
        })
        .collect();
    let nodes = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| !node.is_group)
        .map(|(order, node)| {
            let parent = node
                .parent_id
                .as_ref()
                .and_then(|parent| group_ids.get(parent.as_str()))
                .cloned();
            let mut lines = node_lines(node, opts.charset);
            if parent.is_none()
                && let Some(parent_id) = normalized_option(node.parent_id.as_deref())
            {
                lines.push(format!("column: {parent_id}"));
            }
            BoxNode {
                id: geometry_id(order),
                lines,
                dividers: Vec::new(),
                parent,
                span: 1,
                order,
            }
        })
        .collect();

    box_geometry::render(
        &BoxDiagram {
            family: "kanban",
            title: Some("Kanban".to_owned()),
            nodes,
            groups,
            spacers: Vec::new(),
            edges: Vec::new(),
            columns: None,
            layout: BoxLayout::Packed,
            edge_legend: box_geometry::EdgeLegend::None,
        },
        opts,
    )
}

fn geometry_id(index: usize) -> String {
    format!("kanban-{index}")
}

fn node_lines(node: &KanbanRenderNode, charset: Charset) -> Vec<String> {
    let mut lines = vec![display_identity(node, charset)];
    for (key, value) in [
        ("ticket", node.ticket.as_deref()),
        ("priority", node.priority.as_deref()),
        ("assigned", node.assigned.as_deref()),
        ("icon", node.icon.as_deref()),
    ] {
        if let Some(value) = normalized_option(value) {
            lines.push(format!("{key}: {value}"));
        }
    }
    lines
}

fn display_identity(node: &KanbanRenderNode, charset: Charset) -> String {
    let id = node.id.trim();
    let label = node.label.trim();
    if label.is_empty() {
        return nonempty_or(id, "(unnamed)");
    }
    if id.is_empty() || id == label {
        label.to_owned()
    } else {
        format!("{id}{}{label}", detail_separator(charset))
    }
}

fn normalized_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
