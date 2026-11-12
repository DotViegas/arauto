//! `arauto-hub admin access ...` (`docs/15-mvp.md`, section 1): the matrix of accesses
//! (`docs/08-permissoes.md`).
//!
//! The rules of the other commands hold: every write goes through the repositories, which
//! check the database against the audit mark first (a database that came back from an older
//! copy gets nothing); every change is a security event with what was before and after; the
//! actor is the local admin. The hub hears of each change and the sessions of the Hermes get
//! their new list of tools; a call reads the access again anyway, at every call.
//!
//! What the panel does besides deciding, and the command does too: it warns, and never
//! prevents, when sending is given on a number marked as a command channel, or when another
//! Hermes may also send or be woken on the same number (`docs/08-permissoes.md`, sections 5
//! and 6).

use arauto_protocol::names::{HermesName, NumberAlias};
use uuid::Uuid;

use super::{AdminError, Session};
use crate::core::access::{Grant, Verbs};
use crate::engine::{EngineChatId, Phone};
use crate::repo;
use crate::repo::access::{Changed, ChatRef, Listed, Missing};
use crate::vault::Vault;

/// What the admin is told besides the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// The number is marked as the command channel of some Hermes, and this one may now send
    /// on it: it can write into that channel.
    CommandNumber,
    /// Other Hermes may also send or be woken on this number.
    Shared(Vec<HermesName>),
}

/// What `admin access grant` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    /// What was asked, now in force.
    pub grant: Grant,
    /// What was before: `None` for a new access.
    pub before: Option<Grant>,
    /// It already was as asked: nothing was written.
    pub unchanged: bool,
    /// What the admin should know.
    pub warnings: Vec<Warning>,
}

/// What `admin access chat` and `chat-remove` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatChanged {
    /// The chat, by the identifier of the hub.
    pub chat: Option<Uuid>,
    /// Its verbs before.
    pub before: Option<Verbs>,
    /// Its verbs now: `None` when the line was taken away.
    pub after: Option<Verbs>,
    /// It already was as asked: nothing was written.
    pub unchanged: bool,
}

/// One line of `admin access list`, with the chats as the engine knows them.
#[derive(Debug, Clone)]
pub struct ListedAccess {
    /// The access.
    pub access: Listed,
    /// For each line, the identifiers of its chat in the engine. Show them escaped.
    pub chats: Vec<Vec<EngineChatId>>,
}

fn hermes_name(text: &str) -> Result<HermesName, AdminError> {
    HermesName::parse(text).map_err(|_| AdminError::NoSuchHermes)
}

fn alias(text: &str) -> Result<NumberAlias, AdminError> {
    NumberAlias::parse(text).map_err(|_| AdminError::NoSuchNumber)
}

fn missing(missing: Missing) -> AdminError {
    match missing {
        Missing::Hermes => AdminError::NoSuchHermes,
        Missing::Number => AdminError::NoSuchNumber,
        Missing::Access => AdminError::NoAccess,
        Missing::Chat => AdminError::NoSuchChat,
        Missing::Line => AdminError::NoLine,
    }
}

impl Session {
    /// `admin access grant HERMES NUMBER --scope all|list [--read] [--send] [--modify]
    /// [--wake] [--start-chats]`: the access, whole.
    ///
    /// # Errors
    /// [`AdminError::AccessRule`]; [`AdminError::NoSuchHermes`]; [`AdminError::NoSuchNumber`];
    /// [`AdminError::Restored`].
    pub async fn access_grant(
        &self,
        hermes: &str,
        number: &str,
        grant: Grant,
        now: i64,
    ) -> Result<Granted, AdminError> {
        grant.check().map_err(AdminError::AccessRule)?;
        let name = hermes_name(hermes)?;
        let alias = alias(number)?;
        let changed = repo::access::grant(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            &alias,
            &grant,
            now,
        )
        .await?;
        let (before, unchanged) = match changed {
            Changed::Done(before) => (before, false),
            Changed::Unchanged => (Some(grant), true),
            Changed::Missing(what) => return Err(missing(what)),
        };
        let mut warnings = Vec::new();
        let sends = grant.verbs.send;
        if sends
            && repo::number::by_alias(&self.pool, self.organization, &alias)
                .await?
                .is_some_and(|number| number.command)
        {
            warnings.push(Warning::CommandNumber);
        }
        if sends || grant.verbs.wake {
            let others =
                repo::access::others_sending_or_woken(&self.pool, self.organization, &alias, &name)
                    .await?;
            if !others.is_empty() {
                warnings.push(Warning::Shared(others));
            }
        }
        Ok(Granted {
            grant,
            before,
            unchanged,
            warnings,
        })
    }

    /// `admin access remove HERMES NUMBER`: the access ends, with the lines of its chats.
    /// Returns how many lines went with it.
    ///
    /// # Errors
    /// [`AdminError::NoAccess`]; [`AdminError::NoSuchHermes`]; [`AdminError::NoSuchNumber`];
    /// [`AdminError::Restored`].
    pub async fn access_remove(
        &self,
        hermes: &str,
        number: &str,
        now: i64,
    ) -> Result<u64, AdminError> {
        let name = hermes_name(hermes)?;
        let alias = alias(number)?;
        match repo::access::remove(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            &alias,
            now,
        )
        .await?
        {
            Changed::Done(lines) => Ok(lines),
            Changed::Unchanged => Ok(0),
            Changed::Missing(what) => Err(missing(what)),
        }
    }

    /// `admin access wake-wait HERMES NUMBER SECONDS`: how long the notice of a chat waits,
    /// after its last message, before it wakes the Hermes. Returns the seconds before, or
    /// `None` when it already was so.
    ///
    /// # Errors
    /// [`AdminError::WakeWaitRange`]; [`AdminError::NoAccess`]; [`AdminError::NoSuchHermes`];
    /// [`AdminError::NoSuchNumber`]; [`AdminError::Restored`].
    pub async fn access_wake_wait(
        &self,
        hermes: &str,
        number: &str,
        seconds: u32,
        now: i64,
    ) -> Result<Option<u32>, AdminError> {
        if seconds > 60 {
            return Err(AdminError::WakeWaitRange);
        }
        let name = hermes_name(hermes)?;
        let alias = alias(number)?;
        let wait_ms = i32::try_from(seconds.saturating_mul(1_000)).unwrap_or(60_000);
        match repo::access::set_wake_wait(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            &alias,
            wait_ms,
            now,
        )
        .await?
        {
            Changed::Done(before) => Ok(Some(u32::try_from(before / 1_000).unwrap_or(0))),
            Changed::Unchanged => Ok(None),
            Changed::Missing(what) => Err(missing(what)),
        }
    }

    /// The chat the admin names: the identifier the hub gave it, an identifier of the engine
    /// (`...@c.us`, `...@g.us`, `...@lid`), or a telephone, whose exact chat the engine gives
    /// (with or without the ninth digit).
    async fn chat_ref(
        &self,
        vault: &Vault,
        number: &NumberAlias,
        text: &str,
    ) -> Result<ChatRef, AdminError> {
        let text = text.trim();
        if let Ok(id) = Uuid::try_parse(text)
            && text.len() == 36
        {
            return Ok(ChatRef::Hub(id));
        }
        if text.contains('@') {
            return EngineChatId::parse(text)
                .map(ChatRef::Engine)
                .map_err(|_| AdminError::InvalidChat);
        }
        let phone = Phone::parse(text).map_err(|_| AdminError::InvalidChat)?;
        let found = repo::number::by_alias(&self.pool, self.organization, number)
            .await?
            .ok_or(AdminError::NoSuchNumber)?;
        let engine = self.engine(vault).await?;
        engine
            .client
            .check_number(&found.session, &phone)
            .await?
            .map(ChatRef::Engine)
            .ok_or(AdminError::NoWhatsapp)
    }

    /// `admin access chat HERMES NUMBER CHAT [--read] [--send] [--modify] [--wake]`: the line
    /// of the chat, whose verbs replace those of the number there; with none, the chat is
    /// blocked.
    ///
    /// # Errors
    /// [`AdminError::AccessRule`]; [`AdminError::InvalidChat`]; [`AdminError::NoAccess`];
    /// [`AdminError::NoSuchChat`]; [`AdminError::Restored`].
    pub async fn access_chat(
        &self,
        vault: &Vault,
        hermes: &str,
        number: &str,
        chat: &str,
        verbs: Verbs,
        now: i64,
    ) -> Result<ChatChanged, AdminError> {
        verbs.check().map_err(AdminError::AccessRule)?;
        let name = hermes_name(hermes)?;
        let alias = alias(number)?;
        let chat = self.chat_ref(vault, &alias, chat).await?;
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        match repo::access::set_chat(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &key,
            &name,
            &alias,
            &chat,
            verbs,
            now,
        )
        .await?
        {
            Changed::Done((chat, before)) => Ok(ChatChanged {
                chat: Some(chat),
                before,
                after: Some(verbs),
                unchanged: false,
            }),
            Changed::Unchanged => Ok(ChatChanged {
                chat: None,
                before: Some(verbs),
                after: Some(verbs),
                unchanged: true,
            }),
            Changed::Missing(what) => Err(missing(what)),
        }
    }

    /// `admin access chat-remove HERMES NUMBER CHAT`: the line goes, and the verbs of the
    /// number hold in the chat again (with the scope "list", the chat leaves the scope).
    ///
    /// # Errors
    /// [`AdminError::InvalidChat`]; [`AdminError::NoAccess`]; [`AdminError::NoLine`];
    /// [`AdminError::Restored`].
    pub async fn access_chat_remove(
        &self,
        vault: &Vault,
        hermes: &str,
        number: &str,
        chat: &str,
        now: i64,
    ) -> Result<ChatChanged, AdminError> {
        let name = hermes_name(hermes)?;
        let alias = alias(number)?;
        let chat = self.chat_ref(vault, &alias, chat).await?;
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        match repo::access::remove_chat(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &key,
            &name,
            &alias,
            &chat,
            now,
        )
        .await?
        {
            Changed::Done((chat, before)) => Ok(ChatChanged {
                chat: Some(chat),
                before: Some(before),
                after: None,
                unchanged: false,
            }),
            Changed::Unchanged => Ok(ChatChanged {
                chat: None,
                before: None,
                after: None,
                unchanged: true,
            }),
            Changed::Missing(what) => Err(missing(what)),
        }
    }

    /// `admin access list [HERMES]`.
    ///
    /// # Errors
    /// What the database or the vault return.
    pub async fn access_list(
        &self,
        vault: &Vault,
        hermes: Option<&str>,
    ) -> Result<Vec<ListedAccess>, AdminError> {
        let name = hermes.map(hermes_name).transpose()?;
        let listed = repo::access::list(&self.pool, self.organization, name.as_ref()).await?;
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        let mut shown = Vec::with_capacity(listed.len());
        for access in listed {
            let number = repo::number::by_alias(&self.pool, self.organization, &access.number)
                .await?
                .ok_or(AdminError::NoSuchNumber)?;
            let mut chats = Vec::with_capacity(access.lines.len());
            for (chat, _) in &access.lines {
                chats.push(
                    repo::chat::engine_ids(&self.pool, self.organization, &key, number.id, *chat)
                        .await?,
                );
            }
            shown.push(ListedAccess { access, chats });
        }
        Ok(shown)
    }
}
