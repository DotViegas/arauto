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
    AccessPage, AccessRow, Cell, EngineLine, Frame, HermesPage, HermesRow, Notice, NumberRow,
    NumbersPage, RowAction, Tone,
};
use crate::admin::render;
use crate::channel::Hub;
use crate::core::access::{Scope, Verbs};
use crate::core::model::{Device, DeviceState};
use crate::core::number::NumberState;
use crate::db::ResolvedOrg;
use crate::repo::access::Listed;
use crate::repo::alert::{Alert, AlertFilter, AlertKind};
use crate::repo::hermes::HermesLine;
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
fn standing(line: &HermesLine, in_session: bool) -> (Tone, &'static str) {
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
    } else {
        (Tone::Idle, k("No device"))
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
        .map(|installation| {
            let copy = installation.restored_copy_of.map_or_else(
                || text::tr(lang, k("an unknown date")).to_owned(),
                render::time,
            );
            text::fill(
                lang,
                k("The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses."),
                &[("date", &copy)],
            )
        });
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
        let device = line.in_use.as_ref().or(line.request.as_ref());
        page.rows.push(HermesRow {
            name: name.to_owned(),
            tone,
            state,
            numbers: accesses
                .iter()
                .filter(|access| access.hermes.as_str() == name)
                .map(|access| access.number.as_str().to_owned())
                .collect(),
            woken: line.hermes.can_be_woken,
            connector: device.and_then(|device| device.connector_version.clone()),
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
            action: if line.request.is_some() || line.invite_expires_at.is_some() {
                RowAction::Continue
            } else if line.in_use.is_some() {
                RowAction::Reconnect
            } else {
                RowAction::Connect
            },
        });
    }
    Ok(page)
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
                            when: last.at.map_or_else(
                                || text::tr(lang, k("never")).to_owned(),
                                |at| format!("{} ({})", ago(lang, now, Some(at)), render::time(at)),
                            ),
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
        }
    }))
}

fn number_state(state: NumberState) -> (Tone, &'static str) {
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

/// The page of the numbers.
pub(super) async fn numbers(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
) -> Result<NumbersPage, RepoError> {
    let numbers = repo::number::list(hub.pool(), organization, false).await?;
    let accesses = repo::access::list(hub.pool(), organization, None).await?;
    let mut phones = phones(hub, organization, &numbers).await;
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
                command: number.command,
                hermes: accesses
                    .iter()
                    .filter(|access| access.number.as_str() == alias)
                    .map(|access| access.hermes.as_str().to_owned())
                    .collect(),
                last_message: ago(lang, now, number.last_message_at),
            }
        })
        .collect();
    Ok(NumbersPage { rows, engine })
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
        let sends = here
            .iter()
            .any(|access| Reach::of(access, |verbs| verbs.send).reaches_something());
        if number.command && sends {
            warnings.push(text::fill(
                lang,
                k("{alias} is marked as a command number and there are Hermes allowed to send on it."),
                &[("alias", alias)],
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
}
