use mermansi::{MermansiError, MermansiOptions, OutputMode, render_source, str_display_width};

const IPV4_PACKET: &str = include_str!("scenarios/ipv4.packet.mmd");
const C4_DEPLOYMENT: &str = include_str!("scenarios/deployment.c4.mmd");
const RACK_ALTERNATIVE: &str = include_str!("scenarios/rack-alternative.architecture.mmd");
const INVALID_RACK: &str = include_str!("scenarios/invalid/rack-beta.mmd");
const INVALID_PACKET: &str = include_str!("scenarios/invalid/packet-unquoted.mmd");

fn concise() -> MermansiOptions {
    MermansiOptions::unicode()
        .with_output_mode(OutputMode::Concise)
        .with_max_width(95)
        .with_max_height(200)
}

fn assert_bounded_geometry(output: &str) {
    assert!(!output.trim().is_empty());
    assert!(!output.contains(" semantic model]"), "{output}");
    assert!(
        output.lines().all(|line| str_display_width(line) <= 95),
        "{output}"
    );
}

#[test]
fn rack_beta_stays_outside_the_pinned_parser_inventory() {
    assert!(matches!(
        render_source(INVALID_RACK, &concise()),
        Err(MermansiError::Parse(_))
    ));
    assert!(
        merman_core::diagram_family_capabilities()
            .iter()
            .all(|capability| capability.diagram_type != "rack-beta")
    );

    let architecture = render_source(RACK_ALTERNATIVE, &concise()).unwrap();
    assert_bounded_geometry(&architecture);
    for label in [
        "机架部署替代架构",
        "机架区域",
        "入口网关",
        "计算节点1",
        "计算节点2",
        "存储节点",
    ] {
        assert!(
            architecture.contains(label),
            "missing {label:?}:\n{architecture}"
        );
    }

    let block = render_source(include_str!("fixtures/block.zh.mmd"), &concise()).unwrap();
    assert_bounded_geometry(&block);
}

#[test]
fn packet_fields_require_quotes_and_complex_ipv4_renders() {
    assert!(matches!(
        render_source(INVALID_PACKET, &concise()),
        Err(MermansiError::Parse(_))
    ));

    let output = render_source(IPV4_PACKET, &concise()).unwrap();
    assert_bounded_geometry(&output);
    for label in [
        "IPv4 数据包",
        "版本",
        "首部长度",
        "服务类型",
        "总长度",
        "标识",
        "标志",
        "片偏移",
        "生存时间",
        "协议",
        "首部校验和",
        "源地址",
        "目的地址",
    ] {
        assert!(output.contains(label), "missing {label:?}:\n{output}");
    }
    for word in ["0:", "32:", "64:", "96:", "128:"] {
        assert!(output.contains(word), "missing word {word}:\n{output}");
    }
    assert_eq!(output.matches('┌').count(), 5, "{output}");
    assert_eq!(output.matches('┘').count(), 5, "{output}");
}

#[test]
fn c4deployment_is_supported_and_preserves_nested_topology() {
    let output = render_source(C4_DEPLOYMENT, &concise()).unwrap();
    assert_bounded_geometry(&output);
    for label in [
        "C4Deployment",
        "部署架构",
        "云环境",
        "Kubernetes",
        "数据库节点",
        "API",
        "Worker",
        "PostgreSQL",
        "读写",
        "写入",
    ] {
        assert!(output.contains(label), "missing {label:?}:\n{output}");
    }
    assert!(output.lines().any(|line| line.contains("api --> db")));
    assert!(output.lines().any(|line| line.contains("worker --> db")));
    assert!(output.matches('┌').count() >= 6, "{output}");
    assert!(output.matches('┘').count() >= 6, "{output}");
}
