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
use mermansi::{ColorMode, MermansiOptions, render_model, render_source};
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
fn every_fixture_renders_nonempty_in_unicode_and_ascii() {
    let fixtures = collect_fixtures();
    assert!(!fixtures.is_empty(), "no fixtures found in {FIXTURE_DIR}");

    let total: usize = fixtures.values().map(|f| f.all.len()).sum();
    assert!(total >= 58, "expected at least 58 fixtures, found {total}");

    for (family, f) in &fixtures {
        if family == "json" {
            continue;
        }
        for file in &f.all {
            let path = format!("{FIXTURE_DIR}/{file}");
            let source =
                fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));

            let unicode = render_source(&source, &MermansiOptions::unicode()).unwrap_or_else(|e| {
                panic!(
                    "Unicode render failed for family '{family}' fixture '{file}':\n\
                         source:\n{source}\n\
                         error: {e}"
                )
            });
            assert!(
                !unicode.trim().is_empty(),
                "Unicode output for family '{family}' fixture '{file}' is empty"
            );

            let ascii = render_source(&source, &MermansiOptions::ascii()).unwrap_or_else(|e| {
                panic!(
                    "ASCII render failed for family '{family}' fixture '{file}':\n\
                         source:\n{source}\n\
                         error: {e}"
                )
            });
            assert!(
                !ascii.trim().is_empty(),
                "ASCII output for family '{family}' fixture '{file}' is empty"
            );
            assert!(
                !ascii.chars().any(is_non_ascii_decoration),
                "ASCII output for family '{family}' fixture '{file}' contains Unicode drawing decoration:\n{ascii}"
            );

            let engine = merman_core::Engine::new();
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, merman_core::ParseOptions::strict())
                .expect("fixture should parse for structured-model verification")
                .expect("fixture should produce a render model");
            assert_structured_model_round_trips(&parsed.model, &unicode, file);

            // Determinism: re-render and compare.
            let unicode2 = render_source(&source, &MermansiOptions::unicode())
                .expect("determinism re-render (unicode) should succeed");
            let ascii2 = render_source(&source, &MermansiOptions::ascii())
                .expect("determinism re-render (ascii) should succeed");
            assert_eq!(
                unicode, unicode2,
                "Unicode output is not deterministic for family '{family}' fixture '{file}'"
            );
            assert_eq!(
                ascii, ascii2,
                "ASCII output is not deterministic for family '{family}' fixture '{file}'"
            );
        }
    }
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
        RenderSemanticModel::Architecture(value) => (
            "architecture",
            serde_json::to_value(value).expect("Architecture model should serialize"),
        ),
        RenderSemanticModel::C4(value) => (
            "c4",
            serde_json::to_value(value).expect("C4 model should serialize"),
        ),
        RenderSemanticModel::Pie(value) => (
            "pie",
            serde_json::to_value(value).expect("Pie model should serialize"),
        ),
        RenderSemanticModel::Requirement(value) => (
            "requirement",
            serde_json::to_value(value).expect("Requirement model should serialize"),
        ),
        RenderSemanticModel::Sankey(value) => (
            "sankey",
            serde_json::to_value(value).expect("Sankey model should serialize"),
        ),
        RenderSemanticModel::Radar(value) => (
            "radar",
            serde_json::to_value(value).expect("Radar model should serialize"),
        ),
        RenderSemanticModel::Info(value) => (
            "info",
            serde_json::to_value(value).expect("Info model should serialize"),
        ),
        RenderSemanticModel::Treemap(value) => (
            "treemap",
            serde_json::to_value(value).expect("Treemap model should serialize"),
        ),
        RenderSemanticModel::Block(value) => (
            "block",
            serde_json::to_value(value).expect("Block model should serialize"),
        ),
        RenderSemanticModel::QuadrantChart(value) => (
            "quadrantChart",
            serde_json::to_value(value).expect("QuadrantChart model should serialize"),
        ),
        RenderSemanticModel::Ishikawa(value) => (
            "ishikawa",
            serde_json::to_value(value).expect("Ishikawa model should serialize"),
        ),
        RenderSemanticModel::EventModeling(value) => (
            "eventmodeling",
            serde_json::to_value(value).expect("EventModeling model should serialize"),
        ),
        RenderSemanticModel::Venn(value) => (
            "venn",
            serde_json::to_value(value).expect("Venn model should serialize"),
        ),
        RenderSemanticModel::Mindmap(_)
        | RenderSemanticModel::State(_)
        | RenderSemanticModel::Sequence(_)
        | RenderSemanticModel::Flowchart(_)
        | RenderSemanticModel::Class(_)
        | RenderSemanticModel::Kanban(_)
        | RenderSemanticModel::Gantt(_)
        | RenderSemanticModel::Packet(_)
        | RenderSemanticModel::Timeline(_)
        | RenderSemanticModel::Journey(_)
        | RenderSemanticModel::Er(_)
        | RenderSemanticModel::XyChart(_)
        | RenderSemanticModel::GitGraph(_)
        | RenderSemanticModel::TreeView(_) => return,
    };

    let marker = format!("[{family} semantic model]\n");
    let (_, json) = output
        .split_once(&marker)
        .unwrap_or_else(|| panic!("fixture '{fixture}' is missing marker '{marker}'"));
    let actual = serde_json::from_str::<serde_json::Value>(json.trim())
        .unwrap_or_else(|error| panic!("fixture '{fixture}' semantic JSON is invalid: {error}"));
    assert_eq!(
        actual, expected,
        "fixture '{fixture}' dropped semantic data"
    );
}

#[test]
fn ansi_modes_do_not_change_rendered_geometry_or_text() {
    let fixtures = collect_fixtures();
    for (family, fixture_set) in fixtures {
        if family == "json" {
            continue;
        }
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
fn flowchart_alias_renders_as_flowchart() {
    // `flowchart-v2` and `flowchart-elk` are aliases for `flowchart`.
    let source = "flowchart TD\n  A --> B";
    let output = render_source(source, &MermansiOptions::unicode())
        .expect("'flowchart' should parse and render");
    assert!(!output.trim().is_empty());
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
fn json_render_model_adapter_produces_output() {
    for (file, expected) in [("json.en.mmd", "Alice"), ("json.zh.mmd", "爱丽丝")] {
        let path = format!("{FIXTURE_DIR}/{file}");
        let source = fs::read_to_string(&path).expect("JSON fixture should be readable");
        let value = serde_json::from_str(&source).expect("JSON fixture should contain valid JSON");
        let model = RenderSemanticModel::Json(value);

        for options in [MermansiOptions::unicode(), MermansiOptions::ascii()] {
            let output = render_model(&model, &options).expect("JSON render model should render");
            assert!(!output.trim().is_empty(), "JSON output should be nonempty");
            assert!(
                output.contains(expected),
                "JSON output should contain '{expected}':\n{output}"
            );
            assert_structured_model_round_trips(&model, &output, file);
            let repeated =
                render_model(&model, &options).expect("JSON render should be deterministic");
            assert_eq!(output, repeated, "JSON output should be deterministic");
        }
    }
}
