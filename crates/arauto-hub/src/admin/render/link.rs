//! What `admin link` prints.

use std::fmt::Write as _;

use super::{Report, time};
use crate::admin::link::{Linked, Unlinked};
use crate::repo::peer::{LinkLine, LinkSet};

fn mark(wake: bool) -> &'static str {
    if wake {
        "may send and wake"
    } else {
        "may send, without waking"
    }
}

/// `admin link add`.
#[must_use]
pub fn linked(linked: &Linked) -> Report {
    let mut text = String::new();
    for change in &linked.changes {
        let (from, to) = (&change.from, &change.to);
        let what = mark(change.wake);
        let _ = match change.outcome {
            LinkSet::Created => writeln!(text, "New link: {from} -> {to} ({what})."),
            LinkSet::Changed => writeln!(text, "The link {from} -> {to} changed: {what}."),
            _ => writeln!(text, "The link {from} -> {to} already was so: {what}."),
        };
    }
    if !linked.reach.is_empty() {
        text.push_str(
            "\nA link lets a request cross from one number to another: someone who misleads \
             one Hermes can make it ask the other to act on the other's numbers. What each \
             one reaches on WhatsApp:\n",
        );
        for reach in &linked.reach {
            if reach.numbers.is_empty() {
                let _ = writeln!(text, "  {}: no number", reach.hermes);
            }
            for (alias, verbs) in &reach.numbers {
                let verbs = if verbs.is_empty() {
                    "no verb".to_owned()
                } else {
                    verbs.join(", ")
                };
                let _ = writeln!(text, "  {}: {alias} ({verbs})", reach.hermes);
            }
        }
        text.push_str(
            "A message between Hermes agents is data written by another agent, never an order. \
             Telling a Hermes to obey another makes the hub a path of command, which the \
             project does not guarantee.\n",
        );
    }
    text.push_str("It holds from the next message and the next read.\n");
    Report::done(text)
}

/// `admin link remove`.
#[must_use]
pub fn unlinked(done: &[Unlinked]) -> Report {
    let mut text = String::new();
    for one in done {
        let _ = if one.removed {
            writeln!(
                text,
                "{} can no longer send messages to {}. What it had sent and was not read is \
                 no longer delivered.",
                one.from, one.to
            )
        } else {
            writeln!(text, "There was no link {} -> {}.", one.from, one.to)
        };
    }
    if done.iter().any(|one| one.removed) {
        Report::done(text)
    } else {
        Report::not_done(text)
    }
}

/// `admin link list`.
#[must_use]
pub fn list(links: &[LinkLine]) -> Report {
    if links.is_empty() {
        return Report::done(
            "No links: no Hermes can send a message to another. Create one with \
             `arauto-hub admin link add FROM TO`.\n"
                .to_owned(),
        );
    }
    let mut text = String::new();
    for link in links {
        let _ = writeln!(
            text,
            "{} -> {}: {}; changed {}",
            link.from,
            link.to,
            mark(link.wake),
            time(link.changed_at)
        );
    }
    Report::done(text)
}
