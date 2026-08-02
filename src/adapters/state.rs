//! State terminal geometry backed by `merman-ascii` with closed pseudo-state borders.

use crate::adapters::to_ascii_options;
use crate::ansi::strip_ansi;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::state::StateDiagramRenderModel;

pub fn render_state(model: &StateDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let output = merman_ascii::render_state(model, &to_ascii_options(opts))?;
    if opts.charset == Charset::Ascii {
        return Ok(output);
    }

    Ok(close_pseudostate_borders(output))
}

fn close_pseudostate_borders(output: String) -> String {
    let has_trailing_newline = output.ends_with('\n');
    let mut lines = output.lines().map(str::to_owned).collect::<Vec<_>>();
    let pseudo_rows = lines
        .iter()
        .enumerate()
        .filter_map(|(row, line)| {
            let visible = strip_ansi(line);
            (visible.contains('●') || visible.contains('◎')).then_some(row)
        })
        .collect::<Vec<_>>();

    for row in pseudo_rows {
        if row == 0 || row + 1 >= lines.len() {
            continue;
        }
        let top = strip_ansi(&lines[row - 1]);
        let bottom = strip_ansi(&lines[row + 1]);
        if top.contains('╭') && top.contains('╮') && bottom.contains('╰') && bottom.contains('╯')
        {
            lines[row - 1] = lines[row - 1].replace('╭', "┌").replace('╮', "┐");
            lines[row + 1] = lines[row + 1].replace('╰', "└").replace('╯', "┘");
        }
    }

    let mut closed = lines.join("\n");
    if has_trailing_newline {
        closed.push('\n');
    }
    closed
}
