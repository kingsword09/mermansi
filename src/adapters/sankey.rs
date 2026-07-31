//! Sankey diagram adapter.

use crate::adapters::{align_left_display, align_right_display};
use crate::ansi::sanitize_label_text;
use crate::error::Result;
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::sankey::SankeyDiagramRenderModel;

pub fn render_sankey(model: &SankeyDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();

    if model.graph.nodes.is_empty() && model.graph.links.is_empty() {
        out.push_str("(empty sankey diagram)\n");
        let _ = opts;
        return Ok(out);
    }

    if !model.graph.nodes.is_empty() {
        out.push_str("Nodes:\n");
        for node in &model.graph.nodes {
            out.push_str(&format!("  {}\n", sanitize_label_text(&node.id)));
        }
        out.push('\n');
    }

    if !model.graph.links.is_empty() {
        let links = model
            .graph
            .links
            .iter()
            .map(|link| {
                (
                    sanitize_label_text(&link.source),
                    sanitize_label_text(&link.target),
                    sanitize_label_text(&format_json_value(&link.value)),
                )
            })
            .collect::<Vec<_>>();
        let source_width = links
            .iter()
            .map(|(source, _, _)| str_display_width(source))
            .max()
            .unwrap_or_default()
            .max(20);
        let target_width = links
            .iter()
            .map(|(_, target, _)| str_display_width(target))
            .max()
            .unwrap_or_default()
            .max(20);
        let value_width = links
            .iter()
            .map(|(_, _, value)| str_display_width(value))
            .max()
            .unwrap_or_default()
            .max(10);
        out.push_str("Flows:\n");
        out.push_str(&format!(
            "{} {} {}\n",
            align_left_display("Source", source_width),
            align_right_display("Target", target_width),
            align_right_display("Value", value_width),
        ));
        out.push_str(&"-".repeat(source_width + target_width + value_width + 2));
        out.push('\n');
        for (source, target, value) in links {
            out.push_str(&format!(
                "{} {} {}\n",
                align_left_display(&source, source_width),
                align_right_display(&target, target_width),
                align_right_display(&value, value_width),
            ));
        }
    }

    let _ = opts;
    Ok(out)
}

fn format_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                format!("{f:.2}")
            } else {
                n.to_string()
            }
        }
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
