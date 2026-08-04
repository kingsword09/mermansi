//! Shared deterministic structured output and final render bounds.

use crate::ansi::{AnsiEncoder, AnsiRole, is_bidi_format_control, strip_ansi};
use crate::error::{MermansiError, Result};
use crate::options::{MAX_CANVAS_CELLS, MAX_OUTPUT_BYTES, MermansiOptions, OutputMode};
use crate::str_display_width;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug)]
pub(crate) struct AdapterOutput {
    pub(crate) preview: String,
    semantic: Option<String>,
}

impl AdapterOutput {
    fn preview(preview: String) -> Self {
        Self {
            preview,
            semantic: None,
        }
    }

    fn complete(preview: String, semantic: String) -> Self {
        Self {
            preview,
            semantic: Some(semantic),
        }
    }

    pub(crate) fn to_text(&self) -> Result<String> {
        let separator = if self.semantic.is_some() && !self.preview.is_empty() {
            1 + usize::from(!self.preview.ends_with('\n'))
        } else {
            0
        };
        let semantic_len = self.semantic.as_ref().map_or(0, String::len);
        ensure_byte_capacity(self.preview.len(), separator.saturating_add(semantic_len))?;
        let mut output = String::with_capacity(
            self.preview
                .len()
                .saturating_add(separator)
                .saturating_add(semantic_len),
        );
        output.push_str(&self.preview);
        if let Some(semantic) = &self.semantic {
            if !self.preview.is_empty() {
                if !self.preview.ends_with('\n') {
                    output.push('\n');
                }
                output.push('\n');
            }
            output.push_str(semantic);
        }
        Ok(output)
    }
}

pub(crate) fn render_structured_adapter<T, F>(
    family: &str,
    model: &T,
    opts: &MermansiOptions,
    render_preview: F,
) -> Result<AdapterOutput>
where
    T: Serialize,
    F: FnOnce() -> Result<String>,
{
    if opts.output_mode == OutputMode::Concise {
        let preview = render_preview()?;
        if !preview.trim().is_empty() {
            return Ok(AdapterOutput::preview(preview));
        }
        ensure_serialized_model_within_limit(model)?;
        return Ok(AdapterOutput::preview(render_semantic_section(
            family, model, opts,
        )?));
    }
    ensure_serialized_model_within_limit(model)?;
    Ok(AdapterOutput::complete(
        render_preview()?,
        render_semantic_section(family, model, opts)?,
    ))
}

fn render_semantic_section<T: Serialize>(
    family: &str,
    model: &T,
    opts: &MermansiOptions,
) -> Result<String> {
    let header = format!("[{family} semantic model]");
    let encoder = AnsiEncoder::new(opts.color_mode);
    let painted_header = encoder.paint(AnsiRole::SectionHeader, &header);
    let mut semantic = String::new();
    ensure_byte_capacity(0, painted_header.len().saturating_add(1))?;
    semantic.push_str(&painted_header);
    semantic.push('\n');

    let canonical = canonicalize(serde_json::to_value(model)?);
    let json_start = semantic.len();
    let (serialization, exceeded) = {
        let mut writer = BoundedStringWriter {
            output: &mut semantic,
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
    let escaped_json = escape_terminal_json_controls(&semantic[json_start..])?;
    semantic.truncate(json_start);
    ensure_byte_capacity(semantic.len(), escaped_json.len())?;
    semantic.push_str(&escaped_json);
    ensure_byte_capacity(semantic.len(), 1)?;
    semantic.push('\n');
    Ok(semantic)
}

fn escape_terminal_json_controls(input: &str) -> Result<String> {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')) || is_bidi_format_control(ch) {
            let escaped = format!("\\u{:04x}", ch as u32);
            ensure_byte_capacity(output.len(), escaped.len())?;
            output.push_str(&escaped);
        } else {
            ensure_byte_capacity(output.len(), ch.len_utf8())?;
            output.push(ch);
        }
    }
    Ok(output)
}

pub(crate) fn ensure_serialized_model_within_limit<T: Serialize>(model: &T) -> Result<()> {
    let (serialization, exceeded) = {
        let mut writer = BoundedCountWriter {
            written: 0,
            exceeded: None,
        };
        let serialization = serde_json::to_writer(&mut writer, model);
        (serialization, writer.exceeded)
    };
    if let Some(requested) = exceeded {
        return Err(MermansiError::RenderLimit {
            context: "semantic model bytes",
            requested,
            limit: MAX_OUTPUT_BYTES,
        });
    }
    serialization?;
    Ok(())
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

struct BoundedCountWriter {
    written: usize,
    exceeded: Option<usize>,
}

impl std::io::Write for BoundedCountWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let requested = self.written.saturating_add(buf.len());
        if requested > MAX_OUTPUT_BYTES {
            self.exceeded = Some(requested);
            return Err(std::io::Error::other("semantic model byte limit exceeded"));
        }
        self.written = requested;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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

pub(crate) fn validate_output(output: &str, preview: &str, opts: &MermansiOptions) -> Result<()> {
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(MermansiError::RenderLimit {
            context: "output bytes",
            requested: output.len(),
            limit: MAX_OUTPUT_BYTES,
        });
    }

    let mut rows = 0usize;
    let mut max_width = 0usize;
    for line in preview.lines() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn oversized_structured_model_is_rejected_before_preview_rendering() {
        let model = serde_json::json!({"label": "x".repeat(MAX_OUTPUT_BYTES)});
        let preview_called = Cell::new(false);

        let result = render_structured_adapter("json", &model, &MermansiOptions::unicode(), || {
            preview_called.set(true);
            Ok("unreachable preview".to_owned())
        });

        assert!(matches!(
            result,
            Err(MermansiError::RenderLimit {
                context: "semantic model bytes",
                ..
            })
        ));
        assert!(!preview_called.get());
    }

    #[test]
    fn concise_output_skips_an_unused_oversized_semantic_model() {
        let model = serde_json::json!({"label": "x".repeat(MAX_OUTPUT_BYTES)});
        let result = render_structured_adapter(
            "json",
            &model,
            &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
            || Ok("readable preview\n".to_owned()),
        );

        assert_eq!(result.unwrap().to_text().unwrap(), "readable preview\n");
    }

    #[test]
    fn concise_output_uses_the_semantic_model_when_no_preview_exists() {
        let model = serde_json::json!({"label": "kept"});
        let result = render_structured_adapter(
            "json",
            &model,
            &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
            || Ok(String::new()),
        )
        .unwrap();

        let result = result.to_text().unwrap();
        assert!(result.contains("[json semantic model]"));
        assert!(result.contains("kept"));
    }
}
