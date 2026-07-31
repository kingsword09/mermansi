//! GitGraph terminal geometry.
//!
//! Branches are stable lanes, commits are typed markers, and every parsed parent relationship is
//! painted as a connected orthogonal route. Numbered details preserve metadata without crowding
//! the graph surface.

use std::collections::HashMap;

use crate::adapters::box_geometry::wrap_words;
use crate::adapters::chart_primitives::render_cropped_canvas;
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line, draw_vertical_line};
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
use merman_core::diagrams::git_graph::{GitGraphCommitRenderModel, GitGraphRenderModel};

const MAX_GIT_COMMITS: usize = 4_096;
const MAX_GIT_BRANCHES: usize = 4_096;
const COMMIT_GAP: usize = 6;
const LANE_GAP: usize = 3;

#[derive(Clone, Copy)]
struct Position {
    x: usize,
    y: usize,
}

pub fn render_git_graph(model: &GitGraphRenderModel, opts: &MermansiOptions) -> Result<String> {
    ensure_limit("gitGraph commits", model.commits.len(), MAX_GIT_COMMITS)?;
    let branches = branch_inventory(model);
    ensure_limit("gitGraph branches", branches.len(), MAX_GIT_BRANCHES)?;
    if model.commits.is_empty() {
        return render_empty_graph(model, opts);
    }

    let mut commits = model.commits.iter().enumerate().collect::<Vec<_>>();
    commits.sort_by_key(|(index, commit)| (commit.seq, *index));
    let branch_indices = branches
        .iter()
        .enumerate()
        .map(|(index, branch)| (branch.as_str(), index))
        .collect::<HashMap<_, _>>();
    let id_positions = commit_id_positions(&commits);
    let detail_lines = commit_details(&commits, opts.max_width.saturating_sub(2).max(1));
    let direction = normalized_direction(&model.direction);
    if matches!(direction, "TB" | "BT") {
        render_vertical(
            model,
            opts,
            &branches,
            &branch_indices,
            &commits,
            &id_positions,
            &detail_lines,
            direction == "BT",
        )
    } else {
        render_horizontal(
            model,
            opts,
            &branches,
            &branch_indices,
            &commits,
            &id_positions,
            &detail_lines,
            direction == "RL",
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn render_horizontal(
    model: &GitGraphRenderModel,
    opts: &MermansiOptions,
    branches: &[String],
    branch_indices: &HashMap<&str, usize>,
    commits: &[(usize, &GitGraphCommitRenderModel)],
    id_positions: &HashMap<&str, Vec<usize>>,
    detail_lines: &[String],
    reverse: bool,
) -> Result<String> {
    let desired_label_width = branches
        .iter()
        .map(|branch| str_display_width(branch))
        .max()
        .unwrap_or(4)
        .saturating_add(2)
        .min(24);
    let graph_width = commits
        .len()
        .saturating_sub(1)
        .saturating_mul(COMMIT_GAP)
        .saturating_add(3);
    let required_width = desired_label_width
        .saturating_add(1)
        .saturating_add(graph_width);
    if required_width > opts.max_width {
        return Err(MermansiError::RenderLimit {
            context: "gitGraph columns",
            requested: required_width,
            limit: opts.max_width,
        });
    }
    let width = required_width.max(28);
    if width > opts.max_width {
        return Err(MermansiError::RenderLimit {
            context: "gitGraph columns",
            requested: width,
            limit: opts.max_width,
        });
    }
    let graph_x = desired_label_width + 1;
    let detail_lines = detail_lines
        .iter()
        .flat_map(|line| wrap_words(line, width.saturating_sub(2)))
        .collect::<Vec<_>>();
    let graph_y = 4usize;
    let lane_height = branches
        .len()
        .saturating_sub(1)
        .saturating_mul(LANE_GAP)
        .saturating_add(2);
    let detail_y = graph_y + lane_height + 1;
    let height = detail_y.saturating_add(detail_lines.len()).max(8);
    ensure_height(opts, height)?;
    let mut canvas = Canvas::new(width, height)?;
    paint_header(&mut canvas, model, width)?;

    let mut positions = Vec::with_capacity(commits.len());
    for (commit_index, (_, commit)) in commits.iter().enumerate() {
        let lane = lookup_branch_index(branch_indices, &commit.branch);
        let axis_index = if reverse {
            commits.len() - 1 - commit_index
        } else {
            commit_index
        };
        positions.push(Position {
            x: graph_x + 1 + axis_index * COMMIT_GAP,
            y: graph_y + lane * LANE_GAP,
        });
    }
    paint_horizontal_lanes(
        &mut canvas,
        model,
        branches,
        branch_indices,
        commits,
        &positions,
        graph_x,
        graph_width,
        graph_y,
        opts.charset,
    )?;
    for (child_index, (_, commit)) in commits.iter().enumerate() {
        for parent in &commit.parents {
            let parent_index = resolve_parent(id_positions, parent, child_index)?;
            draw_horizontal_parent(
                &mut canvas,
                positions[parent_index],
                positions[child_index],
                opts.charset,
            )?;
        }
    }
    paint_commits(&mut canvas, commits, &positions, opts.charset, false)?;
    for (offset, line) in detail_lines.iter().enumerate() {
        canvas.set_text(1, detail_y + offset, line)?;
    }
    Ok(render_cropped_canvas(&canvas))
}

#[allow(clippy::too_many_arguments)]
fn render_vertical(
    model: &GitGraphRenderModel,
    opts: &MermansiOptions,
    branches: &[String],
    branch_indices: &HashMap<&str, usize>,
    commits: &[(usize, &GitGraphCommitRenderModel)],
    id_positions: &HashMap<&str, Vec<usize>>,
    detail_lines: &[String],
    reverse: bool,
) -> Result<String> {
    let branch_width = branches
        .iter()
        .map(|branch| str_display_width(branch))
        .max()
        .unwrap_or(4)
        .saturating_add(3)
        .clamp(8, 20);
    let graph_x = 2usize;
    let graph_width = branches
        .len()
        .saturating_sub(1)
        .saturating_mul(branch_width)
        .saturating_add(3);
    let width = graph_x.saturating_add(graph_width).max(28);
    if width > opts.max_width {
        return Err(MermansiError::RenderLimit {
            context: "gitGraph columns",
            requested: width,
            limit: opts.max_width,
        });
    }
    let graph_y = 5usize;
    let detail_lines = detail_lines
        .iter()
        .flat_map(|line| wrap_words(line, width.saturating_sub(2)))
        .collect::<Vec<_>>();
    let graph_height = commits
        .len()
        .saturating_sub(1)
        .saturating_mul(LANE_GAP)
        .saturating_add(2);
    let detail_y = graph_y + graph_height + 1;
    let height = detail_y.saturating_add(detail_lines.len()).max(9);
    ensure_height(opts, height)?;
    let mut canvas = Canvas::new(width, height)?;
    paint_header(&mut canvas, model, width)?;

    for (branch_index, branch) in branches.iter().enumerate() {
        let x = graph_x + 1 + branch_index * branch_width;
        let label = if str_display_width(branch) < branch_width {
            branch.clone()
        } else {
            format!("[b{}]", branch_index + 1)
        };
        let prefix = if is_current_branch(branch, &model.current_branch) {
            "*"
        } else {
            " "
        };
        let visible = format!("{prefix}{label}");
        let start = x.saturating_sub(str_display_width(&visible) / 2);
        canvas.set_text(start, 3, &visible)?;
        draw_vertical_line(
            &mut canvas,
            x,
            graph_y,
            graph_y + graph_height - 1,
            opts.charset,
        )?;
    }

    let mut positions = Vec::with_capacity(commits.len());
    for (commit_index, (_, commit)) in commits.iter().enumerate() {
        let lane = lookup_branch_index(branch_indices, &commit.branch);
        let axis_index = if reverse {
            commits.len() - 1 - commit_index
        } else {
            commit_index
        };
        positions.push(Position {
            x: graph_x + 1 + lane * branch_width,
            y: graph_y + axis_index * LANE_GAP,
        });
    }
    for (child_index, (_, commit)) in commits.iter().enumerate() {
        for parent in &commit.parents {
            let parent_index = resolve_parent(id_positions, parent, child_index)?;
            draw_vertical_parent(
                &mut canvas,
                positions[parent_index],
                positions[child_index],
                opts.charset,
            )?;
        }
    }
    paint_commits(&mut canvas, commits, &positions, opts.charset, true)?;
    for (offset, line) in detail_lines.iter().enumerate() {
        canvas.set_text(1, detail_y + offset, line)?;
    }
    Ok(render_cropped_canvas(&canvas))
}

#[allow(clippy::too_many_arguments)]
fn paint_horizontal_lanes(
    canvas: &mut Canvas,
    model: &GitGraphRenderModel,
    branches: &[String],
    branch_indices: &HashMap<&str, usize>,
    commits: &[(usize, &GitGraphCommitRenderModel)],
    positions: &[Position],
    graph_x: usize,
    graph_width: usize,
    graph_y: usize,
    charset: Charset,
) -> Result<()> {
    for (branch_index, branch) in branches.iter().enumerate() {
        let lane_y = graph_y + branch_index * LANE_GAP;
        let label = if str_display_width(branch) + 2 < graph_x {
            branch.clone()
        } else {
            format!("[b{}]", branch_index + 1)
        };
        let prefix = if is_current_branch(branch, &model.current_branch) {
            "* "
        } else {
            "  "
        };
        canvas.set_text(0, lane_y, &format!("{prefix}{label}"))?;
        let lane_commits = commits
            .iter()
            .enumerate()
            .filter(|(_, (_, commit))| {
                lookup_branch_index(branch_indices, &commit.branch) == branch_index
            })
            .map(|(index, _)| positions[index].x)
            .collect::<Vec<_>>();
        let start = lane_commits.iter().copied().min().unwrap_or(graph_x + 1);
        let end = lane_commits
            .iter()
            .copied()
            .max()
            .unwrap_or((graph_x + graph_width - 2).min(canvas.width() - 1));
        draw_horizontal_line(canvas, lane_y, start, end.max(start), charset)?;
    }
    Ok(())
}

fn draw_horizontal_parent(
    canvas: &mut Canvas,
    parent: Position,
    child: Position,
    charset: Charset,
) -> Result<()> {
    if parent.y == child.y {
        draw_horizontal_line(
            canvas,
            parent.y,
            parent.x.min(child.x),
            parent.x.max(child.x),
            charset,
        )?;
    } else {
        let bend = if parent.x <= child.x {
            child.x.saturating_sub(2).max(parent.x)
        } else {
            child.x.saturating_add(2).min(parent.x)
        };
        draw_horizontal_line(
            canvas,
            parent.y,
            parent.x.min(bend),
            parent.x.max(bend),
            charset,
        )?;
        draw_vertical_line(
            canvas,
            bend,
            parent.y.min(child.y),
            parent.y.max(child.y),
            charset,
        )?;
        draw_horizontal_line(
            canvas,
            child.y,
            bend.min(child.x),
            bend.max(child.x),
            charset,
        )?;
    }
    let marker_x = if parent.x <= child.x {
        child.x.saturating_sub(1)
    } else {
        child.x.saturating_add(1)
    };
    canvas.set_char(
        marker_x,
        child.y,
        if charset == Charset::Unicode {
            if parent.x <= child.x { '▶' } else { '◀' }
        } else if parent.x <= child.x {
            '>'
        } else {
            '<'
        },
    )
}

fn draw_vertical_parent(
    canvas: &mut Canvas,
    parent: Position,
    child: Position,
    charset: Charset,
) -> Result<()> {
    if parent.x == child.x {
        draw_vertical_line(
            canvas,
            parent.x,
            parent.y.min(child.y),
            parent.y.max(child.y),
            charset,
        )?;
    } else {
        let bend = if parent.y <= child.y {
            child.y.saturating_sub(1).max(parent.y)
        } else {
            child.y.saturating_add(1).min(parent.y)
        };
        draw_vertical_line(
            canvas,
            parent.x,
            parent.y.min(bend),
            parent.y.max(bend),
            charset,
        )?;
        draw_horizontal_line(
            canvas,
            bend,
            parent.x.min(child.x),
            parent.x.max(child.x),
            charset,
        )?;
        draw_vertical_line(
            canvas,
            child.x,
            bend.min(child.y),
            bend.max(child.y),
            charset,
        )?;
    }
    let marker_y = if parent.y <= child.y {
        child.y.saturating_sub(1)
    } else {
        child.y.saturating_add(1)
    };
    canvas.set_char(
        child.x,
        marker_y,
        if charset == Charset::Unicode {
            if parent.y <= child.y { '▼' } else { '▲' }
        } else if parent.y <= child.y {
            'v'
        } else {
            '^'
        },
    )
}

fn paint_commits(
    canvas: &mut Canvas,
    commits: &[(usize, &GitGraphCommitRenderModel)],
    positions: &[Position],
    charset: Charset,
    vertical: bool,
) -> Result<()> {
    for (index, ((_, commit), position)) in commits.iter().zip(positions).enumerate() {
        canvas.set_char(
            position.x,
            position.y,
            commit_marker(commit.commit_type, charset),
        )?;
        let number = (index + 1).to_string();
        if vertical {
            canvas.set_text(position.x + 1, position.y, &number)?;
        } else {
            let x = position.x.saturating_sub(str_display_width(&number) / 2);
            canvas.set_text(x, position.y + 1, &number)?;
        }
    }
    Ok(())
}

fn commit_details(commits: &[(usize, &GitGraphCommitRenderModel)], width: usize) -> Vec<String> {
    commits
        .iter()
        .enumerate()
        .flat_map(|(index, (_, commit))| {
            let mut parts = vec![
                format!("[{}] {}", index + 1, nonempty(&commit.id)),
                format!("branch={}", nonempty(&commit.branch)),
                format!("type={}", commit_kind(commit.commit_type)),
            ];
            if !commit.parents.is_empty() {
                parts.push(format!("parents={}", commit.parents.join(",")));
            }
            if !commit.message.trim().is_empty() {
                parts.push(format!("message={}", sanitize_label_text(&commit.message)));
            }
            if !commit.tags.is_empty() {
                parts.push(format!("tags={}", commit.tags.join(",")));
            }
            if let Some(custom_type) = commit.custom_type {
                parts.push(format!("customType={custom_type}"));
            }
            if let Some(custom_id) = commit.custom_id {
                parts.push(format!("customId={custom_id}"));
            }
            wrap_words(&parts.join("  "), width)
        })
        .collect()
}

fn branch_inventory(model: &GitGraphRenderModel) -> Vec<String> {
    let mut branches = Vec::new();
    for branch in &model.branches {
        let name = sanitize_label_text(&branch.name);
        if !branches.contains(&name) {
            branches.push(name);
        }
    }
    for commit in &model.commits {
        let branch = sanitize_label_text(&commit.branch);
        if !branches.contains(&branch) {
            branches.push(branch);
        }
    }
    if branches.is_empty() {
        branches.push("main".to_owned());
    }
    branches
}

fn lookup_branch_index(indices: &HashMap<&str, usize>, branch: &str) -> usize {
    let branch = sanitize_label_text(branch);
    indices.get(branch.as_str()).copied().unwrap_or_default()
}

fn is_current_branch(branch: &str, current: &str) -> bool {
    branch == sanitize_label_text(current)
}

fn commit_id_positions<'a>(
    commits: &[(usize, &'a GitGraphCommitRenderModel)],
) -> HashMap<&'a str, Vec<usize>> {
    let mut positions = HashMap::<&str, Vec<usize>>::new();
    for (index, (_, commit)) in commits.iter().enumerate() {
        positions.entry(commit.id.as_str()).or_default().push(index);
    }
    positions
}

fn resolve_parent(
    positions: &HashMap<&str, Vec<usize>>,
    parent: &str,
    child_index: usize,
) -> Result<usize> {
    positions
        .get(parent)
        .and_then(|positions| {
            positions
                .iter()
                .rev()
                .copied()
                .find(|position| *position < child_index)
                .or_else(|| positions.first().copied())
        })
        .ok_or_else(|| MermansiError::GeometryLayout {
            family: "gitGraph",
            message: format!("parent commit is missing: {}", sanitize_label_text(parent)),
        })
}

fn paint_header(canvas: &mut Canvas, model: &GitGraphRenderModel, width: usize) -> Result<()> {
    let title = model
        .acc_title
        .as_deref()
        .map(sanitize_label_text)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "Git graph".to_owned());
    let x = width.saturating_sub(str_display_width(&title)) / 2;
    canvas.set_text(x, 0, &title)?;
    let summary = format!(
        "direction={}  current={}",
        normalized_direction(&model.direction),
        nonempty(&model.current_branch)
    );
    let x = width.saturating_sub(str_display_width(&summary)) / 2;
    canvas.set_text(x, 1, &summary)
}

fn render_empty_graph(model: &GitGraphRenderModel, opts: &MermansiOptions) -> Result<String> {
    let width = opts.max_width.min(30);
    if width < 16 || opts.max_height < 3 {
        return Err(MermansiError::RenderLimit {
            context: "gitGraph empty card",
            requested: 16,
            limit: width.min(opts.max_height),
        });
    }
    let mut canvas = Canvas::new(width, 3)?;
    draw_box(&mut canvas, 0, 0, width, 3, opts.charset)?;
    let label = format!("Git graph: {}", nonempty(&model.current_branch));
    let x = width.saturating_sub(str_display_width(&label)) / 2;
    canvas.set_text(x, 1, &label)?;
    Ok(render_cropped_canvas(&canvas))
}

fn normalized_direction(direction: &str) -> &'static str {
    match direction.to_ascii_uppercase().as_str() {
        "TB" => "TB",
        "BT" => "BT",
        "RL" => "RL",
        _ => "LR",
    }
}

fn commit_marker(kind: i64, charset: Charset) -> char {
    match (kind, charset) {
        (1, Charset::Unicode) => '○',
        (2, Charset::Unicode) => '★',
        (3, Charset::Unicode) => '◆',
        (4, Charset::Unicode) => '◇',
        (_, Charset::Unicode) => '●',
        (1, Charset::Ascii) => 'r',
        (2, Charset::Ascii) => '*',
        (3, Charset::Ascii) => 'M',
        (4, Charset::Ascii) => 'C',
        (_, Charset::Ascii) => 'o',
    }
}

fn commit_kind(kind: i64) -> &'static str {
    match kind {
        1 => "reverse",
        2 => "highlight",
        3 => "merge",
        4 => "cherry-pick",
        _ => "normal",
    }
}

fn ensure_height(opts: &MermansiOptions, requested: usize) -> Result<()> {
    if requested > opts.max_height {
        return Err(MermansiError::RenderLimit {
            context: "gitGraph rows",
            requested,
            limit: opts.max_height,
        });
    }
    Ok(())
}

fn ensure_limit(context: &'static str, requested: usize, limit: usize) -> Result<()> {
    if requested > limit {
        return Err(MermansiError::RenderLimit {
            context,
            requested,
            limit,
        });
    }
    Ok(())
}

fn nonempty(value: &str) -> String {
    let value = sanitize_label_text(value).trim().to_owned();
    if value.is_empty() {
        "(unnamed)".to_owned()
    } else {
        value
    }
}
