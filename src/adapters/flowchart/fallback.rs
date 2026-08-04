//! Terminal-native fallback for valid Flowcharts rejected by the delegated router.

use std::collections::{HashMap, HashSet};

use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph, FlowchartV2Model};

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxGroup, BoxLayout, BoxNode, BoxNodeShape,
    EdgeLegend, EdgeMarker, EdgeStyle,
};
use crate::ansi::sanitize_label_text;
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;

const MAX_SUBGRAPH_DEPTH: usize = 64;
const MAX_SUBGRAPH_MEMBERSHIPS: usize = 100_000;

pub(super) fn render(model: &FlowchartV2Model, opts: &MermansiOptions) -> Result<String> {
    if model.nodes.is_empty() && model.subgraphs.is_empty() {
        return Ok("(empty flowchart)\n".to_owned());
    }

    let memberships = MembershipIndex::new(&model.nodes, &model.subgraphs)?;
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
            parent: memberships.node_parent(&node.id),
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let node_ranks = box_geometry::directed_ranks(&nodes, &edges);
    let group_ranks = resolve_group_ranks(&model.subgraphs, &node_ranks, &memberships)?;
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
                parent: memberships.group_parent(&group.id),
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

    let mut output = box_geometry::render_with_node_shapes(
        &BoxDiagram {
            family: "flowchart",
            title: None,
            nodes,
            groups,
            spacers: Vec::new(),
            edges: edges.clone(),
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

struct MembershipIndex<'a> {
    group_indices: HashMap<&'a str, usize>,
    node_parents: HashMap<&'a str, &'a str>,
    group_parents: HashMap<&'a str, &'a str>,
}

impl<'a> MembershipIndex<'a> {
    fn new(nodes: &'a [FlowNode], groups: &'a [FlowSubgraph]) -> Result<Self> {
        let membership_count = groups.iter().fold(0usize, |count, group| {
            count.saturating_add(group.nodes.len())
        });
        if membership_count > MAX_SUBGRAPH_MEMBERSHIPS {
            return Err(MermansiError::RenderLimit {
                context: "flowchart subgraph memberships",
                requested: membership_count,
                limit: MAX_SUBGRAPH_MEMBERSHIPS,
            });
        }

        let mut group_indices = HashMap::with_capacity(groups.len());
        for (index, group) in groups.iter().enumerate() {
            if group_indices.insert(group.id.as_str(), index).is_some() {
                return Err(layout_error(format!("duplicate subgraph id: {}", group.id)));
            }
        }

        let mut entity_ids = group_indices.keys().copied().collect::<HashSet<_>>();
        for node in nodes {
            if !entity_ids.insert(node.id.as_str()) {
                return Err(layout_error(format!(
                    "duplicate flowchart entity id: {}",
                    node.id
                )));
            }
        }

        let mut node_parents = HashMap::with_capacity(membership_count);
        let mut group_parents = HashMap::with_capacity(groups.len());
        for group in groups {
            for member in &group.nodes {
                if !entity_ids.contains(member.as_str()) {
                    return Err(layout_error(format!(
                        "subgraph {} references missing member {member}",
                        group.id
                    )));
                }
                node_parents
                    .entry(member.as_str())
                    .or_insert(group.id.as_str());
                if member != &group.id && group_indices.contains_key(member.as_str()) {
                    group_parents
                        .entry(member.as_str())
                        .or_insert(group.id.as_str());
                }
            }
        }
        validate_direct_group_depth(groups, &group_parents)?;

        Ok(Self {
            group_indices,
            node_parents,
            group_parents,
        })
    }

    fn node_parent(&self, node_id: &str) -> Option<String> {
        self.node_parents
            .get(node_id)
            .map(|parent| (*parent).to_owned())
    }

    fn group_parent(&self, group_id: &str) -> Option<String> {
        self.group_parents
            .get(group_id)
            .map(|parent| (*parent).to_owned())
    }
}

fn validate_direct_group_depth(
    groups: &[FlowSubgraph],
    parents: &HashMap<&str, &str>,
) -> Result<()> {
    let mut depths = HashMap::<&str, usize>::with_capacity(groups.len());
    for group in groups {
        if depths.contains_key(group.id.as_str()) {
            continue;
        }
        let mut path = Vec::new();
        let mut current = group.id.as_str();
        let base_depth = loop {
            if let Some(depth) = depths.get(current).copied() {
                break depth;
            }
            if path.contains(&current) {
                return Err(layout_error(format!(
                    "subgraph membership cycle includes {current}"
                )));
            }
            path.push(current);
            if path.len() > MAX_SUBGRAPH_DEPTH {
                return Err(MermansiError::RenderLimit {
                    context: "flowchart subgraph depth",
                    requested: path.len(),
                    limit: MAX_SUBGRAPH_DEPTH,
                });
            }
            let Some(parent) = parents.get(current).copied() else {
                break 0;
            };
            current = parent;
        };

        let requested = base_depth.saturating_add(path.len());
        if requested > MAX_SUBGRAPH_DEPTH {
            return Err(MermansiError::RenderLimit {
                context: "flowchart subgraph depth",
                requested,
                limit: MAX_SUBGRAPH_DEPTH,
            });
        }
        let mut depth = base_depth;
        for id in path.into_iter().rev() {
            depth = depth.saturating_add(1);
            depths.insert(id, depth);
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

fn resolve_group_ranks(
    groups: &[FlowSubgraph],
    node_ranks: &HashMap<String, usize>,
    memberships: &MembershipIndex<'_>,
) -> Result<HashMap<String, usize>> {
    let fallback_rank = node_ranks.values().copied().max().unwrap_or(0);
    let mut ranks = HashMap::with_capacity(groups.len());
    let mut visiting = HashSet::new();
    for group in groups {
        resolve_group_rank(
            &group.id,
            groups,
            &memberships.group_indices,
            node_ranks,
            fallback_rank,
            &mut ranks,
            &mut visiting,
            1,
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
    depth: usize,
) -> Result<usize> {
    if let Some(rank) = ranks.get(group_id) {
        return Ok(*rank);
    }
    if depth > MAX_SUBGRAPH_DEPTH {
        return Err(MermansiError::RenderLimit {
            context: "flowchart subgraph depth",
            requested: depth,
            limit: MAX_SUBGRAPH_DEPTH,
        });
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
                depth.saturating_add(1),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn subgraph(id: impl Into<String>, nodes: Vec<String>) -> FlowSubgraph {
        let id = id.into();
        FlowSubgraph {
            title: id.clone(),
            id,
            dir: None,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes,
        }
    }

    #[test]
    fn subgraph_depth_is_bounded_independent_of_source_order() {
        let mut groups = (0..=MAX_SUBGRAPH_DEPTH)
            .map(|index| {
                let child = if index < MAX_SUBGRAPH_DEPTH {
                    vec![format!("group-{}", index + 1)]
                } else {
                    Vec::new()
                };
                subgraph(format!("group-{index}"), child)
            })
            .collect::<Vec<_>>();
        groups.reverse();

        assert!(matches!(
            MembershipIndex::new(&[], &groups),
            Err(MermansiError::RenderLimit {
                context: "flowchart subgraph depth",
                requested,
                limit: MAX_SUBGRAPH_DEPTH,
            }) if requested == MAX_SUBGRAPH_DEPTH + 1
        ));
    }

    #[test]
    fn subgraph_memberships_are_bounded_before_indexing() {
        let groups = vec![subgraph(
            "group",
            vec![String::new(); MAX_SUBGRAPH_MEMBERSHIPS + 1],
        )];

        assert!(matches!(
            MembershipIndex::new(&[], &groups),
            Err(MermansiError::RenderLimit {
                context: "flowchart subgraph memberships",
                requested,
                limit: MAX_SUBGRAPH_MEMBERSHIPS,
            }) if requested == MAX_SUBGRAPH_MEMBERSHIPS + 1
        ));
    }

    #[test]
    fn missing_subgraph_members_are_not_silently_dropped() {
        let groups = vec![subgraph("group", vec!["missing".to_owned()])];

        assert!(matches!(
            MembershipIndex::new(&[], &groups),
            Err(MermansiError::GeometryLayout {
                family: "flowchart",
                message,
            }) if message.contains("references missing member missing")
        ));
    }
}
