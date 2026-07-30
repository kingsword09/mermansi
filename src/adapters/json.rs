//! JSON render model adapter.
//!
//! Renders a `serde_json::Value` as a readable indented structured text block.

use crate::error::Result;
use crate::options::MermansiOptions;
use crate::output::render_structured_model;
use serde_json::Value;

pub fn render_json(value: &Value, opts: &MermansiOptions) -> Result<String> {
    render_structured_model("json", value, opts)
}
