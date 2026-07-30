//! Info diagram adapter.

use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::info::InfoDiagramRenderModel;

pub fn render_info(model: &InfoDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();

    if model.show_info {
        out.push_str("info: showInfo\n");
    } else {
        out.push_str("info:\n");
    }

    if out.trim().is_empty() {
        out.push_str("(empty info diagram)\n");
    }

    let _ = opts;
    Ok(out)
}
