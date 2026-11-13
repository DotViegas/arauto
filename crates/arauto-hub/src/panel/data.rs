//! Reading what the pages show.
//!
//! Every function here only reads: it calls the same repository functions the admin command
//! calls and turns their rows into the plain values of [`super::view`]. What a page says
//! about a Hermes, a number or an access is what `arauto-hub admin` would say, plus the one
//! thing only the running hub knows: which devices are in a session of the channel now.

use std::collections::{BTreeSet, HashMap};

use uuid::Uuid;

use arauto_protocol::names::HermesName;

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
pub(super) fn ago(now: i64, at: Option<i64>) -> String {
    let Some(at) = at else {
        return "nunca".to_owned();
    };
    let seconds = now.saturating_sub(at).max(0) / 1_000;
    if seconds < 60 {
        "agora".to_owned()
    } else if seconds < 3_600 {
        format!("há {} min", seconds / 60)
    } else if seconds < 48 * 3_600 {
        format!("há {} h", seconds / 3_600)
    } else {
        format!("há {} dias", seconds / 86_400)
    }
}

/// Why the hub stopped trusting a credential, in the words of `docs/07-telas.md`.
fn reason(device: &Device) -> &'static str {
    match device.state_reason.map(|reason| reason.as_str()) {
        Some("reuse") => "uma credencial já trocada foi apresentada",
        Some("double_connection") => "duas conexões ao mesmo tempo",
        Some("unreadable_credential") => "a credencial não pôde ser lida",
        Some("not_recognized") => "o último uso não foi reconhecido no reconectar",
        Some("double_waiting") => "a identidade foi copiada antes da aprovação",
        _ => "o hub deixou de confiar na credencial",
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
        (Tone::Wait, "Aguardando seu código")
    } else if request == Some(DeviceState::Approved) {
        (Tone::Wait, "Aprovado, aguardando o conector")
    } else if line.invite_expires_at.is_some() {
        (Tone::Wait, "Aguardando o Hermes")
    } else if in_use == Some(DeviceState::Active) {
        if in_session {
            (Tone::Ok, "Conectado")
        } else {
            (Tone::Idle, "Desconectado")
        }
    } else if in_use == Some(DeviceState::Quarantined) || request == Some(DeviceState::Quarantined)
    {
        (Tone::Bad, "Em quarentena")
    } else if in_use == Some(DeviceState::Suspended) {
        (Tone::Bad, "Suspenso")
    } else {
        (Tone::Idle, "Sem aparelho")
    }
}

/// The sentence of an alert. The name of the Hermes and the address in it are values: the
/// page escapes the whole sentence.
fn notice(alert: &Alert) -> Notice {
    let hermes = alert.hermes_name.as_deref().unwrap_or("um Hermes");
    let from = alert
        .details
        .get("from")
        .and_then(serde_json::Value::as_str)
        .map(|address| format!(", de {address}"))
        .unwrap_or_default();
    let (grave, lead, text) = match alert.kind {
        AlertKind::InviteMisused => (
            false,
            "Convite",
            format!(
                "outro aparelho tentou usar o convite de {hermes}{from}. O bloco foi parar em \
                 outro lugar. O primeiro pedido continua valendo."
            ),
        ),
        AlertKind::IdentityCopied => (
            true,
            "Identidade copiada",
            format!(
                "a identidade de {hermes} foi copiada antes da aprovação. O pedido foi negado."
            ),
        ),
        AlertKind::Quarantined => {
            let why = match alert
                .details
                .get("reason")
                .and_then(serde_json::Value::as_str)
            {
                Some("reuse") => "uma credencial já trocada foi apresentada",
                Some("double_connection") => "duas conexões ao mesmo tempo",
                Some("unreadable_credential") => "a credencial não pôde ser lida",
                Some("not_recognized") => "o último uso não foi reconhecido no reconectar",
                _ => "o hub deixou de confiar na credencial",
            };
            (
                true,
                "Hermes em quarentena",
                format!("{hermes}. Motivo: {why}{from}."),
            )
        }
        AlertKind::CopySuspected => (
            true,
            "Cópia suspeita",
            format!("o último uso de {hermes} não foi reconhecido no reconectar."),
        ),
        AlertKind::ConnectionTaken => (
            true,
            "Conexão tomada",
            format!("a conexão de {hermes} foi tomada por outra{from}."),
        ),
        AlertKind::DeviceLeft => (
            false,
            "Encerramento",
            format!("{hermes} pediu o próprio encerramento{from}."),
        ),
        AlertKind::ReuseAfterClosing => (
            true,
            "Reuso",
            format!("uma cópia de {hermes} foi usada antes do encerramento{from}."),
        ),
        AlertKind::UnconfirmedSwaps => (
            false,
            "Trocas sem confirmação",
            format!(
                "{hermes} recebeu credenciais e não confirmou, três vezes em 24 horas{from}. \
                 Pode ser rede ruim, pode ser alguém testando uma cópia."
            ),
        ),
        AlertKind::MarkNotAdvanced => (
            false,
            "Marca da auditoria",
            "um evento de segurança foi gravado e a marca fora do banco não acompanhou.".to_owned(),
        ),
        AlertKind::EngineVersionUntested => (
            false,
            "Motor",
            "a versão do motor não é uma das que o hub conferiu.".to_owned(),
        ),
        AlertKind::EngineSessionCorrected => (
            false,
            "Motor",
            "uma sessão do motor estava com outra configuração, e o hub aplicou a dele.".to_owned(),
        ),
        AlertKind::NumberSwapped => (
            true,
            "Número trocado",
            "o celular que pareou é de um número diferente do informado. Os acessos ficam \
             parados até você decidir."
                .to_owned(),
        ),
    };
    Notice { grave, lead, text }
}

/// What every page shows around its content: the restored mode and the alerts.
pub(super) async fn frame(hub: &Hub, organization: ResolvedOrg) -> Result<Frame, RepoError> {
    let restored = restore::installation(hub.pool())
        .await?
        .filter(|installation| installation.restored_mode)
        .map(|installation| {
            let copy = installation
                .restored_copy_of
                .map_or_else(|| "data desconhecida".to_owned(), render::time);
            format!(
                "O hub voltou de uma cópia de {copy}. O que foi aprovado, revogado, reconectado \
                 ou mudado nos acessos depois disso se perdeu. Reconecte cada Hermes e confira \
                 os acessos."
            )
        });
    let alerts = repo::alert::list(hub.pool(), organization, AlertFilter::Unseen, READ_MAX).await?;
    let unknown_devices = repo::device::unknown_listed(hub.pool(), organization, READ_MAX)
        .await?
        .len();
    Ok(Frame {
        restored,
        more_notices: alerts.len().saturating_sub(NOTICES_SHOWN),
        notices: alerts.iter().take(NOTICES_SHOWN).map(notice).collect(),
        unknown_devices,
    })
}

/// The page of the Hermes.
pub(super) async fn hermes(hub: &Hub, organization: ResolvedOrg) -> Result<HermesPage, RepoError> {
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
                format!(
                    "O hub voltou de uma cópia de segurança. {name} está suspenso e só volta \
                     com convite novo: reconecte ou revogue. Não existe \"reativar\"."
                )
            } else {
                format!(
                    "O hub deixou de confiar na credencial de {name}. Motivo: {}. O acesso está \
                     parado. Saídas: reconectar ou revogar. Não existe \"liberar\".",
                    reason(device)
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
                "agora".to_owned()
            } else {
                ago(
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
                        when: ago(now, Some(request.state_since)),
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
                                || "nunca".to_owned(),
                                |at| format!("{} ({})", ago(now, Some(at)), render::time(at)),
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
        NumberState::Unpaired => (Tone::Idle, "Sem pareamento"),
        NumberState::Pairing => (Tone::Wait, "Pareando"),
        NumberState::Connected => (Tone::Ok, "Conectado"),
        NumberState::Reconnecting => (Tone::Wait, "Reconectando"),
        NumberState::Disconnected => (Tone::Idle, "Desconectado"),
        NumberState::NumberSwapped => (Tone::Bad, "Número trocado"),
        NumberState::Failed => (Tone::Bad, "Falha"),
        NumberState::Removed => (Tone::Idle, "Removido"),
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
) -> Result<NumbersPage, RepoError> {
    let numbers = repo::number::list(hub.pool(), organization, false).await?;
    let accesses = repo::access::list(hub.pool(), organization, None).await?;
    let mut phones = phones(hub, organization, &numbers).await;
    let now = hub.now_ms();
    let engine = repo::engine::get(hub.pool(), organization)
        .await?
        .map(|engine| {
            let version = match (&engine.version_seen, engine.version_tested) {
                (Some(version), Some(true)) => format!("versão {version}, conferida"),
                (Some(version), _) => format!("versão {version}, não conferida pelo hub"),
                (None, _) => "versão ainda não lida".to_owned(),
            };
            let answered = match engine.last_ok_at {
                Some(at) => format!("última resposta {}", ago(now, Some(at))),
                None => "ainda não respondeu".to_owned(),
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
                last_message: ago(now, number.last_message_at),
            }
        })
        .collect();
    Ok(NumbersPage { rows, engine })
}

/// What a cell of the matrix says: the verbs that reach some chat, and which chats.
fn cell(access: &Listed) -> Cell {
    let lines = access.lines.len();
    let (verbs, scope) = match access.grant.scope {
        Scope::All => (
            access.grant.verbs,
            if lines == 0 {
                "todas".to_owned()
            } else {
                format!("todas, {lines} com regra própria")
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
            let scope = match lines {
                0 => "nenhuma conversa".to_owned(),
                1 => "1 conversa".to_owned(),
                more => format!("{more} conversas"),
            };
            (verbs, scope)
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
                "Suspenso"
            } else {
                "Em quarentena"
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
                                .map(cell)
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
            warnings.push(format!(
                "{alias}: {woken} Hermes são acordados nas mesmas conversas."
            ));
        }
        let sending = together(&here, |verbs| verbs.send);
        if sending > 1 {
            warnings.push(format!(
                "{alias}: {sending} Hermes enviam nas mesmas conversas."
            ));
        }
        let sends = here
            .iter()
            .any(|access| Reach::of(access, |verbs| verbs.send).reaches_something());
        if number.command && sends {
            warnings.push(format!(
                "{alias} está marcado como \"de comando\" e há Hermes com permissão de enviar nele."
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
        assert_eq!(ago(now, None), "nunca");
        assert_eq!(ago(now, Some(now)), "agora");
        assert_eq!(ago(now, Some(now - 59_000)), "agora");
        assert_eq!(ago(now, Some(now - 60_000)), "há 1 min");
        assert_eq!(ago(now, Some(now - 59 * 60_000)), "há 59 min");
        assert_eq!(ago(now, Some(now - 3_600_000)), "há 1 h");
        assert_eq!(ago(now, Some(now - 47 * 3_600_000)), "há 47 h");
        assert_eq!(ago(now, Some(now - 48 * 3_600_000)), "há 2 dias");
        // A clock that went back does not print a negative time.
        assert_eq!(ago(now, Some(now + 5_000)), "agora");
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
