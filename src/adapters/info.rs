//! Info diagram terminal geometry.

use crate::adapters::chart_primitives::render_cropped_canvas;
use crate::canvas::{Canvas, draw_box, draw_horizontal_line};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::info::InfoDiagramRenderModel;

pub fn render_info(model: &InfoDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let header = "Mermaid Info";
    let state = format!("showInfo: {}", model.show_info);
    let width = str_display_width(header)
        .max(str_display_width(&state))
        .saturating_add(4);
    let height = 5;
    ensure_dimension("info columns", width, opts.max_width)?;
    ensure_dimension("info rows", height, opts.max_height)?;

    let mut canvas = Canvas::new(width, height)?;
    draw_box(&mut canvas, 0, 0, width, height, opts.charset)?;
    draw_horizontal_line(&mut canvas, 2, 0, width - 1, opts.charset)?;
    write_centered(&mut canvas, 1, width, header)?;
    write_centered(&mut canvas, 3, width, &state)?;
    Ok(render_cropped_canvas(&canvas))
}

fn write_centered(canvas: &mut Canvas, y: usize, width: usize, text: &str) -> Result<()> {
    let x = width.saturating_sub(str_display_width(text)) / 2;
    canvas.set_text(x, y, text)
}

fn ensure_dimension(context: &'static str, requested: usize, limit: usize) -> Result<()> {
    if requested > limit {
        return Err(MermansiError::RenderLimit {
            context,
            requested,
            limit,
        });
    }
    Ok(())
}
