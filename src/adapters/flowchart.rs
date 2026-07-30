//! Flowchart adapter with a lossless structured fallback.

use crate::adapters::to_ascii_options;
use crate::error::Result;
use crate::options::MermansiOptions;
use crate::output::render_structured_model;
use merman_ascii::AsciiError;
use merman_core::diagrams::flowchart::FlowchartV2Model;
use std::collections::HashSet;

pub fn render_flowchart(model: &FlowchartV2Model, opts: &MermansiOptions) -> Result<String> {
    if has_parallel_edges(model) {
        return render_structured_model("flowchart", model, opts);
    }

    match merman_ascii::render_flowchart(model, &to_ascii_options(opts)) {
        Ok(output) => Ok(output),
        Err(AsciiError::UnsupportedFeature { .. }) => {
            render_structured_model("flowchart", model, opts)
        }
        Err(error) => Err(error.into()),
    }
}

fn has_parallel_edges(model: &FlowchartV2Model) -> bool {
    let mut endpoints = HashSet::with_capacity(model.edges.len());
    model
        .edges
        .iter()
        .any(|edge| !endpoints.insert((edge.from.as_str(), edge.to.as_str())))
}
