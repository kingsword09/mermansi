//! Event modeling diagram adapter.

use crate::adapters::format_title;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::eventmodeling::EventModelingDiagramRenderModel;

pub fn render_eventmodeling(
    model: &EventModelingDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    if !model.frames.is_empty() {
        out.push_str("Frames:\n");
        for (i, frame) in model.frames.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] {} ({})\n",
                i + 1,
                frame.name,
                frame.frame_kind
            ));
            out.push_str(&format!(
                "       entity: {} ({})\n",
                frame.entity_identifier, frame.model_entity_type
            ));
            if let Some(val) = &frame.data_inline_value {
                out.push_str(&format!("       value: {val}\n"));
            }
            if let Some(reference) = &frame.data_reference {
                out.push_str(&format!("       ref: {reference}\n"));
            }
        }
        out.push('\n');
    }

    if !model.data_entities.is_empty() {
        out.push_str("Data Entities:\n");
        for entity in &model.data_entities {
            out.push_str(&format!(
                "  {} = {}\n",
                entity.name, entity.data_block_value
            ));
        }
    }

    if out.trim().is_empty() {
        out.push_str("(empty event modeling diagram)\n");
    }

    let _ = opts;
    Ok(out)
}
