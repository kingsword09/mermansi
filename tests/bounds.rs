//! Bounded allocation and determinism tests.
//!
//! These tests verify that:
//! - Source size limits are enforced with typed errors.
//! - Canvas cell limits are enforced.
//! - Option validation rejects invalid values.
//! - Output is deterministic (same input → byte-identical output across repeated calls).
//! - Large inputs are bounded and return typed errors, not panics.

use merman_core::diagram::RenderSemanticModel;
use mermansi::canvas::Canvas;
use mermansi::options::{
    DEFAULT_MAX_HEIGHT, DEFAULT_MAX_WIDTH, MAX_CANVAS_CELLS, MAX_OUTPUT_BYTES,
};
use mermansi::{MAX_SOURCE_BYTES, MermansiError, MermansiOptions, render_model, render_source};

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
            context: "output columns",
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
            context: "output rows",
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
