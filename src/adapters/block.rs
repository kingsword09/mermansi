//! Block diagram adapter.

use crate::adapters::{format_title, nonempty_or};
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::block::{BlockDiagramRenderModel, BlockNodeRenderModel};

pub fn render_block(model: &BlockDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&Some(String::new())));

    if !model.blocks_flat.is_empty() {
        out.push_str("Blocks:\n");
        for block in &model.blocks_flat {
            out.push_str(&format_block(block, 1));
        }
    }

    if !model.edges.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("Edges:\n");
        for edge in &model.edges {
            let label = if edge.label.is_empty() {
                ""
            } else {
                &edge.label
            };
            out.push_str(&format!("  {} --{}--> {}\n", edge.start, label, edge.end));
        }
    }

    if out.trim().is_empty() {
        out.push_str("(empty block diagram)\n");
    }

    let _ = opts;
    Ok(out)
}

fn format_block(block: &BlockNodeRenderModel, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let label = nonempty_or(&block.label, &block.id);
    let type_str = if block.block_type.is_empty() {
        ""
    } else {
        &block.block_type
    };
    let mut out = format!("{indent}[{type_str}] {label} ({})\n", block.id);
    for child in &block.children {
        out.push_str(&format_block(child, depth + 1));
    }
    out
}
