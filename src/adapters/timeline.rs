//! Timeline terminal geometry.

use std::collections::HashSet;

use crate::adapters::box_geometry::{
    self, BoxDiagram, BoxDirection, BoxLayout, BoxNode, directed_chain_edges,
};
use crate::adapters::{nonempty_or, starts_new_section_run};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};

const MAX_TIMELINE_NODES: usize = 4_096;

pub fn render_timeline(
    model: &TimelineDiagramRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let base_requested = model.sections.len().saturating_add(model.tasks.len());
    ensure_node_limit(base_requested)?;
    let base_requested =
        base_requested.saturating_add(model.tasks.iter().fold(0usize, |count, task| {
            count.saturating_add(task.events.len())
        }));
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
    let mut emitted_sections = HashSet::<&str>::new();
    for section in &model.sections {
        push_section(&mut items, section);
        if emitted_sections.insert(section.as_str()) {
            for task in model.tasks.iter().filter(|task| task.section == *section) {
                push_period(&mut items, task);
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
        push_period(&mut items, task);
    }

    render_ordered_timeline(
        items,
        nonempty_or(
            model.title.as_deref().unwrap_or_default().trim(),
            "Timeline",
        ),
        opts,
    )
}

fn ensure_node_limit(requested: usize) -> Result<()> {
    if requested > MAX_TIMELINE_NODES {
        return Err(MermansiError::RenderLimit {
            context: "timeline nodes",
            requested,
            limit: MAX_TIMELINE_NODES,
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

fn push_period(items: &mut Vec<Vec<String>>, task: &TimelineRenderTask) {
    let mut lines = vec![format!(
        "[Period] {}",
        nonempty_or(task.task.trim(), "(unnamed)")
    )];
    if !task.task_type.trim().is_empty() && task.task_type != task.section {
        lines.push(format!("type: {}", task.task_type.trim()));
    }
    if task.score != 0 {
        lines.push(format!("score: {}", task.score));
    }
    items.push(lines);
    for event in &task.events {
        items.push(vec![format!(
            "[Event] {}",
            nonempty_or(event.trim(), "(unnamed)")
        )]);
    }
}

fn render_ordered_timeline(
    items: Vec<Vec<String>>,
    title: String,
    opts: &MermansiOptions,
) -> Result<String> {
    if items.is_empty() {
        return Ok(format!("{title}\n\n(empty timeline)\n"));
    }
    let direction = BoxDirection::Lr;
    let nodes = items
        .into_iter()
        .enumerate()
        .map(|(order, lines)| BoxNode {
            id: format!("timeline-{order}"),
            lines,
            dividers: Vec::new(),
            parent: None,
            span: 1,
            order,
        })
        .collect::<Vec<_>>();
    let edges = directed_chain_edges("timeline", nodes.len(), direction);
    box_geometry::render(
        &BoxDiagram {
            family: "timeline",
            title: Some(title),
            nodes,
            groups: Vec::new(),
            spacers: Vec::new(),
            edges,
            columns: None,
            layout: BoxLayout::Packed,
            edge_legend: box_geometry::EdgeLegend::None,
        },
        opts,
    )
}
