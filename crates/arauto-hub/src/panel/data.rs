//! Reading what the pages show.
//!
//! Every function here only reads: it calls the same repository functions the admin command
//! calls and turns their rows into the plain values of [`super::view`]. What a page says
//! about a Hermes, a number or an access is what `arauto-hub admin` would say, plus the one
//! thing only the running hub knows: which devices are in a session of the channel now.

use std::collections::{BTreeSet, HashMap};

use uuid::Uuid;

use arauto_protocol::names::HermesName;

use super::text::{self, Lang, k};

use super::guide::{Ended, Facts, LastUse, Screen};
use super::view::{
    AccessPage, AccessRow, Cell, EngineLine, Frame, HermesPage, HermesRow, LineAction, Notice,
    NumberRow, NumbersPage, OrderTag, RowAction, StatusKind, StatusLine, Tone,
};
use crate::channel::Hub;
use crate::core::access::{Scope, Verbs};
use crate::core::model::{Device, DeviceState, StateReason};
use crate::core::number::NumberState;
use crate::core::reconnect::{Served, Standing, connector_standing, read_declared};
use crate::db::ResolvedOrg;
use crate::engine::Phone;
use crate::repo::access::Listed;
use crate::repo::alert::{Alert, AlertFilter, AlertKind};
use crate::repo::hermes::HermesLine;
use crate::repo::orders::OrdersRecord;
use crate::repo::{self, RepoError};
use crate::restore;

/// How many alerts the band shows; the rest are counted.
const NOTICES_SHOWN: usize = 5;

/// How many alerts and unknown devices are read to be counted.
const READ_MAX: u32 = 100;

/// How long ago something happened, in words.
pub(super) fn ago(lang: Lang, now: i64, at: Option<i64>) -> String {
    let Some(at) = at else {
        return text::tr(lang, k("never")).to_owned();
    };
    let seconds = now.saturating_sub(at).max(0) / 1_000;
    let (key, amount) = if seconds < 60 {
        return text::tr(lang, k("now")).to_owned();
    } else if seconds < 3_600 {
        (k("{n} min ago"), seconds / 60)
    } else if seconds < 48 * 3_600 {
        (k("{n} h ago"), seconds / 3_600)
    } else {
        (k("{n} d ago"), seconds / 86_400)
    };
    text::fill(lang, key, &[("n", &amount.to_string())])
}

/// Why the hub stopped trusting a credential, by the reason the database keeps: a sentence
/// of the screens (`docs/07-telas.md`).
fn reason(stored: Option<&str>) -> &'static str {
    match stored {
        Some("reuse") => k("a credential that had already been swapped was presented"),
        Some("double_connection") => k("two connections at the same time"),
        Some("unreadable_credential") => k("the credential could not be read"),
        Some("not_recognized") => k("the last use was not recognized on the reconnect"),
        Some("double_waiting") => k("the identity was copied before the approval"),
        _ => k("the hub no longer trusts the credential"),
    }
}

/// The device that stopped a Hermes, if one did: the one in use, or a request that went to
/// quarantine before its first credential.
fn stopped(line: &HermesLine) -> Option<&Device> {
    let halted = |device: &&Device| {
        matches!(
            device.state,
            DeviceState::Quarantined | DeviceState::Suspended
        )
    };
    line.in_use
        .as_ref()
        .filter(halted)
        .or_else(|| line.request.as_ref().filter(halted))
}

/// The label of a line of Hermes (`docs/10-maquinas-de-estado.md`, section 3), in the order
/// the admin command decides it, with the one thing the command cannot know: the session.
pub(super) fn standing(line: &HermesLine, in_session: bool) -> (Tone, &'static str) {
    let request = line.request.as_ref().map(|device| device.state);
    let in_use = line.in_use.as_ref().map(|device| device.state);
    if request == Some(DeviceState::Pending) {
        (Tone::Wait, k("Waiting for your code"))
    } else if request == Some(DeviceState::Approved) {
        (Tone::Wait, k("Approved, waiting for the connector"))
    } else if line.invite_expires_at.is_some() {
        (Tone::Wait, k("Waiting for the Hermes"))
    } else if in_use == Some(DeviceState::Active) {
        if in_session {
            (Tone::Ok, k("Connected"))
        } else {
            (Tone::Idle, k("Disconnected"))
        }
    } else if in_use == Some(DeviceState::Quarantined) || request == Some(DeviceState::Quarantined)
    {
        (Tone::Bad, k("In quarantine"))
    } else if in_use == Some(DeviceState::Suspended) {
        (Tone::Bad, k("Suspended"))
    } else if let Some(last) = line
        .newest_final
        .as_ref()
        .filter(|device| device.first_confirmed_at.is_some())
    {
        // A line that was connected before and has no device now says how it ended.
        match (last.state, last.state_reason) {
            (DeviceState::Revoked, Some(StateReason::Left)) => (Tone::Idle, k("Left")),
            (DeviceState::Revoked, _) => (Tone::Idle, k("Revoked")),
            (DeviceState::Expired, _) => (Tone::Idle, k("Expired")),
            _ => (Tone::Idle, k("No device")),
        }
    } else {
        (Tone::Idle, k("No device"))
    }
}

/// What the column of the connector shows: the numbers of a declared version that parses, for
/// a connector the hub compares; the declared text (escaped by the page) otherwise.
fn connector_shown(declared: &str, standing: Option<Standing>) -> String {
    match standing {
        Some(Standing::Current | Standing::Outdated | Standing::OtherBuild) => {
            read_declared(declared)
                .core
                .map_or_else(|| declared.to_owned(), |core| core.to_string())
        }
        _ => declared.to_owned(),
    }
}

/// The line under a row whose connector is not what the hub serves, when there is one to say.
fn connector_explain(
    lang: Lang,
    name: &str,
    declared: Option<&str>,
    standing: Standing,
    served: &Served,
) -> Option<String> {
    match standing {
        Standing::RestartPending => Some(text::fill(
            lang,
            k(
                "{name} has the new connector installed, but its gateway has not restarted since: it still runs the older one. Ask its operator to restart the gateway once. A reconnect is not needed.",
            ),
            &[("name", name)],
        )),
        Standing::OtherBuild => Some(text::fill(
            lang,
            k("{name} declares a connector that is not one this hub serves."),
            &[("name", name)],
        )),
        Standing::Outdated => {
            let bundle = served.bundle()?;
            let old = read_declared(declared.unwrap_or_default())
                .core
                .map(|core| format!(" ({core})"))
                .unwrap_or_default();
            Some(text::fill(
                lang,
                k(
                    "{name} runs an older connector{old}; this hub serves {new}. Reconnect it to update. Its Arauto tools keep working until its gateway restarts, and the new connector runs from that restart.",
                ),
                &[("name", name), ("old", &old), ("new", &bundle.to_string())],
            ))
        }
        Standing::Current | Standing::NotKnown => None,
    }
}

/// The sentence of an alert. The name of the Hermes and the address in it are values: the
/// page escapes the whole sentence.
fn notice(lang: Lang, alert: &Alert) -> Notice {
    let unnamed = text::tr(lang, k("a Hermes"));
    let hermes = alert.hermes_name.as_deref().unwrap_or(unnamed);
    let named = |key: &'static str| text::fill(lang, key, &[("hermes", hermes)]);
    let (grave, lead, mut said) = match alert.kind {
        AlertKind::InviteMisused => (
            false,
            k("Invite"),
            named(k("another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.")),
        ),
        AlertKind::IdentityCopied => (
            true,
            k("Identity copied"),
            named(k("the identity of {hermes} was copied before the approval. The request was denied.")),
        ),
        AlertKind::Quarantined => {
            let why = reason(alert.details.get("reason").and_then(serde_json::Value::as_str));
            (
                true,
                k("Hermes in quarantine"),
                text::fill(
                    lang,
                    k("{hermes}. Reason: {reason}."),
                    &[("hermes", hermes), ("reason", text::tr(lang, why))],
                ),
            )
        }
        AlertKind::CopySuspected => (
            true,
            k("Suspected copy"),
            named(k("the last use of {hermes} was not recognized on the reconnect.")),
        ),
        AlertKind::ConnectionTaken => (
            true,
            k("Connection taken"),
            named(k("the connection of {hermes} was taken by another one.")),
        ),
        AlertKind::DeviceLeft => (
            false,
            k("Closing"),
            named(k("{hermes} asked for its own closing.")),
        ),
        AlertKind::ReuseAfterClosing => (
            true,
            k("Reuse"),
            named(k("a copy of {hermes} was used before the closing.")),
        ),
        AlertKind::UnconfirmedSwaps => (
            false,
            k("Unconfirmed swaps"),
            named(k("{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.")),
        ),
        AlertKind::MarkNotAdvanced => (
            false,
            k("Audit mark"),
            text::tr(lang, k("a security event was written and the mark outside the database did not follow.")).to_owned(),
        ),
        AlertKind::EngineVersionUntested => (
            false,
            k("Engine"),
            text::tr(lang, k("the version of the engine is not one the hub has checked.")).to_owned(),
        ),
        AlertKind::EngineSessionCorrected => (
            false,
            k("Engine"),
            text::tr(lang, k("an engine session had another configuration, and the hub applied its own.")).to_owned(),
        ),
        AlertKind::EngineKeyRefused => {
            // An origin that is not the installation reads as an engine an admin set up: the
            // one for which setting the key again is the right advice.
            let brought = alert.details.get("origin").and_then(serde_json::Value::as_str)
                == Some("installation");
            let said = if brought {
                text::tr(
                    lang,
                    k("the engine refused the key of the hub. Restart the engine: it reads its key only when it starts."),
                )
                .to_owned()
            } else {
                text::fill(
                    lang,
                    k("the engine refused the key of the hub. Set its key again with {command}."),
                    &[("command", super::numbers::KEY_SET)],
                )
            };
            (true, k("Engine"), said)
        }
        AlertKind::NumberSwapped => (
            true,
            k("Number swapped"),
            text::tr(lang, k("the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.")).to_owned(),
        ),
    };
    // Where it came from is a sentence of its own: no language has to fit it inside another.
    if let Some(address) = alert
        .details
        .get("from")
        .and_then(serde_json::Value::as_str)
    {
        said.push(' ');
        said.push_str(&text::fill(
            lang,
            k("Address: {address}."),
            &[("address", address)],
        ));
    }
    Notice {
        id: alert.id,
        grave,
        lead,
        text: said,
    }
}

/// What every page shows around its content: the restored mode and the alerts.
pub(super) async fn frame(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
) -> Result<Frame, RepoError> {
    let restored = restore::installation(hub.pool())
        .await?
        .filter(|installation| installation.restored_mode)
        .map(|installation| installation.restored_copy_of);
    let alerts = repo::alert::list(hub.pool(), organization, AlertFilter::Unseen, READ_MAX).await?;
    let unknown_devices = repo::device::unknown_listed(hub.pool(), organization, READ_MAX)
        .await?
        .len();
    Ok(Frame {
        lang,
        restored,
        more_notices: alerts.len().saturating_sub(NOTICES_SHOWN),
        notices: alerts
            .iter()
            .take(NOTICES_SHOWN)
            .map(|alert| notice(lang, alert))
            .collect(),
        unknown_devices,
    })
}

/// The page of the Hermes.
pub(super) async fn hermes(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
) -> Result<HermesPage, RepoError> {
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    let accesses = repo::access::list(hub.pool(), organization, None).await?;
    let sessions = hub.sessions_by_device();
    let now = hub.now_ms();
    let key = hub.organization_key(organization).await?;
    let mut page = HermesPage::default();
    for line in &lines {
        let in_session = line
            .in_use
            .as_ref()
            .is_some_and(|device| sessions.contains_key(&device.id));
        let (tone, state) = standing(line, in_session);
        match tone {
            Tone::Ok => page.connected = page.connected.saturating_add(1),
            Tone::Wait => page.waiting = page.waiting.saturating_add(1),
            Tone::Bad => page.stopped = page.stopped.saturating_add(1),
            Tone::Idle => {}
        }
        let name = line.hermes.name.as_str();
        let standing = line
            .in_use
            .as_ref()
            .filter(|device| device.state == DeviceState::Active)
            .map(|device| connector_standing(device, hub.served()));
        if standing == Some(Standing::Outdated) {
            page.outdated = page.outdated.saturating_add(1);
        }
        let explain = stopped(line).filter(|_| tone == Tone::Bad).map(|device| {
            if device.state == DeviceState::Suspended {
                text::fill(
                    lang,
                    k("The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\"."),
                    &[("name", name)],
                )
            } else {
                let why = reason(device.state_reason.map(|reason| reason.as_str()));
                text::fill(
                    lang,
                    k("The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\"."),
                    &[("name", name), ("reason", text::tr(lang, why))],
                )
            }
        });
        let explain = explain.or_else(|| {
            standing.and_then(|standing| {
                connector_explain(
                    lang,
                    name,
                    line.in_use
                        .as_ref()
                        .and_then(|device| device.connector_version.as_deref()),
                    standing,
                    hub.served(),
                )
            })
        });
        let device = line.in_use.as_ref().or(line.request.as_ref());
        let record = repo::orders::get(hub.pool(), organization, &key, line.hermes.id).await?;
        let (orders, orders_recorded) = orders_summary(lang, &record);
        page.rows.push(HermesRow {
            name: name.to_owned(),
            tone,
            state,
            numbers: accesses
                .iter()
                .filter(|access| access.hermes.as_str() == name)
                .map(|access| access.number.as_str().to_owned())
                .collect(),
            connector: device
                .and_then(|device| device.connector_version.as_deref())
                .map(|declared| connector_shown(declared, standing)),
            standing,
            seen: if in_session {
                text::tr(lang, k("now")).to_owned()
            } else {
                ago(
                    lang,
                    now,
                    line.in_use
                        .as_ref()
                        .and_then(|device| device.last_contact_at),
                )
            },
            explain,
            orders,
            orders_recorded,
            action: if line.request.is_some() || line.invite_expires_at.is_some() {
                RowAction::Continue
            } else if line.in_use.is_some() {
                RowAction::Reconnect
            } else if line
                .newest_final
                .as_ref()
                .is_some_and(|device| device.first_confirmed_at.is_some())
            {
                RowAction::Return
            } else {
                RowAction::Connect
            },
        });
    }
    Ok(page)
}

/// What the window of the actions of a line says of the invite it would issue: the reason as
/// the write would store it now. `None` when there is no such Hermes.
pub(super) async fn preview(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    name: &HermesName,
    served: &Served,
) -> Result<Option<super::actions::Preview>, RepoError> {
    let now = hub.now_ms();
    let reason = repo::invite::reason_preview(hub.pool(), organization, name, served, now).await?;
    Ok(reason.map(|reason| super::actions::Preview {
        seen: reason.seen(),
        body: super::block::why(lang, &reason),
    }))
}

/// How the orders of a Hermes read in a list, and whether any are recorded.
pub(super) fn orders_summary(lang: Lang, record: &OrdersRecord) -> (String, bool) {
    let summary = if record.outside_whatsapp {
        text::tr(lang, k("outside WhatsApp")).to_owned()
    } else if record.self_chat {
        text::tr(lang, k("its own chat")).to_owned()
    } else if record.orders_from.len() == 1 {
        text::tr(lang, k("1 telephone")).to_owned()
    } else if record.orders_from.is_empty() {
        text::tr(lang, k("not recorded")).to_owned()
    } else {
        text::fill(
            lang,
            k("{n} telephones"),
            &[("n", &record.orders_from.len().to_string())],
        )
    };
    (
        summary,
        record.self_chat || record.outside_whatsapp || !record.orders_from.is_empty(),
    )
}

/// What the screen of a Hermes was drawn from, so that a form that issues an invite can say
/// what it replaces: `request-{id}` when a request is in progress, `invite-{ms}` when an
/// invite is still accepted, else `none`.
pub(super) fn replaces(line: &HermesLine, now: i64) -> String {
    if let Some(request) = &line.request {
        format!("request-{}", request.id.hyphenated())
    } else if let Some(until) = line.invite_expires_at.filter(|until| *until > now) {
        format!("invite-{until}")
    } else {
        "none".to_owned()
    }
}

/// [`replaces`] of the Hermes `name` now. `None` when there is no such Hermes.
pub(super) async fn replaces_now(
    hub: &Hub,
    organization: ResolvedOrg,
    name: &HermesName,
) -> Result<Option<String>, RepoError> {
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    let now = hub.now_ms();
    Ok(lines
        .iter()
        .find(|line| line.hermes.name == *name)
        .map(|line| replaces(line, now)))
}

/// What a write of the orders of a Hermes did, for the line above a list. Only reads.
pub(super) async fn orders_said(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    named: Option<&str>,
) -> Result<Option<StatusLine>, RepoError> {
    let Some(name) = named.and_then(|named| HermesName::parse(named.trim()).ok()) else {
        return Ok(None);
    };
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    let Some(line) = lines.iter().find(|line| line.hermes.name == name) else {
        return Ok(None);
    };
    let key = hub.organization_key(organization).await?;
    let record = repo::orders::get(hub.pool(), organization, &key, line.hermes.id).await?;
    if record.outside_whatsapp {
        return Ok(Some(StatusLine {
            kind: StatusKind::Done(Tone::Ok),
            sentence: k(
                "Saved. {hermes} takes its orders outside WhatsApp: Arauto refuses nothing for it.",
            ),
            texts: vec![("hermes", name.as_str().to_owned())],
            codes: Vec::new(),
        }));
    }
    let (orders, _) = orders_summary(lang, &record);
    Ok(Some(StatusLine {
        kind: StatusKind::Done(Tone::Ok),
        sentence: k("Saved. Who gives {hermes} orders on WhatsApp: {orders}."),
        texts: vec![("hermes", name.as_str().to_owned()), ("orders", orders)],
        codes: Vec::new(),
    }))
}

/// Where the guide of the Hermes `name` is, by what the hub observed so far. `None` when
/// there is no Hermes with that name. Only reads: asking again changes nothing.
pub(super) async fn guide(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    name: &HermesName,
) -> Result<Option<Screen>, RepoError> {
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    let Some(line) = lines.iter().find(|line| line.hermes.name == *name) else {
        return Ok(None);
    };
    let now = hub.now_ms();
    let left =
        |until: Option<i64>| until.map_or(0, |until| until.saturating_sub(now).max(0) / 1_000);
    let name = name.as_str().to_owned();
    let reconnect = line.in_use.is_some();
    let view = repo::approval::request_of(hub.pool(), organization, &line.hermes.name).await?;
    if let Some(view) = view {
        let request = &view.request;
        match request.state {
            DeviceState::Pending => {
                return Ok(Some(Screen::Code {
                    name,
                    facts: Box::new(Facts {
                        address: request.enrolled_from.clone(),
                        when: ago(lang, now, Some(request.state_since)),
                        hostname: request.declared.hostname.as_str().to_owned(),
                        os: request.declared.os.as_str().to_owned(),
                        hermes: request.declared.hermes_version.as_str().to_owned(),
                    }),
                    last_use: view.replaces.as_ref().map(|last| {
                        Box::new(LastUse {
                            device: last.device,
                            at: last.at,
                            state: last.state.as_str(),
                            ago: ago(lang, now, last.at),
                            address: last.address.clone(),
                        })
                    }),
                    error: None,
                    seconds: left(request.confirm_until),
                }));
            }
            DeviceState::Approved => {
                return Ok(Some(Screen::Done {
                    name,
                    connected: false,
                    reconnect,
                }));
            }
            DeviceState::Quarantined => {
                return Ok(Some(Screen::Ended {
                    name,
                    why: Ended::Copied,
                    replaces: replaces(line, now),
                }));
            }
            _ => {}
        }
    }
    // A deadline passed when the time of the hub reached it.
    if line.invite_expires_at.is_some_and(|until| until > now) {
        return Ok(Some(Screen::Waiting {
            name,
            seconds: left(line.invite_expires_at),
            reconnect,
            replaces: replaces(line, now),
        }));
    }
    let connected = line
        .in_use
        .as_ref()
        .is_some_and(|device| device.state == DeviceState::Active);
    Ok(Some(if connected {
        Screen::Done {
            name,
            connected: true,
            reconnect: false,
        }
    } else {
        Screen::Ended {
            name,
            why: Ended::Nothing,
            replaces: replaces(line, now),
        }
    }))
}

pub(super) fn number_state(state: NumberState) -> (Tone, &'static str) {
    match state {
        NumberState::Unpaired => (Tone::Idle, k("Not paired")),
        NumberState::Pairing => (Tone::Wait, k("Pairing")),
        NumberState::Connected => (Tone::Ok, k("Connected")),
        NumberState::Reconnecting => (Tone::Wait, k("Reconnecting")),
        NumberState::Disconnected => (Tone::Idle, k("Disconnected")),
        NumberState::NumberSwapped => (Tone::Bad, k("Number swapped")),
        NumberState::Failed => (Tone::Bad, k("Failed")),
        NumberState::Removed => (Tone::Idle, k("Removed")),
    }
}

/// The telephones of the numbers, by alias. A telephone that does not open is left out: the
/// page shows a dash, and the rest of the page is still true.
async fn phones(
    hub: &Hub,
    organization: ResolvedOrg,
    numbers: &[repo::number::Number],
) -> HashMap<String, String> {
    if numbers.is_empty() {
        return HashMap::new();
    }
    let Ok(key) = hub.organization_key(organization).await else {
        return HashMap::new();
    };
    numbers
        .iter()
        .filter_map(|number| {
            let phone = number.phone(&key).ok()?;
            Some((
                number.alias.as_str().to_owned(),
                format!("+{}", phone.digits()),
            ))
        })
        .collect()
}

/// The telephone of one number as the list shows it (`+digits`), when the key of the
/// organization opens it.
pub(super) async fn phone_of(
    hub: &Hub,
    organization: ResolvedOrg,
    number: &repo::number::Number,
) -> Option<String> {
    let key = hub.organization_key(organization).await.ok()?;
    let phone = number.phone(&key).ok()?;
    Some(format!("+{}", phone.digits()))
}

/// The telephone that paired a swapped number (`+digits`), when the key of the organization
/// opens it.
pub(super) async fn paired_phone_of(
    hub: &Hub,
    organization: ResolvedOrg,
    number: &repo::number::Number,
) -> Option<String> {
    let key = hub.organization_key(organization).await.ok()?;
    let phone = repo::number::paired_phone(hub.pool(), organization, &key, number.id)
        .await
        .ok()??;
    Some(format!("+{}", phone.digits()))
}

/// A number of the hub the orders window offers as a choice.
pub(super) struct Pick {
    /// The alias of the number.
    pub alias: String,
    /// Its telephone, opened with the key of the organization.
    pub phone: Phone,
    /// `+digits`, as the WhatsApps tab shows it.
    pub shown: String,
    /// The Hermes whose own WhatsApp runs on it.
    pub takes_for: Option<String>,
    /// The Hermes it gives orders to.
    pub gives_to: Vec<String>,
}

/// The numbers of the hub, not removed, by alias, whose telephone the key opens.
pub(super) async fn picks(hub: &Hub, organization: ResolvedOrg) -> Result<Vec<Pick>, RepoError> {
    let numbers = repo::number::list(hub.pool(), organization, false).await?;
    let Ok(key) = hub.organization_key(organization).await else {
        return Ok(Vec::new());
    };
    let tags = repo::orders::tags(hub.pool(), organization).await?;
    Ok(numbers
        .iter()
        .filter(|number| number.state != NumberState::Removed)
        .filter_map(|number| {
            let phone = number.phone(&key).ok()?;
            let mut takes_for = None;
            let mut gives_to = Vec::new();
            for tag in tags.iter().filter(|tag| tag.number == number.id) {
                if tag.takes_orders_here {
                    takes_for = Some(tag.hermes.as_str().to_owned());
                } else {
                    gives_to.push(tag.hermes.as_str().to_owned());
                }
            }
            Some(Pick {
                alias: number.alias.as_str().to_owned(),
                shown: format!("+{}", phone.digits()),
                phone,
                takes_for,
                gives_to,
            })
        })
        .collect())
}

/// What the line of a number offers: by its state, and for a pairing, by who runs it.
fn line_actions(
    hub: &Hub,
    organization: ResolvedOrg,
    session: Uuid,
    number: &repo::number::Number,
) -> Vec<LineAction> {
    let mut actions = match number.state {
        NumberState::Unpaired => vec![LineAction::Pair],
        NumberState::Pairing => {
            let run = hub
                .panel()
                .and_then(|panel| panel.pairings.of(organization, number.id))
                .filter(|run| run.live());
            match run {
                Some(run) if run.session == session => {
                    vec![LineAction::Continue(run.id.to_string())]
                }
                Some(_) => vec![LineAction::SeePairing],
                None => vec![LineAction::PairAgain],
            }
        }
        NumberState::Connected | NumberState::Reconnecting => vec![LineAction::Disconnect],
        NumberState::Disconnected => vec![LineAction::PairAgain],
        NumberState::NumberSwapped => vec![LineAction::Decide],
        NumberState::Failed => vec![LineAction::Restart],
        NumberState::Removed => return Vec::new(),
    };
    actions.push(LineAction::Orders);
    actions.push(LineAction::Remove);
    actions
}

/// The page of the numbers, for the panel session `session`.
pub(super) async fn numbers(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    session: Uuid,
) -> Result<NumbersPage, RepoError> {
    let numbers = repo::number::list(hub.pool(), organization, false).await?;
    let accesses = repo::access::list(hub.pool(), organization, None).await?;
    let mut phones = phones(hub, organization, &numbers).await;
    let tags = repo::orders::tags(hub.pool(), organization).await?;
    let now = hub.now_ms();
    let engine = repo::engine::get(hub.pool(), organization)
        .await?
        .map(|engine| {
            let version = match (&engine.version_seen, engine.version_tested) {
                (Some(version), Some(true)) => text::fill(
                    lang,
                    k("version {version}, checked"),
                    &[("version", version)],
                ),
                (Some(version), _) => text::fill(
                    lang,
                    k("version {version}, not checked by the hub"),
                    &[("version", version)],
                ),
                (None, _) => text::tr(lang, k("version not read yet")).to_owned(),
            };
            let answered = match engine.last_ok_at {
                Some(at) => text::fill(
                    lang,
                    k("last answer {when}"),
                    &[("when", &ago(lang, now, Some(at)))],
                ),
                None => text::tr(lang, k("has not answered yet")).to_owned(),
            };
            EngineLine {
                tone: if engine.version_tested == Some(true) && engine.last_ok_at.is_some() {
                    Tone::Ok
                } else {
                    Tone::Wait
                },
                text: format!("{version} · {answered}"),
            }
        });
    let rows = numbers
        .iter()
        .map(|number| {
            let alias = number.alias.as_str();
            let (tone, state) = number_state(number.state);
            NumberRow {
                alias: alias.to_owned(),
                phone: phones.remove(alias),
                tone,
                state,
                hermes: accesses
                    .iter()
                    .filter(|access| access.number.as_str() == alias)
                    .map(|access| access.hermes.as_str().to_owned())
                    .collect(),
                last_message: ago(lang, now, number.last_message_at),
                actions: line_actions(hub, organization, session, number),
                tags: tags
                    .iter()
                    .filter(|tag| tag.number == number.id)
                    .map(|tag| OrderTag {
                        hermes: tag.hermes.as_str().to_owned(),
                        takes_orders_here: tag.takes_orders_here,
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(NumbersPage {
        rows,
        engine,
        said: None,
    })
}

/// What a cell of the matrix says: the verbs that reach some chat, and which chats.
fn cell(lang: Lang, access: &Listed) -> Cell {
    let lines = access.lines.len().to_string();
    let (verbs, scope) = match access.grant.scope {
        Scope::All => (
            access.grant.verbs,
            if access.lines.is_empty() {
                text::tr(lang, k("all")).to_owned()
            } else {
                text::fill(lang, k("all · own rules: {n}"), &[("n", &lines)])
            },
        ),
        // With chosen chats, only the chats with a line exist: the verbs are theirs.
        Scope::List => {
            let mut verbs = Verbs::default();
            for (_, line) in &access.lines {
                verbs.read |= line.read;
                verbs.send |= line.send;
                verbs.modify |= line.modify;
                verbs.wake |= line.wake;
            }
            (
                verbs,
                text::fill(lang, k("chosen chats: {n}"), &[("n", &lines)]),
            )
        }
    };
    Cell {
        verbs: [verbs.read, verbs.send, verbs.modify, verbs.wake],
        scope,
    }
}

/// Where a Hermes is woken, or sends, on one number.
struct Reach {
    everywhere: bool,
    chats: BTreeSet<Uuid>,
}

impl Reach {
    fn of(access: &Listed, has: fn(&Verbs) -> bool) -> Self {
        Self {
            everywhere: access.grant.scope == Scope::All && has(&access.grant.verbs),
            chats: access
                .lines
                .iter()
                .filter(|(_, verbs)| has(verbs))
                .map(|(chat, _)| *chat)
                .collect(),
        }
    }

    fn reaches_something(&self) -> bool {
        self.everywhere || !self.chats.is_empty()
    }

    fn meets(&self, other: &Self) -> bool {
        (self.everywhere && other.reaches_something())
            || (other.everywhere && self.reaches_something())
            || !self.chats.is_disjoint(&other.chats)
    }
}

/// How many Hermes do the same thing in the same chats of one number.
fn together(accesses: &[&Listed], has: fn(&Verbs) -> bool) -> usize {
    let reaches: Vec<Reach> = accesses
        .iter()
        .map(|access| Reach::of(access, has))
        .collect();
    reaches
        .iter()
        .enumerate()
        .filter(|(index, reach)| {
            reaches
                .iter()
                .enumerate()
                .any(|(other, reached)| other != *index && reach.meets(reached))
        })
        .count()
}

/// The page of the accesses.
pub(super) async fn accesses(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
) -> Result<AccessPage, RepoError> {
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    let numbers = repo::number::list(hub.pool(), organization, false).await?;
    let listed = repo::access::list(hub.pool(), organization, None).await?;
    let mut phones = phones(hub, organization, &numbers).await;

    let halted: HashMap<&str, &'static str> = lines
        .iter()
        .filter_map(|line| {
            let device = stopped(line)?;
            let word = if device.state == DeviceState::Suspended {
                k("Suspended")
            } else {
                k("In quarantine")
            };
            Some((line.hermes.name.as_str(), word))
        })
        .collect();

    let rows = lines
        .iter()
        .map(|line| {
            let name = line.hermes.name.as_str();
            let stopped = halted.get(name).copied();
            AccessRow {
                hermes: name.to_owned(),
                stopped,
                cells: if stopped.is_some() {
                    Vec::new()
                } else {
                    numbers
                        .iter()
                        .map(|number| {
                            listed
                                .iter()
                                .find(|access| {
                                    access.hermes.as_str() == name
                                        && access.number.as_str() == number.alias.as_str()
                                })
                                .map(|access| cell(lang, access))
                        })
                        .collect()
                },
            }
        })
        .collect();

    let mut warnings = Vec::new();
    for number in &numbers {
        let alias = number.alias.as_str();
        let here: Vec<&Listed> = listed
            .iter()
            .filter(|access| {
                access.number.as_str() == alias && !halted.contains_key(access.hermes.as_str())
            })
            .collect();
        let woken = together(&here, |verbs| verbs.wake);
        if woken > 1 {
            warnings.push(text::fill(
                lang,
                k("{alias}: {n} Hermes are woken in the same chats."),
                &[("alias", alias), ("n", &woken.to_string())],
            ));
        }
        let sending = together(&here, |verbs| verbs.send);
        if sending > 1 {
            warnings.push(text::fill(
                lang,
                k("{alias}: {n} Hermes send in the same chats."),
                &[("alias", alias), ("n", &sending.to_string())],
            ));
        }
    }

    Ok(AccessPage {
        numbers: numbers
            .iter()
            .map(|number| {
                let alias = number.alias.as_str();
                (alias.to_owned(), phones.remove(alias))
            })
            .collect(),
        rows,
        warnings,
        saved: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn how_long_ago_is_said_in_the_largest_unit_that_fits() {
        let now = 1_800_000_000_000;
        assert_eq!(ago(Lang::En, now, None), "never");
        assert_eq!(ago(Lang::En, now, Some(now)), "now");
        assert_eq!(ago(Lang::En, now, Some(now - 59_000)), "now");
        assert_eq!(ago(Lang::En, now, Some(now - 60_000)), "1 min ago");
        assert_eq!(ago(Lang::En, now, Some(now - 59 * 60_000)), "59 min ago");
        assert_eq!(ago(Lang::En, now, Some(now - 3_600_000)), "1 h ago");
        assert_eq!(ago(Lang::En, now, Some(now - 47 * 3_600_000)), "47 h ago");
        assert_eq!(ago(Lang::En, now, Some(now - 48 * 3_600_000)), "2 d ago");
        // A clock that went back does not print a negative time.
        assert_eq!(ago(Lang::En, now, Some(now + 5_000)), "now");
    }

    #[test]
    fn two_reaches_meet_when_one_is_everywhere_or_they_share_a_chat() {
        let chat = |n: u128| Uuid::from_u128(n);
        let everywhere = Reach {
            everywhere: true,
            chats: BTreeSet::new(),
        };
        let nowhere = Reach {
            everywhere: false,
            chats: BTreeSet::new(),
        };
        let one = Reach {
            everywhere: false,
            chats: [chat(1), chat(2)].into(),
        };
        let other = Reach {
            everywhere: false,
            chats: [chat(2), chat(3)].into(),
        };
        let apart = Reach {
            everywhere: false,
            chats: [chat(9)].into(),
        };
        assert!(everywhere.meets(&everywhere));
        assert!(everywhere.meets(&one) && one.meets(&everywhere));
        assert!(one.meets(&other) && other.meets(&one));
        assert!(!one.meets(&apart));
        // A Hermes that does the thing nowhere meets no one, even one that does it everywhere.
        assert!(!everywhere.meets(&nowhere) && !nowhere.meets(&everywhere));
        assert!(!nowhere.meets(&nowhere));
    }

    fn line_with(newest_final: Option<Device>) -> HermesLine {
        use crate::core::model::{Hermes, HermesMode};
        HermesLine {
            hermes: Hermes {
                id: Uuid::nil(),
                organization_id: Uuid::nil(),
                name: HermesName::parse("ana").unwrap(),
                mode: HermesMode::Fast,
                created_at: 0,
                deleted_at: None,
            },
            in_use: None,
            request: None,
            invite_expires_at: None,
            newest_final,
        }
    }

    #[test]
    fn a_line_that_was_connected_says_how_it_ended() {
        use crate::core::model::StateReason;
        use crate::core::reconnect::test_device;
        let closed = |state, reason, confirmed: bool| {
            line_with(Some(test_device(
                state,
                Some(reason),
                None,
                confirmed.then_some(5),
                5,
            )))
        };
        let said = |line: &HermesLine| standing(line, false);
        assert_eq!(
            said(&closed(DeviceState::Revoked, StateReason::Admin, true)),
            (Tone::Idle, "Revoked")
        );
        assert_eq!(
            said(&closed(DeviceState::Revoked, StateReason::Left, true)),
            (Tone::Idle, "Left")
        );
        assert_eq!(
            said(&closed(DeviceState::Expired, StateReason::Inactivity, true)),
            (Tone::Idle, "Expired")
        );
        // A request that never connected, or a Hermes with nothing: no device.
        assert_eq!(
            said(&closed(
                DeviceState::Denied,
                StateReason::CodeAttempts,
                false
            )),
            (Tone::Idle, "No device")
        );
        assert_eq!(said(&line_with(None)), (Tone::Idle, "No device"));
    }

    #[test]
    fn an_outdated_connector_is_explained_only_with_a_known_bundle() {
        let nothing = Served::nothing();
        let known = Served::of(
            &[],
            crate::core::reconnect::CoreVersion::parse_core("0.1.1"),
        );
        let say = |served: &Served| {
            connector_explain(Lang::En, "ana", Some("0.1.0"), Standing::Outdated, served)
        };
        assert!(say(&nothing).is_none());
        let text = say(&known).unwrap();
        assert!(text.contains(" (0.1.0)"), "{text}");
        assert!(text.contains("0.1.1"), "{text}");
    }

    #[test]
    fn the_connector_is_explained_only_when_there_is_something_to_say() {
        let nothing = Served::nothing();
        let bundle = crate::core::reconnect::CoreVersion::parse_core("0.1.1");
        let known = Served::of(&[], bundle);
        let say = |standing, declared: Option<&str>, served: &Served| {
            connector_explain(Lang::En, "ana", declared, standing, served)
        };
        // The gateway has not restarted.
        let restart = say(Standing::RestartPending, None, &nothing).unwrap();
        assert!(restart.starts_with("ana has the new connector installed"));
        assert!(restart.contains("A reconnect is not needed."));
        // A connector the hub does not serve.
        assert_eq!(
            say(Standing::OtherBuild, None, &nothing).unwrap(),
            "ana declares a connector that is not one this hub serves."
        );
        // Outdated says something only when the hub knows its bundle.
        assert!(say(Standing::Outdated, Some("0.1.0"), &nothing).is_none());
        let older = say(Standing::Outdated, Some("0.1.0"), &known).unwrap();
        assert!(
            older.starts_with("ana runs an older connector (0.1.0); this hub serves 0.1.1."),
            "{older}"
        );
        let unnumbered = say(Standing::Outdated, Some("nonsense"), &known).unwrap();
        assert!(
            unnumbered.starts_with("ana runs an older connector; this hub serves 0.1.1."),
            "{unnumbered}"
        );
        assert!(say(Standing::Current, None, &known).is_none());
        assert!(say(Standing::NotKnown, None, &known).is_none());
        // What the column shows: the numbers when the hub compared it, the text otherwise.
        let marked = format!("0.1.0+{}", "a".repeat(64));
        assert_eq!(connector_shown(&marked, Some(Standing::Current)), "0.1.0");
        assert_eq!(connector_shown(&marked, Some(Standing::Outdated)), "0.1.0");
        assert_eq!(
            connector_shown("nonsense", Some(Standing::OtherBuild)),
            "nonsense"
        );
        assert_eq!(connector_shown(&marked, Some(Standing::NotKnown)), marked);
        assert_eq!(connector_shown(&marked, None), marked);
    }
}
