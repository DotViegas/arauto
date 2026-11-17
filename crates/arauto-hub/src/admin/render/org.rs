//! What `admin org` prints.

use std::fmt::Write as _;

use arauto_protocol::names::HermesName;

use super::{Report, escape};
use crate::core::org::{Chart, Kind, kind};
use crate::repo::org::{PublishOutcome, Published};

/// The word of a kind of arrow.
const fn word(kind: Option<Kind>) -> &'static str {
    match kind {
        Some(Kind::Instruction) => "instruction",
        Some(Kind::Request) => "request",
        Some(Kind::Note) => "note",
        None => "-",
    }
}

/// `admin org show`: the levels top-down with their Hermes and roles, the Hermes not placed
/// yet, the links with their kind, and the version a publish must start from.
#[must_use]
pub fn show(chart: &Chart) -> Report {
    let mut text = String::new();
    if chart.levels.is_empty() {
        text.push_str("No level yet.\n");
    }
    for (position, level) in chart.levels.iter().enumerate() {
        let _ = writeln!(text, "Level {} {}:", position + 1, level.name.as_str());
        let mut any = false;
        for member in &chart.members {
            let Some(place) = member
                .place
                .as_ref()
                .filter(|place| place.level == level.id)
            else {
                continue;
            };
            any = true;
            let role = place
                .role
                .as_ref()
                .map(|role| format!(" - {}", escape(role.as_str())))
                .unwrap_or_default();
            let _ = writeln!(text, "  {} (x {}){role}", member.name, place.x.get());
        }
        if !any {
            text.push_str("  nobody\n");
        }
    }
    let unplaced: Vec<String> = chart
        .members
        .iter()
        .filter(|member| member.place.is_none())
        .map(|member| member.name.to_string())
        .collect();
    if !unplaced.is_empty() {
        let _ = writeln!(text, "Not placed yet: {}", unplaced.join(", "));
    }
    if chart.links.is_empty() {
        text.push_str("No link.\n");
    }
    for link in &chart.links {
        let arrow = kind(
            chart.position_of_member(&link.from),
            chart.position_of_member(&link.to),
        );
        let marks = if link.follow {
            format!("may send and wake; {} follows its instructions", link.to)
        } else if link.wake {
            "wakes".to_owned()
        } else {
            "does not wake".to_owned()
        };
        let _ = writeln!(
            text,
            "Link {} -> {}: {}, {marks}",
            link.from,
            link.to,
            word(arrow),
        );
    }
    text.push_str(
        "The levels, the roles and the kinds of arrow are only shown: no Hermes is told them. A \
         note is data, except on a link with the follow mark, whose notice tells the receiver to \
         follow it in that run.\n",
    );
    let _ = writeln!(text, "version: {}", chart.version().to_hex());
    Report::done(text)
}

/// `admin org publish`. `follow_dropped` are the links the file kept without the mark they had:
/// the publish removed it, and the report says so, link by link.
#[must_use]
pub fn published(outcome: &PublishOutcome, follow_dropped: &[(HermesName, HermesName)]) -> Report {
    match outcome {
        PublishOutcome::Published(Published {
            levels_added,
            levels_removed,
            levels_renamed,
            levels_moved,
            members_changed,
            roles_changed,
            links_created,
            links_changed,
            links_removed,
            version,
        }) => {
            let mut text = format!(
                "Published. Levels: {levels_added} added, {levels_removed} removed, \
                 {levels_renamed} renamed, {levels_moved} moved. Hermes changed: \
                 {members_changed} ({roles_changed} roles). Links: {links_created} created, \
                 {links_changed} changed, {links_removed} removed.\n"
            );
            for (from, to) in follow_dropped {
                let _ = writeln!(
                    text,
                    "The follow mark of {from} -> {to} was removed: the file does not set it."
                );
            }
            let _ = writeln!(text, "version: {}", version.to_hex());
            Report::done(text)
        }
        PublishOutcome::Unchanged => {
            Report::done("The chart already is as the draft says. Nothing was written.\n".into())
        }
        PublishOutcome::Stale => Report::not_done(
            "stale: nothing was published. The chart changed after this draft started: run \
             `arauto-hub admin org show` and start from its version.\n"
                .into(),
        ),
        PublishOutcome::Invalid(reason) => {
            Report::not_done(format!("{reason} Nothing was published.\n"))
        }
    }
}
