//! Optional ANSI color roles for `mermansi`.
//!
//! ANSI roles add color without changing cell geometry. When `ColorMode::Plain` is selected,
//! no escape sequences are emitted. The role system maps semantic elements to SGR codes;
//! [`AnsiEncoder`] handles serialization.

use crate::options::ColorMode;

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
/// Handles CSI sequences (`ESC [`) and drops any bare `ESC` (0x1b) bytes that are
/// not part of a recognised sequence, ensuring no terminal-control bytes survive
/// stripping.
pub fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                // CSI sequence: ESC [ ... final byte in 0x40..=0x7e
                chars.next(); // consume '['
                for control in chars.by_ref() {
                    if ('@'..='~').contains(&control) {
                        break;
                    }
                }
            } else {
                // Bare ESC byte (or ESC not followed by '['): drop it so no
                // terminal-control bytes leak through.
            }
        } else {
            output.push(ch);
        }
    }
    output
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
pub(crate) fn sanitize_label_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // We're at an ESC (0x1b) byte — determine which C1 family this is.
            match chars.peek() {
                // CSI: ESC [ ... final byte in 0x40..=0x7e
                Some('[') => {
                    chars.next(); // consume '['
                    for control in chars.by_ref() {
                        if ('@'..='~').contains(&control) {
                            break;
                        }
                    }
                }
                // String-control families terminated by BEL or ST (ESC \):
                //   OSC: ESC ]    DCS: ESC P    APC: ESC _
                //   PM:  ESC ^    SOS: ESC X
                Some(']' | 'P' | '_' | '^' | 'X') => {
                    chars.next(); // consume the introducer byte
                    // Consume until we hit BEL (0x07) or ST (ESC \).
                    // Embedded non-terminating ESC bytes must continue consuming.
                    loop {
                        match chars.next() {
                            Some('\u{07}') => break, // BEL terminator
                            Some('\u{1b}') => {
                                // Check for ST terminator (ESC \). If the ESC is
                                // not followed by backslash, it is an embedded
                                // non-terminating ESC: keep consuming.
                                if chars.peek() == Some(&'\\') {
                                    chars.next(); // consume '\'
                                    break;
                                }
                                // Non-terminating embedded ESC — continue loop.
                            }
                            Some(_) => continue,
                            None => break, // unterminated string control — drop rest
                        }
                    }
                }
                // ESC \ (ST) with no preceding introducer — drop both bytes
                Some('\\') => {
                    chars.next(); // consume '\'
                }
                // Any other ESC-prefixed sequence (nFe escapes like ESC followed by
                // 0x20..=0x2f then a final byte, or bare ESC): drop the ESC. We
                // conservatively consume one more byte if it's in the intermediate
                // range, then keep scanning.
                Some(&c) if ('\u{20}'..='\u{2f}').contains(&c) => {
                    chars.next(); // consume intermediate byte
                    for control in chars.by_ref() {
                        if ('\u{30}'..='\u{7e}').contains(&control) {
                            break;
                        }
                    }
                }
                // Bare ESC byte with no recognised follower: drop it.
                Some(_) | None => {}
            }
        } else if ch.is_control() {
            // All control characters (C0 0x00..=0x1f, DEL 0x7f, C1 0x80..=0x9f)
            // are removed — including TAB, LF, and CR, which are layout-changing
            // and can break Pie column alignment and the table format.
        } else {
            output.push(ch);
        }
    }
    output
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
