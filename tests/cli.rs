//! End-to-end CLI conformance tests.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const FIXTURE_DIR: &str = "tests/fixtures";

fn run_cli(source: &str, args: &[&str]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mermansi"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI should start");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(source.as_bytes())
        .expect("fixture should be written");
    child.wait_with_output().expect("CLI should exit")
}

#[test]
fn every_mermaid_fixture_renders_through_cli_in_both_charsets() {
    for entry in fs::read_dir(FIXTURE_DIR).expect("fixture directory should exist") {
        let entry = entry.expect("fixture entry should be readable");
        let file = entry.file_name().to_string_lossy().into_owned();
        if !file.ends_with(".mmd") {
            continue;
        }
        let source = fs::read_to_string(entry.path()).expect("fixture should be readable");
        for args in [&[][..], &["--ascii"][..]] {
            let output = run_cli(&source, args);
            assert!(
                output.status.success(),
                "CLI failed for '{file}' with {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                !output.stdout.is_empty(),
                "CLI output was empty for '{file}' with {args:?}"
            );
        }
    }
}

#[test]
fn cli_exit_codes_distinguish_parse_render_and_option_errors() {
    let parse = run_cli("not a mermaid diagram", &[]);
    assert_eq!(parse.status.code(), Some(1));

    let json_parse = run_cli("{\"broken\":}", &[]);
    assert_eq!(json_parse.status.code(), Some(1));

    let render = run_cli("flowchart TD\nA[long label] --> B", &["--width", "2"]);
    assert_eq!(render.status.code(), Some(2));

    let option = run_cli("flowchart TD\nA --> B", &["--width", "0"]);
    assert_eq!(option.status.code(), Some(3));
}

#[test]
fn concise_flag_emits_only_terminal_geometry() {
    let source = "flowchart TD\nA[开始] --> B[结束]";
    let concise = run_cli(source, &["--concise", "--width", "95"]);
    assert!(concise.status.success());
    let concise = String::from_utf8(concise.stdout).unwrap();
    assert!(concise.contains("开始"));
    assert!(concise.contains("结束"));
    assert!(!concise.contains("[flowchart semantic model]"));

    let complete = run_cli(source, &["--complete", "--width", "95"]);
    assert!(complete.status.success());
    assert!(
        String::from_utf8(complete.stdout)
            .unwrap()
            .contains("[flowchart semantic model]")
    );
}
