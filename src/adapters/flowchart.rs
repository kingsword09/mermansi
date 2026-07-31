//! Flowchart geometry preview with a lossless structured fallback.

use crate::adapters::to_ascii_options;
use crate::char_display_width;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_ascii::AsciiError;
use merman_core::diagrams::flowchart::FlowchartV2Model;

mod lanes;

pub fn render_flowchart(model: &FlowchartV2Model, opts: &MermansiOptions) -> Result<String> {
    if lanes::requires_lane_geometry(model)
        && let Some(output) = lanes::render_lane_geometry(model, opts)?
    {
        return Ok(output);
    }

    match merman_ascii::render_flowchart(model, &to_ascii_options(opts)) {
        Ok(output) => Ok(normalize_decision_borders(output, model, opts.charset)),
        Err(AsciiError::UnsupportedFeature { .. }) => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn normalize_decision_borders(
    output: String,
    model: &FlowchartV2Model,
    charset: Charset,
) -> String {
    if !model.nodes.iter().any(|node| {
        matches!(
            node.layout_shape.as_deref(),
            Some("diamond" | "question" | "diam" | "decision")
        )
    }) {
        return output;
    }

    let symbols = DecisionBorderSymbols::new(charset);
    let had_trailing_newline = output.ends_with('\n');
    let mut lines = output.lines().map(str::to_owned).collect::<Vec<_>>();
    let visible = lines
        .iter()
        .map(|line| visible_chars(line))
        .collect::<Vec<_>>();
    let spans = decision_border_spans(&visible, symbols);
    let mut replacements = vec![Vec::new(); lines.len()];

    for span in spans {
        for row in (span.top + 1)..span.bottom {
            for column in [span.left, span.right] {
                let Some(cell) = visible_char_at(&visible[row], column) else {
                    continue;
                };
                if cell.value == ' ' || symbols.is_duplicate_corner(cell.value) {
                    replacements[row].push((cell.byte_start, cell.byte_end, symbols.vertical));
                }
            }
        }
    }

    for (line, mut line_replacements) in lines.iter_mut().zip(replacements) {
        line_replacements.sort_unstable_by_key(|replacement| replacement.0);
        line_replacements.dedup_by_key(|replacement| replacement.0);
        for (start, end, replacement) in line_replacements.into_iter().rev() {
            line.replace_range(start..end, &replacement.to_string());
        }
    }

    let mut normalized = lines.join("\n");
    if had_trailing_newline {
        normalized.push('\n');
    }
    normalized
}

#[derive(Clone, Copy)]
struct DecisionBorderSymbols {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    vertical: char,
}

impl DecisionBorderSymbols {
    const fn new(charset: Charset) -> Self {
        match charset {
            Charset::Unicode => Self {
                top_left: '╭',
                top_right: '╮',
                bottom_left: '╰',
                bottom_right: '╯',
                vertical: '│',
            },
            Charset::Ascii => Self {
                top_left: '/',
                top_right: '\\',
                bottom_left: '\\',
                bottom_right: '/',
                vertical: '|',
            },
        }
    }

    fn is_duplicate_corner(self, value: char) -> bool {
        value == self.top_left
            || value == self.top_right
            || value == self.bottom_left
            || value == self.bottom_right
    }
}

#[derive(Clone, Copy)]
struct DecisionBorderSpan {
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
}

fn decision_border_spans(
    lines: &[Vec<VisibleChar>],
    symbols: DecisionBorderSymbols,
) -> Vec<DecisionBorderSpan> {
    let mut spans = Vec::new();
    for top in 0..lines.len().saturating_sub(1) {
        for left in lines[top]
            .iter()
            .filter(|cell| cell.value == symbols.top_left)
        {
            if visible_char_at(&lines[top + 1], left.column).map(|cell| cell.value)
                != Some(symbols.top_left)
            {
                continue;
            }

            let right = lines[top].iter().find(|cell| {
                cell.column > left.column
                    && cell.value == symbols.top_right
                    && visible_char_at(&lines[top + 1], cell.column)
                        .is_some_and(|next| next.value == symbols.top_right)
                    && is_blank_between(&lines[top + 1], left.column, cell.column)
            });
            let Some(right) = right else {
                continue;
            };

            let bottom = ((top + 2)..lines.len().saturating_sub(1)).find(|row| {
                visible_char_at(&lines[*row], left.column)
                    .is_some_and(|cell| cell.value == symbols.bottom_left)
                    && visible_char_at(&lines[*row], right.column)
                        .is_some_and(|cell| cell.value == symbols.bottom_right)
                    && visible_char_at(&lines[*row + 1], left.column)
                        .is_some_and(|cell| cell.value == symbols.bottom_left)
                    && visible_char_at(&lines[*row + 1], right.column)
                        .is_some_and(|cell| cell.value == symbols.bottom_right)
                    && is_blank_between(&lines[*row], left.column, right.column)
            });
            if let Some(inner_bottom) = bottom {
                spans.push(DecisionBorderSpan {
                    top,
                    bottom: inner_bottom + 1,
                    left: left.column,
                    right: right.column,
                });
            }
        }
    }
    spans
}

fn is_blank_between(line: &[VisibleChar], left: usize, right: usize) -> bool {
    line.iter()
        .filter(|cell| cell.column > left && cell.column < right)
        .all(|cell| cell.value == ' ')
}

#[derive(Clone, Copy)]
struct VisibleChar {
    column: usize,
    byte_start: usize,
    byte_end: usize,
    value: char,
}

fn visible_chars(line: &str) -> Vec<VisibleChar> {
    let bytes = line.as_bytes();
    let mut visible = Vec::new();
    let mut byte = 0;
    let mut column = 0;

    while byte < bytes.len() {
        if bytes[byte] == 0x1b && bytes.get(byte + 1) == Some(&b'[') {
            byte += 2;
            while byte < bytes.len() {
                let control = bytes[byte];
                byte += 1;
                if (0x40..=0x7e).contains(&control) {
                    break;
                }
            }
            continue;
        }

        let value = line[byte..]
            .chars()
            .next()
            .expect("byte offset must stay on a UTF-8 boundary");
        let byte_end = byte + value.len_utf8();
        visible.push(VisibleChar {
            column,
            byte_start: byte,
            byte_end,
            value,
        });
        column += char_display_width(value);
        byte = byte_end;
    }

    visible
}

fn visible_char_at(line: &[VisibleChar], column: usize) -> Option<VisibleChar> {
    line.iter().find(|cell| cell.column == column).copied()
}
