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
use crate::output::render_structured_adapter;
use crate::str_display_width;
use merman_ascii::{AsciiColorMode, AsciiRenderOptions};
use merman_core::diagram::RenderSemanticModel;
use serde::Serialize;

pub mod architecture;
pub mod block;
mod box_geometry;
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
        RenderSemanticModel::Flowchart(m) => {
            render_structured_adapter("flowchart", m, opts, || render_flowchart(m, opts))
        }
        RenderSemanticModel::Sequence(m) => {
            render_ascii("sequence", m, opts, merman_ascii::render_sequence)
        }
        RenderSemanticModel::State(m) => render_ascii("state", m, opts, merman_ascii::render_state),
        RenderSemanticModel::Class(m) => render_ascii("class", m, opts, merman_ascii::render_class),
        RenderSemanticModel::Er(m) => render_ascii("er", m, opts, merman_ascii::render_er),
        RenderSemanticModel::Packet(m) => {
            render_ascii("packet", m, opts, merman_ascii::render_packet)
        }
        RenderSemanticModel::TreeView(m) => {
            render_ascii("treeView", m, opts, merman_ascii::render_tree_view)
        }
        RenderSemanticModel::XyChart(m) => {
            render_ascii("xychart", m, opts, merman_ascii::render_xychart)
        }
        RenderSemanticModel::Mindmap(m) => {
            render_ascii("mindmap", m, opts, merman_ascii::render_mindmap)
        }
        RenderSemanticModel::Gantt(m) => render_ascii("gantt", m, opts, merman_ascii::render_gantt),
        RenderSemanticModel::GitGraph(m) => {
            render_ascii("gitGraph", m, opts, merman_ascii::render_git_graph)
        }
        RenderSemanticModel::Journey(m) => {
            render_ascii("journey", m, opts, merman_ascii::render_journey)
        }
        RenderSemanticModel::Kanban(m) => {
            render_ascii("kanban", m, opts, merman_ascii::render_kanban)
        }
        RenderSemanticModel::Timeline(m) => {
            render_ascii("timeline", m, opts, merman_ascii::render_timeline)
        }

        // --- mermansi structured terminal adapters ---
        RenderSemanticModel::Json(m) => render_json(m, opts),
        RenderSemanticModel::Architecture(m) => {
            render_structured_adapter("architecture", m, opts, || render_architecture(m, opts))
        }
        RenderSemanticModel::C4(m) => {
            render_structured_adapter("c4", m, opts, || render_c4(m, opts))
        }
        RenderSemanticModel::Pie(m) => {
            render_structured_adapter("pie", m, opts, || render_pie(m, opts))
        }
        RenderSemanticModel::Requirement(m) => {
            render_structured_adapter("requirement", m, opts, || render_requirement(m, opts))
        }
        RenderSemanticModel::Sankey(m) => {
            render_structured_adapter("sankey", m, opts, || render_sankey(m, opts))
        }
        RenderSemanticModel::Radar(m) => {
            render_structured_adapter("radar", m, opts, || render_radar(m, opts))
        }
        RenderSemanticModel::Info(m) => {
            render_structured_adapter("info", m, opts, || render_info(m, opts))
        }
        RenderSemanticModel::Treemap(m) => {
            render_structured_adapter("treemap", m, opts, || render_treemap(m, opts))
        }
        RenderSemanticModel::Block(m) => {
            render_structured_adapter("block", m, opts, || render_block(m, opts))
        }
        RenderSemanticModel::QuadrantChart(m) => {
            render_structured_adapter("quadrantChart", m, opts, || render_quadrant_chart(m, opts))
        }
        RenderSemanticModel::Ishikawa(m) => {
            render_structured_adapter("ishikawa", m, opts, || render_ishikawa(m, opts))
        }
        RenderSemanticModel::EventModeling(m) => {
            render_structured_adapter("eventmodeling", m, opts, || render_eventmodeling(m, opts))
        }
        RenderSemanticModel::Venn(m) => {
            render_structured_adapter("venn", m, opts, || render_venn(m, opts))
        }
    }
}

fn render_ascii<T: Serialize>(
    family: &str,
    model: &T,
    opts: &MermansiOptions,
    renderer: fn(&T, &AsciiRenderOptions) -> merman_ascii::Result<String>,
) -> Result<String> {
    render_structured_adapter(family, model, opts, || {
        renderer(model, &to_ascii_options(opts)).map_err(Into::into)
    })
}

/// Ensure a string is non-empty; if empty, substitute a placeholder.
pub(crate) fn nonempty_or(s: &str, placeholder: &str) -> String {
    if s.trim().is_empty() {
        placeholder.to_string()
    } else {
        s.to_string()
    }
}

pub(crate) fn align_left_display(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(str_display_width(text));
    format!("{text}{}", " ".repeat(padding))
}

pub(crate) fn align_right_display(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(str_display_width(text));
    format!("{}{text}", " ".repeat(padding))
}

pub(crate) const fn detail_separator(charset: Charset) -> &'static str {
    match charset {
        Charset::Unicode => " · ",
        Charset::Ascii => " - ",
    }
}

/// Format a title block.
pub(crate) fn format_title(title: &Option<String>) -> String {
    match title {
        Some(t) if !t.trim().is_empty() => format!("{t}\n\n"),
        _ => String::new(),
    }
}
