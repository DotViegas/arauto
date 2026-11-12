//! The engine: the only client of the WhatsApp engine and the only receiver of its events
//! (`docs/03-arquitetura.md`, section 3; `docs/15-mvp.md`, section 4).
//!
//! | Module | What it holds |
//! |---|---|
//! | [`types`] | The requests and answers, with every identifier checked by its type. |
//! | [`error`] | The typed errors, each with its code of the catalog. |
//! | [`config`] | The address of the engine, the address of the events, the key and the secret of the events. |
//! | `client` | The HTTP/1.1 client: deadlines, ceilings, type of content. |
//! | [`waha`] | The adapter of the WAHA. |
//! | [`registry`] | The engine of each organization, loaded from the database and rebuilt when it changes. |
//! | `inbox` | The inbox of events, on the internal port. |
//! | `supervisor` | The check of the engine at the start and every 5 minutes: the version, the configuration of each session, the catch-up of what arrived while the hub was away. |
//!
//! [`WhatsappEngine`] is the interface. Every care of `docs/09-contrato-mcp.md` section 4 and
//! every trap of `referencia/waha-fatos.md` is inside the adapter or in the types: whoever calls
//! cannot send a path segment that was not checked, an edit without a text, a delete without
//! the complete identifier, or a telephone in one form only.

pub mod config;
pub mod error;
pub mod registry;
pub mod types;
pub mod waha;

mod client;
pub(crate) mod inbox;
pub(crate) mod supervisor;

pub use supervisor::supervise;

use futures_util::future::BoxFuture;

pub use client::Deadlines;
pub use config::{AddressError, ApiKey, EngineAddress, EventsSecret, EventsUrl};
pub use error::{Cause, EngineError};
pub use registry::{EngineSettings, Engines};
pub use types::*;

/// The configuration the hub gives each session of the engine. It is applied whole, and only
/// when the session has another one (`referencia/waha-fatos.md`, the first two traps).
pub struct SessionConfig<'a> {
    /// Where the events of the session go: the inbox of this engine on the internal port.
    pub events_url: String,
    /// The secret that signs them.
    pub events_secret: &'a EventsSecret,
}

impl std::fmt::Debug for SessionConfig<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionConfig([redacted])")
    }
}

/// The operations the hub asks of an engine.
///
/// Grouped as `docs/03-arquitetura.md` section 3 says: the server, the life of a session of the
/// engine, reading, sending and changing. Each call has its deadline and its ceilings inside
/// the adapter, and a slow or hostile answer for one session never holds a call for another.
pub trait WhatsappEngine: Send + Sync + std::fmt::Debug {
    // ------------------------------------------------------------------ the server

    /// Whether the engine answers and says it is healthy.
    fn health(&self) -> BoxFuture<'_, Result<(), EngineError>>;

    /// The version and the internal mechanism of the engine.
    fn version(&self) -> BoxFuture<'_, Result<EngineVersion, EngineError>>;

    /// SHA-256 of the whole configuration this adapter gives a session: what the hub records
    /// once it is applied.
    fn config_digest(&self, config: &SessionConfig<'_>) -> [u8; 32];

    // ------------------------------------------------------------------ sessions of the engine

    /// The session, or `None` when the engine does not have it.
    fn session<'a>(
        &'a self,
        session: &'a SessionName,
    ) -> BoxFuture<'a, Result<Option<SessionInfo>, EngineError>>;

    /// Creates the session already configured, when missing, or applies the whole
    /// configuration when the one it has differs. `applied_before` says the hub applied this
    /// very configuration last time: an engine that does not show back the secret of the
    /// events is then not reconfigured for that alone.
    fn ensure_session<'a>(
        &'a self,
        session: &'a SessionName,
        config: &'a SessionConfig<'a>,
        applied_before: bool,
    ) -> BoxFuture<'a, Result<Ensured, EngineError>>;

    /// Starts the session. Starting a started session does nothing.
    fn start<'a>(&'a self, session: &'a SessionName) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Stops the session, without leaving WhatsApp: the history in the engine stays.
    fn stop<'a>(&'a self, session: &'a SessionName) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Restarts the session.
    fn restart<'a>(&'a self, session: &'a SessionName) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Leaves WhatsApp and deletes the session. **The engine erases the history it keeps**
    /// (`referencia/waha-fatos.md`): the admin command asks for it in so many words.
    fn log_out_and_delete<'a>(
        &'a self,
        session: &'a SessionName,
    ) -> BoxFuture<'a, Result<(), EngineError>>;

    /// What the QR of the pairing holds now. It changes every few seconds.
    fn pairing_qr<'a>(
        &'a self,
        session: &'a SessionName,
    ) -> BoxFuture<'a, Result<PairingQr, EngineError>>;

    /// Asks for the code to type on the telephone `phone`.
    fn pairing_code<'a>(
        &'a self,
        session: &'a SessionName,
        phone: &'a Phone,
    ) -> BoxFuture<'a, Result<PairingCode, EngineError>>;

    // ------------------------------------------------------------------ reading

    /// The chats, the most recent first, with the last message.
    fn chats<'a>(
        &'a self,
        session: &'a SessionName,
        page: Page,
    ) -> BoxFuture<'a, Result<Vec<ChatSummary>, EngineError>>;

    /// The messages of a chat, the newest first.
    fn messages<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
        query: MessageQuery,
    ) -> BoxFuture<'a, Result<Vec<Message>, EngineError>>;

    /// The public data of a person.
    fn contact<'a>(
        &'a self,
        session: &'a SessionName,
        contact: &'a EngineChatId,
    ) -> BoxFuture<'a, Result<Contact, EngineError>>;

    /// The file of a message, up to 7 MB.
    fn download_media<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
        message: &'a EngineMessageId,
    ) -> BoxFuture<'a, Result<FileContent, EngineError>>;

    /// The groups of the number, each with how many participants it has.
    fn groups<'a>(
        &'a self,
        session: &'a SessionName,
        page: Page,
    ) -> BoxFuture<'a, Result<Vec<GroupSummary>, EngineError>>;

    /// A group, with one page of its participants.
    fn group<'a>(
        &'a self,
        session: &'a SessionName,
        group: &'a EngineChatId,
        participants: Page,
    ) -> BoxFuture<'a, Result<GroupDetail, EngineError>>;

    // ------------------------------------------------------------------ sending

    /// Whether `phone` has WhatsApp, and the exact identifier of its chat. Both forms of a
    /// Brazilian mobile are tried. Writes to nobody.
    fn check_number<'a>(
        &'a self,
        session: &'a SessionName,
        phone: &'a Phone,
    ) -> BoxFuture<'a, Result<Option<EngineChatId>, EngineError>>;

    /// Sends a text, optionally answering a message and marking people of a group.
    fn send_text<'a>(
        &'a self,
        session: &'a SessionName,
        text: &'a SendText,
    ) -> BoxFuture<'a, Result<Sent, EngineError>>;

    /// Sends a file by content, up to 3 MB.
    fn send_file<'a>(
        &'a self,
        session: &'a SessionName,
        file: &'a SendFile,
    ) -> BoxFuture<'a, Result<Sent, EngineError>>;

    /// Marks the chat as read.
    fn mark_read<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
    ) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Turns "typing" on or off.
    fn set_typing<'a>(
        &'a self,
        session: &'a SessionName,
        chat: &'a EngineChatId,
        on: bool,
    ) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Reacts to a message; an empty reaction takes it away.
    fn react<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a EngineMessageId,
        reaction: &'a Reaction,
    ) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Forwards a message to another chat.
    fn forward<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a EngineMessageId,
        to: &'a EngineChatId,
    ) -> BoxFuture<'a, Result<Sent, EngineError>>;

    // ------------------------------------------------------------------ changing

    /// Edits the text of a message the number sent. The text is not optional, and the
    /// message is identified completely.
    fn edit_message<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a FullMessageId,
        text: &'a MessageBody,
    ) -> BoxFuture<'a, Result<(), EngineError>>;

    /// Deletes a message for everyone. The message is identified completely.
    fn delete_message<'a>(
        &'a self,
        session: &'a SessionName,
        message: &'a FullMessageId,
    ) -> BoxFuture<'a, Result<(), EngineError>>;
}
