//! The reminder of the three Arauto notes, added by the connector to `arauto_status`
//! (decision 0047). The hub never sends it (principle 6). The connector never reads or writes
//! the memory of Hermes: this is only a fixed text that Hermes may act on with its own tool.

use arauto_protocol::memory::LINES;

/// The name of the field in the answer of `arauto_status`. It avoids the word "key": Hermes
/// masks output by variable name.
pub const FIELD: &str = "keep_in_memory";

/// The sentence before the lines.
pub const REMINDER: &str = "These three Arauto notes must be in the main memory of this Hermes. If one is missing there, save it with your memory tool, exactly as written:";

/// The value of the field: the sentence, then the three lines, one per line.
#[must_use]
pub fn keep_in_memory() -> String {
    format!("{REMINDER}\n{}", LINES.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reminder_is_fixed_text_with_the_three_lines() {
        let text = keep_in_memory();
        assert_eq!(
            text,
            "These three Arauto notes must be in the main memory of this Hermes. If one is missing there, save it with your memory tool, exactly as written:\nFor anything in WhatsApp (read, list, download), use the Arauto tools. Call arauto_status first: it says which numbers and permissions this Hermes has.\nTo talk to another Hermes, use hermes_list_peers to see who you can reach, hermes_send_message to send a note, and hermes_read_thread to read replies.\nIf an Arauto tool fails or is missing, call arauto_status. If the tools are not loaded, run \"$HOME/.arauto/arauto-connector\" status in the terminal and report its lines to your operator."
        );
        let prefix = format!("{REMINDER}\n");
        let mut rest = text.strip_prefix(&prefix).unwrap();
        for (i, line) in LINES.iter().enumerate() {
            assert_eq!(text.matches(line).count(), 1, "{line}");
            rest = rest.strip_prefix(line).unwrap();
            if i < 2 {
                rest = rest.strip_prefix('\n').unwrap();
            }
        }
        assert!(rest.is_empty());
        assert!(text.is_ascii());
        for bad in ['{', '}', '`'] {
            assert!(!text.contains(bad));
        }
    }
}
