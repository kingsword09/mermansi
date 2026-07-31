//! Deterministic edge-lane geometry for Flowchart self-loops and parallel edges.

use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line, draw_vertical_line};
use crate::error::{MermansiError, Result};
use crate::{Charset, MermansiOptions, str_display_width};
use merman_core::diagrams::flowchart::{FlowEdge, FlowchartV2Model};
use std::collections::{HashMap, HashSet};

const NODE_MIN_WIDTH: usize = 7;
const NODE_HEIGHT: usize = 5;
const NODE_GAP: usize = 3;
const EDGE_LANE_GAP: usize = 3;

pub(super) fn requires_lane_geometry(model: &FlowchartV2Model) -> bool {
    let mut endpoint_pairs = HashSet::with_capacity(model.edges.len());
    model.edges.iter().any(|edge| {
        if edge.from == edge.to {
            return true;
        }
        let pair = if edge.from <= edge.to {
            (edge.from.as_str(), edge.to.as_str())
        } else {
            (edge.to.as_str(), edge.from.as_str())
        };
        !endpoint_pairs.insert(pair)
    })
}

pub(super) fn render_lane_geometry(
    model: &FlowchartV2Model,
    opts: &MermansiOptions,
) -> Result<Option<String>> {
    let Some(topology) = Topology::new(model) else {
        return Ok(None);
    };
    let labels = Labels::new(model);
    match model.direction.as_deref().unwrap_or("TB") {
        "TB" | "TD" => render_vertical(model, &topology, &labels, opts, false).map(Some),
        "BT" => render_vertical(model, &topology, &labels, opts, true).map(Some),
        "LR" => render_horizontal(model, &topology, &labels, opts, false).map(Some),
        "RL" => render_horizontal(model, &topology, &labels, opts, true).map(Some),
        _ => Ok(None),
    }
}

#[derive(Clone, Copy)]
enum EdgePorts {
    External {
        from_node: usize,
        to_node: usize,
        from_slot: usize,
        to_slot: usize,
    },
    SelfLoop {
        node: usize,
        ordinal: usize,
    },
}

struct Topology {
    ports: Vec<EdgePorts>,
    external_counts: Vec<usize>,
    self_counts: Vec<usize>,
    external_edges: usize,
    self_edges: usize,
}

impl Topology {
    fn new(model: &FlowchartV2Model) -> Option<Self> {
        let mut node_indices = HashMap::with_capacity(model.nodes.len());
        for (index, node) in model.nodes.iter().enumerate() {
            if node_indices.insert(node.id.as_str(), index).is_some() {
                return None;
            }
        }

        let mut external_counts = vec![0usize; model.nodes.len()];
        let mut self_counts = vec![0usize; model.nodes.len()];
        let mut ports = Vec::with_capacity(model.edges.len());
        let mut external_edges = 0usize;
        let mut self_edges = 0usize;

        for edge in &model.edges {
            let from_node = *node_indices.get(edge.from.as_str())?;
            let to_node = *node_indices.get(edge.to.as_str())?;
            if from_node == to_node {
                let ordinal = self_counts[from_node];
                self_counts[from_node] = ordinal.checked_add(1)?;
                self_edges = self_edges.checked_add(1)?;
                ports.push(EdgePorts::SelfLoop {
                    node: from_node,
                    ordinal,
                });
                continue;
            }

            let from_slot = external_counts[from_node];
            external_counts[from_node] = from_slot.checked_add(1)?;
            let to_slot = external_counts[to_node];
            external_counts[to_node] = to_slot.checked_add(1)?;
            external_edges = external_edges.checked_add(1)?;
            ports.push(EdgePorts::External {
                from_node,
                to_node,
                from_slot,
                to_slot,
            });
        }

        Some(Self {
            ports,
            external_counts,
            self_counts,
            external_edges,
            self_edges,
        })
    }
}

struct Labels {
    nodes: Vec<String>,
    edges: Vec<String>,
}

impl Labels {
    fn new(model: &FlowchartV2Model) -> Self {
        let nodes = model
            .nodes
            .iter()
            .map(|node| {
                let label = sanitize_label_text(node.label.as_deref().unwrap_or(&node.id));
                if label.trim().is_empty() {
                    sanitize_label_text(&node.id)
                } else {
                    label
                }
            })
            .collect();
        let edges = model
            .edges
            .iter()
            .map(|edge| sanitize_label_text(edge.label.as_deref().unwrap_or_default()))
            .collect();
        Self { nodes, edges }
    }
}

#[derive(Clone, Copy, Default)]
struct NodeRect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl NodeRect {
    fn right(self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(self) -> usize {
        self.y + self.height - 1
    }
}

#[derive(Clone, Copy)]
struct MarkerPlacement {
    x: usize,
    y: usize,
    value: char,
}

#[derive(Clone, Copy)]
struct LabelPlacement {
    edge: usize,
    x: usize,
    y: usize,
}

fn render_vertical(
    model: &FlowchartV2Model,
    topology: &Topology,
    labels: &Labels,
    opts: &MermansiOptions,
    reverse: bool,
) -> Result<String> {
    let node_width = labels
        .nodes
        .iter()
        .map(|label| str_display_width(label).saturating_add(4))
        .max()
        .unwrap_or(NODE_MIN_WIDTH)
        .max(NODE_MIN_WIDTH);

    let mut lane_x = vec![0usize; model.edges.len()];
    let mut label_x = vec![0usize; model.edges.len()];
    let mut left_cursor = 1usize;
    for (edge_index, ports) in topology.ports.iter().enumerate() {
        if matches!(ports, EdgePorts::SelfLoop { .. }) {
            continue;
        }
        label_x[edge_index] = left_cursor;
        lane_x[edge_index] = left_cursor
            .saturating_add(str_display_width(&labels.edges[edge_index]))
            .saturating_add(1);
        left_cursor = lane_x[edge_index].saturating_add(EDGE_LANE_GAP);
    }
    let node_x = left_cursor.saturating_add(1);

    let mut rects = vec![NodeRect::default(); model.nodes.len()];
    let mut y = 1usize;
    let order = ordered_indices(model.nodes.len(), reverse);
    for node_index in order {
        let self_ports = topology.self_counts[node_index].saturating_mul(2);
        let inner_ports = topology.external_counts[node_index]
            .max(self_ports)
            .max(NODE_HEIGHT - 2);
        let height = inner_ports.saturating_add(2);
        rects[node_index] = NodeRect {
            x: node_x,
            y,
            width: node_width,
            height,
        };
        y = y.saturating_add(height).saturating_add(NODE_GAP);
    }
    let height = y.saturating_sub(NODE_GAP).saturating_add(1).max(1);

    let node_right = node_x.saturating_add(node_width).saturating_sub(1);
    let mut self_lane_x = vec![0usize; model.edges.len()];
    let mut self_ordinal = 0usize;
    let mut width = node_right.saturating_add(2);
    for (edge_index, ports) in topology.ports.iter().enumerate() {
        if !matches!(ports, EdgePorts::SelfLoop { .. }) {
            continue;
        }
        let edge_lane = node_right
            .saturating_add(3)
            .saturating_add(self_ordinal.saturating_mul(EDGE_LANE_GAP));
        self_lane_x[edge_index] = edge_lane;
        label_x[edge_index] = edge_lane.saturating_add(2);
        width = width.max(
            label_x[edge_index]
                .saturating_add(str_display_width(&labels.edges[edge_index]))
                .saturating_add(1),
        );
        self_ordinal = self_ordinal.saturating_add(1);
    }

    let mut canvas = bounded_canvas(width, height, opts)?;
    draw_nodes(&mut canvas, &rects, opts.charset)?;
    let mut markers = Vec::with_capacity(model.edges.len().saturating_mul(2));
    let mut edge_labels = Vec::with_capacity(model.edges.len());

    for (edge_index, (edge, ports)) in model.edges.iter().zip(&topology.ports).enumerate() {
        match *ports {
            EdgePorts::External {
                from_node,
                to_node,
                from_slot,
                to_slot,
            } => {
                let from = rects[from_node];
                let to = rects[to_node];
                let from_y = from.y + 1 + from_slot;
                let to_y = to.y + 1 + to_slot;
                let edge_lane = lane_x[edge_index];
                draw_horizontal_line(&mut canvas, from_y, edge_lane, from.x, opts.charset)?;
                draw_vertical_line(
                    &mut canvas,
                    edge_lane,
                    from_y.min(to_y),
                    from_y.max(to_y),
                    opts.charset,
                )?;
                draw_horizontal_line(&mut canvas, to_y, edge_lane, to.x, opts.charset)?;
                push_markers(
                    &mut markers,
                    edge,
                    MarkerPlacement {
                        x: from.x - 1,
                        y: from_y,
                        value: horizontal_target_marker(opts.charset),
                    },
                    MarkerPlacement {
                        x: to.x - 1,
                        y: to_y,
                        value: horizontal_target_marker(opts.charset),
                    },
                );
                edge_labels.push(LabelPlacement {
                    edge: edge_index,
                    x: label_x[edge_index],
                    y: (from_y + to_y) / 2,
                });
            }
            EdgePorts::SelfLoop { node, ordinal } => {
                let rect = rects[node];
                let from_y = rect.y + 1 + ordinal * 2;
                let to_y = from_y + 1;
                let edge_lane = self_lane_x[edge_index];
                draw_horizontal_line(&mut canvas, from_y, rect.right(), edge_lane, opts.charset)?;
                draw_vertical_line(&mut canvas, edge_lane, from_y, to_y, opts.charset)?;
                draw_horizontal_line(&mut canvas, to_y, rect.right(), edge_lane, opts.charset)?;
                push_markers(
                    &mut markers,
                    edge,
                    MarkerPlacement {
                        x: rect.right() + 1,
                        y: from_y,
                        value: horizontal_source_marker(opts.charset),
                    },
                    MarkerPlacement {
                        x: rect.right() + 1,
                        y: to_y,
                        value: horizontal_source_marker(opts.charset),
                    },
                );
                edge_labels.push(LabelPlacement {
                    edge: edge_index,
                    x: label_x[edge_index],
                    y: from_y,
                });
            }
        }
    }

    finish_canvas(canvas, &rects, labels, markers, edge_labels)
}

fn render_horizontal(
    model: &FlowchartV2Model,
    topology: &Topology,
    labels: &Labels,
    opts: &MermansiOptions,
    reverse: bool,
) -> Result<String> {
    let mut node_widths = Vec::with_capacity(model.nodes.len());
    for node_index in 0..model.nodes.len() {
        let self_ports = topology.self_counts[node_index].saturating_mul(2);
        let self_label_width = topology
            .ports
            .iter()
            .enumerate()
            .filter_map(|(edge_index, ports)| match ports {
                EdgePorts::SelfLoop { node, .. } if *node == node_index => {
                    Some(str_display_width(&labels.edges[edge_index]))
                }
                _ => None,
            })
            .max()
            .unwrap_or_default();
        node_widths.push(
            str_display_width(&labels.nodes[node_index])
                .saturating_add(4)
                .max(topology.external_counts[node_index].saturating_add(2))
                .max(self_ports.saturating_add(3))
                .max(self_label_width.saturating_add(2))
                .max(NODE_MIN_WIDTH),
        );
    }

    let max_edge_label = labels
        .edges
        .iter()
        .map(|label| str_display_width(label))
        .max()
        .unwrap_or_default();
    let node_gap = max_edge_label.saturating_add(4).max(NODE_GAP);
    let node_y = topology
        .external_edges
        .saturating_mul(EDGE_LANE_GAP)
        .saturating_add(2);

    let mut rects = vec![NodeRect::default(); model.nodes.len()];
    let mut x = 1usize;
    let mut width = 1usize;
    for node_index in ordered_indices(model.nodes.len(), reverse) {
        let rect = NodeRect {
            x,
            y: node_y,
            width: node_widths[node_index],
            height: NODE_HEIGHT,
        };
        width = width.max(rect.right().saturating_add(2));
        rects[node_index] = rect;
        x = rect.right().saturating_add(1).saturating_add(node_gap);
    }

    let height = node_y
        .saturating_add(NODE_HEIGHT)
        .saturating_add(2)
        .saturating_add(topology.self_edges.saturating_mul(EDGE_LANE_GAP));
    let mut canvas = bounded_canvas(width, height, opts)?;
    draw_nodes(&mut canvas, &rects, opts.charset)?;
    let mut markers = Vec::with_capacity(model.edges.len().saturating_mul(2));
    let mut edge_labels = Vec::with_capacity(model.edges.len());
    let mut external_ordinal = 0usize;
    let mut self_ordinal = 0usize;

    for (edge_index, (edge, ports)) in model.edges.iter().zip(&topology.ports).enumerate() {
        match *ports {
            EdgePorts::External {
                from_node,
                to_node,
                from_slot,
                to_slot,
            } => {
                let from = rects[from_node];
                let to = rects[to_node];
                let from_x = from.x + 1 + from_slot;
                let to_x = to.x + 1 + to_slot;
                let lane_y = 1 + external_ordinal * EDGE_LANE_GAP;
                draw_vertical_line(&mut canvas, from_x, lane_y, from.y, opts.charset)?;
                draw_horizontal_line(
                    &mut canvas,
                    lane_y,
                    from_x.min(to_x),
                    from_x.max(to_x),
                    opts.charset,
                )?;
                draw_vertical_line(&mut canvas, to_x, lane_y, to.y, opts.charset)?;
                push_markers(
                    &mut markers,
                    edge,
                    MarkerPlacement {
                        x: from_x,
                        y: from.y - 1,
                        value: vertical_target_marker(opts.charset),
                    },
                    MarkerPlacement {
                        x: to_x,
                        y: to.y - 1,
                        value: vertical_target_marker(opts.charset),
                    },
                );
                let label_width = str_display_width(&labels.edges[edge_index]);
                let route_start = from_x.min(to_x);
                let route_width = from_x.max(to_x).saturating_sub(route_start);
                edge_labels.push(LabelPlacement {
                    edge: edge_index,
                    x: route_start.saturating_add(route_width.saturating_sub(label_width) / 2),
                    y: lane_y + 1,
                });
                external_ordinal += 1;
            }
            EdgePorts::SelfLoop { node, ordinal } => {
                let rect = rects[node];
                let from_x = rect.x + 1 + ordinal;
                let to_x = rect.right() - 1 - ordinal;
                let lane_y = rect
                    .bottom()
                    .saturating_add(2)
                    .saturating_add(self_ordinal.saturating_mul(EDGE_LANE_GAP));
                draw_vertical_line(&mut canvas, from_x, rect.bottom(), lane_y, opts.charset)?;
                draw_horizontal_line(&mut canvas, lane_y, from_x, to_x, opts.charset)?;
                draw_vertical_line(&mut canvas, to_x, rect.bottom(), lane_y, opts.charset)?;
                push_markers(
                    &mut markers,
                    edge,
                    MarkerPlacement {
                        x: from_x,
                        y: rect.bottom() + 1,
                        value: vertical_source_marker(opts.charset),
                    },
                    MarkerPlacement {
                        x: to_x,
                        y: rect.bottom() + 1,
                        value: vertical_source_marker(opts.charset),
                    },
                );
                let label_width = str_display_width(&labels.edges[edge_index]);
                edge_labels.push(LabelPlacement {
                    edge: edge_index,
                    x: rect.x + rect.width.saturating_sub(label_width) / 2,
                    y: lane_y + 1,
                });
                self_ordinal += 1;
            }
        }
    }

    finish_canvas(canvas, &rects, labels, markers, edge_labels)
}

fn ordered_indices(count: usize, reverse: bool) -> Vec<usize> {
    if reverse {
        (0..count).rev().collect()
    } else {
        (0..count).collect()
    }
}

fn bounded_canvas(width: usize, height: usize, opts: &MermansiOptions) -> Result<Canvas> {
    if width > opts.max_width {
        return Err(MermansiError::RenderLimit {
            context: "flowchart preview columns",
            requested: width,
            limit: opts.max_width,
        });
    }
    if height > opts.max_height {
        return Err(MermansiError::RenderLimit {
            context: "flowchart preview rows",
            requested: height,
            limit: opts.max_height,
        });
    }
    Canvas::new(width, height)
}

fn draw_nodes(canvas: &mut Canvas, rects: &[NodeRect], charset: Charset) -> Result<()> {
    for rect in rects {
        draw_box(canvas, rect.x, rect.y, rect.width, rect.height, charset)?;
    }
    Ok(())
}

fn finish_canvas(
    mut canvas: Canvas,
    rects: &[NodeRect],
    labels: &Labels,
    markers: Vec<MarkerPlacement>,
    edge_labels: Vec<LabelPlacement>,
) -> Result<String> {
    for marker in markers {
        canvas.set_char(marker.x, marker.y, marker.value)?;
    }
    for placement in edge_labels {
        let label = &labels.edges[placement.edge];
        if !label.is_empty() {
            canvas.set_text(placement.x, placement.y, label)?;
        }
    }
    for (node_index, rect) in rects.iter().enumerate() {
        let label = &labels.nodes[node_index];
        let x = rect.x + rect.width.saturating_sub(str_display_width(label)) / 2;
        let y = rect.y + rect.height / 2;
        canvas.set_text(x, y, label)?;
    }
    let rendered = canvas.render();
    let rendered = rendered.trim_matches('\n');
    if rendered.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{rendered}\n"))
    }
}

fn push_markers(
    markers: &mut Vec<MarkerPlacement>,
    edge: &FlowEdge,
    start: MarkerPlacement,
    end: MarkerPlacement,
) {
    let edge_type = edge.edge_type.as_deref().unwrap_or("arrow_point");
    if edge_type.starts_with("double_")
        && let Some(value) = marker_value(edge_type, start.value)
    {
        markers.push(MarkerPlacement { value, ..start });
    }
    if let Some(value) = marker_value(edge_type, end.value) {
        markers.push(MarkerPlacement { value, ..end });
    }
}

fn marker_value(edge_type: &str, directional: char) -> Option<char> {
    let edge_type = edge_type.strip_prefix("double_").unwrap_or(edge_type);
    if edge_type.contains("open") {
        None
    } else if edge_type.contains("cross") {
        Some('x')
    } else if edge_type.contains("circle") {
        Some('o')
    } else {
        Some(directional)
    }
}

fn horizontal_target_marker(charset: Charset) -> char {
    match charset {
        Charset::Unicode => '▶',
        Charset::Ascii => '>',
    }
}

fn horizontal_source_marker(charset: Charset) -> char {
    match charset {
        Charset::Unicode => '◀',
        Charset::Ascii => '<',
    }
}

fn vertical_target_marker(charset: Charset) -> char {
    match charset {
        Charset::Unicode => '▼',
        Charset::Ascii => 'v',
    }
}

fn vertical_source_marker(charset: Charset) -> char {
    match charset {
        Charset::Unicode => '▲',
        Charset::Ascii => '^',
    }
}
