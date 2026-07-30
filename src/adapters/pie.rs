//! Pie chart adapter.

use crate::adapters::format_title;
use crate::ansi::sanitize_label_text;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use merman_core::diagrams::pie::PieDiagramRenderModel;

pub fn render_pie(model: &PieDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    if model.sections.is_empty() {
        out.push_str("(empty pie chart)\n");
        let _ = opts;
        return Ok(out);
    }

    let total: f64 = model.sections.iter().map(|s| s.value).sum();
    if total <= 0.0 {
        out.push_str("(pie chart has zero total)\n");
        let _ = opts;
        return Ok(out);
    }

    let bar_width = 30usize;
    out.push_str(&format!("{:<30} {:>10} {:>8}\n", "Label", "Value", "Share"));
    out.push_str(&"-".repeat(50));
    out.push('\n');

    for section in &model.sections {
        let pct = (section.value / total) * 100.0;
        let bar_len = ((section.value / total) * bar_width as f64).round() as usize;
        let bar_len = bar_len.min(bar_width);
        let bar_glyph = match opts.charset {
            Charset::Unicode => "█",
            Charset::Ascii => "#",
        };
        let bar: String = bar_glyph.repeat(bar_len);
        // Sanitize label text before formatting so terminal-control sequences cannot
        // affect Pie width calculation, column alignment, or bar geometry — in any
        // ColorMode. This is the primary defense layer (Rule 6).
        let label = sanitize_label_text(&section.label);
        out.push_str(&format!(
            "{:<30} {:>10.2} {:>7.1}% {bar}\n",
            label, section.value, pct
        ));
    }

    Ok(out)
}
