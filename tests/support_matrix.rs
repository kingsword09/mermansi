//! Executable support-matrix conformance test.
//!
//! This test iterates over every fixture in `tests/fixtures/` and verifies that each one
//! parses successfully and renders nonempty deterministic output in both ASCII and Unicode
//! modes. It is the machine-checkable backing for `docs/SUPPORT_MATRIX.md`.
//!
//! Every `RenderSemanticModel` variant in `merman-core` 0.8.0-alpha.3 is accounted for here,
//! plus the ZenUML alias family. At least one English (`.en.mmd`) and one Chinese (`.zh.mmd`)
//! fixture is tested per family.

use merman_core::diagram::RenderSemanticModel;
use mermansi::ansi::strip_ansi;
use mermansi::{
    Charset, ColorMode, MermansiError, MermansiOptions, OutputMode, render_source,
    str_display_width,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// The canonical list of family names that must have both English and Chinese fixtures.
///
/// These 29 family identifiers cover all 28 `RenderSemanticModel` variants plus the ZenUML
/// parser alias (which transforms into the `Sequence` render model).
const EXPECTED_FAMILIES: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "er",
    "eventmodeling",
    "flowchart",
    "gantt",
    "gitgraph",
    "info",
    "ishikawa",
    "journey",
    "json",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrant",
    "radar",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "treemap",
    "treeview",
    "venn",
    "xychart",
    "zenuml",
];

const EXPECTED_RENDER_PARSER_IDS: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "classDiagram",
    "er",
    "erDiagram",
    "eventmodeling",
    "flowchart",
    "flowchart-elk",
    "flowchart-v2",
    "gantt",
    "gitGraph",
    "info",
    "ishikawa",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantChart",
    "radar",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "stateDiagram",
    "timeline",
    "treeView",
    "treemap",
    "venn",
    "xychart",
    "zenuml",
];

const FIXTURE_DIR: &str = "tests/fixtures";

/// Collect all fixture files grouped by family name, then by language suffix.
fn collect_fixtures() -> BTreeMap<String, Fixtures> {
    let mut map: BTreeMap<String, Fixtures> = BTreeMap::new();
    for entry in fs::read_dir(FIXTURE_DIR).expect("fixtures directory must exist") {
        let entry = entry.expect("readable fixture entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".mmd") {
            continue;
        }
        // name format: <family>.<lang>.mmd
        let stem = name.trim_end_matches(".mmd");
        if let Some(dot) = stem.rfind('.') {
            let family = stem[..dot].to_string();
            let lang = stem[dot + 1..].to_string();
            let entry = map.entry(family).or_default();
            match lang.as_str() {
                "en" => entry.en = Some(name.clone()),
                "zh" => entry.zh = Some(name.clone()),
                _ => {}
            }
            entry.all.push(name);
        }
    }
    map
}

#[derive(Default)]
struct Fixtures {
    en: Option<String>,
    zh: Option<String>,
    all: Vec<String>,
}

#[test]
fn all_expected_families_have_en_and_zh_fixtures() {
    let fixtures = collect_fixtures();
    for family in EXPECTED_FAMILIES {
        let f = fixtures
            .get(*family)
            .unwrap_or_else(|| panic!("family '{family}' has no fixtures at all"));
        assert!(
            f.en.is_some(),
            "family '{family}' is missing an English (.en.mmd) fixture"
        );
        assert!(
            f.zh.is_some(),
            "family '{family}' is missing a Chinese (.zh.mmd) fixture"
        );
    }
}

#[test]
fn upstream_render_parser_inventory_is_fully_accounted_for() {
    let actual = merman_core::diagram_family_capabilities()
        .iter()
        .filter(|capability| capability.has_render_parser)
        .map(|capability| capability.diagram_type)
        .collect::<BTreeSet<_>>();
    let expected = EXPECTED_RENDER_PARSER_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "pinned merman-core render parser inventory changed; update dispatch, fixtures, and docs"
    );

    let registry = merman_core::RenderDiagramRegistry::pinned_mermaid_baseline_full();
    for parser_id in EXPECTED_RENDER_PARSER_IDS {
        assert!(
            registry.get(parser_id).is_some(),
            "render parser '{parser_id}' is missing from the full registry"
        );
    }
}

#[test]
fn every_fixture_renders_terminal_geometry_in_the_supported_width_matrix() {
    let fixtures = collect_fixtures();
    assert!(!fixtures.is_empty(), "no fixtures found in {FIXTURE_DIR}");

    let total: usize = fixtures.values().map(|f| f.all.len()).sum();
    assert!(total >= 58, "expected at least 58 fixtures, found {total}");
    let mut attempted_combinations = 0usize;
    let mut required_combinations = 0usize;
    let mut narrow_rejections = 0usize;

    for (family, f) in &fixtures {
        for file in &f.all {
            let path = format!("{FIXTURE_DIR}/{file}");
            let source =
                fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));

            let model = if family == "json" {
                RenderSemanticModel::Json(
                    serde_json::from_str(&source).expect("JSON fixture should contain valid JSON"),
                )
            } else {
                merman_core::Engine::new()
                    .parse_diagram_for_render_model_sync(
                        &source,
                        merman_core::ParseOptions::strict(),
                    )
                    .expect("fixture should parse for structured-model verification")
                    .expect("fixture should produce a render model")
                    .model
            };
            let complete = render_source(&source, &MermansiOptions::unicode())
                .unwrap_or_else(|error| panic!("complete render failed for '{file}': {error}"));
            assert_structured_model_round_trips(&model, &complete, file);

            let expected_labels = quoted_fixture_labels(&source);
            for width in [40, 60, 80, 100, 120] {
                for (charset_name, options) in [
                    ("Unicode", MermansiOptions::unicode()),
                    ("ASCII", MermansiOptions::ascii()),
                ] {
                    let options = options
                        .with_output_mode(OutputMode::Concise)
                        .with_max_width(width);
                    let first = render_source(&source, &options);
                    let second = render_source(&source, &options);
                    attempted_combinations += 1;
                    match (first, second) {
                        (Ok(first), Ok(second)) => {
                            assert_eq!(
                                first, second,
                                "nondeterministic {charset_name} output for '{file}' at width \
                                 {width}"
                            );
                            assert_concise_terminal_geometry(
                                family,
                                file,
                                &source,
                                &first,
                                options.charset,
                                width,
                                &expected_labels,
                            );
                            if width >= 80 {
                                required_combinations += 1;
                            }
                        }
                        (Err(first), Err(second)) if width < 80 => {
                            assert!(
                                matches!(first, MermansiError::RenderLimit { .. }),
                                "narrow render for '{file}' at width {width} failed with a \
                                 non-limit error: {first}"
                            );
                            assert_eq!(
                                first.to_string(),
                                second.to_string(),
                                "nondeterministic narrow error for '{file}' at width {width}"
                            );
                            narrow_rejections += 1;
                        }
                        (Err(error), _) | (_, Err(error)) => panic!(
                            "{charset_name} concise render failed for family '{family}' fixture \
                             '{file}' at required width {width}: {error}\nsource:\n{source}"
                        ),
                    }
                }
            }
        }
    }

    assert_eq!(
        attempted_combinations,
        total * 2 * 5,
        "the complete five-width fixture/charset matrix was not attempted"
    );
    assert_eq!(
        required_combinations,
        total * 2 * 3,
        "every fixture and charset must render at 80, 100, and 120 columns"
    );
    assert!(
        narrow_rejections > 0,
        "40/60-column matrix did not exercise typed width degradation"
    );
}

fn assert_concise_terminal_geometry(
    family: &str,
    fixture: &str,
    source: &str,
    output: &str,
    charset: Charset,
    width: usize,
    expected_labels: &BTreeSet<String>,
) {
    const MIN_GEOMETRY_CELLS: usize = 16;

    assert!(
        !output.trim().is_empty(),
        "empty concise output for family '{family}' fixture '{fixture}' at width {width}"
    );
    assert!(
        output.lines().all(|line| str_display_width(line) <= width),
        "overwide concise output for '{fixture}' at width {width}:\n{output}"
    );
    assert!(
        !output.contains(" semantic model]"),
        "semantic-model fallback leaked for '{fixture}' at width {width}:\n{output}"
    );
    assert!(
        terminal_geometry_cells(output, charset) >= MIN_GEOMETRY_CELLS,
        "too little terminal geometry for '{fixture}' at width {width}:\n{output}"
    );
    assert!(
        !contains_structured_text_fallback(output),
        "structured-text fallback leaked for '{fixture}' at width {width}:\n{output}"
    );
    assert_ne!(
        output.trim(),
        source.trim(),
        "source text was returned instead of geometry for '{fixture}' at width {width}"
    );
    if let Some(header) = first_source_header(source) {
        assert!(
            !output.lines().any(|line| line.trim() == header),
            "source header {header:?} leaked for '{fixture}' at width {width}:\n{output}"
        );
    }
    if charset == Charset::Ascii {
        assert!(
            !output.chars().any(is_non_ascii_decoration),
            "ASCII output for '{fixture}' contains Unicode drawing decoration:\n{output}"
        );
    }
    for label in expected_labels {
        assert!(
            output.contains(label),
            "quoted label {label:?} is missing from '{fixture}' at width {width}:\n{output}"
        );
    }
}

fn terminal_geometry_cells(output: &str, charset: Charset) -> usize {
    output
        .chars()
        .filter(|character| {
            let ascii_geometry = matches!(
                character,
                '-' | '|' | '+' | '/' | '\\' | '<' | '>' | '*' | '#' | '=' | 'o'
            );
            ascii_geometry
                || (charset != Charset::Ascii
                    && matches!(
                        character,
                        '\u{2190}'..='\u{21ff}'
                            | '\u{2500}'..='\u{259f}'
                            | '\u{25a0}'..='\u{25ff}'
                    ))
        })
        .count()
}

fn contains_structured_text_fallback(output: &str) -> bool {
    const MARKERS: &[&str] = &[
        "Nodes:",
        "Groups:",
        "Edges:",
        "Blocks:",
        "Boundaries:",
        "Shapes:",
        "Requirements:",
        "Elements:",
        "Relationships:",
        "Flows:",
        "section:",
        "branches:",
    ];

    output.lines().any(|line| {
        let line = line.trim();
        MARKERS.contains(&line) || (line.starts_with("row ") && line.ends_with(':'))
    })
}

fn first_source_header(source: &str) -> Option<&str> {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("%%"))
}

fn quoted_fixture_labels(source: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with("%%") {
            continue;
        }
        let mut characters = line.chars();
        while let Some(character) = characters.next() {
            if character != '"' {
                continue;
            }
            let mut label = String::new();
            let mut escaped = false;
            for character in characters.by_ref() {
                if escaped {
                    label.push(character);
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    break;
                } else {
                    label.push(character);
                }
            }
            if !label.is_empty() {
                labels.insert(label);
            }
        }
    }
    labels
}

fn is_non_ascii_decoration(ch: char) -> bool {
    matches!(
        ch,
        '\u{2190}'..='\u{21ff}' | '\u{2500}'..='\u{259f}' | '\u{2209}' | '\u{2229}'
    )
}

fn assert_structured_model_round_trips(model: &RenderSemanticModel, output: &str, fixture: &str) {
    let (family, expected) = match model {
        RenderSemanticModel::Json(value) => ("json", value.clone()),
        RenderSemanticModel::Mindmap(value) => serialized_model("mindmap", value),
        RenderSemanticModel::State(value) => serialized_model("state", value),
        RenderSemanticModel::Sequence(value) => serialized_model("sequence", value),
        RenderSemanticModel::Flowchart(value) => serialized_model("flowchart", value),
        RenderSemanticModel::Architecture(value) => serialized_model("architecture", value),
        RenderSemanticModel::Class(value) => serialized_model("class", value),
        RenderSemanticModel::C4(value) => serialized_model("c4", value),
        RenderSemanticModel::Kanban(value) => serialized_model("kanban", value),
        RenderSemanticModel::Gantt(value) => serialized_model("gantt", value),
        RenderSemanticModel::Pie(value) => serialized_model("pie", value),
        RenderSemanticModel::Packet(value) => serialized_model("packet", value),
        RenderSemanticModel::Timeline(value) => serialized_model("timeline", value),
        RenderSemanticModel::Journey(value) => serialized_model("journey", value),
        RenderSemanticModel::Requirement(value) => serialized_model("requirement", value),
        RenderSemanticModel::Sankey(value) => serialized_model("sankey", value),
        RenderSemanticModel::Radar(value) => serialized_model("radar", value),
        RenderSemanticModel::Info(value) => serialized_model("info", value),
        RenderSemanticModel::Treemap(value) => serialized_model("treemap", value),
        RenderSemanticModel::Block(value) => serialized_model("block", value),
        RenderSemanticModel::Er(value) => serialized_model("er", value),
        RenderSemanticModel::QuadrantChart(value) => serialized_model("quadrantChart", value),
        RenderSemanticModel::XyChart(value) => serialized_model("xychart", value),
        RenderSemanticModel::GitGraph(value) => serialized_model("gitGraph", value),
        RenderSemanticModel::TreeView(value) => serialized_model("treeView", value),
        RenderSemanticModel::Ishikawa(value) => serialized_model("ishikawa", value),
        RenderSemanticModel::EventModeling(value) => serialized_model("eventmodeling", value),
        RenderSemanticModel::Venn(value) => serialized_model("venn", value),
    };

    let actual = parse_semantic_model(output, family, fixture);
    assert_eq!(
        actual, expected,
        "fixture '{fixture}' dropped semantic data"
    );
}

fn serialized_model<T: Serialize>(
    family: &'static str,
    model: &T,
) -> (&'static str, serde_json::Value) {
    let value = serde_json::to_value(model)
        .unwrap_or_else(|error| panic!("{family} model should serialize: {error}"));
    (family, value)
}

fn parse_semantic_model(output: &str, family: &str, fixture: &str) -> serde_json::Value {
    let marker = format!("[{family} semantic model]\n");
    let (_, json) = output
        .split_once(&marker)
        .unwrap_or_else(|| panic!("fixture '{fixture}' is missing marker '{marker}'"));
    serde_json::from_str(json.trim())
        .unwrap_or_else(|error| panic!("fixture '{fixture}' semantic JSON is invalid: {error}"))
}

#[test]
fn sequence_actor_links_are_preserved_in_delegated_output() {
    let source = r#"sequenceDiagram
participant Alice
participant Bob
link Alice: Documentation @ https://example.com/docs
Alice->>Bob: Open docs
"#;

    let output = render_source(source, &MermansiOptions::unicode())
        .expect("Sequence actor links should render");
    let semantic = parse_semantic_model(&output, "sequence", "inline sequence actor links");

    assert_eq!(
        semantic["actors"]["Alice"]["links"]["Documentation"],
        "https://example.com/docs"
    );
    assert!(
        output
            .split_once("[sequence semantic model]")
            .expect("Sequence semantic marker should exist")
            .0
            .contains("Open docs"),
        "Sequence geometry preview should remain visible:\n{output}"
    );
}

#[test]
fn flowchart_click_metadata_is_preserved_in_delegated_output() {
    let source = r#"flowchart TD
  A[Docs] --> B[Done]
  click A href "https://example.com/docs" "Open documentation" _blank
"#;

    let output = render_source(source, &MermansiOptions::unicode())
        .expect("Flowchart click metadata should render");
    let semantic = parse_semantic_model(&output, "flowchart", "inline flowchart click metadata");
    let node = semantic["nodes"]
        .as_array()
        .expect("Flowchart nodes should be an array")
        .iter()
        .find(|node| node["id"] == "A")
        .expect("Flowchart node A should exist");

    assert_eq!(node["link"], "https://example.com/docs");
    assert_eq!(node["linkTarget"], "_blank");
    assert_eq!(semantic["tooltips"]["A"], "Open documentation");
    assert!(
        output
            .split_once("[flowchart semantic model]")
            .expect("Flowchart semantic marker should exist")
            .0
            .contains("Docs"),
        "Flowchart geometry preview should remain visible:\n{output}"
    );
}

#[test]
fn ansi_modes_do_not_change_rendered_geometry_or_text() {
    let fixtures = collect_fixtures();
    for (_family, fixture_set) in fixtures {
        for file in fixture_set.all {
            let path = format!("{FIXTURE_DIR}/{file}");
            let source = fs::read_to_string(&path).expect("fixture should be readable");
            let plain = render_source(&source, &MermansiOptions::unicode())
                .expect("plain fixture should render");
            for mode in [ColorMode::Ansi16, ColorMode::TrueColor] {
                let colored = render_source(&source, &MermansiOptions::unicode().with_color(mode))
                    .unwrap_or_else(|error| panic!("colored render failed for '{file}': {error}"));
                assert_eq!(
                    strip_ansi(&colored),
                    plain,
                    "color mode {mode:?} changed fixture '{file}' geometry or text"
                );
            }
        }
    }
}

#[test]
fn zenuml_fixture_renders_as_sequence() {
    // ZenUML is a parser alias that transforms into the Sequence render model.
    // Verify it parses and renders successfully.
    let source = include_str!("fixtures/zenuml.en.mmd");
    let output = render_source(source, &MermansiOptions::unicode())
        .expect("ZenUML English fixture should render");
    assert!(!output.trim().is_empty());

    let source_zh = include_str!("fixtures/zenuml.zh.mmd");
    let output_zh = render_source(source_zh, &MermansiOptions::unicode())
        .expect("ZenUML Chinese fixture should render");
    assert!(!output_zh.trim().is_empty());
}

#[test]
fn graph_alias_renders_as_flowchart() {
    // `graph` is an alias for `flowchart`. Verify it renders.
    let source = "graph TD\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode())
        .expect("'graph' alias should parse and render");
    assert!(!output.trim().is_empty());
}

#[test]
fn flowchart_parser_aliases_render_as_flowchart() {
    for (alias, source) in [
        ("flowchart-v2", "flowchart-v2 TD\n  A --> B"),
        (
            "flowchart-v2 with directive",
            "%%{init: {\"theme\":\"default\"}}%%\nflowchart-v2 TD\n  A --> B",
        ),
        ("flowchart-elk", "flowchart-elk TD\n  A --> B"),
    ] {
        let output = render_source(source, &MermansiOptions::unicode())
            .unwrap_or_else(|error| panic!("'{alias}' should parse and render: {error}"));
        let semantic = parse_semantic_model(&output, "flowchart", alias);
        assert_eq!(semantic["direction"], "TB");
        assert_eq!(
            semantic["edges"].as_array().map(Vec::len),
            Some(1),
            "'{alias}' should resolve to a Flowchart model"
        );
    }
}

#[test]
fn state_diagram_v1_alias_renders() {
    let source = "stateDiagram\n  [*] --> Active";
    let output = render_source(source, &MermansiOptions::unicode())
        .expect("'stateDiagram' alias should parse and render");
    assert!(!output.trim().is_empty());
}

#[test]
fn class_diagram_v1_alias_renders() {
    let source = "classDiagram\n  class Animal {\n    +String name\n  }";
    let output = render_source(source, &MermansiOptions::unicode())
        .expect("'classDiagram' alias should parse and render");
    assert!(!output.trim().is_empty());
}

#[test]
fn json_source_fixtures_produce_output() {
    for (file, expected) in [("json.en.mmd", "Alice"), ("json.zh.mmd", "爱丽丝")] {
        let path = format!("{FIXTURE_DIR}/{file}");
        let source = fs::read_to_string(&path).expect("JSON fixture should be readable");
        let value = serde_json::from_str(&source).expect("JSON fixture should contain valid JSON");
        let model = RenderSemanticModel::Json(value);

        for options in [MermansiOptions::unicode(), MermansiOptions::ascii()] {
            let output = render_source(&source, &options).expect("JSON source should render");
            assert!(!output.trim().is_empty(), "JSON output should be nonempty");
            assert!(
                output.contains(expected),
                "JSON output should contain '{expected}':\n{output}"
            );
            assert_structured_model_round_trips(&model, &output, file);
            let repeated =
                render_source(&source, &options).expect("JSON render should be deterministic");
            assert_eq!(output, repeated, "JSON output should be deterministic");
        }
    }
}
