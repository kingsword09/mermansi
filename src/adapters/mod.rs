//! Semantic adapter dispatch.
//!
//! Routes each [`RenderSemanticModel`] variant to the appropriate renderer. Families covered by
//! `merman-ascii` are delegated; remaining families use mermansi's own structured terminal
//! adapters.

use crate::adapters::{
    architecture::render_architecture, block::render_block, c4::render_c4,
    eventmodeling::render_eventmodeling, flowchart::render_flowchart, info::render_info,
    ishikawa::render_ishikawa, json::render_json, pie::render_pie,
    quadrant_chart::render_quadrant_chart, radar::render_radar, requirement::render_requirement,
    sankey::render_sankey, treemap::render_treemap, venn::render_venn,
};
use crate::error::Result;
use crate::options::{Charset, ColorMode, MermansiOptions};
use crate::output::append_structured_model;
use merman_ascii::{AsciiColorMode, AsciiRenderOptions};
use merman_core::diagram::RenderSemanticModel;

pub mod architecture;
pub mod block;
pub mod c4;
pub mod eventmodeling;
pub mod flowchart;
pub mod info;
pub mod ishikawa;
pub mod json;
pub mod pie;
pub mod quadrant_chart;
pub mod radar;
pub mod requirement;
pub mod sankey;
pub mod treemap;
pub mod venn;

/// Convert `MermansiOptions` to the `merman-ascii` equivalent.
pub(crate) fn to_ascii_options(opts: &MermansiOptions) -> AsciiRenderOptions {
    let color_mode = match opts.color_mode {
        ColorMode::Plain => AsciiColorMode::Plain,
        ColorMode::Ansi16 => AsciiColorMode::Ansi16,
        ColorMode::TrueColor => AsciiColorMode::TrueColor,
    };
    let ascii_options = match opts.charset {
        Charset::Unicode => AsciiRenderOptions::unicode(),
        Charset::Ascii => AsciiRenderOptions::ascii(),
    };
    ascii_options
        .with_max_grid_cells(opts.max_width.saturating_mul(opts.max_height))
        .with_color_mode(color_mode)
}

/// Render any [`RenderSemanticModel`] variant.
pub fn render_model(model: &RenderSemanticModel, opts: &MermansiOptions) -> Result<String> {
    opts.validate()?;
    match model {
        // --- merman-ascii delegated families ---
        RenderSemanticModel::Flowchart(m) => render_flowchart(m, opts),
        RenderSemanticModel::Sequence(m) => merman_ascii::render_model(
            &RenderSemanticModel::Sequence(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::State(m) => merman_ascii::render_model(
            &RenderSemanticModel::State(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Class(m) => merman_ascii::render_model(
            &RenderSemanticModel::Class(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Er(m) => {
            merman_ascii::render_model(&RenderSemanticModel::Er(m.clone()), &to_ascii_options(opts))
                .map_err(Into::into)
        }
        RenderSemanticModel::Packet(m) => merman_ascii::render_model(
            &RenderSemanticModel::Packet(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::TreeView(m) => merman_ascii::render_model(
            &RenderSemanticModel::TreeView(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::XyChart(m) => merman_ascii::render_model(
            &RenderSemanticModel::XyChart(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Mindmap(m) => merman_ascii::render_model(
            &RenderSemanticModel::Mindmap(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Gantt(m) => merman_ascii::render_model(
            &RenderSemanticModel::Gantt(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::GitGraph(m) => merman_ascii::render_model(
            &RenderSemanticModel::GitGraph(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Journey(m) => merman_ascii::render_model(
            &RenderSemanticModel::Journey(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Kanban(m) => merman_ascii::render_model(
            &RenderSemanticModel::Kanban(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),
        RenderSemanticModel::Timeline(m) => merman_ascii::render_model(
            &RenderSemanticModel::Timeline(m.clone()),
            &to_ascii_options(opts),
        )
        .map_err(Into::into),

        // --- mermansi structured terminal adapters ---
        RenderSemanticModel::Json(m) => render_json(m, opts),
        RenderSemanticModel::Architecture(m) => {
            append_structured_model(render_architecture(m, opts)?, "architecture", m, opts)
        }
        RenderSemanticModel::C4(m) => append_structured_model(render_c4(m, opts)?, "c4", m, opts),
        RenderSemanticModel::Pie(m) => {
            append_structured_model(render_pie(m, opts)?, "pie", m, opts)
        }
        RenderSemanticModel::Requirement(m) => {
            append_structured_model(render_requirement(m, opts)?, "requirement", m, opts)
        }
        RenderSemanticModel::Sankey(m) => {
            append_structured_model(render_sankey(m, opts)?, "sankey", m, opts)
        }
        RenderSemanticModel::Radar(m) => {
            append_structured_model(render_radar(m, opts)?, "radar", m, opts)
        }
        RenderSemanticModel::Info(m) => {
            append_structured_model(render_info(m, opts)?, "info", m, opts)
        }
        RenderSemanticModel::Treemap(m) => {
            append_structured_model(render_treemap(m, opts)?, "treemap", m, opts)
        }
        RenderSemanticModel::Block(m) => {
            append_structured_model(render_block(m, opts)?, "block", m, opts)
        }
        RenderSemanticModel::QuadrantChart(m) => {
            append_structured_model(render_quadrant_chart(m, opts)?, "quadrantChart", m, opts)
        }
        RenderSemanticModel::Ishikawa(m) => {
            append_structured_model(render_ishikawa(m, opts)?, "ishikawa", m, opts)
        }
        RenderSemanticModel::EventModeling(m) => {
            append_structured_model(render_eventmodeling(m, opts)?, "eventmodeling", m, opts)
        }
        RenderSemanticModel::Venn(m) => {
            append_structured_model(render_venn(m, opts)?, "venn", m, opts)
        }
    }
}

/// Ensure a string is non-empty; if empty, substitute a placeholder.
pub(crate) fn nonempty_or(s: &str, placeholder: &str) -> String {
    if s.trim().is_empty() {
        placeholder.to_string()
    } else {
        s.to_string()
    }
}

/// Format a title block.
pub(crate) fn format_title(title: &Option<String>) -> String {
    match title {
        Some(t) if !t.trim().is_empty() => format!("{t}\n\n"),
        _ => String::new(),
    }
}
