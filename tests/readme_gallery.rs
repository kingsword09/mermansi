use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

const FAMILY_COUNT: usize = 29;
const ASG_CELL_HEIGHT: usize = 22;
const SHOWCASE_IDS: &[&str] = &[
    "ARCHITECTURE",
    "FLOWCHART",
    "GITGRAPH",
    "MINDMAP",
    "PACKET",
    "PIE",
    "SEQUENCE",
    "TREEMAP",
    "VENN",
];
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
    // The animated hero is a composite asset, not a per-family gallery entry.
    expected.insert("showcase".to_owned());
    let actual = svg_ids(&root.join("docs/gallery"));
    assert_eq!(
        actual, expected,
        "docs/gallery must contain 29 family SVGs, four scenario SVGs, and the showcase hero"
    );

    let readme = fs::read_to_string(root.join("README.md")).expect("README.md must be readable");
    assert!(
        readme.contains("PUBLISH_README=1"),
        "README must document the explicit gallery publication command"
    );
    assert!(
        readme.contains("34-asset gate"),
        "README must describe the family/scenario/showcase publication gate accurately"
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

    // The showcase hero is embedded once near the top of the README.
    let hero_asset = "docs/gallery/showcase.svg";
    assert!(
        readme.contains(&format!("src=\"{hero_asset}\"")),
        "README must embed the {hero_asset} hero"
    );
    let hero_svg = fs::read_to_string(root.join(hero_asset))
        .unwrap_or_else(|error| panic!("{hero_asset} must be readable UTF-8: {error}"));
    let hero_svg = hero_svg.trim();
    assert!(
        hero_svg.starts_with("<svg ") && hero_svg.ends_with("</svg>"),
        "{hero_asset} must contain a complete SVG root"
    );
    assert!(
        hero_svg.contains("@keyframes"),
        "{hero_asset} must remain an animated ASG capture"
    );
    assert!(
        svg_dimension(hero_svg, "height") <= 700,
        "{hero_asset} must remain compact enough for the README"
    );
    for (index, id) in SHOWCASE_IDS.iter().enumerate() {
        assert!(
            hero_svg.contains(&format!("MERMAID / {id}")),
            "{hero_asset} is missing complete showcase frame {id}"
        );
        assert!(
            hero_svg.contains(&format!("{:02} / {:02}", index + 1, SHOWCASE_IDS.len())),
            "{hero_asset} is missing the progress label for {id}"
        );
    }
    assert_svg_viewports_are_complete(hero_asset, hero_svg);
    assert_showcase_frame_offsets(hero_asset, hero_svg, SHOWCASE_IDS.len());

    let info_svg = fs::read_to_string(root.join("docs/gallery/info.svg"))
        .expect("compact info gallery asset must be readable");
    assert!(
        svg_dimension(&info_svg, "width") <= 400,
        "compact diagrams must not retain the old fixed 100-column canvas"
    );
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
    assert!(
        !svg.contains("@keyframes"),
        "{asset} must be a complete static frame, not an animation"
    );
    assert_svg_viewports_are_complete(&asset, svg);
}

fn svg_dimension(svg: &str, name: &str) -> usize {
    let root = svg_opening_tags(svg)
        .into_iter()
        .next()
        .expect("SVG root must close its opening tag");
    svg_tag_dimension(root, name)
}

fn svg_tag_dimension(tag: &str, name: &str) -> usize {
    let marker = format!("{name}=\"");
    let value = tag
        .split_once(&marker)
        .and_then(|(_, rest)| rest.split_once('\"').map(|(value, _)| value))
        .unwrap_or_else(|| panic!("SVG root is missing {name:?}"));
    value
        .parse()
        .unwrap_or_else(|error| panic!("SVG {name:?} is not an integer: {error}"))
}

fn svg_opening_tags(svg: &str) -> Vec<&str> {
    svg.match_indices("<svg ")
        .map(|(start, _)| {
            let rest = &svg[start..];
            let end = rest
                .find('>')
                .unwrap_or_else(|| panic!("SVG opening tag at byte {start} is incomplete"));
            &rest[..=end]
        })
        .collect()
}

fn svg_view_box(tag: &str) -> [usize; 4] {
    let marker = "viewBox=\"";
    let value = tag
        .split_once(marker)
        .and_then(|(_, rest)| rest.split_once('\"').map(|(value, _)| value))
        .unwrap_or_else(|| panic!("SVG tag is missing a viewBox: {tag}"));
    let values = value
        .split_ascii_whitespace()
        .map(|part| {
            part.parse::<usize>()
                .unwrap_or_else(|error| panic!("SVG viewBox value {part:?} is invalid: {error}"))
        })
        .collect::<Vec<_>>();
    values
        .try_into()
        .unwrap_or_else(|_| panic!("SVG viewBox must have four values: {value}"))
}

fn assert_svg_viewports_are_complete(asset: &str, svg: &str) {
    let tags = svg_opening_tags(svg);
    assert_eq!(
        tags.len(),
        2,
        "{asset} must contain one root SVG and one terminal viewport"
    );
    assert_eq!(
        svg.matches("</svg>").count(),
        2,
        "{asset} has an unclosed SVG viewport"
    );

    let root_width = svg_tag_dimension(tags[0], "width");
    let root_height = svg_tag_dimension(tags[0], "height");
    assert_eq!(
        svg_view_box(tags[0]),
        [0, 0, root_width, root_height],
        "{asset} root dimensions and viewBox diverge"
    );

    let terminal_x = svg_tag_dimension(tags[1], "x");
    let terminal_y = svg_tag_dimension(tags[1], "y");
    let terminal_width = svg_tag_dimension(tags[1], "width");
    let terminal_height = svg_tag_dimension(tags[1], "height");
    assert_eq!(
        svg_view_box(tags[1]),
        [0, 0, terminal_width, terminal_height],
        "{asset} terminal dimensions and viewBox diverge"
    );
    assert!(
        terminal_x + terminal_width <= root_width,
        "{asset} terminal viewport overflows the root horizontally"
    );
    assert!(
        terminal_y + terminal_height <= root_height,
        "{asset} terminal viewport overflows the root vertically"
    );
    assert!(
        tags[1].contains("overflow=\"hidden\""),
        "{asset} terminal viewport must clip only outside its declared viewBox"
    );
    for rest in svg.split("transform=\"translate(").skip(1) {
        let coordinates = rest
            .split_once(")\"")
            .map(|(coordinates, _)| coordinates)
            .unwrap_or_else(|| panic!("{asset} has an incomplete translate transform"));
        let values = coordinates
            .split_ascii_whitespace()
            .map(|value| {
                value.parse::<usize>().unwrap_or_else(|error| {
                    panic!("{asset} has an invalid translate coordinate {value:?}: {error}")
                })
            })
            .collect::<Vec<_>>();
        let [_, y] = values.as_slice() else {
            panic!("{asset} translate must have two coordinates: {coordinates}");
        };
        assert!(
            y.saturating_add(ASG_CELL_HEIGHT) <= terminal_height,
            "{asset} row at y={y} is clipped by terminal height {terminal_height}"
        );
    }
}

fn assert_showcase_frame_offsets(asset: &str, svg: &str, frame_count: usize) {
    let terminal_width = svg_tag_dimension(svg_opening_tags(svg)[1], "width");
    let expected_offsets = (0..frame_count)
        .map(|index| index * terminal_width)
        .collect::<Vec<_>>();

    for offset in &expected_offsets {
        assert!(
            svg.contains(&format!("transform=\"translate({offset} 0)\"")),
            "{asset} is missing complete frame offset {offset}"
        );
    }

    let keyframe_offsets = svg
        .split("translateX(-")
        .skip(1)
        .map(|rest| {
            rest.split_once("px)")
                .map(|(value, _)| value)
                .unwrap_or_else(|| panic!("{asset} has an incomplete translateX keyframe"))
                .parse::<usize>()
                .unwrap_or_else(|error| panic!("{asset} has an invalid frame offset: {error}"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        keyframe_offsets.first(),
        Some(&0),
        "{asset} animation must begin on the first complete frame"
    );
    assert_eq!(
        keyframe_offsets.last(),
        expected_offsets.last(),
        "{asset} animation must finish on the last complete frame"
    );
    assert!(
        keyframe_offsets
            .iter()
            .all(|offset| expected_offsets.contains(offset)),
        "{asset} contains a partial-frame animation offset: {keyframe_offsets:?}"
    );
    for window in keyframe_offsets.windows(2) {
        assert!(
            window[0] <= window[1],
            "{asset} frame offsets must be monotonic: {keyframe_offsets:?}"
        );
    }
}
