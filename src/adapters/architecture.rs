//! Architecture diagram terminal geometry.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxEdge, BoxGroup, BoxLayout, BoxNode, Side,
};
use crate::adapters::detail_separator;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::architecture::{
    ArchitectureDiagramRenderModel, ArchitectureRenderEdge, ArchitectureRenderNode,
    ArchitectureRenderNodeType,
};

pub fn render_architecture(
    model: &ArchitectureDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    box_geometry::ensure_inventory(
        model.groups.len().saturating_add(model.nodes.len()),
        model.edges.len(),
    )?;
    let node_order = model.groups.len();
    let layout = architecture_layout(model, node_order);
    let groups = model
        .groups
        .iter()
        .enumerate()
        .map(|(order, group)| {
            let mut lines = vec![display_identity(
                &group.id,
                group.title.as_deref(),
                opts.charset,
            )];
            if let Some(icon) = normalized_option(group.icon.as_deref()) {
                lines.push(format!("[{icon}]"));
            }
            BoxGroup {
                id: group.id.clone(),
                lines,
                parent: group.in_group.clone(),
                columns: layout.columns.get(&group.id).copied(),
                span: 1,
                order,
            }
        })
        .collect::<Vec<_>>();
    let nodes = model
        .nodes
        .iter()
        .map(|node| {
            architecture_node(
                node,
                layout
                    .orders
                    .get(node.id.as_str())
                    .copied()
                    .unwrap_or(node_order),
                opts.charset,
            )
        })
        .collect();
    let edges = model
        .edges
        .iter()
        .map(|edge| architecture_edge(edge, opts.charset))
        .collect();

    box_geometry::render(
        &BoxDiagram {
            family: "architecture",
            title: model
                .title
                .clone()
                .or_else(|| Some("Architecture".to_owned())),
            nodes,
            groups,
            spacers: Vec::new(),
            edges,
            columns: layout.columns.get("").copied(),
            layout: BoxLayout::Packed,
            edge_legend: box_geometry::EdgeLegend::All,
        },
        opts,
    )
}

fn architecture_node(node: &ArchitectureRenderNode, order: usize, charset: Charset) -> BoxNode {
    let mut lines = vec![display_identity(&node.id, node.title.as_deref(), charset)];
    let kind = match node.node_type {
        ArchitectureRenderNodeType::Service => "service",
        ArchitectureRenderNodeType::Junction => "junction",
    };
    let detail = normalized_option(node.icon_text.as_deref())
        .or_else(|| normalized_option(node.icon.as_deref()))
        .map_or_else(
            || kind.to_owned(),
            |icon| format!("{kind}{}{icon}", detail_separator(charset)),
        );
    lines.push(format!("[{detail}]"));
    BoxNode {
        id: node.id.clone(),
        lines,
        dividers: Vec::new(),
        parent: node.in_group.clone(),
        span: 1,
        order,
    }
}

fn architecture_edge(edge: &ArchitectureRenderEdge, charset: Charset) -> BoxEdge {
    let mut label = normalized_option(edge.title.as_deref()).unwrap_or_default();
    let ports = format!("ports {} -> {}", edge.lhs_dir, edge.rhs_dir);
    if label.is_empty() {
        label = ports;
    } else {
        label.push_str(detail_separator(charset));
        label.push_str(&ports);
    }
    BoxEdge {
        from: edge.lhs_id.clone(),
        to: edge.rhs_id.clone(),
        label,
        marker_start: if edge.lhs_into.unwrap_or_default() {
            box_geometry::EdgeMarker::Arrow
        } else {
            box_geometry::EdgeMarker::None
        },
        marker_end: if edge.rhs_into.unwrap_or_default() {
            box_geometry::EdgeMarker::Arrow
        } else {
            box_geometry::EdgeMarker::None
        },
        style: box_geometry::EdgeStyle::Solid,
        from_side: Side::from_port(edge.lhs_dir),
        to_side: Side::from_port(edge.rhs_dir),
    }
}

fn display_identity(id: &str, title: Option<&str>, charset: Charset) -> String {
    normalized_option(title).map_or_else(
        || id.to_owned(),
        |title| {
            if title == id {
                id.to_owned()
            } else {
                format!("{id}{}{title}", detail_separator(charset))
            }
        },
    )
}

struct ArchitectureLayout {
    orders: HashMap<String, usize>,
    columns: HashMap<String, usize>,
}

fn architecture_layout(
    model: &ArchitectureDiagramRenderModel,
    order_base: usize,
) -> ArchitectureLayout {
    let scores = horizontal_port_scores(model);
    let mut parent_indices = HashMap::<Option<&str>, usize>::new();
    let mut partitions = Vec::<(Option<&str>, Vec<(usize, &ArchitectureRenderNode)>)>::new();
    for (source_order, node) in model.nodes.iter().enumerate() {
        let parent = node.in_group.as_deref();
        let partition_index = parent_indices.get(&parent).copied().unwrap_or_else(|| {
            let index = partitions.len();
            parent_indices.insert(parent, index);
            partitions.push((parent, Vec::new()));
            index
        });
        partitions[partition_index].1.push((source_order, node));
    }

    let mut graphs = partitions
        .iter()
        .map(|(_, members)| {
            (
                vec![Vec::<usize>::new(); members.len()],
                vec![0usize; members.len()],
            )
        })
        .collect::<Vec<_>>();
    let mut node_locations = HashMap::<&str, (usize, usize)>::with_capacity(model.nodes.len());
    for (partition_index, (_, members)) in partitions.iter().enumerate() {
        for (member_index, (_, node)) in members.iter().enumerate() {
            node_locations
                .entry(node.id.as_str())
                .or_insert((partition_index, member_index));
        }
    }
    let mut seen_precedence = HashSet::with_capacity(model.edges.len());
    for edge in &model.edges {
        let Some((before_id, after_id)) = vertical_precedence(edge) else {
            continue;
        };
        let (Some(&(before_partition, before)), Some(&(after_partition, after))) =
            (node_locations.get(before_id), node_locations.get(after_id))
        else {
            continue;
        };
        if before_partition != after_partition
            || before == after
            || !seen_precedence.insert((before_partition, before, after))
        {
            continue;
        }
        graphs[before_partition].0[before].push(after);
        graphs[before_partition].1[after] += 1;
    }

    let mut orders = HashMap::new();
    let mut columns = HashMap::new();
    for ((parent, members), (adjacency, mut indegree)) in partitions.into_iter().zip(graphs) {
        let mut layer = vec![0usize; members.len()];
        let mut ready = indegree
            .iter()
            .enumerate()
            .filter_map(|(index, degree)| {
                (*degree == 0).then_some(Reverse((members[index].0, index)))
            })
            .collect::<BinaryHeap<_>>();
        let mut processed = 0usize;
        while let Some(Reverse((_, current))) = ready.pop() {
            processed += 1;
            for neighbor in adjacency[current].iter().copied() {
                layer[neighbor] = layer[neighbor].max(layer[current] + 1);
                indegree[neighbor] -= 1;
                if indegree[neighbor] == 0 {
                    ready.push(Reverse((members[neighbor].0, neighbor)));
                }
            }
        }
        if processed != members.len() {
            layer.fill(0);
        }

        let mut rows = BTreeMap::<usize, Vec<(usize, &ArchitectureRenderNode)>>::new();
        for (member, row) in members.into_iter().zip(layer) {
            rows.entry(row).or_default().push(member);
        }
        for row in rows.values_mut() {
            row.sort_by_key(|(source_order, node)| {
                (
                    scores.get(node.id.as_str()).copied().unwrap_or_default(),
                    *source_order,
                )
            });
        }
        if rows.len() > 1 {
            let width = rows.values().map(Vec::len).max().unwrap_or(1).max(1);
            columns.insert(parent.unwrap_or_default().to_owned(), width);
        }
        for (order, (_, node)) in rows.into_values().flatten().enumerate() {
            orders.insert(node.id.clone(), order_base + order);
        }
    }
    ArchitectureLayout { orders, columns }
}

fn horizontal_port_scores(model: &ArchitectureDiagramRenderModel) -> HashMap<&str, i64> {
    let mut scores = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), 0i64))
        .collect::<HashMap<_, _>>();
    for edge in &model.edges {
        let direction = match (
            edge.lhs_dir.to_ascii_uppercase(),
            edge.rhs_dir.to_ascii_uppercase(),
        ) {
            ('R', 'L') => -1,
            ('L', 'R') => 1,
            ('R', _) | (_, 'L') => -1,
            ('L', _) | (_, 'R') => 1,
            _ => 0,
        };
        if direction == 0 {
            continue;
        }
        if let Some(score) = scores.get_mut(edge.lhs_id.as_str()) {
            *score += direction;
        }
        if let Some(score) = scores.get_mut(edge.rhs_id.as_str()) {
            *score -= direction;
        }
    }
    scores
}

fn vertical_precedence(edge: &ArchitectureRenderEdge) -> Option<(&str, &str)> {
    match (
        edge.lhs_dir.to_ascii_uppercase(),
        edge.rhs_dir.to_ascii_uppercase(),
    ) {
        ('B', 'T') => Some((&edge.lhs_id, &edge.rhs_id)),
        ('T', 'B') => Some((&edge.rhs_id, &edge.lhs_id)),
        ('B', 'L' | 'R') | ('L' | 'R', 'T') => Some((&edge.lhs_id, &edge.rhs_id)),
        ('T', 'L' | 'R') | ('L' | 'R', 'B') => Some((&edge.rhs_id, &edge.lhs_id)),
        _ => None,
    }
}

fn normalized_option(value: Option<&str>) -> Option<String> {
    value
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|value| !value.is_empty())
}
