//! Block diagram terminal geometry.

use std::collections::HashMap;

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxEdge, BoxGroup, BoxLayout, BoxNode, BoxSpacer,
};
use crate::adapters::detail_separator;
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::block::{
    BlockDiagramRenderModel, BlockEdgeRenderModel, BlockNodeRenderModel,
};

pub fn render_block(model: &BlockDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let root = model
        .blocks_flat
        .iter()
        .find(|block| block.id == "root")
        .or_else(|| model.blocks_flat.first());
    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    let mut spacers = Vec::new();
    let mut order = 0usize;
    let mut stack = root
        .map(|root| {
            root.children
                .iter()
                .rev()
                .map(|child| (child, None::<String>, 0usize))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    while let Some((block, parent, depth)) = stack.pop() {
        if depth >= 64 {
            return Err(MermansiError::GeometryLayout {
                family: "block",
                message: "nesting exceeds 64 levels".to_owned(),
            });
        }
        let item_order = order;
        order += 1;
        if is_spacer(block) {
            spacers.push(BoxSpacer {
                parent,
                span: positive(block.width_in_columns).unwrap_or(1),
                order: item_order,
            });
            continue;
        }
        if block.block_type.eq_ignore_ascii_case("composite") {
            let mut lines = vec![display_identity(&block.id, &block.label, opts.charset)];
            let mut details = vec!["group".to_owned()];
            if let Some(columns) = positive(block.columns) {
                details.push(format!("{columns} columns"));
            }
            lines.push(format!(
                "[{}]",
                details.join(detail_separator(opts.charset))
            ));
            groups.push(BoxGroup {
                id: block.id.clone(),
                lines,
                parent: parent.clone(),
                columns: positive(block.columns),
                span: positive(block.width_in_columns).unwrap_or(1),
                order: item_order,
            });
            for child in block.children.iter().rev() {
                stack.push((child, Some(block.id.clone()), depth + 1));
            }
        } else {
            nodes.push(block_node(block, parent, item_order, opts.charset));
        }
    }

    let node_orders = nodes
        .iter()
        .map(|node| (node.id.as_str(), node.order))
        .collect::<HashMap<_, _>>();
    let edges = model
        .edges
        .iter()
        .map(|edge| block_edge(edge, &node_orders))
        .collect();

    box_geometry::render(
        &BoxDiagram {
            family: "block",
            title: Some("Block diagram".to_owned()),
            nodes,
            groups,
            spacers,
            edges,
            columns: root.and_then(|root| positive(root.columns)),
            layout: BoxLayout::Packed,
            edge_legend: box_geometry::EdgeLegend::Labeled,
        },
        opts,
    )
}

fn block_node(
    block: &BlockNodeRenderModel,
    parent: Option<String>,
    order: usize,
    charset: Charset,
) -> BoxNode {
    let mut lines = vec![display_identity(&block.id, &block.label, charset)];
    if !matches!(block.block_type.as_str(), "" | "na" | "square") {
        lines.push(format!("[{}]", block.block_type));
    }
    BoxNode {
        id: block.id.clone(),
        lines,
        dividers: Vec::new(),
        parent,
        span: positive(block.width_in_columns).unwrap_or(1),
        order,
    }
}

fn block_edge(edge: &BlockEdgeRenderModel, orders: &HashMap<&str, usize>) -> BoxEdge {
    let (from_side, to_side) = route_sides(edge, orders);
    BoxEdge {
        from: edge.start.clone(),
        to: edge.end.clone(),
        label: normalized(&edge.label),
        marker_start: if edge.arrow_type_start.as_deref() == Some("arrow_point") {
            box_geometry::EdgeMarker::Arrow
        } else {
            box_geometry::EdgeMarker::None
        },
        marker_end: if edge.arrow_type_end.as_deref() == Some("arrow_point") {
            box_geometry::EdgeMarker::Arrow
        } else {
            box_geometry::EdgeMarker::None
        },
        style: box_geometry::EdgeStyle::Solid,
        from_side,
        to_side,
    }
}

fn route_sides(
    edge: &BlockEdgeRenderModel,
    orders: &HashMap<&str, usize>,
) -> (Option<box_geometry::Side>, Option<box_geometry::Side>) {
    let (Some(&from), Some(&to)) = (
        orders.get(edge.start.as_str()),
        orders.get(edge.end.as_str()),
    ) else {
        return (None, None);
    };
    if from == to {
        return (
            Some(box_geometry::Side::Right),
            Some(box_geometry::Side::Left),
        );
    }
    if from.abs_diff(to) == 1 {
        return if from < to {
            (
                Some(box_geometry::Side::Right),
                Some(box_geometry::Side::Left),
            )
        } else {
            (
                Some(box_geometry::Side::Left),
                Some(box_geometry::Side::Right),
            )
        };
    }
    let side = if from.min(to).saturating_add(from.max(to)).is_multiple_of(2) {
        box_geometry::Side::Bottom
    } else {
        box_geometry::Side::Top
    };
    (Some(side), Some(side))
}

fn display_identity(id: &str, label: &str, charset: Charset) -> String {
    let label = normalized(label);
    if label.is_empty() || label == id {
        id.to_owned()
    } else {
        format!("{id}{}{label}", detail_separator(charset))
    }
}

fn is_spacer(block: &BlockNodeRenderModel) -> bool {
    block.block_type.eq_ignore_ascii_case("space")
}

fn positive(value: Option<i64>) -> Option<usize> {
    value
        .filter(|value| *value > 0)
        .and_then(|value| usize::try_from(value).ok())
}

fn normalized(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
