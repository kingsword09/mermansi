//! Optional ANSI color roles for `mermansi`.
//!
//! ANSI roles add color without changing cell geometry. When `ColorMode::Plain` is selected,
//! no escape sequences are emitted. The role system maps semantic elements to SGR codes;
//! [`AnsiEncoder`] handles serialization.

use crate::options::ColorMode;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiRole {
    NodeBorder,
    NodeText,
    EdgeStroke,
    EdgeArrow,
    EdgeLabel,
    ChartSeries(u8),
    SectionHeader,
    Title,
}

#[derive(Debug, Clone)]
pub struct AnsiEncoder {
    mode: ColorMode,
}

impl AnsiEncoder {
    pub const fn new(mode: ColorMode) -> Self {
        Self { mode }
    }

    pub fn mode(&self) -> ColorMode {
        self.mode
    }

    pub fn prefix(&self, role: AnsiRole) -> &'static str {
        match (self.mode, role) {
            (ColorMode::Plain, _) => "",
            (ColorMode::Ansi16, AnsiRole::NodeBorder | AnsiRole::SectionHeader) => "\x1b[36m",
            (ColorMode::Ansi16, AnsiRole::NodeText) => "\x1b[37m",
            (ColorMode::Ansi16, AnsiRole::EdgeStroke) => "\x1b[34m",
            (ColorMode::Ansi16, AnsiRole::EdgeArrow) => "\x1b[33m",
            (ColorMode::Ansi16, AnsiRole::EdgeLabel) => "\x1b[35m",
            (ColorMode::Ansi16, AnsiRole::Title) => "\x1b[1;37m",
            (ColorMode::Ansi16, AnsiRole::ChartSeries(index)) => match index % 6 {
                0 => "\x1b[31m",
                1 => "\x1b[32m",
                2 => "\x1b[33m",
                3 => "\x1b[34m",
                4 => "\x1b[35m",
                _ => "\x1b[36m",
            },
            (ColorMode::TrueColor, AnsiRole::NodeBorder | AnsiRole::SectionHeader) => {
                "\x1b[38;2;56;189;173m"
            }
            (ColorMode::TrueColor, AnsiRole::NodeText) => "\x1b[38;2;230;237;243m",
            (ColorMode::TrueColor, AnsiRole::EdgeStroke) => "\x1b[38;2;88;166;255m",
            (ColorMode::TrueColor, AnsiRole::EdgeArrow) => "\x1b[38;2;255;196;87m",
            (ColorMode::TrueColor, AnsiRole::EdgeLabel) => "\x1b[38;2;213;128;255m",
            (ColorMode::TrueColor, AnsiRole::Title) => "\x1b[1;38;2;255;255;255m",
            (ColorMode::TrueColor, AnsiRole::ChartSeries(index)) => match index % 6 {
                0 => "\x1b[38;2;239;83;80m",
                1 => "\x1b[38;2;102;187;106m",
                2 => "\x1b[38;2;255;202;40m",
                3 => "\x1b[38;2;66;165;245m",
                4 => "\x1b[38;2;171;71;188m",
                _ => "\x1b[38;2;38;198;218m",
            },
        }
    }

    pub fn suffix(&self) -> &'static str {
        match self.mode {
            ColorMode::Plain => "",
            ColorMode::Ansi16 | ColorMode::TrueColor => "\x1b[0m",
        }
    }

    pub fn paint(&self, role: AnsiRole, text: &str) -> String {
        format!("{}{text}{}", self.prefix(role), self.suffix())
    }
}

/// Remove ANSI escape sequences while preserving all printable Unicode text.
///
/// Handles CSI, OSC, DCS, APC, PM, SOS, ST, their 8-bit C1 forms, and bare ESC
/// bytes. Newlines, carriage returns, and tabs are preserved so stripping color
/// never changes the layout of an already-rendered terminal document.
pub fn strip_ansi(input: &str) -> String {
    strip_terminal_sequences(input, true, false)
}

fn strip_terminal_controls(input: &str) -> String {
    strip_terminal_sequences(input, false, true)
}

fn strip_terminal_sequences(
    input: &str,
    preserve_layout_controls: bool,
    remove_bidi_controls: bool,
) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\u{1b}' => consume_escape_sequence(&mut chars),
            '\u{009b}' => consume_csi(&mut chars),
            '\u{0090}' | '\u{0098}' | '\u{009d}' | '\u{009e}' | '\u{009f}' => {
                consume_string_control(&mut chars)
            }
            '\u{009c}' => {}
            '\n' | '\r' | '\t' if preserve_layout_controls => output.push(ch),
            _ if ch.is_control() => {}
            _ if remove_bidi_controls && is_bidi_format_control(ch) => {}
            _ => output.push(ch),
        }
    }
    output
}

fn consume_escape_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek() {
        Some('[') => {
            chars.next();
            consume_csi(chars);
        }
        Some(']' | 'P' | '_' | '^' | 'X') => {
            chars.next();
            consume_string_control(chars);
        }
        Some('\\') => {
            chars.next();
        }
        Some(&intermediate) if ('\u{20}'..='\u{2f}').contains(&intermediate) => {
            while chars
                .peek()
                .is_some_and(|next| ('\u{20}'..='\u{2f}').contains(next))
            {
                chars.next();
            }
            if chars
                .peek()
                .is_some_and(|next| ('\u{30}'..='\u{7e}').contains(next))
            {
                chars.next();
            }
        }
        Some(_) | None => {}
    }
}

fn consume_csi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    for control in chars.by_ref() {
        if ('@'..='~').contains(&control) {
            break;
        }
    }
}

fn consume_string_control(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    loop {
        match chars.next() {
            Some('\u{07}' | '\u{009c}') | None => break,
            Some('\u{1b}') if chars.peek() == Some(&'\\') => {
                chars.next();
                break;
            }
            Some(_) => {}
        }
    }
}

/// Sanitize label text for terminal rendering by removing all terminal-control
/// sequences before the text is used for width calculation or formatting.
///
/// This handles the full set of C1 control sequences that can appear in user-supplied
/// label text:
///
/// * **CSI** (`ESC [`): Select Graphic Rendition and other control sequences, terminated
///   by a final byte in `0x40..=0x7e`.
/// * **String-control families** — OSC (`ESC ]`), DCS (`ESC P`), APC (`ESC _`), PM
///   (`ESC ^`), SOS (`ESC X`): terminated by either BEL (`0x07`) or ST (`ESC \`).
///   Embedded non-terminating ESC bytes (e.g. a CSI inside an OSC payload) are consumed
///   as part of the string content; only BEL or ST terminates the family.
/// * **C0 controls** (`0x00..=0x1f`), DEL (`0x7f`), and C1 controls (`0x80..=0x9f`):
///   these include BEL (`0x07`), backspace (`0x08`), TAB (`0x09`), LF (`0x0a`),
///   CR (`0x0d`), and other control characters that must not appear in rendered label
///   text. TAB, LF, and CR are layout-changing and can break column alignment.
///
/// All visible Unicode text (including CJK, combining marks, and emoji) is preserved.
/// Called by terminal adapters before label text is measured or placed, so raw control
/// sequences cannot affect table alignment or Canvas geometry regardless of the active
/// `ColorMode`.
pub(crate) struct TerminalTextNormalizer;

impl TerminalTextNormalizer {
    pub(crate) fn normalize(input: &str) -> String {
        let controls_removed = strip_terminal_controls(input);
        let markup_removed = normalize_mermaid_markup(&controls_removed);
        let bold_removed = strip_paired_delimiter(&markup_removed, "**");
        let emphasis_removed = strip_paired_delimiter(&bold_removed, "__");
        let parser_artifacts_removed = strip_unbalanced_bold_artifact(&emphasis_removed);
        stabilize_leading_graphemes(&parser_artifacts_removed)
    }
}

pub(crate) fn sanitize_label_text(input: &str) -> String {
    TerminalTextNormalizer::normalize(input)
}

pub(crate) fn is_bidi_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

fn normalize_mermaid_markup(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0usize;
    while index < input.len() {
        let rest = &input[index..];
        if let Some((decoded, consumed)) = decode_entity(rest) {
            output.push_str(decoded);
            index += consumed;
            continue;
        }
        if rest.starts_with('<')
            && let Some(end) = rest.find('>')
        {
            let body = rest[1..end].trim();
            let name = body
                .trim_start_matches('/')
                .trim_end_matches('/')
                .split_ascii_whitespace()
                .next()
                .unwrap_or_default();
            if is_terminal_markup_tag(name) {
                if matches!(name.to_ascii_lowercase().as_str(), "br" | "div" | "p") {
                    push_text_separator(&mut output);
                }
                index += end + 1;
                continue;
            }
        }
        let ch = rest
            .chars()
            .next()
            .expect("index remains on a character boundary");
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

fn is_terminal_markup_tag(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "br" | "b" | "strong" | "i" | "em" | "u" | "span" | "font" | "div" | "p"
    )
}

fn decode_entity(rest: &str) -> Option<(&'static str, usize)> {
    for (encoded, decoded) in [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        if rest.starts_with(encoded) {
            return Some((decoded, encoded.len()));
        }
    }
    None
}

fn push_text_separator(output: &mut String) {
    if !output.is_empty() && !output.ends_with(char::is_whitespace) {
        output.push(' ');
    }
}

fn strip_paired_delimiter(input: &str, delimiter: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find(delimiter) {
        output.push_str(&rest[..start]);
        let content = &rest[start + delimiter.len()..];
        let Some(end) = content.find(delimiter) else {
            output.push_str(delimiter);
            output.push_str(content);
            return output;
        };
        output.push_str(&content[..end]);
        rest = &content[end + delimiter.len()..];
    }
    output.push_str(rest);
    output
}

fn strip_unbalanced_bold_artifact(input: &str) -> String {
    let Some(open) = input.find("**") else {
        return input.to_owned();
    };
    let content_start = open + 2;
    if input[content_start..].contains("**") {
        return input.to_owned();
    }
    let Some(close_offset) = input[content_start..].rfind('*') else {
        return input.to_owned();
    };
    let close = content_start + close_offset;
    let mut output = String::with_capacity(input.len().saturating_sub(3));
    output.push_str(&input[..open]);
    output.push_str(&input[content_start..close]);
    output.push_str(&input[close + 1..]);
    output
}

fn stabilize_leading_graphemes(input: &str) -> String {
    let mut output = String::with_capacity(input.len().saturating_add(3));
    let mut has_base = false;
    for grapheme in UnicodeSegmentation::graphemes(input, true) {
        let width = UnicodeWidthStr::width(grapheme);
        if width == 0 && !has_base {
            let visible_marks = grapheme
                .chars()
                .filter(|ch| !is_leading_default_ignorable(*ch))
                .collect::<String>();
            if visible_marks.is_empty() {
                continue;
            }
            output.push('◌');
            output.push_str(&visible_marks);
            has_base = true;
            continue;
        }
        output.push_str(grapheme);
        has_base |= width > 0;
    }
    output
}

fn is_leading_default_ignorable(ch: char) -> bool {
    matches!(
        ch,
        '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}' | '\u{e0100}'..='\u{e01ef}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_preserves_plain_text() {
        assert_eq!(sanitize_label_text("Hello World"), "Hello World");
    }

    #[test]
    fn sanitize_strips_csi_sequence() {
        assert_eq!(sanitize_label_text("\u{1b}[31mRed\u{1b}[0m"), "Red");
    }

    #[test]
    fn strip_ansi_removes_string_controls_without_changing_layout() {
        let input = "A\t\u{1b}[31mred\u{1b}[0m\nB\u{1b}]0;title\u{07}C\r\nD\u{009d}hidden\u{009c}E";
        assert_eq!(strip_ansi(input), "A\tred\nBC\r\nDE");
    }

    #[test]
    fn sanitize_strips_osc_bell_terminated() {
        assert_eq!(sanitize_label_text("\u{1b}]0;Title\u{07}"), "");
        assert_eq!(sanitize_label_text("A\u{1b}]0;Title\u{07}B"), "AB");
    }

    #[test]
    fn sanitize_strips_osc_st_terminated() {
        assert_eq!(sanitize_label_text("\u{1b}]0;Title\u{1b}\\"), "");
    }

    #[test]
    fn sanitize_strips_bell_control_byte() {
        assert_eq!(sanitize_label_text("Alert\u{07}"), "Alert");
    }

    #[test]
    fn sanitize_preserves_cjk_emoji_combining() {
        assert_eq!(
            sanitize_label_text("开始 \u{1F680} cafe\u{301}"),
            "开始 \u{1F680} cafe\u{301}"
        );
    }

    #[test]
    fn normalize_terminal_markup_and_markdown_emphasis() {
        assert_eq!(
            sanitize_label_text("<b>Bold</b><br/>Next **strong** &amp; __clear__"),
            "Bold Next strong & clear"
        );
        assert_eq!(sanitize_label_text("a < b"), "a < b");
        assert_eq!(sanitize_label_text("+String **bold*"), "+String bold");
    }

    #[test]
    fn sanitize_removes_bidi_format_controls() {
        assert_eq!(
            sanitize_label_text("safe\u{202e}evil\u{202c}\u{2066}text\u{2069}"),
            "safeeviltext"
        );
    }

    #[test]
    fn sanitize_stabilizes_leading_zero_width_graphemes() {
        assert_eq!(sanitize_label_text("\u{301}Accent"), "◌\u{301}Accent");
        assert_eq!(sanitize_label_text("\u{fe0f}Icon"), "Icon");
        assert_eq!(sanitize_label_text("\u{200d}Join"), "Join");
    }

    #[test]
    fn sanitize_strips_tab_newline_cr() {
        assert_eq!(sanitize_label_text("a\tb\nc\rd"), "abcd");
    }

    #[test]
    fn sanitize_strips_unrecognized_esc_sequences() {
        assert_eq!(sanitize_label_text("\u{1b}Z"), "Z");
    }

    #[test]
    fn sanitize_consumes_embedded_esc_in_osc() {
        // OSC with an embedded non-terminating ESC (not followed by backslash)
        // must continue consuming until the BEL terminator.
        assert_eq!(
            sanitize_label_text("\u{1b}]0;T\u{1b}[31m More\u{07}Visible"),
            "Visible"
        );
    }

    #[test]
    fn sanitize_strips_all_string_control_families() {
        // DCS (ESC P) terminated by BEL
        assert_eq!(sanitize_label_text("A\u{1b}Pdata\u{07}B"), "AB");
        // APC (ESC _) terminated by ST (ESC \)
        assert_eq!(sanitize_label_text("A\u{1b}_data\u{1b}\\B"), "AB");
        // PM (ESC ^) terminated by BEL
        assert_eq!(sanitize_label_text("A\u{1b}^data\u{07}B"), "AB");
        // SOS (ESC X) terminated by ST (ESC \)
        assert_eq!(sanitize_label_text("A\u{1b}Xdata\u{1b}\\B"), "AB");
        // Eight-bit C1 OSC terminated by eight-bit ST.
        assert_eq!(sanitize_label_text("A\u{009d}data\u{009c}B"), "AB");
    }

    #[test]
    fn sanitize_consumes_embedded_esc_in_dcs() {
        // DCS with an embedded non-terminating ESC must keep consuming until
        // the BEL terminator — the ESC [31m is NOT treated as a terminator
        // because it is not ESC backslash.
        assert_eq!(
            sanitize_label_text("\u{1b}Pdata\u{1b}[31m more\u{07}Visible"),
            "Visible"
        );
    }
}
