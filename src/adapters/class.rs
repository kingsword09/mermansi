//! Class diagram terminal geometry.
//!
//! Class-like entities use compartment boxes. Relationships reuse the shared bounded orthogonal
//! router with UML endpoint markers so multi-inheritance and mixed relation kinds stay connected.

use std::collections::HashMap;

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxEdge, BoxGroup, BoxLayout, BoxNode, EdgeLegend, EdgeMarker,
    EdgeStyle, directed_ranks,
};
use crate::ansi::sanitize_label_text;
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use merman_core::models::class_diagram::{
    ClassConstants, ClassDiagram, ClassMember, ClassNode, ClassRelation, RelationShape,
};

const MAX_CLASS_ENTITIES: usize = 4_096;

pub fn render_class(model: &ClassDiagram, opts: &MermansiOptions) -> Result<String> {
    let entity_count = model
        .classes
        .len()
        .saturating_add(model.notes.len())
        .saturating_add(model.interfaces.len())
        .saturating_add(model.namespaces.len());
    ensure_limit("class entities", entity_count, MAX_CLASS_ENTITIES)?;
    ensure_limit(
        "class relationships",
        model
            .relations
            .len()
            .saturating_add(model.notes.len())
            .saturating_add(model.interfaces.len()),
        MAX_CLASS_ENTITIES,
    )?;

    let namespace_ids = model
        .namespaces
        .keys()
        .enumerate()
        .map(|(index, id)| (id.as_str(), format!("__class_namespace_{index}")))
        .collect::<HashMap<_, _>>();
    let mut class_namespaces = HashMap::<&str, String>::new();
    for (namespace_id, namespace) in &model.namespaces {
        let Some(geometry_id) = namespace_ids.get(namespace_id.as_str()) else {
            continue;
        };
        for class_id in &namespace.class_ids {
            class_namespaces
                .entry(class_id.as_str())
                .or_insert_with(|| geometry_id.clone());
        }
    }

    let groups = model
        .namespaces
        .values()
        .enumerate()
        .map(|(order, namespace)| BoxGroup {
            id: namespace_ids[namespace.id.as_str()].clone(),
            lines: vec![format!(
                "[namespace] {}",
                nonempty(if namespace.label.trim().is_empty() {
                    &namespace.id
                } else {
                    &namespace.label
                })
            )],
            parent: namespace
                .parent
                .as_deref()
                .and_then(|parent| namespace_ids.get(parent))
                .cloned(),
            columns: None,
            span: 1,
            order,
        })
        .collect::<Vec<_>>();

    let mut nodes = Vec::with_capacity(entity_count.max(1));
    for (order, class) in model.classes.values().enumerate() {
        let mut node = class_node(class, order);
        node.parent = class
            .parent
            .as_deref()
            .and_then(|parent| namespace_ids.get(parent))
            .cloned()
            .or_else(|| class_namespaces.get(class.id.as_str()).cloned());
        nodes.push(node);
    }

    let note_offset = nodes.len();
    for (index, note) in model.notes.iter().enumerate() {
        let mut lines = vec!["[note]".to_owned()];
        append_multiline(&mut lines, &note.text);
        nodes.push(BoxNode {
            id: note_id(index),
            lines,
            dividers: Vec::new(),
            parent: note
                .parent
                .as_deref()
                .and_then(|parent| namespace_ids.get(parent))
                .cloned(),
            span: 1,
            order: note_offset + index,
        });
    }

    let interface_offset = nodes.len();
    for (index, interface) in model.interfaces.iter().enumerate() {
        nodes.push(BoxNode {
            id: interface_id(index),
            lines: vec![format!("[interface] {}", nonempty(&interface.label))],
            dividers: Vec::new(),
            parent: model
                .classes
                .get(&interface.class_id)
                .and_then(|class| class.parent.as_deref())
                .and_then(|parent| namespace_ids.get(parent))
                .cloned()
                .or_else(|| class_namespaces.get(interface.class_id.as_str()).cloned()),
            span: 1,
            order: interface_offset + index,
        });
    }

    if nodes.is_empty() && groups.is_empty() {
        nodes.push(BoxNode {
            id: "__empty_class".to_owned(),
            lines: vec!["(empty class diagram)".to_owned()],
            dividers: Vec::new(),
            parent: None,
            span: 1,
            order: 0,
        });
    }

    let direction = BoxDirection::from_str(&model.direction);
    let (from_side, to_side) = direction.edge_sides();
    let mut edges = model
        .relations
        .iter()
        .map(|relation| relation_edge(relation, &model.constants, from_side, to_side))
        .collect::<Result<Vec<_>>>()?;
    for (index, note) in model.notes.iter().enumerate() {
        let Some(class_id) = note.class_id.as_deref() else {
            continue;
        };
        ensure_class(model, class_id)?;
        edges.push(BoxEdge {
            from: class_id.to_owned(),
            to: note_id(index),
            label: String::new(),
            marker_start: EdgeMarker::None,
            marker_end: EdgeMarker::None,
            style: EdgeStyle::Dotted,
            from_side: Some(from_side),
            to_side: Some(to_side),
        });
    }
    for (index, interface) in model.interfaces.iter().enumerate() {
        ensure_class(model, &interface.class_id)?;
        edges.push(BoxEdge {
            from: interface.class_id.clone(),
            to: interface_id(index),
            label: String::new(),
            marker_start: EdgeMarker::None,
            marker_end: EdgeMarker::Circle,
            style: EdgeStyle::Solid,
            from_side: Some(from_side),
            to_side: Some(to_side),
        });
    }

    let layout = if groups.is_empty() {
        let ranks = directed_ranks(&nodes, &edges);
        BoxLayout::Layered { direction, ranks }
    } else {
        BoxLayout::Packed
    };
    box_geometry::render(
        &BoxDiagram {
            family: "class",
            title: model
                .acc_title
                .clone()
                .or_else(|| Some("Class diagram".to_owned())),
            nodes,
            groups,
            spacers: Vec::new(),
            edges,
            columns: None,
            layout,
            edge_legend: EdgeLegend::Labeled,
        },
        opts,
    )
}

fn class_node(class: &ClassNode, order: usize) -> BoxNode {
    let mut lines = class
        .annotations
        .iter()
        .map(|annotation| format!("<<{}>>", sanitize_label_text(annotation)))
        .collect::<Vec<_>>();
    let mut title = nonempty(if class.label.trim().is_empty() {
        &class.id
    } else {
        &class.label
    });
    if !class.type_param.trim().is_empty() {
        title.push('<');
        title.push_str(&sanitize_label_text(class.type_param.trim()));
        title.push('>');
    }
    lines.push(title);
    let mut dividers = Vec::new();
    append_compartment(&mut lines, &mut dividers, &class.members);
    append_compartment(&mut lines, &mut dividers, &class.methods);
    BoxNode {
        id: class.id.clone(),
        lines,
        dividers,
        parent: None,
        span: 1,
        order,
    }
}

fn append_compartment(lines: &mut Vec<String>, dividers: &mut Vec<usize>, members: &[ClassMember]) {
    if members.is_empty() {
        return;
    }
    dividers.push(lines.len());
    lines.push(String::new());
    lines.extend(members.iter().map(|member| nonempty(&member.display_text)));
}

fn relation_edge(
    relation: &ClassRelation,
    constants: &ClassConstants,
    from_side: box_geometry::Side,
    to_side: box_geometry::Side,
) -> Result<BoxEdge> {
    if relation.id1.trim().is_empty() || relation.id2.trim().is_empty() {
        return Err(MermansiError::GeometryLayout {
            family: "class",
            message: "relationship endpoint is empty".to_owned(),
        });
    }
    let marker_start = relation_marker(relation.relation.type1, constants);
    let marker_end = relation_marker(relation.relation.type2, constants);
    Ok(BoxEdge {
        from: relation.id1.clone(),
        to: relation.id2.clone(),
        label: relation_label(relation),
        marker_start,
        marker_end,
        style: relation_style(&relation.relation, constants),
        from_side: Some(marker_port(marker_start, from_side)),
        to_side: Some(marker_port(marker_end, to_side)),
    })
}

fn marker_port(marker: EdgeMarker, default: box_geometry::Side) -> box_geometry::Side {
    match (marker, default) {
        (
            EdgeMarker::OpenDiamond | EdgeMarker::Circle,
            box_geometry::Side::Top | box_geometry::Side::Bottom,
        ) => box_geometry::Side::Left,
        (
            EdgeMarker::FilledDiamond | EdgeMarker::Arrow,
            box_geometry::Side::Top | box_geometry::Side::Bottom,
        ) => box_geometry::Side::Right,
        (
            EdgeMarker::OpenDiamond | EdgeMarker::Circle,
            box_geometry::Side::Left | box_geometry::Side::Right,
        ) => box_geometry::Side::Top,
        (
            EdgeMarker::FilledDiamond | EdgeMarker::Arrow,
            box_geometry::Side::Left | box_geometry::Side::Right,
        ) => box_geometry::Side::Bottom,
        _ => default,
    }
}

fn relation_marker(kind: i32, constants: &ClassConstants) -> EdgeMarker {
    let kinds = &constants.relation_type;
    if kind == kinds.extension {
        EdgeMarker::OpenTriangle
    } else if kind == kinds.aggregation {
        EdgeMarker::OpenDiamond
    } else if kind == kinds.composition {
        EdgeMarker::FilledDiamond
    } else if kind == kinds.dependency {
        EdgeMarker::Arrow
    } else if kind == kinds.lollipop {
        EdgeMarker::Circle
    } else {
        EdgeMarker::None
    }
}

fn relation_style(relation: &RelationShape, constants: &ClassConstants) -> EdgeStyle {
    if relation.line_type == constants.line_type.dotted_line {
        EdgeStyle::Dotted
    } else {
        EdgeStyle::Solid
    }
}

fn relation_label(relation: &ClassRelation) -> String {
    let mut parts = Vec::new();
    for value in [
        &relation.title,
        &relation.relation_title_1,
        &relation.relation_title_2,
    ] {
        let value = sanitize_label_text(value).trim().to_owned();
        if !value.is_empty() && !value.eq_ignore_ascii_case("none") && !parts.contains(&value) {
            parts.push(value);
        }
    }
    parts.join(" / ")
}

fn ensure_class(model: &ClassDiagram, id: &str) -> Result<()> {
    if model.classes.contains_key(id) {
        Ok(())
    } else {
        Err(MermansiError::GeometryLayout {
            family: "class",
            message: format!("relationship class is missing: {}", sanitize_label_text(id)),
        })
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

fn append_multiline(lines: &mut Vec<String>, text: &str) {
    lines.extend(
        text.lines()
            .map(sanitize_label_text)
            .filter(|line| !line.trim().is_empty()),
    );
    if lines.len() == 1 {
        lines.push("(empty note)".to_owned());
    }
}

fn nonempty(value: &str) -> String {
    let value = sanitize_label_text(value).trim().to_owned();
    if value.is_empty() {
        "(unnamed)".to_owned()
    } else {
        value
    }
}

fn note_id(index: usize) -> String {
    format!("__class_note_{index}")
}

fn interface_id(index: usize) -> String {
    format!("__class_interface_{index}")
}
