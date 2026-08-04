use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::xychart::{
    XyChartAxisRenderModel, XyChartDiagramRenderModel, XyChartDisplayPolicy,
    XyChartPlotRenderModel, XyChartPlotType,
};
use mermansi::{MermansiOptions, OutputMode, render_model, render_source, str_display_width};

fn concise_unicode(width: usize) -> MermansiOptions {
    MermansiOptions::unicode()
        .with_output_mode(OutputMode::Concise)
        .with_max_width(width)
}

#[test]
fn multiple_bar_series_have_distinct_grouped_geometry_and_exact_values() {
    let source = "xychart-beta\n\
        x-axis [A, B, C]\n\
        y-axis -3 --> 3\n\
        bar [1, 2, 3]\n\
        bar [3, 2, 1]";
    let output = render_source(source, &concise_unicode(80)).unwrap();
    let geometry = output.split_once("\nSeries values").unwrap().0;

    assert!(geometry.contains('█'), "first series is missing:\n{output}");
    assert!(
        geometry.contains('▓'),
        "second series is missing:\n{output}"
    );
    assert!(output.contains("█ Bar 1: A=1, B=2, C=3"), "{output}");
    assert!(output.contains("▓ Bar 2: A=3, B=2, C=1"), "{output}");
    assert!(output.lines().all(|line| str_display_width(line) <= 80));

    let collapsed = render_source(
        "xychart-beta\n  x-axis [A, B, C]\n  y-axis -3 --> 3\n  bar [3, 2, 3]",
        &concise_unicode(80),
    )
    .unwrap();
    assert_ne!(output, collapsed, "two series collapsed into one geometry");
}

#[test]
fn negative_and_positive_bars_grow_from_the_zero_axis() {
    let source = "xychart-beta\n\
        x-axis [A, B, C]\n\
        y-axis -3 --> 3\n\
        bar [-3, 0, 3]";
    let output = render_source(source, &concise_unicode(80)).unwrap();
    let geometry = output.split_once("\nSeries values").unwrap().0;
    let rows = geometry.lines().collect::<Vec<_>>();
    let zero_row = rows
        .iter()
        .position(|line| line.trim_start().starts_with("0 "))
        .unwrap_or_else(|| panic!("zero axis is missing:\n{output}"));

    assert!(
        rows[..zero_row].iter().any(|line| line.contains('█')),
        "{output}"
    );
    assert!(
        rows[zero_row + 1..].iter().any(|line| line.contains('█')),
        "{output}"
    );
    assert!(output.contains("A=-3, B=0, C=3"), "{output}");
}

#[test]
fn equal_explicit_axis_range_keeps_nonzero_bars_visible() {
    let source = "xychart-beta\n  x-axis [A]\n  y-axis 5 --> 5\n  bar [5]";
    let output = render_source(source, &concise_unicode(80)).unwrap();

    assert!(
        output.contains('█'),
        "bar disappeared on equal range:\n{output}"
    );
    assert!(output.contains("█ Bar 1: A=5"), "{output}");
    assert!(
        output
            .lines()
            .any(|line| line.trim_start().starts_with("0 ")),
        "{output}"
    );
}

#[test]
fn horizontal_ascii_bars_keep_each_series_and_sign() {
    let source = "xychart-beta horizontal\n\
        x-axis [A, B, C]\n\
        y-axis -3 --> 3\n\
        bar [-3, 0, 3]\n\
        bar [3, 2, 1]";
    let output = render_source(
        source,
        &MermansiOptions::ascii()
            .with_output_mode(OutputMode::Concise)
            .with_max_width(80),
    )
    .unwrap();

    assert!(output.contains("# Bar 1"), "{output}");
    assert!(output.contains("@ Bar 2"), "{output}");
    assert!(output.contains("A=-3, B=0, C=3"), "{output}");
    assert!(output.contains("A=3, B=2, C=1"), "{output}");
    assert!(output.lines().all(|line| line.is_ascii()), "{output}");
}

#[test]
fn horizontal_mixed_bars_keep_the_line_series_out_of_bar_rows() {
    let source = "xychart-beta horizontal\n\
        x-axis [A, B]\n\
        y-axis -3 --> 3\n\
        bar [-3, 2]\n\
        bar [2, -1]\n\
        line [0, 3]";
    let output = render_source(source, &concise_unicode(60)).unwrap();
    let geometry = output.split_once("\nSeries values").unwrap().0;
    let a_rows = geometry
        .lines()
        .filter(|line| line.starts_with('A'))
        .collect::<Vec<_>>();
    assert_eq!(
        a_rows.len(),
        1,
        "category labels must not duplicate rows:\n{output}"
    );
    assert!(
        geometry.contains("A █"),
        "first bar row is missing:\n{output}"
    );
    assert!(
        geometry.contains("▓▓"),
        "second bar series is missing:\n{output}"
    );
    assert!(
        geometry.contains("■■"),
        "line series row is missing:\n{output}"
    );
    assert!(output.contains("■ Line 1: A=0, B=3"), "{output}");
}

#[test]
fn empty_xychart_has_terminal_native_placeholder() {
    let output = render_source("xychart-beta\n", &concise_unicode(80)).unwrap();
    assert_eq!(output, "(empty XY chart)\n");
}

#[test]
fn non_finite_typed_values_are_disclosed_without_fake_geometry() {
    let values = [1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0];
    let finite_baseline = [1.0, 0.0, 0.0, 0.0, -1.0];
    let render = |values: &[f64]| {
        let model = XyChartDiagramRenderModel {
            orientation: String::new(),
            title: Some("Typed values".to_owned()),
            acc_title: None,
            acc_descr: None,
            x_axis: XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: ["A", "B", "C", "D", "E"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            },
            y_axis: XyChartAxisRenderModel::Linear {
                title: String::new(),
                min: Some(-1.0),
                max: Some(1.0),
            },
            plots: vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: Some("samples".to_owned()),
                values: values.to_vec(),
                data: Vec::new(),
            }],
            display: XyChartDisplayPolicy::default(),
        };
        render_model(&RenderSemanticModel::XyChart(model), &concise_unicode(80)).unwrap()
    };

    let output = render(&values);
    let baseline = render(&finite_baseline);
    assert_eq!(
        output.split_once("\nSeries values").unwrap().0,
        baseline.split_once("\nSeries values").unwrap().0,
        "non-finite values were projected onto fake chart coordinates:\n{output}"
    );
    for value in ["B=NaN", "C=inf", "D=-inf"] {
        assert!(output.contains(value), "missing {value}:\n{output}");
    }
}

#[test]
fn non_finite_typed_axis_bounds_fall_back_to_finite_data_range() {
    let model = XyChartDiagramRenderModel {
        orientation: "horizontal".to_owned(),
        title: None,
        acc_title: None,
        acc_descr: None,
        x_axis: XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_owned(), "B".to_owned()],
        },
        y_axis: XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(f64::NAN),
            max: Some(f64::INFINITY),
        },
        plots: vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Bar,
            title: None,
            values: vec![-2.0, 3.0],
            data: Vec::new(),
        }],
        display: XyChartDisplayPolicy::default(),
    };
    let output = render_model(&RenderSemanticModel::XyChart(model), &concise_unicode(60)).unwrap();

    assert!(output.contains("-2"), "{output}");
    assert!(output.contains('0'), "{output}");
    assert!(output.contains('3'), "{output}");
    assert!(output.lines().all(|line| str_display_width(line) <= 60));
}
