//! Pie chart adapter.

use crate::adapters::{align_left_display, align_right_display, format_title};
use crate::ansi::sanitize_label_text;
use crate::error::Result;
use crate::options::{Charset, MermansiOptions};
use crate::str_display_width;
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
    let bar_glyph = match opts.charset {
        Charset::Unicode => "█",
        Charset::Ascii => "#",
    };
    let rows = model
        .sections
        .iter()
        .map(|section| {
            let pct = (section.value / total) * 100.0;
            let bar_len = ((section.value / total) * bar_width as f64).round() as usize;
            (
                sanitize_label_text(&section.label),
                format!("{:.2}", section.value),
                format!("{pct:.1}%"),
                bar_glyph.repeat(bar_len.min(bar_width)),
            )
        })
        .collect::<Vec<_>>();
    let label_width = rows
        .iter()
        .map(|(label, _, _, _)| str_display_width(label))
        .max()
        .unwrap_or_default()
        .max(30);
    let value_width = rows
        .iter()
        .map(|(_, value, _, _)| str_display_width(value))
        .max()
        .unwrap_or_default()
        .max(10);
    let share_width = rows
        .iter()
        .map(|(_, _, share, _)| str_display_width(share))
        .max()
        .unwrap_or_default()
        .max(8);
    out.push_str(&format!(
        "{} {} {}\n",
        align_left_display("Label", label_width),
        align_right_display("Value", value_width),
        align_right_display("Share", share_width),
    ));
    out.push_str(&"-".repeat(label_width + value_width + share_width + 2));
    out.push('\n');

    for (label, value, share, bar) in rows {
        out.push_str(&format!(
            "{} {} {} {bar}\n",
            align_left_display(&label, label_width),
            align_right_display(&value, value_width),
            align_right_display(&share, share_width),
        ));
    }

    Ok(out)
}
