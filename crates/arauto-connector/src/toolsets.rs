//! The tools of a woken run (proposal, 18/11/2026; `docs/11-conector.md`, section 6; decision
//! 0046).
//!
//! A notice wakes Hermes through its local API. On the one Hermes observed (v0.23.0 and v0.22.5,
//! 18/11/2026, `referencia/hermes-fatos.md`, N25), the tools of the run that notice starts are the
//! list `platform_toolsets.api_server` of its configuration, when that list is explicit. A list
//! without `arauto` means the tools of that run do not include the Arauto ones. This module reads
//! that list through the command of Hermes, and, only in `join`, appends `arauto` to an explicit
//! non-empty list that lacks it.
//!
//! - The read keeps the output of `config get` (up to 64 KiB; an output that reached that ceiling
//!   may have been cut and is unreadable), parses it with a strict parser and drops it: it is
//!   never printed, logged, written to a file or put in an error. The only list form is the one
//!   observed: one `  - NAME` line per item at the end of the output, or `[]` for an empty list.
//!   A line printed before the value is ignored when it does not start with `-`. Any other
//!   output, a one-line `[a, b]` included (it may be a text value), is "unreadable", and then
//!   nothing is written.
//! - Nothing is rolled back, and Hermes is never restarted.
//! - Not tried: whether a woken run then calls the Arauto tools, whether a running gateway needs a
//!   restart to use the new list, and what `[]` means for a woken run (an empty list is left alone
//!   because its meaning is unknown).

use crate::catalog::Told;
use crate::hermes::{Budget, Hermes, SERVER_NAME};
use crate::settings::{Settings, duration};

/// What is known of the tools of the runs a notice wakes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTools {
    /// An explicit list that includes `arauto`.
    Included,
    /// No row: Hermes gives that platform its default tools.
    Default,
    /// `join` appended `arauto` and confirmed it.
    Added,
    /// An explicit non-empty list without `arauto`, or the append was refused.
    Missing,
    /// An explicit empty list: cannot tell.
    Empty,
    /// The read failed, the output is out of form, or a write was not confirmed: cannot tell.
    Unreadable,
    /// No record. Never written to the record.
    NotChecked,
}

impl RunTools {
    /// The word of the record and of `woken_run_tools`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            RunTools::Included => "included",
            RunTools::Default => "default",
            RunTools::Added => "added",
            RunTools::Missing => "missing",
            RunTools::Empty => "empty",
            RunTools::Unreadable => "unreadable",
            RunTools::NotChecked => "not_checked",
        }
    }

    /// The six recorded words (not `not_checked`).
    #[must_use]
    pub fn parse(word: &str) -> Option<RunTools> {
        match word {
            "included" => Some(RunTools::Included),
            "default" => Some(RunTools::Default),
            "added" => Some(RunTools::Added),
            "missing" => Some(RunTools::Missing),
            "empty" => Some(RunTools::Empty),
            "unreadable" => Some(RunTools::Unreadable),
            _ => None,
        }
    }

    /// The fixed sentence of the state.
    #[must_use]
    pub const fn text(self) -> &'static str {
        self.told().phrase()
    }

    pub(crate) const fn told(self) -> Told {
        match self {
            RunTools::Included => Told::RunToolsIncluded,
            RunTools::Default => Told::RunToolsDefault,
            RunTools::Added => Told::RunToolsAddedByJoin,
            RunTools::Missing => Told::RunToolsMissing,
            RunTools::Empty => Told::RunToolsEmpty,
            RunTools::Unreadable => Told::RunToolsUnreadable,
            RunTools::NotChecked => Told::RunToolsNotChecked,
        }
    }
}

/// The most items of a list the parser accepts.
const LIST_ITEMS_MAX: usize = 64;

/// The longest name of a toolset.
const NAME_MAX: usize = 64;

fn name_in_form(name: &str) -> bool {
    let bytes = name.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    bytes.len() <= NAME_MAX
        && (first.is_ascii_alphanumeric() || *first == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

/// The lines of the output as text, without the blank ones at the end: `None` for output that is
/// not UTF-8.
fn lines_of(printed: &[u8]) -> Option<Vec<&str>> {
    let text = std::str::from_utf8(printed).ok()?;
    let mut lines: Vec<&str> = text.lines().collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    Some(lines)
}

/// The name of a block line, exactly `  - NAME` (two spaces, a dash, a space).
fn block_item(line: &str) -> Option<&str> {
    line.strip_prefix("  - ").filter(|name| name_in_form(name))
}

/// A line printed before the value that cannot be part of it: blank, or not indented, not starting
/// with `-`, `[` or `#` and not ending with `:`. An indented line, a list item, a comment or a key
/// may belong to the value (a list whose first item is a mapping, a mapping that holds a list), and
/// reading only the end of it would drop entries on the write.
fn plain_line(line: &str) -> bool {
    let line = line.trim_end();
    line.is_empty()
        || !(line.starts_with(char::is_whitespace)
            || line.starts_with(['-', '[', '#'])
            || line.ends_with(':'))
}

/// The list in what `config get` printed (observed, `referencia/hermes-fatos.md`, N25): the
/// maximal run of lines at the end of the output, each exactly `  - NAME`. The lines before the run
/// are lines printed before the value and are ignored only when every one of them is a plain line
/// ([`plain_line`]); otherwise the run may be the end of a longer value and the output is not read.
/// A last line `[]` is the empty list. Anything else, a one-line flow form such as `[a, b]`
/// included, is `None`: it may be a text value.
pub(crate) fn parse_list(printed: &[u8]) -> Option<Vec<String>> {
    let lines = lines_of(printed)?;
    if lines.last()?.trim() == "[]" {
        return Some(Vec::new());
    }
    let start = lines
        .iter()
        .rposition(|line| block_item(line).is_none())
        .map_or(0, |at| at + 1);
    let items = &lines[start..];
    if items.is_empty() || items.len() > LIST_ITEMS_MAX {
        return None;
    }
    if !lines[..start].iter().all(|line| plain_line(line)) {
        return None;
    }
    Some(
        items
            .iter()
            .filter_map(|line| block_item(line))
            .map(str::to_owned)
            .collect(),
    )
}

/// The compact JSON array of `names`: each in double quotes, no spaces. The names were validated,
/// so nothing needs escaping.
pub(crate) fn list_value(names: &[String]) -> String {
    let items: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
    format!("[{}]", items.join(","))
}

/// How the tools step of `join` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolsOutcome {
    /// No row: nothing to change.
    NoRow,
    /// The list includes `arauto`.
    Included,
    /// An empty list: left alone.
    Empty,
    /// The read failed or was out of form, or there was no time left to write.
    Unreadable,
    /// `arauto` was appended and confirmed.
    Added,
    /// The `set` exited with a code that is not 0.
    SetRefused,
    /// The `set` was cut or its result was not confirmed.
    NotConfirmed,
}

impl ToolsOutcome {
    /// What is recorded.
    pub(crate) const fn record(self) -> RunTools {
        match self {
            ToolsOutcome::NoRow => RunTools::Default,
            ToolsOutcome::Included => RunTools::Included,
            ToolsOutcome::Empty => RunTools::Empty,
            ToolsOutcome::Unreadable | ToolsOutcome::NotConfirmed => RunTools::Unreadable,
            ToolsOutcome::Added => RunTools::Added,
            ToolsOutcome::SetRefused => RunTools::Missing,
        }
    }

    /// What `join` prints, in order.
    pub(crate) const fn told(self) -> &'static [Told] {
        match self {
            ToolsOutcome::NoRow | ToolsOutcome::Included => &[],
            ToolsOutcome::Empty => &[Told::RunToolsEmpty],
            ToolsOutcome::Unreadable => &[Told::RunToolsUnreadable],
            ToolsOutcome::Added => &[Told::RunToolsAdded, Told::RunToolsRestart],
            ToolsOutcome::SetRefused => &[Told::RunToolsNotAdded, Told::RunToolsMissing],
            ToolsOutcome::NotConfirmed => &[Told::RunToolsNotConfirmed],
        }
    }
}

// BEGIN tools step
/// The tools step of `join` (T1): `None` when it was not run (a test that answers Hermes itself).
pub(crate) async fn add_step(hermes: &Hermes, settings: &Settings) -> Option<ToolsOutcome> {
    #[cfg(feature = "testing")]
    if settings.testing.hermes.is_some() {
        return None;
    }
    let budget = Budget::new(settings.tools_step_ms);
    let old = match hermes.read_tool_list(budget.call_limit(settings)).await {
        Err(_) => return Some(ToolsOutcome::Unreadable),
        Ok(None) => return Some(ToolsOutcome::NoRow),
        Ok(Some(printed)) => match parse_list(&printed) {
            None => return Some(ToolsOutcome::Unreadable),
            Some(old) => old,
        },
    };
    if old.is_empty() {
        return Some(ToolsOutcome::Empty);
    }
    if old.iter().any(|name| name == SERVER_NAME) {
        return Some(ToolsOutcome::Included);
    }
    let mut expected = old;
    expected.push(SERVER_NAME.to_owned());
    let value = list_value(&expected);
    if budget.left().is_zero() {
        // Nothing was written.
        return Some(ToolsOutcome::Unreadable);
    }
    match hermes
        .write_tool_list(&value, budget.call_limit(settings))
        .await
    {
        Ok(false) => return Some(ToolsOutcome::SetRefused),
        // A cut `set` may have written.
        Err(_) => return Some(ToolsOutcome::NotConfirmed),
        Ok(true) => {}
    }
    // The read back: in the form Hermes prints for a stored list, and exactly the old list and
    // `arauto`. A value stored as text prints as one plain line, never as `  - ` items.
    match hermes.read_tool_list(budget.call_limit(settings)).await {
        Ok(Some(printed)) if parse_list(&printed).as_ref() == Some(&expected) => {
            Some(ToolsOutcome::Added)
        }
        _ => Some(ToolsOutcome::NotConfirmed),
    }
}
// END tools step

/// The list now, for `status` (T2): one read, never a write. `None` when it was not run (a test
/// that answers Hermes itself).
pub(crate) async fn read_now(hermes: &Hermes, settings: &Settings) -> Option<RunTools> {
    #[cfg(feature = "testing")]
    if settings.testing.hermes.is_some() {
        return None;
    }
    let limit = duration(settings.tools_read_ms).min(duration(settings.hermes_ms));
    Some(match hermes.read_tool_list(limit).await {
        Err(_) => RunTools::Unreadable,
        Ok(None) => RunTools::Default,
        Ok(Some(printed)) => match parse_list(&printed) {
            None => RunTools::Unreadable,
            Some(names) if names.is_empty() => RunTools::Empty,
            Some(names) if names.iter().any(|name| name == SERVER_NAME) => RunTools::Included,
            Some(_) => RunTools::Missing,
        },
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, Instant};

    use arauto_testkit::hermes::{FakeHermes, built_example};
    use serde_json::json;

    use super::*;
    use crate::hermes::LIST_CEILING;

    const GET: &str = "config get platform_toolsets.api_server";

    fn fake() -> FakeHermes {
        FakeHermes::new(&built_example("fake_hermes"))
    }

    fn hermes_of(fake: &FakeHermes) -> Hermes {
        Hermes::new(Some(fake.program().into_os_string()))
    }

    fn names(calls: &[Vec<String>]) -> Vec<String> {
        calls.iter().map(|call| call.join(" ")).collect()
    }

    fn sets(fake: &FakeHermes) -> usize {
        fake.config_calls()
            .iter()
            .filter(|call| call.get(1).is_some_and(|word| word == "set"))
            .count()
    }

    /// U1 (D3): the parser takes only the forms it knows.
    #[test]
    fn the_parser_takes_only_the_forms_it_knows() {
        let web_terminal = Some(vec!["web".to_owned(), "terminal".to_owned()]);
        for block in [
            "  - web\n  - terminal\n",
            "  - web\n  - terminal",
            "  - web\r\n  - terminal\r\n\r\n",
            "Command helper: applied 1 secret\n  - web\n  - terminal\n",
            "a line printed before\n\n  - web\n  - terminal\n",
        ] {
            assert_eq!(parse_list(block.as_bytes()), web_terminal, "{block:?}");
        }
        assert_eq!(parse_list(b"[]"), Some(vec![]));
        assert_eq!(parse_list(b"[]\n"), Some(vec![]));
        assert_eq!(parse_list(b"a line before\n[]\n"), Some(vec![]));
        // The list is the run at the END: the line printed before it is not an item.
        assert_eq!(
            parse_list(b"a line before\n  - web\n"),
            Some(vec!["web".to_owned()])
        );
        for bad in [
            // The one-line flow forms are not lists: a text value prints the same way.
            "['web', 'terminal']",
            "[\"web\", \"terminal\"]",
            "[web, terminal]",
            "[web,terminal]\n",
            "['web', 'terminal']\r\n",
            "[ ]",
            "[[]]",
            "[[web]]",
            "a line before\n[web, terminal]\n",
            // Plain values and nothing.
            "None",
            "null",
            "web",
            "",
            "\n\n",
            // A line out of form ends the run, so no run is left at the end.
            "  - web\n  - b c\n",
            "  - web\n  - ../x\n",
            "  - web\n  - $(x)\n",
            "  - web\n  -  terminal\n",
            "  - web\n  terminal\n",
            "  - web\n - terminal\n",
            // No indent, other indent, a nested or a quoted item.
            "- web\n- terminal\n",
            "    - web\n",
            "  - [web]\n",
            "  - web: x\n",
            // A bad line before the run: the run may have been cut, nothing is read.
            "  - b c\n  - web\n",
            "  - web\nnotice\n",
            "notice\n",
            "  -\n",
            "  - \n",
        ] {
            assert_eq!(parse_list(bad.as_bytes()), None, "{bad:?}");
        }
        let items = |count: usize| {
            (0..count)
                .map(|n| format!("  - n{n}\n"))
                .collect::<String>()
                .into_bytes()
        };
        assert_eq!(parse_list(&items(64)).map(|list| list.len()), Some(64));
        assert_eq!(parse_list(&items(65)), None);
        let long = format!("  - {}\n", "a".repeat(65));
        assert_eq!(parse_list(long.as_bytes()), None);
        let longest = format!("  - {}\n", "a".repeat(64));
        assert!(parse_list(longest.as_bytes()).is_some());
        assert_eq!(parse_list(&[0xff, b'\n']), None);
        assert_eq!(parse_list(b""), None);
        assert_eq!(
            list_value(&["a".to_owned(), "arauto".to_owned()]),
            r#"["a","arauto"]"#
        );
    }

    /// U2 (D5): no row changes nothing.
    #[tokio::test]
    async fn no_row_changes_nothing() {
        let fake = fake();
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::NoRow));
        assert_eq!(names(&fake.config_calls()), [GET]);
        assert_eq!(fake.tool_list("api_server"), None);
    }

    /// U3: a list that includes `arauto` is left alone.
    #[tokio::test]
    async fn a_list_with_arauto_is_left_alone() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web", "arauto"]));
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Included));
        assert_eq!(names(&fake.config_calls()), [GET]);
        assert_eq!(fake.tool_list("api_server"), Some(json!(["web", "arauto"])));
    }

    /// U4 (D6): `arauto` is appended, the others stay in their order, the argument is exact.
    #[tokio::test]
    async fn arauto_is_appended_and_the_others_stay_in_order() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web", "other_server"]));
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Added));
        assert_eq!(
            fake.config_calls(),
            [
                vec!["config", "get", "platform_toolsets.api_server"],
                vec![
                    "config",
                    "set",
                    "platform_toolsets.api_server",
                    r#"["web","other_server","arauto"]"#
                ],
                vec!["config", "get", "platform_toolsets.api_server"],
            ]
        );
        assert_eq!(
            fake.tool_list("api_server"),
            Some(json!(["web", "other_server", "arauto"]))
        );
    }

    /// U5 (D5): an empty list is not written to.
    #[tokio::test]
    async fn an_empty_list_is_left_alone() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&[]));
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Empty));
        assert_eq!(names(&fake.config_calls()), [GET]);
        assert_eq!(sets(&fake), 0);
    }

    /// U6 (D3): a form the parser does not know is unreadable and nothing is written.
    #[tokio::test]
    async fn an_unknown_form_is_unreadable() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web"]));
        fake.tool_list_form("bare");
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Unreadable));
        assert_eq!(sets(&fake), 0);
    }

    /// U7: not knowing is not a "no": a failed, missing or hanging read writes nothing.
    #[tokio::test]
    async fn a_read_that_fails_writes_nothing() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web"]));
        fake.failing_config_get(true);
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Unreadable));
        fake.failing_config_get(false);
        assert_eq!(sets(&fake), 0);

        let missing = Hermes::new(Some("/no/such/program".into()));
        let outcome = add_step(&missing, &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Unreadable));

        fake.hanging(Some("get"));
        let started = Instant::now();
        let outcome = add_step(
            &hermes_of(&fake),
            &Settings::default().with_tools_step_ms(600),
        )
        .await;
        let took = started.elapsed();
        fake.hanging(None);
        assert_eq!(outcome, Some(ToolsOutcome::Unreadable));
        assert!(took < Duration::from_secs(20), "{took:?}");
        assert!(took >= Duration::from_millis(500), "{took:?}");
        assert_eq!(sets(&fake), 0);
    }

    /// U8: a refused `set` is reported as refused, record `missing`, the list stays.
    #[tokio::test]
    async fn a_refused_set_is_reported_as_refused() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web"]));
        fake.failing_config_set(true);
        let outcome = add_step(&hermes_of(&fake), &Settings::default())
            .await
            .unwrap();
        assert_eq!(outcome, ToolsOutcome::SetRefused);
        assert_eq!(outcome.record(), RunTools::Missing);
        assert_eq!(
            outcome.told(),
            [Told::RunToolsNotAdded, Told::RunToolsMissing]
        );
        assert_eq!(fake.tool_list("api_server"), Some(json!(["web"])));
    }

    /// U9 (D7): a Hermes that stored the text is not reported as added.
    #[tokio::test]
    async fn a_value_stored_as_text_is_not_confirmed() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web"]));
        fake.tool_list_set_as_text(true);
        let outcome = add_step(&hermes_of(&fake), &Settings::default())
            .await
            .unwrap();
        assert_eq!(outcome, ToolsOutcome::NotConfirmed);
        assert_eq!(outcome.record(), RunTools::Unreadable);
    }

    /// U10 (D7): a write that loses entries is not reported as added.
    #[tokio::test]
    async fn a_write_that_loses_entries_is_not_confirmed() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web", "other_server"]));
        fake.tool_list_set_replaces(true);
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::NotConfirmed));
    }

    /// U11: a `set` that times out is "not confirmed", at its deadline.
    #[tokio::test]
    async fn a_set_that_hangs_is_not_confirmed_at_its_deadline() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web"]));
        fake.hanging(Some("set"));
        let started = Instant::now();
        let outcome = add_step(
            &hermes_of(&fake),
            &Settings::default().with_tools_step_ms(6_000),
        )
        .await;
        let took = started.elapsed();
        fake.hanging(None);
        assert_eq!(outcome, Some(ToolsOutcome::NotConfirmed));
        assert!(took < Duration::from_secs(20), "{took:?}");
    }

    /// U12 (D3): a name out of form is never written back.
    #[tokio::test]
    async fn a_name_out_of_form_is_never_written_back() {
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["web", "b c"]));
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Unreadable));
        assert_eq!(sets(&fake), 0);
    }

    /// U16: an output that reached the capture ceiling may have been cut, and is not read: nothing
    /// is written, and `status` says unreadable. The fake prints a long line before the value so
    /// that the cut lands on a line boundary inside the list, right after its first item: without
    /// the rule the part kept parses as the list `x` and the write would drop `web`.
    #[tokio::test]
    async fn an_output_at_the_ceiling_is_unreadable() {
        const HEADER: &str = "(fake hermes) reading the configuration\n";
        const FIRST_ITEM: &str = "  - x\n";
        let fake = fake();
        fake.set_tool_list("api_server", Some(&["x", "web"]));
        // One byte for the line break the fake adds after what it is given.
        let preamble = "n".repeat(LIST_CEILING - FIRST_ITEM.len() - HEADER.len() - 1);
        fake.tool_list_preamble(&preamble);
        let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(outcome, Some(ToolsOutcome::Unreadable));
        assert_eq!(sets(&fake), 0);
        assert_eq!(fake.tool_list("api_server"), Some(json!(["x", "web"])));
        let state = read_now(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(state, Some(RunTools::Unreadable));
        // Below the ceiling the same shape is read whole.
        fake.tool_list_preamble(&"n".repeat(1024));
        let state = read_now(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(state, Some(RunTools::Missing));
    }

    /// U18: a run of items at the end of a longer value is not the list: a list whose first item
    /// is a mapping, an item with more lines, a mapping that holds a list, a comment. Nothing is
    /// read and nothing is written, so no entry is dropped.
    #[tokio::test]
    async fn the_end_of_a_longer_value_is_not_read_as_the_list() {
        for printed in [
            "  - name: web\n    extra: x\n  - terminal\n",
            "  - web\n    more\n  - terminal\n",
            "  tools:\n  - web\n",
            "tools:\n  - web\n",
            "  # comment\n  - web\n",
            "# comment\n  - web\n",
            "[other\n  - web\n",
        ] {
            assert_eq!(parse_list(printed.as_bytes()), None, "{printed:?}");
        }
        // The same shapes through the fake: what it prints before the value is the end of a
        // longer value, so neither join nor status reads it, and nothing is written.
        for preamble in ["tools:", "  - name: web\n    extra: x", "# comment"] {
            let fake = fake();
            fake.set_tool_list("api_server", Some(&["terminal"]));
            fake.tool_list_preamble(preamble);
            let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
            assert_eq!(outcome, Some(ToolsOutcome::Unreadable), "{preamble:?}");
            assert_eq!(sets(&fake), 0, "{preamble:?}");
            assert_eq!(
                fake.tool_list("api_server"),
                Some(json!(["terminal"])),
                "{preamble:?}"
            );
            let state = read_now(&hermes_of(&fake), &Settings::default()).await;
            assert_eq!(state, Some(RunTools::Unreadable), "{preamble:?}");
        }
    }

    /// U17: a one-line value is text, never a list: `[web, terminal]` is not turned into one, and
    /// nothing is written.
    #[tokio::test]
    async fn a_one_line_text_value_is_never_turned_into_a_list() {
        for text in [
            "[web, terminal]",
            "['web', 'terminal']",
            "[\"web\",\"terminal\"]",
        ] {
            let fake = fake();
            fake.set_tool_text("api_server", text);
            let outcome = add_step(&hermes_of(&fake), &Settings::default()).await;
            assert_eq!(outcome, Some(ToolsOutcome::Unreadable), "{text}");
            let state = read_now(&hermes_of(&fake), &Settings::default()).await;
            assert_eq!(state, Some(RunTools::Unreadable), "{text}");
            assert_eq!(sets(&fake), 0, "{text}");
            assert_eq!(fake.tool_list("api_server"), Some(json!(text)));
        }
    }

    /// U13: a test that answers Hermes itself never reaches a program.
    #[tokio::test]
    async fn a_test_that_answers_hermes_itself_runs_no_program() {
        let fake = fake();
        let settings = Settings::default().with_hermes(|_| true);
        assert_eq!(add_step(&hermes_of(&fake), &settings).await, None);
        assert_eq!(read_now(&hermes_of(&fake), &settings).await, None);
        assert!(fake.calls().is_empty());
    }

    /// U14: `status` reads once, maps each state, and never writes.
    #[tokio::test]
    async fn status_reads_once_and_never_writes() {
        type Case = (
            Option<&'static [&'static str]>,
            Option<&'static str>,
            RunTools,
        );
        let cases: [Case; 7] = [
            (None, None, RunTools::Default),
            (Some(&["web", "arauto"]), None, RunTools::Included),
            (Some(&["web"]), None, RunTools::Missing),
            (Some(&[]), None, RunTools::Empty),
            (Some(&["web"]), Some("bare"), RunTools::Unreadable),
            (Some(&["web"]), Some("python"), RunTools::Unreadable),
            (Some(&["web", "arauto"]), Some("json"), RunTools::Unreadable),
        ];
        for (list, form, expected) in cases {
            let fake = fake();
            fake.set_tool_list("api_server", list);
            if let Some(form) = form {
                fake.tool_list_form(form);
            }
            let state = read_now(&hermes_of(&fake), &Settings::default()).await;
            assert_eq!(state, Some(expected));
            assert_eq!(names(&fake.config_calls()), [GET]);
        }
        let fake = fake();
        fake.failing_config_get(true);
        let state = read_now(&hermes_of(&fake), &Settings::default()).await;
        assert_eq!(state, Some(RunTools::Unreadable));
        assert_eq!(sets(&fake), 0);
    }

    /// U15: table 2.3 of the spec, exactly.
    #[test]
    fn the_outcomes_map_to_what_is_recorded_and_told() {
        use ToolsOutcome as O;
        let table: [(O, RunTools, &[Told]); 7] = [
            (O::NoRow, RunTools::Default, &[]),
            (O::Included, RunTools::Included, &[]),
            (O::Empty, RunTools::Empty, &[Told::RunToolsEmpty]),
            (
                O::Unreadable,
                RunTools::Unreadable,
                &[Told::RunToolsUnreadable],
            ),
            (
                O::Added,
                RunTools::Added,
                &[Told::RunToolsAdded, Told::RunToolsRestart],
            ),
            (
                O::SetRefused,
                RunTools::Missing,
                &[Told::RunToolsNotAdded, Told::RunToolsMissing],
            ),
            (
                O::NotConfirmed,
                RunTools::Unreadable,
                &[Told::RunToolsNotConfirmed],
            ),
        ];
        for (outcome, record, told) in table {
            assert_eq!(outcome.record(), record);
            assert_eq!(outcome.told(), told);
        }
    }

    #[test]
    fn the_words_round_trip() {
        for state in [
            RunTools::Included,
            RunTools::Default,
            RunTools::Added,
            RunTools::Missing,
            RunTools::Empty,
            RunTools::Unreadable,
        ] {
            assert_eq!(RunTools::parse(state.as_str()), Some(state));
        }
        assert_eq!(RunTools::parse("not_checked"), None);
        assert_eq!(RunTools::NotChecked.as_str(), "not_checked");
        assert_eq!(
            RunTools::NotChecked.text(),
            Told::RunToolsNotChecked.phrase()
        );
    }
}
