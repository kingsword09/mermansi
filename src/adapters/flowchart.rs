//! Flowchart geometry preview with a lossless structured fallback.

use crate::adapters::{box_geometry, to_ascii_options};
use crate::ansi::sanitize_label_text;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_ascii::AsciiError;
use merman_core::diagrams::flowchart::FlowchartV2Model;

mod fallback;
mod lanes;

pub fn render_flowchart(model: &FlowchartV2Model, opts: &MermansiOptions) -> Result<String> {
    box_geometry::ensure_inventory(
        model.nodes.len().saturating_add(model.subgraphs.len()),
        model.edges.len(),
    )?;

    // merman-ascii can route plain rectangles, but its edge painter overwrites the
    // corners of shaped nodes. Keep every shaped topology on the native bounded path.
    if !model.subgraphs.is_empty()
        || model.nodes.iter().any(has_explicit_shape)
        || requires_terminal_text_normalization(model)
    {
        return fallback::render(model, opts);
    }

    if lanes::requires_lane_geometry(model)
        && let Some(output) = lanes::render_lane_geometry(model, opts)?
    {
        return Ok(output);
    }

    match merman_ascii::render_flowchart(model, &to_ascii_options(opts)) {
        Ok(output) if !output.trim().is_empty() => Ok(output),
        Ok(_) | Err(AsciiError::UnsupportedFeature { .. }) => fallback::render(model, opts),
        Err(error) => Err(error.into()),
    }
}

fn requires_terminal_text_normalization(model: &FlowchartV2Model) -> bool {
    model.nodes.iter().any(|node| {
        node.label
            .as_deref()
            .is_some_and(|label| sanitize_label_text(label) != label)
    }) || model.edges.iter().any(|edge| {
        edge.label
            .as_deref()
            .is_some_and(|label| sanitize_label_text(label) != label)
    })
}

fn has_explicit_shape(node: &merman_core::diagrams::flowchart::FlowNode) -> bool {
    !matches!(
        node.layout_shape.as_deref(),
        None | Some("squareRect" | "rect" | "rectangle")
    )
}
