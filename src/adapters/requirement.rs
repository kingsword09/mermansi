//! Requirement diagram terminal geometry.
//!
//! Renders every requirement and element as a closed box, every typed relationship as a
//! connected directed route, and honors TB/BT/LR/RL direction.

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxLayout, BoxNode, directed_ranks,
};
use crate::adapters::nonempty_or;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::requirement::{
    RequirementDiagramRenderModel, RequirementRenderElement, RequirementRenderNode,
};

pub fn render_requirement(
    model: &RequirementDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let direction = BoxDirection::from_str(&model.direction);
    let (from_side, to_side) = direction.edge_sides();
    let mut nodes = Vec::new();
    for (order, req) in model.requirements.iter().enumerate() {
        nodes.push(requirement_node(req, order));
    }
    let node_order = model.requirements.len();
    for (order, elem) in model.elements.iter().enumerate() {
        nodes.push(element_node(elem, node_order + order));
    }
    let edges = model
        .relationships
        .iter()
        .map(|rel| BoxEdge {
            from: rel.src.clone(),
            to: rel.dst.clone(),
            label: rel.rel_type.clone(),
            arrow_start: false,
            arrow_end: true,
            from_side: Some(from_side),
            to_side: Some(to_side),
        })
        .collect::<Vec<_>>();
    let ranks = directed_ranks(&nodes, &edges);

    box_geometry::render(
        &BoxDiagram {
            family: "requirement",
            title: Some("Requirement diagram".to_owned()),
            nodes,
            groups: Vec::new(),
            spacers: Vec::new(),
            edges,
            columns: None,
            layout: BoxLayout::Layered { direction, ranks },
            show_edge_legend: true,
        },
        opts,
    )
}

fn requirement_node(req: &RequirementRenderNode, order: usize) -> BoxNode {
    let mut lines = Vec::new();
    lines.push(format!(
        "[{}] {}",
        req.node_type,
        nonempty_or(&req.name, "(unnamed)")
    ));
    if !req.requirement_id.is_empty() {
        lines.push(format!("id: {}", req.requirement_id));
    }
    if !req.text.is_empty() {
        lines.push(format!("text: {}", req.text));
    }
    if !req.risk.is_empty() {
        lines.push(format!("risk: {}", req.risk));
    }
    if !req.verify_method.is_empty() {
        lines.push(format!("verify: {}", req.verify_method));
    }
    BoxNode {
        id: req.name.clone(),
        lines,
        parent: None,
        span: 1,
        order,
    }
}

fn element_node(elem: &RequirementRenderElement, order: usize) -> BoxNode {
    let mut lines = Vec::new();
    lines.push(format!(
        "[{}] {}",
        elem.element_type,
        nonempty_or(&elem.name, "(unnamed)")
    ));
    if !elem.doc_ref.is_empty() {
        lines.push(format!("docRef: {}", elem.doc_ref));
    }
    BoxNode {
        id: elem.name.clone(),
        lines,
        parent: None,
        span: 1,
        order,
    }
}
