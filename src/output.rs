//! Shared deterministic structured output and final render bounds.

use crate::ansi::{AnsiEncoder, AnsiRole, strip_ansi};
use crate::error::{MermansiError, Result};
use crate::options::{MAX_CANVAS_CELLS, MAX_OUTPUT_BYTES, MermansiOptions};
use crate::str_display_width;
use serde::Serialize;
use serde_json::{Map, Value};

pub(crate) fn render_structured_model<T: Serialize>(
    family: &str,
    model: &T,
    opts: &MermansiOptions,
) -> Result<String> {
    append_structured_model(String::new(), family, model, opts)
}

pub(crate) fn append_structured_model<T: Serialize>(
    mut preview: String,
    family: &str,
    model: &T,
    opts: &MermansiOptions,
) -> Result<String> {
    ensure_byte_capacity(preview.len(), 0)?;
    if !preview.is_empty() {
        ensure_byte_capacity(preview.len(), 2)?;
        if !preview.ends_with('\n') {
            preview.push('\n');
        }
        preview.push('\n');
    }

    let header = format!("[{family} semantic model]");
    let encoder = AnsiEncoder::new(opts.color_mode);
    let painted_header = encoder.paint(AnsiRole::SectionHeader, &header);
    ensure_byte_capacity(preview.len(), painted_header.len().saturating_add(1))?;
    preview.push_str(&painted_header);
    preview.push('\n');

    let canonical = canonicalize(serde_json::to_value(model)?);
    let (serialization, exceeded) = {
        let mut writer = BoundedStringWriter {
            output: &mut preview,
            exceeded: None,
        };
        let serialization = serde_json::to_writer_pretty(&mut writer, &canonical);
        (serialization, writer.exceeded)
    };
    if let Some(requested) = exceeded {
        return Err(MermansiError::RenderLimit {
            context: "output bytes",
            requested,
            limit: MAX_OUTPUT_BYTES,
        });
    }
    serialization?;
    ensure_byte_capacity(preview.len(), 1)?;
    preview.push('\n');
    Ok(preview)
}

fn ensure_byte_capacity(current: usize, additional: usize) -> Result<()> {
    let requested = current.saturating_add(additional);
    if requested > MAX_OUTPUT_BYTES {
        return Err(MermansiError::RenderLimit {
            context: "output bytes",
            requested,
            limit: MAX_OUTPUT_BYTES,
        });
    }
    Ok(())
}

struct BoundedStringWriter<'a> {
    output: &'a mut String,
    exceeded: Option<usize>,
}

impl std::io::Write for BoundedStringWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let requested = self.output.len().saturating_add(buf.len());
        if requested > MAX_OUTPUT_BYTES {
            self.exceeded = Some(requested);
            return Err(std::io::Error::other(
                "structured output byte limit exceeded",
            ));
        }
        let text = std::str::from_utf8(buf)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        self.output.push_str(text);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        Value::Object(values) => {
            let mut entries = values.into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize(value));
            }
            Value::Object(sorted)
        }
        scalar => scalar,
    }
}

pub(crate) fn validate_output(output: &str, opts: &MermansiOptions) -> Result<()> {
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(MermansiError::RenderLimit {
            context: "output bytes",
            requested: output.len(),
            limit: MAX_OUTPUT_BYTES,
        });
    }

    let mut rows = 0usize;
    let mut max_width = 0usize;
    for line in output.lines() {
        rows = rows.checked_add(1).ok_or(MermansiError::RenderLimit {
            context: "output rows",
            requested: usize::MAX,
            limit: opts.max_height,
        })?;
        max_width = max_width.max(str_display_width(&strip_ansi(line)));
    }
    if rows > opts.max_height {
        return Err(MermansiError::RenderLimit {
            context: "output rows",
            requested: rows,
            limit: opts.max_height,
        });
    }
    if max_width > opts.max_width {
        return Err(MermansiError::RenderLimit {
            context: "output columns",
            requested: max_width,
            limit: opts.max_width,
        });
    }

    let cells = max_width
        .checked_mul(rows)
        .ok_or(MermansiError::RenderLimit {
            context: "output cells",
            requested: usize::MAX,
            limit: MAX_CANVAS_CELLS,
        })?;
    if cells > MAX_CANVAS_CELLS {
        return Err(MermansiError::RenderLimit {
            context: "output cells",
            requested: cells,
            limit: MAX_CANVAS_CELLS,
        });
    }
    Ok(())
}
