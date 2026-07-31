//! Event Modeling terminal geometry.
//!
//! Frames and data entities are closed boxes. Explicit source-frame dependencies are routed as
//! arrows; adjacent frames retain their parsed order when the semantic model has no explicit edge.

use std::collections::HashMap;

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxLayout, BoxNode, directed_ranks,
};
use crate::ansi::sanitize_label_text;
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use merman_core::diagrams::eventmodeling::EventModelingDiagramRenderModel;

const MAX_EVENTMODELING_NODES: usize = 4_096;
const MAX_EVENTMODELING_EDGES: usize = 8_192;

pub fn render_eventmodeling(
    model: &EventModelingDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let node_count = model
        .frames
        .len()
        .checked_add(model.data_entities.len())
        .ok_or(MermansiError::RenderLimit {
            context: "eventmodeling nodes",
            requested: usize::MAX,
            limit: MAX_EVENTMODELING_NODES,
        })?;
    ensure_limit("eventmodeling nodes", node_count, MAX_EVENTMODELING_NODES)?;
    if node_count == 0 {
        return Ok("(empty event modeling diagram)\n".to_owned());
    }

    let mut nodes = Vec::with_capacity(node_count);
    let mut frame_names = HashMap::<&str, usize>::new();
    for (order, frame) in model.frames.iter().enumerate() {
        frame_names.entry(frame.name.as_str()).or_insert(order);
        nodes.push(BoxNode {
            id: frame_id(order),
            lines: frame_lines(frame),
            dividers: Vec::new(),
            parent: None,
            span: 1,
            order,
        });
    }
    let mut data_names = HashMap::<&str, usize>::new();
    for (index, entity) in model.data_entities.iter().enumerate() {
        data_names.entry(entity.name.as_str()).or_insert(index);
        let mut lines = vec![format!(
            "[data] {}",
            nonempty(&sanitize_label_text(&entity.name))
        )];
        append_multiline(&mut lines, "", &entity.data_block_value);
        nodes.push(BoxNode {
            id: data_id(index),
            lines,
            dividers: Vec::new(),
            parent: None,
            span: 1,
            order: model.frames.len() + index,
        });
    }

    let direction = BoxDirection::Lr;
    let (from_side, to_side) = direction.edge_sides();
    let mut edges = Vec::new();
    for (target, frame) in model.frames.iter().enumerate() {
        for source in &frame.source_frames {
            let Some(&source) = frame_names.get(source.as_str()) else {
                return Err(MermansiError::GeometryLayout {
                    family: "eventmodeling",
                    message: format!(
                        "source frame is not present in the frame inventory: {}",
                        sanitize_label_text(source)
                    ),
                });
            };
            push_edge(
                &mut edges,
                edge(
                    frame_id(source),
                    frame_id(target),
                    "source",
                    from_side,
                    to_side,
                ),
            )?;
        }
    }

    for target in 1..model.frames.len() {
        if model.frames[target].source_frames.is_empty() {
            push_edge(
                &mut edges,
                edge(
                    frame_id(target - 1),
                    frame_id(target),
                    "next",
                    from_side,
                    to_side,
                ),
            )?;
        }
    }

    let mut connected_data = vec![false; model.data_entities.len()];
    for (frame_index, frame) in model.frames.iter().enumerate() {
        if let Some(data_index) = frame
            .data_reference
            .as_deref()
            .and_then(|name| data_names.get(name).copied())
        {
            push_edge(
                &mut edges,
                edge(
                    frame_id(frame_index),
                    data_id(data_index),
                    "data",
                    from_side,
                    to_side,
                ),
            )?;
            connected_data[data_index] = true;
        }
    }
    for (data_index, entity) in model.data_entities.iter().enumerate() {
        if connected_data[data_index] {
            continue;
        }
        let source = model
            .frames
            .iter()
            .enumerate()
            .rev()
            .find(|(_, frame)| frame.model_entity_type == entity.name)
            .map(|(frame_index, _)| frame_id(frame_index))
            .or_else(|| data_index.checked_sub(1).map(data_id))
            .or_else(|| model.frames.len().checked_sub(1).map(frame_id));
        if let Some(source) = source {
            push_edge(
                &mut edges,
                edge(source, data_id(data_index), "data", from_side, to_side),
            )?;
        }
    }
    ensure_limit("eventmodeling edges", edges.len(), MAX_EVENTMODELING_EDGES)?;
    let ranks = directed_ranks(&nodes, &edges);

    box_geometry::render(
        &BoxDiagram {
            family: "eventmodeling",
            title: model
                .title
                .clone()
                .or_else(|| Some("Event Modeling".to_owned())),
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

fn frame_lines(
    frame: &merman_core::diagrams::eventmodeling::EventModelingFrameRenderModel,
) -> Vec<String> {
    let mut lines = vec![format!(
        "[{}] {}",
        nonempty(&sanitize_label_text(&frame.frame_kind)),
        nonempty(&sanitize_label_text(&frame.name))
    )];
    lines.push(format!(
        "type: {}",
        nonempty(&sanitize_label_text(&frame.model_entity_type))
    ));
    lines.push(format!(
        "entity: {}",
        nonempty(&sanitize_label_text(&frame.entity_identifier))
    ));
    if let Some(value) = &frame.data_inline_value {
        append_multiline(&mut lines, "value: ", value);
    }
    if let Some(reference) = &frame.data_reference {
        lines.push(format!("ref: {}", sanitize_label_text(reference)));
    }
    lines
}

fn append_multiline(lines: &mut Vec<String>, prefix: &str, value: &str) {
    let mut first = true;
    for raw in value.lines() {
        let text = sanitize_label_text(raw).trim().to_owned();
        if text.is_empty() {
            continue;
        }
        if first {
            lines.push(format!("{prefix}{text}"));
            first = false;
        } else {
            lines.push(text);
        }
    }
}

fn edge(
    from: String,
    to: String,
    label: &str,
    from_side: box_geometry::Side,
    to_side: box_geometry::Side,
) -> BoxEdge {
    BoxEdge {
        from,
        to,
        label: label.to_owned(),
        marker_start: box_geometry::EdgeMarker::None,
        marker_end: box_geometry::EdgeMarker::Arrow,
        style: box_geometry::EdgeStyle::Solid,
        from_side: Some(from_side),
        to_side: Some(to_side),
    }
}

fn push_edge(edges: &mut Vec<BoxEdge>, value: BoxEdge) -> Result<()> {
    ensure_limit(
        "eventmodeling edges",
        edges.len().saturating_add(1),
        MAX_EVENTMODELING_EDGES,
    )?;
    edges.push(value);
    Ok(())
}

fn frame_id(index: usize) -> String {
    format!("event-frame-{index}")
}

fn data_id(index: usize) -> String {
    format!("event-data-{index}")
}

fn nonempty(value: &str) -> &str {
    if value.trim().is_empty() {
        "(unnamed)"
    } else {
        value
    }
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
