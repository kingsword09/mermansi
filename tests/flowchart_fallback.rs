use mermansi::ansi::strip_ansi;
use mermansi::{
    Charset, ColorMode, MermansiError, MermansiOptions, OutputMode, render_source,
    str_display_width,
};

const DEPLOYMENT_SOURCE: &str = include_str!("scenarios/deployment.flowchart.mmd");
const C4_DEPLOYMENT_SOURCE: &str = include_str!("scenarios/deployment.c4.mmd");
const RACK_ARCHITECTURE_SOURCE: &str = include_str!("scenarios/rack-alternative.architecture.mmd");

fn deployment_source(direction: &str) -> String {
    DEPLOYMENT_SOURCE.replacen("flowchart TB", &format!("flowchart {direction}"), 1)
}

fn concise_options(charset: Charset, width: usize) -> MermansiOptions {
    let options = match charset {
        Charset::Unicode => MermansiOptions::unicode(),
        Charset::Ascii => MermansiOptions::ascii(),
        _ => panic!("test requires a built-in charset"),
    };
    options
        .with_output_mode(OutputMode::Concise)
        .with_max_width(width)
        .with_max_height(200)
}

fn label_position(output: &str, label: &str) -> (usize, usize) {
    output
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            line.find(label)
                .map(|byte| (row, str_display_width(&line[..byte])))
        })
        .unwrap_or_else(|| panic!("missing label {label:?}:\n{output}"))
}

fn relationship_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains(" --> ") || line.contains(" ..> "))
        .collect()
}

#[test]
fn deployment_flowchart_uses_complete_native_geometry() {
    let source = deployment_source("TB");
    let output = render_source(&source, &concise_options(Charset::Unicode, 95)).unwrap();

    assert!(!output.contains("[flowchart semantic model]"), "{output}");
    for label in [
        "用户端",
        "负载层",
        "应用层",
        "数据层",
        "Web浏览器",
        "移动APP",
        "Nginx负载均衡",
        "应用服务器1",
        "应用服务器2",
        "MySQL主",
        "MySQL从",
        "Redis集群",
    ] {
        assert!(output.contains(label), "missing {label:?}:\n{output}");
    }

    let relationships = relationship_lines(&output);
    assert_eq!(relationships.len(), 9, "{output}");
    for expected in [
        "Web --> LB",
        "APP --> LB",
        "LB --> APP1",
        "LB --> APP2",
        "APP1 --> Master",
        "APP2 --> Slave",
        "APP1 --> Cache",
        "APP2 --> Cache",
    ] {
        assert!(
            relationships.iter().any(|line| line.contains(expected)),
            "missing relationship {expected:?}:\n{output}"
        );
    }
    assert!(
        relationships
            .iter()
            .any(|line| line.contains("Master ..> Slave") && line.contains("同步")),
        "dotted labeled relationship is missing:\n{output}"
    );
    assert!(
        output.matches('▼').count() >= 4,
        "the compact grouped geometry lost a representative route:\n{output}"
    );

    let database_line = output
        .lines()
        .find(|line| line.contains("MySQL主"))
        .expect("database node label");
    assert!(
        database_line.contains('(') && database_line.contains(')'),
        "database node is not visibly distinct:\n{output}"
    );
    let rectangle_line = output
        .lines()
        .find(|line| line.contains("Web浏览器"))
        .expect("rectangle node label");
    assert!(
        rectangle_line.contains('│') && !rectangle_line.contains('('),
        "rectangle node shape changed:\n{output}"
    );
    assert!(
        output.lines().all(|line| str_display_width(line) <= 95),
        "deployment output exceeded 95 columns:\n{output}"
    );
}

#[test]
fn deployment_fallback_is_bounded_deterministic_and_directional() {
    for direction in ["TD", "TB", "BT", "LR", "RL"] {
        for charset in [Charset::Unicode, Charset::Ascii] {
            for width in [95, 115] {
                let source = deployment_source(direction);
                let options = concise_options(charset, width);
                let first = render_source(&source, &options).unwrap_or_else(|error| {
                    panic!("{direction} {charset:?} width {width} failed: {error}")
                });
                let second = render_source(&source, &options).unwrap();
                assert_eq!(first, second, "{direction} {charset:?} width {width}");
                assert!(!first.contains(" semantic model]"), "{first}");
                assert_eq!(relationship_lines(&first).len(), 9, "{first}");
                assert!(
                    first.lines().all(|line| str_display_width(line) <= width),
                    "{direction} {charset:?} width {width}:\n{first}"
                );
                if charset == Charset::Ascii {
                    assert!(
                        !first.chars().any(|character| matches!(
                            character,
                            '\u{2190}'..='\u{21ff}'
                                | '\u{2500}'..='\u{259f}'
                                | '\u{25a0}'..='\u{25ff}'
                        )),
                        "ASCII geometry leaked Unicode decoration:\n{first}"
                    );
                }

                let user = label_position(&first, "用户端");
                let data = label_position(&first, "数据层");
                match direction {
                    "TD" | "TB" => assert!(user.0 < data.0, "{direction}:\n{first}"),
                    "BT" => assert!(user.0 > data.0, "{direction}:\n{first}"),
                    "LR" => assert!(user.1 < data.1, "{direction}:\n{first}"),
                    "RL" => assert!(user.1 > data.1, "{direction}:\n{first}"),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn deployment_fallback_ansi_does_not_change_geometry() {
    let source = deployment_source("TB");
    let plain_options = concise_options(Charset::Unicode, 95);
    let plain = render_source(&source, &plain_options).unwrap();
    for color in [ColorMode::Ansi16, ColorMode::TrueColor] {
        let colored = render_source(&source, &plain_options.with_color(color)).unwrap();
        assert_eq!(strip_ansi(&colored), plain, "{color:?}");
    }
}

#[test]
fn nested_subgraphs_render_closed_concise_geometry() {
    let source = "flowchart TD\n\
        subgraph Outer\n\
          subgraph Inner\n\
            A[Alpha] --> B[Beta]\n\
          end\n\
          C[Gamma] --> D[Delta]\n\
        end";
    let output = render_source(source, &concise_options(Charset::Unicode, 95)).unwrap();
    assert!(!output.contains(" semantic model]"), "{output}");
    for label in ["Outer", "Inner", "Alpha", "Beta", "Gamma", "Delta"] {
        assert!(output.contains(label), "missing {label:?}:\n{output}");
    }
    assert_eq!(output.matches('▼').count(), 2, "{output}");
    assert!(output.matches('┌').count() >= 6, "{output}");
    assert!(output.matches('┘').count() >= 6, "{output}");
}

#[test]
fn connected_decision_uses_one_closed_native_shape() {
    let source = "flowchart TD\n\
        A[开始] --> B{决策}\n\
        B -->|是| C[处理]\n\
        B -->|否| D[结束]\n\
        C --> D";
    let output = render_source(source, &concise_options(Charset::Unicode, 95)).unwrap();

    assert!(!output.contains(" semantic model]"), "{output}");
    for label in ["开始", "决策", "处理", "结束", "是", "否"] {
        assert!(output.contains(label), "missing {label:?}:\n{output}");
    }
    assert_eq!(output.matches('╱').count(), 2, "{output}");
    assert_eq!(output.matches('╲').count(), 2, "{output}");
    assert_eq!(relationship_lines(&output).len(), 4, "{output}");
    assert!(
        output.lines().all(|line| str_display_width(line) <= 95),
        "{output}"
    );

    let ascii = render_source(source, &concise_options(Charset::Ascii, 95)).unwrap();
    assert_eq!(ascii.matches('/').count(), 2, "{ascii}");
    assert_eq!(ascii.matches('\\').count(), 2, "{ascii}");
}

#[test]
fn deployment_fallback_rejects_an_insufficient_height() {
    let source = deployment_source("TB");
    let result = render_source(
        &source,
        &MermansiOptions::unicode()
            .with_output_mode(OutputMode::Concise)
            .with_max_width(95)
            .with_max_height(8),
    );
    assert!(matches!(
        result,
        Err(MermansiError::RenderLimit {
            context: "box geometry rows",
            ..
        })
    ));
}

#[test]
fn box_family_routes_preserve_closed_boundaries() {
    for source in [
        DEPLOYMENT_SOURCE,
        C4_DEPLOYMENT_SOURCE,
        RACK_ARCHITECTURE_SOURCE,
    ] {
        for charset in [Charset::Unicode, Charset::Ascii] {
            let output = render_source(source, &concise_options(charset, 95)).unwrap();
            assert!(
                output.lines().all(|line| str_display_width(line) <= 95),
                "{output}"
            );
            assert!(!output.contains(" semantic model]"), "{output}");
            if charset == Charset::Ascii {
                continue;
            }

            for corner in ['┌', '┐', '└', '┘'] {
                assert!(
                    output.matches(corner).count() >= 5,
                    "missing closed boundaries:\n{output}"
                );
            }
            for line in output.lines() {
                assert!(
                    !(line.contains('├') && line.contains('┘')),
                    "route folded into a closing border:\n{output}"
                );
                if line.contains('└') && line.contains('┘') {
                    assert!(
                        line.matches('┼').count() <= 1,
                        "route followed a bottom border:\n{output}"
                    );
                }
            }
        }
    }
}
