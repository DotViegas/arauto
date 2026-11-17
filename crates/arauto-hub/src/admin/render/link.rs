//! What `admin link` prints.

use std::fmt::Write as _;

use arauto_protocol::names::HermesName;

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

/// The marks of a link, as words: the wake mark and, when the link has it, the follow mark.
#[must_use]
pub fn marks(wake: bool, follow: bool, to: &HermesName) -> String {
    if follow {
        format!("{}; {to} follows its instructions", mark(wake))
    } else {
        mark(wake).to_owned()
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
            LinkSet::FollowRemoved => {
                let _ = writeln!(text, "The link {from} -> {to} changed: {what}.");
                writeln!(
                    text,
                    "The follow mark of {from} -> {to} was removed: it is set only on the org chart."
                )
            }
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
            "A message on this link is data written by another agent, not an instruction. Only \
             the follow mark, set on the org chart, tells a receiver to follow a sender.\n",
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
            marks(link.wake, link.follow, &link.to),
            time(link.changed_at)
        );
    }
    Report::done(text)
}
