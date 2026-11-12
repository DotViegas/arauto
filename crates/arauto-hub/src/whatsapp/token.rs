//! The identifier of a message as the hub gives it to a Hermes (`message_id`).
//!
//! A Hermes never names a message of the engine by itself: it gives back what a reading gave
//! it. The hub signs what it gives, and a value it did not sign never reaches the engine, so
//! a Hermes cannot point at a message of another chat, of another number or of another
//! organization by writing an identifier of its own (`docs/09-contrato-mcp.md`, section 4).
//!
//! ```text
//! base64url, without padding, of:  chat (16 bytes) | signature (32 bytes) | identifier in the engine
//! signature = HMAC-SHA-256, with the key of the WhatsApp digests of the organization,
//!             over the field, the chat and the identifier
//! ```
//!
//! The pieces are known ones: HMAC-SHA-256 ([`OrganizationKey::whatsapp_digest`]), compared
//! in constant time. The identifier is not hidden, only signed: it carries the chat and the
//! author, which a reading already shows the Hermes. It is the same every time the same
//! message is read, so a Hermes can tell a message it already read.

use data_encoding::BASE64URL_NOPAD;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use arauto_protocol::tools::MessageRef;

use crate::engine::EngineMessageId;
use crate::vault::{OrganizationKey, WhatsappField};

const CHAT_LEN: usize = 16;
const SIGNATURE_LEN: usize = 32;

/// The identifier of `message`, of the chat `chat` of the hub, for the Hermes.
pub(crate) fn sign(key: &OrganizationKey, chat: Uuid, message: &EngineMessageId) -> String {
    let id = message.as_str().as_bytes();
    let signature = key.whatsapp_digest(WhatsappField::MessageRef, chat, id);
    let mut bytes = Vec::with_capacity(CHAT_LEN + SIGNATURE_LEN + id.len());
    bytes.extend_from_slice(chat.as_bytes());
    bytes.extend_from_slice(&signature);
    bytes.extend_from_slice(id);
    BASE64URL_NOPAD.encode(&bytes)
}

/// The chat and the message an identifier given by a Hermes names, when the hub signed it
/// with the key of this organization. `None` for anything else, without saying why.
pub(crate) fn open(key: &OrganizationKey, given: &MessageRef) -> Option<(Uuid, EngineMessageId)> {
    let bytes = BASE64URL_NOPAD.decode(given.as_str().as_bytes()).ok()?;
    let (chat, rest) = bytes.split_at_checked(CHAT_LEN)?;
    let (signature, id) = rest.split_at_checked(SIGNATURE_LEN)?;
    let chat = Uuid::from_slice(chat).ok()?;
    let expected = key.whatsapp_digest(WhatsappField::MessageRef, chat, id);
    if !bool::from(expected.as_slice().ct_eq(signature)) {
        return None;
    }
    let id = std::str::from_utf8(id).ok()?;
    let message = EngineMessageId::parse(id).ok()?;
    Some((chat, message))
}
