//! Quadrant chart adapter.

use crate::adapters::format_title;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::quadrant_chart::QuadrantChartRenderModel;

pub fn render_quadrant_chart(
    model: &QuadrantChartRenderModel,
    opts: &MermansiOptions,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    let q = &model.quadrants;
    let axes = &model.axes;

    out.push_str("Quadrants:\n");
    out.push_str(&format!("  Q1 (top-right):    {}\n", q.quadrant1_text));
    out.push_str(&format!("  Q2 (top-left):     {}\n", q.quadrant2_text));
    out.push_str(&format!("  Q3 (bottom-left):  {}\n", q.quadrant3_text));
    out.push_str(&format!("  Q4 (bottom-right): {}\n", q.quadrant4_text));
    out.push('\n');

    out.push_str("Axes:\n");
    out.push_str(&format!(
        "  X: {} <---> {}\n",
        axes.x_axis_left_text, axes.x_axis_right_text
    ));
    out.push_str(&format!(
        "  Y: {} <---> {}\n",
        axes.y_axis_bottom_text, axes.y_axis_top_text
    ));
    out.push('\n');

    if !model.points.is_empty() {
        out.push_str("Points:\n");
        for point in &model.points {
            out.push_str(&format!(
                "  {} ({:.2}, {:.2})\n",
                point.text, point.x, point.y
            ));
        }
    } else {
        out.push_str("(no points)\n");
    }

    let _ = opts;
    Ok(out)
}
