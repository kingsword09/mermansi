//! Render options for `mermansi`.
//!
//! [`MermansiOptions`] controls charset, color mode, output detail, and dimensions. It is
//! validated before use, and invalid values produce
//! [`MermansiError::InvalidOption`](crate::error::MermansiError::InvalidOption).

use crate::error::{MermansiError, Result};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    #[default]
    Unicode,
    Ascii,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Plain,
    Ansi16,
    TrueColor,
}

/// Controls whether rendered previews include the canonical semantic model.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Append the canonical semantic model after the readable terminal preview.
    #[default]
    Complete,
    /// Return only the readable preview, falling back to the semantic model when needed.
    Concise,
}

/// Maximum output width in terminal display columns.
pub const DEFAULT_MAX_WIDTH: usize = 200;
/// Maximum output height (rows).
pub const DEFAULT_MAX_HEIGHT: usize = 500;
/// Maximum total canvas cells (width * height).
pub const MAX_CANVAS_CELLS: usize = 250_000;
/// Maximum source text length in bytes.
pub const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum rendered output length in bytes.
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MermansiOptions {
    pub charset: Charset,
    pub color_mode: ColorMode,
    pub output_mode: OutputMode,
    pub max_width: usize,
    pub max_height: usize,
}

impl Default for MermansiOptions {
    fn default() -> Self {
        Self {
            charset: Charset::Unicode,
            color_mode: ColorMode::Plain,
            output_mode: OutputMode::Complete,
            max_width: DEFAULT_MAX_WIDTH,
            max_height: DEFAULT_MAX_HEIGHT,
        }
    }
}

impl MermansiOptions {
    pub fn unicode() -> Self {
        Self::default()
    }

    pub fn ascii() -> Self {
        Self {
            charset: Charset::Ascii,
            ..Self::default()
        }
    }

    pub fn with_color(mut self, color_mode: ColorMode) -> Self {
        self.color_mode = color_mode;
        self
    }

    pub fn with_output_mode(mut self, output_mode: OutputMode) -> Self {
        self.output_mode = output_mode;
        self
    }

    pub fn with_max_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    pub fn with_max_height(mut self, height: usize) -> Self {
        self.max_height = height;
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.max_width == 0 {
            return Err(MermansiError::InvalidOption {
                field: "max_width",
                message: "must be greater than 0",
            });
        }
        if self.max_height == 0 {
            return Err(MermansiError::InvalidOption {
                field: "max_height",
                message: "must be greater than 0",
            });
        }
        let cells = self.max_width.saturating_mul(self.max_height);
        if cells > MAX_CANVAS_CELLS {
            return Err(MermansiError::RenderLimit {
                context: "canvas cells",
                requested: cells,
                limit: MAX_CANVAS_CELLS,
            });
        }
        Ok(())
    }
}
