//! The supervision of the engine: at the start of the hub and every 5 minutes
//! (`docs/03-arquitetura.md`, section 2; `docs/10-maquinas-de-estado.md`, section 4).
//!
//! For each organization with an engine:
//!
//! 1. the pairings whose QR or code expired go back to unpaired;
//! 2. the version of the engine is read and recorded; a version the hub was not checked
//!    against raises an alert, once per version;
//! 3. for each number, all at once (a slow one does not hold the others): the session of the
//!    engine is created already configured when missing, or given the whole configuration of
//!    the hub when it has another one (an alert when it had the one of the hub before: someone
//!    changed it from outside); its status is read and applied; a number that should be
//!    connected and whose session stopped is started again;
//! 4. for each connected number, the catch-up: the engine gives up resending an event after a
//!    few minutes (`referencia/waha-fatos.md`), so the hub asks it for the messages since the
//!    newest one recorded, at most a day back, and records them as events, once each.

use std::sync::Arc;

use futures_util::future::join_all;
use serde_json::json;

use super::registry::LoadedEngine;
use super::types::{Author, EngineStatus, Ensured, Message, MessageQuery, Page};
use crate::audit::{self, Action, Actor, Details, Event, TargetKind};
use crate::channel::Hub;
use crate::core::number::NumberState;
use crate::db::{self, ResolvedOrg, SystemTask};
use crate::repo::alert::{AlertDetails, AlertKind, NewAlert, raise};
use crate::repo::engine_event::{self, EventKind, NewEvent, Source};
use crate::repo::number::{self, Number, Observed};
use crate::repo::{self, RepoError};
use crate::vault::OrganizationKey;

/// Runs until the hub stops: once at once, then at every interval or when asked.
pub(crate) async fn run(hub: Arc<Hub>) {
    let mut stopping = hub.stopping();
    loop {
        supervise(&hub).await;
        tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => return,
            () = tokio::time::sleep(hub.engines().interval()) => {}
            () = hub.engines().supervise_asked() => {}
        }
    }
}

/// One round, in every organization: what the hub does at its start and every 5 minutes.
pub async fn supervise(hub: &Hub) {
    let organizations =
        match db::organizations_for_system_task(hub.pool(), SystemTask::Engine).await {
            Ok(organizations) => organizations,
            Err(error) => {
                tracing::warn!(%error, "the supervision of the engine could not read the database");
                return;
            }
        };
    for organization in organizations {
        if let Err(error) = supervise_organization(hub, organization).await {
            tracing::warn!(%error, "the supervision of the engine stopped for an organization");
        }
    }
}

async fn supervise_organization(hub: &Hub, organization: ResolvedOrg) -> Result<(), RepoError> {
    let now = hub.now_ms();
    number::expire_pairings(hub.pool(), organization, now).await?;
    let Ok(engine) = hub
        .engines()
        .of(hub.pool(), hub.vault(), organization)
        .await
    else {
        return Ok(());
    };
    let version = match engine.client.version().await {
        Ok(version) => version,
        Err(error) => {
            tracing::warn!(%error, "the engine did not answer its version");
            return Ok(());
        }
    };
    let changed =
        repo::engine::record_version(hub.pool(), organization, engine.id, &version, now).await?;
    if changed && !version.is_tested() {
        tracing::warn!("the engine is of a version the hub was not checked against");
        let mut connection = hub.pool().acquire().await?;
        raise(
            &mut connection,
            organization,
            now,
            &NewAlert {
                kind: AlertKind::EngineVersionUntested,
                hermes: None,
                device: None,
                details: AlertDetails::new().id("engine", engine.id),
            },
        )
        .await?;
    }
    let key = hub.organization_key(organization).await?;
    let numbers = number::list(hub.pool(), organization, false).await?;
    let rounds = numbers
        .iter()
        .map(|number| supervise_number(hub, organization, &engine, &key, number));
    for result in join_all(rounds).await {
        if let Err(error) = result {
            tracing::warn!(%error, "the supervision of a number failed");
        }
    }
    Ok(())
}

async fn supervise_number(
    hub: &Hub,
    organization: ResolvedOrg,
    engine: &LoadedEngine,
    key: &OrganizationKey,
    number: &Number,
) -> Result<(), RepoError> {
    let client = &engine.client;
    let config = engine.session_config();
    let digest = client.config_digest(&config);
    let applied_before = number.config_digest.as_deref() == Some(digest.as_slice());
    let ensured = match client
        .ensure_session(&number.session, &config, applied_before)
        .await
    {
        Ok(ensured) => ensured,
        Err(error) => {
            tracing::warn!(%error, number = %number.id, "the session of the engine was not checked");
            return Ok(());
        }
    };
    let now = hub.now_ms();
    match &ensured {
        Ensured::Unchanged if applied_before => {}
        Ensured::Created | Ensured::Unchanged => {
            number::config_applied(hub.pool(), organization, number.id, &digest, now).await?;
        }
        Ensured::Reconfigured(differences) => {
            number::config_applied(hub.pool(), organization, number.id, &digest, now).await?;
            if applied_before {
                corrected(hub, organization, number, differences.len(), now).await?;
            }
        }
    }
    let info = match client.session(&number.session).await {
        Ok(Some(info)) => info,
        Ok(None) => return Ok(()),
        Err(error) => {
            tracing::warn!(%error, number = %number.id, "the session of the engine was not read");
            return Ok(());
        }
    };
    let observed = Observed {
        status: info.status,
        phone: info.me.as_ref().and_then(|me| me.phone.clone()),
        at: None,
    };
    let mut transaction = hub.pool().begin().await?;
    let changed = number::observe(
        &mut transaction,
        organization,
        key,
        number.id,
        &observed,
        now,
    )
    .await?;
    transaction.commit().await?;
    let state = changed.map_or(number.state, |(_, to)| to);
    if matches!(state, NumberState::Connected | NumberState::Reconnecting)
        && info.status == EngineStatus::Stopped
        && let Err(error) = client.start(&number.session).await
    {
        tracing::warn!(%error, number = %number.id, "the session of the engine was not started");
    }
    if state == NumberState::Connected && info.status == EngineStatus::Working {
        let since = number.last_message_at.unwrap_or(now);
        catch_up(hub, organization, engine, key, number, since).await?;
    }
    Ok(())
}

/// The session of the engine had another configuration than the one the hub gave it.
async fn corrected(
    hub: &Hub,
    organization: ResolvedOrg,
    number: &Number,
    differences: usize,
    now: i64,
) -> Result<(), RepoError> {
    tracing::warn!(
        number = %number.id,
        differences,
        "a session of the engine was configured from outside; the hub applied its own"
    );
    let mut transaction = hub.pool().begin().await?;
    raise(
        &mut transaction,
        organization,
        now,
        &NewAlert {
            kind: AlertKind::EngineSessionCorrected,
            hermes: None,
            device: None,
            details: AlertDetails::new().id("number", number.id).number(
                "differences",
                i64::try_from(differences).unwrap_or(i64::MAX),
            ),
        },
    )
    .await?;
    audit::record(
        &mut *transaction,
        now,
        &Event::new(Action::EngineSessionCorrected, Actor::System)
            .in_organization(organization)
            .about(TargetKind::Number, number.id)
            .with(Details::new().alias("alias", &number.alias)),
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

/// Records the messages of `number` since `since` that the hub does not have yet.
async fn catch_up(
    hub: &Hub,
    organization: ResolvedOrg,
    engine: &LoadedEngine,
    key: &OrganizationKey,
    number: &Number,
    since: i64,
) -> Result<(), RepoError> {
    let settings = hub.engines().settings();
    let now = hub.now_ms();
    let since = since.max(now.saturating_sub(settings.catch_up_horizon_ms));
    let (Ok(chats), Ok(messages)) = (
        Page::new(settings.catch_up_chats, 0),
        Page::new(settings.catch_up_messages, 0),
    ) else {
        return Ok(());
    };
    let chats = match engine.client.chats(&number.session, chats).await {
        Ok(chats) => chats,
        Err(error) => {
            tracing::warn!(%error, number = %number.id, "the catch-up could not list the chats");
            return Ok(());
        }
    };
    let mut recorded = 0usize;
    for chat in chats {
        if chat.last.as_ref().is_some_and(|last| last.at_ms < since) {
            continue;
        }
        let query = MessageQuery {
            page: messages,
            before_s: None,
            after_s: Some(since.div_euclid(1_000)),
        };
        let found = match engine
            .client
            .messages(&number.session, &chat.id, query)
            .await
        {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(%error, number = %number.id, "the catch-up could not read a chat");
                continue;
            }
        };
        let mut transaction = hub.pool().begin().await?;
        for message in found.iter().filter(|message| message.at_ms >= since) {
            let body = caught_up_body(&number.session, message);
            let key_text = format!("message:{}", message.id.as_str());
            let event = NewEvent {
                engine: engine.id,
                number: number.id,
                kind: EventKind::Message,
                key: key_text.as_bytes(),
                body: &body,
                engine_at: message.at_ms,
                source: Source::CatchUp,
            };
            if engine_event::record(&mut transaction, organization, key, &event, now)
                .await?
                .is_some()
            {
                recorded = recorded.saturating_add(1);
            }
            number::advance_messages(&mut transaction, organization, number.id, message.at_ms)
                .await?;
        }
        transaction.commit().await?;
    }
    if recorded > 0 {
        tracing::info!(number = %number.id, recorded, "the catch-up recorded messages");
    }
    Ok(())
}

/// A message found by the catch-up, in the shape of the event the engine would have sent:
/// whoever reads the inbox reads one shape.
fn caught_up_body(session: &super::types::SessionName, message: &Message) -> Vec<u8> {
    let (from_me, participant) = match &message.author {
        Author::Me => (true, None),
        Author::Other(other) if other != &message.chat => (false, Some(other.as_str())),
        Author::Other(_) | Author::Unknown => (false, None),
    };
    json!({
        "event": EventKind::Message.as_str(),
        "session": session.as_str(),
        "payload": {
            "id": message.id.as_str(),
            "from": message.chat.as_str(),
            "fromMe": from_me,
            "participant": participant,
            "timestamp": message.at_ms.div_euclid(1_000),
            "body": message.body.expose(),
            "hasMedia": message.media.is_some(),
            "replyTo": message.reply_to.as_ref().map(|reply| reply.as_str()),
        },
    })
    .to_string()
    .into_bytes()
}
