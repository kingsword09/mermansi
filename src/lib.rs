//! `mermansi` — production-quality pure Rust terminal renderer for Mermaid.
//!
//! This crate provides a single public source-to-text API for rendering Mermaid diagrams as
//! deterministic ASCII or Unicode terminal output. It uses [`merman_core`] as the sole
//! Mermaid parser and semantic source, reuses [`merman_ascii`] where it satisfies the
//! contract, and implements structured terminal adapters for remaining diagram families.
//!
//! ## Deterministic output guarantee
//!
//! Rendering the same Mermaid source text with the same options always produces byte-identical
//! output. No family silently drops parsed semantic entities, relationships, labels, endpoints,
//! markers, hierarchy, or chart values.
//!
//! ## Example
//!
//! ```
//! use mermansi::{render_source, MermansiOptions};
//!
//! let mermaid_text = "flowchart TD\n  A --> B";
//! let output = render_source(mermaid_text, &MermansiOptions::unicode()).unwrap();
//! assert!(!output.is_empty());
//! ```

pub mod adapters;
pub mod ansi;
pub mod canvas;
pub mod error;
mod input;
pub mod options;
mod output;

pub use ansi::{AnsiEncoder, AnsiRole};
pub use canvas::{Canvas, char_display_width, label_display_width, str_display_width};
pub use error::{MermansiError, Result};
pub use options::{Charset, ColorMode, MermansiOptions};

use merman_core::diagram::RenderSemanticModel;

/// Maximum source text length accepted by [`render_source`].
pub const MAX_SOURCE_BYTES: usize = options::MAX_SOURCE_BYTES;

/// Render a Mermaid source string or raw JSON object/array to terminal text.
///
/// Parses Mermaid with `merman_core::Engine` using strict options. Raw JSON is decoded directly
/// into [`RenderSemanticModel::Json`]. The resulting model is rendered via [`render_model`].
pub fn render_source(mermaid_text: &str, opts: &MermansiOptions) -> Result<String> {
    opts.validate()?;

    if mermaid_text.len() > MAX_SOURCE_BYTES {
        return Err(MermansiError::RenderLimit {
            context: "source bytes",
            requested: mermaid_text.len(),
            limit: MAX_SOURCE_BYTES,
        });
    }

    match input::parse_source_model(mermaid_text)? {
        Some(model) => render_model(&model, opts),
        None => Ok("(no diagram detected)\n".to_string()),
    }
}

/// Render a parsed [`RenderSemanticModel`] to terminal text.
pub fn render_model(model: &RenderSemanticModel, opts: &MermansiOptions) -> Result<String> {
    opts.validate()?;
    let mut result = adapters::render_model(model, opts)?;
    if result.is_empty() {
        result.push_str("(empty render output)\n");
    }
    // Rule 6: when ColorMode::Plain, no ANSI escape sequences — including any
    // that may be embedded in semantic label text — may appear in the final
    // output. Strip them before validation and return.
    if opts.color_mode == ColorMode::Plain {
        result = crate::ansi::strip_ansi(&result);
    }
    output::validate_output(&result, opts)?;
    Ok(result)
}

/// Convenience: render Mermaid source as ASCII text.
pub fn render_source_ascii(mermaid_text: &str) -> Result<String> {
    render_source(mermaid_text, &MermansiOptions::ascii())
}

/// Convenience: render Mermaid source as Unicode text.
pub fn render_source_unicode(mermaid_text: &str) -> Result<String> {
    render_source(mermaid_text, &MermansiOptions::unicode())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_flowchart() {
        let text = "flowchart TD\n  A --> B";
        let output = render_source(text, &MermansiOptions::unicode()).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn render_deterministic_unicode() {
        let text = "flowchart TD\n  A --> B";
        let a = render_source(text, &MermansiOptions::unicode()).unwrap();
        let b = render_source(text, &MermansiOptions::unicode()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn render_deterministic_ascii() {
        let text = "flowchart TD\n  A --> B";
        let a = render_source(text, &MermansiOptions::ascii()).unwrap();
        let b = render_source(text, &MermansiOptions::ascii()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn render_non_diagram_returns_parse_error() {
        // Non-diagram text like "hello world" triggers a DetectType parse error
        // from merman-core. This is expected behavior — the strict parser refuses
        // to guess a diagram type for unrecognised input.
        let result = render_source("hello world", &MermansiOptions::default());
        assert!(result.is_err());
    }
}
