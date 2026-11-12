//! What the admin command prints.
//!
//! Two rules:
//!
//! - **No secret but the invite.** The invite appears in [`block`], once, because the admin
//!   has to carry it to the Hermes. Nothing else here is a secret: the key of the hub that is
//!   shown is the public one, and the verification code is never shown.
//! - **Every text that came from a device is escaped** ([`escape`]): what a device declares,
//!   the version of its connector and the addresses stored with it reach the terminal as
//!   plain ASCII, so that no escape sequence and no character that reorders or hides text
//!   gets there.

use std::fmt::Write as _;

use crate::core::model::{Device, DeviceState};
use crate::db::SchemaState;
use crate::mark::MarkValue;
use crate::repo::alert::Alert;
use crate::repo::approval::{ApproveOutcome, CODE_ATTEMPTS_MAX, DenyOutcome, LastUse, RequestView};
use crate::repo::audit_log::AuditRow;
use crate::repo::closing::Closed;
use crate::repo::device::UnknownListed;
use crate::repo::hermes::{HermesLine, Removed, WakeOutcome};
use crate::restore::{Installation, Standing};

use super::{Block, InitOutcome, Status, UNSEEN_COUNT_MAX};

pub mod access;
pub mod whatsapp;

/// What a command has to say, and whether it did what was asked. The program exits with an
/// error when it did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The text for the terminal, ending in a line feed.
    pub text: String,
    /// Whether what was asked was done.
    pub done: bool,
}

impl Report {
    const fn done(text: String) -> Self {
        Self { text, done: true }
    }

    const fn not_done(text: String) -> Self {
        Self { text, done: false }
    }
}

/// Makes a text that is not trusted safe to print: printable ASCII stays, the backslash is
/// doubled, and everything else (control characters, the escape character, and every
/// character outside ASCII, among them the ones that reorder or hide text) is written as
/// `\u{...}`.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(character),
            other => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(other));
            }
        }
    }
    out
}

/// A stored JSON value, on one line, with everything outside printable ASCII escaped.
fn json(value: &serde_json::Value) -> String {
    let mut out = String::new();
    for character in value.to_string().chars() {
        match character {
            ' '..='~' => out.push(character),
            other => {
                let _ = write!(out, "\\u{:04x}", u32::from(other));
            }
        }
    }
    out
}

/// A time of the hub, in UTC, as `2027-01-15 08:00:00Z`.
#[must_use]
pub fn time(ms: i64) -> String {
    let seconds = ms.div_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    // The civil date of a day number, in the proleptic Gregorian calendar (the algorithm of
    // Howard Hinnant, with eras of 400 years that start on March 1st).
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era
        .saturating_add(era.saturating_mul(400))
        .saturating_add(i64::from(month <= 2));
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60
    )
}

fn maybe_time(ms: Option<i64>) -> String {
    ms.map_or_else(|| "never".to_owned(), time)
}

fn maybe_text(text: Option<&str>) -> String {
    text.map_or_else(|| "unknown".to_owned(), escape)
}

/// What every screen says while the restored mode is on (C11). `None` with the mode off.
#[must_use]
pub fn restored_notice(installation: &Installation) -> Option<String> {
    installation.restored_mode.then(|| {
        format!(
            "RESTORED MODE: the hub came back from a copy of {}. What was approved, revoked, \
             reconnected or changed in the accesses after that was lost. Reconnect each Hermes \
             and check the accesses and the list of admins.\n\n",
            maybe_time(installation.restored_copy_of)
        )
    })
}

/// `init`.
#[must_use]
pub fn init(outcome: &InitOutcome) -> Report {
    let (first, installed) = match outcome {
        InitOutcome::Created(installed) => ("The hub is installed.", installed),
        InitOutcome::AlreadyInstalled(installed) => (
            "The hub was already installed. Nothing was changed.",
            installed,
        ),
    };
    Report::done(format!(
        "{first}\nhub key: {}\n",
        installed.hub_key.to_text()
    ))
}

/// `migrate`.
#[must_use]
pub fn migrate(applied: &[i64]) -> Report {
    if applied.is_empty() {
        return Report::done("The database is already at the version of this program.\n".into());
    }
    let versions: Vec<String> = applied.iter().map(i64::to_string).collect();
    Report::done(format!("Applied migration(s): {}.\n", versions.join(", ")))
}

fn schema(state: &SchemaState) -> String {
    match state {
        SchemaState::Empty => "empty: run `arauto-hub init`".to_owned(),
        SchemaState::Current => "current".to_owned(),
        SchemaState::Pending { pending } => format!(
            "{} pending migration(s): take a backup and run `arauto-hub migrate \
             --i-have-a-backup`",
            pending.len()
        ),
        SchemaState::Newer { newest } => {
            format!("newer than this program (migration {newest})")
        }
        SchemaState::Changed { version } => {
            format!("migration {version} is not the one of this program")
        }
        SchemaState::Failed { version } => format!("migration {version} failed halfway"),
        SchemaState::Foreign => "not a database of arauto".to_owned(),
    }
}

/// `admin status`.
#[must_use]
pub fn status(status: &Status) -> Report {
    let mut text = String::new();
    let _ = writeln!(text, "database: {}", schema(&status.schema));
    let hub_key = status
        .hub_key
        .map_or_else(|| "none (not installed)".to_owned(), |key| key.to_text());
    let _ = writeln!(text, "hub key: {hub_key}");
    let mark = match status.mark {
        MarkValue::Absent => "absent".to_owned(),
        MarkValue::At(number) => number.to_string(),
    };
    let _ = writeln!(text, "audit mark: {mark}");
    let mut healthy = status.schema == SchemaState::Current;
    match status.standing {
        Some(Standing::Consistent) => {
            let _ = writeln!(text, "database against the mark: consistent");
        }
        Some(Standing::NotInstalled) => {
            healthy = false;
            let _ = writeln!(text, "installation: none. Run `arauto-hub init`");
        }
        Some(Standing::Restored(evidence)) => {
            healthy = false;
            let _ = writeln!(
                text,
                "database against the mark: RESTORED FROM AN OLDER COPY ({evidence}). Start \
                 `arauto-hub serve`: the hub turns the restored mode on"
            );
        }
        None => {}
    }
    if let Some(installation) = status.installation {
        if installation.restored_mode {
            let _ = writeln!(
                text,
                "restored mode: ON since {}. The hub came back from a copy of {}. What was \
                 approved, revoked, reconnected or changed after that was lost. Reconnect each \
                 Hermes.",
                maybe_time(installation.restored_at),
                maybe_time(installation.restored_copy_of)
            );
        } else {
            let _ = writeln!(text, "restored mode: off");
        }
    }
    let count = |state: DeviceState| {
        status
            .hermes
            .iter()
            .flat_map(|line| line.in_use.iter().chain(line.request.iter()))
            .filter(|device| device.state == state)
            .count()
    };
    let _ = writeln!(
        text,
        "hermes: {} line(s); devices: {} active, {} approved, {} pending, {} quarantined, {} \
         suspended",
        status.hermes.len(),
        count(DeviceState::Active),
        count(DeviceState::Approved),
        count(DeviceState::Pending),
        count(DeviceState::Quarantined),
        count(DeviceState::Suspended)
    );
    let ceiling = usize::try_from(UNSEEN_COUNT_MAX).unwrap_or(usize::MAX);
    let more = if status.unseen_alerts >= ceiling {
        " or more"
    } else {
        ""
    };
    let _ = writeln!(text, "alerts not seen: {}{more}", status.unseen_alerts);
    if status.unknown_devices > 0 {
        let more = if status.unknown_devices >= ceiling {
            " or more"
        } else {
            ""
        };
        let _ = writeln!(
            text,
            "devices the hub does not know: {}{more}. They presented a valid credential: see \
             `admin alerts`",
            status.unknown_devices
        );
    }
    Report {
        text,
        done: healthy,
    }
}

/// The label of a Hermes line (`docs/10-maquinas-de-estado.md`, section 3), from what the
/// database knows. Whether a session is open is of the running hub, not of this command.
fn label(line: &HermesLine) -> &'static str {
    if line.hermes.deleted_at.is_some() {
        return "removed";
    }
    let in_use = line.in_use.as_ref().map(|device| device.state);
    let request = line.request.as_ref().map(|device| device.state);
    match (request, in_use) {
        (Some(DeviceState::Pending), _) => "waiting for your code",
        (Some(DeviceState::Approved), _) => "approved, waiting for the connector",
        (_, _) if line.invite_expires_at.is_some() => "waiting for the Hermes",
        (_, Some(DeviceState::Active)) => "active",
        (_, Some(DeviceState::Quarantined)) | (Some(DeviceState::Quarantined), _) => {
            "in quarantine"
        }
        (_, Some(DeviceState::Suspended)) => "suspended",
        _ => "no device",
    }
}

/// `admin hermes list`.
#[must_use]
pub fn hermes_list(lines: &[HermesLine]) -> Report {
    if lines.is_empty() {
        return Report::done("No Hermes yet. Create one with `admin hermes add NAME`.\n".into());
    }
    let mut text = String::new();
    for line in lines {
        let wake = if line.hermes.can_be_woken {
            "can be woken"
        } else {
            "is not woken"
        };
        let last = line
            .in_use
            .as_ref()
            .map(|device| {
                format!(
                    "; last use {} from {}",
                    maybe_time(device.last_contact_at),
                    maybe_text(device.last_address.as_deref())
                )
            })
            .unwrap_or_default();
        let _ = writeln!(
            text,
            "{:<32}  {}; {wake}{last}",
            line.hermes.name.as_str(),
            label(line)
        );
    }
    Report::done(text)
}

/// `admin hermes add`.
#[must_use]
pub fn hermes_added(name: &str) -> Report {
    Report::done(format!(
        "Hermes {} was created. It has no device and no access yet: connect it with `admin \
         invite {}`.\n",
        escape(name),
        escape(name)
    ))
}

fn closed(closed: &Closed) -> String {
    let mut text = String::new();
    for device in &closed.devices {
        let _ = writeln!(
            text,
            "device {}: was {}, now {}",
            device.id.as_hyphenated(),
            device.was,
            device.now
        );
    }
    if closed.invite.is_some() {
        let _ = writeln!(text, "the issued invite was cancelled");
    }
    text
}

/// `admin hermes remove`.
#[must_use]
pub fn hermes_removed(removed: &Removed) -> Report {
    Report::done(format!(
        "Hermes {} was removed. Its name is never used again.\n{}{} link(s) with other Hermes \
         removed.\n",
        removed.hermes.name.as_str(),
        closed(&removed.closed),
        removed.links_removed
    ))
}

/// `admin revoke`.
#[must_use]
pub fn revoked(name: &str, what: &Closed) -> Report {
    if what.devices.is_empty() && what.invite.is_none() {
        return Report::done(format!(
            "Hermes {} had nothing to revoke: no device, no request and no invite.\n",
            escape(name)
        ));
    }
    Report::done(format!(
        "Hermes {} was revoked.\n{}The connector does not erase anything by itself.\n",
        escape(name),
        closed(what)
    ))
}

/// `admin wake`.
#[must_use]
pub fn wake(name: &str, on: bool, outcome: WakeOutcome) -> Report {
    let state = if on { "can be woken" } else { "is not woken" };
    let text = match outcome {
        WakeOutcome::Changed => format!("Hermes {} now {state}.\n", escape(name)),
        WakeOutcome::Unchanged | WakeOutcome::NotFound => {
            format!(
                "Hermes {} already {state}. Nothing was changed.\n",
                escape(name)
            )
        }
    };
    Report::done(text)
}

/// `admin invite`: the block. **This shows the invite**, the one secret this command ever
/// prints, and it is the only time it appears.
#[must_use]
pub fn block(block: &Block, now: i64) -> Report {
    let issued = &block.issued;
    let mut text = String::new();
    let kind = if issued.reconnect {
        "reconnect"
    } else {
        "connect"
    };
    let minutes = issued.expires_at.saturating_sub(now).max(0) / 60_000;
    let _ = writeln!(
        text,
        "Invite to {kind} Hermes {}. It is valid for {minutes} minutes and for one use.",
        issued.hermes.name.as_str()
    );
    if issued.cancelled.is_some() {
        let _ = writeln!(text, "The invite issued before was cancelled.");
    }
    if let Some(request) = issued.closed_request {
        let _ = writeln!(
            text,
            "The request in progress was closed (it was {}).",
            request.was
        );
    }
    if issued.reconnect {
        let _ = writeln!(
            text,
            "The device in use goes on until the new one confirms its first credential."
        );
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "hub:     {}", block.hub_url);
    let _ = writeln!(text, "hub key: {}", block.hub_key.to_text());
    let invite = issued.invite.reveal();
    let _ = writeln!(text, "invite:  {}", invite.as_str());
    let _ = writeln!(text);
    let _ = writeln!(
        text,
        "arauto-connector join \"{}\" \"{}\" \"{}\"",
        block.hub_url,
        block.hub_key.to_text(),
        invite.as_str()
    );
    let _ = writeln!(text);
    let _ = writeln!(
        text,
        "The invite is shown only now. The Hermes answers a six-digit code: type it in `admin \
         approve {}`.",
        issued.hermes.name.as_str()
    );
    Report::done(text)
}

fn last_use(last: &LastUse) -> String {
    format!(
        "Last use of the current device ({}): {}, from {}.",
        last.state,
        maybe_time(last.at),
        maybe_text(last.address.as_deref())
    )
}

fn declared(device: &Device) -> String {
    format!(
        "Informed by the device, not verified: machine \"{}\", system \"{}\", Hermes \"{}\", \
         connector \"{}\", from {}.",
        escape(device.declared.hostname.as_str()),
        escape(device.declared.os.as_str()),
        escape(device.declared.hermes_version.as_str()),
        maybe_text(device.connector_version.as_deref()),
        maybe_text(device.enrolled_from.as_deref())
    )
}

/// What `admin approve` shows before asking for the code. The code itself is never shown.
#[must_use]
pub fn request(name: &str, view: &RequestView) -> String {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "Request of Hermes {} ({}), enrollment {} of 5, {} wrong code(s) of {CODE_ATTEMPTS_MAX}.",
        escape(name),
        view.request.state,
        view.request.enrollments,
        view.request.code_attempts
    );
    let _ = writeln!(text, "{}", declared(&view.request));
    if let Some(last) = &view.replaces {
        let _ = writeln!(text, "This is a reconnect. {}", last_use(last));
    }
    text
}

/// `admin approve`.
#[must_use]
pub fn approved(name: &str, outcome: &ApproveOutcome) -> Report {
    let name = escape(name);
    match outcome {
        ApproveOutcome::Approved(approved) => {
            let mut text = format!(
                "{name} was approved. As soon as the connector confirms, it is connected.\n"
            );
            if let Some(last) = &approved.replaces {
                // The answer was about this: it is said again beside what it approved.
                text.push_str(&last_use(last));
                text.push('\n');
            }
            if approved.quarantined.is_some() {
                text.push_str(
                    "The last use was not recognized: the alert of a copy was raised and the \
                     old device is in quarantine.\n",
                );
            } else if approved.replaces.is_some() {
                text.push_str(
                    "The accesses this Hermes already had are valid for the new device once it \
                     confirms.\n",
                );
            } else {
                text.push_str("It has no access to any number yet.\n");
            }
            Report::done(text)
        }
        ApproveOutcome::WrongCode { attempts } => Report::not_done(format!(
            "Wrong code: attempt {attempts} of {CODE_ATTEMPTS_MAX}. Type the last code the \
             Hermes answered.\n"
        )),
        ApproveOutcome::Denied { .. } => Report::not_done(format!(
            "Wrong code for the fifth time: the request of {name} was denied. Issue a new \
             invite.\n"
        )),
        ApproveOutcome::Expired { .. } => Report::not_done(format!(
            "The 10 minutes passed: the request of {name} expired. Issue a new invite.\n"
        )),
        ApproveOutcome::NeedsRecognition(last) => Report::not_done(format!(
            "This is a reconnect. {}\nIf the Hermes was off and there was use, someone used a \
             copy.\nRun the command again with --recognized or --not-recognized, about this \
             last use. Nothing was approved.\n",
            last_use(last)
        )),
        ApproveOutcome::NoRequest | ApproveOutcome::NotFound => Report::not_done(format!(
            "{name} has no pending request. If the Hermes answered no code, do not approve; \
             if it answered AR-102, another device used the invite: issue another.\n"
        )),
    }
}

/// `admin deny`.
#[must_use]
pub fn denied(name: &str, outcome: DenyOutcome) -> Report {
    let name = escape(name);
    match outcome {
        DenyOutcome::Denied { .. } => Report::done(format!("The request of {name} was denied.\n")),
        DenyOutcome::NoRequest | DenyOutcome::NotFound => Report::not_done(format!(
            "{name} has no pending request. An approved request is closed by `admin revoke` or \
             by a new invite.\n"
        )),
    }
}

/// The list of the devices the hub does not know (C11): the time, the address and what each
/// one declares. Empty when there is none.
fn unknown_devices(unknown: &[UnknownListed]) -> String {
    let mut text = String::new();
    if unknown.is_empty() {
        return text;
    }
    let _ = writeln!(
        text,
        "Devices the hub does not know. Each one presented a valid credential: it was connected \
         or reconnected after the copy the hub came back from. Its way back is a new invite."
    );
    for device in unknown {
        let proof = if device.proof_pending {
            "  PROOF OF A RESTORATION NOT HANDLED YET"
        } else {
            ""
        };
        let _ = writeln!(
            text,
            "{}  {}  from {}  connector \"{}\" (informed by the device, not verified){proof}",
            device.id.as_hyphenated(),
            time(device.seen_at),
            maybe_text(device.address.as_deref()),
            maybe_text(device.connector_version.as_deref())
        );
    }
    text
}

/// `admin alerts`: the alerts, and after them the devices the hub does not know.
#[must_use]
pub fn alerts(alerts: &[Alert], unknown: &[UnknownListed]) -> Report {
    if alerts.is_empty() {
        return Report::done(format!("No alerts.\n{}", unknown_devices(unknown)));
    }
    let mut text = String::new();
    for alert in alerts {
        let seen = alert
            .seen_at
            .map_or_else(|| "NOT SEEN".to_owned(), |at| format!("seen {}", time(at)));
        let _ = writeln!(
            text,
            "{}  {}  {:<20}  hermes {}  {}  {}",
            alert.id.as_hyphenated(),
            time(alert.at),
            alert.kind,
            maybe_text(alert.hermes_name.as_deref()),
            json(&alert.details),
            seen
        );
    }
    text.push_str(&unknown_devices(unknown));
    Report::done(text)
}

/// `admin alerts --ack`.
#[must_use]
pub fn acknowledged(found: bool) -> Report {
    if found {
        Report::done("The alert was marked as seen.\n".into())
    } else {
        Report::not_done("There is no unseen alert with that identifier.\n".into())
    }
}

/// `admin audit`.
#[must_use]
pub fn audit(rows: &[AuditRow]) -> Report {
    let mut text = String::new();
    for row in rows {
        let actor = match (&row.actor_id, &row.actor_note) {
            (Some(id), Some(note)) => format!(
                "{} {} ({})",
                escape(&row.actor_kind),
                id.as_hyphenated(),
                escape(note)
            ),
            (Some(id), None) => format!("{} {}", escape(&row.actor_kind), id.as_hyphenated()),
            (None, _) => escape(&row.actor_kind),
        };
        let target = match (&row.target_kind, &row.target_id) {
            (Some(kind), Some(id)) => format!("  {} {}", escape(kind), id.as_hyphenated()),
            _ => String::new(),
        };
        let details = json(&row.details);
        let details = if details == "{}" {
            String::new()
        } else {
            format!("  {details}")
        };
        let code = row
            .code
            .as_deref()
            .map(|code| format!(" {}", escape(code)))
            .unwrap_or_default();
        let from = row
            .address
            .as_deref()
            .map(|address| format!("  from {}", escape(address)))
            .unwrap_or_default();
        let _ = writeln!(
            text,
            "{:>8}{} {}  {:<28} {}{code}  by {actor}{from}{target}{details}",
            row.seq,
            if row.security { "*" } else { " " },
            time(row.at),
            escape(&row.action),
            escape(&row.result),
        );
    }
    if rows.is_empty() {
        text.push_str("The audit is empty.\n");
    }
    Report::done(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untrusted_text_reaches_the_terminal_as_plain_ascii() {
        assert_eq!(escape("build-7 (x86_64)"), "build-7 (x86_64)");
        // The escape character, a line feed, a bell and a backspace.
        assert_eq!(
            escape("a\u{1b}[31mred\n\u{7}\u{8}"),
            "a\\u{1b}[31mred\\u{a}\\u{7}\\u{8}"
        );
        // Characters that reorder or hide text, and any letter outside ASCII.
        assert_eq!(escape("\u{202e}evil"), "\\u{202e}evil");
        assert_eq!(escape("caf\u{e9}\u{200b}"), "caf\\u{e9}\\u{200b}");
        // What looks like an escape of this function cannot be forged.
        assert_eq!(escape("\\u{1b}"), "\\\\u{1b}");
        for text in [
            "\u{1b}]0;title\u{7}",
            "\r\n",
            "\u{9b}2J",
            "\u{2028}\u{2029}",
        ] {
            let shown = escape(text);
            assert!(
                shown.bytes().all(|byte| (0x20..=0x7e).contains(&byte)),
                "{shown:?}"
            );
        }
    }

    #[test]
    fn a_stored_value_is_printed_on_one_plain_line() {
        let value = serde_json::json!({ "note": "a\u{1b}b\n\u{202e}c", "count": 3 });
        let shown = json(&value);
        assert!(shown.bytes().all(|byte| (0x20..=0x7e).contains(&byte)));
        assert!(
            shown.contains("\\u001b") && shown.contains("\\u202e"),
            "{shown}"
        );
        assert_eq!(json(&serde_json::json!({})), "{}");
    }

    #[test]
    fn a_time_is_written_in_utc() {
        assert_eq!(time(0), "1970-01-01 00:00:00Z");
        assert_eq!(time(1_800_000_000_000), "2027-01-15 08:00:00Z");
        assert_eq!(time(951_782_400_000), "2000-02-29 00:00:00Z");
        assert_eq!(time(951_868_799_999), "2000-02-29 23:59:59Z");
        assert_eq!(time(1_709_251_199_000), "2024-02-29 23:59:59Z");
        assert_eq!(time(1_709_251_200_000), "2024-03-01 00:00:00Z");
        assert_eq!(time(1_767_225_599_000), "2025-12-31 23:59:59Z");
        assert_eq!(time(-1_000), "1969-12-31 23:59:59Z");
        // No time makes it fail.
        for extreme in [i64::MIN, i64::MAX] {
            assert!(time(extreme).ends_with('Z'));
        }
    }
}
