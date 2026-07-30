//! Venn diagram adapter.

use crate::adapters::format_title;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::venn::VennDiagramRenderModel;

pub fn render_venn(model: &VennDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    if !model.subsets.is_empty() {
        out.push_str("Subsets:\n");
        for subset in &model.subsets {
            let separator = match opts.charset {
                Charset::Unicode => " ∩ ",
                Charset::Ascii => " & ",
            };
            let sets_str = subset.sets.join(separator);
            let label = subset
                .label
                .as_deref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {{{sets_str}}} size={:.1}{label}\n",
                subset.size
            ));
        }
        out.push('\n');
    }

    if !model.text_nodes.is_empty() {
        out.push_str("Text Nodes:\n");
        for node in &model.text_nodes {
            let separator = match opts.charset {
                Charset::Unicode => " ∩ ",
                Charset::Ascii => " & ",
            };
            let sets_str = node.sets.join(separator);
            let label = node
                .label
                .as_deref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            out.push_str(&format!("  {{{sets_str}}} {node}{label}\n", node = node.id));
        }
    }

    if out.trim().is_empty() {
        out.push_str("(empty venn diagram)\n");
    }

    Ok(out)
}
