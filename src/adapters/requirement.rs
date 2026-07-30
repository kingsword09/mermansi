//! Requirement diagram adapter.

use crate::adapters::nonempty_or;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::requirement::{
    RequirementDiagramRenderModel, RequirementRenderElement, RequirementRenderNode,
};

pub fn render_requirement(
    model: &RequirementDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();

    if !model.requirements.is_empty() {
        out.push_str("Requirements:\n");
        for req in &model.requirements {
            out.push_str(&format_requirement(req));
        }
        out.push('\n');
    }

    if !model.elements.is_empty() {
        out.push_str("Elements:\n");
        for elem in &model.elements {
            out.push_str(&format_element(elem));
        }
        out.push('\n');
    }

    if !model.relationships.is_empty() {
        out.push_str("Relationships:\n");
        for rel in &model.relationships {
            out.push_str(&format!(
                "  {} -[{}]-> {}\n",
                rel.src, rel.rel_type, rel.dst
            ));
        }
    }

    if out.trim().is_empty() {
        out.push_str("(empty requirement diagram)\n");
    }

    let _ = opts;
    Ok(out)
}

fn format_requirement(req: &RequirementRenderNode) -> String {
    let mut out = format!(
        "  [{}] {} ({})\n",
        req.node_type,
        nonempty_or(&req.name, "(unnamed)"),
        req.requirement_id
    );
    if !req.text.is_empty() {
        out.push_str(&format!("    Text: {}\n", req.text));
    }
    if !req.risk.is_empty() {
        out.push_str(&format!("    Risk: {}\n", req.risk));
    }
    if !req.verify_method.is_empty() {
        out.push_str(&format!("    Verify: {}\n", req.verify_method));
    }
    out
}

fn format_element(elem: &RequirementRenderElement) -> String {
    format!(
        "  [{}] {} ({})\n",
        elem.element_type,
        nonempty_or(&elem.name, "(unnamed)"),
        elem.doc_ref
    )
}
