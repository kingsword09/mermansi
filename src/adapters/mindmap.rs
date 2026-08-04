//! Mindmap terminal geometry.

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxLayout, BoxNode, directed_ranks,
};
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::mindmap::MindmapDiagramRenderModel;

pub fn render_mindmap(model: &MindmapDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    box_geometry::ensure_inventory(model.nodes.len(), model.edges.len())?;
    let direction = BoxDirection::Lr;
    let (from_side, to_side) = direction.edge_sides();
    let nodes = model
        .nodes
        .iter()
        .enumerate()
        .map(|(order, node)| BoxNode {
            id: node.id.clone(),
            lines: vec![if node.label.trim().is_empty() {
                "(unnamed)".to_owned()
            } else {
                node.label.clone()
            }],
            dividers: Vec::new(),
            parent: None,
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let edges = model
        .edges
        .iter()
        .map(|edge| BoxEdge {
            from: edge.start.clone(),
            to: edge.end.clone(),
            label: String::new(),
            marker_start: box_geometry::EdgeMarker::None,
            marker_end: box_geometry::EdgeMarker::Arrow,
            style: box_geometry::EdgeStyle::Solid,
            from_side: Some(from_side),
            to_side: Some(to_side),
        })
        .collect::<Vec<_>>();
    let ranks = directed_ranks(&nodes, &edges);

    box_geometry::render(
        &BoxDiagram {
            family: "mindmap",
            title: Some("Mindmap".to_owned()),
            nodes,
            groups: Vec::new(),
            spacers: Vec::new(),
            edges,
            columns: None,
            layout: BoxLayout::Layered { direction, ranks },
            edge_legend: box_geometry::EdgeLegend::None,
        },
        opts,
    )
}
