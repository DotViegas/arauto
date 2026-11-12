//! The WhatsApp tools, answered by the hub in a session of the channel
//! (`docs/09-contrato-mcp.md`, sections 2 to 4; `docs/08-permissoes.md`, section 3).
//!
//! Every call reads its access in the database, at that call (rule 9 of the project): taking
//! an access away holds at the next call, whatever list of tools the session was given. The
//! order of the checks, after the session read the device again and checked the name and the
//! arguments (AR-711):
//!
//! | Step | Refusal |
//! |---|---|
//! | The Hermes has the verb of the tool on no number | AR-701 |
//! | `number` names no number this Hermes has an access to (or is left out with more than one) | AR-701 (AR-711) |
//! | The telephone of the number was swapped: its accesses wait for the admin | AR-702 |
//! | The verb on the number, or in the chat (its line, or the scope) | AR-701, AR-705 |
//! | The number is not connected to WhatsApp | AR-702 |
//! | The engine is not there | AR-703 |
//! | The send limit | AR-704, with `retry_after_s` |
//! | What the engine answered | its code |
//!
//! "Number does not exist" and "no access to it" are the same answer, and so are "chat does
//! not exist" and "chat outside the scope". A chat is named by the identifier the hub gave it
//! (a UUID of the table `chat`); a message, by the identifier the hub signed ([`token`]). The
//! name of a session of the engine is never an argument: the hub takes it from the access.
//!
//! The engine is reached only through [`Authorized`], which only [`Call::authorized`] builds,
//! after the checks above: the one place where a call of a Hermes meets the engine.

pub mod ceiling;
mod token;

use std::sync::Arc;

use arauto_protocol::errors::{ErrorCode, RetryAfter};
use arauto_protocol::limits::WHATSAPP_READ_BUDGET_BYTES;
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::names::NumberAlias;
use arauto_protocol::tools::{
    FileKind as ArgFileKind, Grants, Listing, MessageRef, ReadChat, Recipient, WhatsappCall,
    WhatsappOp, WhatsappVerb,
};
use data_encoding::BASE64;
use serde_json::{Value, json};
use uuid::Uuid;

pub use ceiling::SendCeiling;

use crate::channel::Hub;
use crate::core::access::{Scope, Verb};
use crate::core::number::NumberState;
use crate::db::ResolvedOrg;
use crate::engine::registry::LoadedEngine;
use crate::engine::{
    Author, ChatKind, EngineChatId, EngineError, EngineMessageId, FileContent, FileKind, FileName,
    FullMessageId, Message, MessageBody, MessageQuery, MimeType, PAGE_MAX, Page, Phone, Reaction,
    Role, SendFile, SendText, SessionName,
};
use crate::repo::access::HermesAccess;
use crate::repo::{self, RepoError};
use crate::vault::OrganizationKey;

/// What is kept apart, in the budget of a reading, for the fields around the list.
const ENVELOPE_BYTES: usize = 1_024;

/// Why a call is refused: the code, and when a limit comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Refusal {
    code: ErrorCode,
    retry_after: Option<RetryAfter>,
}

impl From<ErrorCode> for Refusal {
    fn from(code: ErrorCode) -> Self {
        Self {
            code,
            retry_after: None,
        }
    }
}

impl From<EngineError> for Refusal {
    fn from(error: EngineError) -> Self {
        error.code().into()
    }
}

type Answer = Result<Value, Refusal>;

/// A failure of the hub itself, logged without any value of the call: AR-205.
fn failed(error: &RepoError, what: &'static str) -> Refusal {
    tracing::error!(%error, "{what}");
    ErrorCode::Ar205.into()
}

/// What answers a WhatsApp call.
pub(crate) async fn answer(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    call: &WhatsappCall,
) -> CallOutcome {
    match run(hub, organization, hermes, call).await {
        Ok(Value::Object(content)) => CallOutcome::Ok(Payload::new(content)),
        Ok(_) => CallOutcome::Refused {
            code: ErrorCode::Ar205,
            retry_after: None,
        },
        Err(refusal) => CallOutcome::Refused {
            code: refusal.code,
            retry_after: refusal.retry_after,
        },
    }
}

/// The grants of a Hermes now, from the database: what its list of tools follows.
pub(crate) async fn grants_now(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
) -> Result<Grants, RepoError> {
    let accesses = repo::access::of_hermes(hub.pool(), organization, hermes).await?;
    Ok(repo::access::grants(&accesses))
}

/// The numbers of a Hermes as `arauto_status` shows them: the alias, the state, the scope and
/// the verbs. Nothing else: no telephone, no session of the engine.
pub(crate) async fn numbers_of(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
) -> Result<Vec<Value>, RepoError> {
    let accesses = repo::access::of_hermes(hub.pool(), organization, hermes).await?;
    Ok(accesses
        .iter()
        .map(|access| {
            json!({
                "number": access.number.alias.as_str(),
                "state": access.number.state.as_str(),
                "scope": access.grant.scope.as_str(),
                "verbs": access.grant.verbs.words(),
                "chat_verbs": access.in_lines.words(),
                "can_start_chats": access.grant.can_start_chats,
            })
        })
        .collect())
}

const fn verb_of(verb: WhatsappVerb) -> Verb {
    match verb {
        WhatsappVerb::Read => Verb::Read,
        WhatsappVerb::Send => Verb::Send,
        WhatsappVerb::Modify => Verb::Modify,
    }
}

/// The access the call names: by its alias, or the only one.
fn chosen<'a>(
    accesses: &'a [HermesAccess],
    alias: Option<&NumberAlias>,
) -> Result<&'a HermesAccess, Refusal> {
    match alias {
        Some(alias) => accesses
            .iter()
            .find(|access| &access.number.alias == alias)
            .ok_or_else(|| ErrorCode::Ar701.into()),
        None => match accesses {
            [] => Err(ErrorCode::Ar701.into()),
            [only] => Ok(only),
            _ => Err(ErrorCode::Ar711.into()),
        },
    }
}

async fn run(hub: &Hub, organization: ResolvedOrg, hermes: Uuid, call: &WhatsappCall) -> Answer {
    let accesses = repo::access::of_hermes(hub.pool(), organization, hermes)
        .await
        .map_err(|error| failed(&error, "the accesses of a Hermes could not be read"))?;
    let verb = verb_of(call.op.verb());
    if !accesses.iter().any(|access| access.anywhere(verb)) {
        return Err(ErrorCode::Ar701.into());
    }
    let access = chosen(&accesses, call.number.as_ref())?;
    if access.number.state == NumberState::NumberSwapped {
        return Err(ErrorCode::Ar702.into());
    }
    let key = hub
        .organization_key(organization)
        .await
        .map_err(|error| failed(&error, "the key of an organization could not be opened"))?;
    let call_of = Call {
        hub,
        organization,
        hermes,
        access,
        key: &key,
    };
    call_of.run(&call.op).await
}

/// One call, with its access chosen.
struct Call<'a> {
    hub: &'a Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    access: &'a HermesAccess,
    key: &'a OrganizationKey,
}

/// The engine, for a call that passed every check of its access: the only way a call of a
/// Hermes reaches it.
struct Authorized<'a> {
    engine: Arc<LoadedEngine>,
    session: &'a SessionName,
}

/// A chat of the number, with the identifier the engine knows it by.
struct Reached {
    chat: Uuid,
    engine_chat: EngineChatId,
}

/// Where a sending goes. `chat` is `None` for a chat the hub does not have yet.
struct Destination {
    chat: Option<Uuid>,
    engine_chat: EngineChatId,
}

impl<'a> Call<'a> {
    fn alias(&self) -> &str {
        self.access.number.alias.as_str()
    }

    /// The verb on the number itself.
    fn on_the_number(&self, verb: Verb) -> Result<(), Refusal> {
        Ok(self.access.grant.on_the_number(verb)?)
    }

    /// The verb on the number, or in some chat: what the listings need.
    fn anywhere(&self, verb: Verb) -> Result<(), Refusal> {
        if self.access.anywhere(verb) {
            Ok(())
        } else {
            Err(ErrorCode::Ar701.into())
        }
    }

    /// The engine, once the number is connected (`docs/08-permissoes.md`, section 3, rule 6).
    async fn authorized(&self) -> Result<Authorized<'a>, Refusal> {
        if !self.access.number.state.serves_calls() {
            return Err(ErrorCode::Ar702.into());
        }
        let engine = self
            .hub
            .engines()
            .of(self.hub.pool(), self.hub.vault(), self.organization)
            .await?;
        Ok(Authorized {
            engine,
            session: &self.access.number.session,
        })
    }

    /// A place in the send limit, for a call that is about to go to the engine, when the
    /// call is one that counts.
    fn spend(&self, op: &WhatsappOp) -> Result<(), Refusal> {
        if !op.counts_toward_the_send_limit() {
            return Ok(());
        }
        self.hub
            .send_ceiling()
            .take(self.hermes, self.access.number.id, self.hub.now_ms())
            .map_err(|wait| Refusal {
                code: ErrorCode::Ar704,
                retry_after: Some(wait),
            })
    }

    /// The identifiers the engine knows a chat of this number by; empty for a chat that is
    /// not of this number.
    async fn engine_ids(&self, chat: Uuid) -> Result<Vec<EngineChatId>, Refusal> {
        repo::chat::engine_ids(
            self.hub.pool(),
            self.organization,
            self.key,
            self.access.number.id,
            chat,
        )
        .await
        .map_err(|error| failed(&error, "the identifiers of a chat could not be read"))
    }

    async fn line(&self, chat: Uuid) -> Result<Option<crate::core::access::Verbs>, Refusal> {
        repo::access::chat_line(self.hub.pool(), self.organization, self.access.id, chat)
            .await
            .map_err(|error| failed(&error, "the line of a chat could not be read"))
    }

    /// A chat of this number, with `verb` in it. A chat of another number, or none at all, is
    /// the chat that does not exist (AR-705).
    async fn chat(&self, chat: Uuid, verb: Verb) -> Result<Reached, Refusal> {
        let Some(engine_chat) = self.engine_ids(chat).await?.into_iter().next() else {
            return Err(ErrorCode::Ar705.into());
        };
        let line = self.line(chat).await?;
        self.access.grant.in_chat(line, verb)?;
        Ok(Reached { chat, engine_chat })
    }

    /// The message an identifier given by the Hermes names, in a chat of this number with
    /// `verb`. An identifier the hub did not sign, of another number or outside the scope is
    /// the message that does not exist (AR-712); only a verb the number lacks says so
    /// (AR-701).
    async fn message(
        &self,
        given: &MessageRef,
        verb: Verb,
    ) -> Result<(Reached, EngineMessageId), Refusal> {
        let (chat, message) = token::open(self.key, given).ok_or(ErrorCode::Ar712)?;
        let ids = self.engine_ids(chat).await?;
        let Some(engine_chat) = ids.first().cloned() else {
            return Err(ErrorCode::Ar712.into());
        };
        // The identifier names its chat: it must be this one.
        if let Some(full) = message.full()
            && !ids.contains(full.chat())
        {
            return Err(ErrorCode::Ar712.into());
        }
        let line = self.line(chat).await?;
        self.access
            .grant
            .in_chat(line, verb)
            .map_err(|code| match code {
                ErrorCode::Ar701 => Refusal::from(ErrorCode::Ar701),
                _ => Refusal::from(ErrorCode::Ar712),
            })?;
        Ok((Reached { chat, engine_chat }, message))
    }

    /// Where a sending goes, with the verb send there: a chat of the hub, or a telephone,
    /// whose exact form the engine gives. A telephone whose chat the hub does not have yet is
    /// a chat that exists when the engine has a message in it; otherwise it is a new chat,
    /// which needs the scope "all" and "may start chats" (rule 5).
    async fn destination(
        &self,
        engine: &Authorized<'_>,
        to: &Recipient,
    ) -> Result<Destination, Refusal> {
        match to {
            Recipient::Chat(chat) => {
                let reached = self.chat(*chat, Verb::Send).await?;
                Ok(Destination {
                    chat: Some(reached.chat),
                    engine_chat: reached.engine_chat,
                })
            }
            Recipient::Phone(phone) => {
                let phone =
                    Phone::parse(phone.digits()).map_err(|_| Refusal::from(ErrorCode::Ar711))?;
                let Some(engine_chat) = engine
                    .engine
                    .client
                    .check_number(engine.session, &phone)
                    .await?
                else {
                    return Err(ErrorCode::Ar705.into());
                };
                let known = repo::chat::find(
                    self.hub.pool(),
                    self.organization,
                    self.key,
                    self.access.number.id,
                    &engine_chat,
                )
                .await
                .map_err(|error| failed(&error, "a chat could not be looked for"))?;
                if let Some(chat) = known {
                    let line = self.line(chat).await?;
                    self.access.grant.in_chat(line, Verb::Send)?;
                    return Ok(Destination {
                        chat: Some(chat),
                        engine_chat,
                    });
                }
                if self.access.grant.scope == Scope::List {
                    return Err(ErrorCode::Ar705.into());
                }
                let query = MessageQuery {
                    page: Page::new(1, 0)?,
                    before_s: None,
                    after_s: None,
                };
                let talked = !engine
                    .engine
                    .client
                    .messages(engine.session, &engine_chat, query)
                    .await?
                    .is_empty();
                if talked {
                    self.access.grant.in_chat(None, Verb::Send)?;
                } else {
                    self.access.grant.new_chat()?;
                }
                Ok(Destination {
                    chat: None,
                    engine_chat,
                })
            }
        }
    }

    /// The chat of the hub of a destination, created when it is new.
    async fn chat_of(&self, destination: &Destination) -> Result<Uuid, Refusal> {
        if let Some(chat) = destination.chat {
            return Ok(chat);
        }
        let mut connection = self
            .hub
            .pool()
            .acquire()
            .await
            .map_err(|error| failed(&RepoError::from(error), "no connection for a chat"))?;
        repo::chat::resolve(
            &mut connection,
            self.organization,
            self.key,
            self.access.number.id,
            &destination.engine_chat,
            self.hub.now_ms(),
        )
        .await
        .map_err(|error| failed(&error, "a chat could not be recorded"))
    }

    /// What a sending answers: the chat and the new message.
    async fn sent(&self, destination: &Destination, id: Option<&EngineMessageId>) -> Answer {
        let chat = self.chat_of(destination).await?;
        Ok(json!({
            "number": self.alias(),
            "chat": chat.as_hyphenated().to_string(),
            "message_id": id.map(|id| token::sign(self.key, chat, id)),
        }))
    }

    fn done(&self) -> Value {
        json!({"number": self.alias(), "done": true})
    }

    async fn run(&self, op: &WhatsappOp) -> Answer {
        match op {
            WhatsappOp::Status => self.status().await,
            WhatsappOp::ListChats(listing) => self.list_chats(*listing).await,
            WhatsappOp::ReadChat(read) => self.read_chat(read).await,
            WhatsappOp::GetContact { chat } => self.contact(*chat).await,
            WhatsappOp::DownloadMedia { message } => self.download(message).await,
            WhatsappOp::ListGroups(listing) => self.list_groups(*listing).await,
            WhatsappOp::GetGroup(group) => self.group(group.chat, group.participants).await,
            WhatsappOp::CheckNumber { phone } => {
                self.on_the_number(Verb::Send)?;
                let engine = self.authorized().await?;
                let phone =
                    Phone::parse(phone.digits()).map_err(|_| Refusal::from(ErrorCode::Ar711))?;
                self.spend(op)?;
                let found = engine
                    .engine
                    .client
                    .check_number(engine.session, &phone)
                    .await?;
                Ok(json!({
                    "number": self.alias(),
                    "phone": format!("+{}", phone.digits()),
                    "has_whatsapp": found.is_some(),
                    "exact": found
                        .as_ref()
                        .and_then(EngineChatId::phone)
                        .map(|exact| format!("+{}", exact.digits())),
                }))
            }
            WhatsappOp::SendText(send) => {
                let engine = self.authorized().await?;
                let destination = self.destination(&engine, &send.to).await?;
                let reply_to = match &send.reply_to {
                    Some(given) => Some(self.reply_in(&destination, given).await?),
                    None => None,
                };
                let mentions = send
                    .mentions
                    .iter()
                    .map(|phone| Phone::parse(phone.digits()).map(|phone| phone.chat_id()))
                    .collect::<Result<Vec<_>, _>>()?;
                let text = SendText {
                    chat: destination.engine_chat.clone(),
                    text: MessageBody::to_send(send.text.expose())?,
                    reply_to,
                    mentions,
                };
                self.spend(op)?;
                let sent = engine
                    .engine
                    .client
                    .send_text(engine.session, &text)
                    .await?;
                self.sent(&destination, sent.id.as_ref()).await
            }
            WhatsappOp::SendFile(send) => {
                let engine = self.authorized().await?;
                let destination = self.destination(&engine, &send.to).await?;
                let file = SendFile {
                    chat: destination.engine_chat.clone(),
                    kind: match send.file.kind {
                        ArgFileKind::Image => FileKind::Image,
                        ArgFileKind::Document => FileKind::Document,
                        ArgFileKind::Voice => FileKind::Voice,
                    },
                    file: FileContent {
                        mimetype: MimeType::parse(&send.file.mimetype)?,
                        filename: send
                            .file
                            .filename
                            .as_deref()
                            .map(FileName::parse)
                            .transpose()?,
                        data: send.file.data.clone(),
                    },
                    caption: send
                        .caption
                        .as_ref()
                        .map(|caption| MessageBody::to_send(caption.expose()))
                        .transpose()?,
                };
                self.spend(op)?;
                let sent = engine
                    .engine
                    .client
                    .send_file(engine.session, &file)
                    .await?;
                self.sent(&destination, sent.id.as_ref()).await
            }
            WhatsappOp::MarkRead { chat } => {
                let reached = self.chat(*chat, Verb::Send).await?;
                let engine = self.authorized().await?;
                self.spend(op)?;
                engine
                    .engine
                    .client
                    .mark_read(engine.session, &reached.engine_chat)
                    .await?;
                Ok(self.done())
            }
            WhatsappOp::SetTyping { chat, on } => {
                let reached = self.chat(*chat, Verb::Send).await?;
                let engine = self.authorized().await?;
                self.spend(op)?;
                engine
                    .engine
                    .client
                    .set_typing(engine.session, &reached.engine_chat, *on)
                    .await?;
                Ok(self.done())
            }
            WhatsappOp::React { message, reaction } => {
                let (_, message) = self.message(message, Verb::Send).await?;
                let engine = self.authorized().await?;
                let reaction = Reaction::parse(reaction.as_str())?;
                self.spend(op)?;
                engine
                    .engine
                    .client
                    .react(engine.session, &message, &reaction)
                    .await
                    .map_err(message_error)?;
                Ok(self.done())
            }
            WhatsappOp::Forward { message, to } => {
                // Reading in the chat of the message, and sending in the other.
                let (_, message) = self.message(message, Verb::Read).await?;
                let engine = self.authorized().await?;
                let destination = self.destination(&engine, to).await?;
                self.spend(op)?;
                let sent = engine
                    .engine
                    .client
                    .forward(engine.session, &message, &destination.engine_chat)
                    .await
                    .map_err(message_error)?;
                self.sent(&destination, sent.id.as_ref()).await
            }
            WhatsappOp::EditMessage { message, text } => {
                let (_, message) = self.message(message, Verb::Modify).await?;
                let full = complete(&message)?;
                let engine = self.authorized().await?;
                let text = MessageBody::to_send(text.expose())?;
                engine
                    .engine
                    .client
                    .edit_message(engine.session, &full, &text)
                    .await
                    .map_err(message_error)?;
                Ok(self.done())
            }
            WhatsappOp::DeleteMessage { message } => {
                let (_, message) = self.message(message, Verb::Modify).await?;
                let full = complete(&message)?;
                let engine = self.authorized().await?;
                engine
                    .engine
                    .client
                    .delete_message(engine.session, &full)
                    .await
                    .map_err(message_error)?;
                Ok(self.done())
            }
        }
    }

    /// The message a sending answers: it must be of the chat the sending goes to.
    async fn reply_in(
        &self,
        destination: &Destination,
        given: &MessageRef,
    ) -> Result<EngineMessageId, Refusal> {
        let (chat, message) = token::open(self.key, given).ok_or(ErrorCode::Ar712)?;
        if destination.chat != Some(chat) {
            return Err(ErrorCode::Ar711.into());
        }
        if let Some(full) = message.full()
            && full.chat() != &destination.engine_chat
            && !self.engine_ids(chat).await?.contains(full.chat())
        {
            return Err(ErrorCode::Ar712.into());
        }
        Ok(message)
    }

    async fn status(&self) -> Answer {
        self.anywhere(Verb::Read)?;
        let state = self.access.number.state;
        let mut limits = Value::Null;
        // The limits come from the engine; a number that is not connected has none to say,
        // and an engine that does not answer leaves them unknown.
        if state.serves_calls()
            && let Ok(engine) = self.authorized().await
            && let Ok(Some(info)) = engine.engine.client.session(engine.session).await
        {
            limits = json!({
                "message_capping": info.limits.message_capping,
                "reachout_timelock": info.limits.reachout_timelock,
            });
        }
        Ok(json!({
            "number": self.alias(),
            "state": state.as_str(),
            "connected": state.serves_calls(),
            "whatsapp_limits": limits,
        }))
    }

    /// The chats of the hub of what a listing of the engine gave, in its order: the ones the
    /// hub has, and, with the scope "all", the new ones, created now. With the scope "list" a
    /// chat the hub does not have has no line, and is left out.
    async fn chats_of(&self, listed: &[EngineChatId]) -> Result<Vec<Option<Uuid>>, Refusal> {
        let known = repo::chat::find_many(
            self.hub.pool(),
            self.organization,
            self.key,
            self.access.number.id,
            listed,
        )
        .await
        .map_err(|error| failed(&error, "the chats of a listing could not be read"))?;
        let mut chats = Vec::with_capacity(listed.len());
        for (index, engine_chat) in listed.iter().enumerate() {
            let chat = match known.get(&index) {
                Some(chat) => Some(*chat),
                None if self.access.grant.scope == Scope::All => Some(
                    self.chat_of(&Destination {
                        chat: None,
                        engine_chat: engine_chat.clone(),
                    })
                    .await?,
                ),
                None => None,
            };
            chats.push(chat);
        }
        Ok(chats)
    }

    /// Of the chats of a listing, the ones this Hermes may read.
    async fn readable(&self, listed: &[EngineChatId]) -> Result<Vec<Option<Uuid>>, Refusal> {
        let lines = repo::access::chat_lines(self.hub.pool(), self.organization, self.access.id)
            .await
            .map_err(|error| failed(&error, "the lines of an access could not be read"))?;
        Ok(self
            .chats_of(listed)
            .await?
            .into_iter()
            .map(|chat| {
                chat.filter(|chat| {
                    self.access
                        .grant
                        .in_chat(lines.get(chat).copied(), Verb::Read)
                        .is_ok()
                })
            })
            .collect())
    }

    async fn list_chats(&self, listing: Listing) -> Answer {
        self.anywhere(Verb::Read)?;
        let engine = self.authorized().await?;
        let page = Page::new(listing.limit, listing.offset)?;
        let listed = engine.engine.client.chats(engine.session, page).await?;
        let ids: Vec<EngineChatId> = listed.iter().map(|chat| chat.id.clone()).collect();
        let readable = self.readable(&ids).await?;
        let mut budget = Budget::new();
        let mut chats = Vec::new();
        let mut next = None;
        for (index, (summary, chat)) in listed.iter().zip(readable).enumerate() {
            let Some(chat) = chat else { continue };
            let item = json!({
                "chat": chat.as_hyphenated().to_string(),
                "kind": summary.id.kind().as_str(),
                "name": summary.name.as_ref().map(|name| name.expose()),
                "phone": phone_of(&summary.id),
                "last_message": summary.last.as_ref().map(|last| self.message_item(chat, last)),
            });
            if !budget.fits(&item)? {
                next = Some(index);
                break;
            }
            chats.push(item);
        }
        let full_page = listed.len() >= usize::try_from(listing.limit).unwrap_or(usize::MAX);
        let next_offset = match next {
            Some(index) => Some(offset_after(listing.offset, index)),
            None if full_page => Some(offset_after(listing.offset, listed.len())),
            None => None,
        };
        Ok(json!({
            "number": self.alias(),
            "chats": chats,
            "more": next_offset.is_some(),
            "next_offset": next_offset,
        }))
    }

    async fn list_groups(&self, listing: Listing) -> Answer {
        self.anywhere(Verb::Read)?;
        let engine = self.authorized().await?;
        let page = Page::new(listing.limit, listing.offset)?;
        let listed = engine.engine.client.groups(engine.session, page).await?;
        let ids: Vec<EngineChatId> = listed.iter().map(|group| group.id.clone()).collect();
        let readable = self.readable(&ids).await?;
        let mut budget = Budget::new();
        let mut groups = Vec::new();
        let mut next = None;
        for (index, (summary, chat)) in listed.iter().zip(readable).enumerate() {
            let Some(chat) = chat else { continue };
            let item = json!({
                "chat": chat.as_hyphenated().to_string(),
                "subject": summary.subject.as_ref().map(|subject| subject.expose()),
                "participants": summary.participants,
            });
            if !budget.fits(&item)? {
                next = Some(index);
                break;
            }
            groups.push(item);
        }
        let full_page = listed.len() >= usize::try_from(listing.limit).unwrap_or(usize::MAX);
        let next_offset = match next {
            Some(index) => Some(offset_after(listing.offset, index)),
            None if full_page => Some(offset_after(listing.offset, listed.len())),
            None => None,
        };
        Ok(json!({
            "number": self.alias(),
            "groups": groups,
            "more": next_offset.is_some(),
            "next_offset": next_offset,
        }))
    }

    /// `whatsapp_read_chat`: the newest messages of the window, cut at whole messages within
    /// 256 KiB. When more remain, the page ends at a change of second, so that `next_before`
    /// (exclusive) loses none of them.
    async fn read_chat(&self, read: &ReadChat) -> Answer {
        let reached = self.chat(read.chat, Verb::Read).await?;
        let engine = self.authorized().await?;
        // One more than asked, when the engine allows: it says whether more remain, and in
        // which second the next one is.
        let asked = usize::try_from(read.limit).unwrap_or(usize::MAX);
        let query = MessageQuery {
            page: Page::new(read.limit.saturating_add(1).min(PAGE_MAX), 0)?,
            before_s: read.before,
            after_s: read.after,
        };
        let messages = engine
            .engine
            .client
            .messages(engine.session, &reached.engine_chat, query)
            .await?;
        let mut budget = Budget::new();
        let mut kept: Vec<(i64, Value)> = Vec::with_capacity(messages.len().min(asked));
        for message in messages.iter().take(asked) {
            let item = self.message_item(reached.chat, message);
            if !budget.fits(&item)? {
                break;
            }
            kept.push((message.at_ms.div_euclid(1_000), item));
        }
        let more = messages.len() > kept.len()
            || (messages.len() >= asked && read.limit.saturating_add(1) > PAGE_MAX);
        let mut next_before = None;
        if more && let Some(&(oldest, _)) = kept.last() {
            // The second of the first message left out; unknown when the engine gave no more
            // than was asked.
            let next = messages
                .get(kept.len())
                .map(|message| message.at_ms.div_euclid(1_000));
            let shares_the_second = next.is_none_or(|next| next == oldest);
            if shares_the_second && kept.iter().any(|(second, _)| *second > oldest) {
                // The messages of the oldest second go on past this page: they come whole in
                // the next one.
                while kept.last().is_some_and(|(second, _)| *second == oldest) {
                    kept.pop();
                }
            }
            next_before = kept.last().map(|(second, _)| *second);
        }
        let messages: Vec<Value> = kept.into_iter().map(|(_, item)| item).collect();
        Ok(json!({
            "number": self.alias(),
            "chat": reached.chat.as_hyphenated().to_string(),
            "messages": messages,
            "more": more,
            "next_before": next_before,
        }))
    }

    async fn contact(&self, chat: Uuid) -> Answer {
        let reached = self.chat(chat, Verb::Read).await?;
        if reached.engine_chat.kind() != ChatKind::Contact {
            return Err(ErrorCode::Ar711.into());
        }
        let engine = self.authorized().await?;
        let contact = engine
            .engine
            .client
            .contact(engine.session, &reached.engine_chat)
            .await?;
        Ok(json!({
            "number": self.alias(),
            "chat": reached.chat.as_hyphenated().to_string(),
            "phone": phone_of(&reached.engine_chat),
            "name": contact.name.as_ref().map(|text| text.expose()),
            "push_name": contact.push_name.as_ref().map(|text| text.expose()),
            "about": contact.about.as_ref().map(|text| text.expose()),
            "picture_url": contact.picture_url,
            "is_business": contact.is_business,
        }))
    }

    async fn download(&self, given: &MessageRef) -> Answer {
        let (reached, message) = self.message(given, Verb::Read).await?;
        let engine = self.authorized().await?;
        let file = engine
            .engine
            .client
            .download_media(engine.session, &reached.engine_chat, &message)
            .await
            .map_err(message_error)?;
        Ok(json!({
            "number": self.alias(),
            "mimetype": file.mimetype.as_str(),
            "filename": file.filename.as_ref().map(FileName::as_str),
            "size": file.data.len(),
            "data": BASE64.encode(&file.data),
        }))
    }

    async fn group(&self, chat: Uuid, participants: Listing) -> Answer {
        let reached = self.chat(chat, Verb::Read).await?;
        if reached.engine_chat.kind() != ChatKind::Group {
            return Err(ErrorCode::Ar711.into());
        }
        let engine = self.authorized().await?;
        let page = Page::new(participants.limit, participants.offset)?;
        let group = engine
            .engine
            .client
            .group(engine.session, &reached.engine_chat, page)
            .await?;
        let listed: Vec<Value> = group
            .participants
            .iter()
            .map(|participant| {
                json!({
                    "phone": phone_of(&participant.id),
                    "role": role(participant.role),
                })
            })
            .collect();
        Ok(json!({
            "number": self.alias(),
            "chat": reached.chat.as_hyphenated().to_string(),
            "subject": group.subject.as_ref().map(|text| text.expose()),
            "description": group.description.as_ref().map(|text| text.expose()),
            "total": group.total,
            "participants": listed,
            "next_offset": group.next_offset,
        }))
    }

    /// A message as a reading shows it. The text, the name of the file and the author are of
    /// the other side: they go to the Hermes, which the descriptions tell they are data.
    fn message_item(&self, chat: Uuid, message: &Message) -> Value {
        let (from_me, author) = match &message.author {
            Author::Me => (true, None),
            Author::Other(id) => (false, phone_of(id)),
            Author::Unknown => (false, None),
        };
        json!({
            "message_id": token::sign(self.key, chat, &message.id),
            "time": message.at_ms.div_euclid(1_000),
            "from_me": from_me,
            "author": author,
            "text": message.body.expose(),
            "file": message.media.as_ref().map(|media| json!({
                "mimetype": media.mimetype.as_ref().map(MimeType::as_str),
                "filename": media.filename.as_ref().map(FileName::as_str),
            })),
            "reply_to": message
                .reply_to
                .as_ref()
                .map(|reply| token::sign(self.key, chat, reply)),
        })
    }
}

/// Editing and deleting ask for the complete identifier (`docs/09-contrato-mcp.md`,
/// section 4): one the engine gave alone is refused before the engine.
fn complete(message: &EngineMessageId) -> Result<FullMessageId, Refusal> {
    message.full().ok_or_else(|| ErrorCode::Ar711.into())
}

/// What the engine says about a message it does not have, or no longer lets be changed: the
/// message that does not exist.
fn message_error(error: EngineError) -> Refusal {
    match error {
        EngineError::NotFound | EngineError::Refused => ErrorCode::Ar712.into(),
        other => other.into(),
    }
}

/// `+digits` for a person whose identifier is a telephone; nothing for a hidden one.
fn phone_of(id: &EngineChatId) -> Option<String> {
    id.phone().map(|phone| format!("+{}", phone.digits()))
}

const fn role(role: Role) -> &'static str {
    match role {
        Role::Participant => "participant",
        Role::Admin => "admin",
        Role::SuperAdmin => "creator",
        Role::Left => "left",
    }
}

fn offset_after(offset: u32, index: usize) -> u32 {
    offset.saturating_add(u32::try_from(index).unwrap_or(u32::MAX))
}

/// The bytes of a reading, counted item by item.
struct Budget {
    used: usize,
}

impl Budget {
    const fn new() -> Self {
        Self { used: 0 }
    }

    /// Whether `item` still fits; it is counted when it does. The first item that does not
    /// fit alone is AR-709: there is nothing smaller to answer with.
    fn fits(&mut self, item: &Value) -> Result<bool, Refusal> {
        let size = serde_json::to_vec(item).map_or(usize::MAX, |bytes| bytes.len());
        let room = WHATSAPP_READ_BUDGET_BYTES.saturating_sub(ENVELOPE_BYTES);
        let after = self.used.saturating_add(size).saturating_add(1);
        if after > room {
            if self.used == 0 {
                return Err(ErrorCode::Ar709.into());
            }
            return Ok(false);
        }
        self.used = after;
        Ok(true)
    }
}
