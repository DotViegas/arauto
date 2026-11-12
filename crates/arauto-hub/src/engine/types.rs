//! The types of the requests to the engine and of its answers.
//!
//! Every identifier that becomes part of an address of the engine is checked here, by its
//! type, before any request is put together: a session name, a chat, a message, a telephone.
//! What the checks guarantee is that a segment never carries a slash, a dot segment, a percent
//! sign, a NUL, a control character or an absurd length, so a value written by a Hermes or by
//! the other side of a chat never reaches another path of the engine (`docs/09-contrato-mcp.md`,
//! section 4).
//!
//! Telephones, identifiers of chats and messages (which carry telephones) and texts of
//! messages are personal data of third parties: their `Debug` prints `[redacted]`, they have
//! no `Display`, and no error carries them.

use std::fmt;

use zeroize::Zeroizing;

use super::error::EngineError;

/// Longest identifier of a chat or of a person in the engine.
pub const ENGINE_ID_MAX_LEN: usize = 64;

/// Longest identifier of a message in the engine, in its complete form.
pub const MESSAGE_ID_MAX_LEN: usize = 200;

/// Longest text of a message, in bytes.
pub const MESSAGE_TEXT_MAX_BYTES: usize = 65_536;

/// Largest file sent by content (`docs/09-contrato-mcp.md`, section 3).
pub const SEND_FILE_MAX_BYTES: usize = 3 * 1024 * 1024;

/// Largest file downloaded from a message (`docs/09-contrato-mcp.md`, section 3).
pub const DOWNLOAD_MAX_BYTES: usize = 7 * 1024 * 1024;

/// Most items of one page asked of the engine.
pub const PAGE_MAX: u32 = 200;

/// Longest name or text shown by the other side kept from an answer, in bytes.
pub const SHOWN_TEXT_MAX_BYTES: usize = 1_024;

macro_rules! redacted_debug {
    ($name:ident) => {
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}

/// Encodes a checked identifier for one segment of a path. The only character of the checked
/// alphabets that is not left as it is, is `@` (the engine asks for `%40`).
fn path_segment(text: &str) -> String {
    text.replace('@', "%40")
}

// ---------------------------------------------------------------------------- session

/// The name of a session of the engine: drawn by the hub, `a` and 26 more characters of
/// `a-z 2-7`. A Hermes never gives one (`docs/06-modelo-de-dados.md`, section 4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionName(String);

impl SessionName {
    /// Draws a new name with the generator of the system.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] when the generator fails.
    pub fn draw() -> Result<Self, EngineError> {
        let mut bytes = [0u8; 17];
        getrandom::fill(&mut bytes).map_err(|_| EngineError::Invalid("random"))?;
        let text = data_encoding::BASE32_NOPAD
            .encode(&bytes)
            .to_ascii_lowercase();
        let text: String = text.chars().take(26).collect();
        Self::parse(&format!("a{text}"))
    }

    /// Checks `^[a-z][a-z0-9]{15,62}$`: what the hub draws, and what the database accepts.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for any other text.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        let bytes = text.as_bytes();
        let starts = bytes.first().is_some_and(u8::is_ascii_lowercase);
        let rest = bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        if !(16..=63).contains(&bytes.len()) || !starts || !rest {
            return Err(EngineError::Invalid("session name"));
        }
        Ok(Self(text.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn segment(&self) -> String {
        path_segment(&self.0)
    }
}

/// The status of a session of the engine, as the engine reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    /// Stopped: the engine does not connect it.
    Stopped,
    /// Starting.
    Starting,
    /// Waiting for a QR or a code from the telephone.
    ScanQrCode,
    /// The WhatsApp asks for a passkey of the telephone.
    PasskeyRequired,
    /// Connected and working.
    Working,
    /// The engine reported a failure of the session.
    Failed,
}

impl EngineStatus {
    /// Reads the text of the engine. An unknown status is not guessed.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "STOPPED" => EngineStatus::Stopped,
            "STARTING" => EngineStatus::Starting,
            "SCAN_QR_CODE" => EngineStatus::ScanQrCode,
            "PASSKEY_REQUIRED" | "PASSKEY_CONFIRMATION_REQUIRED" => EngineStatus::PasskeyRequired,
            "WORKING" => EngineStatus::Working,
            "FAILED" => EngineStatus::Failed,
            _ => return None,
        })
    }

    /// A word for the screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            EngineStatus::Stopped => "stopped",
            EngineStatus::Starting => "starting",
            EngineStatus::ScanQrCode => "waiting for the QR or the code",
            EngineStatus::PasskeyRequired => "waiting for a passkey",
            EngineStatus::Working => "working",
            EngineStatus::Failed => "failed",
        }
    }
}

/// The own account of a session that is connected.
#[derive(Clone, PartialEq, Eq)]
pub struct Me {
    /// The identifier of the account.
    pub id: EngineChatId,
    /// The telephone, when the identifier is one.
    pub phone: Option<Phone>,
}
redacted_debug!(Me);

/// What the engine says the WhatsApp imposes on a connected session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WhatsappLimits {
    /// A ceiling on the messages the number may send.
    pub message_capping: bool,
    /// A block on writing first to someone new.
    pub reachout_timelock: bool,
}

/// A session of the engine, as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// The status.
    pub status: EngineStatus,
    /// The own account, when connected.
    pub me: Option<Me>,
    /// The limits, when the engine reports them.
    pub limits: WhatsappLimits,
}

/// What a check of the configuration of a session of the engine did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ensured {
    /// The session did not exist: it was created, already configured, and not started.
    Created,
    /// The configuration was different: the whole of it was applied. The words say what
    /// differed (never a value).
    Reconfigured(Vec<&'static str>),
    /// It was already the expected one: nothing was sent.
    Unchanged,
}

// ---------------------------------------------------------------------------- telephones and identifiers

/// A telephone with the code of the country, as digits only: from 8 to 15 (E.164).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Phone(String);
redacted_debug!(Phone);

impl Phone {
    /// Reads `+5511999990000`, `5511999990000` or `55 11 99999-0000`. Spaces, hyphens,
    /// parentheses and one leading `+` are dropped; any other character refuses the text.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        if text.len() > 32 {
            return Err(EngineError::Invalid("telephone"));
        }
        let text = text.trim();
        let text = text.strip_prefix('+').unwrap_or(text);
        let mut digits = String::with_capacity(15);
        for c in text.chars() {
            match c {
                '0'..='9' => digits.push(c),
                ' ' | '-' | '(' | ')' => {}
                _ => return Err(EngineError::Invalid("telephone")),
            }
        }
        if !(8..=15).contains(&digits.len()) || digits.starts_with('0') {
            return Err(EngineError::Invalid("telephone"));
        }
        Ok(Self(digits))
    }

    /// The digits.
    #[must_use]
    pub fn digits(&self) -> &str {
        &self.0
    }

    /// The forms to try, the one given first. A Brazilian mobile telephone exists with and
    /// without the ninth digit (`docs/09-contrato-mcp.md`, section 4): `55 DD 9XXXXXXXX` is
    /// also tried as `55 DD XXXXXXXX`, and the other way around when the local number is of a
    /// mobile (starts with 6 to 9).
    #[must_use]
    pub fn forms(&self) -> Vec<Phone> {
        let mut forms = vec![self.clone()];
        let digits = self.0.as_bytes();
        if digits.starts_with(b"55") {
            match digits.len() {
                13 if digits.get(4) == Some(&b'9') => {
                    let mut other = String::with_capacity(12);
                    other.push_str(self.0.get(..4).unwrap_or_default());
                    other.push_str(self.0.get(5..).unwrap_or_default());
                    forms.push(Phone(other));
                }
                12 if digits.get(4).is_some_and(|d| (b'6'..=b'9').contains(d)) => {
                    let mut other = String::with_capacity(13);
                    other.push_str(self.0.get(..4).unwrap_or_default());
                    other.push('9');
                    other.push_str(self.0.get(4..).unwrap_or_default());
                    forms.push(Phone(other));
                }
                _ => {}
            }
        }
        forms
    }

    /// Whether `other` is this telephone, with or without the ninth digit.
    #[must_use]
    pub fn same_as(&self, other: &Phone) -> bool {
        self.forms().iter().any(|form| form == other)
    }

    /// The identifier of the chat of a person with this telephone.
    #[must_use]
    pub fn chat_id(&self) -> EngineChatId {
        EngineChatId(format!("{}@c.us", self.0))
    }
}

/// The kind of a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    /// A person.
    Contact,
    /// A group.
    Group,
}

impl ChatKind {
    /// The text stored in `chat.kind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ChatKind::Contact => "contact",
            ChatKind::Group => "group",
        }
    }
}

/// The identifier of a chat or of a person in the engine: `digits@c.us`, `digits@lid`, the
/// same with `@s.whatsapp.net` (read as `@c.us`), or `digits-digits@g.us` for a group. Up to
/// 64 characters. Channels, lists and the status are not chats of the Arauto.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EngineChatId(String);
redacted_debug!(EngineChatId);

impl EngineChatId {
    /// Checks the format.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        let invalid = EngineError::Invalid("chat identifier");
        if text.is_empty() || text.len() > ENGINE_ID_MAX_LEN {
            return Err(invalid);
        }
        let (user, server) = text.split_once('@').ok_or(invalid.clone())?;
        let (server, group) = match server {
            "c.us" | "s.whatsapp.net" => ("c.us", false),
            "lid" => ("lid", false),
            "g.us" => ("g.us", true),
            _ => return Err(invalid),
        };
        let digits_ok = |part: &str| {
            !part.is_empty() && part.len() <= 40 && part.bytes().all(|b| b.is_ascii_digit())
        };
        let user_ok = if group {
            let mut parts = user.split('-');
            let first = parts.next().is_some_and(digits_ok);
            let rest: Vec<&str> = parts.collect();
            first && rest.len() <= 1 && rest.iter().all(|part| digits_ok(part))
        } else {
            digits_ok(user)
        };
        if !user_ok {
            return Err(invalid);
        }
        Ok(Self(format!("{user}@{server}")))
    }

    /// The identifier, as the engine takes it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is a group or a person.
    #[must_use]
    pub fn kind(&self) -> ChatKind {
        if self.0.ends_with("@g.us") {
            ChatKind::Group
        } else {
            ChatKind::Contact
        }
    }

    /// The telephone, when the identifier is `digits@c.us`.
    #[must_use]
    pub fn phone(&self) -> Option<Phone> {
        self.0
            .strip_suffix("@c.us")
            .and_then(|digits| Phone::parse(digits).ok())
    }

    pub(crate) fn segment(&self) -> String {
        path_segment(&self.0)
    }
}

/// The identifier of a message in the engine, as it came in a reading.
///
/// The complete form is `true_` or `false_` (whether the number sent it), the chat, `_`, the
/// identifier of WhatsApp and, in a group, `_` and the author. Some engines also take the
/// identifier of WhatsApp alone.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EngineMessageId(String);
redacted_debug!(EngineMessageId);

/// A message identified completely: what editing and deleting ask for
/// (`docs/09-contrato-mcp.md`, section 4).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FullMessageId {
    text: String,
    from_me: bool,
    chat: EngineChatId,
}
redacted_debug!(FullMessageId);

fn is_whatsapp_id(text: &str) -> bool {
    (4..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
}

impl EngineMessageId {
    /// Checks the complete form or the identifier of WhatsApp alone.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        if text.len() > MESSAGE_ID_MAX_LEN {
            return Err(EngineError::Invalid("message identifier"));
        }
        if is_whatsapp_id(text) || FullMessageId::parse(text).is_ok() {
            return Ok(Self(text.to_owned()));
        }
        Err(EngineError::Invalid("message identifier"))
    }

    /// The identifier, as the engine takes it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The complete form, when this is one.
    #[must_use]
    pub fn full(&self) -> Option<FullMessageId> {
        FullMessageId::parse(&self.0).ok()
    }

    pub(crate) fn segment(&self) -> String {
        path_segment(&self.0)
    }
}

impl FullMessageId {
    /// Checks the complete form: `true_` or `false_`, a chat, `_`, the identifier of WhatsApp
    /// and an optional `_` and author.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else, the identifier alone included.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        let invalid = EngineError::Invalid("complete message identifier");
        if text.len() > MESSAGE_ID_MAX_LEN {
            return Err(invalid);
        }
        let (from_me, rest) = if let Some(rest) = text.strip_prefix("true_") {
            (true, rest)
        } else if let Some(rest) = text.strip_prefix("false_") {
            (false, rest)
        } else {
            return Err(invalid);
        };
        let mut parts = rest.split('_');
        let chat = parts.next().ok_or(invalid.clone())?;
        let id = parts.next().ok_or(invalid.clone())?;
        let author = parts.next();
        if parts.next().is_some() || !is_whatsapp_id(id) {
            return Err(invalid);
        }
        let chat = EngineChatId::parse(chat)?;
        if let Some(author) = author {
            EngineChatId::parse(author)?;
        }
        Ok(Self {
            text: text.to_owned(),
            from_me,
            chat,
        })
    }

    /// Whether the number sent it.
    #[must_use]
    pub const fn from_me(&self) -> bool {
        self.from_me
    }

    /// The chat of the message.
    #[must_use]
    pub fn chat(&self) -> &EngineChatId {
        &self.chat
    }

    /// The identifier, as the engine takes it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// As a message identifier.
    #[must_use]
    pub fn to_message_id(&self) -> EngineMessageId {
        EngineMessageId(self.text.clone())
    }

    pub(crate) fn segment(&self) -> String {
        path_segment(&self.text)
    }
}

// ---------------------------------------------------------------------------- texts

/// The text of a message, read or to be sent: up to 64 KiB, with no NUL.
#[derive(Clone, PartialEq, Eq)]
pub struct MessageBody(String);
redacted_debug!(MessageBody);

impl MessageBody {
    /// Checks the size and that there is no NUL. Empty is allowed for a text that was read
    /// (a message with only a file).
    ///
    /// # Errors
    /// [`EngineError::Invalid`] when the text is too long or has a NUL.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        if text.len() > MESSAGE_TEXT_MAX_BYTES || text.contains('\0') {
            return Err(EngineError::Invalid("message text"));
        }
        Ok(Self(text.to_owned()))
    }

    /// A text read from the engine: cut at the ceiling on a character boundary, with every
    /// NUL dropped. Never fails.
    #[must_use]
    pub fn from_engine(text: &str) -> Self {
        let mut end = text.len().min(MESSAGE_TEXT_MAX_BYTES);
        while end > 0 && !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        Self(text.get(..end).unwrap_or_default().replace('\0', ""))
    }

    /// A text to send: also not empty and not only spaces.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] when empty, too long or with a NUL.
    pub fn to_send(text: &str) -> Result<Self, EngineError> {
        if text.trim().is_empty() {
            return Err(EngineError::Invalid("message text"));
        }
        Self::parse(text)
    }

    /// The text. Whoever calls this answers for where it goes: never a log or an error.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A text the other side shows (a name, a subject, a profile phrase): untrusted, cut at
/// 1 KiB on a character boundary, with control characters dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct ShownText(String);
redacted_debug!(ShownText);

impl ShownText {
    /// Cuts and cleans what came from the engine.
    #[must_use]
    pub fn from_engine(text: &str) -> Self {
        let mut kept = String::with_capacity(text.len().min(SHOWN_TEXT_MAX_BYTES));
        for c in text.chars() {
            if c.is_control() && c != '\n' {
                continue;
            }
            if kept.len().saturating_add(c.len_utf8()) > SHOWN_TEXT_MAX_BYTES {
                break;
            }
            kept.push(c);
        }
        Self(kept)
    }

    /// The text. Whoever calls this answers for where it goes.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// A reaction: an emoji of up to 32 bytes, or empty to take the reaction away.
#[derive(Clone, PartialEq, Eq)]
pub struct Reaction(String);
redacted_debug!(Reaction);

impl Reaction {
    /// Checks the size and that there is no control character.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        if text.len() > 32 || text.chars().any(char::is_control) {
            return Err(EngineError::Invalid("reaction"));
        }
        Ok(Self(text.to_owned()))
    }

    /// The emoji.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------- pages

/// A page of a list: from `offset`, up to `limit` items (1 to 200).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    limit: u32,
    offset: u32,
}

impl Page {
    /// Checks the limit.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] when the limit is 0 or above 200, or the offset above
    /// 1,000,000.
    pub fn new(limit: u32, offset: u32) -> Result<Self, EngineError> {
        if limit == 0 || limit > PAGE_MAX || offset > 1_000_000 {
            return Err(EngineError::Invalid("page"));
        }
        Ok(Self { limit, offset })
    }

    /// How many items.
    #[must_use]
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    /// From which item.
    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.offset
    }
}

/// Which messages of a chat to read: up to `limit`, the newest first, sent before `before`
/// and at or after `after` (seconds of the clock of the engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageQuery {
    /// How many, from 1 to 200.
    pub page: Page,
    /// Only older than this.
    pub before_s: Option<i64>,
    /// Only at or after this.
    pub after_s: Option<i64>,
}

// ---------------------------------------------------------------------------- answers

/// Who wrote a message. The field of origin of the engine is always the chat, even for what
/// the number itself sent: the author comes from the other fields (`referencia/waha-fatos.md`).
#[derive(Clone, PartialEq, Eq)]
pub enum Author {
    /// The number itself.
    Me,
    /// Someone else: the person of the chat, or the participant of the group.
    Other(EngineChatId),
    /// The engine did not say who, in a group.
    Unknown,
}
redacted_debug!(Author);

/// A file attached to a message, without its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    /// The media type, when the engine says it.
    pub mimetype: Option<MimeType>,
    /// The name, when the engine says it.
    pub filename: Option<FileName>,
}

/// A message, as the engine gave it.
#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    /// Its identifier.
    pub id: EngineMessageId,
    /// The chat.
    pub chat: EngineChatId,
    /// When, in milliseconds of the clock of the engine.
    pub at_ms: i64,
    /// Who wrote it.
    pub author: Author,
    /// The text, empty when there is none.
    pub body: MessageBody,
    /// The attached file, when there is one.
    pub media: Option<MediaInfo>,
    /// The message it answers, when it answers one.
    pub reply_to: Option<EngineMessageId>,
}
redacted_debug!(Message);

/// A chat in the list of the number.
#[derive(Clone, PartialEq, Eq)]
pub struct ChatSummary {
    /// Its identifier.
    pub id: EngineChatId,
    /// The name the engine shows.
    pub name: Option<ShownText>,
    /// The last message, without its file.
    pub last: Option<Message>,
}
redacted_debug!(ChatSummary);

/// The public data of a person.
#[derive(Clone, PartialEq, Eq)]
pub struct Contact {
    /// Its identifier.
    pub id: EngineChatId,
    /// The name kept in the agenda of the number.
    pub name: Option<ShownText>,
    /// The name the person chose.
    pub push_name: Option<ShownText>,
    /// The profile phrase.
    pub about: Option<ShownText>,
    /// The address of the profile picture, on the servers of WhatsApp.
    pub picture_url: Option<String>,
    /// Whether it is a business account. The engine documentation does not show this field:
    /// `None` until a version is checked.
    pub is_business: Option<bool>,
}
redacted_debug!(Contact);

/// A group in the list of the number. The count of participants is the length of the list:
/// the count field of the engine reads zero (`referencia/waha-fatos.md`).
#[derive(Clone, PartialEq, Eq)]
pub struct GroupSummary {
    /// Its identifier.
    pub id: EngineChatId,
    /// The subject.
    pub subject: Option<ShownText>,
    /// How many participants.
    pub participants: u32,
}
redacted_debug!(GroupSummary);

/// The role of a participant of a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A participant.
    Participant,
    /// An admin of the group.
    Admin,
    /// The creator.
    SuperAdmin,
    /// Someone who left.
    Left,
}

/// A participant of a group.
#[derive(Clone, PartialEq, Eq)]
pub struct Participant {
    /// Its identifier.
    pub id: EngineChatId,
    /// Its role.
    pub role: Role,
}
redacted_debug!(Participant);

/// A group with one page of its participants.
#[derive(Clone, PartialEq, Eq)]
pub struct GroupDetail {
    /// Its identifier.
    pub id: EngineChatId,
    /// The subject.
    pub subject: Option<ShownText>,
    /// The description.
    pub description: Option<ShownText>,
    /// How many participants, in all.
    pub total: u32,
    /// The participants of this page.
    pub participants: Vec<Participant>,
    /// Where the next page starts, when there is one.
    pub next_offset: Option<u32>,
}
redacted_debug!(GroupDetail);

/// A media type: `type/subtype`, up to 127 characters of a narrow set, as the engine takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeType(String);

impl MimeType {
    /// Checks `type/subtype`, with an optional `; codecs=...`.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        let invalid = EngineError::Invalid("media type");
        if text.is_empty() || text.len() > 127 {
            return Err(invalid);
        }
        let token = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
        };
        let (essence, parameter) = match text.split_once(';') {
            Some((essence, parameter)) => (essence, Some(parameter.trim())),
            None => (text, None),
        };
        let essence_ok = essence
            .split_once('/')
            .is_some_and(|(kind, sub)| token(kind) && token(sub));
        let parameter_ok = parameter.is_none_or(|parameter| {
            parameter
                .split_once('=')
                .is_some_and(|(key, value)| token(key) && token(value))
        });
        if !essence_ok || !parameter_ok {
            return Err(invalid);
        }
        Ok(Self(text.to_ascii_lowercase()))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name of a file: up to 255 bytes, with no slash, backslash, control character or
/// leading dot.
#[derive(Clone, PartialEq, Eq)]
pub struct FileName(String);
redacted_debug!(FileName);

impl FileName {
    /// Checks the name.
    ///
    /// # Errors
    /// [`EngineError::Invalid`] for anything else.
    pub fn parse(text: &str) -> Result<Self, EngineError> {
        if text.is_empty()
            || text.len() > 255
            || text.starts_with('.')
            || text
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\')
        {
            return Err(EngineError::Invalid("file name"));
        }
        Ok(Self(text.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kind of a file to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// An image, shown in the chat.
    Image,
    /// A document.
    Document,
    /// A voice message.
    Voice,
}

/// A file, sent by content or downloaded from a message.
#[derive(Clone, PartialEq, Eq)]
pub struct FileContent {
    /// The media type.
    pub mimetype: MimeType,
    /// The name.
    pub filename: Option<FileName>,
    /// The bytes.
    pub data: Vec<u8>,
}
redacted_debug!(FileContent);

/// A text to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendText {
    /// The chat.
    pub chat: EngineChatId,
    /// The text, not empty.
    pub text: MessageBody,
    /// The message it answers.
    pub reply_to: Option<EngineMessageId>,
    /// The people marked, in a group.
    pub mentions: Vec<EngineChatId>,
}

/// A file to send by content, up to 3 MB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFile {
    /// The chat.
    pub chat: EngineChatId,
    /// How it shows in the chat.
    pub kind: FileKind,
    /// The file.
    pub file: FileContent,
    /// The text that goes with an image or a document.
    pub caption: Option<MessageBody>,
}

/// What a sending returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    /// The identifier of the new message, when the engine gave it back.
    pub id: Option<EngineMessageId>,
}

/// What the engine says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineVersion {
    /// The version, like `2026.10.1`.
    pub version: String,
    /// The internal mechanism, like `GOWS`.
    pub engine: String,
    /// The edition, like `CORE`.
    pub tier: String,
}

impl EngineVersion {
    /// The versions the hub was checked against (`referencia/waha-fatos.md`): the version and
    /// the internal mechanism.
    pub const TESTED: &'static [(&'static str, &'static str)] = &[("2026.10.1", "GOWS")];

    /// Whether this is a version the hub was checked against.
    #[must_use]
    pub fn is_tested(&self) -> bool {
        Self::TESTED.iter().any(|(version, engine)| {
            *version == self.version && engine.eq_ignore_ascii_case(&self.engine)
        })
    }
}

/// What the QR of a pairing holds: a short-lived secret, shown once on the terminal of the
/// machine of the hub and never stored.
pub struct PairingQr(Zeroizing<String>);
redacted_debug!(PairingQr);

impl PairingQr {
    /// Checks the size and the characters.
    ///
    /// # Errors
    /// [`EngineError::Unavailable`] when the engine gave something else.
    pub(crate) fn new(value: String) -> Result<Self, EngineError> {
        let value = Zeroizing::new(value);
        if value.is_empty() || value.len() > 1_024 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(EngineError::malformed());
        }
        Ok(Self(value))
    }

    /// The value to draw.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// The code a person types on the telephone to pair it: short-lived, shown once.
pub struct PairingCode(Zeroizing<String>);
redacted_debug!(PairingCode);

impl PairingCode {
    /// Checks the size and the characters.
    ///
    /// # Errors
    /// [`EngineError::Unavailable`] when the engine gave something else.
    pub(crate) fn new(value: String) -> Result<Self, EngineError> {
        let value = Zeroizing::new(value);
        if value.is_empty()
            || value.len() > 16
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(EngineError::malformed());
        }
        Ok(Self(value))
    }

    /// The code to show.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drawn_session_name_is_in_the_format_and_never_the_same() {
        let first = SessionName::draw().unwrap();
        let second = SessionName::draw().unwrap();
        assert_ne!(first, second);
        assert_eq!(first.as_str().len(), 27);
        assert!(SessionName::parse(first.as_str()).is_ok());
    }

    #[test]
    fn a_session_name_outside_the_format_is_refused() {
        for wrong in [
            "",
            "default",
            "../aaaaaaaaaaaaaaaaaaaa",
            "aaaaaaaaaaaaaaaa/x",
            "aaaaaaaaaaaaaaaa%2e",
            "aaaaaaaaaaaaaaaa\0",
            "Aaaaaaaaaaaaaaaaaa",
            "1aaaaaaaaaaaaaaaaa",
            "aaaaaaaaaaaaaaaa-x",
            "aaaaaaaaaaaaaaaa.x",
            &"a".repeat(64),
        ] {
            assert!(SessionName::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn a_telephone_is_digits_with_the_country() {
        assert_eq!(
            Phone::parse("+55 (11) 99999-0000").unwrap().digits(),
            "5511999990000"
        );
        for wrong in [
            "",
            "123",
            "0055119999",
            "+55 11 9999a0000",
            "55/11/999990000",
            "5511999990000000000",
            "+5511\u{0}999990000",
            &"5".repeat(40),
        ] {
            assert!(Phone::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn a_brazilian_mobile_is_tried_with_and_without_the_ninth_digit() {
        let with = Phone::parse("5511987654321").unwrap();
        let forms: Vec<String> = with.forms().iter().map(|p| p.digits().to_owned()).collect();
        assert_eq!(forms, ["5511987654321", "551187654321"]);
        let without = Phone::parse("551187654321").unwrap();
        let forms: Vec<String> = without
            .forms()
            .iter()
            .map(|p| p.digits().to_owned())
            .collect();
        assert_eq!(forms, ["551187654321", "5511987654321"]);
        // A landline (starts with 2 to 5) has no ninth digit; another country, no rule.
        assert_eq!(Phone::parse("551132654321").unwrap().forms().len(), 1);
        assert_eq!(Phone::parse("15555550123").unwrap().forms().len(), 1);
        assert!(with.same_as(&without));
        assert!(!with.same_as(&Phone::parse("5511987654322").unwrap()));
    }

    #[test]
    fn a_chat_identifier_is_one_of_the_known_forms() {
        for good in [
            "5511999990000@c.us",
            "120363012345678901@g.us",
            "5511999990000-1600000000@g.us",
            "23423462304912@lid",
        ] {
            assert_eq!(EngineChatId::parse(good).unwrap().as_str(), good);
        }
        assert_eq!(
            EngineChatId::parse("5511999990000@s.whatsapp.net")
                .unwrap()
                .as_str(),
            "5511999990000@c.us"
        );
        for wrong in [
            "",
            "5511999990000",
            "../../sessions@c.us",
            "5511999990000@c.us/../x",
            "5511%2e@c.us",
            "%2e%2e@c.us",
            "5511999990000@c.us\0",
            "55119\n99990000@c.us",
            "status@broadcast",
            "123@newsletter",
            "abc@c.us",
            "1-2-3@g.us",
            "1-2@c.us",
            "@c.us",
            "5511999990000@C.US",
            &format!("{}@c.us", "1".repeat(70)),
        ] {
            assert!(EngineChatId::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn a_path_segment_has_only_the_checked_characters() {
        let chat = EngineChatId::parse("5511999990000@c.us").unwrap();
        assert_eq!(chat.segment(), "5511999990000%40c.us");
        let message = EngineMessageId::parse("true_5511999990000@c.us_3EB0ABCDEF").unwrap();
        assert_eq!(message.segment(), "true_5511999990000%40c.us_3EB0ABCDEF");
    }

    #[test]
    fn a_message_identifier_is_complete_or_the_whatsapp_one_alone() {
        let full = EngineMessageId::parse("false_5511999990000@c.us_3EB0ABCDEF").unwrap();
        let full = full.full().unwrap();
        assert!(!full.from_me());
        assert_eq!(full.chat().as_str(), "5511999990000@c.us");
        assert!(
            FullMessageId::parse("false_120363012345678901@g.us_3EB0ABCDEF_5511999990000@c.us")
                .is_ok()
        );
        let plain = EngineMessageId::parse("3EB0ABCDEF").unwrap();
        assert!(plain.full().is_none());
        assert!(FullMessageId::parse("3EB0ABCDEF").is_err());
        for wrong in [
            "",
            "true_",
            "true_5511999990000@c.us",
            "true_5511999990000@c.us_",
            "true_5511999990000@c.us_../x",
            "true_../5511@c.us_3EB0",
            "maybe_5511999990000@c.us_3EB0ABCDEF",
            "true_5511999990000@c.us_3EB0%2e",
            "true_5511999990000@c.us_3EB0_x_y",
            "3EB0/ABCDEF",
            "..",
            "%2e%2e",
            "3EB0\0ABCD",
            &"A".repeat(300),
        ] {
            assert!(EngineMessageId::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn texts_and_reactions_are_bounded() {
        assert!(MessageBody::to_send("").is_err());
        assert!(MessageBody::to_send("   ").is_err());
        assert!(MessageBody::to_send("a\0b").is_err());
        assert!(MessageBody::to_send(&"a".repeat(MESSAGE_TEXT_MAX_BYTES + 1)).is_err());
        assert!(MessageBody::parse("").is_ok());
        assert!(Reaction::parse("").is_ok());
        assert!(Reaction::parse("\u{1F44D}").is_ok());
        assert!(Reaction::parse("\u{7}").is_err());
        assert!(Reaction::parse(&"x".repeat(33)).is_err());
        let shown = ShownText::from_engine(&format!("a\u{1b}[31m{}", "b".repeat(2_000)));
        assert!(!shown.expose().contains('\u{1b}'));
        assert!(shown.expose().len() <= SHOWN_TEXT_MAX_BYTES);
    }

    #[test]
    fn media_types_and_file_names_are_narrow() {
        assert!(MimeType::parse("image/jpeg").is_ok());
        assert!(MimeType::parse("audio/ogg; codecs=opus").is_ok());
        for wrong in [
            "",
            "image",
            "image/",
            "image/jpeg\r\nX: y",
            "a/b; c",
            "a b/c",
        ] {
            assert!(MimeType::parse(wrong).is_err(), "{wrong:?}");
        }
        assert!(FileName::parse("report.pdf").is_ok());
        for wrong in [
            "",
            ".hidden",
            "../x",
            "a/b",
            "a\\b",
            "a\nb",
            &"a".repeat(256),
        ] {
            assert!(FileName::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn no_personal_data_shows_in_debug() {
        let phone = Phone::parse("5511987654321").unwrap();
        let text = MessageBody::parse("a secret text").unwrap();
        let chat = phone.chat_id();
        for shown in [
            format!("{phone:?}"),
            format!("{text:?}"),
            format!("{chat:?}"),
        ] {
            assert!(
                !shown.contains("987654321") && !shown.contains("secret"),
                "{shown}"
            );
        }
    }

    #[test]
    fn only_the_checked_versions_are_tested() {
        let tested = EngineVersion {
            version: "2026.10.1".to_owned(),
            engine: "GOWS".to_owned(),
            tier: "CORE".to_owned(),
        };
        assert!(tested.is_tested());
        let other = EngineVersion {
            version: "2026.11.1".to_owned(),
            ..tested.clone()
        };
        assert!(!other.is_tested());
        let mechanism = EngineVersion {
            engine: "NOWEB".to_owned(),
            ..tested
        };
        assert!(!mechanism.is_tested());
    }
}
