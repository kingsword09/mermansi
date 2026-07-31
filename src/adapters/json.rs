//! JSON terminal geometry.

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxLayout, BoxNode, directed_ranks,
};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use serde_json::Value;

const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_NODES: usize = 10_000;

pub fn render_json(value: &Value, opts: &MermansiOptions) -> Result<String> {
    let direction = BoxDirection::Lr;
    let (from_side, to_side) = direction.edge_sides();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    collect_json(value, None, None, 0, &mut nodes, &mut edges)?;
    for edge in &mut edges {
        edge.from_side = Some(from_side);
        edge.to_side = Some(to_side);
    }
    let ranks = directed_ranks(&nodes, &edges);

    box_geometry::render(
        &BoxDiagram {
            family: "json",
            title: Some("JSON".to_owned()),
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

fn collect_json(
    value: &Value,
    prefix: Option<String>,
    parent_id: Option<&str>,
    depth: usize,
    nodes: &mut Vec<BoxNode>,
    edges: &mut Vec<BoxEdge>,
) -> Result<()> {
    if depth > MAX_JSON_DEPTH {
        return Err(MermansiError::RenderLimit {
            context: "json tree depth",
            requested: depth,
            limit: MAX_JSON_DEPTH,
        });
    }
    let requested = nodes.len().saturating_add(1);
    if requested > MAX_JSON_NODES {
        return Err(MermansiError::RenderLimit {
            context: "json tree nodes",
            requested,
            limit: MAX_JSON_NODES,
        });
    }

    let id = format!("json-{}", nodes.len());
    let summary = json_summary(value);
    let label = prefix.map_or(summary.clone(), |prefix| format!("{prefix}: {summary}"));
    let order = nodes.len();
    nodes.push(BoxNode {
        id: id.clone(),
        lines: vec![label],
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

    match value {
        Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
            for (key, child) in entries {
                collect_json(child, Some(key.clone()), Some(&id), depth + 1, nodes, edges)?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_json(
                    child,
                    Some(format!("[{index}]")),
                    Some(&id),
                    depth + 1,
                    nodes,
                    edges,
                )?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn json_summary(value: &Value) -> String {
    match value {
        Value::Object(map) if map.is_empty() => "{}".to_owned(),
        Value::Object(map) => format!("{{}} ({} fields)", map.len()),
        Value::Array(values) if values.is_empty() => "[]".to_owned(),
        Value::Array(values) => format!("[] ({} items)", values.len()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.to_string(),
    }
}
