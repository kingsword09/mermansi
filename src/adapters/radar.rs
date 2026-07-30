//! Radar chart adapter.

use crate::adapters::format_title;
use crate::error::Result;
use crate::options::MermansiOptions;
use merman_core::diagrams::radar::RadarDiagramRenderModel;

pub fn render_radar(model: &RadarDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut out = String::new();
    out.push_str(&format_title(&model.title));

    if model.axes.is_empty() {
        out.push_str("(empty radar chart)\n");
        let _ = opts;
        return Ok(out);
    }

    out.push_str("Axes:\n");
    for axis in &model.axes {
        out.push_str(&format!("  {} = {}\n", axis.name, axis.label));
    }
    out.push('\n');

    if !model.curves.is_empty() {
        out.push_str("Curves:\n");
        for curve in &model.curves {
            out.push_str(&format!("  {} ({})\n", curve.name, curve.label));
            for (i, entry) in curve.entries.iter().enumerate() {
                let axis_name = model.axes.get(i).map(|a| a.name.as_str()).unwrap_or("?");
                out.push_str(&format!("    {axis_name}: {}\n", format_value(entry)));
            }
        }
    }

    let _ = opts;
    Ok(out)
}

fn format_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                format!("{f:.2}")
            } else {
                n.to_string()
            }
        }
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
