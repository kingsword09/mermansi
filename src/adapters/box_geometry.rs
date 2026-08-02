//! Shared bounded geometry for node-and-relationship diagram families.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, VecDeque};

use unicode_segmentation::UnicodeSegmentation;

use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line, draw_vertical_line};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;

const MAX_INNER_WIDTH: usize = 32;
const GROUP_PADDING_X: usize = 3;
const GROUP_ROUTE_GAP_Y: usize = 1;
const ITEM_GAP_X: usize = 4;
const ITEM_GAP_Y: usize = 1;
const LAYER_GAP_X: usize = 8;
const LAYER_GAP_Y: usize = 2;
const ROUTE_MARGIN: usize = 2;
const MAX_OUTER_ROUTE_LANES: usize = 8;
const MAX_DEPTH: usize = 64;
const MIN_CANVAS_WIDTH: usize = 12;
const OCCUPIED_ROUTE_PENALTY: usize = 12;

#[derive(Clone, Debug)]
pub(crate) struct BoxNode {
    pub(crate) id: String,
    pub(crate) lines: Vec<String>,
    /// Line-row offsets that are painted as full-width compartment dividers.
    pub(crate) dividers: Vec<usize>,
    pub(crate) parent: Option<String>,
    pub(crate) span: usize,
    pub(crate) order: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum BoxNodeShape {
    #[default]
    Rectangle,
    Rounded,
    Cylinder,
    Decision,
}

#[derive(Clone, Debug)]
pub(crate) struct BoxGroup {
    pub(crate) id: String,
    pub(crate) lines: Vec<String>,
    pub(crate) parent: Option<String>,
    pub(crate) columns: Option<usize>,
    pub(crate) span: usize,
    pub(crate) order: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct BoxSpacer {
    pub(crate) parent: Option<String>,
    pub(crate) span: usize,
    pub(crate) order: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct BoxEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: String,
    pub(crate) marker_start: EdgeMarker,
    pub(crate) marker_end: EdgeMarker,
    pub(crate) style: EdgeStyle,
    pub(crate) from_side: Option<Side>,
    pub(crate) to_side: Option<Side>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum EdgeMarker {
    #[default]
    None,
    Arrow,
    OpenTriangle,
    OpenDiamond,
    FilledDiamond,
    Circle,
    Cross,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum EdgeStyle {
    #[default]
    Solid,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum EdgeLegend {
    #[default]
    None,
    Labeled,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub(crate) enum BoxDirection {
    #[default]
    Tb,
    Bt,
    Lr,
    Rl,
}

impl BoxDirection {
    pub(crate) fn from_str(direction: &str) -> Self {
        match direction.to_ascii_uppercase().as_str() {
            "BT" => Self::Bt,
            "LR" => Self::Lr,
            "RL" => Self::Rl,
            _ => Self::Tb,
        }
    }

    pub(crate) const fn edge_sides(self) -> (Side, Side) {
        match self {
            Self::Tb => (Side::Bottom, Side::Top),
            Self::Bt => (Side::Top, Side::Bottom),
            Self::Lr => (Side::Right, Side::Left),
            Self::Rl => (Side::Left, Side::Right),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) enum BoxLayout {
    #[default]
    Packed,
    Layered {
        direction: BoxDirection,
        ranks: HashMap<String, usize>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct BoxDiagram {
    pub(crate) family: &'static str,
    pub(crate) title: Option<String>,
    pub(crate) nodes: Vec<BoxNode>,
    pub(crate) groups: Vec<BoxGroup>,
    pub(crate) spacers: Vec<BoxSpacer>,
    pub(crate) edges: Vec<BoxEdge>,
    pub(crate) columns: Option<usize>,
    pub(crate) layout: BoxLayout,
    pub(crate) edge_legend: EdgeLegend,
}

pub(crate) fn directed_ranks(nodes: &[BoxNode], edges: &[BoxEdge]) -> HashMap<String, usize> {
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut adjacency = vec![Vec::<usize>::new(); nodes.len()];
    let mut indegree = vec![0usize; nodes.len()];

    for edge in edges {
        let (Some(&source), Some(&target)) = (
            indices.get(edge.from.as_str()),
            indices.get(edge.to.as_str()),
        ) else {
            continue;
        };
        if source != target && !adjacency[source].contains(&target) {
            adjacency[source].push(target);
            indegree[target] += 1;
        }
    }

    let mut ranks = vec![0usize; nodes.len()];
    let mut queue = VecDeque::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            queue.push_back(index);
        }
    }
    while let Some(source) = queue.pop_front() {
        for &target in &adjacency[source] {
            ranks[target] = ranks[target].max(ranks[source].saturating_add(1));
            indegree[target] -= 1;
            if indegree[target] == 0 {
                queue.push_back(target);
            }
        }
    }

    // A directed cycle cannot satisfy every edge direction geometrically. Keep its
    // nodes visible on deterministic consecutive layers so each return edge can route.
    let mut cycle_rank = ranks.iter().copied().max().unwrap_or(0);
    for (index, degree) in indegree.iter().enumerate() {
        if *degree > 0 {
            ranks[index] = ranks[index].max(cycle_rank);
            cycle_rank = cycle_rank.saturating_add(1);
        }
    }

    nodes
        .iter()
        .zip(ranks)
        .map(|(node, rank)| (node.id.clone(), rank))
        .collect()
}

pub(crate) fn directed_chain_edges(
    prefix: &str,
    count: usize,
    direction: BoxDirection,
) -> Vec<BoxEdge> {
    let (from_side, to_side) = direction.edge_sides();
    (1..count)
        .map(|index| BoxEdge {
            from: format!("{prefix}-{}", index - 1),
            to: format!("{prefix}-{index}"),
            label: String::new(),
            marker_start: EdgeMarker::None,
            marker_end: EdgeMarker::Arrow,
            style: EdgeStyle::Solid,
            from_side: Some(from_side),
            to_side: Some(to_side),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    pub(crate) const fn from_port(port: char) -> Option<Self> {
        match port {
            'L' | 'l' => Some(Self::Left),
            'R' | 'r' => Some(Self::Right),
            'T' | 't' => Some(Self::Top),
            'B' | 'b' => Some(Self::Bottom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ItemKind {
    Node,
    Group,
    Spacer,
}

#[derive(Clone, Debug)]
struct Item {
    id: String,
    kind: ItemKind,
    lines: Vec<String>,
    dividers: Vec<usize>,
    width: usize,
    height: usize,
    span: usize,
    children: Vec<PlacedItem>,
}

#[derive(Clone, Debug)]
struct PlacedItem {
    x: usize,
    y: usize,
    item: Item,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Point {
    x: usize,
    y: usize,
}

impl Point {
    const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl Rect {
    const fn right(self) -> usize {
        self.x + self.width - 1
    }

    const fn bottom(self) -> usize {
        self.y + self.height - 1
    }

    const fn center(self) -> Point {
        Point::new(self.x + self.width / 2, self.y + self.height / 2)
    }
}

#[derive(Clone, Copy, Debug)]
struct Endpoint {
    border: Point,
    outside: Point,
}

pub(crate) fn render(diagram: &BoxDiagram, opts: &MermansiOptions) -> Result<String> {
    render_with_node_shapes(diagram, opts, &HashMap::new())
}

pub(crate) fn render_with_node_shapes(
    diagram: &BoxDiagram,
    opts: &MermansiOptions,
    node_shapes: &HashMap<String, BoxNodeShape>,
) -> Result<String> {
    if opts.max_width < MIN_CANVAS_WIDTH {
        return Err(MermansiError::RenderLimit {
            context: "box geometry columns",
            requested: MIN_CANVAS_WIDTH,
            limit: opts.max_width,
        });
    }

    let content_limit = opts.max_width - ROUTE_MARGIN * 2;
    let mut ancestry = Vec::new();
    let mut root = Vec::new();
    for (index, group) in diagram.groups.iter().enumerate() {
        if group.parent.is_none() {
            root.push((
                group.order,
                build_group(diagram, index, content_limit, &mut ancestry)?,
            ));
        }
    }
    for node in &diagram.nodes {
        if node.parent.is_none() {
            root.push((node.order, build_node(node, content_limit)));
        }
    }
    for spacer in &diagram.spacers {
        if spacer.parent.is_none() {
            root.push((spacer.order, build_spacer(spacer, content_limit)));
        }
    }
    root.sort_by_key(|(order, _)| *order);
    let root = root.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
    if root.is_empty() {
        return Ok(diagram.title.clone().unwrap_or_default());
    }

    let (mut placed, diagram_width, diagram_height) = match &diagram.layout {
        BoxLayout::Packed => pack_items(root, content_limit, diagram.columns),
        BoxLayout::Layered { direction, ranks } => {
            pack_layered_items(root, ranks, *direction, diagram.family)?
        }
    };
    if diagram_width > content_limit {
        return Err(MermansiError::RenderLimit {
            context: "box geometry columns",
            requested: diagram_width.saturating_add(ROUTE_MARGIN * 2),
            limit: opts.max_width,
        });
    }
    let title_lines = diagram
        .title
        .as_deref()
        .map(normalized)
        .filter(|title| !title.is_empty())
        .map_or_else(Vec::new, |title| wrap_display(&title, content_limit));
    let title_width = title_lines
        .iter()
        .map(|line| str_display_width(line))
        .max()
        .unwrap_or(0);
    let content_width = diagram_width.max(title_width).max(1);
    let legend_width = if diagram.edge_legend != EdgeLegend::None {
        diagram
            .edges
            .iter()
            .filter(|edge| include_legend_edge(diagram.edge_legend, edge))
            .map(|edge| str_display_width(&edge_legend_text(edge)))
            .max()
            .unwrap_or(0)
            .min(opts.max_width.saturating_sub(2))
    } else {
        0
    };
    let width = content_width
        .saturating_add(ROUTE_MARGIN * 2)
        .max(legend_width.saturating_add(2))
        .min(opts.max_width);
    let drawing_width = width.saturating_sub(ROUTE_MARGIN * 2);
    let diagram_x = ROUTE_MARGIN + drawing_width.saturating_sub(diagram_width) / 2;
    let title_y = 1;
    let geometry_edges = coalesced_geometry_edges(&diagram.edges);
    let (mut top_route_lanes, mut bottom_route_lanes) = if geometry_edges.is_empty() {
        (0, 0)
    } else if matches!(&diagram.layout, BoxLayout::Layered { .. }) {
        (1, 1)
    } else {
        let route_lanes = geometry_edges.len().min(MAX_OUTER_ROUTE_LANES);
        (route_lanes.div_ceil(2), route_lanes / 2)
    };
    if geometry_edges
        .iter()
        .any(|edge| edge.from_side == Some(Side::Top) || edge.to_side == Some(Side::Top))
    {
        top_route_lanes = top_route_lanes.max(2);
    }
    if geometry_edges
        .iter()
        .any(|edge| edge.from_side == Some(Side::Bottom) || edge.to_side == Some(Side::Bottom))
    {
        bottom_route_lanes = bottom_route_lanes.max(2);
    }
    let diagram_y =
        title_y + title_lines.len() + usize::from(!title_lines.is_empty()) + top_route_lanes;
    for item in &mut placed {
        item.x += diagram_x;
        item.y += diagram_y;
    }

    let legend = if diagram.edge_legend != EdgeLegend::None {
        edge_legend(&diagram.edges, diagram.edge_legend, width.saturating_sub(2))
    } else {
        Vec::new()
    };
    let legend_y =
        diagram_y + diagram_height + bottom_route_lanes + usize::from(!legend.is_empty());
    let height = legend_y + legend.len() + 1;
    if height > opts.max_height {
        return Err(MermansiError::RenderLimit {
            context: "box geometry rows",
            requested: height,
            limit: opts.max_height,
        });
    }
    let mut canvas = Canvas::new(width, height)?;
    for (offset, line) in title_lines.iter().enumerate() {
        write_centered(
            &mut canvas,
            ROUTE_MARGIN,
            title_y + offset,
            drawing_width,
            line,
        )?;
    }

    let mut geometry = HashMap::new();
    let mut obstacles = Vec::new();
    if !title_lines.is_empty() {
        obstacles.push(Rect {
            x: ROUTE_MARGIN,
            y: title_y,
            width: drawing_width,
            height: title_lines.len(),
        });
    }
    for item in &placed {
        paint_item(
            &mut canvas,
            item,
            &mut geometry,
            &mut obstacles,
            node_shapes,
            opts.charset,
        )?;
    }
    if geometry.len() != diagram.nodes.len() + diagram.groups.len() {
        return Err(layout_error(
            diagram.family,
            "hierarchy contains unreachable entities",
        ));
    }

    let mut routed = vec![false; width.saturating_mul(legend_y)];
    for edge in &geometry_edges {
        draw_edge(
            &mut canvas,
            edge,
            &geometry,
            &obstacles,
            &mut routed,
            legend_y,
            opts.charset,
            diagram.family,
        )?;
    }
    for (offset, line) in legend.iter().enumerate() {
        canvas.set_text(1, legend_y + offset, line)?;
    }

    let rendered = canvas.render();
    let trimmed = rendered.trim_matches('\n');
    Ok(if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    })
}

fn build_group(
    diagram: &BoxDiagram,
    index: usize,
    max_width: usize,
    ancestry: &mut Vec<String>,
) -> Result<Item> {
    if ancestry.len() >= MAX_DEPTH {
        return Err(layout_error(
            diagram.family,
            format!("nesting exceeds {MAX_DEPTH} levels"),
        ));
    }
    let group = &diagram.groups[index];
    if ancestry.contains(&group.id) {
        return Err(layout_error(
            diagram.family,
            format!("group cycle includes {}", group.id),
        ));
    }
    ancestry.push(group.id.clone());
    let inner_limit = max_width.saturating_sub(GROUP_PADDING_X * 2).max(5);
    let mut children = Vec::new();
    for (child_index, child) in diagram.groups.iter().enumerate() {
        if child.parent.as_deref() == Some(group.id.as_str()) {
            children.push((
                child.order,
                build_group(diagram, child_index, inner_limit, ancestry)?,
            ));
        }
    }
    for child in &diagram.nodes {
        if child.parent.as_deref() == Some(group.id.as_str()) {
            children.push((child.order, build_node(child, inner_limit)));
        }
    }
    for spacer in &diagram.spacers {
        if spacer.parent.as_deref() == Some(group.id.as_str()) {
            children.push((spacer.order, build_spacer(spacer, inner_limit)));
        }
    }
    ancestry.pop();
    children.sort_by_key(|(order, _)| *order);
    let children = children.into_iter().map(|(_, item)| item).collect();

    let lines = wrapped_lines(&group.lines, max_width.saturating_sub(2).max(1));
    let (mut placed, child_width, child_height) = pack_items(children, inner_limit, group.columns);
    let label_width = lines
        .iter()
        .map(|line| str_display_width(line))
        .max()
        .unwrap_or(0);
    let width = label_width
        .saturating_add(2)
        .max(child_width.saturating_add(GROUP_PADDING_X * 2))
        .max(group.span.max(1).saturating_mul(10).saturating_add(2))
        .max(8)
        .min(max_width.max(8));
    let content_y = 1 + lines.len() + GROUP_ROUTE_GAP_Y * usize::from(!placed.is_empty());
    let content_x = GROUP_PADDING_X + width.saturating_sub(GROUP_PADDING_X * 2 + child_width) / 2;
    for child in &mut placed {
        child.x += content_x;
        child.y += content_y;
    }
    let height = if placed.is_empty() {
        lines.len() + 3
    } else {
        content_y + child_height + GROUP_ROUTE_GAP_Y + 1
    };
    Ok(Item {
        id: group.id.clone(),
        kind: ItemKind::Group,
        lines,
        dividers: Vec::new(),
        width,
        height,
        span: group.span.max(1),
        children: placed,
    })
}

fn build_node(node: &BoxNode, max_width: usize) -> Item {
    let inner_limit = max_width.saturating_sub(2).clamp(1, MAX_INNER_WIDTH);
    let mut lines = Vec::new();
    let mut dividers = Vec::new();
    for (index, line) in node.lines.iter().enumerate() {
        if node.dividers.contains(&index) {
            dividers.push(lines.len());
        }
        lines.extend(wrap_words(&normalized(line), inner_limit));
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    let natural_width = lines
        .iter()
        .map(|line| str_display_width(line))
        .max()
        .unwrap_or(1)
        .saturating_add(2)
        .max(7);
    let span_width = node.span.max(1).saturating_mul(10).saturating_add(2);
    let width = natural_width.max(span_width).min(max_width.max(7));
    let height = 2 + lines.len().max(1);
    Item {
        id: node.id.clone(),
        kind: ItemKind::Node,
        lines,
        dividers,
        width,
        height,
        span: node.span.max(1),
        children: Vec::new(),
    }
}

fn build_spacer(spacer: &BoxSpacer, max_width: usize) -> Item {
    Item {
        id: String::new(),
        kind: ItemKind::Spacer,
        lines: Vec::new(),
        dividers: Vec::new(),
        width: spacer
            .span
            .max(1)
            .saturating_mul(10)
            .saturating_add(2)
            .min(max_width.max(1)),
        height: 1,
        span: spacer.span.max(1),
        children: Vec::new(),
    }
}

fn wrapped_lines(lines: &[String], width: usize) -> Vec<String> {
    let mut wrapped = lines
        .iter()
        .flat_map(|line| wrap_words(&normalized(line), width))
        .collect::<Vec<_>>();
    if wrapped.is_empty() {
        wrapped.push(String::new());
    }
    wrapped
}

fn pack_items(
    items: Vec<Item>,
    max_width: usize,
    columns: Option<usize>,
) -> (Vec<PlacedItem>, usize, usize) {
    let mut rows = Vec::<Vec<Item>>::new();
    let mut row = Vec::new();
    let mut row_width = 0usize;
    let mut row_columns = 0usize;
    let columns = columns.filter(|columns| *columns > 0);
    for item in items {
        let proposed = if row.is_empty() {
            item.width
        } else {
            row_width + ITEM_GAP_X + item.width
        };
        let item_columns = columns.map_or(1, |columns| item.span.min(columns).max(1));
        let column_break =
            columns.is_some_and(|columns| row_columns.saturating_add(item_columns) > columns);
        if !row.is_empty() && (column_break || proposed > max_width) {
            rows.push(std::mem::take(&mut row));
            row_width = 0;
            row_columns = 0;
        }
        row_width = if row.is_empty() {
            item.width
        } else {
            row_width + ITEM_GAP_X + item.width
        };
        row_columns = row_columns.saturating_add(item_columns);
        row.push(item);
    }
    if !row.is_empty() {
        rows.push(row);
    }

    let mut placed = Vec::new();
    let mut y = 0usize;
    let mut total_width = 0usize;
    for row in rows {
        let row_height = row.iter().map(|item| item.height).max().unwrap_or(0);
        let width = row
            .iter()
            .map(|item| item.width)
            .sum::<usize>()
            .saturating_add(ITEM_GAP_X * row.len().saturating_sub(1));
        let mut x = 0usize;
        for item in row {
            let item_width = item.width;
            let item_height = item.height;
            placed.push(PlacedItem {
                x,
                y: y + row_height.saturating_sub(item_height) / 2,
                item,
            });
            x += item_width + ITEM_GAP_X;
        }
        total_width = total_width.max(width);
        y += row_height + ITEM_GAP_Y;
    }
    (placed, total_width, y.saturating_sub(ITEM_GAP_Y))
}

fn pack_layered_items(
    items: Vec<Item>,
    ranks: &HashMap<String, usize>,
    direction: BoxDirection,
    family: &'static str,
) -> Result<(Vec<PlacedItem>, usize, usize)> {
    let mut by_rank = BTreeMap::<usize, Vec<Item>>::new();
    for item in items {
        let rank = ranks.get(&item.id).copied().ok_or_else(|| {
            layout_error(family, format!("layer rank is missing for {}", item.id))
        })?;
        by_rank.entry(rank).or_default().push(item);
    }
    let mut layers = by_rank.into_values().collect::<Vec<_>>();
    if matches!(direction, BoxDirection::Bt | BoxDirection::Rl) {
        layers.reverse();
    }
    Ok(
        if matches!(direction, BoxDirection::Tb | BoxDirection::Bt) {
            pack_layer_rows(layers)
        } else {
            pack_layer_columns(layers)
        },
    )
}

fn pack_layer_rows(layers: Vec<Vec<Item>>) -> (Vec<PlacedItem>, usize, usize) {
    let total_width = layers
        .iter()
        .map(|layer| {
            layer.iter().map(|item| item.width).sum::<usize>()
                + ITEM_GAP_X * layer.len().saturating_sub(1)
        })
        .max()
        .unwrap_or(0);
    let mut placed = Vec::new();
    let mut y = 0usize;
    for layer in layers {
        let layer_width = layer.iter().map(|item| item.width).sum::<usize>()
            + ITEM_GAP_X * layer.len().saturating_sub(1);
        let layer_height = layer.iter().map(|item| item.height).max().unwrap_or(0);
        let mut x = if layer.len() == 1 {
            (total_width / 2).saturating_sub(layer[0].width / 2)
        } else {
            total_width.saturating_sub(layer_width) / 2
        };
        for item in layer {
            let item_width = item.width;
            let item_height = item.height;
            placed.push(PlacedItem {
                x,
                y: y + layer_height.saturating_sub(item_height) / 2,
                item,
            });
            x += item_width + ITEM_GAP_X;
        }
        y += layer_height + LAYER_GAP_Y;
    }
    (placed, total_width, y.saturating_sub(LAYER_GAP_Y))
}

fn pack_layer_columns(layers: Vec<Vec<Item>>) -> (Vec<PlacedItem>, usize, usize) {
    let total_height = layers
        .iter()
        .map(|layer| {
            layer.iter().map(|item| item.height).sum::<usize>()
                + ITEM_GAP_Y * layer.len().saturating_sub(1)
        })
        .max()
        .unwrap_or(0);
    let mut placed = Vec::new();
    let mut x = 0usize;
    for layer in layers {
        let layer_width = layer.iter().map(|item| item.width).max().unwrap_or(0);
        let layer_height = layer.iter().map(|item| item.height).sum::<usize>()
            + ITEM_GAP_Y * layer.len().saturating_sub(1);
        let mut y = if layer.len() == 1 {
            (total_height / 2).saturating_sub(layer[0].height / 2)
        } else {
            total_height.saturating_sub(layer_height) / 2
        };
        for item in layer {
            let item_width = item.width;
            let item_height = item.height;
            placed.push(PlacedItem {
                x: x + layer_width.saturating_sub(item_width) / 2,
                y,
                item,
            });
            y += item_height + ITEM_GAP_Y;
        }
        x += layer_width + LAYER_GAP_X;
    }
    (placed, x.saturating_sub(LAYER_GAP_X), total_height)
}

fn paint_item(
    canvas: &mut Canvas,
    placed: &PlacedItem,
    geometry: &mut HashMap<String, Rect>,
    obstacles: &mut Vec<Rect>,
    node_shapes: &HashMap<String, BoxNodeShape>,
    charset: Charset,
) -> Result<()> {
    if matches!(placed.item.kind, ItemKind::Spacer) {
        return Ok(());
    }
    let rect = Rect {
        x: placed.x,
        y: placed.y,
        width: placed.item.width,
        height: placed.item.height,
    };
    draw_box(canvas, rect.x, rect.y, rect.width, rect.height, charset)?;
    if matches!(placed.item.kind, ItemKind::Node) {
        paint_node_shape(
            canvas,
            rect,
            node_shapes
                .get(&placed.item.id)
                .copied()
                .unwrap_or_default(),
            charset,
        )?;
    }
    geometry.insert(placed.item.id.clone(), rect);
    match placed.item.kind {
        ItemKind::Node => {
            obstacles.push(rect);
            for divider in &placed.item.dividers {
                draw_horizontal_line(canvas, rect.y + 1 + divider, rect.x, rect.right(), charset)?;
            }
            for (offset, line) in placed.item.lines.iter().enumerate() {
                write_centered(
                    canvas,
                    rect.x + 1,
                    rect.y + 1 + offset,
                    rect.width - 2,
                    line,
                )?;
            }
        }
        ItemKind::Group => {
            obstacles.extend([
                Rect {
                    x: rect.x,
                    y: rect.y,
                    width: 1,
                    height: 1,
                },
                Rect {
                    x: rect.right(),
                    y: rect.y,
                    width: 1,
                    height: 1,
                },
                Rect {
                    x: rect.x,
                    y: rect.bottom(),
                    width: 1,
                    height: 1,
                },
                Rect {
                    x: rect.right(),
                    y: rect.bottom(),
                    width: 1,
                    height: 1,
                },
            ]);
            for (offset, line) in placed.item.lines.iter().enumerate() {
                let line_width = str_display_width(line);
                let label_x = rect.x + 1;
                canvas.set_text(label_x, rect.y + 1 + offset, line)?;
                if line_width > 0 {
                    obstacles.push(Rect {
                        x: label_x,
                        y: rect.y + 1 + offset,
                        width: line_width,
                        height: 1,
                    });
                }
            }
            for child in &placed.item.children {
                let nested = PlacedItem {
                    x: rect.x + child.x,
                    y: rect.y + child.y,
                    item: child.item.clone(),
                };
                paint_item(canvas, &nested, geometry, obstacles, node_shapes, charset)?;
            }
        }
        ItemKind::Spacer => {}
    }
    Ok(())
}

fn paint_node_shape(
    canvas: &mut Canvas,
    rect: Rect,
    shape: BoxNodeShape,
    charset: Charset,
) -> Result<()> {
    if shape == BoxNodeShape::Rectangle {
        return Ok(());
    }

    let (top_left, top_right, bottom_left, bottom_right) = match (charset, shape) {
        (Charset::Unicode, BoxNodeShape::Rounded | BoxNodeShape::Cylinder) => ('╭', '╮', '╰', '╯'),
        (Charset::Unicode, BoxNodeShape::Decision) => ('╱', '╲', '╲', '╱'),
        (Charset::Ascii, BoxNodeShape::Rounded | BoxNodeShape::Cylinder) => ('.', '.', '\'', '\''),
        (Charset::Ascii, BoxNodeShape::Decision) => ('/', '\\', '\\', '/'),
        (_, BoxNodeShape::Rectangle) => return Ok(()),
    };
    canvas.set_char(rect.x, rect.y, top_left)?;
    canvas.set_char(rect.right(), rect.y, top_right)?;
    canvas.set_char(rect.x, rect.bottom(), bottom_left)?;
    canvas.set_char(rect.right(), rect.bottom(), bottom_right)?;

    let middle = rect.y + rect.height / 2;
    match shape {
        BoxNodeShape::Cylinder => {
            canvas.set_char(rect.x, middle, '(')?;
            canvas.set_char(rect.right(), middle, ')')?;
        }
        BoxNodeShape::Decision => {
            canvas.set_char(rect.x, middle, '<')?;
            canvas.set_char(rect.right(), middle, '>')?;
        }
        BoxNodeShape::Rectangle | BoxNodeShape::Rounded => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_edge(
    canvas: &mut Canvas,
    edge: &BoxEdge,
    geometry: &HashMap<String, Rect>,
    obstacles: &[Rect],
    routed: &mut [bool],
    route_height: usize,
    charset: Charset,
    family: &'static str,
) -> Result<()> {
    let source = geometry
        .get(&edge.from)
        .copied()
        .ok_or_else(|| layout_error(family, format!("edge source is missing: {}", edge.from)))?;
    let target = geometry
        .get(&edge.to)
        .copied()
        .ok_or_else(|| layout_error(family, format!("edge target is missing: {}", edge.to)))?;
    let width = canvas.width();
    let prefer_shared_route =
        edge.marker_start == EdgeMarker::None && edge.marker_end == EdgeMarker::None;
    let path = if prefer_shared_route {
        route_edge(
            canvas,
            source,
            target,
            edge,
            obstacles,
            routed,
            route_height,
            false,
        )
    } else {
        route_edge(
            canvas,
            source,
            target,
            edge,
            obstacles,
            routed,
            route_height,
            true,
        )
        .or_else(|| {
            route_edge(
                canvas,
                source,
                target,
                edge,
                obstacles,
                routed,
                route_height,
                false,
            )
        })
    }
    .ok_or_else(|| layout_error(family, format!("no route for {} -> {}", edge.from, edge.to)))?;
    draw_path(canvas, &path, charset, edge.style)?;
    for point in path_cells(&path)
        .into_iter()
        .filter(|point| point.x < width && point.y < route_height)
    {
        routed[grid_index(point, width)] = true;
    }

    let (source_marker, source_arrow) =
        endpoint_marker(path[0], path[1], edge.marker_start, charset);
    let last = path.len() - 1;
    let (target_marker, target_arrow) =
        endpoint_marker(path[last], path[last - 1], edge.marker_end, charset);
    if edge.marker_start != EdgeMarker::None {
        canvas.set_char(source_marker.x, source_marker.y, source_arrow)?;
    }
    if edge.marker_end != EdgeMarker::None {
        canvas.set_char(target_marker.x, target_marker.y, target_arrow)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn route_edge(
    canvas: &Canvas,
    source: Rect,
    target: Rect,
    edge: &BoxEdge,
    obstacles: &[Rect],
    routed: &[bool],
    height: usize,
    avoid_routed: bool,
) -> Option<Vec<Point>> {
    let width = canvas.width();
    if width == 0 || height == 0 {
        return None;
    }
    let mut blocked = blocked_cells(obstacles, width, height);
    block_route_frame(&mut blocked, width, height);
    if avoid_routed {
        for (blocked, routed) in blocked.iter_mut().zip(routed) {
            *blocked |= *routed;
        }
    }
    for rect in [source, target] {
        for side in Side::ALL {
            if let Some(endpoint) = endpoint(rect, side, width, height) {
                blocked[grid_index(endpoint.border, width)] = true;
            }
        }
    }

    let side_pairs = ordered_side_pairs(source, target);
    let source_sides = edge
        .from_side
        .map_or_else(|| Side::ALL.to_vec(), |side| vec![side]);
    let target_sides = edge
        .to_side
        .map_or_else(|| Side::ALL.to_vec(), |side| vec![side]);
    let mut best = None::<((usize, usize), Vec<Point>)>;
    for source_side in source_sides {
        let source_endpoint = endpoint(source, source_side, width, height)?;
        let source_index = grid_index(source_endpoint.outside, width);
        if blocked[source_index] {
            continue;
        }
        let predecessors = weighted_predecessors(
            source_endpoint.outside,
            width,
            height,
            &blocked,
            routed,
            canvas,
        );
        for target_side in target_sides.iter().copied() {
            if source == target && source_side == target_side {
                continue;
            }
            let Some(target_endpoint) = endpoint(target, target_side, width, height) else {
                continue;
            };
            let target_index = grid_index(target_endpoint.outside, width);
            if blocked[target_index] {
                continue;
            }
            let Some(cells) = reconstruct_path(source_index, target_index, width, &predecessors)
            else {
                continue;
            };
            let mut points = Vec::with_capacity(cells.len() + 2);
            points.push(source_endpoint.border);
            points.extend(cells);
            points.push(target_endpoint.border);
            let path = compress_path(points);
            let preference = side_pairs
                .iter()
                .position(|pair| *pair == (source_side, target_side))
                .unwrap_or(side_pairs.len());
            let distance = path
                .windows(2)
                .map(|pair| pair[0].x.abs_diff(pair[1].x) + pair[0].y.abs_diff(pair[1].y))
                .sum::<usize>();
            let overlap = path_cells(&path)
                .into_iter()
                .filter(|point| routed[grid_index(*point, width)])
                .count();
            let occupied = path_cells(&path)
                .iter()
                .filter(|point| {
                    let index = grid_index(**point, width);
                    !routed[index]
                        && canvas
                            .get_cell(point.x, point.y)
                            .is_some_and(|cell| !cell.is_empty())
                })
                .count();
            let score = distance
                .saturating_add(path.len().saturating_sub(2) * 2)
                .saturating_add(occupied * 4)
                .saturating_add(preference * 2);
            let key = if avoid_routed {
                (overlap, score)
            } else {
                (0, score.saturating_add(overlap))
            };
            if best.as_ref().is_none_or(|(best_key, _)| key < *best_key) {
                best = Some((key, path));
            }
        }
    }
    best.map(|(_, path)| path)
}

fn blocked_cells(obstacles: &[Rect], width: usize, height: usize) -> Vec<bool> {
    let mut blocked = vec![false; width * height];
    for obstacle in obstacles {
        let bottom = obstacle.bottom().min(height - 1);
        let right = obstacle.right().min(width - 1);
        for y in obstacle.y.min(height)..=bottom {
            for x in obstacle.x.min(width)..=right {
                blocked[y * width + x] = true;
            }
        }
    }
    blocked
}

fn block_route_frame(blocked: &mut [bool], width: usize, height: usize) {
    if width == 0 || height == 0 {
        return;
    }
    for x in 0..width {
        blocked[x] = true;
        blocked[(height - 1) * width + x] = true;
    }
    for y in 0..height {
        blocked[y * width] = true;
        blocked[y * width + width - 1] = true;
    }
}

fn weighted_predecessors(
    start: Point,
    width: usize,
    height: usize,
    blocked: &[bool],
    routed: &[bool],
    canvas: &Canvas,
) -> Vec<usize> {
    let unvisited = usize::MAX;
    let start_index = grid_index(start, width);
    let mut predecessors = vec![unvisited; width * height];
    let mut distances = vec![usize::MAX; width * height];
    predecessors[start_index] = start_index;
    distances[start_index] = 0;
    let mut queue = BinaryHeap::from([Reverse((0usize, start_index))]);
    while let Some(Reverse((distance, point_index))) = queue.pop() {
        if distance != distances[point_index] {
            continue;
        }
        let point = Point::new(point_index % width, point_index / width);
        let neighbors = [
            (point.x + 1 < width).then(|| Point::new(point.x + 1, point.y)),
            (point.y + 1 < height).then(|| Point::new(point.x, point.y + 1)),
            point.x.checked_sub(1).map(|x| Point::new(x, point.y)),
            point.y.checked_sub(1).map(|y| Point::new(point.x, y)),
        ];
        for neighbor in neighbors.into_iter().flatten() {
            let neighbor_index = grid_index(neighbor, width);
            if blocked[neighbor_index] {
                continue;
            }
            let occupied = !routed[neighbor_index]
                && canvas
                    .get_cell(neighbor.x, neighbor.y)
                    .is_some_and(|cell| !cell.is_empty());
            let candidate = distance
                .saturating_add(1)
                .saturating_add(usize::from(occupied) * OCCUPIED_ROUTE_PENALTY);
            if candidate < distances[neighbor_index]
                || (candidate == distances[neighbor_index]
                    && point_index < predecessors[neighbor_index])
            {
                distances[neighbor_index] = candidate;
                predecessors[neighbor_index] = point_index;
                queue.push(Reverse((candidate, neighbor_index)));
            }
        }
    }
    predecessors
}

fn reconstruct_path(
    start: usize,
    target: usize,
    width: usize,
    predecessors: &[usize],
) -> Option<Vec<Point>> {
    let mut current = target;
    let mut reversed = Vec::new();
    while current != start {
        reversed.push(Point::new(current % width, current / width));
        current = *predecessors.get(current)?;
        if current == usize::MAX {
            return None;
        }
    }
    reversed.push(Point::new(start % width, start / width));
    reversed.reverse();
    Some(reversed)
}

fn ordered_side_pairs(source: Rect, target: Rect) -> Vec<(Side, Side)> {
    let source_center = source.center();
    let target_center = target.center();
    let horizontal = if source_center.x <= target_center.x {
        (Side::Right, Side::Left)
    } else {
        (Side::Left, Side::Right)
    };
    let vertical = if source_center.y <= target_center.y {
        (Side::Bottom, Side::Top)
    } else {
        (Side::Top, Side::Bottom)
    };
    let mut pairs =
        if source_center.x.abs_diff(target_center.x) >= source_center.y.abs_diff(target_center.y) {
            vec![horizontal, vertical]
        } else {
            vec![vertical, horizontal]
        };
    for source_side in Side::ALL {
        for target_side in Side::ALL {
            let pair = (source_side, target_side);
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
    }
    pairs
}

fn endpoint(rect: Rect, side: Side, width: usize, height: usize) -> Option<Endpoint> {
    let border = match side {
        Side::Left => Point::new(rect.x, rect.center().y),
        Side::Right => Point::new(rect.right(), rect.center().y),
        Side::Top => Point::new(rect.center().x, rect.y),
        Side::Bottom => Point::new(rect.center().x, rect.bottom()),
    };
    let outside = match side {
        Side::Left => Point::new(border.x.checked_sub(1)?, border.y),
        Side::Right => Point::new(border.x.checked_add(1)?, border.y),
        Side::Top => Point::new(border.x, border.y.checked_sub(1)?),
        Side::Bottom => Point::new(border.x, border.y.checked_add(1)?),
    };
    (border.x < width && border.y < height && outside.x < width && outside.y < height)
        .then_some(Endpoint { border, outside })
}

const fn grid_index(point: Point, width: usize) -> usize {
    point.y * width + point.x
}

fn compress_path(points: Vec<Point>) -> Vec<Point> {
    let mut normalized = Vec::new();
    for point in points {
        if normalized.last() != Some(&point) {
            normalized.push(point);
        }
    }
    let mut index = 1usize;
    while index + 1 < normalized.len() {
        let previous = normalized[index - 1];
        let current = normalized[index];
        let next = normalized[index + 1];
        if (previous.x == current.x && current.x == next.x)
            || (previous.y == current.y && current.y == next.y)
        {
            normalized.remove(index);
        } else {
            index += 1;
        }
    }
    normalized
}

fn endpoint_marker(
    endpoint: Point,
    neighbor: Point,
    marker_kind: EdgeMarker,
    charset: Charset,
) -> (Point, char) {
    let unicode = charset == Charset::Unicode;
    let directional = |left, right, up, down| {
        if endpoint.x < neighbor.x {
            left
        } else if endpoint.x > neighbor.x {
            right
        } else if endpoint.y < neighbor.y {
            up
        } else {
            down
        }
    };
    let glyph = match marker_kind {
        EdgeMarker::None => ' ',
        EdgeMarker::Arrow if unicode => directional('◀', '▶', '▲', '▼'),
        EdgeMarker::Arrow => directional('<', '>', '^', 'v'),
        EdgeMarker::OpenTriangle if unicode => directional('◁', '▷', '△', '▽'),
        EdgeMarker::OpenTriangle => directional('<', '>', '^', 'v'),
        EdgeMarker::OpenDiamond if unicode => '◇',
        EdgeMarker::FilledDiamond if unicode => '◆',
        EdgeMarker::Circle if unicode => '○',
        EdgeMarker::Cross => 'x',
        EdgeMarker::OpenDiamond | EdgeMarker::Circle => 'o',
        EdgeMarker::FilledDiamond => '*',
    };
    let marker = if endpoint.x < neighbor.x {
        Point::new(endpoint.x.saturating_add(1), endpoint.y)
    } else if endpoint.x > neighbor.x {
        Point::new(endpoint.x.saturating_sub(1), endpoint.y)
    } else if endpoint.y < neighbor.y {
        Point::new(endpoint.x, endpoint.y.saturating_add(1))
    } else {
        Point::new(endpoint.x, endpoint.y.saturating_sub(1))
    };
    (marker, glyph)
}

fn draw_path(
    canvas: &mut Canvas,
    points: &[Point],
    charset: Charset,
    style: EdgeStyle,
) -> Result<()> {
    if style == EdgeStyle::Dotted {
        let cells = path_cells(points);
        let last = cells.len().saturating_sub(1);
        let glyph = if charset == Charset::Unicode {
            '·'
        } else {
            '.'
        };
        for (index, point) in cells.into_iter().enumerate() {
            if index != 0 && index != last && index.is_multiple_of(2) {
                canvas.set_char(point.x, point.y, glyph)?;
            }
        }
        return Ok(());
    }
    for pair in points.windows(2) {
        if pair[0].x == pair[1].x {
            let (start, end) = ordered(pair[0].y, pair[1].y);
            draw_vertical_line(canvas, pair[0].x, start, end, charset)?;
        } else if pair[0].y == pair[1].y {
            let (start, end) = ordered(pair[0].x, pair[1].x);
            draw_horizontal_line(canvas, pair[0].y, start, end, charset)?;
        }
    }
    Ok(())
}

fn path_cells(points: &[Point]) -> Vec<Point> {
    let mut cells = Vec::new();
    for pair in points.windows(2) {
        if pair[0].x == pair[1].x {
            let (start, end) = ordered(pair[0].y, pair[1].y);
            for y in start..=end {
                cells.push(Point::new(pair[0].x, y));
            }
        } else if pair[0].y == pair[1].y {
            let (start, end) = ordered(pair[0].x, pair[1].x);
            for x in start..=end {
                cells.push(Point::new(x, pair[0].y));
            }
        }
    }
    cells.sort_unstable_by_key(|point| (point.y, point.x));
    cells.dedup();
    cells
}

fn edge_legend(edges: &[BoxEdge], policy: EdgeLegend, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for edge in edges
        .iter()
        .filter(|edge| include_legend_edge(policy, edge))
    {
        lines.extend(wrap_words(&edge_legend_text(edge), width));
    }
    lines
}

pub(crate) fn edge_legend_text(edge: &BoxEdge) -> String {
    let line = if edge.style == EdgeStyle::Dotted {
        ".."
    } else {
        "--"
    };
    let arrow = format!(
        "{}{line}{}",
        marker_legend(edge.marker_start, true),
        marker_legend(edge.marker_end, false)
    );
    let label = normalized(&edge.label);
    if label.is_empty() {
        format!("{} {arrow} {}", edge.from, edge.to)
    } else {
        format!("{} {arrow} {}  {label}", edge.from, edge.to)
    }
}

fn include_legend_edge(policy: EdgeLegend, edge: &BoxEdge) -> bool {
    match policy {
        EdgeLegend::None => false,
        EdgeLegend::Labeled => !normalized(&edge.label).is_empty(),
        EdgeLegend::All => true,
    }
}

fn marker_legend(marker: EdgeMarker, start: bool) -> &'static str {
    match (marker, start) {
        (EdgeMarker::None, _) => "",
        (EdgeMarker::Arrow, true) => "<",
        (EdgeMarker::Arrow, false) => ">",
        (EdgeMarker::OpenTriangle, true) => "<|",
        (EdgeMarker::OpenTriangle, false) => "|>",
        (EdgeMarker::OpenDiamond, _) => "o",
        (EdgeMarker::FilledDiamond, _) => "*",
        (EdgeMarker::Circle, _) => "()",
        (EdgeMarker::Cross, _) => "x",
    }
}

pub(crate) fn wrap_words(text: &str, max_width: usize) -> Vec<String> {
    let max_width = max_width.max(1);
    let sanitized = sanitize_label_text(text);
    if str_display_width(&sanitized) <= max_width {
        return vec![sanitized];
    }
    let text = normalized(&sanitized);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0usize;

    for word in text.split_whitespace() {
        let word_width = str_display_width(word);
        if !line.is_empty() && line_width.saturating_add(1 + word_width) <= max_width {
            line.push(' ');
            line.push_str(word);
            line_width += 1 + word_width;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            line_width = 0;
        }
        if word_width <= max_width {
            line.push_str(word);
            line_width = word_width;
            continue;
        }

        let mut fragments = wrap_display(word, max_width);
        if let Some(last) = fragments.pop() {
            lines.extend(fragments);
            line_width = str_display_width(&last);
            line = last;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn coalesced_geometry_edges(edges: &[BoxEdge]) -> Vec<BoxEdge> {
    let mut consumed = vec![false; edges.len()];
    let mut geometry = Vec::with_capacity(edges.len());
    for (index, edge) in edges.iter().enumerate() {
        if consumed[index] {
            continue;
        }
        consumed[index] = true;
        let mut combined = edge.clone();
        if edge.from != edge.to
            && let Some((reverse_index, reverse)) =
                edges
                    .iter()
                    .enumerate()
                    .skip(index + 1)
                    .find(|(candidate_index, candidate)| {
                        !consumed[*candidate_index]
                            && candidate.from == edge.to
                            && candidate.to == edge.from
                            && candidate.from_side == edge.to_side
                            && candidate.to_side == edge.from_side
                    })
        {
            consumed[reverse_index] = true;
            combined.marker_start = combine_marker(combined.marker_start, reverse.marker_end);
            combined.marker_end = combine_marker(combined.marker_end, reverse.marker_start);
        }
        geometry.push(combined);
    }
    geometry
}

fn combine_marker(existing: EdgeMarker, reverse: EdgeMarker) -> EdgeMarker {
    if existing == EdgeMarker::None {
        reverse
    } else {
        existing
    }
}

fn write_centered(
    canvas: &mut Canvas,
    left: usize,
    y: usize,
    width: usize,
    text: &str,
) -> Result<()> {
    let text_width = str_display_width(text);
    let x = left.saturating_add(width.saturating_sub(text_width) / 2);
    canvas.set_text(x, y, text)
}

pub(crate) fn wrap_display(text: &str, max_width: usize) -> Vec<String> {
    let max_width = max_width.max(1);
    let text = sanitize_label_text(text);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut width = 0usize;
    for grapheme in UnicodeSegmentation::graphemes(text.as_str(), true) {
        if grapheme == "\n" {
            lines.push(line.trim_end().to_owned());
            line.clear();
            width = 0;
            continue;
        }
        let grapheme_width = str_display_width(grapheme).max(1);
        if width > 0 && width.saturating_add(grapheme_width) > max_width {
            lines.push(line.trim_end().to_owned());
            line.clear();
            width = 0;
        }
        if width == 0 && grapheme.chars().all(char::is_whitespace) {
            continue;
        }
        line.push_str(grapheme);
        width += grapheme_width;
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line.trim_end().to_owned());
    }
    lines
}

fn normalized(value: &str) -> String {
    sanitize_label_text(value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const fn ordered(first: usize, second: usize) -> (usize, usize) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn layout_error(family: &'static str, message: impl Into<String>) -> MermansiError {
    MermansiError::GeometryLayout {
        family,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_router_crosses_painted_border_instead_of_following_it() {
        let mut canvas = Canvas::new(16, 9).expect("canvas");
        draw_horizontal_line(&mut canvas, 4, 2, 13, Charset::Unicode).expect("border");
        let width = canvas.width();
        let height = canvas.height();
        let start = Point::new(3, 4);
        let target = Point::new(12, 4);
        let blocked = vec![false; width * height];
        let routed = vec![false; width * height];

        let predecessors = weighted_predecessors(start, width, height, &blocked, &routed, &canvas);
        let cells = reconstruct_path(
            grid_index(start, width),
            grid_index(target, width),
            width,
            &predecessors,
        )
        .expect("route");
        let painted_cells = cells
            .iter()
            .filter(|point| {
                canvas
                    .get_cell(point.x, point.y)
                    .is_some_and(|cell| !cell.is_empty())
            })
            .count();

        assert_eq!(
            painted_cells, 2,
            "path followed the painted border: {cells:?}"
        );
    }
}
