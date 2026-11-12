//! What `admin engine` and `admin number` print.
//!
//! No secret of the engine is ever printed: the key and the secret of the events show only as
//! "set". The QR and the code of a pairing are the only short-lived secrets shown, once, on the
//! terminal of the machine of the hub. What the engine said of itself is escaped.

use std::fmt::Write as _;

use super::{Report, escape, maybe_time, time};
use crate::admin::qr::QrCode;
use crate::admin::whatsapp::{Added, Changed, PairOutcome, Shown};
use crate::engine::{EngineError, EngineVersion, Ensured, Phone};
use crate::repo::engine::{EngineRow, SetOutcome};
use crate::repo::number::Number;

/// `admin engine set`.
#[must_use]
pub fn engine_set(outcome: SetOutcome) -> Report {
    Report::done(match outcome {
        SetOutcome::Created(_) => "The engine was created, with a new secret for its events. Set \
                                   its key with `arauto-hub admin engine set-key`.\n"
            .to_owned(),
        SetOutcome::Changed(_) => "The addresses of the engine changed. The hub applies them \
                                   to every number at its next check.\n"
            .to_owned(),
        SetOutcome::Unchanged(_) => "The engine already had these addresses.\n".to_owned(),
    })
}

/// `admin engine set-key` and `admin engine new-events-secret`.
#[must_use]
pub fn engine_secret(what: &str) -> Report {
    Report::done(format!(
        "{what}. It is sealed in the hub and is never shown. The hub applies it to every \
         number at its next check.\n"
    ))
}

/// `admin engine show`.
#[must_use]
pub fn engine_show(row: Option<&EngineRow>) -> Report {
    let Some(row) = row else {
        return Report::not_done(
            "There is no engine yet: run `arauto-hub admin engine set --url ... --events-url ...`.\n"
                .to_owned(),
        );
    };
    let mut text = String::new();
    let _ = writeln!(text, "Engine {}", row.id.as_hyphenated());
    let _ = writeln!(text, "  address        {}", escape(&row.address));
    let _ = writeln!(text, "  events from    {}", escape(&row.events_url));
    let _ = writeln!(
        text,
        "  key            {}",
        if row.api_key_enc.is_some() {
            "set"
        } else {
            "NOT SET"
        }
    );
    let _ = writeln!(text, "  events secret  set");
    let version = row
        .version_seen
        .as_deref()
        .map_or_else(|| "-".to_owned(), escape);
    let tested = match row.version_tested {
        Some(true) => "tested",
        Some(false) => "NOT TESTED",
        None => "not read yet",
    };
    let _ = writeln!(text, "  version        {version} ({tested})");
    let _ = writeln!(text, "  last answer    {}", maybe_time(row.last_ok_at));
    Report::done(text)
}

/// `admin engine health`.
#[must_use]
pub fn engine_health(result: Result<(), EngineError>) -> Report {
    match result {
        Ok(()) => Report::done("The engine is healthy.\n".to_owned()),
        Err(error) => Report::not_done(format!("The engine is not healthy: {error}.\n")),
    }
}

/// `admin engine version`.
#[must_use]
pub fn engine_version(version: &EngineVersion) -> Report {
    let tested = if version.is_tested() {
        "a version the hub was checked against"
    } else {
        "NOT a version the hub was checked against: run the contract test by hand before \
         using it (referencia/waha-fatos.md)"
    };
    Report::done(format!(
        "The engine is {} {} {}: {tested}.\n",
        escape(&version.version),
        escape(&version.engine),
        escape(&version.tier)
    ))
}

fn engine_said(result: &Result<(), EngineError>) -> String {
    match result {
        Ok(()) => String::new(),
        Err(error) => format!(
            " The engine did not do its part ({error}); the hub tries again at its next check.\n"
        ),
    }
}

/// `admin number add`.
#[must_use]
pub fn number_added(added: &Added) -> Report {
    let engine = match &added.engine {
        Ok(Ensured::Created) => "Its session was created in the engine, already configured.",
        Ok(_) => "Its session in the engine is configured.",
        Err(_) => "The engine did not answer; the hub creates the session at its next check.",
    };
    Report::done(format!(
        "Number {} added, unpaired. {engine} Pair it with `arauto-hub admin number pair {}`.\n",
        added.number.alias, added.number.alias
    ))
}

/// `admin number list`. `phones` holds the telephone of each number, in the same order.
#[must_use]
pub fn number_list(numbers: &[(Number, Option<Phone>)]) -> Report {
    if numbers.is_empty() {
        return Report::done("No numbers.\n".to_owned());
    }
    let mut text = String::new();
    for (number, phone) in numbers {
        let phone = phone
            .as_ref()
            .map_or_else(|| "?".to_owned(), |phone| format!("+{}", phone.digits()));
        let _ = writeln!(
            text,
            "{:<32}  {:<16}  {:<15}  since {}{}",
            number.alias,
            phone,
            number.state.as_str(),
            time(number.state_since),
            if number.command {
                "  (command channel)"
            } else {
                ""
            }
        );
    }
    Report::done(text)
}

/// What a pairing shows while it goes: the QR, or the code.
#[must_use]
pub fn shown(shown: &Shown<'_>) -> String {
    match shown {
        Shown::Qr(qr) => match QrCode::encode(qr.expose().as_bytes()) {
            Ok(code) => format!(
                "\nScan with the telephone: WhatsApp > Linked devices > Link a device.\n{}",
                code.to_terminal()
            ),
            Err(_) => "\nThe QR is too long to draw: use `--code` instead.\n".to_owned(),
        },
        Shown::Code(code) => format!(
            "\nOn the telephone: WhatsApp > Linked devices > Link with phone number, and type \
             this code:\n\n    {}\n\n",
            code.expose()
        ),
    }
}

/// How `admin number pair` ended.
#[must_use]
pub fn paired(alias: &str, outcome: &PairOutcome) -> Report {
    match outcome {
        PairOutcome::Connected => Report::done(format!("Number {alias} is connected.\n")),
        PairOutcome::Swapped => Report::not_done(format!(
            "Number {alias} paired with ANOTHER telephone than the one that was added. Its \
             accesses wait: run `arauto-hub admin number confirm {alias}` to accept it, or \
             `arauto-hub admin number unpair {alias} --erase-engine-history` to undo it.\n"
        )),
        PairOutcome::Expired => Report::not_done(format!(
            "Nothing was scanned or typed in time: number {alias} is unpaired again.\n"
        )),
        PairOutcome::Failed => Report::not_done(format!(
            "The engine reported a failure of the session: number {alias} is unpaired again.\n"
        )),
    }
}

/// A change of a number that also went to the engine.
#[must_use]
pub fn number_changed(what: &str, changed: &Changed) -> Report {
    Report::done(format!(
        "Number {} {what}; it is {} now.{}\n",
        changed.number.alias,
        changed.number.state.as_str(),
        engine_said(&changed.engine)
    ))
}

/// `admin number confirm`.
#[must_use]
pub fn number_confirmed(number: &Number) -> Report {
    Report::done(format!(
        "Number {} is connected with its new telephone, and its accesses work again.\n",
        number.alias
    ))
}
