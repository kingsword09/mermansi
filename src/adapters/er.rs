//! ER entity compartments and explicit cardinality relationship routes.

use std::collections::HashMap;

use crate::adapters::box_geometry::{self, BoxDiagram, BoxLayout, BoxNode, wrap_display};
use crate::adapters::{detail_separator, nonempty_or};
use crate::ansi::sanitize_label_text;
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};

const MAX_ER_ITEMS: usize = 4_096;

pub fn render_er(model: &ErDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let attribute_count = model.entities.values().fold(0usize, |count, entity| {
        count.saturating_add(entity.attributes.len())
    });
    let item_count = model
        .entities
        .len()
        .saturating_add(model.relationships.len())
        .saturating_add(attribute_count);
    if item_count > MAX_ER_ITEMS {
        return Err(MermansiError::RenderLimit {
            context: "er items",
            requested: item_count,
            limit: MAX_ER_ITEMS,
        });
    }

    if model.entities.is_empty() && model.relationships.is_empty() {
        return Ok("ER diagram\n\n(empty ER diagram)\n".to_owned());
    }

    let nodes = model
        .entities
        .iter()
        .enumerate()
        .map(|(order, (id, entity))| entity_node(id, entity, order))
        .collect::<Vec<_>>();
    let mut output = if nodes.is_empty() {
        String::new()
    } else {
        box_geometry::render(
            &BoxDiagram {
                family: "er",
                title: model.acc_title.clone(),
                nodes,
                groups: Vec::new(),
                spacers: Vec::new(),
                edges: Vec::new(),
                columns: None,
                layout: BoxLayout::Packed,
                edge_legend: box_geometry::EdgeLegend::None,
            },
            opts,
        )?
    };

    if model.relationships.is_empty() {
        return Ok(output);
    }

    let labels = entity_labels(model);
    let (direction, reverse) = normalized_direction(&model.direction);
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.trim().is_empty() {
        output.push('\n');
    }
    for line in wrap_display(
        &format!("Relationships{}{direction}", detail_separator(opts.charset)),
        opts.max_width,
    ) {
        output.push_str(&line);
        output.push('\n');
    }

    for (index, relationship) in model.relationships.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        for line in relationship_lines(relationship, &labels, reverse, opts)? {
            output.push_str(&line);
            output.push('\n');
        }
    }
    Ok(output)
}

fn entity_node(id: &str, entity: &ErEntityRenderModel, order: usize) -> BoxNode {
    let mut lines = vec![entity_label(id, entity)];
    let mut dividers = Vec::new();
    if !entity.attributes.is_empty() {
        dividers.push(lines.len());
        lines.push(String::new());
        lines.extend(
            entity
                .attributes
                .iter()
                .map(attribute_text)
                .map(|attribute| nonempty_or(&attribute, "(empty attribute)")),
        );
    }
    BoxNode {
        id: format!("er-{order}"),
        lines,
        dividers,
        parent: None,
        span: 1,
        order,
    }
}

fn entity_label(id: &str, entity: &ErEntityRenderModel) -> String {
    let label = if !entity.alias.trim().is_empty() {
        &entity.alias
    } else if !entity.label.trim().is_empty() {
        &entity.label
    } else if !entity.id.trim().is_empty() {
        &entity.id
    } else {
        id
    };
    nonempty_or(&sanitize_label_text(label), "(unnamed entity)")
}

fn attribute_text(attribute: &ErAttributeRenderModel) -> String {
    let mut parts = [&attribute.ty, &attribute.name]
        .into_iter()
        .map(|value| sanitize_label_text(value))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let keys = attribute
        .keys
        .iter()
        .map(|key| sanitize_label_text(key))
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
        .collect::<Vec<_>>()
        .join(",");
    if !keys.is_empty() {
        parts.push(keys);
    }
    let comment = sanitize_label_text(&attribute.comment).trim().to_owned();
    if !comment.is_empty() {
        parts.push(comment);
    }
    parts.join(" ")
}

fn entity_labels(model: &ErDiagramRenderModel) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    for (id, entity) in &model.entities {
        let label = entity_label(id, entity);
        labels.insert(id.clone(), label.clone());
        if !entity.id.is_empty() {
            labels.entry(entity.id.clone()).or_insert(label);
        }
    }
    labels
}

fn relationship_lines(
    relationship: &ErRelationshipRenderModel,
    labels: &HashMap<String, String>,
    reverse: bool,
    opts: &MermansiOptions,
) -> Result<Vec<String>> {
    let source = (
        endpoint_label(labels, &relationship.entity_a),
        cardinality_marker(&relationship.rel_spec.card_b)?,
    );
    let target = (
        endpoint_label(labels, &relationship.entity_b),
        cardinality_marker(&relationship.rel_spec.card_a)?,
    );
    let (first, last) = if reverse {
        (target, source)
    } else {
        (source, target)
    };
    let prefix_width = 3usize;
    let content_width = opts.max_width.saturating_sub(prefix_width);
    if content_width < 4 {
        return Err(MermansiError::RenderLimit {
            context: "er relationship columns",
            requested: prefix_width.saturating_add(4),
            limit: opts.max_width,
        });
    }

    let first_lines = endpoint_lines(&first.0, first.1, content_width);
    let role = sanitize_label_text(&relationship.role_a);
    let role = nonempty_or(role.trim(), "(unlabeled)");
    let role_lines = wrap_display(&role, content_width);
    let last_lines = endpoint_lines(&last.0, last.1, content_width);
    let chars = RelationshipChars::new(opts.charset, &relationship.rel_spec.rel_type)?;
    let mut lines = Vec::with_capacity(first_lines.len() + role_lines.len() + last_lines.len());

    for (index, line) in first_lines.iter().enumerate() {
        lines.push(format!(
            "{}{}",
            if index == 0 { chars.top } else { chars.stem },
            line
        ));
    }
    for (index, line) in role_lines.iter().enumerate() {
        lines.push(format!(
            "{}{}",
            if index == 0 {
                chars.relationship
            } else {
                chars.stem
            },
            line
        ));
    }
    for (index, line) in last_lines.iter().enumerate() {
        lines.push(format!(
            "{}{}",
            if index + 1 == last_lines.len() {
                chars.bottom
            } else {
                chars.stem
            },
            line
        ));
    }
    Ok(lines)
}

fn endpoint_label(labels: &HashMap<String, String>, id: &str) -> String {
    labels
        .get(id)
        .cloned()
        .unwrap_or_else(|| nonempty_or(&sanitize_label_text(id), "(missing entity)"))
}

fn endpoint_lines(label: &str, marker: &str, width: usize) -> Vec<String> {
    let suffix = format!("  {marker}");
    let label_width = width
        .saturating_sub(crate::str_display_width(&suffix))
        .max(1);
    let mut lines = wrap_display(label, label_width);
    if let Some(last) = lines.last_mut() {
        last.push_str(&suffix);
    }
    lines
}

fn cardinality_marker(cardinality: &str) -> Result<&'static str> {
    match cardinality {
        "ONLY_ONE" => Ok("||"),
        "ZERO_OR_ONE" => Ok("o|"),
        "ONE_OR_MORE" => Ok("|{"),
        "ZERO_OR_MORE" => Ok("o{"),
        "MD_PARENT" => Ok("P|"),
        _ => Err(layout_error(format!(
            "unknown relationship cardinality: {cardinality}"
        ))),
    }
}

fn normalized_direction(direction: &str) -> (&'static str, bool) {
    match direction.to_ascii_uppercase().as_str() {
        "BT" => ("BT", true),
        "LR" => ("LR", false),
        "RL" => ("RL", true),
        _ => ("TB", false),
    }
}

struct RelationshipChars {
    top: &'static str,
    stem: &'static str,
    relationship: &'static str,
    bottom: &'static str,
}

impl RelationshipChars {
    fn new(charset: Charset, relationship_type: &str) -> Result<Self> {
        let dotted = match relationship_type {
            "" | "IDENTIFYING" => false,
            "NON_IDENTIFYING" => true,
            _ => {
                return Err(layout_error(format!(
                    "unknown relationship identification type: {relationship_type}"
                )));
            }
        };
        Ok(match (charset, dotted) {
            (Charset::Unicode, false) => Self {
                top: "┌─ ",
                stem: "│  ",
                relationship: "├─ ",
                bottom: "└─ ",
            },
            (Charset::Unicode, true) => Self {
                top: "┌─ ",
                stem: "│  ",
                relationship: "├╌ ",
                bottom: "└─ ",
            },
            (Charset::Ascii, false) => Self {
                top: "+- ",
                stem: "|  ",
                relationship: "|- ",
                bottom: "+- ",
            },
            (Charset::Ascii, true) => Self {
                top: "+- ",
                stem: "|  ",
                relationship: "|. ",
                bottom: "+- ",
            },
        })
    }
}

fn layout_error(message: String) -> MermansiError {
    MermansiError::GeometryLayout {
        family: "er",
        message,
    }
}
