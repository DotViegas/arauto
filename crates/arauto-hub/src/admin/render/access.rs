//! What `admin access` prints. The identifiers of the chats in the engine are personal data of
//! third parties: they are shown to the admin, on the terminal of the machine of the hub,
//! escaped, and nowhere else.

use std::fmt::Write as _;

use super::{Report, escape, time};
use crate::admin::access::{ChatChanged, Granted, ListedAccess, Warning};
use crate::core::access::{Grant, Verbs};

fn verbs(verbs: &Verbs) -> String {
    let words = verbs.words();
    if words.is_empty() {
        "none".to_owned()
    } else {
        words.join(", ")
    }
}

fn grant(grant: &Grant) -> String {
    format!(
        "scope {}, verbs {}{}",
        grant.scope.as_str(),
        verbs(&grant.verbs),
        if grant.can_start_chats {
            ", may start chats"
        } else {
            ""
        }
    )
}

/// `admin access grant`.
#[must_use]
pub fn granted(hermes: &str, number: &str, granted: &Granted) -> Report {
    let mut text = String::new();
    let hermes = escape(hermes);
    let number = escape(number);
    if granted.unchanged {
        let _ = writeln!(
            text,
            "{hermes} already had this access to {number}: {}.",
            grant(&granted.grant)
        );
    } else {
        match &granted.before {
            None => {
                let _ = writeln!(
                    text,
                    "{hermes} now has access to {number}: {}.",
                    grant(&granted.grant)
                );
            }
            Some(before) => {
                let _ = writeln!(
                    text,
                    "The access of {hermes} to {number} changed: {}; before, {}.",
                    grant(&granted.grant),
                    grant(before)
                );
            }
        }
        let _ = writeln!(text, "It holds from the next call of the Hermes.");
    }
    for warning in &granted.warnings {
        match warning {
            Warning::CommandNumber => {
                let _ = writeln!(
                    text,
                    "WARNING: {number} is marked as the command channel of some Hermes. {hermes} \
                     can now write into that channel."
                );
            }
            Warning::Shared(others) => {
                let names: Vec<&str> = others.iter().map(|name| name.as_str()).collect();
                let _ = writeln!(
                    text,
                    "WARNING: other Hermes may also send or be woken on {number}: {}. The hub \
                     does not choose which one acts.",
                    names.join(", ")
                );
            }
        }
    }
    Report::done(text)
}

/// `admin access remove`.
#[must_use]
pub fn removed(hermes: &str, number: &str, lines: u64) -> Report {
    Report::done(format!(
        "{} no longer has access to {}; {lines} line(s) of chats went with it. It holds from the \
         next call of the Hermes.\n",
        escape(hermes),
        escape(number)
    ))
}

/// `admin access wake-wait`.
#[must_use]
pub fn wake_wait(hermes: &str, number: &str, seconds: u32, before: Option<u32>) -> Report {
    let hermes = escape(hermes);
    let number = escape(number);
    Report::done(match before {
        Some(before) => format!(
            "A message on {number} now wakes {hermes} after {seconds} s without a new message \
             in that chat (before, {before} s). Never more than 60 s after the first one.\n"
        ),
        None => format!("{hermes} already waited {seconds} s on {number}.\n"),
    })
}

/// `admin access chat` and `admin access chat-remove`.
#[must_use]
pub fn chat_changed(hermes: &str, number: &str, changed: &ChatChanged) -> Report {
    let hermes = escape(hermes);
    let number = escape(number);
    let chat = changed
        .chat
        .map_or_else(String::new, |chat| format!(" {}", chat.as_hyphenated()));
    let text = match (changed.unchanged, changed.after) {
        (true, Some(after)) => {
            format!(
                "The chat already had these verbs for {hermes} on {number}: {}.\n",
                verbs(&after)
            )
        }
        (true, None) => "Nothing changed.\n".to_owned(),
        (false, Some(after)) => format!(
            "In the chat{chat} of {number}, {hermes} now has: {}{}. It holds from the next call.\n",
            verbs(&after),
            changed.before.map_or_else(String::new, |before| format!(
                " (before: {})",
                verbs(&before)
            ))
        ),
        (false, None) => format!(
            "The chat{chat} of {number} has no line of its own for {hermes} any more: the verbs \
             of the number hold there, or, with the scope list, the chat is out of its scope.\n"
        ),
    };
    Report::done(text)
}

/// `admin access list`.
#[must_use]
pub fn list(listed: &[ListedAccess]) -> Report {
    if listed.is_empty() {
        return Report::done("No access.\n".to_owned());
    }
    let mut text = String::new();
    for shown in listed {
        let access = &shown.access;
        let _ = writeln!(
            text,
            "{} on {} ({}): {}; changed {}",
            access.hermes.as_str(),
            access.number.as_str(),
            access.number_state.as_str(),
            grant(&access.grant),
            time(access.changed_at)
        );
        for ((chat, line), ids) in access.lines.iter().zip(&shown.chats) {
            let ids: Vec<String> = ids.iter().map(|id| escape(id.as_str())).collect();
            let _ = writeln!(
                text,
                "    chat {} [{}]: {}",
                chat.as_hyphenated(),
                ids.join(", "),
                verbs(line)
            );
        }
    }
    Report::done(text)
}
