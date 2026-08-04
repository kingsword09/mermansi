//! Shared bounded chart primitives for terminal-native chart geometry.
//!
//! These primitives are genuinely shared by the Pie, Radar, QuadrantChart, and Venn
//! adapters: midpoint circle outline/fill, Bresenham line draw, and radial-line helpers.
//! All operations respect Canvas bounds and return typed errors on overflow.

use crate::canvas::Canvas;
use crate::error::{MermansiError, Result};
use crate::options::{Charset, MermansiOptions};

/// Maximum number of chart entities (sections, axes, curves, points, sets) rendered.
pub const MAX_CHART_ENTITIES: usize = 4_096;

/// Maximum bounded work spent searching for chart labels and collision-free points.
pub const MAX_CHART_WORK: usize = 2_000_000;

/// Shared deterministic work budget for chart placement searches.
#[derive(Debug)]
pub struct ChartWorkBudget {
    context: &'static str,
    used: usize,
}

impl ChartWorkBudget {
    pub const fn new(context: &'static str) -> Self {
        Self { context, used: 0 }
    }

    pub fn consume(&mut self, amount: usize) -> Result<()> {
        self.used = self.used.saturating_add(amount);
        if self.used > MAX_CHART_WORK {
            return Err(MermansiError::RenderLimit {
                context: self.context,
                requested: self.used,
                limit: MAX_CHART_WORK,
            });
        }
        Ok(())
    }
}

/// Horizontal character cells per vertical row for visually balanced radial geometry.
pub const RADIAL_X_SCALE: f64 = 2.0;

/// Project a logical polar coordinate into terminal character-cell coordinates.
pub fn radial_point(center_x: f64, center_y: f64, radius: f64, angle: f64) -> (f64, f64) {
    (
        center_x + radius * RADIAL_X_SCALE * angle.cos(),
        center_y + radius * angle.sin(),
    )
}

/// Choose a terminal-chart canvas size without exceeding the caller's limits.
///
/// Width and height are checked independently because radial charts and Cartesian charts require
/// different aspect ratios. A chart that cannot meet its minimum dimensions returns a typed limit
/// error before any [`Canvas`] is allocated.
pub fn checked_chart_dimensions(
    opts: &MermansiOptions,
    minimum: (usize, usize),
    preferred_maximum: (usize, usize),
) -> Result<(usize, usize)> {
    let (minimum_width, minimum_height) = minimum;
    let (maximum_width, maximum_height) = preferred_maximum;

    if opts.max_width < minimum_width {
        return Err(MermansiError::RenderLimit {
            context: "chart width",
            requested: minimum_width,
            limit: opts.max_width,
        });
    }
    if opts.max_height < minimum_height {
        return Err(MermansiError::RenderLimit {
            context: "chart height",
            requested: minimum_height,
            limit: opts.max_height,
        });
    }

    let width = opts.max_width.min(maximum_width);
    if width < minimum_width {
        return Err(MermansiError::RenderLimit {
            context: "chart width",
            requested: minimum_width,
            limit: width,
        });
    }

    let height = opts.max_height.min(maximum_height);
    if height < minimum_height {
        return Err(MermansiError::RenderLimit {
            context: "chart height",
            requested: minimum_height,
            limit: height,
        });
    }

    Ok((width, height))
}

/// Render a chart canvas without unused outer rows or a shared left margin.
pub fn render_cropped_canvas(canvas: &Canvas) -> String {
    let rendered = canvas.render();
    let lines = rendered.lines().collect::<Vec<_>>();
    let Some(first) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .unwrap_or(first);
    let common_indent = lines[first..=last]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.bytes().take_while(|byte| *byte == b' ').count())
        .min()
        .unwrap_or(0);

    let mut cropped = String::new();
    for line in &lines[first..=last] {
        cropped.push_str(line.get(common_indent..).unwrap_or(line));
        cropped.push('\n');
    }
    cropped
}

/// Ensure an entity count does not exceed the chart entity limit.
pub fn ensure_entity_limit(context: &'static str, requested: usize) -> Result<()> {
    if requested > MAX_CHART_ENTITIES {
        return Err(MermansiError::RenderLimit {
            context,
            requested,
            limit: MAX_CHART_ENTITIES,
        });
    }
    Ok(())
}

/// Returns the fill character for the given series index and charset.
pub fn fill_char(index: usize, charset: Charset) -> &'static str {
    match charset {
        Charset::Unicode => UNICODE_FILLS[index % UNICODE_FILLS.len()],
        Charset::Ascii => ASCII_FILLS[index % ASCII_FILLS.len()],
    }
}

const UNICODE_FILLS: &[&str] = &["█", "▓", "▒", "░", "●", "◆", "■", "▲"];
const ASCII_FILLS: &[&str] = &["#", "@", "%", "&", "+", "=", "*", "o"];

/// Returns the line/dot character for plotted vertices and markers.
pub fn marker_char(index: usize, charset: Charset) -> &'static str {
    match charset {
        Charset::Unicode => UNICODE_MARKERS[index % UNICODE_MARKERS.len()],
        Charset::Ascii => ASCII_MARKERS[index % ASCII_MARKERS.len()],
    }
}

const UNICODE_MARKERS: &[&str] = &["●", "◆", "■", "▲", "▼", "★", "◉", "◈"];
const ASCII_MARKERS: &[&str] = &["*", "+", "x", "o", "#", "@", "%", "&"];

/// Draw a visually circular outline on the terminal canvas.
///
/// `(cx, cy)` is the center and `radius` is measured in terminal rows. Horizontal coordinates
/// use [`RADIAL_X_SCALE`] so the result remains circular when character cells are taller than
/// they are wide. Points outside the canvas bounds are silently skipped.
pub fn draw_circle_outline(
    canvas: &mut Canvas,
    cx: i64,
    cy: i64,
    radius: i64,
    glyph: &str,
) -> Result<()> {
    if radius <= 0 {
        return Ok(());
    }
    let horizontal_radius = (radius as f64 * RADIAL_X_SCALE).round() as i64;
    for y in -radius..=radius {
        let normalized = y as f64 / radius as f64;
        let x = (horizontal_radius as f64 * (1.0 - normalized * normalized).sqrt()).round() as i64;
        plot(canvas, cx - x, cy + y, glyph)?;
        plot(canvas, cx + x, cy + y, glyph)?;
    }
    for x in -horizontal_radius..=horizontal_radius {
        let normalized = x as f64 / horizontal_radius as f64;
        let y = (radius as f64 * (1.0 - normalized * normalized).sqrt()).round() as i64;
        plot(canvas, cx + x, cy - y, glyph)?;
        plot(canvas, cx + x, cy + y, glyph)?;
    }
    Ok(())
}

/// Fill a circle on the canvas with the given glyph.
///
/// This draws horizontal scan lines at each y from top to bottom of the circle.
pub fn fill_circle(canvas: &mut Canvas, cx: i64, cy: i64, radius: i64, glyph: &str) -> Result<()> {
    if radius <= 0 {
        return Ok(());
    }
    for y in -radius..=radius {
        let chord = (radius as f64).powi(2) - (y as f64).powi(2);
        if chord < 0.0 {
            continue;
        }
        let dx = (chord.sqrt() * RADIAL_X_SCALE).round() as i64;
        for x in -dx..=dx {
            plot_safe(canvas, cx + x, cy + y, glyph)?;
        }
    }
    Ok(())
}

/// Draw a line between two points using Bresenham's algorithm.
///
/// Each cell on the line receives the given glyph (grapheme write, not merge_stroke).
pub fn draw_line(
    canvas: &mut Canvas,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    glyph: &str,
) -> Result<()> {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);

    loop {
        plot_safe(canvas, x, y, glyph)?;
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    Ok(())
}

/// Draw a filled circle sector (pie slice) between two angles.
///
/// `start_angle` and `end_angle` are in radians. The sector is filled from center outward.
pub fn fill_pie_sector(
    canvas: &mut Canvas,
    cx: i64,
    cy: i64,
    radius: i64,
    start_angle: f64,
    end_angle: f64,
    glyph: &str,
) -> Result<()> {
    if radius <= 0 {
        return Ok(());
    }
    // Normalize angles so end > start.
    let (sa, mut ea) = (start_angle, end_angle);
    while ea < sa {
        ea += std::f64::consts::TAU;
    }
    if ea - sa >= std::f64::consts::TAU - 1e-9 {
        // Full circle
        return fill_circle(canvas, cx, cy, radius, glyph);
    }

    let horizontal_radius = (radius as f64 * RADIAL_X_SCALE).round() as i64;
    for y in -radius..=radius {
        for x in -horizontal_radius..=horizontal_radius {
            let logical_x = x as f64 / RADIAL_X_SCALE;
            let dist_sq = logical_x * logical_x + (y * y) as f64;
            if dist_sq > (radius * radius) as f64 {
                continue;
            }
            let angle = (y as f64).atan2(logical_x);
            let mut a = angle;
            // Normalize a to be >= sa - 2π
            while a < sa - 1e-9 {
                a += std::f64::consts::TAU;
            }
            while a >= sa + std::f64::consts::TAU {
                a -= std::f64::consts::TAU;
            }
            if a >= sa - 1e-9 && a <= ea + 1e-9 {
                plot_safe(canvas, cx + x, cy + y, glyph)?;
            }
        }
    }
    Ok(())
}

/// Plot a point at the given coordinates if within bounds, overwriting existing cells.
fn plot_force(canvas: &mut Canvas, x: i64, y: i64, glyph: &str) -> Result<()> {
    if x < 0 || y < 0 {
        return Ok(());
    }
    let (ux, uy) = (x as usize, y as usize);
    if ux >= canvas.width() || uy >= canvas.height() {
        return Ok(());
    }
    canvas.set_text(ux, uy, glyph)
}

/// Draw a radial line from center outward at the given angle.
pub fn draw_radial_line(
    canvas: &mut Canvas,
    cx: i64,
    cy: i64,
    radius: i64,
    angle: f64,
    glyph: &str,
) -> Result<()> {
    let (ex, ey) = radial_point(cx as f64, cy as f64, radius as f64, angle);
    let ex = ex.round() as i64;
    let ey = ey.round() as i64;
    draw_line_over(canvas, cx, cy, ex, ey, glyph)
}

/// Draw a line between two points, overwriting existing cells (Bresenham).
pub fn draw_line_over(
    canvas: &mut Canvas,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    glyph: &str,
) -> Result<()> {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);

    loop {
        plot_force(canvas, x, y, glyph)?;
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    Ok(())
}

/// Plot a point at the given coordinates if within bounds.
fn plot_safe(canvas: &mut Canvas, x: i64, y: i64, glyph: &str) -> Result<()> {
    if x < 0 || y < 0 {
        return Ok(());
    }
    let (ux, uy) = (x as usize, y as usize);
    if ux >= canvas.width() || uy >= canvas.height() {
        return Ok(());
    }
    // Only write if the cell is empty — this preserves outlines over fills.
    if canvas.get_cell(ux, uy).is_some_and(|s| !s.is_empty()) {
        return Ok(());
    }
    canvas.set_text(ux, uy, glyph)
}

/// Plot a point, overwriting whatever is there (used for outlines).
fn plot(canvas: &mut Canvas, x: i64, y: i64, glyph: &str) -> Result<()> {
    if x < 0 || y < 0 {
        return Ok(());
    }
    let (ux, uy) = (x as usize, y as usize);
    if ux >= canvas.width() || uy >= canvas.height() {
        return Ok(());
    }
    canvas.set_text(ux, uy, glyph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_projection_compensates_terminal_cells() {
        let center = (20.0, 10.0);
        let east = radial_point(center.0, center.1, 4.0, 0.0);
        let north = radial_point(center.0, center.1, 4.0, -std::f64::consts::FRAC_PI_2);

        assert_eq!(east.0 - center.0, 8.0);
        assert_eq!(center.1 - north.1, 4.0);
    }

    #[test]
    fn chart_work_budget_is_typed_and_bounded() {
        let mut budget = ChartWorkBudget::new("chart test work");
        assert!(budget.consume(MAX_CHART_WORK).is_ok());
        assert!(matches!(
            budget.consume(1),
            Err(MermansiError::RenderLimit {
                context: "chart test work",
                requested,
                limit: MAX_CHART_WORK,
            }) if requested == MAX_CHART_WORK + 1
        ));
    }
}
