//! Sankey diagram adapter.

use crate::error::Result;
use crate::options::MermansiOptions;
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
            out.push_str(&format!("  {}\n", node.id));
        }
        out.push('\n');
    }

    if !model.graph.links.is_empty() {
        out.push_str("Flows:\n");
        out.push_str(&format!(
            "{:<20} {:>20} {:>10}\n",
            "Source", "Target", "Value"
        ));
        out.push_str(&"-".repeat(52));
        out.push('\n');
        for link in &model.graph.links {
            let value_str = format_json_value(&link.value);
            out.push_str(&format!(
                "{:<20} {:>20} {:>10}\n",
                link.source, link.target, value_str
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
