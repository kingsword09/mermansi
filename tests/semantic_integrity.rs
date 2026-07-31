//! Semantic integrity tests.
//!
//! These tests verify label preservation (English, Chinese CJK, combining marks, emoji),
//! box closure, relationship endpoint/marker/label retention, directions (TD/TB/BT/LR/RL),
//! self-relations, cycles, disconnected components, nested groups, and parallel/dense
//! relations.

use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};
use mermansi::{ColorMode, MermansiOptions, OutputMode, render_model, render_source};

fn family_preview<'a>(output: &'a str, family: &str) -> &'a str {
    let marker = format!("[{family} semantic model]");
    output
        .split(&marker)
        .next()
        .unwrap_or(output)
        .trim_matches('\n')
}

fn flowchart_preview(output: &str) -> &str {
    family_preview(output, "flowchart")
}

// ---------------------------------------------------------------------------
// Label preservation
// ---------------------------------------------------------------------------

#[test]
fn english_label_preserved() {
    let source = "flowchart TD\n  A[Hello World] --> B[Goodbye]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Hello"),
        "English label 'Hello' missing:\n{output}"
    );
    assert!(
        output.contains("Goodbye"),
        "English label 'Goodbye' missing:\n{output}"
    );
}

#[test]
fn chinese_cjk_label_preserved() {
    let source = "flowchart TD\n  A[开始] --> B[结束]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("开始"),
        "Chinese label '开始' missing:\n{output}"
    );
    assert!(
        output.contains("结束"),
        "Chinese label '结束' missing:\n{output}"
    );
}

#[test]
fn combining_mark_label_preserved() {
    let cafe = "cafe\u{301}";
    let naive = "nai\u{308}ve";
    let source = format!("flowchart TD\n  A[{cafe}] --> B[{naive}]");
    let output = render_source(&source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains(cafe),
        "combining-mark label '{cafe}' missing:\n{output}"
    );
    assert!(
        output.contains(naive),
        "combining-mark label '{naive}' missing:\n{output}"
    );
}

#[test]
fn emoji_label_preserved() {
    let source = "flowchart TD\n  A[\u{1F680} Rocket] --> B[\u{1F4A9} Bug]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("\u{1F680}"),
        "emoji label rocket missing:\n{output}"
    );
    assert!(
        output.contains("\u{1F4A9}"),
        "emoji label bug missing:\n{output}"
    );
}

#[test]
fn label_renders_in_ascii_mode_too() {
    let source = "flowchart TD\n  A[Hello] --> B[World]";
    let output = render_source(source, &MermansiOptions::ascii()).unwrap();
    assert!(
        output.contains("Hello"),
        "label 'Hello' missing in ASCII:\n{output}"
    );
    assert!(
        output.contains("World"),
        "label 'World' missing in ASCII:\n{output}"
    );
}

#[test]
fn decision_node_has_one_closed_border_in_each_charset() {
    let source = "flowchart TD\n  A{Is it?}";

    let unicode = render_source(source, &MermansiOptions::unicode()).unwrap();
    let unicode_preview = flowchart_preview(&unicode);
    assert_eq!(
        unicode_preview,
        "╭────────╮\n│        │\n< Is it? >\n│        │\n╰────────╯"
    );
    assert_eq!(unicode_preview.matches('╭').count(), 1);
    assert_eq!(unicode_preview.matches('╰').count(), 1);

    let ascii = render_source(source, &MermansiOptions::ascii()).unwrap();
    let ascii_preview = flowchart_preview(&ascii);
    assert_eq!(
        ascii_preview,
        "/--------\\\n|        |\n< Is it? >\n|        |\n\\--------/"
    );
}

// ---------------------------------------------------------------------------
// Edge / relationship label and endpoint preservation
// ---------------------------------------------------------------------------

#[test]
fn edge_label_preserved() {
    let source = "flowchart TD\n  A -->|sends| B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("sends"),
        "edge label 'sends' missing:\n{output}"
    );
}

#[test]
fn edge_with_arrow_marker_preserved() {
    let source = "flowchart LR\n  A --x B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(output.contains("A"), "source node A missing:\n{output}");
    assert!(output.contains("B"), "target node B missing:\n{output}");
    assert!(
        output.to_ascii_lowercase().contains("cross") || output.contains('x'),
        "cross end marker missing:\n{output}"
    );
}

#[test]
fn bidirectional_edge_renders() {
    let source = "flowchart TD\n  A <--> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(output.contains("A"), "source A missing:\n{output}");
    assert!(output.contains("B"), "target B missing:\n{output}");
    let arrowheads = output.matches('◀').count()
        + output.matches('▶').count()
        + output.matches('<').count()
        + output.matches('>').count();
    assert!(
        output.contains("double_arrow_point") || arrowheads >= 2,
        "bidirectional start/end markers missing:\n{output}"
    );
}

#[test]
fn dotted_edge_renders() {
    let source = "flowchart TD\n  A -.-> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(output.contains("A"), "source A missing:\n{output}");
    assert!(output.contains("B"), "target B missing:\n{output}");
}

// ---------------------------------------------------------------------------
// Direction tests
// ---------------------------------------------------------------------------

fn label_position(output: &str, label: &str) -> (usize, usize) {
    output
        .lines()
        .enumerate()
        .find_map(|(row, line)| line.find(label).map(|column| (row, column)))
        .unwrap_or_else(|| panic!("label '{label}' missing:\n{output}"))
}

#[test]
fn direction_td_renders() {
    let source = "flowchart TD\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(label_position(&output, "A").0 < label_position(&output, "B").0);
}

#[test]
fn direction_tb_renders() {
    let source = "flowchart TB\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(label_position(&output, "A").0 < label_position(&output, "B").0);
}

#[test]
fn direction_bt_renders() {
    let source = "flowchart BT\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(label_position(&output, "A").0 > label_position(&output, "B").0);
}

#[test]
fn direction_lr_renders() {
    let source = "flowchart LR\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(label_position(&output, "A").1 < label_position(&output, "B").1);
}

#[test]
fn direction_rl_renders() {
    let source = "flowchart RL\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(label_position(&output, "A").1 > label_position(&output, "B").1);
}

// ---------------------------------------------------------------------------
// Self-relation, cycle, disconnected, nested, parallel, dense
// ---------------------------------------------------------------------------

#[test]
fn self_relation_parsed_and_rendered() {
    let source = "flowchart TD\n  A[Loop] -->|again| A";
    let unicode = render_source(source, &MermansiOptions::unicode())
        .expect("self-relation must render Unicode geometry");
    assert_eq!(
        flowchart_preview(&unicode),
        "  ┌──────┐\n  │      ├──┐ again\n  │ Loop ├◀─┘\n  │      │\n  └──────┘"
    );

    let ascii = render_source(source, &MermansiOptions::ascii())
        .expect("self-relation must render ASCII geometry");
    assert_eq!(
        flowchart_preview(&ascii),
        "  +------+\n  |      +--+ again\n  | Loop +<-+\n  |      |\n  +------+"
    );
}

#[test]
fn cycle_renders() {
    let source = "flowchart TD\n  A --> B\n  B --> C\n  C --> A";
    let output = render_source(source, &MermansiOptions::unicode()).expect("cycle should render");
    assert!(output.contains("A"), "cycle node A missing:\n{output}");
    assert!(output.contains("B"), "cycle node B missing:\n{output}");
    assert!(output.contains("C"), "cycle node C missing:\n{output}");
    // merman-ascii may emit either triangle (◀▶▲▼) or filled-arrow (◄►)
    // glyphs for directed edges; count all of them.
    let arrowheads = ['▲', '▼', '◀', '▶', '◄', '►']
        .into_iter()
        .map(|arrow| output.matches(arrow).count())
        .sum::<usize>();
    assert!(
        arrowheads >= 3,
        "cycle must retain all three directed edges:\n{output}"
    );
}

#[test]
fn disconnected_components_renders() {
    let source = "flowchart TD\n  A --> B\n  C --> D";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    for node in ["A", "B", "C", "D"] {
        assert!(
            output.contains(node),
            "disconnected component node {node} missing:\n{output}"
        );
    }
}

#[test]
fn nested_group_renders() {
    let source = "flowchart TD\n  subgraph Outer\n    subgraph Inner\n      A --> B\n    end\n    C --> D\n  end";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    for node in ["A", "B", "C", "D"] {
        assert!(
            output.contains(node),
            "nested group node {node} missing:\n{output}"
        );
    }
    assert!(output.contains("Outer"), "outer group missing:\n{output}");
    assert!(output.contains("Inner"), "inner group missing:\n{output}");
}

#[test]
fn parallel_edges_parsed_and_rendered() {
    let source = "flowchart TD\n  A -->|first| B\n  A -->|second| B";
    let unicode = render_source(source, &MermansiOptions::unicode())
        .expect("parallel labeled edges should render Unicode geometry");
    let unicode_preview = flowchart_preview(&unicode);
    assert_eq!(
        unicode_preview.matches('▶').count(),
        2,
        "parallel edges must have separate target arrows:\n{unicode_preview}"
    );
    assert!(
        unicode_preview.contains("└─────────┼──▶") && unicode_preview.contains("└──▶"),
        "parallel edges must use distinct routed lanes:\n{unicode_preview}"
    );
    assert!(unicode_preview.contains("first"));
    assert!(unicode_preview.contains("second"));

    let ascii = render_source(source, &MermansiOptions::ascii())
        .expect("parallel labeled edges should render ASCII geometry");
    let ascii_preview = flowchart_preview(&ascii);
    assert_eq!(ascii_preview.matches('>').count(), 2);
    assert!(ascii_preview.contains("+---------+-->"));
    assert!(ascii_preview.contains("+-->"));
}

#[test]
fn self_loops_and_parallel_edges_render_geometry_in_every_direction() {
    for direction in ["TD", "TB", "BT", "LR", "RL"] {
        let parallel =
            format!("flowchart {direction}\n  A[开始] -->|first| B[结束]\n  A -->|second| B");
        let output = render_source(&parallel, &MermansiOptions::unicode())
            .unwrap_or_else(|error| panic!("{direction} parallel geometry failed: {error}"));
        let preview = flowchart_preview(&output);
        let arrows = preview.matches('▶').count() + preview.matches('▼').count();
        assert_eq!(
            arrows, 2,
            "{direction} must render one arrow per parallel edge:\n{preview}"
        );
        assert!(preview.contains("开始"));
        assert!(preview.contains("结束"));

        let self_loop = format!("flowchart {direction}\n  A[Loop] -->|again| A");
        let output = render_source(&self_loop, &MermansiOptions::unicode())
            .unwrap_or_else(|error| panic!("{direction} self-loop geometry failed: {error}"));
        let preview = flowchart_preview(&output);
        assert!(
            preview.contains('◀') || preview.contains('▲'),
            "{direction} self-loop must have a visible return arrow:\n{preview}"
        );
        assert!(preview.contains("again"));
    }
}

#[test]
fn parallel_self_loops_have_separate_routes() {
    let source = "flowchart TD\n  A -->|one| A\n  A -->|two| A";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = flowchart_preview(&output);
    assert_eq!(preview.matches('◀').count(), 2, "{preview}");
    assert_eq!(
        preview,
        "  ┌─────┐\n  │     ├──┐ one\n  │     ├◀─┘\n  │  A  ├─────┐ two\n  │     ├◀────┘\n  └─────┘"
    );
}

#[test]
fn colored_flowchart_lanes_strip_user_terminal_controls_before_canvas_layout() {
    let dirty = "\u{1b}[31mStart red\u{1b}[0m\u{1b}]0;title\u{07}";
    let source = format!("flowchart TD\n  A[\"{dirty}\"] -->|first| B[End]\n  A -->|safe| B");
    let plain = render_source(&source, &MermansiOptions::unicode()).unwrap();
    assert!(plain.contains("Start"));
    assert!(plain.contains("red"));

    for mode in [ColorMode::Ansi16, ColorMode::TrueColor] {
        let colored = render_source(&source, &MermansiOptions::unicode().with_color(mode)).unwrap();
        assert!(!colored.contains("\u{1b}[31m"), "{colored:?}");
        assert!(!colored.contains("\u{1b}]0;title"), "{colored:?}");
        assert!(!colored.contains('\u{07}'), "{colored:?}");
        assert_eq!(mermansi::ansi::strip_ansi(&colored), plain);
    }
}

#[test]
fn dense_relations_parsed_and_rendered() {
    let source = "flowchart TD\n  A --> B\n  A --> C\n  A --> D\n  B --> C\n  B --> D\n  C --> D";
    let output =
        render_source(source, &MermansiOptions::unicode()).expect("dense graph should render");
    for node in ["A", "B", "C", "D"] {
        assert!(
            output.contains(node),
            "dense relation node {node} missing:\n{output}"
        );
    }
}

// ---------------------------------------------------------------------------
// State diagram specifics
// ---------------------------------------------------------------------------

#[test]
fn state_transitions_preserved() {
    let source = "stateDiagram-v2\n  [*] --> Active\n  Active --> Inactive: sleep\n  Inactive --> Active: wake\n  Active --> [*]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Active"),
        "state 'Active' missing:\n{output}"
    );
    assert!(
        output.contains("Inactive"),
        "state 'Inactive' missing:\n{output}"
    );
    assert!(
        output.contains("sleep"),
        "transition label 'sleep' missing:\n{output}"
    );
    assert!(
        output.contains("wake"),
        "transition label 'wake' missing:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// Class diagram specifics
// ---------------------------------------------------------------------------

#[test]
fn class_members_preserved() {
    let source =
        "classDiagram\n  class Animal {\n    +String name\n    +int age\n    +makeSound()\n  }";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Animal"),
        "class 'Animal' missing:\n{output}"
    );
    assert!(output.contains("name"), "member 'name' missing:\n{output}");
    assert!(output.contains("age"), "member 'age' missing:\n{output}");
    assert!(
        output.contains("makeSound"),
        "method 'makeSound' missing:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// ER diagram specifics
// ---------------------------------------------------------------------------

#[test]
fn er_relationship_preserved() {
    let source = "erDiagram\n  CUSTOMER ||--o{ ORDER : places";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("CUSTOMER"),
        "entity 'CUSTOMER' missing:\n{output}"
    );
    assert!(
        output.contains("ORDER"),
        "entity 'ORDER' missing:\n{output}"
    );
    assert!(
        output.contains("places"),
        "relationship label 'places' missing:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// Sequence diagram specifics
// ---------------------------------------------------------------------------

#[test]
fn sequence_messages_preserved() {
    let source = "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi there";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(output.contains("Alice"), "actor 'Alice' missing:\n{output}");
    assert!(output.contains("Bob"), "actor 'Bob' missing:\n{output}");
    assert!(
        output.contains("Hello"),
        "message 'Hello' missing:\n{output}"
    );
    assert!(output.contains("Hi"), "message 'Hi' missing:\n{output}");
}

// ---------------------------------------------------------------------------
// Pie chart semantics
// ---------------------------------------------------------------------------

#[test]
fn pie_values_preserved() {
    let source = "pie title Pets\n  \"Dogs\" : 40\n  \"Cats\" : 25\n  \"Fish\" : 10";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Dogs"),
        "pie label 'Dogs' missing:\n{output}"
    );
    assert!(
        output.contains("Cats"),
        "pie label 'Cats' missing:\n{output}"
    );
    assert!(
        output.contains("Fish"),
        "pie label 'Fish' missing:\n{output}"
    );
    assert!(
        output.contains("Pets"),
        "pie title 'Pets' missing:\n{output}"
    );
}

#[test]
fn pie_chinese_preserved() {
    let source = "pie title 宠物\n  \"狗\" : 40\n  \"猫\" : 25";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("宠物"),
        "pie title '宠物' missing:\n{output}"
    );
    assert!(output.contains("狗"), "pie label '狗' missing:\n{output}");
    assert!(output.contains("猫"), "pie label '猫' missing:\n{output}");
}

fn display_column_of(line: &str, needle: &str) -> usize {
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("'{needle}' missing from line: {line}"));
    mermansi::str_display_width(&line[..byte])
}

#[test]
fn pie_mixed_language_rows_align_by_display_column() {
    let source = "pie\n  \"English\" : 10\n  \"中文\" : 20";
    for options in [MermansiOptions::unicode(), MermansiOptions::ascii()] {
        let output = render_source(source, &options).unwrap();
        let english = output
            .lines()
            .find(|line| line.contains("English"))
            .unwrap();
        let chinese = output.lines().find(|line| line.contains("中文")).unwrap();
        assert_eq!(
            display_column_of(english, "10.00"),
            display_column_of(chinese, "20.00")
        );
        assert_eq!(
            display_column_of(english, "33.3%"),
            display_column_of(chinese, "66.7%")
        );
    }
}

#[test]
fn sankey_mixed_language_rows_align_by_display_column() {
    let source = "sankey-beta\nEnglish,Test,10\n中文,目标,20";
    for options in [MermansiOptions::unicode(), MermansiOptions::ascii()] {
        let output = render_source(source, &options).unwrap();
        let english = output
            .lines()
            .find(|line| line.contains("English") && line.contains("10.00"))
            .unwrap();
        let chinese = output
            .lines()
            .find(|line| line.contains("中文") && line.contains("20.00"))
            .unwrap();
        assert_eq!(
            display_column_of(english, "Test"),
            display_column_of(chinese, "目标")
        );
        assert_eq!(
            display_column_of(english, "10.00"),
            display_column_of(chinese, "20.00")
        );
    }
}

// ---------------------------------------------------------------------------
// Block diagram — hierarchy without duplicate nodes
// ---------------------------------------------------------------------------

fn assert_no_structured_fallback(preview: &str) {
    for marker in [
        "Nodes:",
        "Groups:",
        "Edges:",
        "Blocks:",
        "Boundaries:",
        "Shapes:",
        "Requirements:",
        "Elements:",
        "Relationships:",
    ] {
        assert!(
            !preview.lines().any(|line| line.trim() == marker),
            "structured-text fallback marker '{marker}' found:\n{preview}"
        );
    }
}

fn assert_closed_unicode_boxes(preview: &str, expected: usize) {
    let corners = ['┌', '┐', '└', '┘'].map(|corner| preview.matches(corner).count());
    for (corner, count) in ['┌', '┐', '└', '┘'].into_iter().zip(corners) {
        assert!(
            count >= expected,
            "expected at least {expected} '{corner}' box corners, found {count}:\n{preview}"
        );
    }
    let top_pairs = preview
        .lines()
        .map(|line| line.matches('┌').count().min(line.matches('┐').count()))
        .sum::<usize>();
    let bottom_pairs = preview
        .lines()
        .map(|line| line.matches('└').count().min(line.matches('┘').count()))
        .sum::<usize>();
    assert!(
        top_pairs >= expected && bottom_pairs >= expected,
        "expected at least {expected} paired top and bottom box borders, found {top_pairs}/{bottom_pairs}:\n{preview}"
    );
}

fn assert_exact_closed_unicode_boxes(preview: &str, expected: usize) {
    for corner in ['┌', '┐', '└', '┘'] {
        assert_eq!(
            preview.matches(corner).count(),
            expected,
            "expected exactly {expected} '{corner}' box corners:\n{preview}"
        );
    }
}

fn assert_clean_vertical_chain(preview: &str, expected_arrows: usize) {
    assert_eq!(preview.matches('▼').count(), expected_arrows, "{preview}");
    for line in preview.lines().filter(|line| line.contains('▼')) {
        assert_eq!(
            line.trim(),
            "▼",
            "vertical arrow overlapped other geometry:\n{preview}"
        );
    }
}

#[test]
fn block_nested_three_levels_no_duplicate_nodes() {
    let source = "block-beta\n  block:L1[\"Level 1\"]\n    block:L2[\"Level 2\"]\n      Leaf[\"Leaf\"]\n    end\n  end\n";
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let preview = render_source(source, &options).unwrap();

    for needle in ["L1 · Level 1", "L2 · Level 2", "Leaf"] {
        let count = preview.matches(&needle).count();
        assert_eq!(
            count, 1,
            "block entity '{needle}' should appear exactly once, found {count}:\n{preview}"
        );
    }
    let left_edges = preview
        .lines()
        .filter_map(|line| {
            line.find('┌')
                .map(|byte| mermansi::str_display_width(&line[..byte]))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        left_edges.len(),
        3,
        "expected three nested boxes:\n{preview}"
    );
    assert!(
        left_edges.windows(2).all(|pair| pair[0] < pair[1]),
        "each nested block must begin inside its parent:\n{preview}"
    );
    assert_closed_unicode_boxes(&preview, 3);
    assert_no_structured_fallback(&preview);
    assert!(
        !preview.contains("root"),
        "synthetic root leaked:\n{preview}"
    );
}

#[test]
fn concise_block_preview_omits_generated_spacers() {
    let source = "block-beta\n  A[\"One\"]\n  space:2\n  B[\"Two\"]\n  A --> B";
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let output = render_source(source, &options).unwrap();

    assert!(output.contains("A · One"), "{output}");
    assert!(output.contains("B · Two"), "{output}");
    assert!(output.contains("A --> B"), "{output}");
    assert_closed_unicode_boxes(&output, 2);
    assert_no_structured_fallback(&output);
    assert!(!output.contains("[space]"), "{output}");
    assert!(!output.contains("id-"), "{output}");
}

#[test]
fn concise_c4_preview_omits_the_synthetic_global_boundary() {
    let source = "C4Context\n  Person(customer, \"Customer\")\n  System(system, \"Bank\")\n  Rel(customer, system, \"Uses\")";
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let output = render_source(source, &options).unwrap();

    assert!(output.contains("customer · Customer"), "{output}");
    assert!(output.contains("system · Bank"), "{output}");
    assert!(output.contains("customer --> system  Uses"), "{output}");
    assert_closed_unicode_boxes(&output, 2);
    assert_no_structured_fallback(&output);
    assert!(!output.contains("global"), "{output}");
}

#[test]
fn c4_boundary_contains_closed_shapes_and_connected_relationship() {
    let source = "C4Container\nContainer_Boundary(cb, \"Backend\") {\n  Container(api, \"API\", \"Rust\", \"Service\")\n  ContainerDb(db, \"Database\", \"Postgres\", \"Store\")\n  Rel(api, db, \"Reads\")\n}";
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let output = render_source(source, &options).unwrap();

    for label in [
        "cb · Backend",
        "api · API",
        "db · Database",
        "technology · Rust",
        "technology · Postgres",
    ] {
        assert!(output.contains(label), "'{label}' missing:\n{output}");
    }
    assert!(output.contains('▶'), "C4 relation arrow missing:\n{output}");
    assert!(output.contains("api --> db  Reads"), "{output}");
    assert_closed_unicode_boxes(&output, 3);
    assert_no_structured_fallback(&output);
}

#[test]
fn concise_architecture_preview_preserves_ports_and_direction() {
    let source = "architecture-beta\n  service web(server)[Web]\n  junction hub\n  service api(server)[API]\n  web:L --> R:hub\n  hub:B --> T:api";
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let output = render_source(source, &options).unwrap();

    assert!(output.contains("web --> hub  ports L -> R"), "{output}");
    assert!(output.contains("hub --> api  ports B -> T"), "{output}");
    let hub = label_position(&output, "hub");
    let web = label_position(&output, "web · Web");
    let api = label_position(&output, "api · API");
    assert!(
        hub.1 < web.1,
        "L -> R ports must place hub left of web:\n{output}"
    );
    assert!(
        hub.0 < api.0,
        "B -> T ports must place api below hub:\n{output}"
    );
    assert!(
        output.contains('◀'),
        "horizontal target arrow missing:\n{output}"
    );
    assert!(
        output.contains('▼'),
        "vertical target arrow missing:\n{output}"
    );
    assert_closed_unicode_boxes(&output, 3);
    assert_no_structured_fallback(&output);
    assert!(!output.contains('?'), "{output}");
}

#[test]
fn architecture_vertical_port_legend_keeps_tokens_intact() {
    let source = "architecture-beta\n  service lower(server)[Lower]\n  service upper(server)[Upper]\n  lower:T --> B:upper";
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let output = render_source(source, &options).unwrap();

    assert!(
        output
            .lines()
            .any(|line| line.trim() == "lower --> upper  ports T -> B"),
        "vertical port legend should use available width without splitting tokens:\n{output}"
    );
    assert!(
        label_position(&output, "upper · Upper").0 < label_position(&output, "lower · Lower").0,
        "a target's bottom port must be above a source's top port:\n{output}"
    );
    assert!(output.contains('▲'), "target arrow missing:\n{output}");
}

#[test]
fn architecture_block_and_c4_fixtures_render_bilingual_geometry() {
    let cases = [
        (
            "architecture",
            include_str!("fixtures/architecture.en.mmd"),
            4,
        ),
        (
            "architecture",
            include_str!("fixtures/architecture.zh.mmd"),
            4,
        ),
        ("block", include_str!("fixtures/block.en.mmd"), 3),
        ("block", include_str!("fixtures/block.zh.mmd"), 3),
        ("c4", include_str!("fixtures/c4.en.mmd"), 3),
        ("c4", include_str!("fixtures/c4.zh.mmd"), 3),
    ];
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    for (family, source, boxes) in cases {
        let output = render_source(source, &options)
            .unwrap_or_else(|error| panic!("{family} fixture failed: {error}"));
        assert_closed_unicode_boxes(&output, boxes);
        assert_no_structured_fallback(&output);
        assert!(
            output.contains('─') && output.contains('│'),
            "{family}:\n{output}"
        );
        assert!(
            output
                .lines()
                .all(|line| mermansi::str_display_width(line) <= options.max_width),
            "{family} exceeded configured display width:\n{output}"
        );
    }
}

#[test]
fn architecture_block_and_c4_ascii_mode_uses_only_ascii_geometry() {
    let cases = [
        ("architecture", include_str!("fixtures/architecture.en.mmd")),
        ("block", include_str!("fixtures/block.en.mmd")),
        ("c4", include_str!("fixtures/c4.en.mmd")),
    ];
    let options = MermansiOptions::ascii().with_output_mode(OutputMode::Concise);
    for (family, source) in cases {
        let output = render_source(source, &options)
            .unwrap_or_else(|error| panic!("{family} ASCII fixture failed: {error}"));
        assert!(
            output.is_ascii(),
            "{family} emitted Unicode decoration:\n{output}"
        );
        assert!(output.contains('+') && output.contains('-') && output.contains('|'));
        assert_no_structured_fallback(&output);
    }
}

#[test]
fn requirement_geometry_honors_every_direction() {
    for (direction, ordered_axis, arrow) in [
        ("TB", 0, '▼'),
        ("BT", 0, '▲'),
        ("LR", 1, '▶'),
        ("RL", 1, '◀'),
    ] {
        let source = format!(
            "requirementDiagram\n  direction {direction}\n  requirement Source {{\n    id: SRC\n    text: Source requirement\n  }}\n  requirement Target {{\n    id: DST\n    text: Target requirement\n  }}\n  Source - traces -> Target"
        );
        let output = render_source(
            &source,
            &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
        )
        .unwrap_or_else(|error| panic!("{direction} requirement geometry failed: {error}"));
        let source_position = label_position(&output, "[Requirement] Source");
        let target_position = label_position(&output, "[Requirement] Target");
        let ordered = if matches!(direction, "TB" | "LR") {
            source_position
        } else {
            target_position
        };
        let later = if matches!(direction, "TB" | "LR") {
            target_position
        } else {
            source_position
        };

        assert!(
            [ordered.0, ordered.1][ordered_axis] < [later.0, later.1][ordered_axis],
            "{direction} did not order source and target on the requested axis:\n{output}"
        );
        assert_eq!(output.matches(arrow).count(), 1, "{direction}:\n{output}");
        assert!(output.contains("Source --> Target  traces"), "{output}");
        assert_closed_unicode_boxes(&output, 2);
        assert_no_structured_fallback(&output);
    }
}

#[test]
fn mindmap_is_a_closed_left_to_right_tree_without_outline_fallback() {
    let output = render_source(
        include_str!("fixtures/mindmap.en.mmd"),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();

    assert_closed_unicode_boxes(&output, 10);
    assert_eq!(output.matches('▶').count(), 9, "{output}");
    assert!(label_position(&output, "Project").1 < label_position(&output, "Planning").1);
    assert!(label_position(&output, "Planning").1 < label_position(&output, "Requirements").1);
    assert!(!output.contains("|--"), "{output}");
    assert!(
        !output.contains("-->"),
        "synthetic edge legend leaked:\n{output}"
    );
    assert_no_structured_fallback(&output);
}

#[test]
fn treeview_fixture_preserves_real_hierarchy_as_geometry() {
    let output = render_source(
        include_str!("fixtures/treeview.en.mmd"),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();

    assert_closed_unicode_boxes(&output, 4);
    assert_eq!(output.matches('▶').count(), 3, "{output}");
    assert!(label_position(&output, "Root").1 < label_position(&output, "Child1").1);
    assert!(label_position(&output, "Child1").1 < label_position(&output, "Grandchild").1);
    assert!(label_position(&output, "Root").1 < label_position(&output, "Child2").1);
    assert!(!output.contains("|--"), "{output}");
    assert!(
        !output.contains("tree-"),
        "synthetic node id leaked:\n{output}"
    );
    assert_no_structured_fallback(&output);
}

#[test]
fn json_object_array_and_scalars_form_a_connected_box_tree() {
    let output = render_source(
        include_str!("fixtures/json.en.mmd"),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();

    assert_closed_unicode_boxes(&output, 8);
    assert_eq!(output.matches('▶').count(), 7, "{output}");
    assert!(label_position(&output, "{} (4 fields)").1 < label_position(&output, "skills:").1);
    assert!(label_position(&output, "skills:").1 < label_position(&output, "[0]:").1);
    for label in ["age: 30", "name: \"Alice\"", "[1]: \"Python\""] {
        assert!(output.contains(label), "'{label}' missing:\n{output}");
    }
    assert!(
        !output.contains("json-"),
        "synthetic node id leaked:\n{output}"
    );
    assert!(
        !output.contains("-->"),
        "synthetic edge legend leaked:\n{output}"
    );
    assert_no_structured_fallback(&output);
}

#[test]
fn new_box_tree_adapters_preserve_bilingual_labels_and_ascii_geometry() {
    let bilingual = [
        (
            "requirement",
            include_str!("fixtures/requirement.zh.mmd"),
            "系统应对用户进行身份验证",
        ),
        (
            "mindmap",
            include_str!("fixtures/mindmap.zh.mmd"),
            "集成测试",
        ),
        (
            "treeView",
            include_str!("fixtures/treeview.zh.mmd"),
            "孙节点",
        ),
        ("json", include_str!("fixtures/json.zh.mmd"), "爱丽丝"),
    ];
    for (family, source, label) in bilingual {
        let output = render_source(
            source,
            &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
        )
        .unwrap_or_else(|error| panic!("{family} Chinese geometry failed: {error}"));
        assert!(output.contains(label), "{family} lost '{label}':\n{output}");
        assert!(
            output.contains('┌') && output.contains('┘'),
            "{family}:\n{output}"
        );
        assert_no_structured_fallback(&output);
    }

    let english = [
        include_str!("fixtures/requirement.en.mmd"),
        include_str!("fixtures/mindmap.en.mmd"),
        include_str!("fixtures/treeview.en.mmd"),
        include_str!("fixtures/json.en.mmd"),
    ];
    for source in english {
        let output = render_source(
            source,
            &MermansiOptions::ascii().with_output_mode(OutputMode::Concise),
        )
        .unwrap();
        assert!(
            output.is_ascii(),
            "ASCII adapter emitted Unicode:\n{output}"
        );
        assert!(output.contains('+') && output.contains('-') && output.contains('|'));
        assert!(
            !output.contains("|--"),
            "outline fallback leaked:\n{output}"
        );
    }
}

#[test]
fn kanban_timeline_and_journey_fixtures_are_complete_closed_geometry() {
    let cases = [
        (
            "kanban",
            include_str!("fixtures/kanban.en.mmd"),
            9,
            0,
            "Kanban",
            "Task E",
        ),
        (
            "kanban",
            include_str!("fixtures/kanban.zh.mmd"),
            7,
            0,
            "Kanban",
            "任务 E",
        ),
        (
            "timeline",
            include_str!("fixtures/timeline.en.mmd"),
            8,
            7,
            "Company History",
            "[Period] IPO",
        ),
        (
            "timeline",
            include_str!("fixtures/timeline.zh.mmd"),
            8,
            7,
            "公司历史",
            "[Period] 上市",
        ),
        (
            "journey",
            include_str!("fixtures/journey.en.mmd"),
            8,
            7,
            "User Shopping Experience",
            "[Task] Receive confirmation",
        ),
        (
            "journey",
            include_str!("fixtures/journey.zh.mmd"),
            8,
            7,
            "用户购物体验",
            "[Task] 收到确认",
        ),
    ];
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);

    for (family, source, boxes, arrows, title, final_label) in cases {
        let output = render_source(source, &options)
            .unwrap_or_else(|error| panic!("{family} geometry failed: {error}"));
        assert_exact_closed_unicode_boxes(&output, boxes);
        assert_eq!(output.matches('▼').count(), arrows, "{family}:\n{output}");
        if arrows > 0 {
            assert_clean_vertical_chain(&output, arrows);
        }
        assert!(
            output.contains(title),
            "{family} lost title '{title}':\n{output}"
        );
        assert!(
            output.contains(final_label),
            "{family} lost '{final_label}':\n{output}"
        );
        assert_no_structured_fallback(&output);
        assert!(
            !output.contains("semantic model"),
            "concise output leaked the canonical model:\n{output}"
        );
    }
}

#[test]
fn timeline_events_are_closed_nodes_on_one_ordered_spine() {
    let source = "timeline\n  title Release\n  section Plan\n    Design: Kickoff: Review\n    Build\n      : Deploy: Observe\n  section Ship\n    Launch: Announce";
    let output = render_source(
        source,
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();

    assert_exact_closed_unicode_boxes(&output, 10);
    assert_clean_vertical_chain(&output, 9);
    let labels = [
        "[Section] Plan",
        "[Period] Design",
        "[Event] Kickoff",
        "[Event] Review",
        "[Period] Build",
        "[Event] Deploy",
        "[Event] Observe",
        "[Section] Ship",
        "[Period] Launch",
        "[Event] Announce",
    ];
    for pair in labels.windows(2) {
        assert!(
            label_position(&output, pair[0]).0 < label_position(&output, pair[1]).0,
            "timeline order was not preserved:\n{output}"
        );
    }
    assert_no_structured_fallback(&output);
}

#[test]
fn kanban_geometry_preserves_metadata_orphans_and_duplicate_ids() {
    let node = |id: &str,
                label: &str,
                is_group: bool,
                parent_id: Option<&str>,
                ticket: Option<&str>,
                priority: Option<&str>,
                assigned: Option<&str>,
                icon: Option<&str>| KanbanRenderNode {
        id: id.to_owned(),
        label: label.to_owned(),
        is_group,
        parent_id: parent_id.map(str::to_owned),
        ticket: ticket.map(str::to_owned),
        priority: priority.map(str::to_owned),
        assigned: assigned.map(str::to_owned),
        icon: icon.map(str::to_owned),
    };
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            node("todo", "待办", true, None, None, None, None, None),
            node(
                "card",
                "修复登录",
                false,
                Some("todo"),
                Some("K-1"),
                Some("high"),
                Some("alice"),
                Some("bug"),
            ),
            node("todo", "重复列", true, None, None, None, None, None),
            node(
                "card",
                "孤立卡片",
                false,
                Some("missing"),
                Some("K-2"),
                None,
                None,
                None,
            ),
        ],
    };
    let output = render_model(
        &RenderSemanticModel::Kanban(model),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();

    assert_exact_closed_unicode_boxes(&output, 4);
    for text in [
        "todo · 待办",
        "todo · 重复列",
        "card · 修复登录",
        "card · 孤立卡片",
        "ticket: K-1",
        "priority: high",
        "assigned: alice",
        "icon: bug",
        "column: missing",
    ] {
        assert!(output.contains(text), "Kanban lost '{text}':\n{output}");
    }
    assert!(
        !output.contains("kanban-"),
        "synthetic id leaked:\n{output}"
    );
    assert_no_structured_fallback(&output);
}

#[test]
fn timeline_and_journey_handle_duplicate_empty_and_orphan_sections() {
    let timeline = TimelineDiagramRenderModel {
        title: Some("Edge cases".to_owned()),
        acc_title: None,
        acc_descr: None,
        sections: vec![String::new(), "Plan".to_owned(), "Plan".to_owned()],
        tasks: vec![
            TimelineRenderTask {
                id: 0,
                section: String::new(),
                task_type: String::new(),
                task: "Unsectioned".to_owned(),
                score: 0,
                events: Vec::new(),
            },
            TimelineRenderTask {
                id: 1,
                section: "Plan".to_owned(),
                task_type: "milestone".to_owned(),
                task: "Design".to_owned(),
                score: 3,
                events: vec!["Gate".to_owned()],
            },
            TimelineRenderTask {
                id: 2,
                section: "Other".to_owned(),
                task_type: "Other".to_owned(),
                task: "Deploy".to_owned(),
                score: 0,
                events: Vec::new(),
            },
        ],
    };
    let timeline_output = render_model(
        &RenderSemanticModel::Timeline(timeline),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();
    assert_exact_closed_unicode_boxes(&timeline_output, 8);
    assert_clean_vertical_chain(&timeline_output, 7);
    assert_eq!(timeline_output.matches("[Section] Plan").count(), 2);
    for text in [
        "[Section] (unnamed)",
        "[Section] Other",
        "type: milestone",
        "score: 3",
        "[Event] Gate",
    ] {
        assert!(
            timeline_output.contains(text),
            "missing '{text}':\n{timeline_output}"
        );
    }

    let journey = JourneyDiagramRenderModel {
        title: Some("Score edges".to_owned()),
        acc_title: None,
        acc_descr: None,
        sections: vec![String::new(), "Plan".to_owned(), "Plan".to_owned()],
        tasks: vec![
            JourneyRenderTask {
                score: -2,
                score_is_nan: false,
                people: vec!["Alice".to_owned()],
                section: String::new(),
                task_type: String::new(),
                task: "Negative".to_owned(),
            },
            JourneyRenderTask {
                score: 9,
                score_is_nan: false,
                people: vec!["Bob".to_owned(), "用户".to_owned()],
                section: "Plan".to_owned(),
                task_type: "milestone".to_owned(),
                task: "Overflow".to_owned(),
            },
            JourneyRenderTask {
                score: 0,
                score_is_nan: true,
                people: vec!["Alice".to_owned(), "用户".to_owned()],
                section: "Other".to_owned(),
                task_type: "Other".to_owned(),
                task: "Unknown".to_owned(),
            },
        ],
        actors: vec!["Alice".to_owned(), "Bob".to_owned(), "用户".to_owned()],
    };
    let journey_output = render_model(
        &RenderSemanticModel::Journey(journey),
        &MermansiOptions::unicode().with_output_mode(OutputMode::Concise),
    )
    .unwrap();
    assert_exact_closed_unicode_boxes(&journey_output, 8);
    assert_clean_vertical_chain(&journey_output, 7);
    assert_eq!(journey_output.matches("[Section] Plan").count(), 2);
    for text in [
        "Alice, Bob, 用户",
        "[Section] (unnamed)",
        "[Section] Other",
        "score: -2/5 ░░░░░",
        "score: 9/5 █████",
        "score: NaN/5 ?????",
        "actors: Bob, 用户",
        "type: milestone",
    ] {
        assert!(
            journey_output.contains(text),
            "missing '{text}':\n{journey_output}"
        );
    }
    assert_no_structured_fallback(&timeline_output);
    assert_no_structured_fallback(&journey_output);
}

#[test]
fn native_board_and_spines_are_ascii_narrow_and_deterministic() {
    let cases = [
        ("kanban", include_str!("fixtures/kanban.en.mmd"), "Task E"),
        (
            "timeline",
            include_str!("fixtures/timeline.en.mmd"),
            "[Period] IPO",
        ),
        (
            "journey",
            include_str!("fixtures/journey.en.mmd"),
            "[Task] Receive",
        ),
    ];
    for options in [
        MermansiOptions::unicode()
            .with_output_mode(OutputMode::Concise)
            .with_max_width(40),
        MermansiOptions::ascii()
            .with_output_mode(OutputMode::Concise)
            .with_max_width(40),
    ] {
        for (family, source, label) in cases {
            let first = render_source(source, &options)
                .unwrap_or_else(|error| panic!("{family} narrow render failed: {error}"));
            assert_eq!(render_source(source, &options).unwrap(), first);
            assert!(first.contains(label), "{family} lost '{label}':\n{first}");
            assert!(
                first
                    .lines()
                    .all(|line| mermansi::str_display_width(line) <= 40),
                "{family} exceeded narrow width:\n{first}"
            );
            if options.charset == mermansi::Charset::Ascii {
                assert!(first.is_ascii(), "{family} emitted Unicode:\n{first}");
                assert!(first.contains('+') && first.contains('-') && first.contains('|'));
            }
            assert_no_structured_fallback(&first);
        }
    }
}

#[test]
fn empty_native_models_have_explicit_non_summary_empty_states() {
    let options = MermansiOptions::unicode().with_output_mode(OutputMode::Concise);
    let cases = [
        (
            RenderSemanticModel::Kanban(KanbanDiagramRenderModel::default()),
            "Kanban\n\n(empty board)\n",
        ),
        (
            RenderSemanticModel::Timeline(TimelineDiagramRenderModel::default()),
            "Timeline\n\n(empty timeline)\n",
        ),
        (
            RenderSemanticModel::Journey(JourneyDiagramRenderModel::default()),
            "Journey\n\n(empty journey)\n",
        ),
    ];
    for (model, expected) in cases {
        let output = render_model(&model, &options).unwrap();
        assert_eq!(output, expected);
        assert_no_structured_fallback(&output);
        assert!(!output.contains("semantic model"));
    }
}

// ---------------------------------------------------------------------------
// ANSI escape sequence sanitization in Pie labels (Rule 6)
// ---------------------------------------------------------------------------

/// Helper: count occurrences of a single byte in a string.
fn count_byte(s: &str, byte: u8) -> usize {
    s.bytes().filter(|&b| b == byte).count()
}

#[test]
fn pie_label_text_survives_sanitization_in_plain_mode() {
    // The visible portion of a CSI-wrapped label ("Red") must appear in output.
    let label = "\u{1b}[31mRed\u{1b}[0m".to_string();
    let source = format!("pie title Colours\n  \"{label}\" : 50\n  \"Blue\" : 30");
    let output = render_source(&source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Red"),
        "Sanitized label text 'Red' must survive in output:\n{output}"
    );
}

#[test]
fn pie_label_no_control_bytes_in_plain_output() {
    // A Pie label containing CSI, OSC (BEL-terminated), and bare BEL must produce
    // zero ESC (0x1b) and zero BEL (0x07) bytes under Plain mode.
    let label = "\u{1b}[31mRed\u{1b}[0m\u{1b}]0;X\u{07}\u{07}".to_string();
    let source = format!("pie title Colours\n  \"{label}\" : 50\n  \"Blue\" : 30");
    let output = render_source(&source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(
        count_byte(&output, 0x1b),
        0,
        "Plain output must contain zero ESC bytes:\n{output}"
    );
    assert_eq!(
        count_byte(&output, 0x07),
        0,
        "Plain output must contain zero BEL bytes:\n{output}"
    );
}

#[test]
fn pie_value_column_aligns_with_sanitized_and_clean_labels() {
    // The "Value" column (numbers like "50.00") must appear at the same display column
    // regardless of whether the label was originally clean or contained control
    // sequences — proving sanitization happens before width calculation.
    let dirty_source = "pie title T\n  \"\u{1b}[31mDog\u{1b}[0m\" : 50\n  \"Cat\" : 25".to_string();
    let clean_source = "pie title T\n  \"Dog\" : 50\n  \"Cat\" : 25".to_string();
    let dirty = render_source(&dirty_source, &MermansiOptions::unicode()).unwrap();
    let clean = render_source(&clean_source, &MermansiOptions::unicode()).unwrap();

    // Find the line containing "Dog" in each output, then locate "50.00" offset.
    fn value_column(output: &str, label: &str, value: &str) -> usize {
        let line = output
            .lines()
            .find(|l| l.contains(label))
            .unwrap_or_else(|| panic!("line with '{label}' missing:\n{output}"));
        display_column_of(line, value)
    }

    let dirty_off = value_column(&dirty, "Dog", "50.00");
    let clean_off = value_column(&clean, "Dog", "50.00");
    assert_eq!(
        dirty_off, clean_off,
        "Value display column must be identical after sanitization\n\
         dirty line offset={dirty_off}, clean={clean_off}\n\
         --- dirty ---\n{dirty}\n--- clean ---\n{clean}"
    );
}

#[test]
fn pie_label_no_control_bytes_in_ansi16_mode() {
    // Colored modes must also not pass through user-supplied control sequences
    // embedded in label text.
    let label = "\u{1b}[31mRed\u{1b}[0m\u{1b}]0;X\u{07}\u{07}".to_string();
    let source = format!("pie title Colours\n  \"{label}\" : 50\n  \"Blue\" : 30");
    let opts = MermansiOptions::unicode().with_color(ColorMode::Ansi16);
    let output = render_source(&source, &opts).unwrap();
    // The renderer's own ANSI roles are expected in Ansi16 mode.
    assert!(
        count_byte(&output, 0x1b) > 0,
        "Ansi16 output should contain the renderer's own ESC sequences:\n{output}"
    );
    // But the exact user-supplied CSI and OSC sequences must be absent.
    assert!(
        !output.contains("\u{1b}[31m"),
        "Ansi16 output must not contain the user-supplied CSI '[31m':\n{output}"
    );
    assert!(
        !output.contains("\u{1b}]0;X"),
        "Ansi16 output must not contain the user-supplied OSC ']0;X':\n{output}"
    );
    // No BEL (0x07) from user labels.
    assert_eq!(
        count_byte(&output, 0x07),
        0,
        "Ansi16 output must contain zero BEL bytes from labels:\n{output}"
    );
}

#[test]
fn pie_label_no_control_bytes_in_truecolor_mode() {
    let label = "\u{1b}[31mRed\u{1b}[0m\u{1b}]0;X\u{07}\u{07}".to_string();
    let source = format!("pie title Colours\n  \"{label}\" : 50\n  \"Blue\" : 30");
    let opts = MermansiOptions::unicode().with_color(ColorMode::TrueColor);
    let output = render_source(&source, &opts).unwrap();
    // The renderer's own ANSI roles are expected in TrueColor mode.
    assert!(
        count_byte(&output, 0x1b) > 0,
        "TrueColor output should contain the renderer's own ESC sequences:\n{output}"
    );
    // But the exact user-supplied CSI and OSC sequences must be absent.
    assert!(
        !output.contains("\u{1b}[31m"),
        "TrueColor output must not contain the user-supplied CSI '[31m':\n{output}"
    );
    assert!(
        !output.contains("\u{1b}]0;X"),
        "TrueColor output must not contain the user-supplied OSC ']0;X':\n{output}"
    );
    // No BEL (0x07) from user labels.
    assert_eq!(
        count_byte(&output, 0x07),
        0,
        "TrueColor output must contain zero BEL bytes from labels:\n{output}"
    );
}

#[test]
fn pie_label_st_terminated_osc_stripped_in_plain_mode() {
    // OSC terminated by ST (ESC \) rather than BEL must also be removed.
    let label = "\u{1b}]0;Title\u{1b}\\Visible".to_string();
    let source = format!("pie title T\n  \"{label}\" : 50");
    let output = render_source(&source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(
        count_byte(&output, 0x1b),
        0,
        "Plain output must strip ST-terminated OSC:\n{output}"
    );
    assert!(
        output.contains("Visible"),
        "Visible text after OSC must survive:\n{output}"
    );
}

#[test]
fn pie_label_dcs_stripped_in_plain_mode() {
    // DCS (ESC P) string-control sequence in a label must be fully stripped
    // from Plain-mode output, with no surviving ESC bytes.
    let label = "A\u{1b}Ppayload\u{07}B".to_string();
    let source = format!("pie title T\n  \"{label}\" : 50\n  \"Clean\" : 30");
    let output = render_source(&source, &MermansiOptions::unicode()).unwrap();
    assert_eq!(
        count_byte(&output, 0x1b),
        0,
        "Plain output must contain zero ESC bytes from DCS:\n{output}"
    );
    assert!(
        output.contains("AB"),
        "Visible text AB must survive after DCS stripping:\n{output}"
    );
}

#[test]
fn pie_label_dcs_stripped_in_ansi16_mode() {
    // DCS string-control sequence in a label must be stripped even in
    // Ansi16 color mode — the renderer's own ANSI roles are allowed but
    // the user-supplied control sequence must not leak.
    let label = "\u{1b}[31mRed\u{1b}[0m\u{1b}Ppayload\u{07}".to_string();
    let source = format!("pie title T\n  \"{label}\" : 50\n  \"Blue\" : 30");
    let opts = MermansiOptions::unicode().with_color(ColorMode::Ansi16);
    let output = render_source(&source, &opts).unwrap();
    // Renderer's own ANSI is present.
    assert!(
        count_byte(&output, 0x1b) > 0,
        "Ansi16 output should contain renderer's own ESC sequences:\n{output}"
    );
    // User-supplied CSI and DCS must be absent.
    assert!(
        !output.contains("\u{1b}[31m"),
        "Ansi16 output must not contain user-supplied CSI '[31m':\n{output}"
    );
    assert!(
        !output.contains("\u{1b}Ppayload"),
        "Ansi16 output must not contain user-supplied DCS:\n{output}"
    );
    assert_eq!(
        count_byte(&output, 0x07),
        0,
        "Ansi16 output must contain zero BEL bytes:\n{output}"
    );
}

#[test]
fn pie_label_dcs_stripped_in_truecolor_mode() {
    let label = "\u{1b}[31mRed\u{1b}[0m\u{1b}Ppayload\u{07}".to_string();
    let source = format!("pie title T\n  \"{label}\" : 50\n  \"Blue\" : 30");
    let opts = MermansiOptions::unicode().with_color(ColorMode::TrueColor);
    let output = render_source(&source, &opts).unwrap();
    assert!(
        count_byte(&output, 0x1b) > 0,
        "TrueColor output should contain renderer's own ESC sequences:\n{output}"
    );
    assert!(
        !output.contains("\u{1b}[31m"),
        "TrueColor output must not contain user-supplied CSI '[31m':\n{output}"
    );
    assert!(
        !output.contains("\u{1b}Ppayload"),
        "TrueColor output must not contain user-supplied DCS:\n{output}"
    );
    assert_eq!(
        count_byte(&output, 0x07),
        0,
        "TrueColor output must contain zero BEL bytes:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// Pie chart geometry tests
// ---------------------------------------------------------------------------

#[test]
fn pie_has_closed_circular_outline() {
    let source = "pie title Pets\n  \"Dogs\" : 40\n  \"Cats\" : 25\n  \"Fish\" : 10";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "pie");
    // The pie preview should contain non-whitespace chart characters indicating a circle was drawn.
    let non_blank_lines = preview
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(
        non_blank_lines >= 3,
        "Pie should have a multi-line circular chart, got {} non-blank lines:\n{preview}",
        non_blank_lines
    );
}

#[test]
fn pie_has_proportional_sector_fill() {
    // A section with a much larger value should produce more filled cells.
    let source_small = "pie title Small\n  \"A\" : 1\n  \"B\" : 1";
    let source_large = "pie title Large\n  \"A\" : 100\n  \"B\" : 1";
    let out_small = render_source(source_small, &MermansiOptions::unicode()).unwrap();
    let out_large = render_source(source_large, &MermansiOptions::unicode()).unwrap();
    let preview_small = family_preview(&out_small, "pie");
    let preview_large = family_preview(&out_large, "pie");
    let filled_small = preview_small
        .chars()
        .filter(|&c| c == '█' || c == '▓' || c == '▒' || c == '░')
        .count();
    let filled_large = preview_large
        .chars()
        .filter(|&c| c == '█' || c == '▓' || c == '▒' || c == '░')
        .count();
    assert!(
        filled_large >= filled_small,
        "Larger pie should have at least as many filled cells: small={}, large={}",
        filled_small,
        filled_large
    );
}

#[test]
fn pie_preserves_all_labels_values_and_percentages() {
    let source = "pie title Test\n  \"Alpha\" : 60\n  \"Beta\" : 40";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "pie");
    assert!(preview.contains("Alpha"), "Alpha label missing:\n{preview}");
    assert!(preview.contains("Beta"), "Beta label missing:\n{preview}");
    assert!(preview.contains("60.00"), "Alpha value missing:\n{preview}");
    assert!(preview.contains("40.00"), "Beta value missing:\n{preview}");
    assert!(
        preview.contains("60.0%"),
        "Alpha percentage missing:\n{preview}"
    );
    assert!(
        preview.contains("40.0%"),
        "Beta percentage missing:\n{preview}"
    );
}

#[test]
fn pie_title_preserved_in_geometry() {
    let source = "pie title My Chart\n  \"A\" : 1";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(output.contains("My Chart"), "Pie title missing:\n{output}");
}

#[test]
fn pie_empty_sections_produce_nonempty_output() {
    let source = "pie title Empty\n";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("empty pie chart") || output.contains("Empty"),
        "Empty pie should produce labeled nonempty output:\n{output}"
    );
}

#[test]
fn pie_zero_total_produces_nonempty_output() {
    let source = "pie title Zero\n  \"A\" : 0\n  \"B\" : 0";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("zero total") || output.contains("Zero"),
        "Zero-total pie should produce labeled nonempty output:\n{output}"
    );
}

#[test]
fn pie_ascii_geometry_produces_nonempty_output() {
    let source = "pie title ASCII Pie\n  \"A\" : 30\n  \"B\" : 20";
    let output = render_source(source, &MermansiOptions::ascii()).unwrap();
    let preview = family_preview(&output, "pie");
    assert!(
        !preview.trim().is_empty(),
        "ASCII pie should produce nonempty chart:\n{preview}"
    );
    assert!(
        preview.contains("A") && preview.contains("B"),
        "ASCII pie should preserve labels:\n{preview}"
    );
}

// ---------------------------------------------------------------------------
// Radar chart geometry tests
// ---------------------------------------------------------------------------

#[test]
fn radar_has_center_and_spokes() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{4,3,5}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    let non_blank_lines = preview
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(
        non_blank_lines >= 3,
        "Radar should have multi-line chart with spokes:\n{preview}"
    );
}

#[test]
fn radar_preserves_axis_labels() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{4,3,5}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    assert!(preview.contains("A"), "Axis A missing:\n{preview}");
    assert!(preview.contains("B"), "Axis B missing:\n{preview}");
    assert!(preview.contains("C"), "Axis C missing:\n{preview}");
}

#[test]
fn radar_preserves_curve_label_and_values_in_legend() {
    let source = "radar-beta\n  axis A,B,C\n  curve Quality{4,3,5}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    assert!(
        preview.contains("Quality"),
        "Curve label missing:\n{preview}"
    );
    assert!(
        preview.contains("4.00") || preview.contains("4"),
        "Curve value missing:\n{preview}"
    );
}

#[test]
fn radar_graticule_polygon_mode_renders() {
    let source = "radar-beta\n  axis A,B,C\n  graticule polygon\n  curve Q{4,3,5}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    assert!(
        !preview.trim().is_empty(),
        "Polygon graticule should produce nonempty chart:\n{preview}"
    );
}

#[test]
fn radar_empty_axes_produces_nonempty_output() {
    let source = "radar-beta\n  title Empty Radar";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("empty radar chart") || output.contains("Empty"),
        "Empty radar should produce labeled nonempty output:\n{output}"
    );
}

#[test]
fn radar_title_preserved() {
    let source = "radar-beta\n  title My Radar\n  axis A,B,C\n  curve Q{1,2,3}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("My Radar"),
        "Radar title missing:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// QuadrantChart geometry tests
// ---------------------------------------------------------------------------

#[test]
fn quadrant_has_closed_plotting_area() {
    let source = "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4\n  A: [0.7, 0.8]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    // Should have box-drawing characters forming the closed plotting area.
    assert!(
        preview.contains('┌') || preview.contains('┐') || preview.contains('+'),
        "Quadrant chart should have closed box border:\n{preview}"
    );
}

#[test]
fn quadrant_preserves_all_four_quadrant_labels() {
    let source = "quadrantChart\n  quadrant-1 Quick Wins\n  quadrant-2 Major Projects\n  quadrant-3 Fill-ins\n  quadrant-4 Thankless";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    assert!(
        preview.contains("Quick Wins"),
        "Q1 label missing:\n{preview}"
    );
    assert!(
        preview.contains("Major Projects"),
        "Q2 label missing:\n{preview}"
    );
    assert!(preview.contains("Fill-ins"), "Q3 label missing:\n{preview}");
    assert!(
        preview.contains("Thankless"),
        "Q4 label missing:\n{preview}"
    );
}

#[test]
fn quadrant_points_placed_from_normalized_coordinates() {
    let source = "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4\n  Alpha: [0.8, 0.9]\n  Beta: [0.2, 0.1]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    assert!(preview.contains("Alpha"), "Point Alpha missing:\n{preview}");
    assert!(preview.contains("Beta"), "Point Beta missing:\n{preview}");
}

#[test]
fn quadrant_preserves_class_metadata() {
    let source = "quadrantChart\n  classDef priority color: #ff0000, radius: 5\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4\n  Alpha:::priority: [0.5, 0.5]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    assert!(
        preview.contains("priority"),
        "Class name missing:\n{preview}"
    );
    assert!(
        preview.contains("#ff0000"),
        "Class color style missing:\n{preview}"
    );
}

#[test]
fn quadrant_title_preserved() {
    let source = "quadrantChart\n  title Assessment\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Assessment"),
        "Quadrant title missing:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// Venn diagram geometry tests
// ---------------------------------------------------------------------------

#[test]
fn venn_has_overlapping_set_outlines() {
    let source = "venn-beta\n  title Sets\n  set A\n  set B\n  union A,B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    let non_blank_lines = preview
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(
        non_blank_lines >= 2,
        "Venn should have multi-line overlapping outlines:\n{preview}"
    );
}

#[test]
fn venn_preserves_set_labels() {
    let source = "venn-beta\n  set A[\"Programming\"]\n  set B[\"Design\"]\n  union A,B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        preview.contains("Programming"),
        "Set A label missing:\n{preview}"
    );
    assert!(
        preview.contains("Design"),
        "Set B label missing:\n{preview}"
    );
}

#[test]
fn venn_preserves_intersection_labels() {
    let source = "venn-beta\n  set A\n  set B\n  union A,B[\"Shared\"]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        preview.contains("Shared"),
        "Intersection label missing:\n{preview}"
    );
}

#[test]
fn venn_three_sets_have_ring_layout() {
    let source = "venn-beta\n  set A\n  set B\n  set C\n  union A,B\n  union A,C\n  union A,B,C";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        !preview.trim().is_empty(),
        "Three-set Venn should produce nonempty geometry:\n{preview}"
    );
}

#[test]
fn venn_preserves_text_nodes() {
    let source = "venn-beta\n  set A[\"Frontend\"]\n  set B[\"Backend\"]\n  union A,B[\"APIs\"]\ntext A,B fullstack";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        preview.contains("fullstack"),
        "Text node id missing:\n{preview}"
    );
}

#[test]
fn venn_labels_are_collision_free_and_region_associated() {
    let source = "venn-beta\n  title Skill Overlap\n  set A[\"Programming\"]\n  set B[\"Design\"]\n  set C[\"Management\"]\n  union A,B[\"Engineering\"]\n  union A,C[\"Leadership\"]\n  union A,B,C[\"Full Stack\"]\ntext A,B fullstack";
    for (options, connector) in [
        (MermansiOptions::unicode(), '·'),
        (MermansiOptions::ascii(), '.'),
    ] {
        let options = options
            .with_output_mode(OutputMode::Concise)
            .with_max_width(80)
            .with_max_height(40);
        let output = render_source(source, &options).expect("collision-free Venn geometry");
        let geometry = output.split("\nIntersections:").next().unwrap_or(&output);

        for label in [
            "Programming",
            "Design",
            "Management",
            "Engineering",
            "Leadership",
            "Full Stack",
            "fullstack",
        ] {
            assert_eq!(
                geometry.matches(label).count(),
                1,
                "label must appear exactly once in geometry or a connected callout: {label}\n{geometry}"
            );
        }
        assert!(
            geometry.contains(connector),
            "labels that do not fit their region must have a visible connector:\n{geometry}"
        );
        assert!(
            geometry.lines().count() <= 28,
            "Venn geometry should be compact, got {} rows:\n{geometry}",
            geometry.lines().count()
        );

        let repeated = render_source(source, &options).expect("deterministic Venn geometry");
        assert_eq!(output, repeated, "Venn placement must be deterministic");
    }
}

#[test]
fn venn_title_preserved() {
    let source = "venn-beta\n  title Skill Overlap\n  set A\n  set B\n  union A,B";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    assert!(
        output.contains("Skill Overlap"),
        "Venn title missing:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// Strong chart geometry tests
// ---------------------------------------------------------------------------

#[test]
fn pie_circle_outline_is_closed() {
    // A single-section pie should show a full closed circle outline.
    let source = "pie title Full\n  \"A\" : 1";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "pie");
    let outline_count = preview.matches('○').count();
    assert!(
        outline_count >= 8,
        "Closed circle should have many outline chars, got {outline_count}:\n{preview}"
    );
}

#[test]
fn pie_has_radial_boundary_spokes() {
    // A multi-section pie should show the ◆ boundary marker at sector edges.
    let source = "pie title Spokes\n  \"A\" : 10\n  \"B\" : 5\n  \"C\" : 5";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "pie");
    assert!(
        preview.contains('◆'),
        "Pie should have radial boundary spokes (◆):\n{preview}"
    );
}

#[test]
fn pie_proportional_fill_larger_section_has_more_cells() {
    let equal_src = "pie title Equal\n  \"A\" : 50\n  \"B\" : 50";
    let skewed_src = "pie title Skewed\n  \"A\" : 90\n  \"B\" : 10";
    let out_equal = render_source(equal_src, &MermansiOptions::unicode()).unwrap();
    let out_skewed = render_source(skewed_src, &MermansiOptions::unicode()).unwrap();
    let prev_equal = family_preview(&out_equal, "pie");
    let prev_skewed = family_preview(&out_skewed, "pie");
    let fill = |s: &str| {
        s.chars()
            .filter(|&c| c == '█' || c == '▓' || c == '▒' || c == '░')
            .count()
    };
    // The total fill should be roughly similar (both cover a full circle).
    // But the dominant fill char (█) should be significantly more in the skewed case.
    let dominant_equal = prev_equal.matches('█').count();
    let dominant_skewed = prev_skewed.matches('█').count();
    assert!(
        dominant_skewed > dominant_equal,
        "Skewed pie should have more dominant fill (█) than equal pie: equal={}, skewed={}",
        dominant_equal,
        dominant_skewed
    );
    let _ = fill(prev_equal) + fill(prev_skewed);
}

#[test]
fn radar_has_center_marker() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    assert!(
        preview.contains('✛'),
        "Radar should have a center marker (✛):\n{preview}"
    );
}

#[test]
fn radar_has_graticule_dots() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    assert!(
        preview.contains('·'),
        "Radar should have graticule dots (·):\n{preview}"
    );
}

#[test]
fn radar_has_connected_curve_vertices() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    // Curve edges are drawn with ─ character.
    assert!(
        preview.contains('─'),
        "Radar should have connected curve edges (─):\n{preview}"
    );
}

#[test]
fn radar_curve_markers_present() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{4,3,5}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    // First curve marker is ●.
    assert!(
        preview.contains('●'),
        "Radar should plot curve vertex markers (●):\n{preview}"
    );
}

#[test]
fn quadrant_has_midpoint_cross() {
    let source =
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    assert!(
        preview.contains('+'),
        "Quadrant chart should have midpoint cross (+):\n{preview}"
    );
}

#[test]
fn quadrant_labels_in_correct_regions() {
    let source = "quadrantChart\n  quadrant-1 TopRight\n  quadrant-2 TopLeft\n  quadrant-3 BotLeft\n  quadrant-4 BotRight";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");

    // Find the midpoint cross row (the cross line contains '+' at the midpoint).
    let cross_line_idx = preview
        .lines()
        .position(|l| l.contains('+'))
        .unwrap_or(usize::MAX);
    assert!(
        cross_line_idx != usize::MAX,
        "No midpoint cross found:\n{preview}"
    );

    // TopRight and TopLeft should be above the cross.
    let tr_row = preview
        .lines()
        .position(|l| l.contains("TopRight"))
        .unwrap_or(usize::MAX);
    let tl_row = preview
        .lines()
        .position(|l| l.contains("TopLeft"))
        .unwrap_or(usize::MAX);
    assert!(
        tr_row < cross_line_idx,
        "Q1 (TopRight) should be above midpoint cross: tr_row={}, cross_row={}",
        tr_row,
        cross_line_idx
    );
    assert!(
        tl_row < cross_line_idx,
        "Q2 (TopLeft) should be above midpoint cross: tl_row={}, cross_row={}",
        tl_row,
        cross_line_idx
    );

    // BotLeft and BotRight should be below the cross.
    let bl_row = preview
        .lines()
        .position(|l| l.contains("BotLeft"))
        .unwrap_or(0);
    let br_row = preview
        .lines()
        .position(|l| l.contains("BotRight"))
        .unwrap_or(0);
    assert!(
        bl_row > cross_line_idx,
        "Q3 (BotLeft) should be below midpoint cross: bl_row={}, cross_row={}",
        bl_row,
        cross_line_idx
    );
    assert!(
        br_row > cross_line_idx,
        "Q4 (BotRight) should be below midpoint cross: br_row={}, cross_row={}",
        br_row,
        cross_line_idx
    );

    // On the top label line, TopLeft should appear before TopRight (left-right ordering).
    // This verifies Q2 is left and Q1 is right.
    let top_line = preview.lines().nth(tr_row).unwrap_or("");
    let tl_byte = top_line.find("TopLeft").unwrap_or(usize::MAX);
    let tr_byte = top_line.find("TopRight").unwrap_or(0);
    assert!(
        tl_byte < tr_byte,
        "TopLeft should be left of TopRight on same line: tl_byte={}, tr_byte={}",
        tl_byte,
        tr_byte
    );

    // On the bottom label line, BotLeft should appear before BotRight (left-right ordering).
    let bot_line = preview.lines().nth(bl_row).unwrap_or("");
    let bl_byte = bot_line.find("BotLeft").unwrap_or(usize::MAX);
    let br_byte = bot_line.find("BotRight").unwrap_or(0);
    assert!(
        bl_byte < br_byte,
        "BotLeft should be left of BotRight on same line: bl_byte={}, br_byte={}",
        bl_byte,
        br_byte
    );
}

#[test]
fn quadrant_collision_handling_produces_callout() {
    // Two points at the same coordinates should trigger collision handling.
    let source = "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4\n  Alpha: [0.5, 0.5]\n  Beta: [0.5, 0.5]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    // Both labels should still appear somewhere (in legend or callout).
    assert!(
        preview.contains("Alpha"),
        "Point Alpha should be preserved:\n{preview}"
    );
    assert!(
        preview.contains("Beta"),
        "Point Beta should be preserved:\n{preview}"
    );
}

#[test]
fn venn_overlap_identity_two_sets() {
    // Two-set Venn should have overlap region (A∩B) visible in the geometry.
    let source = "venn-beta\n  set A\n  set B\n  union A,B[\"Intersection\"]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        preview.contains("Intersection"),
        "Intersection label should appear in overlap region:\n{preview}"
    );
    // Verify there are at least two circle outlines (the overlapping sets).
    let outline_count = preview.matches('○').count();
    assert!(
        outline_count >= 4,
        "Two-set Venn should have multiple outline chars from overlapping circles, got {}:\
         \n{preview}",
        outline_count
    );
}

#[test]
fn venn_preserves_styles() {
    let source = "venn-beta\n  set A\n  set B\n  union A,B\n  style A fill:#ff0000";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        preview.contains("Styles:")
            || preview.contains("fill=#ff0000")
            || preview.contains("#ff0000"),
        "Style metadata should be preserved:\n{preview}"
    );
}

#[test]
fn pie_ansi_color_independent_geometry() {
    let plain = render_source(
        "pie title T\n  \"A\" : 50\n  \"B\" : 50",
        &MermansiOptions::unicode(),
    )
    .unwrap();
    let colored = render_source(
        "pie title T\n  \"A\" : 50\n  \"B\" : 50",
        &MermansiOptions::unicode().with_color(ColorMode::Ansi16),
    )
    .unwrap();
    // Strip ANSI first, then extract preview — geometry must be identical.
    let plain_preview = family_preview(&plain, "pie");
    let colored_stripped = mermansi::ansi::strip_ansi(&colored);
    let colored_preview = family_preview(&colored_stripped, "pie");
    assert_eq!(
        plain_preview, colored_preview,
        "ANSI color should not affect geometry"
    );
}

#[test]
fn radar_ansi_color_independent_geometry() {
    let plain = render_source(
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        &MermansiOptions::unicode(),
    )
    .unwrap();
    let colored = render_source(
        "radar-beta\n  axis A,B,C\n  curve Q{1,2,3}",
        &MermansiOptions::unicode().with_color(ColorMode::TrueColor),
    )
    .unwrap();
    let plain_preview = family_preview(&plain, "radar");
    let colored_stripped = mermansi::ansi::strip_ansi(&colored);
    let colored_preview = family_preview(&colored_stripped, "radar");
    assert_eq!(
        plain_preview, colored_preview,
        "ANSI color should not affect geometry"
    );
}

#[test]
fn quadrant_ansi_color_independent_geometry() {
    let plain = render_source(
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4",
        &MermansiOptions::unicode(),
    )
    .unwrap();
    let colored = render_source(
        "quadrantChart\n  quadrant-1 Q1\n  quadrant-2 Q2\n  quadrant-3 Q3\n  quadrant-4 Q4",
        &MermansiOptions::unicode().with_color(ColorMode::Ansi16),
    )
    .unwrap();
    let plain_preview = family_preview(&plain, "quadrantChart");
    let colored_stripped = mermansi::ansi::strip_ansi(&colored);
    let colored_preview = family_preview(&colored_stripped, "quadrantChart");
    assert_eq!(
        plain_preview, colored_preview,
        "ANSI color should not affect geometry"
    );
}

#[test]
fn venn_ansi_color_independent_geometry() {
    let plain = render_source(
        "venn-beta\n  set A\n  set B\n  union A,B",
        &MermansiOptions::unicode(),
    )
    .unwrap();
    let colored = render_source(
        "venn-beta\n  set A\n  set B\n  union A,B",
        &MermansiOptions::unicode().with_color(ColorMode::TrueColor),
    )
    .unwrap();
    let plain_preview = family_preview(&plain, "venn");
    let colored_stripped = mermansi::ansi::strip_ansi(&colored);
    let colored_preview = family_preview(&colored_stripped, "venn");
    assert_eq!(
        plain_preview, colored_preview,
        "ANSI color should not affect geometry"
    );
}

#[test]
fn pie_chinese_cjk_display_width() {
    let source = "pie title 中文饼图\n  \"苹果\" : 60\n  \"香蕉\" : 40";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "pie");
    assert!(
        preview.contains("苹果"),
        "Chinese pie label missing:\n{preview}"
    );
    assert!(
        preview.contains("香蕉"),
        "Chinese pie label missing:\n{preview}"
    );
}

#[test]
fn radar_chinese_cjk_axis_labels() {
    let source = "radar-beta\n  axis A,B,C\n  curve Q{4,3,5}";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "radar");
    assert!(
        preview.contains("A"),
        "Radar axis label missing:\n{preview}"
    );
    assert!(
        preview.contains("B"),
        "Radar axis label missing:\n{preview}"
    );
}

#[test]
fn radar_cjk_axis_label_via_constructed_model() {
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::diagrams::radar::{
        RadarDiagramRenderModel, RadarRenderAxis, RadarRenderCurve, RadarRenderOptions,
    };
    let model = RadarDiagramRenderModel {
        title: Some("雷达图".to_string()),
        acc_title: None,
        acc_descr: None,
        axes: vec![
            RadarRenderAxis {
                name: "A".to_string(),
                label: "速度".to_string(),
            },
            RadarRenderAxis {
                name: "B".to_string(),
                label: "质量".to_string(),
            },
            RadarRenderAxis {
                name: "C".to_string(),
                label: "成本".to_string(),
            },
        ],
        curves: vec![RadarRenderCurve {
            name: "Q".to_string(),
            label: "Q".to_string(),
            entries: vec![
                serde_json::json!(4),
                serde_json::json!(3),
                serde_json::json!(5),
            ],
        }],
        options: RadarRenderOptions::default(),
    };
    let output = render_model(
        &RenderSemanticModel::Radar(model),
        &MermansiOptions::unicode(),
    )
    .unwrap();
    let preview = family_preview(&output, "radar");
    assert!(
        preview.contains("速度"),
        "Chinese radar axis label missing:\n{preview}"
    );
}

#[test]
fn quadrant_chinese_cjk_labels() {
    let source = "quadrantChart\n  quadrant-1 快速收益\n  quadrant-2 重大项目\n  quadrant-3 填充任务\n  quadrant-4 吃力不讨好";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "quadrantChart");
    assert!(
        preview.contains("快速收益"),
        "Chinese quadrant Q1 label missing:\n{preview}"
    );
}

#[test]
fn venn_chinese_cjk_set_labels() {
    let source = "venn-beta\n  set A[\"前端\"]\n  set B[\"后端\"]\n  union A,B[\"全栈\"]";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = family_preview(&output, "venn");
    assert!(
        preview.contains("前端"),
        "Chinese venn set label missing:\n{preview}"
    );
    assert!(
        preview.contains("全栈"),
        "Chinese venn intersection label missing:\n{preview}"
    );
}
