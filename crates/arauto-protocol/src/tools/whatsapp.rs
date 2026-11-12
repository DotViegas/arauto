//! The arguments of the WhatsApp tools (`docs/09-contrato-mcp.md`, sections 2 and 3), as the
//! hub checks them before it looks at any permission.
//!
//! What is checked here is the form: the alias of a number, the identifier of a chat, a
//! telephone, the identifier of a message, a text, a file. Whether the Hermes may use what it
//! named is the hub's to say, against the matrix of accesses, at every call.
//!
//! - **`number`** is the alias the admin gave the number, in the restricted format. It may be
//!   left out; the hub then takes the only number of the Hermes, or refuses.
//! - **`chat`** is the opaque identifier of a chat the hub gave in a listing or a notice: a
//!   UUID. A Hermes never names a session or a chat of the engine.
//! - **`to`** is a telephone with the code of the country, or such a chat identifier.
//! - **`message_id`** is the identifier of a message exactly as a reading of the hub gave it.
//!   The hub signs it; here only its alphabet and its length are checked.
//!
//! Telephones, texts, files and identifiers of messages are personal data of third parties:
//! their `Debug` prints neither them nor a part of them.

use core::fmt;

use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{ToolArgError, only, optional, optional_uuid};
use crate::limits::{
    WHATSAPP_LIST_DEFAULT, WHATSAPP_LIST_MAX, WHATSAPP_MENTIONS_MAX, WHATSAPP_MESSAGE_ID_MAX_LEN,
    WHATSAPP_OFFSET_MAX, WHATSAPP_PARTICIPANTS_DEFAULT, WHATSAPP_PARTICIPANTS_MAX,
    WHATSAPP_READ_DEFAULT, WHATSAPP_READ_MAX, WHATSAPP_SEND_FILE_MAX_BYTES,
    WHATSAPP_TEXT_MAX_BYTES, WHATSAPP_TIME_MAX_S,
};
use crate::names::NumberAlias;

/// Shows the state of one number.
pub const WHATSAPP_STATUS: &str = "whatsapp_status";
/// Lists the chats of a number.
pub const WHATSAPP_LIST_CHATS: &str = "whatsapp_list_chats";
/// Reads the messages of one chat.
pub const WHATSAPP_READ_CHAT: &str = "whatsapp_read_chat";
/// Shows the public data of the person of a chat.
pub const WHATSAPP_GET_CONTACT: &str = "whatsapp_get_contact";
/// Downloads the file of a message.
pub const WHATSAPP_DOWNLOAD_MEDIA: &str = "whatsapp_download_media";
/// Lists the groups of a number.
pub const WHATSAPP_LIST_GROUPS: &str = "whatsapp_list_groups";
/// Shows one group and its participants.
pub const WHATSAPP_GET_GROUP: &str = "whatsapp_get_group";
/// Asks whether a telephone has WhatsApp.
pub const WHATSAPP_CHECK_NUMBER: &str = "whatsapp_check_number";
/// Sends a text.
pub const WHATSAPP_SEND_TEXT: &str = "whatsapp_send_text";
/// Sends a file by content.
pub const WHATSAPP_SEND_FILE: &str = "whatsapp_send_file";
/// Marks a chat as read.
pub const WHATSAPP_MARK_READ: &str = "whatsapp_mark_read";
/// Turns "typing" on or off.
pub const WHATSAPP_SET_TYPING: &str = "whatsapp_set_typing";
/// Reacts to a message.
pub const WHATSAPP_REACT: &str = "whatsapp_react";
/// Forwards a message to another chat.
pub const WHATSAPP_FORWARD: &str = "whatsapp_forward";
/// Edits a message the number sent.
pub const WHATSAPP_EDIT_MESSAGE: &str = "whatsapp_edit_message";
/// Deletes a message for everyone.
pub const WHATSAPP_DELETE_MESSAGE: &str = "whatsapp_delete_message";

/// The longest text a telephone is read from, before anything else is looked at.
const PHONE_TEXT_MAX_LEN: usize = 32;

/// Longest media type.
const MIMETYPE_MAX_LEN: usize = 127;

/// Longest name of a file.
const FILENAME_MAX_LEN: usize = 255;

/// Longest reaction, in bytes.
const REACTION_MAX_BYTES: usize = 32;

/// Shortest identifier of a message the hub gives.
const MESSAGE_ID_MIN_LEN: usize = 16;

// ---------------------------------------------------------------------------- values

/// A telephone with the code of the country, as digits only: from 8 to 15, the first not a
/// zero. Read from `+55 11 98765-4321`, `5511987654321` and the like: spaces, hyphens,
/// parentheses and one leading `+` are dropped, and any other character refuses the text.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WhatsappPhone(String);

impl fmt::Debug for WhatsappPhone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WhatsappPhone([redacted])")
    }
}

impl WhatsappPhone {
    /// Checks the text and keeps its digits.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() > PHONE_TEXT_MAX_LEN {
            return None;
        }
        let text = text.trim();
        let text = text.strip_prefix('+').unwrap_or(text);
        let mut digits = String::with_capacity(15);
        for c in text.chars() {
            match c {
                '0'..='9' => digits.push(c),
                ' ' | '-' | '(' | ')' => {}
                _ => return None,
            }
        }
        if !(8..=15).contains(&digits.len()) || digits.starts_with('0') {
            return None;
        }
        Some(Self(digits))
    }

    /// The digits. Whoever calls this answers for where they go: never a log or an error.
    #[must_use]
    pub fn digits(&self) -> &str {
        &self.0
    }
}

/// Where a message goes: a telephone, or a chat the hub already gave an identifier to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    /// A telephone with the code of the country. The hub asks WhatsApp for its exact form.
    Phone(WhatsappPhone),
    /// The opaque identifier of a chat or group, as a listing or a notice gave it.
    Chat(Uuid),
}

/// The identifier of a message as a reading of the hub gave it: from 16 to 400 characters of
/// `A-Z a-z 0-9 _ -`. The hub signed it and checks the signature; here only the form.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MessageRef(String);

impl fmt::Debug for MessageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MessageRef([redacted])")
    }
}

impl MessageRef {
    /// Checks the alphabet and the length.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let allowed = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-';
        let bytes = text.as_bytes();
        if !(MESSAGE_ID_MIN_LEN..=WHATSAPP_MESSAGE_ID_MAX_LEN).contains(&bytes.len())
            || !bytes.iter().all(allowed)
        {
            return None;
        }
        Some(Self(text.to_owned()))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The text of a WhatsApp message a Hermes sends: from 1 to 65,536 bytes of UTF-8, with no
/// NUL, and not made only of white space. What it says is not the hub's business.
#[derive(Clone, PartialEq, Eq)]
pub struct WhatsappText(String);

impl fmt::Debug for WhatsappText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WhatsappText({} bytes)", self.0.len())
    }
}

impl WhatsappText {
    /// Checks the size, the NUL and the white space.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty()
            || text.len() > WHATSAPP_TEXT_MAX_BYTES
            || text.contains('\0')
            || text.trim().is_empty()
        {
            return None;
        }
        Some(Self(text.to_owned()))
    }

    /// The text. Whoever calls this answers for where it goes: never a log or an error.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// A reaction: an emoji of up to 32 bytes, with no control character; empty takes the
/// reaction away.
#[derive(Clone, PartialEq, Eq)]
pub struct ReactionText(String);

impl fmt::Debug for ReactionText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReactionText([redacted])")
    }
}

impl ReactionText {
    /// Checks the size and the characters.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() > REACTION_MAX_BYTES || text.chars().any(char::is_control) {
            return None;
        }
        Some(Self(text.to_owned()))
    }

    /// The emoji.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How a file shows in the chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// An image.
    Image,
    /// A document.
    Document,
    /// A voice message.
    Voice,
}

/// A file a Hermes sends by content.
#[derive(Clone, PartialEq, Eq)]
pub struct FileArg {
    /// How it shows.
    pub kind: FileKind,
    /// The media type, `type/subtype`, with an optional parameter.
    pub mimetype: String,
    /// The name, when given.
    pub filename: Option<String>,
    /// The bytes, at most 3 MB.
    pub data: Vec<u8>,
}

impl fmt::Debug for FileArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileArg({:?}, {} bytes)", self.kind, self.data.len())
    }
}

// ---------------------------------------------------------------------------- calls

/// Which part of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listing {
    /// How many items.
    pub limit: u32,
    /// From which item.
    pub offset: u32,
}

/// The arguments of `whatsapp_read_chat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadChat {
    /// The chat.
    pub chat: Uuid,
    /// How many messages: from 1 to 200, 30 when absent.
    pub limit: u32,
    /// Only messages older than this, in seconds since 1970.
    pub before: Option<i64>,
    /// Only messages at or after this, in seconds since 1970.
    pub after: Option<i64>,
}

/// The arguments of `whatsapp_get_group`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetGroup {
    /// The group.
    pub chat: Uuid,
    /// Which participants.
    pub participants: Listing,
}

/// The arguments of `whatsapp_send_text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendTextArgs {
    /// Where.
    pub to: Recipient,
    /// The text.
    pub text: WhatsappText,
    /// The message it answers.
    pub reply_to: Option<MessageRef>,
    /// The people of a group it marks.
    pub mentions: Vec<WhatsappPhone>,
}

/// The arguments of `whatsapp_send_file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFileArgs {
    /// Where.
    pub to: Recipient,
    /// The file.
    pub file: FileArg,
    /// The text that goes with an image or a document.
    pub caption: Option<WhatsappText>,
}

/// What a WhatsApp call asks, once its arguments passed the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhatsappOp {
    /// `whatsapp_status`
    Status,
    /// `whatsapp_list_chats`
    ListChats(Listing),
    /// `whatsapp_read_chat`
    ReadChat(ReadChat),
    /// `whatsapp_get_contact`
    GetContact {
        /// The chat of the person.
        chat: Uuid,
    },
    /// `whatsapp_download_media`
    DownloadMedia {
        /// The message.
        message: MessageRef,
    },
    /// `whatsapp_list_groups`
    ListGroups(Listing),
    /// `whatsapp_get_group`
    GetGroup(GetGroup),
    /// `whatsapp_check_number`
    CheckNumber {
        /// The telephone.
        phone: WhatsappPhone,
    },
    /// `whatsapp_send_text`
    SendText(SendTextArgs),
    /// `whatsapp_send_file`
    SendFile(SendFileArgs),
    /// `whatsapp_mark_read`
    MarkRead {
        /// The chat.
        chat: Uuid,
    },
    /// `whatsapp_set_typing`
    SetTyping {
        /// The chat.
        chat: Uuid,
        /// On or off.
        on: bool,
    },
    /// `whatsapp_react`
    React {
        /// The message.
        message: MessageRef,
        /// The emoji, or empty.
        reaction: ReactionText,
    },
    /// `whatsapp_forward`
    Forward {
        /// The message.
        message: MessageRef,
        /// Where it goes.
        to: Recipient,
    },
    /// `whatsapp_edit_message`
    EditMessage {
        /// The message.
        message: MessageRef,
        /// The new text.
        text: WhatsappText,
    },
    /// `whatsapp_delete_message`
    DeleteMessage {
        /// The message.
        message: MessageRef,
    },
}

/// The family a WhatsApp call needs, by the verb it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatsappVerb {
    /// Read.
    Read,
    /// Send.
    Send,
    /// Modify.
    Modify,
}

impl WhatsappOp {
    /// The name of the tool.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            WhatsappOp::Status => WHATSAPP_STATUS,
            WhatsappOp::ListChats(_) => WHATSAPP_LIST_CHATS,
            WhatsappOp::ReadChat(_) => WHATSAPP_READ_CHAT,
            WhatsappOp::GetContact { .. } => WHATSAPP_GET_CONTACT,
            WhatsappOp::DownloadMedia { .. } => WHATSAPP_DOWNLOAD_MEDIA,
            WhatsappOp::ListGroups(_) => WHATSAPP_LIST_GROUPS,
            WhatsappOp::GetGroup(_) => WHATSAPP_GET_GROUP,
            WhatsappOp::CheckNumber { .. } => WHATSAPP_CHECK_NUMBER,
            WhatsappOp::SendText(_) => WHATSAPP_SEND_TEXT,
            WhatsappOp::SendFile(_) => WHATSAPP_SEND_FILE,
            WhatsappOp::MarkRead { .. } => WHATSAPP_MARK_READ,
            WhatsappOp::SetTyping { .. } => WHATSAPP_SET_TYPING,
            WhatsappOp::React { .. } => WHATSAPP_REACT,
            WhatsappOp::Forward { .. } => WHATSAPP_FORWARD,
            WhatsappOp::EditMessage { .. } => WHATSAPP_EDIT_MESSAGE,
            WhatsappOp::DeleteMessage { .. } => WHATSAPP_DELETE_MESSAGE,
        }
    }

    /// The verb the tool belongs to: what releases its family. Forwarding is a sending: the
    /// reading in the chat of origin is checked on top of it.
    #[must_use]
    pub const fn verb(&self) -> WhatsappVerb {
        match self {
            WhatsappOp::Status
            | WhatsappOp::ListChats(_)
            | WhatsappOp::ReadChat(_)
            | WhatsappOp::GetContact { .. }
            | WhatsappOp::DownloadMedia { .. }
            | WhatsappOp::ListGroups(_)
            | WhatsappOp::GetGroup(_) => WhatsappVerb::Read,
            WhatsappOp::CheckNumber { .. }
            | WhatsappOp::SendText(_)
            | WhatsappOp::SendFile(_)
            | WhatsappOp::MarkRead { .. }
            | WhatsappOp::SetTyping { .. }
            | WhatsappOp::React { .. }
            | WhatsappOp::Forward { .. } => WhatsappVerb::Send,
            WhatsappOp::EditMessage { .. } | WhatsappOp::DeleteMessage { .. } => {
                WhatsappVerb::Modify
            }
        }
    }

    /// Whether the call counts toward the send limit: what writes something new that the
    /// other side sees, and the question to WhatsApp about a telephone outside the chats.
    #[must_use]
    pub const fn counts_toward_the_send_limit(&self) -> bool {
        matches!(
            self,
            WhatsappOp::CheckNumber { .. }
                | WhatsappOp::SendText(_)
                | WhatsappOp::SendFile(_)
                | WhatsappOp::React { .. }
                | WhatsappOp::Forward { .. }
        )
    }
}

/// A WhatsApp call whose name and arguments passed the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsappCall {
    /// The alias of the number, when given.
    pub number: Option<NumberAlias>,
    /// What it asks.
    pub op: WhatsappOp,
}

/// Checks the arguments of the WhatsApp tool `tool`.
pub(super) fn parse(tool: &str, a: &Map<String, Value>) -> Result<WhatsappCall, ToolArgError> {
    let op = match tool {
        WHATSAPP_STATUS => {
            only(a, &["number"])?;
            WhatsappOp::Status
        }
        WHATSAPP_LIST_CHATS => {
            only(a, &["number", "limit", "offset"])?;
            WhatsappOp::ListChats(listing(a, WHATSAPP_LIST_DEFAULT, WHATSAPP_LIST_MAX)?)
        }
        WHATSAPP_READ_CHAT => {
            only(a, &["number", "chat", "limit", "before", "after"])?;
            WhatsappOp::ReadChat(ReadChat {
                chat: required_chat(a)?,
                limit: count(a, "limit", WHATSAPP_READ_DEFAULT, WHATSAPP_READ_MAX)?,
                before: seconds(a, "before")?,
                after: seconds(a, "after")?,
            })
        }
        WHATSAPP_GET_CONTACT => {
            only(a, &["number", "chat"])?;
            WhatsappOp::GetContact {
                chat: required_chat(a)?,
            }
        }
        WHATSAPP_DOWNLOAD_MEDIA => {
            only(a, &["number", "message_id"])?;
            WhatsappOp::DownloadMedia {
                message: message(a, "message_id")?,
            }
        }
        WHATSAPP_LIST_GROUPS => {
            only(a, &["number", "limit", "offset"])?;
            WhatsappOp::ListGroups(listing(a, WHATSAPP_LIST_DEFAULT, WHATSAPP_LIST_MAX)?)
        }
        WHATSAPP_GET_GROUP => {
            only(a, &["number", "chat", "limit", "offset"])?;
            WhatsappOp::GetGroup(GetGroup {
                chat: required_chat(a)?,
                participants: listing(a, WHATSAPP_PARTICIPANTS_DEFAULT, WHATSAPP_PARTICIPANTS_MAX)?,
            })
        }
        WHATSAPP_CHECK_NUMBER => {
            only(a, &["number", "phone"])?;
            let phone = optional(a, "phone").ok_or(ToolArgError::Missing("phone"))?;
            WhatsappOp::CheckNumber {
                phone: phone
                    .as_str()
                    .and_then(WhatsappPhone::parse)
                    .ok_or(ToolArgError::Invalid("phone"))?,
            }
        }
        WHATSAPP_SEND_TEXT => {
            only(a, &["number", "to", "text", "reply_to", "mentions"])?;
            WhatsappOp::SendText(SendTextArgs {
                to: recipient(a)?,
                text: text(a, "text")?.ok_or(ToolArgError::Missing("text"))?,
                reply_to: optional(a, "reply_to")
                    .map(|_| message(a, "reply_to"))
                    .transpose()?,
                mentions: mentions(a)?,
            })
        }
        WHATSAPP_SEND_FILE => {
            only(
                a,
                &[
                    "number", "to", "kind", "mimetype", "filename", "data", "caption",
                ],
            )?;
            WhatsappOp::SendFile(SendFileArgs {
                to: recipient(a)?,
                file: file(a)?,
                caption: text(a, "caption")?,
            })
        }
        WHATSAPP_MARK_READ => {
            only(a, &["number", "chat"])?;
            WhatsappOp::MarkRead {
                chat: required_chat(a)?,
            }
        }
        WHATSAPP_SET_TYPING => {
            only(a, &["number", "chat", "on"])?;
            WhatsappOp::SetTyping {
                chat: required_chat(a)?,
                on: optional(a, "on")
                    .ok_or(ToolArgError::Missing("on"))?
                    .as_bool()
                    .ok_or(ToolArgError::Invalid("on"))?,
            }
        }
        WHATSAPP_REACT => {
            only(a, &["number", "message_id", "reaction"])?;
            WhatsappOp::React {
                message: message(a, "message_id")?,
                reaction: optional(a, "reaction")
                    .ok_or(ToolArgError::Missing("reaction"))?
                    .as_str()
                    .and_then(ReactionText::parse)
                    .ok_or(ToolArgError::Invalid("reaction"))?,
            }
        }
        WHATSAPP_FORWARD => {
            only(a, &["number", "message_id", "to"])?;
            WhatsappOp::Forward {
                message: message(a, "message_id")?,
                to: recipient(a)?,
            }
        }
        WHATSAPP_EDIT_MESSAGE => {
            only(a, &["number", "message_id", "text"])?;
            WhatsappOp::EditMessage {
                message: message(a, "message_id")?,
                // Editing without a text is dangerous in the engine: it is required here.
                text: text(a, "text")?.ok_or(ToolArgError::Missing("text"))?,
            }
        }
        WHATSAPP_DELETE_MESSAGE => {
            only(a, &["number", "message_id"])?;
            WhatsappOp::DeleteMessage {
                message: message(a, "message_id")?,
            }
        }
        _ => return Err(ToolArgError::UnknownTool),
    };
    let number = optional(a, "number")
        .map(|value| {
            value
                .as_str()
                .and_then(|text| NumberAlias::parse(text).ok())
                .ok_or(ToolArgError::Invalid("number"))
        })
        .transpose()?;
    Ok(WhatsappCall { number, op })
}

fn required_chat(a: &Map<String, Value>) -> Result<Uuid, ToolArgError> {
    optional_uuid(a, "chat")?.ok_or(ToolArgError::Missing("chat"))
}

fn message(a: &Map<String, Value>, name: &'static str) -> Result<MessageRef, ToolArgError> {
    optional(a, name)
        .ok_or(ToolArgError::Missing(name))?
        .as_str()
        .and_then(MessageRef::parse)
        .ok_or(ToolArgError::Invalid(name))
}

fn recipient(a: &Map<String, Value>) -> Result<Recipient, ToolArgError> {
    let text = optional(a, "to")
        .ok_or(ToolArgError::Missing("to"))?
        .as_str()
        .ok_or(ToolArgError::Invalid("to"))?;
    if let Some(chat) = crate::serde_util::uuid_text(text) {
        return Ok(Recipient::Chat(chat));
    }
    WhatsappPhone::parse(text)
        .map(Recipient::Phone)
        .ok_or(ToolArgError::Invalid("to"))
}

fn text(a: &Map<String, Value>, name: &'static str) -> Result<Option<WhatsappText>, ToolArgError> {
    optional(a, name)
        .map(|value| {
            value
                .as_str()
                .and_then(WhatsappText::parse)
                .ok_or(ToolArgError::Invalid(name))
        })
        .transpose()
}

fn mentions(a: &Map<String, Value>) -> Result<Vec<WhatsappPhone>, ToolArgError> {
    let Some(value) = optional(a, "mentions") else {
        return Ok(Vec::new());
    };
    let items = value.as_array().ok_or(ToolArgError::Invalid("mentions"))?;
    if items.len() > WHATSAPP_MENTIONS_MAX {
        return Err(ToolArgError::Invalid("mentions"));
    }
    items
        .iter()
        .map(|item| {
            item.as_str()
                .and_then(WhatsappPhone::parse)
                .ok_or(ToolArgError::Invalid("mentions"))
        })
        .collect()
}

fn file(a: &Map<String, Value>) -> Result<FileArg, ToolArgError> {
    let kind = match optional(a, "kind")
        .ok_or(ToolArgError::Missing("kind"))?
        .as_str()
    {
        Some("image") => FileKind::Image,
        Some("document") => FileKind::Document,
        Some("voice") => FileKind::Voice,
        _ => return Err(ToolArgError::Invalid("kind")),
    };
    let mimetype = optional(a, "mimetype")
        .ok_or(ToolArgError::Missing("mimetype"))?
        .as_str()
        .filter(|text| mimetype_ok(text))
        .ok_or(ToolArgError::Invalid("mimetype"))?
        .to_owned();
    let filename = optional(a, "filename")
        .map(|value| {
            value
                .as_str()
                .filter(|text| filename_ok(text))
                .map(str::to_owned)
                .ok_or(ToolArgError::Invalid("filename"))
        })
        .transpose()?;
    let data = optional(a, "data")
        .ok_or(ToolArgError::Missing("data"))?
        .as_str()
        .ok_or(ToolArgError::Invalid("data"))?;
    // The size is checked on the text, before anything is decoded.
    let data = crate::base64::decode(data, WHATSAPP_SEND_FILE_MAX_BYTES)
        .ok()
        .filter(|bytes| !bytes.is_empty())
        .ok_or(ToolArgError::Invalid("data"))?;
    Ok(FileArg {
        kind,
        mimetype,
        filename,
        data,
    })
}

/// `type/subtype`, with an optional `; key=value`, of a narrow set of characters.
fn mimetype_ok(text: &str) -> bool {
    let token = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
    };
    if text.is_empty() || text.len() > MIMETYPE_MAX_LEN {
        return false;
    }
    let (essence, parameter) = match text.split_once(';') {
        Some((essence, parameter)) => (essence, Some(parameter.trim())),
        None => (text, None),
    };
    essence
        .split_once('/')
        .is_some_and(|(kind, sub)| token(kind) && token(sub))
        && parameter.is_none_or(|parameter| {
            parameter
                .split_once('=')
                .is_some_and(|(key, value)| token(key) && token(value))
        })
}

/// Up to 255 bytes, with no slash, backslash or control character, not starting with a dot.
fn filename_ok(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= FILENAME_MAX_LEN
        && !text.starts_with('.')
        && !text
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
}

/// The argument `name`: an integer from 1 to `max`, or `default` when absent.
fn count(
    a: &Map<String, Value>,
    name: &'static str,
    default: u32,
    max: u32,
) -> Result<u32, ToolArgError> {
    let Some(value) = optional(a, name) else {
        return Ok(default);
    };
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| (1..=max).contains(number))
        .ok_or(ToolArgError::Invalid(name))
}

fn listing(a: &Map<String, Value>, default: u32, max: u32) -> Result<Listing, ToolArgError> {
    let offset = match optional(a, "offset") {
        None => 0,
        Some(value) => value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .filter(|number| *number <= WHATSAPP_OFFSET_MAX)
            .ok_or(ToolArgError::Invalid("offset"))?,
    };
    Ok(Listing {
        limit: count(a, "limit", default, max)?,
        offset,
    })
}

/// A time in seconds since 1970, from 1 to the year 5138.
fn seconds(a: &Map<String, Value>, name: &'static str) -> Result<Option<i64>, ToolArgError> {
    optional(a, name)
        .map(|value| {
            value
                .as_i64()
                .filter(|seconds| (1..=WHATSAPP_TIME_MAX_S).contains(seconds))
                .ok_or(ToolArgError::Invalid(name))
        })
        .transpose()
}

// ---------------------------------------------------------------------------- schemas

const NUMBER_PATTERN: &str = "^[a-z0-9][a-z0-9_-]{0,31}$";
const MESSAGE_ID_PATTERN: &str = "^[A-Za-z0-9_-]{16,400}$";

fn number_property() -> Value {
    json!({
        "type": "string",
        "pattern": NUMBER_PATTERN,
        "description": "Alias of the WhatsApp number. May be left out when this Hermes has a single number."
    })
}

fn chat_property(what: &str) -> Value {
    json!({
        "type": "string",
        "format": "uuid",
        "description": format!("Identifier of the {what}, as a listing or a notice gave it.")
    })
}

fn to_property() -> Value {
    json!({
        "type": "string",
        "maxLength": 36,
        "description": "A telephone with the code of the country, like +5511987654321, or the identifier of a chat or group as a listing gave it."
    })
}

fn message_property(description: &str) -> Value {
    json!({
        "type": "string",
        "pattern": MESSAGE_ID_PATTERN,
        "description": description
    })
}

fn limit_property(default: u32, max: u32, what: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "maximum": max,
        "default": default,
        "description": format!("How many {what}.")
    })
}

fn offset_property() -> Value {
    json!({
        "type": "integer",
        "minimum": 0,
        "maximum": WHATSAPP_OFFSET_MAX,
        "default": 0,
        "description": "How many items of the list to skip."
    })
}

fn text_property(what: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "maxLength": WHATSAPP_TEXT_MAX_BYTES,
        "description": format!("{what}, as plain text: at most {WHATSAPP_TEXT_MAX_BYTES} bytes in UTF-8.")
    })
}

fn object(properties: Value, required: &[&str]) -> Value {
    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false
    });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}

pub(super) fn status_schema() -> Value {
    object(json!({"number": number_property()}), &[])
}

pub(super) fn list_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "limit": limit_property(WHATSAPP_LIST_DEFAULT, WHATSAPP_LIST_MAX, "items to list"),
            "offset": offset_property()
        }),
        &[],
    )
}

pub(super) fn read_chat_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "chat": chat_property("chat"),
            "limit": limit_property(WHATSAPP_READ_DEFAULT, WHATSAPP_READ_MAX, "messages to read"),
            "before": {
                "type": "integer",
                "minimum": 1,
                "maximum": WHATSAPP_TIME_MAX_S,
                "description": "Only messages older than this time, in seconds since 1970."
            },
            "after": {
                "type": "integer",
                "minimum": 1,
                "maximum": WHATSAPP_TIME_MAX_S,
                "description": "Only messages at or after this time, in seconds since 1970."
            }
        }),
        &["chat"],
    )
}

pub(super) fn chat_schema() -> Value {
    object(
        json!({"number": number_property(), "chat": chat_property("chat")}),
        &["chat"],
    )
}

pub(super) fn message_schema() -> Value {
    object(
        json!({"number": number_property(), "message_id": message_property("Identifier of the message, exactly as a reading gave it.")}),
        &["message_id"],
    )
}

pub(super) fn get_group_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "chat": chat_property("group"),
            "limit": limit_property(
                WHATSAPP_PARTICIPANTS_DEFAULT,
                WHATSAPP_PARTICIPANTS_MAX,
                "participants to show"
            ),
            "offset": offset_property()
        }),
        &["chat"],
    )
}

pub(super) fn check_number_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "phone": {
                "type": "string",
                "maxLength": PHONE_TEXT_MAX_LEN,
                "description": "The telephone, with the code of the country, like +5511987654321."
            }
        }),
        &["phone"],
    )
}

pub(super) fn send_text_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "to": to_property(),
            "text": text_property("The message"),
            "reply_to": message_property("Identifier of the message to answer, exactly as a reading gave it."),
            "mentions": {
                "type": "array",
                "maxItems": WHATSAPP_MENTIONS_MAX,
                "items": {"type": "string", "maxLength": PHONE_TEXT_MAX_LEN},
                "description": "Telephones of people of the group to mark, with the code of the country."
            }
        }),
        &["to", "text"],
    )
}

pub(super) fn send_file_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "to": to_property(),
            "kind": {
                "type": "string",
                "enum": ["image", "document", "voice"],
                "description": "How the file shows in the chat."
            },
            "mimetype": {
                "type": "string",
                "maxLength": MIMETYPE_MAX_LEN,
                "description": "The media type of the file, like image/jpeg or application/pdf."
            },
            "filename": {
                "type": "string",
                "maxLength": FILENAME_MAX_LEN,
                "description": "The name of the file, for a document."
            },
            "data": {
                "type": "string",
                "contentEncoding": "base64",
                "description": "The file in standard base64, with padding: at most 3 MB once decoded."
            },
            "caption": text_property("The text that goes with an image or a document")
        }),
        &["to", "kind", "mimetype", "data"],
    )
}

pub(super) fn set_typing_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "chat": chat_property("chat"),
            "on": {"type": "boolean", "description": "true turns typing on, false turns it off."}
        }),
        &["chat", "on"],
    )
}

pub(super) fn react_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "message_id": message_property("Identifier of the message to react to, exactly as a reading gave it."),
            "reaction": {
                "type": "string",
                "maxLength": REACTION_MAX_BYTES,
                "description": "One emoji, or empty to take the reaction away."
            }
        }),
        &["message_id", "reaction"],
    )
}

pub(super) fn forward_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "message_id": message_property("Identifier of the message to forward, exactly as a reading gave it."),
            "to": to_property()
        }),
        &["message_id", "to"],
    )
}

pub(super) fn edit_schema() -> Value {
    object(
        json!({
            "number": number_property(),
            "message_id": message_property("Identifier of the message to edit, exactly as a reading gave it."),
            "text": text_property("The new text")
        }),
        &["message_id", "text"],
    )
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tools::ToolCall;

    const CHAT: &str = "0b9c5f5e-6f0a-4c3e-9d54-1c2f3a4b5c6d";
    const MESSAGE: &str = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldY";

    fn args(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other => panic!("not an object: {other}"),
        }
    }

    fn call(tool: &str, value: Value) -> Result<WhatsappCall, ToolArgError> {
        match ToolCall::parse(tool, &args(value))? {
            ToolCall::Whatsapp(call) => Ok(call),
            other => panic!("not a WhatsApp call: {other:?}"),
        }
    }

    fn chat() -> Uuid {
        Uuid::try_parse(CHAT).unwrap()
    }

    /// One call of each WhatsApp tool, in the order of the catalog.
    pub(crate) fn one_valid_call_of_each_tool() -> Vec<ToolCall> {
        let data = crate::base64::encode(b"%PDF-1.4");
        [
            (WHATSAPP_STATUS, json!({})),
            (WHATSAPP_LIST_CHATS, json!({})),
            (WHATSAPP_READ_CHAT, json!({"chat": CHAT})),
            (WHATSAPP_GET_CONTACT, json!({"chat": CHAT})),
            (WHATSAPP_DOWNLOAD_MEDIA, json!({"message_id": MESSAGE})),
            (WHATSAPP_LIST_GROUPS, json!({})),
            (WHATSAPP_GET_GROUP, json!({"chat": CHAT})),
            (WHATSAPP_CHECK_NUMBER, json!({"phone": "+5511987650001"})),
            (WHATSAPP_SEND_TEXT, json!({"to": CHAT, "text": "hello"})),
            (
                WHATSAPP_SEND_FILE,
                json!({"to": CHAT, "kind": "document", "mimetype": "application/pdf", "data": data}),
            ),
            (WHATSAPP_MARK_READ, json!({"chat": CHAT})),
            (WHATSAPP_SET_TYPING, json!({"chat": CHAT, "on": true})),
            (WHATSAPP_REACT, json!({"message_id": MESSAGE, "reaction": ""})),
            (WHATSAPP_FORWARD, json!({"message_id": MESSAGE, "to": CHAT})),
            (WHATSAPP_EDIT_MESSAGE, json!({"message_id": MESSAGE, "text": "fixed"})),
            (WHATSAPP_DELETE_MESSAGE, json!({"message_id": MESSAGE})),
        ]
        .into_iter()
        .map(|(tool, value)| ToolCall::parse(tool, &args(value)).unwrap())
        .collect()
    }

    #[test]
    fn every_tool_reads_its_valid_arguments() {
        let calls = one_valid_call_of_each_tool();
        assert_eq!(calls.len(), 16);
        let read = call(
            WHATSAPP_READ_CHAT,
            json!({"number": "balcao", "chat": CHAT, "limit": 200, "before": 1_700_000_000,
                   "after": 1_600_000_000}),
        )
        .unwrap();
        assert_eq!(read.number, Some(NumberAlias::parse("balcao").unwrap()));
        assert_eq!(
            read.op,
            WhatsappOp::ReadChat(ReadChat {
                chat: chat(),
                limit: 200,
                before: Some(1_700_000_000),
                after: Some(1_600_000_000),
            })
        );
        let default = call(WHATSAPP_READ_CHAT, json!({"chat": CHAT, "number": null})).unwrap();
        assert_eq!(default.number, None);
        assert!(matches!(
            default.op,
            WhatsappOp::ReadChat(ReadChat { limit: 30, .. })
        ));
        let listed = call(WHATSAPP_LIST_CHATS, json!({"limit": 100, "offset": 40})).unwrap();
        assert_eq!(
            listed.op,
            WhatsappOp::ListChats(Listing {
                limit: 100,
                offset: 40
            })
        );
        let sent = call(
            WHATSAPP_SEND_TEXT,
            json!({"to": "+55 (11) 98765-0001", "text": "hi", "reply_to": MESSAGE,
                   "mentions": ["5511987650002"]}),
        )
        .unwrap();
        let WhatsappOp::SendText(sent) = sent.op else {
            panic!("not a text");
        };
        assert_eq!(
            sent.to,
            Recipient::Phone(WhatsappPhone::parse("5511987650001").unwrap())
        );
        assert_eq!(sent.reply_to, MessageRef::parse(MESSAGE));
        assert_eq!(sent.mentions.len(), 1);
        let to_chat = call(WHATSAPP_FORWARD, json!({"message_id": MESSAGE, "to": CHAT})).unwrap();
        assert!(matches!(
            to_chat.op,
            WhatsappOp::Forward {
                to: Recipient::Chat(_),
                ..
            }
        ));
        let file = call(
            WHATSAPP_SEND_FILE,
            json!({"to": CHAT, "kind": "image", "mimetype": "image/jpeg",
                   "filename": "photo.jpg", "data": crate::base64::encode(&[1, 2, 3]),
                   "caption": "look"}),
        )
        .unwrap();
        let WhatsappOp::SendFile(file) = file.op else {
            panic!("not a file");
        };
        assert_eq!(file.file.data, [1, 2, 3]);
        assert_eq!(file.file.kind, FileKind::Image);
        assert_eq!(
            file.caption.map(|c| c.expose().to_owned()).as_deref(),
            Some("look")
        );
    }

    /// The values a Hermes could use to reach another path of the engine, another session or
    /// another chat are refused here, before the hub looks at anything else
    /// (`docs/09-contrato-mcp.md`, section 4).
    #[test]
    fn malicious_identifiers_are_refused() {
        for to in [
            "",
            "../../sessions/other",
            "5511987650001@c.us",
            "120363012345678901@g.us",
            "5511%2e%2e",
            "+55 11 9876\u{0}50001",
            "5511987650001/../x",
            "0011987650001",
            "1234567",
            "1234567890123456",
            "{0b9c5f5e-6f0a-4c3e-9d54-1c2f3a4b5c6d}",
            "0b9c5f5e6f0a4c3e9d541c2f3a4b5c6d",
            "default",
            "status@broadcast",
        ] {
            assert_eq!(
                call(WHATSAPP_SEND_TEXT, json!({"to": to, "text": "hi"})),
                Err(ToolArgError::Invalid("to")),
                "{to:?}"
            );
        }
        let long = "A".repeat(401);
        for message in [
            "",
            "short",
            "../../../../../../x",
            "true_5511987650001@c.us_3EB0ABCDEF",
            "QUJD%2E%2EREVGR0hJSktMTU5PUFFS",
            "QUJDREVGR0hJSktM TU5PUFFSU1RVVldY",
            "QUJDREVGR0hJSktM/TU5PUFFSU1RVVldY",
            "QUJDREVGR0hJSktM+TU5PUFFSU1RVVldY",
            long.as_str(),
        ] {
            assert_eq!(
                call(WHATSAPP_DELETE_MESSAGE, json!({"message_id": message})),
                Err(ToolArgError::Invalid("message_id")),
                "{message:?}"
            );
        }
        for chat in ["", "x", "../x", "5511987650001@c.us", "0"] {
            assert_eq!(
                call(WHATSAPP_READ_CHAT, json!({"chat": chat})),
                Err(ToolArgError::Invalid("chat")),
                "{chat:?}"
            );
        }
        let too_long = "a".repeat(33);
        for number in ["", "Balcao", "a b", "admin", "../x", too_long.as_str()] {
            assert_eq!(
                call(WHATSAPP_STATUS, json!({"number": number})),
                Err(ToolArgError::Invalid("number")),
                "{number:?}"
            );
        }
        assert_eq!(
            call(WHATSAPP_STATUS, json!({"number": 7})),
            Err(ToolArgError::Invalid("number"))
        );
        // A session of the engine is never an argument.
        assert_eq!(
            call(WHATSAPP_STATUS, json!({"session": "abcdefghijklmnopq"})),
            Err(ToolArgError::UnknownArgument)
        );
    }

    #[test]
    fn missing_and_invalid_arguments_are_refused() {
        let cases = [
            (WHATSAPP_READ_CHAT, json!({}), ToolArgError::Missing("chat")),
            (
                WHATSAPP_READ_CHAT,
                json!({"chat": CHAT, "limit": 0}),
                ToolArgError::Invalid("limit"),
            ),
            (
                WHATSAPP_READ_CHAT,
                json!({"chat": CHAT, "limit": 201}),
                ToolArgError::Invalid("limit"),
            ),
            (
                WHATSAPP_READ_CHAT,
                json!({"chat": CHAT, "before": 0}),
                ToolArgError::Invalid("before"),
            ),
            (
                WHATSAPP_READ_CHAT,
                json!({"chat": CHAT, "after": -5}),
                ToolArgError::Invalid("after"),
            ),
            (
                WHATSAPP_READ_CHAT,
                json!({"chat": CHAT, "before": "1700000000"}),
                ToolArgError::Invalid("before"),
            ),
            (
                WHATSAPP_READ_CHAT,
                json!({"chat": CHAT, "after": 100_000_000_000i64}),
                ToolArgError::Invalid("after"),
            ),
            (
                WHATSAPP_LIST_CHATS,
                json!({"limit": 101}),
                ToolArgError::Invalid("limit"),
            ),
            (
                WHATSAPP_LIST_CHATS,
                json!({"offset": -1}),
                ToolArgError::Invalid("offset"),
            ),
            (
                WHATSAPP_LIST_CHATS,
                json!({"offset": 100_001}),
                ToolArgError::Invalid("offset"),
            ),
            (
                WHATSAPP_GET_GROUP,
                json!({"chat": CHAT, "limit": 201}),
                ToolArgError::Invalid("limit"),
            ),
            (
                WHATSAPP_CHECK_NUMBER,
                json!({}),
                ToolArgError::Missing("phone"),
            ),
            (
                WHATSAPP_CHECK_NUMBER,
                json!({"phone": CHAT}),
                ToolArgError::Invalid("phone"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"text": "hi"}),
                ToolArgError::Missing("to"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT}),
                ToolArgError::Missing("text"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": ""}),
                ToolArgError::Invalid("text"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": " \n\t "}),
                ToolArgError::Invalid("text"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": "a\u{0}b"}),
                ToolArgError::Invalid("text"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": "a".repeat(WHATSAPP_TEXT_MAX_BYTES + 1)}),
                ToolArgError::Invalid("text"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": "hi", "mentions": "5511987650002"}),
                ToolArgError::Invalid("mentions"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": "hi", "mentions": ["x"]}),
                ToolArgError::Invalid("mentions"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": "hi", "mentions": vec!["5511987650002"; 51]}),
                ToolArgError::Invalid("mentions"),
            ),
            (
                WHATSAPP_SEND_TEXT,
                json!({"to": CHAT, "text": "hi", "reply_to": "x"}),
                ToolArgError::Invalid("reply_to"),
            ),
            (
                WHATSAPP_SET_TYPING,
                json!({"chat": CHAT}),
                ToolArgError::Missing("on"),
            ),
            (
                WHATSAPP_SET_TYPING,
                json!({"chat": CHAT, "on": "yes"}),
                ToolArgError::Invalid("on"),
            ),
            (
                WHATSAPP_REACT,
                json!({"message_id": MESSAGE}),
                ToolArgError::Missing("reaction"),
            ),
            (
                WHATSAPP_REACT,
                json!({"message_id": MESSAGE, "reaction": "x".repeat(33)}),
                ToolArgError::Invalid("reaction"),
            ),
            (
                WHATSAPP_REACT,
                json!({"message_id": MESSAGE, "reaction": "\u{7}"}),
                ToolArgError::Invalid("reaction"),
            ),
            (
                WHATSAPP_EDIT_MESSAGE,
                json!({"message_id": MESSAGE}),
                ToolArgError::Missing("text"),
            ),
            (
                WHATSAPP_EDIT_MESSAGE,
                json!({"message_id": MESSAGE, "text": null}),
                ToolArgError::Missing("text"),
            ),
            (
                WHATSAPP_DELETE_MESSAGE,
                json!({}),
                ToolArgError::Missing("message_id"),
            ),
            (
                WHATSAPP_FORWARD,
                json!({"message_id": MESSAGE}),
                ToolArgError::Missing("to"),
            ),
            (
                WHATSAPP_STATUS,
                json!({"number": "balcao", "chat": CHAT}),
                ToolArgError::UnknownArgument,
            ),
        ];
        for (tool, arguments, expected) in cases {
            let shown = arguments.to_string();
            let shown = &shown[..shown.len().min(80)];
            assert_eq!(call(tool, arguments), Err(expected), "{tool}: {shown}");
        }
    }

    #[test]
    fn a_file_is_read_strictly_and_never_above_3_mb() {
        let file = |extra: Value| {
            let mut value = json!({"to": CHAT, "kind": "document", "mimetype": "application/pdf",
                                   "data": crate::base64::encode(b"x")});
            for (key, item) in args(extra) {
                value[key] = item;
            }
            call(WHATSAPP_SEND_FILE, value)
        };
        assert!(file(json!({})).is_ok());
        let largest = vec![7u8; WHATSAPP_SEND_FILE_MAX_BYTES];
        assert!(file(json!({"data": crate::base64::encode(&largest)})).is_ok());
        let above = vec![7u8; WHATSAPP_SEND_FILE_MAX_BYTES + 1];
        assert_eq!(
            file(json!({"data": crate::base64::encode(&above)})),
            Err(ToolArgError::Invalid("data"))
        );
        for data in ["", "eA", "eA==eA==", "e A==", "@@@@"] {
            assert_eq!(
                file(json!({"data": data})),
                Err(ToolArgError::Invalid("data")),
                "{data:?}"
            );
        }
        assert!(file(json!({"data": "eA=="})).is_ok());
        for kind in ["", "video", "IMAGE", "sticker"] {
            assert_eq!(
                file(json!({"kind": kind})),
                Err(ToolArgError::Invalid("kind"))
            );
        }
        for mimetype in ["", "pdf", "application/", "a/b\r\nX: y", "a b/c", "a/b; c"] {
            assert_eq!(
                file(json!({"mimetype": mimetype})),
                Err(ToolArgError::Invalid("mimetype")),
                "{mimetype:?}"
            );
        }
        for filename in ["", ".hidden", "../x", "a/b", "a\\b", "a\nb"] {
            assert_eq!(
                file(json!({"filename": filename})),
                Err(ToolArgError::Invalid("filename")),
                "{filename:?}"
            );
        }
    }

    #[test]
    fn the_verbs_and_the_send_limit_of_each_tool() {
        use WhatsappVerb::{Modify, Read, Send};
        let verbs: Vec<(&str, WhatsappVerb, bool)> = one_valid_call_of_each_tool()
            .iter()
            .map(|call| match call {
                ToolCall::Whatsapp(call) => (
                    call.op.name(),
                    call.op.verb(),
                    call.op.counts_toward_the_send_limit(),
                ),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            verbs,
            [
                (WHATSAPP_STATUS, Read, false),
                (WHATSAPP_LIST_CHATS, Read, false),
                (WHATSAPP_READ_CHAT, Read, false),
                (WHATSAPP_GET_CONTACT, Read, false),
                (WHATSAPP_DOWNLOAD_MEDIA, Read, false),
                (WHATSAPP_LIST_GROUPS, Read, false),
                (WHATSAPP_GET_GROUP, Read, false),
                (WHATSAPP_CHECK_NUMBER, Send, true),
                (WHATSAPP_SEND_TEXT, Send, true),
                (WHATSAPP_SEND_FILE, Send, true),
                (WHATSAPP_MARK_READ, Send, false),
                (WHATSAPP_SET_TYPING, Send, false),
                (WHATSAPP_REACT, Send, true),
                (WHATSAPP_FORWARD, Send, true),
                (WHATSAPP_EDIT_MESSAGE, Modify, false),
                (WHATSAPP_DELETE_MESSAGE, Modify, false),
            ]
        );
    }

    #[test]
    fn the_schemas_state_the_limits_the_hub_checks() {
        let schema = |name: &str| crate::tools::find(name).unwrap().input_schema();
        let read = schema(WHATSAPP_READ_CHAT);
        assert_eq!(read["required"], json!(["chat"]));
        assert_eq!(read["properties"]["limit"]["maximum"], 200);
        assert_eq!(read["properties"]["limit"]["default"], 30);
        let send = schema(WHATSAPP_SEND_TEXT);
        assert_eq!(send["required"], json!(["to", "text"]));
        assert_eq!(
            send["properties"]["text"]["maxLength"],
            WHATSAPP_TEXT_MAX_BYTES
        );
        assert_eq!(
            send["properties"]["mentions"]["maxItems"],
            WHATSAPP_MENTIONS_MAX
        );
        assert_eq!(
            schema(WHATSAPP_EDIT_MESSAGE)["required"],
            json!(["message_id", "text"])
        );
        assert_eq!(
            schema(WHATSAPP_DELETE_MESSAGE)["required"],
            json!(["message_id"])
        );
        assert_eq!(
            schema(WHATSAPP_SEND_FILE)["required"],
            json!(["to", "kind", "mimetype", "data"])
        );
        for name in [WHATSAPP_STATUS, WHATSAPP_LIST_CHATS, WHATSAPP_LIST_GROUPS] {
            assert!(schema(name).get("required").is_none(), "{name}");
        }
        // Every WhatsApp tool takes the alias of the number, in the format of a name.
        for tool in crate::tools::CATALOG
            .iter()
            .filter(|tool| tool.name.starts_with("whatsapp_"))
        {
            let schema = tool.input_schema();
            assert_eq!(
                schema["properties"]["number"]["pattern"], NUMBER_PATTERN,
                "{}",
                tool.name
            );
        }
    }

    #[test]
    fn telephones_texts_files_and_identifiers_do_not_show_in_debug() {
        let phone = WhatsappPhone::parse("+5511987650001").unwrap();
        assert_eq!(phone.digits(), "5511987650001");
        let sent = call(
            WHATSAPP_SEND_TEXT,
            json!({"to": "+5511987650001", "text": "CANARY text", "reply_to": MESSAGE,
                   "mentions": ["5511987650002"]}),
        )
        .unwrap();
        let shown = format!("{sent:?}");
        for hidden in ["987650001", "987650002", "CANARY", MESSAGE] {
            assert!(!shown.contains(hidden), "{shown}");
        }
        let file = call(
            WHATSAPP_SEND_FILE,
            json!({"to": CHAT, "kind": "image", "mimetype": "image/png",
                   "filename": "CANARY.png", "data": crate::base64::encode(b"CANARY")}),
        )
        .unwrap();
        let shown = format!("{file:?}");
        assert!(
            !shown.contains("CANARY") && !shown.contains("Q0FOQVJZ"),
            "{shown}"
        );
    }
}
