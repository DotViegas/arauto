//! The key of the Hermes session of a contact (decision 0054, section 1), built by the hub when
//! a WhatsApp notice leaves.
//!
//! The key is a digest of fixed words and telephones ([`arauto_protocol::session_key`]); it
//! needs the telephone of the number, sealed with the key of the organization, and the
//! identifiers the engine knows the chat by. The telephones never leave this module: only the
//! key does.
//!
//! | The chat | The key |
//! |---|---|
//! | A contact with a telephone identifier | the contact key |
//! | A group | the group key, from the identifier of the engine without `@g.us` |
//! | A contact known only by a hidden identifier | the engine is asked for the telephone: known, the contact key; "not known", the hidden key; failed, [`Keyed::EngineFailed`] and the notice goes back to the queue, until [`ENGINE_FAILURES_MAX`] failures in a row, when the hidden key is used |
//!
//! When the engine says the telephone of a hidden identifier, the hub also records the pairing
//! when it has the chat of that telephone and no chat has the identifier yet
//! ([`repo::chat::link_hidden`]), and so does a sending to a telephone
//! ([`record_pairing_after_sending`]). A pairing comes only from an answer of the engine, never
//! from the text of a message.

use arauto_protocol::names::NumberAlias;
use arauto_protocol::session_key::{
    SessionKey, whatsapp_contact_key, whatsapp_group_key, whatsapp_hidden_key,
};
use uuid::Uuid;

use crate::channel::Hub;
use crate::db::ResolvedOrg;
use crate::engine::{ChatKind, EngineChatId, Phone, SessionName};
use crate::repo::peer::ENGINE_FAILURES_MAX;
use crate::repo::{self, RepoError};
use crate::vault::OrganizationKey;

/// What building the key of a notice came to.
#[derive(Debug)]
pub(crate) enum Keyed {
    /// The key: the notice carries it.
    Key(SessionKey),
    /// The engine failed and the notice may wait for it: the lease is released.
    EngineFailed,
    /// Nothing to build a key from (the number or the chat is gone): the notice is left to its
    /// lease and to the judging that comes with it.
    Nothing,
}

/// The key of the notice `notice` for the chat `chat` of the number `alias` (`number_id`).
/// `failures` is how many times in a row the engine failed for this notice already.
///
/// # Errors
/// [`RepoError`] when the database or the vault refuses.
pub(crate) async fn key_of(
    hub: &Hub,
    organization: ResolvedOrg,
    notice: Uuid,
    alias: &NumberAlias,
    number_id: Uuid,
    chat: Uuid,
    failures: u32,
) -> Result<Keyed, RepoError> {
    let key = hub.organization_key(organization).await?;
    let Some(number) = repo::number::by_alias(hub.pool(), organization, alias).await? else {
        return Ok(Keyed::Nothing);
    };
    if number.id != number_id {
        return Ok(Keyed::Nothing);
    }
    let business = number.phone(&key)?;
    let ids = repo::chat::engine_ids(hub.pool(), organization, &key, number_id, chat).await?;
    let Some(first) = ids.first() else {
        return Ok(Keyed::Nothing);
    };
    // A telephone identifier is the contact, whatever else the chat is known by.
    if let Some(phone) = ids.iter().find_map(EngineChatId::phone) {
        return Ok(typed(&whatsapp_contact_key(
            business.digits(),
            phone.digits(),
        )));
    }
    if first.kind() == ChatKind::Group {
        let group = first.as_str().strip_suffix("@g.us").unwrap_or_default();
        return Ok(typed(&whatsapp_group_key(business.digits(), group)));
    }
    let Some(hidden) = ids.iter().find(|id| id.as_str().ends_with("@lid")) else {
        return Ok(Keyed::Nothing);
    };
    let digits = hidden.as_str().strip_suffix("@lid").unwrap_or_default();
    let hidden_key = || typed(&whatsapp_hidden_key(business.digits(), digits));
    let asked = ask_for_the_phone(hub, organization, &number.session, hidden).await;
    match asked {
        Ok(Some(phone)) => {
            repo::peer::engine_answered(hub.pool(), organization, notice).await?;
            link(hub, organization, &key, number_id, chat, &phone, hidden).await;
            Ok(typed(&whatsapp_contact_key(
                business.digits(),
                phone.digits(),
            )))
        }
        Ok(None) => {
            repo::peer::engine_answered(hub.pool(), organization, notice).await?;
            Ok(hidden_key())
        }
        // Five failures in a row: the conversation moves on under the hidden key rather than
        // wait for the engine for ever.
        Err(()) if failures >= ENGINE_FAILURES_MAX => Ok(hidden_key()),
        Err(()) => Ok(Keyed::EngineFailed),
    }
}

fn typed(key: &str) -> Keyed {
    match SessionKey::parse(key) {
        Ok(key) => Keyed::Key(key),
        Err(_) => Keyed::Nothing,
    }
}

/// The telephone the engine keeps for `hidden`: `Err` when the engine is not there or fails.
async fn ask_for_the_phone(
    hub: &Hub,
    organization: ResolvedOrg,
    session: &SessionName,
    hidden: &EngineChatId,
) -> Result<Option<Phone>, ()> {
    let engine = hub
        .engines()
        .of(hub.pool(), hub.vault(), organization)
        .await
        .map_err(|_| ())?;
    engine
        .client
        .phone_of_hidden(session, hidden)
        .await
        .map_err(|_| ())
}

/// Records the pairing of `hidden` with the chat of `phone`, when the hub has that chat and it
/// is not the chat the hidden identifier already belongs to. A failure is logged and changes
/// nothing: the key does not wait for the pairing.
async fn link(
    hub: &Hub,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    chat: Uuid,
    phone: &Phone,
    hidden: &EngineChatId,
) {
    link_to_phone_chat(hub, organization, key, number, Some(chat), phone, hidden).await;
}

/// Records `hidden` in the chat of `phone` when the hub has it and it is not `except`.
async fn link_to_phone_chat(
    hub: &Hub,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    except: Option<Uuid>,
    phone: &Phone,
    hidden: &EngineChatId,
) {
    let found = repo::chat::find_by_phone(hub.pool(), organization, key, number, phone).await;
    match found {
        Ok(Some(of_phone)) if Some(of_phone) != except => {
            let linked = repo::chat::link_hidden(
                hub.pool(),
                organization,
                key,
                number,
                of_phone,
                hidden,
                hub.now_ms(),
            )
            .await;
            if let Err(error) = linked {
                tracing::error!(%error, "a hidden identifier could not be linked to its chat");
            }
        }
        Ok(_) => {}
        Err(error) => {
            tracing::error!(%error, "the chat of a telephone could not be looked for");
        }
    }
}

/// A message arrives under the hidden identifier `chat` (decision 0054, section 10): when no
/// chat of the number has that identifier yet, the engine is asked for its telephone, and when
/// the hub has the chat of that telephone the identifier is recorded there, so that the message
/// lands in the chat of the telephone and no second chat is created. It runs before the write
/// of the message. An engine that fails, does not know the identifier or does not answer
/// changes nothing: the message makes its own chat as before.
pub(crate) async fn link_on_arrival(
    hub: &Hub,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    session: &SessionName,
    chat: &EngineChatId,
) {
    if !chat.as_str().ends_with("@lid") {
        return;
    }
    match repo::chat::find(hub.pool(), organization, key, number, chat).await {
        Ok(None) => {}
        Ok(Some(_)) => return,
        Err(error) => {
            tracing::error!(%error, "the chat of a hidden identifier could not be looked for");
            return;
        }
    }
    if let Ok(Some(phone)) = ask_for_the_phone(hub, organization, session, chat).await {
        link_to_phone_chat(hub, organization, key, number, None, &phone, chat).await;
    }
}

/// After a sending to the telephone chat `chat`: asks the engine for the hidden identifier of
/// that telephone and records the pairing. An engine that does not offer the question, or does
/// not know the telephone, changes nothing.
pub(crate) async fn record_pairing_after_sending(
    hub: &Hub,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    session: &SessionName,
    chat: Uuid,
    sent_to: &EngineChatId,
) {
    let Some(phone) = sent_to.phone() else {
        return;
    };
    let Ok(engine) = hub
        .engines()
        .of(hub.pool(), hub.vault(), organization)
        .await
    else {
        return;
    };
    let Ok(Some(hidden)) = engine.client.lid_of_phone(session, &phone).await else {
        return;
    };
    let linked = repo::chat::link_hidden(
        hub.pool(),
        organization,
        key,
        number,
        chat,
        &hidden,
        hub.now_ms(),
    )
    .await;
    if let Err(error) = linked {
        tracing::error!(%error, "a hidden identifier could not be linked to its chat");
    }
}
