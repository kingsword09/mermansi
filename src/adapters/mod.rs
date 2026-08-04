//! Semantic adapter dispatch.
//!
//! Routes each [`RenderSemanticModel`] variant to the appropriate renderer. Families covered by
//! `merman-ascii` are delegated; remaining families use mermansi's own native terminal
//! adapters.

use crate::adapters::{
    architecture::render_architecture, block::render_block, c4::render_c4, class::render_class,
    er::render_er, eventmodeling::render_eventmodeling, flowchart::render_flowchart,
    gantt::render_gantt, git_graph::render_git_graph, info::render_info, ishikawa::render_ishikawa,
    journey::render_journey, json::render_json, kanban::render_kanban, mindmap::render_mindmap,
    packet::render_packet, pie::render_pie, quadrant_chart::render_quadrant_chart,
    radar::render_radar, requirement::render_requirement, sankey::render_sankey,
    sequence::render_sequence, state::render_state, timeline::render_timeline,
    treemap::render_treemap, treeview::render_treeview, venn::render_venn, xychart::render_xychart,
};
use crate::ansi::sanitize_label_text;
use crate::error::Result;
use crate::options::{Charset, ColorMode, MermansiOptions};
use crate::output::{AdapterOutput, render_structured_adapter};
use crate::str_display_width;
use merman_ascii::{AsciiColorMode, AsciiRenderOptions};
use merman_core::diagram::RenderSemanticModel;

pub mod architecture;
pub mod block;
mod box_geometry;
pub mod c4;
mod chart_primitives;
pub mod class;
pub mod er;
pub mod eventmodeling;
pub mod flowchart;
pub mod gantt;
pub mod git_graph;
pub mod info;
pub mod ishikawa;
pub mod journey;
pub mod json;
pub mod kanban;
pub mod mindmap;
pub mod packet;
pub mod pie;
pub mod quadrant_chart;
pub mod radar;
pub mod requirement;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod timeline;
pub mod treemap;
pub mod treeview;
pub mod venn;
pub mod xychart;

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
    crate::render_model(model, opts)
}

pub(crate) fn render_model_output(
    model: &RenderSemanticModel,
    opts: &MermansiOptions,
) -> Result<AdapterOutput> {
    opts.validate()?;
    match model {
        // --- merman-ascii delegated families ---
        RenderSemanticModel::Flowchart(m) => {
            render_structured_adapter("flowchart", m, opts, || render_flowchart(m, opts))
        }
        RenderSemanticModel::Sequence(m) => {
            render_structured_adapter("sequence", m, opts, || render_sequence(m, opts))
        }
        RenderSemanticModel::State(m) => {
            render_structured_adapter("state", m, opts, || render_state(m, opts))
        }
        RenderSemanticModel::Class(m) => {
            render_structured_adapter("class", m, opts, || render_class(m, opts))
        }
        RenderSemanticModel::Er(m) => {
            render_structured_adapter("er", m, opts, || render_er(m, opts))
        }
        RenderSemanticModel::Packet(m) => {
            render_structured_adapter("packet", m, opts, || render_packet(m, opts))
        }
        RenderSemanticModel::TreeView(m) => {
            render_structured_adapter("treeView", m, opts, || render_treeview(m, opts))
        }
        RenderSemanticModel::XyChart(m) => {
            render_structured_adapter("xychart", m, opts, || render_xychart(m, opts))
        }
        RenderSemanticModel::Mindmap(m) => {
            render_structured_adapter("mindmap", m, opts, || render_mindmap(m, opts))
        }
        RenderSemanticModel::Gantt(m) => {
            render_structured_adapter("gantt", m, opts, || render_gantt(m, opts))
        }
        RenderSemanticModel::GitGraph(m) => {
            render_structured_adapter("gitGraph", m, opts, || render_git_graph(m, opts))
        }
        RenderSemanticModel::Journey(m) => {
            render_structured_adapter("journey", m, opts, || render_journey(m, opts))
        }
        RenderSemanticModel::Kanban(m) => {
            render_structured_adapter("kanban", m, opts, || render_kanban(m, opts))
        }
        RenderSemanticModel::Timeline(m) => {
            render_structured_adapter("timeline", m, opts, || render_timeline(m, opts))
        }

        // --- mermansi native terminal adapters ---
        RenderSemanticModel::Json(m) => {
            render_structured_adapter("json", m, opts, || render_json(m, opts))
        }
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

/// Ensure a string is non-empty; if empty, substitute a placeholder.
pub(crate) fn nonempty_or(s: &str, placeholder: &str) -> String {
    let sanitized = sanitize_label_text(s);
    if sanitized.trim().is_empty() {
        placeholder.to_string()
    } else {
        sanitized
    }
}

pub(crate) fn starts_new_section_run<'a>(previous: &mut Option<&'a str>, section: &'a str) -> bool {
    if section.trim().is_empty() {
        *previous = None;
        return false;
    }
    if *previous == Some(section) {
        return false;
    }
    *previous = Some(section);
    true
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
pub(crate) fn format_title(title: &Option<String>, max_width: usize) -> String {
    let Some(title) = title.as_deref().map(sanitize_label_text) else {
        return String::new();
    };
    if title.trim().is_empty() {
        String::new()
    } else {
        let lines = box_geometry::wrap_display(&title, max_width.max(1));
        format!("{}\n\n", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{format_title, nonempty_or};

    #[test]
    fn shared_chart_titles_are_terminal_safe_before_layout() {
        let title = Some("\u{1b}[31mSales\u{1b}[0m\u{1b}]0;hidden\u{07}\nReport".to_owned());
        assert_eq!(format_title(&title, 80), "SalesReport\n\n");
    }

    #[test]
    fn shared_nonempty_text_is_terminal_safe() {
        assert_eq!(
            nonempty_or("\u{1b}[31mVisible\u{1b}[0m", "fallback"),
            "Visible"
        );
        assert_eq!(nonempty_or("\u{1b}]0;hidden\u{07}", "fallback"), "fallback");
    }

    #[test]
    fn shared_titles_wrap_by_display_column() {
        assert_eq!(
            format_title(&Some("中文标题".to_owned()), 4),
            "中文\n标题\n\n"
        );
    }
}
