//! The three lines a Hermes keeps in its own main memory about Arauto (decision 0047).
//!
//! Fixed text. The page of the hub carries a copy (`page.md`), which a hub test compares with
//! these lines; the connector puts them in the answer of `arauto_status`. Neither program sends
//! them anywhere else, and neither one reads or writes the memory of Hermes.

/// The three lines, as approved. One per line, without a number.
pub const LINES: [&str; 3] = [
    "For anything in WhatsApp (read, list, download), use the Arauto tools. Call arauto_status first: it says which numbers and permissions this Hermes has.",
    "To talk to another Hermes, use hermes_list_peers to see who you can reach, hermes_send_message to send a note, and hermes_read_thread to read replies.",
    "If an Arauto tool fails or is missing, call arauto_status. If the tools are not loaded, run \"$HOME/.arauto/arauto-connector\" status in the terminal and report its lines to your operator.",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_the_approved_text() {
        assert_eq!(
            LINES,
            [
                r#"For anything in WhatsApp (read, list, download), use the Arauto tools. Call arauto_status first: it says which numbers and permissions this Hermes has."#,
                r#"To talk to another Hermes, use hermes_list_peers to see who you can reach, hermes_send_message to send a note, and hermes_read_thread to read replies."#,
                r#"If an Arauto tool fails or is missing, call arauto_status. If the tools are not loaded, run "$HOME/.arauto/arauto-connector" status in the terminal and report its lines to your operator."#,
            ]
        );
    }

    #[test]
    fn lines_are_plain_one_line_ascii() {
        for line in LINES {
            assert!(line.is_ascii(), "{line}");
            assert!(!line.contains('\n'), "{line}");
            for bad in ['{', '}', '`'] {
                assert!(!line.contains(bad), "{line}");
            }
            assert!(line.ends_with('.'), "{line}");
            assert!(line.len() <= 200, "{line}");
        }
    }

    #[test]
    fn the_tools_the_lines_name_are_in_the_catalog() {
        let mut named = std::collections::BTreeSet::new();
        for line in LINES {
            for token in line.split(|c: char| !(c.is_ascii_lowercase() || c == '_')) {
                if token.contains('_') {
                    named.insert(token);
                }
            }
        }
        let expected: std::collections::BTreeSet<&str> = [
            "arauto_status",
            "hermes_list_peers",
            "hermes_send_message",
            "hermes_read_thread",
        ]
        .into_iter()
        .collect();
        assert_eq!(named, expected);
        for tool in named {
            assert!(crate::tools::find(tool).is_some(), "{tool}");
        }
    }
}
