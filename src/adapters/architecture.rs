//! Architecture diagram terminal geometry.

use std::collections::{BTreeMap, HashMap};

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
            show_edge_legend: true,
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
        arrow_start: edge.lhs_into.unwrap_or_default(),
        arrow_end: edge.rhs_into.unwrap_or_default(),
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
    let mut parents = Vec::<Option<&str>>::new();
    for node in &model.nodes {
        let parent = node.in_group.as_deref();
        if !parents.contains(&parent) {
            parents.push(parent);
        }
    }

    let mut orders = HashMap::new();
    let mut columns = HashMap::new();
    for parent in parents {
        let members = model
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.in_group.as_deref() == parent)
            .collect::<Vec<_>>();
        let mut layer = vec![0usize; members.len()];
        let mut adjacency = vec![Vec::<usize>::new(); members.len()];
        let mut indegree = vec![0usize; members.len()];
        for edge in &model.edges {
            let Some((before_id, after_id)) = vertical_precedence(edge) else {
                continue;
            };
            let before = members.iter().position(|(_, node)| node.id == before_id);
            let after = members.iter().position(|(_, node)| node.id == after_id);
            let (Some(before), Some(after)) = (before, after) else {
                continue;
            };
            if before != after && !adjacency[before].contains(&after) {
                adjacency[before].push(after);
                indegree[after] += 1;
            }
        }

        let mut ready = indegree
            .iter()
            .enumerate()
            .filter_map(|(index, degree)| (*degree == 0).then_some(index))
            .collect::<Vec<_>>();
        let mut processed = 0usize;
        while !ready.is_empty() {
            ready.sort_unstable_by_key(|index| members[*index].0);
            let current = ready.remove(0);
            processed += 1;
            for neighbor in adjacency[current].iter().copied() {
                layer[neighbor] = layer[neighbor].max(layer[current] + 1);
                indegree[neighbor] -= 1;
                if indegree[neighbor] == 0 {
                    ready.push(neighbor);
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
