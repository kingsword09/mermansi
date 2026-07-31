//! Source classification and parser integration.

use crate::error::{MermansiError, Result};
use merman_core::diagram::RenderSemanticModel;

pub(crate) fn parse_source_model(source: &str) -> Result<Option<RenderSemanticModel>> {
    let trimmed = source.trim_start();
    if matches!(trimmed.as_bytes().first(), Some(b'{') | Some(b'[')) {
        let value =
            serde_json::from_str(source).map_err(|source| MermansiError::JsonSource { source })?;
        return Ok(Some(RenderSemanticModel::Json(value)));
    }

    let engine = merman_core::Engine::new();
    let parsed = if let Some(normalized) = normalize_flowchart_v2_header(source) {
        engine.parse_diagram_for_render_model_with_type_sync(
            "flowchart-v2",
            &normalized,
            merman_core::ParseOptions::strict(),
        )?
    } else {
        engine.parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())?
    };
    Ok(parsed.map(|diagram| diagram.model))
}

/// `flowchart-v2` is an upstream parser id, while the grammar accepts the public
/// `flowchart` header. Locate only the first semantic token after upstream-style
/// preambles, then preserve byte offsets while parsing through merman-core's
/// complete known-type pipeline.
fn normalize_flowchart_v2_header(source: &str) -> Option<String> {
    const ALIAS: &str = "flowchart-v2";
    const HEADER: &str = "flowchart   ";
    debug_assert_eq!(ALIAS.len(), HEADER.len());

    let header_start = diagram_header_offset(source)?;
    let suffix = source[header_start..].strip_prefix(ALIAS)?;
    if suffix
        .chars()
        .next()
        .is_some_and(|character| !character.is_whitespace())
    {
        return None;
    }

    let mut normalized = source.to_string();
    normalized.replace_range(header_start..header_start + ALIAS.len(), HEADER);
    Some(normalized)
}

fn diagram_header_offset(source: &str) -> Option<usize> {
    let mut cursor =
        merman_core::preprocess::split_frontmatter_block(source).map_or(0, |block| block.full.end);

    loop {
        let remaining = source.get(cursor..)?;
        let trimmed = remaining.trim_start();
        cursor += remaining.len() - trimmed.len();
        if trimmed.is_empty() {
            return Some(cursor);
        }

        if let Some(after_start) = trimmed.strip_prefix("%%{") {
            let directive_end = after_start.find("}%%")?;
            cursor += "%%{".len() + directive_end + "}%%".len();
            continue;
        }

        if let Some(after_marker) = trimmed.strip_prefix("%%") {
            let is_comment = !after_marker.starts_with('{')
                && after_marker
                    .chars()
                    .next()
                    .is_some_and(|character| character != '\n');
            if is_comment {
                cursor += trimmed
                    .find('\n')
                    .map_or(trimmed.len(), |offset| offset + 1);
                continue;
            }
        }

        return Some(cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_flowchart_v2_header, parse_source_model};
    use crate::error::MermansiError;
    use merman_core::diagram::RenderSemanticModel;

    #[test]
    fn normalizes_only_the_exact_leading_alias() {
        let source = "  flowchart-v2 TD\nA --> B";
        let normalized = normalize_flowchart_v2_header(source).expect("alias should normalize");
        assert_eq!(
            normalized, "  flowchart    TD\nA --> B",
            "the grammar header should replace only the parser-id token"
        );
        assert_eq!(
            normalized.len(),
            source.len(),
            "source offsets must not shift"
        );
        assert!(normalize_flowchart_v2_header("flowchart-v20 TD").is_none());
        assert!(normalize_flowchart_v2_header("flowchart TD").is_none());
    }

    #[test]
    fn parses_raw_json_objects_and_arrays() {
        for source in ["{\"name\":\"Alice\"}", "[\"Rust\",\"C\"]"] {
            assert!(matches!(
                parse_source_model(source),
                Ok(Some(RenderSemanticModel::Json(_)))
            ));
        }
    }

    #[test]
    fn reports_invalid_raw_json_as_a_source_error() {
        assert!(matches!(
            parse_source_model("{\"name\":}"),
            Err(MermansiError::JsonSource { .. })
        ));
    }

    #[test]
    fn parses_flowchart_v2_after_upstream_preambles() {
        for source in [
            "%% comment\nflowchart-v2 TD\nA --> B",
            "%%{init: {\"theme\":\"default\"}}%%\nflowchart-v2 TD\nA --> B",
            "%%{init: {\"flowchart\":{\"defaultRenderer\":\"dagre-d3\"}}}%%\nflowchart-v2 TD\nA --> B",
            "---\ntitle: Alias graph\n---\nflowchart-v2 TD\nA --> B",
        ] {
            assert!(matches!(
                parse_source_model(source),
                Ok(Some(RenderSemanticModel::Flowchart(_)))
            ));
        }
    }
}
