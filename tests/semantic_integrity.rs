//! Semantic integrity tests.
//!
//! These tests verify label preservation (English, Chinese CJK, combining marks, emoji),
//! box closure, relationship endpoint/marker/label retention, directions (TD/TB/BT/LR/RL),
//! self-relations, cycles, disconnected components, nested groups, and parallel/dense
//! relations.

use mermansi::{ColorMode, MermansiOptions, render_source};

fn flowchart_preview(output: &str) -> &str {
    output
        .split("[flowchart semantic model]")
        .next()
        .unwrap_or(output)
        .trim_matches('\n')
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

// ---------------------------------------------------------------------------
// Block diagram — hierarchy without duplicate nodes
// ---------------------------------------------------------------------------

/// Extract only the readable preview portion of `render_source` output — i.e.
/// everything before the `[<family> semantic model]` delimiter — so that node
/// occurrence counts are not affected by the appended canonical JSON.
fn preview_only(output: &str) -> &str {
    output
        .split("[block semantic model]")
        .next()
        .unwrap_or(output)
}

#[test]
fn block_nested_three_levels_no_duplicate_nodes() {
    // Three-level nested hierarchy: Level1 > Level2 > Leaf.
    let source = "block-beta\n  block:L1[\"Level 1\"]\n    block:L2[\"Level 2\"]\n      Leaf[\"Leaf\"]\n    end\n  end\n";
    let output = render_source(source, &MermansiOptions::unicode()).unwrap();
    let preview = preview_only(&output);

    // Each semantic node id must appear exactly once in the preview. The id is
    // rendered parenthesized as `(id)`, which is unique to a single node line.
    for node_id in ["L1", "L2", "Leaf"] {
        let needle = format!("({node_id})");
        let count = preview.matches(&needle).count();
        assert_eq!(
            count, 1,
            "block node '{node_id}' should appear exactly once in the preview, found {count}:\n{preview}"
        );
    }

    // Hierarchy indentation must be preserved: L2 is a child of L1, Leaf a child of L2.
    let l1_line = preview
        .lines()
        .find(|line| line.contains("Level 1"))
        .unwrap_or_else(|| panic!("Level 1 missing:\n{preview}"));
    let l2_line = preview
        .lines()
        .find(|line| line.contains("Level 2"))
        .unwrap_or_else(|| panic!("Level 2 missing:\n{preview}"));
    let leaf_line = preview
        .lines()
        .find(|line| line.contains("Leaf"))
        .unwrap_or_else(|| panic!("Leaf missing:\n{preview}"));

    fn leading_spaces(line: &str) -> usize {
        line.chars().take_while(|c| *c == ' ').count()
    }

    assert!(
        leading_spaces(l2_line) > leading_spaces(l1_line),
        "Level 2 should be indented deeper than Level 1:\n{preview}"
    );
    assert!(
        leading_spaces(leaf_line) > leading_spaces(l2_line),
        "Leaf should be indented deeper than Level 2:\n{preview}"
    );

    // The synthetic "root" node must not be emitted in the preview.
    assert!(
        !preview.contains("(root)"),
        "synthetic root node should not appear in the preview:\n{preview}"
    );
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
    // The "Value" column (numbers like "50.00") must appear at the same byte offset
    // regardless of whether the label was originally clean or contained control
    // sequences — proving sanitization happens before width calculation.
    let dirty_source = "pie title T\n  \"\u{1b}[31mDog\u{1b}[0m\" : 50\n  \"Cat\" : 25".to_string();
    let clean_source = "pie title T\n  \"Dog\" : 50\n  \"Cat\" : 25".to_string();
    let dirty = render_source(&dirty_source, &MermansiOptions::unicode()).unwrap();
    let clean = render_source(&clean_source, &MermansiOptions::unicode()).unwrap();

    // Find the line containing "Dog" in each output, then locate "50.00" offset.
    fn value_offset(output: &str, label: &str, value: &str) -> usize {
        let line = output
            .lines()
            .find(|l| l.contains(label))
            .unwrap_or_else(|| panic!("line with '{label}' missing:\n{output}"));
        line.find(value)
            .unwrap_or_else(|| panic!("value '{value}' missing in line:\n{line}"))
    }

    let dirty_off = value_offset(&dirty, "Dog", "50.00");
    let clean_off = value_offset(&clean, "Dog", "50.00");
    assert_eq!(
        dirty_off, clean_off,
        "Value column offset must be identical after sanitization\n\
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
