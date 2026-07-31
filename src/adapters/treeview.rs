//! TreeView terminal geometry.

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxLayout, BoxNode, directed_ranks,
};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

const MAX_TREE_DEPTH: usize = 64;
const MAX_TREE_NODES: usize = 4_096;

pub fn render_treeview(
    model: &TreeViewDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let direction = BoxDirection::Lr;
    let (from_side, to_side) = direction.edge_sides();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for child in &model.root.children {
        collect_tree(child, None, 0, &mut nodes, &mut edges)?;
    }
    for edge in &mut edges {
        edge.from_side = Some(from_side);
        edge.to_side = Some(to_side);
    }
    let ranks = directed_ranks(&nodes, &edges);

    box_geometry::render(
        &BoxDiagram {
            family: "treeView",
            title: model.title.clone().or_else(|| Some("Tree view".to_owned())),
            nodes,
            groups: Vec::new(),
            spacers: Vec::new(),
            edges,
            columns: None,
            layout: BoxLayout::Layered { direction, ranks },
            show_edge_legend: false,
        },
        opts,
    )
}

fn collect_tree(
    node: &TreeViewNodeRenderModel,
    parent_id: Option<&str>,
    depth: usize,
    nodes: &mut Vec<BoxNode>,
    edges: &mut Vec<BoxEdge>,
) -> Result<()> {
    if depth > MAX_TREE_DEPTH {
        return Err(MermansiError::RenderLimit {
            context: "treeView depth",
            requested: depth,
            limit: MAX_TREE_DEPTH,
        });
    }
    let requested = nodes.len().saturating_add(1);
    if requested > MAX_TREE_NODES {
        return Err(MermansiError::RenderLimit {
            context: "treeView nodes",
            requested,
            limit: MAX_TREE_NODES,
        });
    }

    let id = format!("tree-{}", nodes.len());
    let order = nodes.len();
    nodes.push(BoxNode {
        id: id.clone(),
        lines: vec![if node.name.trim().is_empty() {
            "(unnamed)".to_owned()
        } else {
            node.name.clone()
        }],
        parent: None,
        span: 1,
        order,
    });
    if let Some(parent) = parent_id {
        edges.push(BoxEdge {
            from: parent.to_owned(),
            to: id.clone(),
            label: String::new(),
            arrow_start: false,
            arrow_end: true,
            from_side: None,
            to_side: None,
        });
    }
    for child in &node.children {
        collect_tree(child, Some(&id), depth + 1, nodes, edges)?;
    }
    Ok(())
}
