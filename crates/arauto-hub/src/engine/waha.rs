//! The adapter of the WAHA (`referencia/waha-fatos.md`, the section on the formats).
//!
//! Every path is put together from segments checked by their types ([`super::types`]), with
//! `@` written `%40`, as the documentation of the engine asks; every value of a query is
//! encoded whole. Every answer is read with the ceiling of its call and parsed into typed
//! values; an item of a list that is outside its format is left out, and an answer that is not
//! of the documented shape makes the engine count as unavailable for that call. Nothing of an
//! answer goes into an error or a log.

use std::collections::BTreeSet;

use data_encoding::BASE64;
use futures_util::future::BoxFuture;
use hyper::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{Answer, Call, Deadlines, Expect, HttpClient, JSON_ANSWER_MAX};
use super::config::{ApiKey, EngineAddress};
use super::error::{Cause, EngineError};
use super::types::*;
use super::{SessionConfig, WhatsappEngine};

/// The events each session of the engine sends to the hub.
pub const EVENTS: &[&str] = &[
    "session.status",
    "message.any",
    "message.reaction",
    "message.edited",
    "message.revoked",
];

/// The name with which the connected device shows on the telephone.
const DEVICE_NAME: &str = "Arauto";

/// How the engine tries again an event the hub did not take: it gives up after a few
/// minutes, and the catch-up at the start of the hub covers the rest.
const RETRY_POLICY: &str = "exponential";
const RETRY_DELAY_SECONDS: u32 = 2;
const RETRY_ATTEMPTS: u32 = 7;

/// The ceiling of the answer that carries the content of a file. The file itself is checked
/// against [`DOWNLOAD_MAX_BYTES`].
const MEDIA_ANSWER_MAX: usize = DOWNLOAD_MAX_BYTES;

/// The prefix of the path of a downloaded file in the engine. The address the engine gives is
/// never followed: only its path, under this prefix, is asked of the engine the hub knows.
const FILES_PREFIX: &str = "/api/files/";

/// The adapter of one WAHA.
#[derive(Debug, Clone)]
pub struct Waha {
    http: HttpClient,
}

impl Waha {
    /// An adapter for the engine at `address`, with its key.
    #[must_use]
    pub fn new(address: EngineAddress, key: Option<ApiKey>, deadlines: Deadlines) -> Self {
        Self {
            http: HttpClient::new(address, key, deadlines),
        }
    }

    async fn json(&self, call: Call) -> Result<Value, EngineError> {
        let answer = self.http.send(call).await?;
        let answer = check(answer)?;
        parse_json(&answer.body)
    }

    async fn typed<T: for<'de> Deserialize<'de>>(&self, call: Call) -> Result<T, EngineError> {
        let answer = self.http.send(call).await?;
        let answer = check(answer)?;
        serde_json::from_slice(&answer.body).map_err(|_| EngineError::malformed())
    }

    async fn done(&self, call: Call) -> Result<(), EngineError> {
        let answer = self.http.send(call).await?;
        check(answer).map(|_| ())
    }

    async fn get_session(&self, session: &SessionName) -> Result<Option<RawSession>, EngineError> {
        let call = Call::new(
            Method::GET,
            format!("/api/sessions/{}", session.segment()),
            Expect::Json(JSON_ANSWER_MAX),
        );
        match self.typed::<RawSession>(call).await {
            Ok(raw) => Ok(Some(raw)),
            Err(EngineError::NotFound) => Ok(None),
            Err(other) => Err(other),
        }
    }

    async fn ensure(
        &self,
        session: &SessionName,
        config: &SessionConfig<'_>,
        applied_before: bool,
    ) -> Result<Ensured, EngineError> {
        let wanted = desired_config(config);
        let Some(current) = self.get_session(session).await? else {
            let body = json!({"name": session.as_str(), "start": false, "config": wanted});
            self.done(
                Call::new(
                    Method::POST,
                    "/api/sessions".to_owned(),
                    Expect::Json(JSON_ANSWER_MAX),
                )
                .json(&body),
            )
            .await?;
            return Ok(Ensured::Created);
        };
        let differences = differences(current.config.as_ref(), config, applied_before);
        if differences.is_empty() {
            return Ok(Ensured::Unchanged);
        }
        // The engine replaces the whole configuration and restarts the session: it is only
        // sent when something differs, and always whole.
        let body = json!({"name": session.as_str(), "config": wanted});
        self.done(
            Call::new(
                Method::PUT,
                format!("/api/sessions/{}", session.segment()),
                Expect::Json(JSON_ANSWER_MAX),
            )
            .json(&body),
        )
        .await?;
        Ok(Ensured::Reconfigured(differences))
    }

    async fn session_action(&self, session: &SessionName, action: &str) -> Result<(), EngineError> {
        self.done(Call::new(
            Method::POST,
            format!("/api/sessions/{}/{action}", session.segment()),
            Expect::Json(JSON_ANSWER_MAX),
        ))
        .await
    }

    async fn read_messages(
        &self,
        session: &SessionName,
        chat: &EngineChatId,
        query: MessageQuery,
    ) -> Result<Vec<Message>, EngineError> {
        let mut call = Call::new(
            Method::GET,
            format!(
                "/api/{}/chats/{}/messages",
                session.segment(),
                chat.segment()
            ),
            Expect::Json(JSON_ANSWER_MAX),
        )
        .query("limit", query.page.limit().to_string())
        .query("offset", query.page.offset().to_string())
        .query("downloadMedia", "false");
        if let Some(before) = query.before_s {
            call = call.query("filter.timestamp.lte", before.saturating_sub(1).to_string());
        }
        if let Some(after) = query.after_s {
            call = call.query("filter.timestamp.gte", after.to_string());
        }
        let value = self.json(call).await?;
        let items = value.as_array().ok_or(EngineError::malformed())?;
        let mut messages: Vec<Message> = items
            .iter()
            .filter_map(|item| parse_message(item, Some(chat)))
            .collect();
        // The newest first, whatever order the engine used.
        messages.sort_by_key(|message| std::cmp::Reverse(message.at_ms));
        messages.truncate(usize::try_from(query.page.limit()).unwrap_or(usize::MAX));
        Ok(messages)
    }

    async fn read_contact(
        &self,
        session: &SessionName,
        contact: &EngineChatId,
    ) -> Result<Contact, EngineError> {
        let ask = |path: &str| {
            Call::new(Method::GET, path.to_owned(), Expect::Json(JSON_ANSWER_MAX))
                .query("contactId", contact.as_str())
                .query("session", session.as_str())
        };
        let raw: RawContact = self.typed(ask("/api/contacts")).await?;
        let about = match self.typed::<RawAbout>(ask("/api/contacts/about")).await {
            Ok(raw) => raw.about.as_deref().map(ShownText::from_engine),
            Err(EngineError::NotFound) => None,
            Err(other) => return Err(other),
        };
        let picture_url = match self
            .typed::<RawPicture>(ask("/api/contacts/profile-picture"))
            .await
        {
            Ok(raw) => raw.profile_picture_url.and_then(|url| public_url(&url)),
            Err(EngineError::NotFound) => None,
            Err(other) => return Err(other),
        };
        Ok(Contact {
            id: contact.clone(),
            name: raw.name.as_deref().map(ShownText::from_engine),
            push_name: raw.pushname.as_deref().map(ShownText::from_engine),
            about,
            picture_url,
            is_business: None,
        })
    }

    async fn download(
        &self,
        session: &SessionName,
        chat: &EngineChatId,
        message: &EngineMessageId,
    ) -> Result<FileContent, EngineError> {
        let call = Call::new(
            Method::GET,
            format!(
                "/api/{}/chats/{}/messages/{}",
                session.segment(),
                chat.segment(),
                message.segment()
            ),
            Expect::Json(JSON_ANSWER_MAX),
        )
        .query("downloadMedia", "true");
        let value = self.json(call).await?;
        let media = value.get("media").filter(|media| !media.is_null());
        let Some(media) = media else {
            return Err(EngineError::NotFound);
        };
        let url = media
            .get("url")
            .and_then(Value::as_str)
            .ok_or(EngineError::NotFound)?;
        let path = files_path(url).ok_or(EngineError::malformed())?;
        let mimetype = media
            .get("mimetype")
            .and_then(Value::as_str)
            .and_then(|text| MimeType::parse(text).ok());
        let filename = media
            .get("filename")
            .and_then(Value::as_str)
            .and_then(|text| FileName::parse(text).ok());
        let answer = self
            .http
            .send(Call::new(
                Method::GET,
                path,
                Expect::Bytes(MEDIA_ANSWER_MAX),
            ))
            .await?;
        let answer = check(answer)?;
        if answer.body.len() > DOWNLOAD_MAX_BYTES {
            return Err(EngineError::Unavailable(Cause::TooLarge));
        }
        Ok(FileContent {
            mimetype: match mimetype {
                Some(mimetype) => mimetype,
                None => MimeType::parse("application/octet-stream")?,
            },
            filename,
            data: answer.body.to_vec(),
        })
    }

    async fn read_group(
        &self,
        session: &SessionName,
        group: &EngineChatId,
        page: Page,
    ) -> Result<GroupDetail, EngineError> {
        if group.kind() != ChatKind::Group {
            return Err(EngineError::Invalid("group identifier"));
        }
        let base = format!("/api/{}/groups/{}", session.segment(), group.segment());
        let value = self
            .json(Call::new(
                Method::GET,
                base.clone(),
                Expect::Json(JSON_ANSWER_MAX),
            ))
            .await?;
        let listed = self
            .json(Call::new(
                Method::GET,
                format!("{base}/participants/v2"),
                Expect::Json(JSON_ANSWER_MAX),
            ))
            .await?;
        let all: Vec<Participant> = listed
            .as_array()
            .ok_or(EngineError::malformed())?
            .iter()
            .filter_map(parse_participant)
            .collect();
        let total = u32::try_from(all.len()).unwrap_or(u32::MAX);
        let start = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let limit = usize::try_from(page.limit()).unwrap_or(usize::MAX);
        let end = start.saturating_add(limit);
        let participants: Vec<Participant> = all.iter().skip(start).take(limit).cloned().collect();
        let next_offset = (end < all.len()).then(|| u32::try_from(end).unwrap_or(u32::MAX));
        Ok(GroupDetail {
            id: group.clone(),
            subject: text_field(&value, &["subject", "name"]),
            description: text_field(&value, &["description", "desc"]),
            total,
            participants,
            next_offset,
        })
    }

    async fn check_phone(
        &self,
        session: &SessionName,
        phone: &Phone,
    ) -> Result<Option<EngineChatId>, EngineError> {
        for form in phone.forms() {
            let raw: RawCheck = self
                .typed(
                    Call::new(
                        Method::GET,
                        "/api/contacts/check-exists".to_owned(),
                        Expect::Json(JSON_ANSWER_MAX),
                    )
                    .query("phone", form.digits())
                    .query("session", session.as_str()),
                )
                .await?;
            if raw.number_exists {
                let id = raw.chat_id.as_deref().ok_or(EngineError::malformed())?;
                return EngineChatId::parse(id)
                    .map(Some)
                    .map_err(|_| EngineError::malformed());
            }
        }
        Ok(None)
    }

    async fn post_sent(&self, path: &str, body: &Value) -> Result<Sent, EngineError> {
        let answer = self
            .http
            .send(
                Call::new(Method::POST, path.to_owned(), Expect::Json(JSON_ANSWER_MAX)).json(body),
            )
            .await?;
        let answer = check(answer)?;
        // The engine said the message left. A body the hub cannot read does not undo that:
        // the sending is done, without the identifier of the new message.
        let id = serde_json::from_slice::<Value>(&answer.body)
            .ok()
            .and_then(|value| id_field(value.get("id")))
            .and_then(|id| EngineMessageId::parse(&id).ok());
        if id.is_none() {
            tracing::warn!("the engine sent a message and did not say its identifier");
        }
        Ok(Sent { id })
    }
}

/// The status of an answer, as an error of the catalog. The body of an error is never read.
fn check(answer: Answer) -> Result<Answer, EngineError> {
    let status = answer.status;
    if status.is_success() {
        return Ok(answer);
    }
    Err(match status {
        StatusCode::NOT_FOUND => EngineError::NotFound,
        StatusCode::TOO_MANY_REQUESTS => EngineError::WhatsappLimit,
        status if status.is_client_error() => EngineError::Refused,
        status => EngineError::Unavailable(Cause::Status(status.as_u16())),
    })
}

fn parse_json(body: &[u8]) -> Result<Value, EngineError> {
    serde_json::from_slice(body).map_err(|_| EngineError::malformed())
}

/// The configuration of a session of the engine, whole (`referencia/waha-fatos.md`).
pub(crate) fn desired_config(config: &SessionConfig<'_>) -> Value {
    json!({
        "webhooks": [{
            "url": config.events_url,
            "events": EVENTS,
            "hmac": {"key": config.events_secret.hmac_key().as_str()},
            "retries": {
                "policy": RETRY_POLICY,
                "delaySeconds": RETRY_DELAY_SECONDS,
                "attempts": RETRY_ATTEMPTS,
            },
            "customHeaders": [],
        }],
        // The history of groups is kept; status, channels and lists are not chats of the Arauto.
        "ignore": {"status": true, "groups": false, "channels": true, "broadcast": true},
        "client": {"deviceName": DEVICE_NAME},
        "gows": {"storage": {
            "messages": true, "groups": true, "chats": true, "labels": true, "contacts": true,
        }},
        "metadata": {},
        "debug": false,
    })
}

/// What differs between the configuration a session has and the one the hub gives it.
fn differences(
    current: Option<&RawConfig>,
    config: &SessionConfig<'_>,
    applied_before: bool,
) -> Vec<&'static str> {
    let empty = RawConfig::default();
    let current = current.unwrap_or(&empty);
    let mut differs = Vec::new();
    let webhooks = current.webhooks.as_deref().unwrap_or_default();
    match webhooks {
        [only] => {
            if only.url != config.events_url {
                differs.push("events address");
            }
            let events: BTreeSet<&str> = only.events.iter().map(String::as_str).collect();
            let wanted: BTreeSet<&str> = EVENTS.iter().copied().collect();
            if events != wanted {
                differs.push("events");
            }
            match only.hmac.as_ref().and_then(|hmac| hmac.key.as_deref()) {
                Some(key) if config.events_secret.is_hmac_key(key) => {}
                // An engine that does not show the secret back is trusted on what the hub
                // applied last time; one that shows another, or none it was given, is not.
                None if applied_before => {}
                _ => differs.push("secret of the events"),
            }
        }
        // None, or more than one: a destination that is not the hub is corrected.
        _ => differs.push("destinations of the events"),
    }
    let ignore = current.ignore.clone().unwrap_or_default();
    if ignore.status != Some(true)
        || ignore.groups.unwrap_or(false)
        || ignore.channels != Some(true)
        || ignore.broadcast != Some(true)
    {
        differs.push("what is ignored");
    }
    let device = current
        .client
        .as_ref()
        .and_then(|client| client.device_name.as_deref());
    if device != Some(DEVICE_NAME) {
        differs.push("name of the device");
    }
    let storage = current
        .gows
        .as_ref()
        .and_then(|gows| gows.storage.clone())
        .unwrap_or_default();
    let kept = [
        storage.messages,
        storage.groups,
        storage.chats,
        storage.labels,
        storage.contacts,
    ];
    if kept.iter().any(|kept| *kept != Some(true)) {
        differs.push("what the engine keeps");
    }
    if current.proxy.as_ref().is_some_and(|proxy| !proxy.is_null()) {
        differs.push("proxy");
    }
    differs
}

// ---------------------------------------------------------------------------- the shapes of the answers

#[derive(Debug, Deserialize)]
struct RawSession {
    status: String,
    #[serde(default)]
    me: Option<RawMe>,
    #[serde(default)]
    config: Option<RawConfig>,
}

#[derive(Debug, Deserialize)]
struct RawMe {
    id: String,
    #[serde(default, rename = "reachoutTimelock")]
    reachout_timelock: Option<Value>,
    #[serde(default, rename = "messageCapping")]
    message_capping: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    webhooks: Option<Vec<RawWebhook>>,
    #[serde(default)]
    ignore: Option<RawIgnore>,
    #[serde(default)]
    client: Option<RawClient>,
    #[serde(default)]
    gows: Option<RawGows>,
    #[serde(default)]
    proxy: Option<Value>,
}

#[derive(Deserialize)]
struct RawWebhook {
    url: String,
    #[serde(default)]
    events: Vec<String>,
    #[serde(default)]
    hmac: Option<RawHmac>,
}

impl std::fmt::Debug for RawWebhook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RawWebhook([redacted])")
    }
}

#[derive(Deserialize)]
struct RawHmac {
    #[serde(default)]
    key: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawIgnore {
    #[serde(default)]
    status: Option<bool>,
    #[serde(default)]
    groups: Option<bool>,
    #[serde(default)]
    channels: Option<bool>,
    #[serde(default)]
    broadcast: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawClient {
    #[serde(default, rename = "deviceName")]
    device_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawGows {
    #[serde(default)]
    storage: Option<RawStorage>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawStorage {
    #[serde(default)]
    messages: Option<bool>,
    #[serde(default)]
    groups: Option<bool>,
    #[serde(default)]
    chats: Option<bool>,
    #[serde(default)]
    labels: Option<bool>,
    #[serde(default)]
    contacts: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawVersion {
    version: String,
    engine: String,
    #[serde(default)]
    tier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawQr {
    value: String,
}

#[derive(Debug, Deserialize)]
struct RawCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct RawContact {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    pushname: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAbout {
    #[serde(default)]
    about: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPicture {
    #[serde(default, rename = "profilePictureURL")]
    profile_picture_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCheck {
    #[serde(rename = "numberExists")]
    number_exists: bool,
    #[serde(default, rename = "chatId")]
    chat_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawHealth {
    status: String,
}

/// An identifier as the engine writes it: a text, or an object with `_serialized`.
fn id_field(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Object(object) => object
            .get("_serialized")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

fn text_field(value: &Value, keys: &[&str]) -> Option<ShownText> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ShownText::from_engine)
}

fn bounded_body(text: &str) -> MessageBody {
    MessageBody::from_engine(text)
}

/// A message of the engine. `chat` is the chat it was read from, when it was read from one;
/// otherwise the chat is the field of origin, which is always the chat
/// (`referencia/waha-fatos.md`). An item outside its format is `None`.
pub(crate) fn parse_message(value: &Value, chat: Option<&EngineChatId>) -> Option<Message> {
    let id = EngineMessageId::parse(&id_field(value.get("id"))?).ok()?;
    let from_me = value
        .get("fromMe")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let chat = match chat {
        Some(chat) => chat.clone(),
        None => EngineChatId::parse(value.get("from")?.as_str()?).ok()?,
    };
    let seconds = value.get("timestamp").and_then(Value::as_i64)?;
    if seconds < 0 {
        return None;
    }
    let at_ms = seconds.checked_mul(1_000)?;
    let author = if from_me {
        Author::Me
    } else if chat.kind() == ChatKind::Group {
        match value
            .get("participant")
            .and_then(Value::as_str)
            .and_then(|text| EngineChatId::parse(text).ok())
        {
            Some(participant) => Author::Other(participant),
            None => Author::Unknown,
        }
    } else {
        Author::Other(chat.clone())
    };
    let body = bounded_body(
        value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let media = if value
        .get("hasMedia")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let media = value.get("media");
        Some(MediaInfo {
            mimetype: media
                .and_then(|media| media.get("mimetype"))
                .and_then(Value::as_str)
                .and_then(|text| MimeType::parse(text).ok()),
            filename: media
                .and_then(|media| media.get("filename"))
                .and_then(Value::as_str)
                .and_then(|text| FileName::parse(text).ok()),
        })
    } else {
        None
    };
    let reply_to = value
        .get("replyTo")
        .and_then(|reply| match reply {
            Value::String(_) => id_field(Some(reply)),
            Value::Object(_) => id_field(reply.get("id")),
            _ => None,
        })
        .and_then(|id| EngineMessageId::parse(&id).ok());
    Some(Message {
        id,
        chat,
        at_ms,
        author,
        body,
        media,
        reply_to,
    })
}

fn parse_participant(value: &Value) -> Option<Participant> {
    let id = EngineChatId::parse(&id_field(value.get("id"))?).ok()?;
    let role = match value
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("participant")
    {
        "participant" => Role::Participant,
        "admin" => Role::Admin,
        "superadmin" => Role::SuperAdmin,
        "left" => Role::Left,
        _ => return None,
    };
    Some(Participant { id, role })
}

fn parse_group(value: &Value) -> Option<GroupSummary> {
    let id = EngineChatId::parse(&id_field(value.get("id"))?).ok()?;
    if id.kind() != ChatKind::Group {
        return None;
    }
    // The count of the engine reads zero: the list is counted.
    let participants = value
        .get("participants")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    Some(GroupSummary {
        id,
        subject: text_field(value, &["subject", "name"]),
        participants: u32::try_from(participants).unwrap_or(u32::MAX),
    })
}

/// The path of a downloaded file, from the address the engine gave: only a path under
/// `/api/files/`, of a narrow alphabet (the name of a file carries the identifier of its
/// message, with `@`, which goes as `%40`), with no dot segment. The host of that address is
/// not followed.
fn files_path(url: &str) -> Option<String> {
    let path = match url.find("://") {
        Some(scheme) => {
            let rest = url.get(scheme.saturating_add(3)..)?;
            rest.get(rest.find('/')?..)?
        }
        None => url,
    };
    let ok = path.len() <= 512
        && path.starts_with(FILES_PREFIX)
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-' | b'@'))
        && !path.contains("//")
        && path
            .split('/')
            .all(|segment| segment != "." && segment != "..");
    ok.then(|| path.replace('@', "%40"))
}

/// An address of a picture on the servers of WhatsApp: `https://`, up to 2,048 visible ASCII
/// characters.
fn public_url(url: &str) -> Option<String> {
    (url.starts_with("https://") && url.len() <= 2_048 && url.bytes().all(|b| b.is_ascii_graphic()))
        .then(|| url.to_owned())
}

fn word_ok(text: &str, max: usize) -> bool {
    !text.is_empty()
        && text.len() <= max
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'_' | b'-'))
}

fn file_body(session: &SessionName, file: &SendFile) -> Result<(&'static str, Value), EngineError> {
    if file.file.data.is_empty() || file.file.data.len() > SEND_FILE_MAX_BYTES {
        return Err(EngineError::Invalid("file size"));
    }
    let filename = file
        .file
        .filename
        .as_ref()
        .map_or("file", |name| name.as_str());
    let mut body = json!({
        "session": session.as_str(),
        "chatId": file.chat.as_str(),
        "file": {
            "mimetype": file.file.mimetype.as_str(),
            "filename": filename,
            "data": BASE64.encode(&file.file.data),
        },
    });
    let path = match file.kind {
        FileKind::Image => "/api/sendImage",
        FileKind::Document => "/api/sendFile",
        FileKind::Voice => {
            body["convert"] = Value::Bool(false);
            "/api/sendVoice"
        }
    };
    if let Some(caption) = &file.caption {
        if file.kind == FileKind::Voice {
            return Err(EngineError::Invalid("caption"));
        }
        body["caption"] = Value::String(caption.expose().to_owned());
    }
    Ok((path, body))
}

impl WhatsappEngine for Waha {
    fn health(&self) -> BoxFuture<'_, Result<(), EngineError>> {
        Box::pin(async move {
            let raw: RawHealth = self
                .typed(Call::new(
                    Method::GET,
                    "/health".to_owned(),
                    Expect::Json(64 * 1024),
                ))
                .await?;
            if raw.status == "ok" {
                Ok(())
            } else {
                Err(EngineError::Unavailable(Cause::Status(503)))
            }
        })
    }

    fn version(&self) -> BoxFuture<'_, Result<EngineVersion, EngineError>> {
        Box::pin(async move {
            let raw: RawVersion = self
                .typed(Call::new(
                    Method::GET,
                    "/api/server/version".to_owned(),
                    Expect::Json(64 * 1024),
                ))
                .await?;
            let tier = raw.tier.unwrap_or_default();
            if !word_ok(&raw.version, 64)
                || !word_ok(&raw.engine, 32)
                || !(tier.is_empty() || word_ok(&tier, 32))
            {
                return Err(EngineError::malformed());
            }
            Ok(EngineVersion {
                version: raw.version,
                engine: raw.engine,
                tier,
            })
        })
    }

    fn config_digest(&self, config: &SessionConfig<'_>) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let text = zeroize::Zeroizing::new(desired_config(config).to_string());
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&Sha256::digest(text.as_bytes()));
        digest
    }

    fn session<'a>(
        &'a self,
        session: &'a SessionName,
    ) -> BoxFuture<'a, Result<Option<SessionInfo>, EngineError>> {
        Box::pin(async move {
            let Some(raw) = self.get_session(session).await? else {
                return Ok(None);
            };
            let status = EngineStatus::parse(&raw.status).ok_or(EngineError::malformed())?;
            let (me, limits) = match raw.me {
                Some(me) => {
                    let id = EngineChatId::parse(&me.id).map_err(|_| EngineError::malformed())?;
                    let limits = WhatsappLimits {
                        message_capping: me.message_capping.is_some_and(|value| !value.is_null()),
                        reachout_timelock: me
                            .reachout_timelock
                            .is_some_and(|value| !value.is_null()),
                    };
                    let phone = id.phone();
                    (Some(Me { id, phone }), limits)
                }
                None => (None, WhatsappLimits::default()),
            };
            Ok(Some(SessionInfo { status, me, limits }))
        })
    }

    fn ensure_session<'a>(
        &'a self,
        session: &'a SessionName,
        config: &'a SessionConfig<'a>,
        applied_before: bool,
    ) -> BoxFuture<'a, Result<Ensured, EngineError>> {
        Box::pin(self.ensure(session, config, applied_before))
    }

    fn start<'a>(&'a self, session: &'a SessionName) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(self.session_action(session, "start"))
    }

    fn stop<'a>(&'a self, session: &'a SessionName) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(self.session_action(session, "stop"))
    }

    fn restart<'a>(&'a self, session: &'a SessionName) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(self.session_action(session, "restart"))
    }

    fn log_out_and_delete<'a>(
        &'a self,
        session: &'a SessionName,
    ) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(async move {
            match self.session_action(session, "logout").await {
                Ok(()) | Err(EngineError::NotFound) => {}
                Err(other) => return Err(other),
            }
            match self
                .done(Call::new(
                    Method::DELETE,
                    format!("/api/sessions/{}", session.segment()),
                    Expect::Json(JSON_ANSWER_MAX),
                ))
                .await
            {
                Ok(()) | Err(EngineError::NotFound) => Ok(()),
                Err(other) => Err(other),
            }
        })
    }

    fn pairing_qr<'a>(
        &'a self,
        session: &'a SessionName,
    ) -> BoxFuture<'a, Result<PairingQr, EngineError>> {
        Box::pin(async move {
            let raw: RawQr = self
                .typed(
                    Call::new(
                        Method::GET,
                        format!("/api/{}/auth/qr", session.segment()),
                        Expect::Json(64 * 1024),
                    )
                    .query("format", "raw"),
                )
                .await?;
            PairingQr::new(raw.value)
        })
    }

    fn pairing_code<'a>(
        &'a self,
        session: &'a SessionName,
        phone: &'a Phone,
    ) -> BoxFuture<'a, Result<PairingCode, EngineError>> {
        Box::pin(async move {
            let raw: RawCode = self
                .typed(
                    Call::new(
                        Method::POST,
                        format!("/api/{}/auth/request-code", session.segment()),
                        Expect::Json(64 * 1024),
                    )
                    .json(&json!({"phoneNumber": phone.digits()})),
                )
                .await?;
            PairingCode::new(raw.code)
        })
    }

    fn chats<'a>(
        &'a self,
        session: &'a SessionName,
        page: Page,
    ) -> BoxFuture<'a, Result<Vec<ChatSummary>, EngineError>> {
        Box::pin(async move {
            let value = self
                .json(
                    Call::new(
                        Method::GET,
                        format!("/api/{}/chats/overview", session.segment()),
                        Expect::Json(JSON_ANSWER_MAX),
                    )
                    .query("limit", page.limit().to_string())
                    .query("offset", page.offset().to_string()),
                )
                .await?;
            let items = value.as_array().ok_or(EngineError::malformed())?;
            Ok(items
                .iter()
                .filter_map(|item| {
                    let id = EngineChatId::parse(&id_field(item.get("id"))?).ok()?;
                    let last = item
                        .get("lastMessage")
                        .filter(|last| !last.is_null())
                        .and_then(|last| parse_message(last, Some(&id)));
                    Some(ChatSummary {
                        name: text_field(item, &["name"]),
                        last,
                        id,
                    })
                })
                .collect())
        })
    }

    fn messages<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
        query: MessageQuery,
    ) -> BoxFuture<'a, Result<Vec<Message>, EngineError>> {
        Box::pin(self.read_messages(session, chat, query))
    }

    fn contact<'a>(
        &'a self,
        session: &'a SessionName,
        contact: &'a EngineChatId,
    ) -> BoxFuture<'a, Result<Contact, EngineError>> {
        Box::pin(self.read_contact(session, contact))
    }

    fn download_media<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
        message: &'a EngineMessageId,
    ) -> BoxFuture<'a, Result<FileContent, EngineError>> {
        Box::pin(self.download(session, chat, message))
    }

    fn groups<'a>(
        &'a self,
        session: &'a SessionName,
        page: Page,
    ) -> BoxFuture<'a, Result<Vec<GroupSummary>, EngineError>> {
        Box::pin(async move {
            let value = self
                .json(
                    Call::new(
                        Method::GET,
                        format!("/api/{}/groups", session.segment()),
                        Expect::Json(JSON_ANSWER_MAX),
                    )
                    .query("limit", page.limit().to_string())
                    .query("offset", page.offset().to_string()),
                )
                .await?;
            // The shape depends on the internal mechanism: a list, or an object by identifier.
            let items: Vec<&Value> = match &value {
                Value::Array(items) => items.iter().collect(),
                Value::Object(items) => items.values().collect(),
                _ => return Err(EngineError::malformed()),
            };
            let mut groups: Vec<GroupSummary> = items.into_iter().filter_map(parse_group).collect();
            groups.truncate(usize::try_from(page.limit()).unwrap_or(usize::MAX));
            Ok(groups)
        })
    }

    fn group<'a>(
        &'a self,
        session: &'a SessionName,
        group: &'a EngineChatId,
        participants: Page,
    ) -> BoxFuture<'a, Result<GroupDetail, EngineError>> {
        Box::pin(self.read_group(session, group, participants))
    }

    fn check_number<'a>(
        &'a self,
        session: &'a SessionName,
        phone: &'a Phone,
    ) -> BoxFuture<'a, Result<Option<EngineChatId>, EngineError>> {
        Box::pin(self.check_phone(session, phone))
    }

    fn send_text<'a>(
        &'a self,
        session: &'a SessionName,
        text: &'a SendText,
    ) -> BoxFuture<'a, Result<Sent, EngineError>> {
        Box::pin(async move {
            if text.text.expose().trim().is_empty() {
                return Err(EngineError::Invalid("message text"));
            }
            let mut body = json!({
                "session": session.as_str(),
                "chatId": text.chat.as_str(),
                "text": text.text.expose(),
            });
            if let Some(reply) = &text.reply_to {
                body["reply_to"] = Value::String(reply.as_str().to_owned());
            }
            if !text.mentions.is_empty() {
                if text.chat.kind() != ChatKind::Group || text.mentions.len() > 256 {
                    return Err(EngineError::Invalid("mentions"));
                }
                body["mentions"] = Value::Array(
                    text.mentions
                        .iter()
                        .map(|mention| Value::String(mention.as_str().to_owned()))
                        .collect(),
                );
            }
            self.post_sent("/api/sendText", &body).await
        })
    }

    fn send_file<'a>(
        &'a self,
        session: &'a SessionName,
        file: &'a SendFile,
    ) -> BoxFuture<'a, Result<Sent, EngineError>> {
        Box::pin(async move {
            let (path, body) = file_body(session, file)?;
            self.post_sent(path, &body).await
        })
    }

    fn mark_read<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
    ) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(async move {
            // Without the list of messages, the engine marks every unread one of the chat.
            let body = json!({"session": session.as_str(), "chatId": chat.as_str()});
            self.done(
                Call::new(
                    Method::POST,
                    "/api/sendSeen".to_owned(),
                    Expect::Json(JSON_ANSWER_MAX),
                )
                .json(&body),
            )
            .await
        })
    }

    fn set_typing<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
        on: bool,
    ) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(async move {
            let path = if on {
                "/api/startTyping"
            } else {
                "/api/stopTyping"
            };
            let body = json!({"session": session.as_str(), "chatId": chat.as_str()});
            self.done(
                Call::new(Method::POST, path.to_owned(), Expect::Json(JSON_ANSWER_MAX)).json(&body),
            )
            .await
        })
    }

    fn react<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a EngineMessageId,
        reaction: &'a Reaction,
    ) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(async move {
            let body = json!({
                "session": session.as_str(),
                "messageId": message.as_str(),
                "reaction": reaction.as_str(),
            });
            // The documentation insists: the reaction is a PUT.
            self.done(
                Call::new(
                    Method::PUT,
                    "/api/reaction".to_owned(),
                    Expect::Json(JSON_ANSWER_MAX),
                )
                .json(&body),
            )
            .await
        })
    }

    fn forward<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a EngineMessageId,
        to: &'a EngineChatId,
    ) -> BoxFuture<'a, Result<Sent, EngineError>> {
        Box::pin(async move {
            let body = json!({
                "session": session.as_str(),
                "chatId": to.as_str(),
                "messageId": message.as_str(),
            });
            self.post_sent("/api/forwardMessage", &body).await
        })
    }

    fn edit_message<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a FullMessageId,
        text: &'a MessageBody,
    ) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(async move {
            // Editing without a text is dangerous in the engine: refused here, before it.
            if text.expose().trim().is_empty() {
                return Err(EngineError::Invalid("message text"));
            }
            if !message.from_me() {
                return Err(EngineError::Invalid("message of another author"));
            }
            self.done(
                Call::new(
                    Method::PUT,
                    format!(
                        "/api/{}/chats/{}/messages/{}",
                        session.segment(),
                        message.chat().segment(),
                        message.segment()
                    ),
                    Expect::Json(JSON_ANSWER_MAX),
                )
                .json(&json!({"text": text.expose()})),
            )
            .await
        })
    }

    fn delete_message<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a FullMessageId,
    ) -> BoxFuture<'a, Result<(), EngineError>> {
        Box::pin(async move {
            self.done(Call::new(
                Method::DELETE,
                format!(
                    "/api/{}/chats/{}/messages/{}",
                    session.segment(),
                    message.chat().segment(),
                    message.segment()
                ),
                Expect::Json(JSON_ANSWER_MAX),
            ))
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_of_a_file_is_only_under_the_files_of_the_engine() {
        assert_eq!(
            files_path("http://waha:3000/api/files/s1/abc.jpeg").as_deref(),
            Some("/api/files/s1/abc.jpeg")
        );
        assert_eq!(
            files_path("https://elsewhere.example.com/api/files/x.pdf").as_deref(),
            Some("/api/files/x.pdf")
        );
        assert_eq!(
            files_path("http://waha:3000/api/files/true_5511900000000@c.us_3EB0AA.jpg").as_deref(),
            Some("/api/files/true_5511900000000%40c.us_3EB0AA.jpg")
        );
        for wrong in [
            "http://waha:3000/api/sessions",
            "http://waha:3000/api/files/../sessions",
            "http://waha:3000/api/files/a//b",
            "http://waha:3000/api/files/a%2e%2e",
            "http://waha:3000/api/files/a?x=1",
            "http://waha:3000",
            "/api/files/./x",
        ] {
            assert!(files_path(wrong).is_none(), "{wrong:?}");
        }
    }

    #[test]
    fn a_message_is_read_with_its_author_from_the_other_fields() {
        let chat = EngineChatId::parse("5511999990000@c.us").unwrap();
        let mine = json!({"id": "true_5511999990000@c.us_3EB0AAAA", "timestamp": 1_700_000_000,
                          "from": "5511999990000@c.us", "fromMe": true, "body": "hi"});
        let message = parse_message(&mine, None).unwrap();
        assert_eq!(message.author, Author::Me);
        assert_eq!(message.chat, chat);
        assert_eq!(message.at_ms, 1_700_000_000_000);

        let theirs = json!({"id": "false_5511999990000@c.us_3EB0BBBB", "timestamp": 1,
                            "from": "5511999990000@c.us", "fromMe": false, "body": "hello"});
        assert_eq!(
            parse_message(&theirs, None).unwrap().author,
            Author::Other(chat)
        );

        let group = json!({"id": "false_120363012345678901@g.us_3EB0CCCC_5511888880000@c.us",
                           "timestamp": 1, "from": "120363012345678901@g.us",
                           "participant": "5511888880000@c.us", "body": ""});
        assert_eq!(
            parse_message(&group, None).unwrap().author,
            Author::Other(EngineChatId::parse("5511888880000@c.us").unwrap())
        );

        for wrong in [
            json!({"id": "../x", "timestamp": 1, "from": "5511999990000@c.us"}),
            json!({"id": "3EB0AAAA", "timestamp": -1, "from": "5511999990000@c.us"}),
            json!({"id": "3EB0AAAA", "timestamp": i64::MAX, "from": "5511999990000@c.us"}),
            json!({"id": "3EB0AAAA", "from": "5511999990000@c.us"}),
            json!({"id": "3EB0AAAA", "timestamp": 1, "from": "status@broadcast"}),
        ] {
            assert!(parse_message(&wrong, None).is_none(), "{wrong}");
        }
    }

    #[test]
    fn the_participants_are_counted_by_the_list() {
        let group = json!({"id": "120363012345678901@g.us", "subject": "Team",
                           "participantsCount": 0,
                           "participants": [{"id": "1@c.us"}, {"id": "2@c.us"}, {"id": "3@c.us"}]});
        assert_eq!(parse_group(&group).unwrap().participants, 3);
        let id_object = json!({"id": {"_serialized": "120363012345678901@g.us"}});
        assert_eq!(parse_group(&id_object).unwrap().participants, 0);
        assert!(parse_group(&json!({"id": "5511@c.us"})).is_none());
    }

    #[test]
    fn a_long_text_is_cut_on_a_character_boundary() {
        let text = format!("{}\u{e9}", "a".repeat(MESSAGE_TEXT_MAX_BYTES - 1));
        let body = bounded_body(&text);
        assert_eq!(body.expose().len(), MESSAGE_TEXT_MAX_BYTES - 1);
        assert_eq!(bounded_body("a\0b").expose(), "ab");
    }
}
