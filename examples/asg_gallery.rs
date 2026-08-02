use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use mermansi::str_display_width;
use serde_json::json;

const EXPECTED_FAMILY_COUNT: usize = 29;

struct Config {
    project: PathBuf,
    output: PathBuf,
    mermansi_bin: PathBuf,
    render_width: usize,
    terminal_width: usize,
    max_height: usize,
}

struct GalleryEntry {
    id: String,
    kind: &'static str,
    source: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse()?;
    let entries = gallery_entries(&config.project)?;
    let text_root = config.output.join("text");
    let cast_root = config.output.join("casts");
    let svg_root = config.output.join("svg");
    fs::create_dir_all(&text_root)?;
    fs::create_dir_all(&cast_root)?;
    fs::create_dir_all(&svg_root)?;

    let mut manifest = Vec::with_capacity(entries.len());
    let mut figures = String::new();
    for entry in entries {
        let rendered = render_cli(&config, &entry.source)?;
        let rows = rendered.lines().count();
        let columns = rendered.lines().map(str_display_width).max().unwrap_or(0);
        let terminal_rows = u16::try_from(rows.saturating_add(2).max(3))?;
        let terminal_columns = u16::try_from(config.terminal_width)?;
        let terminal_text = format!(
            "\u{1b}[1;36mMERMAID / {}\u{1b}[0m\r\n{}",
            entry.id.to_ascii_uppercase(),
            rendered.replace('\n', "\r\n"),
        );
        let header = json!({
            "version": 3,
            "term": {
                "cols": terminal_columns,
                "rows": terminal_rows,
                "type": "xterm-256color",
            },
            "title": format!("mermansi / {}", entry.id),
        });
        let event = json!([0.001, "o", terminal_text]);
        fs::write(text_root.join(format!("{}.txt", entry.id)), &rendered)?;
        fs::write(
            cast_root.join(format!("{}.cast", entry.id)),
            format!("{header}\n{event}\n"),
        )?;

        let fixture = entry
            .source
            .strip_prefix(&config.project)
            .unwrap_or(&entry.source)
            .to_string_lossy();
        manifest.push(json!({
            "id": entry.id,
            "kind": entry.kind,
            "fixture": fixture,
            "render_columns": columns,
            "render_rows": rows,
            "terminal_columns": terminal_columns,
            "terminal_rows": terminal_rows,
            "text": format!("text/{}.txt", entry.id),
            "cast": format!("casts/{}.cast", entry.id),
            "svg": format!("svg/{}.svg", entry.id),
        }));
        figures.push_str(&format!(
            "<figure id=\"{id}\"><figcaption><strong>{id}</strong><span>{kind} · {columns}x{rows}</span></figcaption><a href=\"svg/{id}.svg\"><img loading=\"lazy\" src=\"svg/{id}.svg\" alt=\"{id} terminal rendering\"></a></figure>",
            id = entry.id,
            kind = entry.kind,
        ));
    }

    fs::write(
        config.output.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    fs::write(config.output.join("index.html"), gallery_html(&figures))?;
    println!("{}", config.output.display());
    Ok(())
}

impl Config {
    fn parse() -> Result<Self, Box<dyn Error>> {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut config = Self {
            output: project.join(".aicode/state/asg-gallery"),
            mermansi_bin: project.join("target/debug/mermansi"),
            render_width: 95,
            terminal_width: 100,
            max_height: 1_000,
            project,
        };
        let mut args = env::args_os().skip(1);
        while let Some(argument) = args.next() {
            match argument.to_string_lossy().as_ref() {
                "--output" => config.output = PathBuf::from(required_value(&mut args, "--output")?),
                "--mermansi-bin" => {
                    config.mermansi_bin =
                        PathBuf::from(required_value(&mut args, "--mermansi-bin")?)
                }
                "--render-width" => {
                    config.render_width = positive_usize(
                        required_value(&mut args, "--render-width")?,
                        "--render-width",
                    )?
                }
                "--terminal-width" => {
                    config.terminal_width = positive_usize(
                        required_value(&mut args, "--terminal-width")?,
                        "--terminal-width",
                    )?
                }
                "--max-height" => {
                    config.max_height =
                        positive_usize(required_value(&mut args, "--max-height")?, "--max-height")?
                }
                other => return Err(invalid_input(format!("unknown argument: {other}")).into()),
            }
        }
        Ok(config)
    }
}

fn required_value(args: &mut impl Iterator<Item = OsString>, name: &str) -> io::Result<OsString> {
    args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} requires a value"),
        )
    })
}

fn positive_usize(value: OsString, name: &str) -> io::Result<usize> {
    let text = value
        .to_str()
        .ok_or_else(|| invalid_input(format!("{name} must be UTF-8")))?;
    let parsed = text
        .parse::<usize>()
        .map_err(|_| invalid_input(format!("{name} must be a positive integer")))?;
    if parsed == 0 {
        return Err(invalid_input(format!("{name} must be greater than zero")));
    }
    Ok(parsed)
}

fn invalid_input(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn gallery_entries(project: &Path) -> Result<Vec<GalleryEntry>, Box<dyn Error>> {
    let fixture_root = project.join("tests/fixtures");
    let mut families = fs::read_dir(&fixture_root)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".zh.mmd")?.to_owned();
            Some(GalleryEntry {
                id,
                kind: "family",
                source: entry.path(),
            })
        })
        .collect::<Vec<_>>();
    families.sort_by(|left, right| left.id.cmp(&right.id));
    if families.len() != EXPECTED_FAMILY_COUNT {
        return Err(invalid_input(format!(
            "expected {EXPECTED_FAMILY_COUNT} Chinese family fixtures, found {}",
            families.len()
        ))
        .into());
    }

    for (id, relative) in [
        (
            "deployment-flowchart",
            "tests/scenarios/deployment.flowchart.mmd",
        ),
        ("deployment-c4", "tests/scenarios/deployment.c4.mmd"),
        ("ipv4-packet", "tests/scenarios/ipv4.packet.mmd"),
        (
            "rack-architecture-alternative",
            "tests/scenarios/rack-alternative.architecture.mmd",
        ),
    ] {
        families.push(GalleryEntry {
            id: id.to_owned(),
            kind: "scenario",
            source: project.join(relative),
        });
    }
    Ok(families)
}

fn render_cli(config: &Config, source: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new(&config.mermansi_bin)
        .arg("--file")
        .arg(source)
        .arg("--unicode")
        .arg("--no-color")
        .arg("--concise")
        .arg("--width")
        .arg(config.render_width.to_string())
        .arg("--height")
        .arg(config.max_height.to_string())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "mermansi failed for {}: {}",
            source.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn gallery_html(figures: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>mermansi ASG visual audit</title><style>html{{color-scheme:dark}}body{{margin:0;background:#111418;color:#e8edf2;font:14px ui-monospace,SFMono-Regular,Menlo,monospace}}header{{position:sticky;top:0;z-index:1;padding:12px 20px;background:#171b20;border-bottom:1px solid #343a42}}h1{{margin:0;font-size:16px}}main{{display:grid;grid-template-columns:repeat(auto-fit,minmax(420px,1fr));gap:16px;padding:16px}}figure{{min-width:0;margin:0;border:1px solid #343a42;background:#20252b}}figcaption{{display:flex;justify-content:space-between;padding:8px 10px;color:#a9bac8;border-bottom:1px solid #343a42}}img{{display:block;width:100%;height:auto;background:#0d1117}}</style></head><body><header><h1>mermansi / ASG / 29 families + scenarios</h1></header><main>{figures}</main></body></html>"
    )
}
