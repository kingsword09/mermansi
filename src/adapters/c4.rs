//! C4 diagram adapter.
//!
//! Renders C4 boundaries, shapes, and relationships as structured terminal text.

use crate::adapters::{format_title, nonempty_or};
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::c4::{
    C4BoundaryRenderModel, C4DiagramRenderModel, C4RelRenderModel, C4ShapeRenderModel, C4Text,
};

pub fn render_c4(model: &C4DiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    if !model.boundaries.is_empty() {
        out.push_str("Boundaries:\n");
        for b in &model.boundaries {
            out.push_str(&format_boundary(b, 1));
        }
        out.push('\n');
    }

    if !model.shapes.is_empty() {
        out.push_str("Shapes:\n");
        for s in &model.shapes {
            out.push_str(&format_shape(s));
        }
        out.push('\n');
    }

    if !model.rels.is_empty() {
        out.push_str("Relationships:\n");
        for r in &model.rels {
            out.push_str(&format_rel(r));
        }
    }

    if out.trim().is_empty() {
        out.push_str("(empty C4 diagram)\n");
    }

    let _ = opts;
    Ok(out)
}

fn format_boundary(b: &C4BoundaryRenderModel, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let alias = &b.alias;
    let type_label =
        b.ty.as_ref()
            .map(|t| c4_text_or(t, b.parent_boundary.as_str()))
            .unwrap_or_else(|| b.parent_boundary.clone());
    let label = b.label.as_str();
    let parent = if b.parent_boundary.is_empty() {
        "-"
    } else {
        &b.parent_boundary
    };
    let mut out = format!(
        "{indent}[{type_label}] {alias}: {} (parent: {parent})\n",
        nonempty_or(label, alias)
    );
    if let Some(descr) = &b.descr {
        let d = descr.as_str();
        if !d.is_empty() {
            out.push_str(&format!("{indent}  description: {d}\n"));
        }
    }
    out
}

fn format_shape(s: &C4ShapeRenderModel) -> String {
    let label = nonempty_or(s.label.as_str(), &s.alias);
    let shape_type = c4_text_or(&s.type_c4_shape, "shape");
    let parent = if s.parent_boundary.is_empty() {
        "-"
    } else {
        &s.parent_boundary
    };
    let mut out = format!("  [{shape_type}] {label} ({}) parent: {parent}\n", s.alias);
    if let Some(techn) = &s.techn {
        let t = techn.as_str();
        if !t.is_empty() {
            out.push_str(&format!("    techn: {t}\n"));
        }
    }
    if let Some(descr) = &s.descr {
        let d = descr.as_str();
        if !d.is_empty() {
            out.push_str(&format!("    description: {d}\n"));
        }
    }
    out
}

fn format_rel(r: &C4RelRenderModel) -> String {
    let label = r.label.as_str();
    let label = nonempty_or(label, "(unlabeled)");
    let techn = r
        .techn
        .as_ref()
        .map(|t| t.as_str().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_default();
    if techn.is_empty() {
        format!(
            "  {} -[{}]-> {}\n    {}\n",
            r.from_alias, r.rel_type, r.to_alias, label
        )
    } else {
        format!(
            "  {} -[{}]-> {}\n    {} [{}]\n",
            r.from_alias, r.rel_type, r.to_alias, label, techn
        )
    }
}

fn c4_text_or(text: &C4Text, fallback: &str) -> String {
    let s = text.as_str();
    if s.is_empty() {
        fallback.to_string()
    } else {
        s.to_string()
    }
}
