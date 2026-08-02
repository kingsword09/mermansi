use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

const FAMILY_COUNT: usize = 29;
const SCENARIOS: &[(&str, &str)] = &[
    ("deployment-c4", "tests/scenarios/deployment.c4.mmd"),
    (
        "deployment-flowchart",
        "tests/scenarios/deployment.flowchart.mmd",
    ),
    ("ipv4-packet", "tests/scenarios/ipv4.packet.mmd"),
    (
        "rack-architecture-alternative",
        "tests/scenarios/rack-alternative.architecture.mmd",
    ),
];

#[test]
fn readme_embeds_the_exact_supported_svg_gallery() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let families = family_ids(root);
    assert_eq!(
        families.len(),
        FAMILY_COUNT,
        "the gallery contract must be updated when the supported family inventory changes"
    );

    let mut expected = families.clone();
    expected.extend(SCENARIOS.iter().map(|(id, _)| (*id).to_owned()));
    let actual = svg_ids(&root.join("docs/gallery"));
    assert_eq!(
        actual, expected,
        "docs/gallery must contain exactly 29 family and four scenario SVGs"
    );

    let readme = fs::read_to_string(root.join("README.md")).expect("README.md must be readable");
    assert!(
        readme.contains("PUBLISH_README=1"),
        "README must document the explicit gallery publication command"
    );

    for family in families {
        assert_gallery_entry(
            root,
            &readme,
            &family,
            &format!("tests/fixtures/{family}.zh.mmd"),
        );
    }
    for (id, source) in SCENARIOS {
        assert_gallery_entry(root, &readme, id, source);
    }
}

fn family_ids(root: &Path) -> BTreeSet<String> {
    fs::read_dir(root.join("tests/fixtures"))
        .expect("tests/fixtures must be readable")
        .map(|entry| entry.expect("fixture entry must be readable"))
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".zh.mmd"))
                .map(str::to_owned)
        })
        .collect()
}

fn svg_ids(gallery: &Path) -> BTreeSet<String> {
    fs::read_dir(gallery)
        .expect("docs/gallery must be readable")
        .map(|entry| entry.expect("gallery entry must be readable"))
        .filter(|entry| entry.path().extension() == Some(OsStr::new("svg")))
        .map(|entry| {
            entry
                .path()
                .file_stem()
                .expect("SVG must have a file stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn assert_gallery_entry(root: &Path, readme: &str, id: &str, source: &str) {
    let asset = format!("docs/gallery/{id}.svg");
    assert!(
        readme.contains(&format!("src=\"{asset}\"")),
        "README must embed {asset}"
    );
    assert!(
        readme.contains(&format!("href=\"{source}\"")),
        "README must link {id} to {source}"
    );

    let svg = fs::read_to_string(root.join(&asset))
        .unwrap_or_else(|error| panic!("{asset} must be readable UTF-8: {error}"));
    let svg = svg.trim();
    assert!(
        svg.starts_with("<svg ") && svg.ends_with("</svg>"),
        "{asset} must contain a complete SVG root"
    );
    assert!(
        svg.contains("viewBox=") && svg.contains("MERMAID / "),
        "{asset} must remain a real ASG terminal capture"
    );
}
