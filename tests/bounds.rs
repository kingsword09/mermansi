//! Bounded allocation and determinism tests.
//!
//! These tests verify that:
//! - Source size limits are enforced with typed errors.
//! - Canvas cell limits are enforced.
//! - Option validation rejects invalid values.
//! - Output is deterministic (same input → byte-identical output across repeated calls).
//! - Large inputs are bounded and return typed errors, not panics.

use merman_core::diagram::RenderSemanticModel;
use mermansi::ansi::strip_ansi;
use mermansi::canvas::Canvas;
use mermansi::options::{
    DEFAULT_MAX_HEIGHT, DEFAULT_MAX_WIDTH, MAX_CANVAS_CELLS, MAX_OUTPUT_BYTES,
};
use mermansi::{
    AnsiEncoder, AnsiRole, ColorMode, MAX_SOURCE_BYTES, MermansiError, MermansiOptions, OutputMode,
    render_model, render_source,
};

// ---------------------------------------------------------------------------
// Option validation
// ---------------------------------------------------------------------------

#[test]
fn zero_width_rejected() {
    let opts = MermansiOptions::default().with_max_width(0);
    assert!(matches!(
        render_source("flowchart TD\n  A --> B", &opts),
        Err(MermansiError::InvalidOption {
            field: "max_width",
            ..
        })
    ));
}

#[test]
fn zero_height_rejected() {
    let opts = MermansiOptions::default().with_max_height(0);
    assert!(matches!(
        render_source("flowchart TD\n  A --> B", &opts),
        Err(MermansiError::InvalidOption {
            field: "max_height",
            ..
        })
    ));
}

#[test]
fn excessive_canvas_cells_rejected() {
    // Request a width*height that exceeds MAX_CANVAS_CELLS.
    let opts = MermansiOptions::default()
        .with_max_width(5000)
        .with_max_height(5000);
    assert!(
        matches!(
            render_source("flowchart TD\n  A --> B", &opts),
            Err(MermansiError::RenderLimit { context, .. }) if context == "canvas cells"
        ),
        "expected canvas cell limit error for 25M cells"
    );
}

#[test]
fn default_options_are_valid() {
    let opts = MermansiOptions::default();
    opts.validate().expect("default options should be valid");
}

#[test]
fn default_dimensions_within_bounds() {
    let cells = DEFAULT_MAX_WIDTH * DEFAULT_MAX_HEIGHT;
    assert!(
        cells <= MAX_CANVAS_CELLS,
        "default canvas cells {cells} exceed limit {MAX_CANVAS_CELLS}"
    );
}

// ---------------------------------------------------------------------------
// Source size bound
// ---------------------------------------------------------------------------

#[test]
fn oversized_source_rejected() {
    // Build a source just over MAX_SOURCE_BYTES.
    let mut source = String::from("flowchart TD\n  A --> B\n");
    let padding_needed = MAX_SOURCE_BYTES + 1;
    // Fill with comments that don't break the diagram line count beyond limit.
    source.push_str(&"%% ".repeat(padding_needed / 3));
    // Pad to exceed the limit.
    while source.len() <= MAX_SOURCE_BYTES {
        source.push_str("%% padding comment that is safe for the lexer\n");
    }
    assert!(
        source.len() > MAX_SOURCE_BYTES,
        "test source should exceed MAX_SOURCE_BYTES ({MAX_SOURCE_BYTES}), got {}",
        source.len()
    );
    let result = render_source(&source, &MermansiOptions::unicode());
    assert!(
        matches!(
            &result,
            Err(MermansiError::RenderLimit { context, .. }) if *context == "source bytes"
        ),
        "expected source bytes limit error, got: {result:?}"
    );
}

#[test]
fn structured_output_width_is_bounded() {
    let model = RenderSemanticModel::Json(serde_json::json!({"label": "value"}));
    let result = render_model(&model, &MermansiOptions::unicode().with_max_width(10));
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "box geometry columns",
            ..
        })
    ));
}

#[test]
fn structured_output_height_is_bounded() {
    let model = RenderSemanticModel::Json(serde_json::json!({"label": "value"}));
    let result = render_model(&model, &MermansiOptions::unicode().with_max_height(1));
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "box geometry rows",
            ..
        })
    ));
}

#[test]
fn structured_output_bytes_are_bounded() {
    let model = RenderSemanticModel::Json(serde_json::json!({
        "label": "x".repeat(MAX_OUTPUT_BYTES)
    }));
    let result = render_model(&model, &MermansiOptions::unicode());
    assert!(
        matches!(
            &result,
            Err(MermansiError::RenderLimit {
                context: "semantic model bytes",
                ..
            })
        ),
        "expected semantic model byte limit, got {result:?}"
    );
}

#[test]
fn json_tree_depth_is_bounded_before_geometry_allocation() {
    let mut value = serde_json::Value::Null;
    for _ in 0..66 {
        value = serde_json::Value::Array(vec![value]);
    }

    let result = render_model(
        &RenderSemanticModel::Json(value),
        &MermansiOptions::unicode(),
    );
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "json tree depth",
            ..
        })
    ));
}

#[test]
fn json_tree_node_count_is_bounded_before_canvas_allocation() {
    let value = serde_json::Value::Array(vec![serde_json::Value::Null; 10_000]);
    let result = render_model(
        &RenderSemanticModel::Json(value),
        &MermansiOptions::unicode(),
    );
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "json tree nodes",
            requested: 10_001,
            limit: 10_000,
        })
    ));
}

#[test]
fn treeview_depth_is_bounded_before_geometry_allocation() {
    use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

    let mut child = TreeViewNodeRenderModel {
        id: 66,
        level: 66,
        name: "leaf".to_owned(),
        children: Vec::new(),
    };
    for level in (0..66).rev() {
        child = TreeViewNodeRenderModel {
            id: level,
            level,
            name: format!("level {level}"),
            children: vec![child],
        };
    }
    let model = TreeViewDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        root: TreeViewNodeRenderModel {
            children: vec![child],
            ..TreeViewNodeRenderModel::default()
        },
    };
    let result = render_model(
        &RenderSemanticModel::TreeView(model),
        &MermansiOptions::unicode(),
    );
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "treeView depth",
            ..
        })
    ));
}

#[test]
fn treeview_node_count_is_bounded_before_canvas_allocation() {
    use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

    let children = (0..4_097)
        .map(|id| TreeViewNodeRenderModel {
            id,
            level: 0,
            name: format!("node {id}"),
            children: Vec::new(),
        })
        .collect();
    let model = TreeViewDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        root: TreeViewNodeRenderModel {
            children,
            ..TreeViewNodeRenderModel::default()
        },
    };
    let result = render_model(
        &RenderSemanticModel::TreeView(model),
        &MermansiOptions::unicode(),
    );
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "treeView nodes",
            requested: 4_097,
            limit: 4_096,
        })
    ));
}

#[test]
fn native_kanban_timeline_and_journey_node_counts_are_bounded() {
    use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
    use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
    use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};

    let kanban = KanbanDiagramRenderModel {
        nodes: (0..4_097)
            .map(|index| KanbanRenderNode {
                id: format!("card-{index}"),
                label: "card".to_owned(),
                is_group: false,
                parent_id: None,
                ticket: None,
                priority: None,
                assigned: None,
                icon: None,
            })
            .collect(),
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::Kanban(kanban),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "kanban nodes",
            requested: 4_097,
            limit: 4_096,
        })
    ));

    let timeline = TimelineDiagramRenderModel {
        title: None,
        acc_title: None,
        acc_descr: None,
        sections: Vec::new(),
        tasks: (0..4_097)
            .map(|id| TimelineRenderTask {
                id,
                section: String::new(),
                task_type: String::new(),
                task: "period".to_owned(),
                score: 0,
                events: Vec::new(),
            })
            .collect(),
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::Timeline(timeline),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "timeline nodes",
            requested: 4_097,
            limit: 4_096,
        })
    ));

    let journey = JourneyDiagramRenderModel {
        title: None,
        acc_title: None,
        acc_descr: None,
        sections: Vec::new(),
        tasks: (0..4_097)
            .map(|_| JourneyRenderTask {
                score: 0,
                score_is_nan: false,
                people: Vec::new(),
                section: String::new(),
                task_type: String::new(),
                task: "task".to_owned(),
            })
            .collect(),
        actors: Vec::new(),
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::Journey(journey),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "journey nodes",
            requested: 4_097,
            limit: 4_096,
        })
    ));
}

#[test]
fn sankey_treemap_ishikawa_eventmodeling_and_info_are_preflight_bounded() {
    use merman_core::diagrams::eventmodeling::{
        EventModelingDataEntityRenderModel, EventModelingDiagramRenderModel,
    };
    use merman_core::diagrams::info::InfoDiagramRenderModel;
    use merman_core::diagrams::ishikawa::{IshikawaDiagramRenderModel, IshikawaNodeRenderModel};
    use merman_core::diagrams::sankey::{
        SankeyDiagramRenderModel, SankeyRenderGraph, SankeyRenderNode,
    };
    use merman_core::diagrams::treemap::{TreemapDiagramRenderModel, TreemapNodeRenderModel};

    let sankey = SankeyDiagramRenderModel {
        graph: SankeyRenderGraph {
            nodes: (0..4_097)
                .map(|index| SankeyRenderNode {
                    id: format!("node-{index}"),
                })
                .collect(),
            links: Vec::new(),
        },
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::Sankey(sankey),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "sankey nodes",
            requested: 4_097,
            limit: 4_096,
        })
    ));

    let mut treemap_node = TreemapNodeRenderModel {
        name: "leaf".to_owned(),
        children: None,
        value: None,
        class_selector: None,
        css_compiled_styles: None,
    };
    for depth in 0..65 {
        treemap_node = TreemapNodeRenderModel {
            name: format!("depth-{depth}"),
            children: Some(vec![treemap_node]),
            value: None,
            class_selector: None,
            css_compiled_styles: None,
        };
    }
    let treemap = TreemapDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        root: treemap_node,
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::Treemap(treemap),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "treemap depth",
            requested: 65,
            limit: 64,
        })
    ));

    let mut cause = IshikawaNodeRenderModel {
        text: "leaf".to_owned(),
        children: Vec::new(),
    };
    for depth in 0..65 {
        cause = IshikawaNodeRenderModel {
            text: format!("depth-{depth}"),
            children: vec![cause],
        };
    }
    let ishikawa = IshikawaDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        root: Some(cause),
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::Ishikawa(ishikawa),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "ishikawa depth",
            requested: 65,
            limit: 64,
        })
    ));

    let eventmodeling = EventModelingDiagramRenderModel {
        data_entities: (0..4_097)
            .map(|index| EventModelingDataEntityRenderModel {
                name: format!("data-{index}"),
                data_block_value: "{}".to_owned(),
            })
            .collect(),
        ..EventModelingDiagramRenderModel::default()
    };
    assert!(matches!(
        render_model(
            &RenderSemanticModel::EventModeling(eventmodeling),
            &MermansiOptions::unicode()
        ),
        Err(MermansiError::RenderLimit {
            context: "eventmodeling nodes",
            requested: 4_097,
            limit: 4_096,
        })
    ));

    let info = RenderSemanticModel::Info(InfoDiagramRenderModel { show_info: true });
    assert!(matches!(
        render_model(&info, &MermansiOptions::unicode().with_max_width(10)),
        Err(MermansiError::RenderLimit {
            context: "info columns",
            ..
        })
    ));
}

#[test]
fn five_native_geometry_adapters_reject_impossible_narrow_layouts() {
    let sources = [
        include_str!("fixtures/sankey.en.mmd"),
        include_str!("fixtures/treemap.en.mmd"),
        include_str!("fixtures/ishikawa.en.mmd"),
        include_str!("fixtures/eventmodeling.en.mmd"),
        include_str!("fixtures/info.en.mmd"),
    ];
    let options = MermansiOptions::unicode()
        .with_output_mode(OutputMode::Concise)
        .with_max_width(10)
        .with_max_height(10);
    for source in sources {
        let result = render_source(source, &options);
        assert!(
            matches!(
                result,
                Err(MermansiError::RenderLimit { .. }) | Err(MermansiError::GeometryLayout { .. })
            ),
            "narrow layout unexpectedly rendered: {result:?}"
        );
    }
}

#[test]
fn delegated_model_is_bounded_before_ascii_rendering() {
    let engine = merman_core::Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\n  participant A",
            merman_core::ParseOptions::strict(),
        )
        .expect("sequence source should parse")
        .expect("sequence source should produce a model");
    let RenderSemanticModel::Sequence(mut model) = parsed.model else {
        panic!("expected sequence render model");
    };
    model
        .actors
        .get_mut("A")
        .expect("participant A should exist")
        .description = "x".repeat(MAX_OUTPUT_BYTES);

    let result = render_model(
        &RenderSemanticModel::Sequence(model),
        &MermansiOptions::unicode(),
    );
    assert!(
        matches!(
            &result,
            Err(MermansiError::RenderLimit {
                context: "semantic model bytes",
                ..
            })
        ),
        "expected semantic model byte limit, got {result:?}"
    );
}

#[test]
fn flowchart_edge_lane_dimensions_are_bounded_before_canvas_creation() {
    let wide = "flowchart TD\n  A -->|a very long first parallel edge label| B\n  A -->|a very long second parallel edge label| B";
    let width_result = render_source(wide, &MermansiOptions::unicode().with_max_width(40));
    assert!(matches!(
        width_result,
        Err(MermansiError::RenderLimit {
            context: "flowchart preview columns",
            ..
        })
    ));

    let tall = "flowchart LR\n  A --> B\n  A --> B\n  A --> B\n  A --> B";
    let height_result = render_source(tall, &MermansiOptions::unicode().with_max_height(10));
    assert!(matches!(
        height_result,
        Err(MermansiError::RenderLimit {
            context: "flowchart preview rows",
            ..
        })
    ));
}

// ---------------------------------------------------------------------------
// Canvas bounds
// ---------------------------------------------------------------------------

#[test]
fn canvas_creation_enforces_max_cells() {
    // This should exceed MAX_CELLS.
    let result = Canvas::new(100_000, 100_000);
    assert!(matches!(result, Err(MermansiError::CanvasOverflow { .. })));
}

#[test]
fn canvas_dimension_overflow_returns_typed_error() {
    let result = Canvas::new(usize::MAX, 2);
    assert!(matches!(result, Err(MermansiError::CanvasOverflow { .. })));
}

#[test]
fn canvas_creation_within_bounds() {
    let canvas = Canvas::new(10, 10).expect("10x10 canvas should be valid");
    assert_eq!(canvas.width(), 10);
    assert_eq!(canvas.height(), 10);
}

#[test]
fn canvas_set_text_works() {
    let mut canvas = Canvas::new(20, 5).expect("canvas");
    canvas.set_text(0, 0, "Hello").expect("set_text");
    assert_eq!(canvas.get_cell(0, 0), Some("H"));
    assert_eq!(canvas.get_cell(4, 0), Some("o"));
}

#[test]
fn canvas_set_text_with_chinese() {
    let mut canvas = Canvas::new(20, 5).expect("canvas");
    canvas.set_text(0, 0, "你好").expect("set_text with CJK");
    // '你' occupies columns 0-1 (wide char), '好' occupies columns 2-3.
    assert_eq!(canvas.get_cell(0, 0), Some("你"));
    assert_eq!(canvas.continuation_owner(1, 0), Some((0, 0)));
    assert_eq!(canvas.get_cell(2, 0), Some("好"));
    assert_eq!(canvas.continuation_owner(3, 0), Some((2, 0)));
}

#[test]
fn canvas_styled_text_applies_to_every_grapheme_owner() {
    let encoder = AnsiEncoder::new(ColorMode::Ansi16);
    let prefix = encoder.prefix(AnsiRole::NodeText);
    let suffix = encoder.suffix();
    let text = "A中👩‍💻e\u{301}";
    let mut canvas = Canvas::new(10, 1).expect("canvas");
    canvas
        .set_styled_text(0, 0, text, prefix, suffix)
        .expect("styled text");

    let rendered = canvas.render();
    assert_eq!(rendered.matches(prefix).count(), 4, "{rendered:?}");
    assert_eq!(rendered.matches(suffix).count(), 4, "{rendered:?}");
    assert_eq!(strip_ansi(&rendered), format!("{text}\n"));
    assert_eq!(canvas.continuation_owner(2, 0), Some((1, 0)));
    assert_eq!(canvas.continuation_owner(4, 0), Some((3, 0)));
}

#[test]
fn canvas_styled_combining_append_styles_its_existing_owner() {
    let encoder = AnsiEncoder::new(ColorMode::TrueColor);
    let prefix = encoder.prefix(AnsiRole::EdgeLabel);
    let suffix = encoder.suffix();
    let mut canvas = Canvas::new(3, 1).expect("canvas");
    canvas.set_text(0, 0, "e").expect("base grapheme");
    canvas
        .set_styled_text(1, 0, "\u{301}", prefix, suffix)
        .expect("combining append");

    let rendered = canvas.render();
    assert_eq!(rendered.matches(prefix).count(), 1, "{rendered:?}");
    assert_eq!(strip_ansi(&rendered), "e\u{301}\n");
}

#[test]
fn canvas_stores_decomposed_grapheme_as_one_cell() {
    let mut canvas = Canvas::new(5, 1).expect("canvas");
    canvas
        .set_text(0, 0, "e\u{301}")
        .expect("decomposed grapheme");
    assert_eq!(canvas.get_cell(0, 0), Some("e\u{301}"));
    assert_eq!(canvas.get_cell(1, 0), Some(""));

    canvas
        .set_text(1, 0, "\u{308}")
        .expect("standalone combining mark attaches left");
    assert_eq!(canvas.get_cell(0, 0), Some("e\u{301}\u{308}"));
}

#[test]
fn canvas_tracks_emoji_grapheme_continuation() {
    let mut canvas = Canvas::new(5, 1).expect("canvas");
    canvas.set_text(0, 0, "👩‍💻").expect("emoji grapheme");
    assert_eq!(canvas.get_cell(0, 0), Some("👩‍💻"));
    assert_eq!(canvas.continuation_owner(1, 0), Some((0, 0)));
}

#[test]
fn overwriting_continuation_clears_the_whole_old_grapheme() {
    let mut canvas = Canvas::new(4, 1).expect("canvas");
    canvas.set_text(0, 0, "你").expect("wide grapheme");
    canvas.set_text(1, 0, "A").expect("replacement");
    assert_eq!(canvas.get_cell(0, 0), Some(""));
    assert_eq!(canvas.get_cell(1, 0), Some("A"));
    assert_eq!(canvas.continuation_owner(1, 0), None);
}

#[test]
fn right_edge_failure_is_atomic() {
    let mut canvas = Canvas::new(4, 1).expect("canvas");
    canvas.set_text(0, 0, "OK").expect("initial text");
    let before = canvas.clone();
    let result = canvas.set_text(1, 0, "你好");
    assert!(matches!(result, Err(MermansiError::CanvasOverflow { .. })));
    assert_eq!(canvas, before);
}

#[test]
fn canvas_box_drawing_works() {
    use mermansi::Charset;
    use mermansi::canvas::draw_box;
    let mut canvas = Canvas::new(20, 10).expect("canvas");
    draw_box(&mut canvas, 2, 2, 8, 4, Charset::Unicode).expect("draw_box");
    assert_eq!(canvas.get_cell(2, 2), Some("┌"));
    assert_eq!(canvas.get_cell(9, 2), Some("┐"));
    assert_eq!(canvas.get_cell(2, 5), Some("└"));
    assert_eq!(canvas.get_cell(9, 5), Some("┘"));
}

#[test]
fn canvas_merges_crossing_strokes_independent_of_order() {
    use mermansi::Charset;
    use mermansi::canvas::{draw_horizontal_line, draw_vertical_line};

    let mut horizontal_first = Canvas::new(5, 5).expect("canvas");
    draw_horizontal_line(&mut horizontal_first, 2, 0, 4, Charset::Unicode).expect("horizontal");
    draw_vertical_line(&mut horizontal_first, 2, 0, 4, Charset::Unicode).expect("vertical");

    let mut vertical_first = Canvas::new(5, 5).expect("canvas");
    draw_vertical_line(&mut vertical_first, 2, 0, 4, Charset::Unicode).expect("vertical");
    draw_horizontal_line(&mut vertical_first, 2, 0, 4, Charset::Unicode).expect("horizontal");

    assert_eq!(horizontal_first.get_cell(2, 2), Some("┼"));
    assert_eq!(horizontal_first, vertical_first);
}

#[test]
fn canvas_ascii_strokes_merge_with_ascii_glyphs() {
    use mermansi::Charset;
    use mermansi::canvas::{draw_horizontal_line, draw_vertical_line};
    let mut canvas = Canvas::new(3, 3).expect("canvas");
    draw_horizontal_line(&mut canvas, 1, 0, 2, Charset::Ascii).expect("horizontal");
    draw_vertical_line(&mut canvas, 1, 0, 2, Charset::Ascii).expect("vertical");
    assert_eq!(canvas.get_cell(1, 1), Some("+"));
    assert!(canvas.render().is_ascii());
}

#[test]
fn text_has_priority_over_strokes() {
    use mermansi::Charset;
    use mermansi::canvas::draw_horizontal_line;
    let mut canvas = Canvas::new(5, 1).expect("canvas");
    draw_horizontal_line(&mut canvas, 0, 0, 4, Charset::Unicode).expect("line");
    canvas.set_text(2, 0, "X").expect("text");
    draw_horizontal_line(&mut canvas, 0, 0, 4, Charset::Unicode).expect("second line");
    assert_eq!(canvas.get_cell(2, 0), Some("X"));
}

#[test]
fn canvas_overflow_on_out_of_bounds_set() {
    let mut canvas = Canvas::new(5, 5).expect("canvas");
    // Position beyond the canvas bounds.
    let result = canvas.set_char(10, 10, 'X');
    assert!(matches!(result, Err(MermansiError::CanvasOverflow { .. })));
}

// ---------------------------------------------------------------------------
// Determinism across repeated calls
// ---------------------------------------------------------------------------

#[test]
fn deterministic_across_repeated_calls() {
    let source = "flowchart TD\n  A --> B\n  B --> C\n  C --> D";
    for _ in 0..5 {
        let a = render_source(source, &MermansiOptions::unicode()).unwrap();
        let b = render_source(source, &MermansiOptions::unicode()).unwrap();
        assert_eq!(a, b, "output should be identical across repeated calls");
    }
}

#[test]
fn deterministic_ascii_across_repeated_calls() {
    let source = "flowchart TD\n  A --> B\n  B --> C";
    for _ in 0..5 {
        let a = render_source(source, &MermansiOptions::ascii()).unwrap();
        let b = render_source(source, &MermansiOptions::ascii()).unwrap();
        assert_eq!(a, b);
    }
}

#[test]
fn flowchart_edge_lane_geometry_is_deterministic() {
    let source = "flowchart TD\n  A[开始] -->|first| B[结束]\n  A -->|second| B\n  B -->|again| B";
    for options in [MermansiOptions::unicode(), MermansiOptions::ascii()] {
        let first = render_source(source, &options).unwrap();
        for _ in 0..4 {
            assert_eq!(render_source(source, &options).unwrap(), first);
        }
    }
}

#[test]
fn flowchart_edge_lane_canvas_emits_roles_without_changing_geometry() {
    let source = "flowchart TD\n  A[开始] -->|first| B[结束]\n  A -->|second| B";
    let plain = render_source(source, &MermansiOptions::unicode()).unwrap();
    for mode in [ColorMode::Ansi16, ColorMode::TrueColor] {
        let encoder = AnsiEncoder::new(mode);
        let colored = render_source(source, &MermansiOptions::unicode().with_color(mode)).unwrap();
        assert!(colored.contains(encoder.prefix(AnsiRole::NodeText)));
        assert!(colored.contains(encoder.prefix(AnsiRole::EdgeLabel)));
        assert!(colored.contains(encoder.prefix(AnsiRole::EdgeArrow)));
        assert_eq!(strip_ansi(&colored), plain);
    }
}

#[test]
fn deterministic_for_sequence() {
    let source = "sequenceDiagram\n  A->>B: Hello\n  B-->>A: World";
    let a = render_source(source, &MermansiOptions::unicode()).unwrap();
    let b = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn deterministic_for_pie() {
    let source = "pie title Test\n  \"X\" : 10\n  \"Y\" : 20";
    let a = render_source(source, &MermansiOptions::unicode()).unwrap();
    let b = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn deterministic_for_state() {
    let source = "stateDiagram-v2\n  [*] --> Active\n  Active --> [*]";
    let a = render_source(source, &MermansiOptions::unicode()).unwrap();
    let b = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Non-diagram input produces error (no silent empty output)
// ---------------------------------------------------------------------------

#[test]
fn empty_string_produces_error_not_empty_output() {
    let result = render_source("", &MermansiOptions::unicode());
    assert!(
        result.is_err(),
        "empty input should produce a parse error, not empty output"
    );
}

// ---------------------------------------------------------------------------
// Chart geometry determinism and bounds
// ---------------------------------------------------------------------------

#[test]
fn deterministic_for_radar() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{4,3,5}";
    let a = render_source(source, &MermansiOptions::unicode()).unwrap();
    let b = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn deterministic_for_quadrant() {
    let source = "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4\n  A: [0.5, 0.5]";
    let a = render_source(source, &MermansiOptions::unicode()).unwrap();
    let b = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn deterministic_for_venn() {
    let source = "venn-beta\n  set A\n  set B\n  union A,B";
    let a = render_source(source, &MermansiOptions::unicode()).unwrap();
    let b = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn chart_narrow_width_does_not_panic() {
    let opts = MermansiOptions::unicode().with_max_width(40);
    let sources = [
        "pie title Narrow\n  \"A\" : 10\n  \"B\" : 20",
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4",
        "venn-beta\n  set A\n  set B\n  union A,B",
    ];
    for source in &sources {
        let result = render_source(source, &opts);
        assert!(
            result.is_ok(),
            "narrow width render failed for: {source}\n{result:?}"
        );
    }
}

#[test]
fn chart_dimensions_never_exceed_requested_limits() {
    let sources = [
        "pie title Bounds\n  \"A\" : 10\n  \"B\" : 20",
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4",
        "venn-beta\n  set A\n  set B\n  union A,B",
    ];

    for source in sources {
        let width_result = render_source(
            source,
            &MermansiOptions::unicode()
                .with_output_mode(OutputMode::Concise)
                .with_max_width(19)
                .with_max_height(50),
        );
        assert!(
            matches!(
                width_result,
                Err(MermansiError::RenderLimit {
                    context: "chart width",
                    requested: 20,
                    limit: 19,
                })
            ),
            "expected a pre-allocation width error for {source:?}, got {width_result:?}"
        );

        let height_result = render_source(
            source,
            &MermansiOptions::unicode()
                .with_output_mode(OutputMode::Concise)
                .with_max_width(80)
                .with_max_height(9),
        );
        assert!(
            matches!(
                height_result,
                Err(MermansiError::RenderLimit {
                    context: "chart height",
                    requested: 10,
                    limit: 9,
                })
            ),
            "expected a pre-allocation height error for {source:?}, got {height_result:?}"
        );
    }
}

#[test]
fn chart_ascii_mode_produces_nonempty_output() {
    let sources = [
        "pie title ASCII\n  \"A\" : 10\n  \"B\" : 20",
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4",
        "venn-beta\n  set A\n  set B\n  union A,B",
    ];
    for source in &sources {
        let output = render_source(source, &MermansiOptions::ascii()).unwrap();
        assert!(
            !output.trim().is_empty(),
            "ASCII output is empty for: {source}"
        );
    }
}

#[test]
fn chart_concise_mode_produces_geometry() {
    let opts = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let sources = [
        "pie title Concise\n  \"A\" : 10\n  \"B\" : 20",
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4",
        "venn-beta\n  set A\n  set B\n  union A,B",
    ];
    for source in &sources {
        let output = render_source(source, &opts).unwrap();
        assert!(
            !output.trim().is_empty(),
            "Concise output is empty for: {source}"
        );
        // In concise mode, the semantic model should NOT appear (no structured text fallback).
        assert!(
            !output.contains("semantic model]"),
            "Concise mode should not include structured-text fallback for: {source}\n{output}"
        );
    }
}

// ---------------------------------------------------------------------------
// Constructed-model bounds and empty/zero/invalid tests
// ---------------------------------------------------------------------------

#[test]
fn pie_empty_constructed_model_produces_nonempty_output() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::pie::PieDiagramRenderModel;

    let model = PieDiagramRenderModel {
        show_data: false,
        title: Some("Empty".to_string()),
        acc_title: None,
        acc_descr: None,
        sections: vec![],
    };
    let output = render_model(
        &RenderSemanticModel::Pie(model),
        &MermansiOptions::unicode(),
    );
    assert!(
        output.is_ok(),
        "Empty pie model should not error: {:?}",
        output
    );
    let out = output.unwrap();
    assert!(
        !out.trim().is_empty(),
        "Empty pie model should produce nonempty output"
    );
}

#[test]
fn pie_zero_total_constructed_model_produces_nonempty_output() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::pie::{PieDiagramRenderModel, PieRenderSection};

    let model = PieDiagramRenderModel {
        show_data: false,
        title: Some("Zero".to_string()),
        acc_title: None,
        acc_descr: None,
        sections: vec![
            PieRenderSection {
                label: "A".to_string(),
                value: 0.0,
            },
            PieRenderSection {
                label: "B".to_string(),
                value: 0.0,
            },
        ],
    };
    let output = render_model(
        &RenderSemanticModel::Pie(model),
        &MermansiOptions::unicode(),
    );
    assert!(output.is_ok(), "Zero-total pie should not error");
    let out = output.unwrap();
    assert!(!out.trim().is_empty());
}

#[test]
fn pie_nan_inf_values_filtered_safely() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::pie::{PieDiagramRenderModel, PieRenderSection};

    let model = PieDiagramRenderModel {
        show_data: false,
        title: Some("Invalid".to_string()),
        acc_title: None,
        acc_descr: None,
        sections: vec![
            PieRenderSection {
                label: "NaN".to_string(),
                value: f64::NAN,
            },
            PieRenderSection {
                label: "Inf".to_string(),
                value: f64::INFINITY,
            },
            PieRenderSection {
                label: "Valid".to_string(),
                value: 10.0,
            },
            PieRenderSection {
                label: "Neg".to_string(),
                value: -5.0,
            },
        ],
    };
    let output = render_model(
        &RenderSemanticModel::Pie(model),
        &MermansiOptions::unicode(),
    );
    assert!(output.is_ok(), "Invalid values should not error");
    let out = output.unwrap();
    assert!(
        out.contains("Valid"),
        "Valid section should survive filtering"
    );
}

#[test]
fn radar_empty_constructed_model_produces_nonempty_output() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::radar::RadarDiagramRenderModel;

    let model = RadarDiagramRenderModel::default();
    let output = render_model(
        &RenderSemanticModel::Radar(model),
        &MermansiOptions::unicode(),
    );
    assert!(output.is_ok(), "Empty radar model should not error");
    let out = output.unwrap();
    assert!(!out.trim().is_empty());
}

#[test]
fn quadrant_empty_constructed_model_produces_nonempty_output() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::quadrant_chart::{
        QuadrantChartAxesModel, QuadrantChartQuadrantsModel, QuadrantChartRenderModel,
    };

    let model = QuadrantChartRenderModel {
        title: Some("Empty".to_string()),
        acc_title: None,
        acc_descr: None,
        quadrants: QuadrantChartQuadrantsModel {
            quadrant1_text: "Q1".to_string(),
            quadrant2_text: "Q2".to_string(),
            quadrant3_text: "Q3".to_string(),
            quadrant4_text: "Q4".to_string(),
        },
        axes: QuadrantChartAxesModel {
            x_axis_left_text: String::new(),
            x_axis_right_text: String::new(),
            y_axis_bottom_text: String::new(),
            y_axis_top_text: String::new(),
        },
        points: vec![],
        classes: Default::default(),
    };
    let output = render_model(
        &RenderSemanticModel::QuadrantChart(model),
        &MermansiOptions::unicode(),
    );
    assert!(output.is_ok(), "Empty quadrant model should not error");
    let out = output.unwrap();
    assert!(!out.trim().is_empty());
}

#[test]
fn venn_empty_constructed_model_produces_nonempty_output() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::venn::VennDiagramRenderModel;

    let model = VennDiagramRenderModel::default();
    let output = render_model(
        &RenderSemanticModel::Venn(model),
        &MermansiOptions::unicode(),
    );
    assert!(output.is_ok(), "Empty venn model should not error");
    let out = output.unwrap();
    assert!(!out.trim().is_empty());
}

#[test]
fn chart_entity_limit_enforced_for_pie() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::pie::{PieDiagramRenderModel, PieRenderSection};

    let sections: Vec<PieRenderSection> = (0..5000)
        .map(|i| PieRenderSection {
            label: format!("S{i}"),
            value: 1.0,
        })
        .collect();
    let model = PieDiagramRenderModel {
        show_data: false,
        title: None,
        acc_title: None,
        acc_descr: None,
        sections,
    };
    let result = render_model(
        &RenderSemanticModel::Pie(model),
        &MermansiOptions::unicode(),
    );
    assert!(
        matches!(&result, Err(MermansiError::RenderLimit { context, .. }) if *context == "pie sections"),
        "Expected pie sections entity limit error, got: {result:?}"
    );
}

#[test]
fn chart_entity_limit_enforced_for_radar() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::radar::{RadarDiagramRenderModel, RadarRenderAxis};

    let axes: Vec<RadarRenderAxis> = (0..5000)
        .map(|i| RadarRenderAxis {
            name: format!("A{i}"),
            label: format!("A{i}"),
        })
        .collect();
    let model = RadarDiagramRenderModel {
        title: None,
        acc_title: None,
        acc_descr: None,
        axes,
        curves: vec![],
        options: Default::default(),
    };
    let result = render_model(
        &RenderSemanticModel::Radar(model),
        &MermansiOptions::unicode(),
    );
    assert!(
        matches!(&result, Err(MermansiError::RenderLimit { context, .. }) if *context == "radar axes"),
        "Expected radar axes entity limit error, got: {result:?}"
    );
}

#[test]
fn radar_curve_entry_limit_enforced_before_geometry() {
    use merman_core::diagrams::radar::{
        RadarDiagramRenderModel, RadarRenderAxis, RadarRenderCurve,
    };

    let model = RadarDiagramRenderModel {
        axes: vec![RadarRenderAxis {
            name: "A".to_owned(),
            label: "A".to_owned(),
        }],
        curves: vec![RadarRenderCurve {
            name: "oversized".to_owned(),
            label: "oversized".to_owned(),
            entries: vec![serde_json::json!(1); 4_097],
        }],
        ..RadarDiagramRenderModel::default()
    };
    let result = render_model(
        &RenderSemanticModel::Radar(model),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    );
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "radar curve entries",
            requested: 4_097,
            limit: 4_096,
        })
    ));
}

#[test]
fn chart_entity_limit_enforced_for_quadrant() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::quadrant_chart::{
        QuadrantChartAxesModel, QuadrantChartPointModel, QuadrantChartQuadrantsModel,
        QuadrantChartRenderModel, QuadrantChartStyles,
    };

    let points: Vec<QuadrantChartPointModel> = (0..5000)
        .map(|i| QuadrantChartPointModel {
            text: format!("P{i}"),
            x: 0.5,
            y: 0.5,
            class_name: None,
            styles: QuadrantChartStyles::default(),
        })
        .collect();
    let model = QuadrantChartRenderModel {
        title: None,
        acc_title: None,
        acc_descr: None,
        quadrants: QuadrantChartQuadrantsModel {
            quadrant1_text: "Q1".to_string(),
            quadrant2_text: "Q2".to_string(),
            quadrant3_text: "Q3".to_string(),
            quadrant4_text: "Q4".to_string(),
        },
        axes: QuadrantChartAxesModel {
            x_axis_left_text: String::new(),
            x_axis_right_text: String::new(),
            y_axis_bottom_text: String::new(),
            y_axis_top_text: String::new(),
        },
        points,
        classes: Default::default(),
    };
    let result = render_model(
        &RenderSemanticModel::QuadrantChart(model),
        &MermansiOptions::unicode(),
    );
    assert!(
        matches!(&result, Err(MermansiError::RenderLimit { context, .. }) if *context == "quadrant points"),
        "Expected quadrant points entity limit error, got: {result:?}"
    );
}

#[test]
fn chart_entity_limit_enforced_for_venn() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::venn::{VennDiagramRenderModel, VennSubsetRenderModel};

    let subsets: Vec<VennSubsetRenderModel> = (0..5000)
        .map(|i| VennSubsetRenderModel {
            sets: vec![format!("S{i}")],
            size: 10.0,
            label: None,
        })
        .collect();
    let model = VennDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        subsets,
        text_nodes: vec![],
        style_entries: vec![],
    };
    let result = render_model(
        &RenderSemanticModel::Venn(model),
        &MermansiOptions::unicode(),
    );
    assert!(
        matches!(&result, Err(MermansiError::RenderLimit { context, .. }) if *context == "venn subsets"),
        "Expected venn subsets entity limit error, got: {result:?}"
    );
}

#[test]
fn chart_geometry_is_deterministic_across_calls() {
    let sources = [
        "pie title Det\n  \"A\" : 30\n  \"B\" : 70",
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4\n  P: [0.5, 0.5]",
        "venn-beta\n  set A\n  set B\n  union A,B",
    ];
    for source in &sources {
        let a = render_source(source, &MermansiOptions::unicode()).unwrap();
        let b = render_source(source, &MermansiOptions::unicode()).unwrap();
        assert_eq!(a, b, "Non-deterministic output for: {source}");
    }
}
