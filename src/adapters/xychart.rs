//! XY chart terminal geometry backed by `merman-ascii` with display-width-aware categories.

use crate::adapters::to_ascii_options;
use crate::error::Result;
use crate::options::MermansiOptions;
use crate::str_display_width;
use merman_core::diagrams::xychart::{XyChartAxisRenderModel, XyChartDiagramRenderModel};

const DEFAULT_CATEGORY_BAND_WIDTH: usize = 3;
const AXIS_AND_TICK_BUDGET: usize = 16;

pub fn render_xychart(model: &XyChartDiagramRenderModel, opts: &MermansiOptions) -> Result<String> {
    let mut ascii_options = to_ascii_options(opts);
    if let XyChartAxisRenderModel::Band { categories, .. } = &model.x_axis {
        let category_count = categories.len().max(
            model
                .plots
                .iter()
                .map(|plot| plot.values.len())
                .max()
                .unwrap_or_default(),
        );
        let available = opts.max_width.saturating_sub(AXIS_AND_TICK_BUDGET);
        if let Some(maximum) = available
            .saturating_sub(category_count.saturating_sub(1))
            .checked_div(category_count)
        {
            let desired = categories
                .iter()
                .map(|category| str_display_width(category))
                .max()
                .unwrap_or_default()
                .max(DEFAULT_CATEGORY_BAND_WIDTH);
            ascii_options =
                ascii_options.with_xychart_category_band_width(desired.min(maximum.max(1)));
        }
    }

    merman_ascii::render_xychart(model, &ascii_options).map_err(Into::into)
}
