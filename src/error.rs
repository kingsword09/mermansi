//! Typed errors for `mermansi`.
//!
//! All fallible operations return [`Result<T>`](crate::error::Result). No function in the public
//! API or internal rendering pipeline panics.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, MermansiError>;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum MermansiError {
    /// A Mermaid parse error propagated from `merman-core`.
    #[error("Mermaid parse error: {0}")]
    Parse(#[from] merman_core::Error),

    /// A source recognized as raw JSON could not be decoded.
    #[error("JSON source parse error: {source}")]
    JsonSource {
        #[source]
        source: serde_json::Error,
    },

    /// A render error propagated from `merman-ascii`.
    #[error("Render error: {0}")]
    AsciiRender(#[from] merman_ascii::AsciiError),

    /// A typed semantic model could not be encoded for structured terminal output.
    #[error("structured render serialization error: {0}")]
    StructuredSerialization(#[from] serde_json::Error),

    /// The diagram family is not supported.
    #[error("unsupported diagram family: {family}")]
    UnsupportedFamily { family: String },

    /// A source, canvas, or output allocation exceeded its bounded limit.
    #[error("render limit exceeded: {context} (requested {requested}, limit {limit})")]
    RenderLimit {
        context: &'static str,
        requested: usize,
        limit: usize,
    },

    /// A canvas dimension overflowed.
    #[error("canvas overflow: requested {requested} cells, max {max}")]
    CanvasOverflow { requested: usize, max: usize },

    /// A parsed semantic model could not be arranged as terminal geometry.
    #[error("terminal geometry layout failed for {family}: {message}")]
    GeometryLayout {
        family: &'static str,
        message: String,
    },

    /// A zero-width grapheme could not be attached to a preceding canvas grapheme.
    #[error("invalid canvas grapheme placement: {message}")]
    CanvasGrapheme { message: &'static str },

    /// An invalid option was supplied.
    #[error("invalid option `{field}`: {message}")]
    InvalidOption {
        field: &'static str,
        message: &'static str,
    },
}
