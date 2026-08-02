//! Terminal-native fallback for valid Flowcharts rejected by the delegated router.

use std::collections::{HashMap, HashSet};

use merman_core::diagrams::flowchart::{FlowEdge, FlowSubgraph, FlowchartV2Model};

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxGroup, BoxLayout, BoxNode, BoxNodeShape,
    EdgeLegend, EdgeMarker, EdgeStyle,
};
use crate::ansi::sanitize_label_text;
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;

pub(super) fn render(model: &FlowchartV2Model, opts: &MermansiOptions) -> Result<String> {
    if model.nodes.is_empty() && model.subgraphs.is_empty() {
        return Ok("(empty flowchart)\n".to_owned());
    }

    validate_group_ids(&model.subgraphs)?;
    let direction = BoxDirection::from_str(model.direction.as_deref().unwrap_or("TB"));
    let (from_side, to_side) = direction.edge_sides();
    let edges = model
        .edges
        .iter()
        .map(|edge| flow_edge(edge, from_side, to_side))
        .collect::<Vec<_>>();

    let mut nodes = model
        .nodes
        .iter()
        .enumerate()
        .map(|(order, node)| BoxNode {
            id: node.id.clone(),
            lines: vec![node_label(node.label.as_deref(), &node.id)],
            dividers: Vec::new(),
            parent: direct_node_parent(&node.id, &model.subgraphs),
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let node_ranks = box_geometry::directed_ranks(&nodes, &edges);
    let group_ranks = resolve_group_ranks(&model.subgraphs, &node_ranks)?;
    let max_rank = node_ranks
        .values()
        .chain(group_ranks.values())
        .copied()
        .max()
        .unwrap_or(0);
    let stride = model
        .nodes
        .len()
        .saturating_add(model.subgraphs.len())
        .saturating_add(1);
    for (source_order, node) in nodes.iter_mut().enumerate() {
        let rank = node_ranks.get(&node.id).copied().unwrap_or(max_rank);
        node.order = directional_order(rank, source_order, max_rank, stride, direction);
    }

    let groups = model
        .subgraphs
        .iter()
        .enumerate()
        .map(|(source_order, group)| {
            let rank = group_ranks.get(&group.id).copied().unwrap_or(max_rank);
            BoxGroup {
                id: group.id.clone(),
                lines: vec![node_label(Some(&group.title), &group.id)],
                parent: direct_group_parent(&group.id, &model.subgraphs),
                columns: Some(group_columns(group, direction, &node_ranks, &group_ranks)),
                span: 1,
                order: directional_order(rank, source_order, max_rank, stride, direction),
            }
        })
        .collect::<Vec<_>>();

    let mut root_ranks = HashMap::new();
    for node in &nodes {
        if node.parent.is_none() {
            root_ranks.insert(
                node.id.clone(),
                node_ranks.get(&node.id).copied().unwrap_or(max_rank),
            );
        }
    }
    for group in &groups {
        if group.parent.is_none() {
            root_ranks.insert(
                group.id.clone(),
                group_ranks.get(&group.id).copied().unwrap_or(max_rank),
            );
        }
    }

    let node_shapes = model
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                flow_node_shape(node.layout_shape.as_deref()),
            )
        })
        .collect::<HashMap<_, _>>();
    let horizontal = matches!(direction, BoxDirection::Lr | BoxDirection::Rl);
    let root_columns = horizontal.then_some(root_ranks.len().max(1));
    let layout = if horizontal {
        BoxLayout::Packed
    } else {
        BoxLayout::Layered {
            direction,
            ranks: root_ranks,
        }
    };

    let node_order = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let geometry_edges = compact_geometry_edges(&edges, &model.subgraphs, &node_order);
    let mut output = box_geometry::render_with_node_shapes(
        &BoxDiagram {
            family: "flowchart",
            title: None,
            nodes,
            groups,
            spacers: Vec::new(),
            edges: geometry_edges,
            columns: root_columns,
            layout,
            edge_legend: EdgeLegend::None,
        },
        opts,
        &node_shapes,
    )?;
    append_relationships(&mut output, &edges, opts.max_width);
    Ok(output)
}

fn compact_geometry_edges(
    edges: &[BoxEdge],
    groups: &[FlowSubgraph],
    node_order: &HashMap<&str, usize>,
) -> Vec<BoxEdge> {
    let mut selected = HashMap::<(Option<String>, Option<String>), (usize, usize)>::new();
    for (index, edge) in edges.iter().enumerate() {
        let pair = (
            top_level_group(&edge.from, groups),
            top_level_group(&edge.to, groups),
        );
        if pair.0 == pair.1 {
            continue;
        }
        let score = node_order
            .get(edge.from.as_str())
            .copied()
            .unwrap_or(usize::MAX)
            .abs_diff(
                node_order
                    .get(edge.to.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
            );
        let candidate = selected.entry(pair).or_insert((score, index));
        if score < candidate.0 {
            *candidate = (score, index);
        }
    }

    edges
        .iter()
        .enumerate()
        .filter(|(index, edge)| {
            let from_group = top_level_group(&edge.from, groups);
            let to_group = top_level_group(&edge.to, groups);
            from_group == to_group
                || selected
                    .get(&(from_group, to_group))
                    .is_some_and(|(_, selected)| selected == index)
        })
        .map(|(_, edge)| edge.clone())
        .collect()
}

fn top_level_group(entity_id: &str, groups: &[FlowSubgraph]) -> Option<String> {
    let mut current = direct_node_parent(entity_id, groups).or_else(|| {
        groups
            .iter()
            .any(|group| group.id == entity_id)
            .then(|| entity_id.to_owned())
    });
    let mut visited = HashSet::new();
    while let Some(group_id) = current.clone() {
        if !visited.insert(group_id.clone()) {
            break;
        }
        let Some(parent) = direct_group_parent(&group_id, groups) else {
            return Some(group_id);
        };
        current = Some(parent);
    }
    current
}

fn append_relationships(output: &mut String, edges: &[BoxEdge], max_width: usize) {
    if edges.is_empty() {
        return;
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push('\n');
    let width = max_width.saturating_sub(1).max(1);
    for edge in edges {
        for line in box_geometry::wrap_words(&box_geometry::edge_legend_text(edge), width) {
            output.push(' ');
            output.push_str(&line);
            output.push('\n');
        }
    }
}

fn validate_group_ids(groups: &[FlowSubgraph]) -> Result<()> {
    let mut ids = HashSet::with_capacity(groups.len());
    for group in groups {
        if !ids.insert(group.id.as_str()) {
            return Err(layout_error(format!("duplicate subgraph id: {}", group.id)));
        }
    }
    Ok(())
}

fn node_label(label: Option<&str>, id: &str) -> String {
    let label = sanitize_label_text(label.unwrap_or(id));
    if label.trim().is_empty() {
        sanitize_label_text(id)
    } else {
        label
    }
}

fn direct_node_parent(node_id: &str, groups: &[FlowSubgraph]) -> Option<String> {
    groups
        .iter()
        .find(|group| group.nodes.iter().any(|member| member == node_id))
        .map(|group| group.id.clone())
}

fn direct_group_parent(group_id: &str, groups: &[FlowSubgraph]) -> Option<String> {
    groups
        .iter()
        .find(|candidate| {
            candidate.id != group_id && candidate.nodes.iter().any(|member| member == group_id)
        })
        .map(|group| group.id.clone())
}

fn resolve_group_ranks(
    groups: &[FlowSubgraph],
    node_ranks: &HashMap<String, usize>,
) -> Result<HashMap<String, usize>> {
    let indices = groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let fallback_rank = node_ranks.values().copied().max().unwrap_or(0);
    let mut ranks = HashMap::with_capacity(groups.len());
    let mut visiting = HashSet::new();
    for group in groups {
        resolve_group_rank(
            &group.id,
            groups,
            &indices,
            node_ranks,
            fallback_rank,
            &mut ranks,
            &mut visiting,
        )?;
    }
    Ok(ranks)
}

#[allow(clippy::too_many_arguments)]
fn resolve_group_rank(
    group_id: &str,
    groups: &[FlowSubgraph],
    indices: &HashMap<&str, usize>,
    node_ranks: &HashMap<String, usize>,
    fallback_rank: usize,
    ranks: &mut HashMap<String, usize>,
    visiting: &mut HashSet<String>,
) -> Result<usize> {
    if let Some(rank) = ranks.get(group_id) {
        return Ok(*rank);
    }
    if !visiting.insert(group_id.to_owned()) {
        return Err(layout_error(format!(
            "subgraph membership cycle includes {group_id}"
        )));
    }
    let group_index = *indices
        .get(group_id)
        .ok_or_else(|| layout_error(format!("missing subgraph: {group_id}")))?;
    let group = &groups[group_index];
    let mut rank = None::<usize>;
    for member in &group.nodes {
        let member_rank = if let Some(node_rank) = node_ranks.get(member) {
            Some(*node_rank)
        } else if indices.contains_key(member.as_str()) {
            Some(resolve_group_rank(
                member,
                groups,
                indices,
                node_ranks,
                fallback_rank,
                ranks,
                visiting,
            )?)
        } else {
            None
        };
        if let Some(member_rank) = member_rank {
            rank = Some(rank.map_or(member_rank, |current| current.min(member_rank)));
        }
    }
    visiting.remove(group_id);
    let rank = rank.unwrap_or_else(|| fallback_rank.saturating_add(group_index));
    ranks.insert(group_id.to_owned(), rank);
    Ok(rank)
}

fn group_columns(
    group: &FlowSubgraph,
    direction: BoxDirection,
    node_ranks: &HashMap<String, usize>,
    group_ranks: &HashMap<String, usize>,
) -> usize {
    if matches!(direction, BoxDirection::Lr | BoxDirection::Rl) {
        return 1;
    }
    let mut counts = HashMap::<usize, usize>::new();
    for member in &group.nodes {
        let rank = node_ranks
            .get(member)
            .or_else(|| group_ranks.get(member))
            .copied();
        if let Some(rank) = rank {
            *counts.entry(rank).or_default() += 1;
        }
    }
    counts.values().copied().max().unwrap_or(1).max(1)
}

fn directional_order(
    rank: usize,
    source_order: usize,
    max_rank: usize,
    stride: usize,
    direction: BoxDirection,
) -> usize {
    let rank = if matches!(direction, BoxDirection::Bt | BoxDirection::Rl) {
        max_rank.saturating_sub(rank)
    } else {
        rank
    };
    rank.saturating_mul(stride).saturating_add(source_order)
}

fn flow_edge(
    edge: &FlowEdge,
    from_side: box_geometry::Side,
    to_side: box_geometry::Side,
) -> BoxEdge {
    let marker = edge_marker(edge.edge_type.as_deref());
    BoxEdge {
        from: edge.from.clone(),
        to: edge.to.clone(),
        label: sanitize_label_text(edge.label.as_deref().unwrap_or_default()),
        marker_start: if edge
            .edge_type
            .as_deref()
            .is_some_and(|edge_type| edge_type.starts_with("double_"))
        {
            marker
        } else {
            EdgeMarker::None
        },
        marker_end: marker,
        style: if edge.stroke.as_deref() == Some("dotted") {
            EdgeStyle::Dotted
        } else {
            EdgeStyle::Solid
        },
        from_side: Some(from_side),
        to_side: Some(to_side),
    }
}

fn edge_marker(edge_type: Option<&str>) -> EdgeMarker {
    let edge_type = edge_type
        .unwrap_or("arrow_point")
        .strip_prefix("double_")
        .unwrap_or(edge_type.unwrap_or("arrow_point"));
    if edge_type.contains("open") {
        EdgeMarker::None
    } else if edge_type.contains("circle") {
        EdgeMarker::Circle
    } else if edge_type.contains("cross") {
        EdgeMarker::Cross
    } else {
        EdgeMarker::Arrow
    }
}

fn flow_node_shape(shape: Option<&str>) -> BoxNodeShape {
    match shape.unwrap_or_default().to_ascii_lowercase().as_str() {
        "cyl" | "cylinder" | "database" | "db" | "datastore" | "data-store" | "disk"
        | "lin-cyl" | "lined-cylinder" => BoxNodeShape::Cylinder,
        "diam" | "diamond" | "decision" | "question" | "choice" => BoxNodeShape::Decision,
        "roundedrect" | "rounded" | "round" | "stadium" | "pill" | "circle" | "ellipse" => {
            BoxNodeShape::Rounded
        }
        _ => BoxNodeShape::Rectangle,
    }
}

fn layout_error(message: String) -> MermansiError {
    MermansiError::GeometryLayout {
        family: "flowchart",
        message,
    }
}
