//! Architecture diagram adapter.
//!
//! Renders architecture nodes, groups, and edges as structured terminal text.

use crate::adapters::{format_title, nonempty_or};
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::architecture::{ArchitectureDiagramRenderModel, ArchitectureRenderEdge};

pub fn render_architecture(
    model: &ArchitectureDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    if !model.nodes.is_empty() {
        out.push_str("Nodes:\n");
        for node in &model.nodes {
            let type_str = match node.node_type {
                merman_core::diagrams::architecture::ArchitectureRenderNodeType::Service => {
                    "service"
                }
                merman_core::diagrams::architecture::ArchitectureRenderNodeType::Junction => {
                    "junction"
                }
            };
            let title = node.title.as_deref().unwrap_or(&node.id);
            out.push_str(&format!(
                "  [{type_str}] {} ({})\n",
                nonempty_or(title, &node.id),
                node.id
            ));
        }
        out.push('\n');
    }

    if !model.groups.is_empty() {
        out.push_str("Groups:\n");
        for group in &model.groups {
            let title = group.title.as_deref().unwrap_or(&group.id);
            let parent = group.in_group.as_deref().unwrap_or("-");
            out.push_str(&format!("  [{title}] ({}) parent: {parent}\n", group.id));
        }
        out.push('\n');
    }

    if !model.edges.is_empty() {
        out.push_str("Edges:\n");
        for edge in &model.edges {
            out.push_str(&format_edge(edge));
        }
    }

    if out.trim().is_empty() {
        out.push_str("(empty architecture diagram)\n");
    }

    let _ = opts;
    Ok(out)
}

fn format_edge(edge: &ArchitectureRenderEdge) -> String {
    let label = edge.title.as_deref().unwrap_or("");
    let connector = match (
        edge.lhs_into.unwrap_or_default(),
        edge.rhs_into.unwrap_or_default(),
    ) {
        (true, true) => "<-->",
        (true, false) => "<--",
        (false, true) => "-->",
        (false, false) => "--",
    };
    let label = if label.is_empty() {
        String::new()
    } else {
        format!(" [{label}]")
    };
    format!(
        "  {}:{} {connector} {}:{}{label}\n",
        edge.lhs_id, edge.lhs_dir, edge.rhs_dir, edge.rhs_id
    )
}
