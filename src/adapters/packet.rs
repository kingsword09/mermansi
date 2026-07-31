//! Packet diagram terminal geometry.
//!
//! Every 32-bit word is a closed row whose field widths are proportional to the parsed bit range.
//! Repeated contiguous labels share a callout so fields spanning word boundaries remain explicit.

use crate::adapters::box_geometry::wrap_display;
use crate::adapters::chart_primitives::render_cropped_canvas;
use crate::ansi::sanitize_label_text;
use crate::canvas::{Canvas, draw_box, draw_vertical_line};
use crate::error::{MermansiError, Result};
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::packet::{PacketDiagramRenderModel, PacketRenderBlock};

const WORD_BITS: i64 = 32;
const MAX_PACKET_FIELDS: usize = 4_096;
const MAX_PACKET_ROWS: usize = 4_096;
const ROW_HEIGHT: usize = 4;
const ROW_GAP: usize = 1;

#[derive(Clone, Debug)]
struct Field {
    label: String,
    start: i64,
    end: i64,
}

pub fn render_packet(model: &PacketDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    ensure_limit("packet rows", model.packet.len(), MAX_PACKET_ROWS)?;
    let segment_count = model
        .packet
        .iter()
        .fold(0usize, |count, row| count.saturating_add(row.len()));
    ensure_limit("packet fields", segment_count, MAX_PACKET_FIELDS)?;
    if model.packet.is_empty() {
        return render_empty_packet(model, opts);
    }

    let (fields, field_indices) = collect_fields(model)?;
    let widest_base = model
        .packet
        .iter()
        .enumerate()
        .map(|(index, row)| word_base(index, row).to_string().len())
        .max()
        .unwrap_or(2)
        .max(2);
    let prefix_width = widest_base.saturating_add(2);
    let minimum_width = prefix_width.saturating_add(35);
    if opts.max_width < minimum_width {
        return Err(MermansiError::RenderLimit {
            context: "packet columns",
            requested: minimum_width,
            limit: opts.max_width,
        });
    }
    let width = opts.max_width.min(100);
    let box_x = prefix_width;
    let box_width = width - box_x;
    let legend_width = width.saturating_sub(2).max(1);
    let legend_lines = fields
        .iter()
        .enumerate()
        .flat_map(|(index, field)| {
            let bits = field.end.saturating_sub(field.start).saturating_add(1);
            wrap_display(
                &format!(
                    "[{}] {}  {}..{}  ({} bits)",
                    index + 1,
                    nonempty(&field.label),
                    field.start,
                    field.end,
                    bits
                ),
                legend_width,
            )
        })
        .collect::<Vec<_>>();
    let first_row_y = 3usize;
    let rows_height = model
        .packet
        .len()
        .saturating_mul(ROW_HEIGHT + ROW_GAP)
        .saturating_sub(ROW_GAP);
    let legend_y = first_row_y + rows_height + 1;
    let height = legend_y.saturating_add(legend_lines.len()).max(7);
    if opts.max_height < height {
        return Err(MermansiError::RenderLimit {
            context: "packet rows",
            requested: height,
            limit: opts.max_height,
        });
    }

    let mut canvas = Canvas::new(width, height)?;
    let title = model
        .title
        .as_deref()
        .map(sanitize_label_text)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "Packet diagram".to_owned());
    write_centered(&mut canvas, box_x, 0, box_width, &title)?;
    for bit in [0usize, 8, 16, 24, 31] {
        let label = bit.to_string();
        let x = bit_boundary(box_x, box_width, bit)
            .saturating_sub(str_display_width(&label) / 2)
            .min(width.saturating_sub(str_display_width(&label)));
        canvas.set_text(x, 2, &label)?;
    }

    for (row_index, row) in model.packet.iter().enumerate() {
        let y = first_row_y + row_index * (ROW_HEIGHT + ROW_GAP);
        let base = word_base(row_index, row);
        canvas.set_text(0, y + 1, &format!("{base:>widest_base$}:"))?;
        draw_box(&mut canvas, box_x, y, box_width, ROW_HEIGHT, opts.charset)?;
        for (block_index, block) in row.iter().enumerate() {
            validate_segment(block, base)?;
            let start = usize::try_from(block.start - base).map_err(|_| layout_error(block))?;
            let end = usize::try_from(block.end - base + 1).map_err(|_| layout_error(block))?;
            let left = bit_boundary(box_x, box_width, start);
            let right = bit_boundary(box_x, box_width, end);
            if start > 0 {
                draw_vertical_line(&mut canvas, left, y, y + ROW_HEIGHT - 1, opts.charset)?;
            }
            if end < 32 {
                draw_vertical_line(&mut canvas, right, y, y + ROW_HEIGHT - 1, opts.charset)?;
            }
            let interior_left = left.saturating_add(1);
            let interior_width = right.saturating_sub(interior_left);
            if interior_width == 0 {
                continue;
            }
            let label = sanitize_label_text(&block.label);
            let field_index = field_indices[row_index][block_index] + 1;
            let display_label = nonempty(&label);
            let visible = if str_display_width(&display_label) <= interior_width {
                display_label
            } else {
                format!("[{field_index}]")
            };
            write_centered(&mut canvas, interior_left, y + 1, interior_width, &visible)?;
            let range = format!("{}..{}", block.start, block.end);
            let range = if str_display_width(&range) <= interior_width {
                range
            } else {
                format!("{field_index}")
            };
            write_centered(&mut canvas, interior_left, y + 2, interior_width, &range)?;
        }
    }
    for (offset, line) in legend_lines.iter().enumerate() {
        canvas.set_text(1, legend_y + offset, line)?;
    }
    Ok(render_cropped_canvas(&canvas))
}

fn collect_fields(model: &PacketDiagramRenderModel) -> Result<(Vec<Field>, Vec<Vec<usize>>)> {
    let mut fields = Vec::<Field>::new();
    let mut indices = Vec::with_capacity(model.packet.len());
    for (row_index, row) in model.packet.iter().enumerate() {
        let base = word_base(row_index, row);
        let mut row_indices = Vec::with_capacity(row.len());
        for block in row {
            validate_segment(block, base)?;
            let label = sanitize_label_text(&block.label);
            let index = if let Some(last) = fields.last_mut()
                && last.label == label
                && last.end.checked_add(1) == Some(block.start)
                && last.end.rem_euclid(WORD_BITS) == WORD_BITS - 1
                && block.start.rem_euclid(WORD_BITS) == 0
            {
                last.end = block.end;
                fields.len() - 1
            } else {
                fields.push(Field {
                    label,
                    start: block.start,
                    end: block.end,
                });
                fields.len() - 1
            };
            row_indices.push(index);
        }
        indices.push(row_indices);
    }
    Ok((fields, indices))
}

fn validate_segment(block: &PacketRenderBlock, base: i64) -> Result<()> {
    let word_end = base.saturating_add(WORD_BITS - 1);
    if block.start < base || block.end < block.start || block.end > word_end {
        return Err(layout_error(block));
    }
    Ok(())
}

fn layout_error(block: &PacketRenderBlock) -> MermansiError {
    MermansiError::GeometryLayout {
        family: "packet",
        message: format!(
            "field range is outside its 32-bit word: {}..{}",
            block.start, block.end
        ),
    }
}

fn word_base(row_index: usize, row: &[PacketRenderBlock]) -> i64 {
    row.first().map_or_else(
        || {
            i64::try_from(row_index)
                .unwrap_or(i64::MAX)
                .saturating_mul(WORD_BITS)
        },
        |block| block.start.div_euclid(WORD_BITS).saturating_mul(WORD_BITS),
    )
}

fn bit_boundary(box_x: usize, box_width: usize, bit: usize) -> usize {
    box_x + bit.min(32) * (box_width - 1) / 32
}

fn render_empty_packet(model: &PacketDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let width = opts.max_width.min(32);
    let height = opts.max_height.min(5);
    if width < 16 || height < 3 {
        return Err(MermansiError::RenderLimit {
            context: "packet empty card",
            requested: 16,
            limit: width.min(height),
        });
    }
    let mut canvas = Canvas::new(width, height)?;
    draw_box(&mut canvas, 0, 0, width, 3, opts.charset)?;
    let label = model.title.as_deref().unwrap_or("empty packet");
    write_centered(&mut canvas, 1, 1, width - 2, &nonempty(label))?;
    Ok(render_cropped_canvas(&canvas))
}

fn write_centered(
    canvas: &mut Canvas,
    left: usize,
    y: usize,
    width: usize,
    text: &str,
) -> Result<()> {
    let text = sanitize_label_text(text);
    let text_width = str_display_width(&text);
    if text_width > width {
        return Err(MermansiError::GeometryLayout {
            family: "packet",
            message: "packet cell text exceeds its assigned width".to_owned(),
        });
    }
    canvas.set_text(left + (width - text_width) / 2, y, &text)
}

fn nonempty(value: &str) -> String {
    let value = sanitize_label_text(value).trim().to_owned();
    if value.is_empty() {
        "(unnamed)".to_owned()
    } else {
        value
    }
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
