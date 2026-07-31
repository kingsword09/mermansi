//! User journey terminal geometry.

use std::collections::HashSet;

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxLayout, BoxNode, directed_chain_edges, directed_ranks,
};
use crate::adapters::{nonempty_or, starts_new_section_run};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};

const MAX_JOURNEY_NODES: usize = 4_096;

pub fn render_journey(model: &JourneyDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let base_requested = model
        .sections
        .len()
        .saturating_add(model.tasks.len())
        .saturating_add(usize::from(!model.actors.is_empty()));
    ensure_node_limit(base_requested)?;

    let known_sections = model
        .sections
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut previous_orphan_section = None::<&str>;
    let mut orphan_sections = 0usize;
    for task in &model.tasks {
        if known_sections.contains(task.section.as_str()) {
            continue;
        }
        let section = task.section.as_str();
        if starts_new_section_run(&mut previous_orphan_section, section) {
            orphan_sections = orphan_sections.saturating_add(1);
        }
    }
    let requested = base_requested.saturating_add(orphan_sections);
    ensure_node_limit(requested)?;

    let mut items = Vec::with_capacity(requested);
    if !model.actors.is_empty() {
        let actors = model
            .actors
            .iter()
            .map(|actor| nonempty_or(actor.trim(), "(unnamed)"))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(vec!["[Actors]".to_owned(), actors]);
    }

    let mut emitted_sections = HashSet::<&str>::new();
    for section in &model.sections {
        push_section(&mut items, section);
        if emitted_sections.insert(section.as_str()) {
            for task in model.tasks.iter().filter(|task| task.section == *section) {
                items.push(task_lines(task, opts.charset));
            }
        }
    }

    let mut previous_orphan_section = None::<&str>;
    for task in &model.tasks {
        if known_sections.contains(task.section.as_str()) {
            continue;
        }
        let section = task.section.as_str();
        if starts_new_section_run(&mut previous_orphan_section, section) {
            push_section(&mut items, section);
        }
        items.push(task_lines(task, opts.charset));
    }

    render_ordered_journey(
        items,
        nonempty_or(model.title.as_deref().unwrap_or_default().trim(), "Journey"),
        opts,
    )
}

fn ensure_node_limit(requested: usize) -> Result<()> {
    if requested > MAX_JOURNEY_NODES {
        return Err(MermansiError::RenderLimit {
            context: "journey nodes",
            requested,
            limit: MAX_JOURNEY_NODES,
        });
    }
    Ok(())
}

fn push_section(items: &mut Vec<Vec<String>>, section: &str) {
    items.push(vec![format!(
        "[Section] {}",
        nonempty_or(section.trim(), "(unnamed)")
    )]);
}

fn task_lines(task: &JourneyRenderTask, charset: Charset) -> Vec<String> {
    let mut lines = vec![format!(
        "[Task] {}",
        nonempty_or(task.task.trim(), "(unnamed)")
    )];
    let score = if task.score_is_nan {
        "NaN".to_owned()
    } else {
        task.score.to_string()
    };
    lines.push(format!("score: {score}/5 {}", score_bar(task, charset)));
    let actors = task
        .people
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|actor| !actor.is_empty())
        .collect::<Vec<_>>();
    if !actors.is_empty() {
        lines.push(format!("actors: {}", actors.join(", ")));
    }
    if !task.task_type.trim().is_empty() && task.task_type != task.section {
        lines.push(format!("type: {}", task.task_type.trim()));
    }
    lines
}

fn score_bar(task: &JourneyRenderTask, charset: Charset) -> String {
    if task.score_is_nan {
        return "?????".to_owned();
    }
    let filled = usize::try_from(task.score.clamp(0, 5)).unwrap_or_default();
    let (fill, empty) = match charset {
        Charset::Unicode => ('█', '░'),
        Charset::Ascii => ('#', '-'),
    };
    std::iter::repeat_n(fill, filled)
        .chain(std::iter::repeat_n(empty, 5 - filled))
        .collect()
}

fn render_ordered_journey(
    items: Vec<Vec<String>>,
    title: String,
    opts: &MermansiOptions,
) -> Result<String> {
    if items.is_empty() {
        return Ok(format!("{title}\n\n(empty journey)\n"));
    }
    let direction = BoxDirection::Tb;
    let nodes = items
        .into_iter()
        .enumerate()
        .map(|(order, lines)| BoxNode {
            id: format!("journey-{order}"),
            lines,
            parent: None,
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let edges = directed_chain_edges("journey", nodes.len(), direction);
    let ranks = directed_ranks(&nodes, &edges);
    box_geometry::render(
        &BoxDiagram {
            family: "journey",
            title: Some(title),
            nodes,
            groups: Vec::new(),
            spacers: Vec::new(),
            edges,
            columns: None,
            layout: BoxLayout::Layered { direction, ranks },
            show_edge_legend: false,
        },
        opts,
    )
}
