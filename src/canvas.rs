//! Display-column-aware bounded canvas with Unicode width support.
//!
//! The canvas tracks terminal display columns (not byte counts or scalar-value counts) so that
//! wide characters (CJK), combining marks, and emoji are positioned correctly. All allocations
//! are bounded; overflows return [`MermansiError::CanvasOverflow`](crate::error::MermansiError::CanvasOverflow).

use crate::error::{MermansiError, Result};
use crate::options::Charset;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Returns the terminal display width of a character (0, 1, or 2 columns).
pub fn char_display_width(ch: char) -> usize {
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Returns the terminal display width of a string, accounting for wide characters.
pub fn str_display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Returns the terminal display width of a label, handling combining marks correctly.
///
/// Combining marks (Unicode general category `Mn`/`Me`) have width 0 and attach to the
/// preceding base character. Emoji and CJK characters have width 2.
pub fn label_display_width(s: &str) -> usize {
    str_display_width(s)
}

/// Maximum number of canvas cells.
pub const MAX_CELLS: usize = 250_000;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cell {
    Empty,
    Grapheme {
        text: String,
        width: usize,
    },
    Continuation {
        owner: usize,
    },
    Stroke {
        directions: u8,
        glyph: &'static str,
        charset: Charset,
    },
}

impl Cell {
    fn text(&self) -> &str {
        match self {
            Self::Grapheme { text, .. } => text,
            Self::Stroke { glyph, .. } => glyph,
            Self::Empty | Self::Continuation { .. } => "",
        }
    }
}

#[derive(Debug)]
struct PlannedGrapheme {
    owner: usize,
    text: String,
    width: usize,
}

#[derive(Debug, Default)]
struct TextPlan {
    writes: Vec<PlannedGrapheme>,
    appends: Vec<(usize, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
    style_prefix: Vec<String>,
    style_suffix: Vec<String>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Result<Self> {
        let cells = width
            .checked_mul(height)
            .ok_or(MermansiError::CanvasOverflow {
                requested: usize::MAX,
                max: MAX_CELLS,
            })?;
        if cells > MAX_CELLS {
            return Err(MermansiError::CanvasOverflow {
                requested: cells,
                max: MAX_CELLS,
            });
        }
        Ok(Self {
            width,
            height,
            cells: vec![Cell::Empty; cells],
            style_prefix: vec![String::new(); cells],
            style_suffix: vec![String::new(); cells],
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            y.checked_mul(self.width)?.checked_add(x)
        } else {
            None
        }
    }

    fn requested_index(&self, x: usize, y: usize) -> usize {
        y.checked_mul(self.width)
            .and_then(|offset| offset.checked_add(x))
            .unwrap_or(usize::MAX)
    }

    fn bounds_error(&self, x: usize, y: usize) -> MermansiError {
        MermansiError::CanvasOverflow {
            requested: self.requested_index(x, y),
            max: self.cells.len(),
        }
    }

    fn owner_for_index(&self, index: usize) -> Option<usize> {
        match self.cells.get(index)? {
            Cell::Grapheme { .. } => Some(index),
            Cell::Continuation { owner } => Some(*owner),
            Cell::Empty | Cell::Stroke { .. } => None,
        }
    }

    fn clear_owner(&mut self, owner: usize) {
        let width = match self.cells.get(owner) {
            Some(Cell::Grapheme { width, .. }) => *width,
            _ => return,
        };
        for index in owner..owner.saturating_add(width).min(self.cells.len()) {
            let belongs_to_owner = index == owner
                || matches!(
                    self.cells.get(index),
                    Some(Cell::Continuation {
                        owner: continuation_owner
                    }) if *continuation_owner == owner
                );
            if belongs_to_owner {
                self.cells[index] = Cell::Empty;
                self.style_prefix[index].clear();
                self.style_suffix[index].clear();
            }
        }
    }

    fn preceding_owner(&self, x: usize, y: usize) -> Option<usize> {
        let previous_x = x.checked_sub(1)?;
        let previous = self.index(previous_x, y)?;
        self.owner_for_index(previous)
    }

    fn plan_text(&self, x: usize, y: usize, text: &str) -> Result<TextPlan> {
        if y >= self.height || x > self.width {
            return Err(self.bounds_error(x, y));
        }

        let mut plan = TextPlan::default();
        let mut cursor = x;
        for grapheme in UnicodeSegmentation::graphemes(text, true) {
            let width = UnicodeWidthStr::width(grapheme);
            if width == 0 {
                if let Some(previous) = plan.writes.last_mut() {
                    previous.text.push_str(grapheme);
                    continue;
                }
                let owner =
                    self.preceding_owner(cursor, y)
                        .ok_or(MermansiError::CanvasGrapheme {
                            message: "zero-width grapheme requires a preceding base grapheme",
                        })?;
                if let Some((_, pending)) = plan
                    .appends
                    .iter_mut()
                    .find(|(pending_owner, _)| *pending_owner == owner)
                {
                    pending.push_str(grapheme);
                } else {
                    plan.appends.push((owner, grapheme.to_owned()));
                }
                continue;
            }

            let end = cursor
                .checked_add(width)
                .ok_or_else(|| self.bounds_error(cursor, y))?;
            if end > self.width {
                return Err(self.bounds_error(end.saturating_sub(1), y));
            }
            let owner = self
                .index(cursor, y)
                .ok_or_else(|| self.bounds_error(cursor, y))?;
            plan.writes.push(PlannedGrapheme {
                owner,
                text: grapheme.to_owned(),
                width,
            });
            cursor = end;
        }
        Ok(plan)
    }

    fn apply_text_plan(&mut self, plan: TextPlan) -> Result<()> {
        let mut owners_to_clear = BTreeSet::new();
        for write in &plan.writes {
            for index in write.owner..write.owner + write.width {
                if let Some(owner) = self.owner_for_index(index) {
                    owners_to_clear.insert(owner);
                }
            }
        }
        if plan
            .appends
            .iter()
            .any(|(owner, _)| owners_to_clear.contains(owner))
        {
            return Err(MermansiError::CanvasGrapheme {
                message: "zero-width grapheme base is overwritten by the same write",
            });
        }

        for owner in owners_to_clear {
            self.clear_owner(owner);
        }
        for (owner, suffix) in plan.appends {
            let Some(Cell::Grapheme { text, .. }) = self.cells.get_mut(owner) else {
                return Err(MermansiError::CanvasGrapheme {
                    message: "zero-width grapheme base no longer exists",
                });
            };
            text.push_str(&suffix);
        }
        for write in plan.writes {
            for index in write.owner..write.owner + write.width {
                self.cells[index] = Cell::Empty;
                self.style_prefix[index].clear();
                self.style_suffix[index].clear();
            }
            self.cells[write.owner] = Cell::Grapheme {
                text: write.text,
                width: write.width,
            };
            for index in (write.owner + 1)..(write.owner + write.width) {
                self.cells[index] = Cell::Continuation { owner: write.owner };
            }
        }
        Ok(())
    }

    fn merge_stroke(&mut self, x: usize, y: usize, directions: u8, charset: Charset) {
        let Some(index) = self.index(x, y) else {
            return;
        };
        let merged = match self.cells[index] {
            Cell::Empty => directions,
            Cell::Stroke {
                directions: existing,
                charset: existing_charset,
                ..
            } if existing_charset == charset => existing | directions,
            Cell::Stroke { .. } => directions,
            Cell::Grapheme { .. } | Cell::Continuation { .. } => return,
        };
        self.cells[index] = Cell::Stroke {
            directions: merged,
            glyph: stroke_glyph(merged, charset),
            charset,
        };
        self.style_prefix[index].clear();
        self.style_suffix[index].clear();
    }

    /// Set a single character at `(x, y)` as an atomic grapheme write.
    pub fn set_char(&mut self, x: usize, y: usize, ch: char) -> Result<()> {
        let mut encoded = [0u8; 4];
        self.set_text(x, y, ch.encode_utf8(&mut encoded))
    }

    /// Place grapheme clusters starting at `(x, y)` without partially applying failed writes.
    pub fn set_text(&mut self, x: usize, y: usize, text: &str) -> Result<()> {
        let plan = self.plan_text(x, y, text)?;
        self.apply_text_plan(plan)
    }

    pub fn set_styled_text(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        prefix: &str,
        suffix: &str,
    ) -> Result<()> {
        self.set_text(x, y, text)?;
        let owner = self
            .index(x, y)
            .and_then(|index| self.owner_for_index(index))
            .or_else(|| self.preceding_owner(x, y));
        if let Some(owner) = owner {
            self.style_prefix[owner] = prefix.to_string();
            self.style_suffix[owner] = suffix.to_string();
        }
        Ok(())
    }

    pub fn get_cell(&self, x: usize, y: usize) -> Option<&str> {
        self.index(x, y).map(|index| self.cells[index].text())
    }

    /// Return the owning cell coordinate when `(x, y)` is a continuation cell.
    pub fn continuation_owner(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        let index = self.index(x, y)?;
        let Cell::Continuation { owner } = self.cells[index] else {
            return None;
        };
        Some((owner % self.width, owner / self.width))
    }

    /// Render the canvas to a `String` with trailing whitespace trimmed per line.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.width * self.height);
        for y in 0..self.height {
            let row_start = y * self.width;
            let row_end = row_start + self.width;
            let mut line = String::with_capacity(self.width);
            let mut last_nonblank = 0usize;
            for x in 0..self.width {
                let idx = row_start + x;
                let prefix = &self.style_prefix[idx];
                let cell = &self.cells[idx];
                let suffix = &self.style_suffix[idx];
                match cell {
                    Cell::Grapheme { text, .. } => {
                        let _ = write!(line, "{prefix}{text}{suffix}");
                        last_nonblank = line.len();
                    }
                    Cell::Stroke { glyph, .. } => {
                        let _ = write!(line, "{prefix}{glyph}{suffix}");
                        last_nonblank = line.len();
                    }
                    Cell::Empty if !prefix.is_empty() || !suffix.is_empty() => {
                        let _ = write!(line, "{prefix} {suffix}");
                        last_nonblank = line.len();
                    }
                    Cell::Empty => line.push(' '),
                    Cell::Continuation { .. } => {}
                }
            }
            // Trim trailing whitespace but keep any styled suffixes
            line.truncate(last_nonblank.max(1));
            if line.trim().is_empty() {
                line.clear();
            }
            out.push_str(&line);
            out.push('\n');
            let _ = row_end; // suppress unused warning
        }
        out
    }
}

const NORTH: u8 = 1 << 0;
const EAST: u8 = 1 << 1;
const SOUTH: u8 = 1 << 2;
const WEST: u8 = 1 << 3;

fn stroke_glyph(directions: u8, charset: Charset) -> &'static str {
    let horizontal = directions & (EAST | WEST) != 0;
    let vertical = directions & (NORTH | SOUTH) != 0;
    if charset == Charset::Ascii {
        return match (horizontal, vertical) {
            (true, true) => "+",
            (true, false) => "-",
            (false, true) => "|",
            (false, false) => "+",
        };
    }

    match directions {
        d if d == (EAST | WEST) => "─",
        d if d == (NORTH | SOUTH) => "│",
        d if d == (EAST | SOUTH) => "┌",
        d if d == (SOUTH | WEST) => "┐",
        d if d == (NORTH | EAST) => "└",
        d if d == (NORTH | WEST) => "┘",
        d if d == (EAST | SOUTH | WEST) => "┬",
        d if d == (NORTH | EAST | WEST) => "┴",
        d if d == (NORTH | EAST | SOUTH) => "├",
        d if d == (NORTH | SOUTH | WEST) => "┤",
        d if d == (NORTH | EAST | SOUTH | WEST) => "┼",
        d if d & (EAST | WEST) != 0 => "─",
        d if d & (NORTH | SOUTH) != 0 => "│",
        _ => "┼",
    }
}

/// Draw a horizontal line.
pub fn draw_horizontal_line(
    canvas: &mut Canvas,
    y: usize,
    x1: usize,
    x2: usize,
    charset: Charset,
) -> Result<()> {
    if x1 > x2 {
        return Ok(());
    }
    canvas
        .index(x1, y)
        .ok_or_else(|| canvas.bounds_error(x1, y))?;
    canvas
        .index(x2, y)
        .ok_or_else(|| canvas.bounds_error(x2, y))?;
    for x in x1..=x2 {
        let directions = if x1 == x2 {
            EAST | WEST
        } else {
            (if x > x1 { WEST } else { 0 }) | (if x < x2 { EAST } else { 0 })
        };
        canvas.merge_stroke(x, y, directions, charset);
    }
    Ok(())
}

/// Draw a vertical line.
pub fn draw_vertical_line(
    canvas: &mut Canvas,
    x: usize,
    y1: usize,
    y2: usize,
    charset: Charset,
) -> Result<()> {
    if y1 > y2 {
        return Ok(());
    }
    canvas
        .index(x, y1)
        .ok_or_else(|| canvas.bounds_error(x, y1))?;
    canvas
        .index(x, y2)
        .ok_or_else(|| canvas.bounds_error(x, y2))?;
    for y in y1..=y2 {
        let directions = if y1 == y2 {
            NORTH | SOUTH
        } else {
            (if y > y1 { NORTH } else { 0 }) | (if y < y2 { SOUTH } else { 0 })
        };
        canvas.merge_stroke(x, y, directions, charset);
    }
    Ok(())
}

/// Draw a rectangular box border with corners.
pub fn draw_box(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    charset: Charset,
) -> Result<()> {
    if width < 2 || height < 2 {
        return Ok(());
    }
    let x2 = x
        .checked_add(width - 1)
        .ok_or_else(|| canvas.bounds_error(usize::MAX, y))?;
    let y2 = y
        .checked_add(height - 1)
        .ok_or_else(|| canvas.bounds_error(x, usize::MAX))?;
    canvas
        .index(x, y)
        .ok_or_else(|| canvas.bounds_error(x, y))?;
    canvas
        .index(x2, y2)
        .ok_or_else(|| canvas.bounds_error(x2, y2))?;

    draw_horizontal_line(canvas, y, x, x2, charset)?;
    draw_horizontal_line(canvas, y2, x, x2, charset)?;
    draw_vertical_line(canvas, x, y, y2, charset)?;
    draw_vertical_line(canvas, x2, y, y2, charset)?;
    Ok(())
}
