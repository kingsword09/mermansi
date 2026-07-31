//! C4 diagram terminal geometry.

use crate::adapters::box_geometry::{self, BoxDiagram, BoxEdge, BoxGroup, BoxNode};
use crate::adapters::detail_separator;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::c4::{
    C4BoundaryRenderModel, C4DiagramRenderModel, C4RelRenderModel, C4ShapeRenderModel, C4Text,
};

pub fn render_c4(model: &C4DiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let groups = model
        .boundaries
        .iter()
        .filter(|boundary| !is_global(boundary))
        .enumerate()
        .map(|(order, boundary)| boundary_group(boundary, model, order, opts.charset))
        .collect::<Vec<_>>();
    let node_order = groups.len();
    let nodes = model
        .shapes
        .iter()
        .enumerate()
        .map(|(order, shape)| shape_node(shape, node_order + order, opts.charset))
        .collect();
    let mut title = if model.c4_type.is_empty() {
        "C4".to_owned()
    } else {
        model.c4_type.clone()
    };
    if let Some(diagram_title) = model
        .title
        .as_deref()
        .map(normalized)
        .filter(|title| !title.is_empty())
    {
        title.push_str(detail_separator(opts.charset));
        title.push_str(&diagram_title);
    }

    box_geometry::render(
        &BoxDiagram {
            family: "c4",
            title: Some(title),
            nodes,
            groups,
            spacers: Vec::new(),
            edges: model
                .rels
                .iter()
                .map(|relation| relation_edge(relation, opts.charset))
                .collect(),
            columns: positive(model.layout.c4_boundary_in_row)
                .or_else(|| positive(model.layout.c4_shape_in_row)),
        },
        opts,
    )
}

fn boundary_group(
    boundary: &C4BoundaryRenderModel,
    model: &C4DiagramRenderModel,
    order: usize,
    charset: Charset,
) -> BoxGroup {
    let mut lines = vec![display_identity(
        &boundary.alias,
        boundary.label.as_str(),
        charset,
    )];
    let kind = boundary
        .node_type
        .as_deref()
        .or_else(|| boundary.ty.as_ref().map(C4Text::as_str))
        .map(normalized)
        .filter(|kind| !kind.is_empty());
    if let Some(kind) = kind {
        lines.push(format!("[{kind}]"));
    }
    if let Some(description) = boundary
        .descr
        .as_ref()
        .map(C4Text::as_str)
        .map(normalized)
        .filter(|description| !description.is_empty())
    {
        lines.push(description);
    }
    BoxGroup {
        id: boundary.alias.clone(),
        lines,
        parent: visible_parent(&boundary.parent_boundary),
        columns: positive(model.layout.c4_shape_in_row),
        span: 1,
        order,
    }
}

fn shape_node(shape: &C4ShapeRenderModel, order: usize, charset: Charset) -> BoxNode {
    let mut lines = vec![display_identity(
        &shape.alias,
        shape.label.as_str(),
        charset,
    )];
    let kind = normalized(shape.type_c4_shape.as_str());
    if !kind.is_empty() {
        lines.push(format!("[{kind}]"));
    }
    if let Some(technology) = shape
        .techn
        .as_ref()
        .map(C4Text::as_str)
        .map(normalized)
        .filter(|technology| !technology.is_empty())
    {
        lines.push(format!(
            "technology{}{technology}",
            detail_separator(charset)
        ));
    }
    if let Some(description) = shape
        .descr
        .as_ref()
        .map(C4Text::as_str)
        .map(normalized)
        .filter(|description| !description.is_empty())
    {
        lines.push(description);
    }
    BoxNode {
        id: shape.alias.clone(),
        lines,
        parent: visible_parent(&shape.parent_boundary),
        span: 1,
        order,
    }
}

fn relation_edge(relation: &C4RelRenderModel, charset: Charset) -> BoxEdge {
    let mut details = normalized(relation.label.as_str());
    if let Some(technology) = relation
        .techn
        .as_ref()
        .map(C4Text::as_str)
        .map(normalized)
        .filter(|technology| !technology.is_empty())
    {
        if !details.is_empty() {
            details.push_str(detail_separator(charset));
        }
        details.push_str(&technology);
    }
    if let Some(description) = relation
        .descr
        .as_ref()
        .map(C4Text::as_str)
        .map(normalized)
        .filter(|description| !description.is_empty())
    {
        if !details.is_empty() {
            details.push_str(detail_separator(charset));
        }
        details.push_str(&description);
    }
    let bidirectional = relation.rel_type.to_ascii_lowercase().contains("birel");
    BoxEdge {
        from: relation.from_alias.clone(),
        to: relation.to_alias.clone(),
        label: details,
        arrow_start: bidirectional,
        arrow_end: true,
        from_side: None,
        to_side: None,
    }
}

fn is_global(boundary: &C4BoundaryRenderModel) -> bool {
    boundary.alias == "global" && boundary.parent_boundary.is_empty()
}

fn visible_parent(parent: &str) -> Option<String> {
    (!parent.is_empty() && parent != "global").then(|| parent.to_owned())
}

fn display_identity(id: &str, label: &str, charset: Charset) -> String {
    let label = normalized(label);
    if label.is_empty() || label == id {
        id.to_owned()
    } else {
        format!("{id}{}{label}", detail_separator(charset))
    }
}

fn positive(value: i64) -> Option<usize> {
    (value > 0)
        .then_some(value)
        .and_then(|value| usize::try_from(value).ok())
}

fn normalized(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
