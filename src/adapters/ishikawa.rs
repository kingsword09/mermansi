//! Ishikawa (fishbone) diagram adapter.

use crate::adapters::format_title;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::ishikawa::{IshikawaDiagramRenderModel, IshikawaNodeRenderModel};

pub fn render_ishikawa(
    model: &IshikawaDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    match &model.root {
        Some(root) => {
            out.push_str(&format!("Effect: {}\n", root.text));
            if !root.children.is_empty() {
                out.push_str("Causes:\n");
                for child in &root.children {
                    render_node(child, 1, opts.charset, &mut out);
                }
            } else {
                out.push_str("(no causes)\n");
            }
        }
        None => {
            out.push_str("(empty ishikawa diagram)\n");
        }
    }

    Ok(out)
}

fn render_node(node: &IshikawaNodeRenderModel, depth: usize, charset: Charset, out: &mut String) {
    let (branch, indent) = match charset {
        Charset::Unicode => ("├─ ", "│   "),
        Charset::Ascii => ("+- ", "|   "),
    };
    let prefix = "  ".to_string() + &indent.repeat(depth.saturating_sub(1)) + branch;
    out.push_str(&format!("{prefix}{}\n", node.text));
    for child in &node.children {
        render_node(child, depth + 1, charset, out);
    }
}
