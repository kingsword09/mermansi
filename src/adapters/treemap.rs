//! Treemap adapter.

use crate::adapters::format_title;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::treemap::{TreemapDiagramRenderModel, TreemapNodeRenderModel};

pub fn render_treemap(model: &TreemapDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    render_node(&model.root, 0, &mut out);

    if out.trim().is_empty() {
        out.push_str("(empty treemap)\n");
    }

    let _ = opts;
    Ok(out)
}

fn render_node(node: &TreemapNodeRenderModel, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let value_str = node
        .value
        .as_ref()
        .map(|v| match v {
            serde_json::Value::Number(n) => format!(" = {}", n),
            serde_json::Value::String(s) => format!(" = {s}"),
            other => format!(" = {other}"),
        })
        .unwrap_or_default();

    out.push_str(&format!("{}{}{}\n", indent, node.name, value_str));

    if let Some(children) = &node.children {
        for child in children {
            render_node(child, depth + 1, out);
        }
    }
}
