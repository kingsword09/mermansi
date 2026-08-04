//! Sankey terminal geometry.
//!
//! Nodes are closed boxes arranged by flow rank. Every link is a connected directed route whose
//! label includes the exact parsed value and a proportional terminal bar.

use std::collections::HashMap;

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxLayout, BoxNode, directed_ranks,
};
use crate::ansi::sanitize_label_text;
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::sankey::SankeyDiagramRenderModel;
use serde_json::Value;

const MAX_SANKEY_NODES: usize = 4_096;
const MAX_SANKEY_LINKS: usize = 4_096;
const MAX_FLOW_BAR: usize = 8;

pub fn render_sankey(model: &SankeyDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    ensure_limit("sankey nodes", model.graph.nodes.len(), MAX_SANKEY_NODES)?;
    ensure_limit("sankey links", model.graph.links.len(), MAX_SANKEY_LINKS)?;
    if model.graph.nodes.is_empty() {
        return Ok("(empty sankey diagram)\n".to_owned());
    }

    let mut geometry_ids = HashMap::new();
    for node in &model.graph.nodes {
        if geometry_ids
            .insert(node.id.as_str(), node.id.clone())
            .is_some()
        {
            return Err(MermansiError::GeometryLayout {
                family: "sankey",
                message: format!(
                    "duplicate node id cannot be routed unambiguously: {}",
                    sanitize_label_text(&node.id)
                ),
            });
        }
    }
    for link in &model.graph.links {
        if !geometry_ids.contains_key(link.source.as_str())
            || !geometry_ids.contains_key(link.target.as_str())
        {
            return Err(MermansiError::GeometryLayout {
                family: "sankey",
                message: format!(
                    "link endpoint is not present in the node inventory: {} -> {}",
                    sanitize_label_text(&link.source),
                    sanitize_label_text(&link.target)
                ),
            });
        }
    }

    let numeric_links = model
        .graph
        .links
        .iter()
        .map(|link| finite_nonnegative(&link.value))
        .collect::<Vec<_>>();
    let maximum_link = numeric_links
        .iter()
        .flatten()
        .copied()
        .fold(0.0_f64, f64::max);
    let mut incoming = vec![0.0_f64; model.graph.nodes.len()];
    let mut outgoing = vec![0.0_f64; model.graph.nodes.len()];
    let node_indices = model
        .graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    for (link, value) in model.graph.links.iter().zip(&numeric_links) {
        let Some(value) = value else {
            continue;
        };
        outgoing[node_indices[link.source.as_str()]] += value;
        incoming[node_indices[link.target.as_str()]] += value;
    }
    let maximum_node = incoming
        .iter()
        .zip(&outgoing)
        .map(|(incoming, outgoing)| incoming.max(*outgoing))
        .fold(0.0_f64, f64::max);

    let fill = match opts.charset {
        Charset::Unicode => "█",
        Charset::Ascii => "#",
    };
    let nodes = model
        .graph
        .nodes
        .iter()
        .enumerate()
        .map(|(order, node)| {
            let total = incoming[order].max(outgoing[order]);
            let bar = fill.repeat(proportional_length(total, maximum_node));
            let label = sanitize_label_text(&node.id);
            BoxNode {
                id: geometry_ids[node.id.as_str()].clone(),
                lines: vec![
                    if label.is_empty() {
                        "(unnamed)".to_owned()
                    } else {
                        label
                    },
                    format!("flow {} {bar}", format_f64(total)),
                ],
                dividers: Vec::new(),
                parent: None,
                span: 1,
                order,
            }
        })
        .collect::<Vec<_>>();
    let direction = BoxDirection::Lr;
    let (from_side, to_side) = direction.edge_sides();
    let edges = model
        .graph
        .links
        .iter()
        .zip(&numeric_links)
        .map(|(link, value)| {
            let length = value
                .map(|value| proportional_length(value, maximum_link))
                .unwrap_or(1);
            BoxEdge {
                from: geometry_ids[link.source.as_str()].clone(),
                to: geometry_ids[link.target.as_str()].clone(),
                label: format!("{} {}", format_json_value(&link.value), fill.repeat(length)),
                marker_start: box_geometry::EdgeMarker::None,
                marker_end: box_geometry::EdgeMarker::Arrow,
                style: box_geometry::EdgeStyle::Solid,
                from_side: Some(from_side),
                to_side: Some(to_side),
            }
        })
        .collect::<Vec<_>>();
    let ranks = directed_ranks(&nodes, &edges);

    match render_layout(&nodes, &edges, &ranks, direction, opts) {
        Err(MermansiError::RenderLimit {
            context: "box geometry columns",
            ..
        }) if direction == BoxDirection::Lr => {
            // Directed ranks are axis-independent; reflowing only changes ports and orientation.
            render_layout(&nodes, &edges, &ranks, BoxDirection::Tb, opts)
        }
        result => result,
    }
}

fn render_layout(
    nodes: &[BoxNode],
    edges: &[BoxEdge],
    ranks: &HashMap<String, usize>,
    direction: BoxDirection,
    opts: &MermansiOptions,
) -> Result<String> {
    let (from_side, to_side) = direction.edge_sides();
    let mut edges = edges.to_vec();
    for edge in &mut edges {
        edge.from_side = Some(from_side);
        edge.to_side = Some(to_side);
    }
    let mut outer_route = 0usize;
    for edge in &mut edges {
        let rank_gap = ranks
            .get(&edge.from)
            .zip(ranks.get(&edge.to))
            .map_or(0, |(from, to)| from.abs_diff(*to));
        if rank_gap > 1 && matches!(direction, BoxDirection::Lr | BoxDirection::Rl) {
            let side = if outer_route.is_multiple_of(2) {
                box_geometry::Side::Top
            } else {
                box_geometry::Side::Bottom
            };
            edge.from_side = Some(side);
            edge.to_side = Some(side);
            outer_route += 1;
        }
    }

    box_geometry::render(
        &BoxDiagram {
            family: "sankey",
            title: Some("Sankey".to_owned()),
            nodes: nodes.to_vec(),
            groups: Vec::new(),
            spacers: Vec::new(),
            edges,
            columns: None,
            layout: BoxLayout::Layered {
                direction,
                ranks: ranks.clone(),
            },
            edge_legend: box_geometry::EdgeLegend::All,
        },
        opts,
    )
}

fn ensure_limit(context: &'static str, requested: usize, limit: usize) -> Result<()> {
    if requested > limit {
        return Err(MermansiError::RenderLimit {
            context,
            requested,
            limit,
        });
    }
    Ok(())
}

fn finite_nonnegative(value: &Value) -> Option<f64> {
    let number = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(number) => number.parse().ok(),
        _ => None,
    }?;
    (number.is_finite() && number >= 0.0).then_some(number)
}

fn proportional_length(value: f64, maximum: f64) -> usize {
    if maximum <= 0.0 {
        return 1;
    }
    ((value / maximum * MAX_FLOW_BAR as f64).round() as usize).clamp(1, MAX_FLOW_BAR)
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::Number(number) => number
            .as_f64()
            .map(format_f64)
            .unwrap_or_else(|| number.to_string()),
        Value::String(text) => sanitize_label_text(text),
        other => sanitize_label_text(&other.to_string()),
    }
}

fn format_f64(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}
