//! The matrix of accesses (`docs/06-modelo-de-dados.md`, section 5; `docs/08-permissoes.md`):
//! one access per pair (Hermes, number), and the lines of the chats under it. The rule that
//! reads them is [`crate::core::access`].
//!
//! Every change is a security event, with what was before and what is after, written through
//! [`crate::audit::begin_security`]: a database that came back from an older copy gets
//! nothing. Each change tells the hub (`hermes:{id}`), and the sessions of that Hermes read
//! their list of tools again; the sweep finds it in 2 seconds anyway. A call never trusts
//! that list: it reads its access here, at every call.

use std::collections::HashMap;

use arauto_protocol::names::{HermesName, NumberAlias};
use arauto_protocol::tools::Grants;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Admin, RepoError, notify_hub};
use crate::audit::{self, Action, Actor, Details, Event, SecurityWrite, TargetKind};
use crate::core::access::{Grant, Scope, Verb, Verbs};
use crate::core::number::NumberState;
use crate::db::ResolvedOrg;
use crate::engine::{EngineChatId, SessionName};
use crate::mark::Mark;
use crate::vault::OrganizationKey;

/// A number as a call of a Hermes sees it through its access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessedNumber {
    /// The row of the number.
    pub id: Uuid,
    /// Its alias.
    pub alias: NumberAlias,
    /// Its state.
    pub state: NumberState,
    /// The name of its session in the engine: never shown to the Hermes.
    pub session: SessionName,
}

/// An access of a Hermes, with its number, as a call reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesAccess {
    /// The row of the access.
    pub id: Uuid,
    /// The scope and the verbs of the number.
    pub grant: Grant,
    /// The number.
    pub number: AccessedNumber,
    /// Which verbs some line of a chat has.
    pub in_lines: Verbs,
}

impl HermesAccess {
    /// Whether this access has `verb` on the number or in some chat. A number whose telephone
    /// was swapped has its accesses stopped until the admin decides
    /// (`docs/10-maquinas-de-estado.md`, section 4): it has nothing.
    #[must_use]
    pub fn anywhere(&self, verb: Verb) -> bool {
        self.number.state != NumberState::NumberSwapped
            && self.grant.anywhere([self.in_lines], verb)
    }
}

/// What the tools of a Hermes are released by: the WhatsApp verbs it has anywhere.
#[must_use]
pub fn grants(accesses: &[HermesAccess]) -> Grants {
    let any = |verb| accesses.iter().any(|access| access.anywhere(verb));
    Grants {
        peer_any: false,
        peer_outbound: false,
        whatsapp_read: any(Verb::Read),
        whatsapp_send: any(Verb::Send),
        whatsapp_modify: any(Verb::Modify),
    }
}

#[derive(sqlx::FromRow)]
struct AccessRowDb {
    id: Uuid,
    hermes_id: Uuid,
    scope: String,
    can_read: bool,
    can_send: bool,
    can_modify: bool,
    can_wake: bool,
    can_start_chats: bool,
    number_id: Uuid,
    alias: String,
    state: String,
    session_name: String,
    lines_read: bool,
    lines_send: bool,
    lines_modify: bool,
    lines_wake: bool,
}

const SELECT_ACCESSES: &str = "\
    SELECT a.id, a.hermes_id, a.scope, a.can_read, a.can_send, a.can_modify, a.can_wake, \
           a.can_start_chats, n.id AS number_id, n.alias, n.state, n.session_name, \
           COALESCE(l.lines_read, FALSE) AS lines_read, \
           COALESCE(l.lines_send, FALSE) AS lines_send, \
           COALESCE(l.lines_modify, FALSE) AS lines_modify, \
           COALESCE(l.lines_wake, FALSE) AS lines_wake \
    FROM access a \
    JOIN number n ON n.organization_id = a.organization_id AND n.id = a.number_id \
    JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id \
    LEFT JOIN LATERAL ( \
        SELECT bool_or(c.can_read) AS lines_read, bool_or(c.can_send) AS lines_send, \
               bool_or(c.can_modify) AS lines_modify, bool_or(c.can_wake) AS lines_wake \
        FROM access_chat c WHERE c.organization_id = a.organization_id AND c.access_id = a.id \
    ) l ON TRUE \
    WHERE a.organization_id = $1 AND a.hermes_id = ANY($2) \
      AND n.removed_at IS NULL AND h.deleted_at IS NULL \
    ORDER BY n.alias";

fn grant_of(
    scope: &str,
    read: bool,
    send: bool,
    modify: bool,
    wake: bool,
    start: bool,
) -> Result<Grant, RepoError> {
    Ok(Grant {
        scope: Scope::parse(scope).ok_or(RepoError::Inconsistent("the scope of an access"))?,
        verbs: Verbs {
            read,
            send,
            modify,
            wake,
        },
        can_start_chats: start,
    })
}

impl TryFrom<AccessRowDb> for (Uuid, HermesAccess) {
    type Error = RepoError;

    fn try_from(row: AccessRowDb) -> Result<Self, RepoError> {
        let grant = grant_of(
            &row.scope,
            row.can_read,
            row.can_send,
            row.can_modify,
            row.can_wake,
            row.can_start_chats,
        )?;
        Ok((
            row.hermes_id,
            HermesAccess {
                id: row.id,
                grant,
                number: AccessedNumber {
                    id: row.number_id,
                    alias: NumberAlias::parse(&row.alias)
                        .map_err(|_| RepoError::Inconsistent("the alias of a number"))?,
                    state: NumberState::parse(&row.state)
                        .ok_or(RepoError::Inconsistent("the state of a number"))?,
                    session: SessionName::parse(&row.session_name)
                        .map_err(|_| RepoError::Inconsistent("the session name of a number"))?,
                },
                in_lines: Verbs {
                    read: row.lines_read,
                    send: row.lines_send,
                    modify: row.lines_modify,
                    wake: row.lines_wake,
                },
            },
        ))
    }
}

/// The accesses of the Hermes `hermes` to numbers that were not removed, by alias. What every
/// call reads, in one query by index.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn of_hermes(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
) -> Result<Vec<HermesAccess>, RepoError> {
    let rows: Vec<AccessRowDb> = sqlx::query_as(SELECT_ACCESSES)
        .bind(organization.id())
        .bind(&[hermes][..])
        .fetch_all(pool)
        .await?;
    rows.into_iter()
        .map(|row| <(Uuid, HermesAccess)>::try_from(row).map(|(_, access)| access))
        .collect()
}

/// The grants of each of these Hermes, in one query: what the sweep compares with the list of
/// tools each session was given. A Hermes without accesses has the empty grants.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn grants_of(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: &[Uuid],
) -> Result<HashMap<Uuid, Grants>, RepoError> {
    let mut by_hermes: HashMap<Uuid, Vec<HermesAccess>> =
        hermes.iter().map(|id| (*id, Vec::new())).collect();
    if hermes.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<AccessRowDb> = sqlx::query_as(SELECT_ACCESSES)
        .bind(organization.id())
        .bind(hermes)
        .fetch_all(pool)
        .await?;
    for row in rows {
        let (owner, access) = <(Uuid, HermesAccess)>::try_from(row)?;
        by_hermes.entry(owner).or_default().push(access);
    }
    Ok(by_hermes
        .into_iter()
        .map(|(id, accesses)| (id, grants(&accesses)))
        .collect())
}

fn verbs_of(read: bool, send: bool, modify: bool, wake: bool) -> Verbs {
    Verbs {
        read,
        send,
        modify,
        wake,
    }
}

/// The line of the chat `chat` under the access `access`, if it has one.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn chat_line(
    pool: &PgPool,
    organization: ResolvedOrg,
    access: Uuid,
    chat: Uuid,
) -> Result<Option<Verbs>, RepoError> {
    let row: Option<(bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT can_read, can_send, can_modify, can_wake FROM access_chat \
         WHERE organization_id = $1 AND access_id = $2 AND chat_id = $3",
    )
    .bind(organization.id())
    .bind(access)
    .bind(chat)
    .fetch_optional(pool)
    .await?;
    if let Some((read, send, modify, wake)) = row {
        return Ok(Some(verbs_of(read, send, modify, wake)));
    }
    // A chat under a hidden identifier, in an access that has lines: one of those lines may
    // be about this very person (given by telephone), and the hub cannot tell. It counts as
    // a chat with a line that blocks it, until the admin gives this chat a line of its own.
    let blocked: Option<bool> = sqlx::query_scalar(HIDDEN_AND_LINED)
        .bind(organization.id())
        .bind(access)
        .bind(chat)
        .fetch_optional(pool)
        .await?;
    Ok((blocked == Some(true)).then(|| verbs_of(false, false, false, false)))
}

/// Whether the chat `$3` is under a hidden identifier and the access `$2` has some line.
const HIDDEN_AND_LINED: &str = "\
    SELECT ch.hidden_id AND EXISTS (SELECT 1 FROM access_chat x \
                                    WHERE x.organization_id = $1 AND x.access_id = $2) \
    FROM chat ch WHERE ch.organization_id = $1 AND ch.id = $3";

/// Every line of the access `access`, by chat.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn chat_lines(
    pool: &PgPool,
    organization: ResolvedOrg,
    access: Uuid,
) -> Result<HashMap<Uuid, Verbs>, RepoError> {
    let rows: Vec<(Uuid, bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT chat_id, can_read, can_send, can_modify, can_wake FROM access_chat \
         WHERE organization_id = $1 AND access_id = $2",
    )
    .bind(organization.id())
    .bind(access)
    .fetch_all(pool)
    .await?;
    let mut lines: HashMap<Uuid, Verbs> = rows
        .into_iter()
        .map(|(chat, read, send, modify, wake)| (chat, verbs_of(read, send, modify, wake)))
        .collect();
    // As in [`chat_line`]: with any line, the chats under a hidden identifier are blocked.
    if !lines.is_empty() {
        let hidden: Vec<Uuid> = sqlx::query_scalar(
            "SELECT ch.id FROM chat ch \
             JOIN access a ON a.organization_id = ch.organization_id \
                          AND a.number_id = ch.number_id \
             WHERE ch.organization_id = $1 AND a.id = $2 AND ch.hidden_id",
        )
        .bind(organization.id())
        .bind(access)
        .fetch_all(pool)
        .await?;
        for chat in hidden {
            lines
                .entry(chat)
                .or_insert_with(|| verbs_of(false, false, false, false));
        }
    }
    Ok(lines)
}

/// The numbers the Hermes `name` has an access to, each with the words of its verbs: what
/// the admin is shown when a link between Hermes is created.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn reach_of(
    pool: &PgPool,
    organization: ResolvedOrg,
    name: &HermesName,
) -> Result<Vec<(String, Vec<&'static str>)>, RepoError> {
    let rows: Vec<(String, bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT n.alias, a.can_read, a.can_send, a.can_modify, a.can_wake \
         FROM access a \
         JOIN number n ON n.organization_id = a.organization_id AND n.id = a.number_id \
         JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id \
         WHERE a.organization_id = $1 AND h.name = $2 AND h.deleted_at IS NULL \
           AND n.removed_at IS NULL \
         ORDER BY n.alias",
    )
    .bind(organization.id())
    .bind(name.as_str())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(alias, read, send, modify, wake)| {
            (alias, verbs_of(read, send, modify, wake).words())
        })
        .collect())
}

// ---------------------------------------------------------------------------- the admin

/// An access as the admin reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The row of the access.
    pub id: Uuid,
    /// The Hermes.
    pub hermes: HermesName,
    /// The number.
    pub number: NumberAlias,
    /// The state of the number.
    pub number_state: NumberState,
    /// The scope and the verbs.
    pub grant: Grant,
    /// The lines of its chats, by when they were set.
    pub lines: Vec<(Uuid, Verbs)>,
    /// How long a notice of a chat waits, after its last message, before it wakes the Hermes.
    pub wake_wait_ms: i32,
    /// When it changed last.
    pub changed_at: i64,
}

/// The accesses of the organization, by Hermes and number; only those of `hermes` when given.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn list(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Option<&HermesName>,
) -> Result<Vec<Listed>, RepoError> {
    type Row = (
        Uuid,
        String,
        String,
        String,
        String,
        bool,
        bool,
        bool,
        bool,
        bool,
        i32,
        i64,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT a.id, h.name, n.alias, n.state, a.scope, a.can_read, a.can_send, a.can_modify, \
                a.can_wake, a.can_start_chats, a.wake_wait_ms, a.changed_at \
         FROM access a \
         JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id \
         JOIN number n ON n.organization_id = a.organization_id AND n.id = a.number_id \
         WHERE a.organization_id = $1 AND ($2::text IS NULL OR h.name = $2) \
           AND h.deleted_at IS NULL AND n.removed_at IS NULL \
         ORDER BY h.name, n.alias",
    )
    .bind(organization.id())
    .bind(hermes.map(HermesName::as_str))
    .fetch_all(pool)
    .await?;
    let mut listed = Vec::with_capacity(rows.len());
    for (
        id,
        name,
        alias,
        state,
        scope,
        read,
        send,
        modify,
        wake,
        start,
        wake_wait_ms,
        changed_at,
    ) in rows
    {
        let lines: Vec<(Uuid, bool, bool, bool, bool)> = sqlx::query_as(
            "SELECT chat_id, can_read, can_send, can_modify, can_wake FROM access_chat \
             WHERE organization_id = $1 AND access_id = $2 ORDER BY created_at, chat_id",
        )
        .bind(organization.id())
        .bind(id)
        .fetch_all(pool)
        .await?;
        listed.push(Listed {
            id,
            hermes: HermesName::parse(&name)
                .map_err(|_| RepoError::Inconsistent("a Hermes with an invalid name"))?,
            number: NumberAlias::parse(&alias)
                .map_err(|_| RepoError::Inconsistent("the alias of a number"))?,
            number_state: NumberState::parse(&state)
                .ok_or(RepoError::Inconsistent("the state of a number"))?,
            grant: grant_of(&scope, read, send, modify, wake, start)?,
            lines: lines
                .into_iter()
                .map(|(chat, read, send, modify, wake)| (chat, verbs_of(read, send, modify, wake)))
                .collect(),
            wake_wait_ms,
            changed_at,
        });
    }
    Ok(listed)
}

/// Why a change of the matrix found nothing to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// There is no Hermes with that name.
    Hermes,
    /// There is no number with that alias.
    Number,
    /// The Hermes has no access to that number.
    Access,
    /// The chat is not of that number.
    Chat,
    /// The access has no line for that chat.
    Line,
}

/// What a change of the matrix did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed<T> {
    /// It was written; the value says what was before.
    Done(T),
    /// It already was as asked. Nothing was written.
    Unchanged,
    /// What it names does not exist. Nothing was written.
    Missing(Missing),
}

/// The Hermes and the number a change names, locked: the Hermes first, as every write of a
/// Hermes does, then the number.
struct Target {
    hermes: Uuid,
    name: HermesName,
    number: Uuid,
    alias: NumberAlias,
}

async fn target(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    name: &HermesName,
    alias: &NumberAlias,
) -> Result<Result<Target, Missing>, RepoError> {
    let hermes: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM hermes WHERE organization_id = $1 AND name = $2 AND deleted_at IS NULL \
         FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(name.as_str())
    .fetch_optional(&mut *connection)
    .await?;
    let Some(hermes) = hermes else {
        return Ok(Err(Missing::Hermes));
    };
    let number: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM number WHERE organization_id = $1 AND alias = $2 AND removed_at IS NULL \
         FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(alias.as_str())
    .fetch_optional(&mut *connection)
    .await?;
    let Some(number) = number else {
        return Ok(Err(Missing::Number));
    };
    Ok(Ok(Target {
        hermes,
        name: name.clone(),
        number,
        alias: alias.clone(),
    }))
}

async fn locked_access(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    target: &Target,
) -> Result<Option<(Uuid, Grant)>, RepoError> {
    let row: Option<(Uuid, String, bool, bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT id, scope, can_read, can_send, can_modify, can_wake, can_start_chats \
         FROM access WHERE organization_id = $1 AND hermes_id = $2 AND number_id = $3 \
         FOR UPDATE",
    )
    .bind(organization.id())
    .bind(target.hermes)
    .bind(target.number)
    .fetch_optional(&mut *connection)
    .await?;
    row.map(|(id, scope, read, send, modify, wake, start)| {
        grant_of(&scope, read, send, modify, wake, start).map(|grant| (id, grant))
    })
    .transpose()
}

fn grant_details(details: Details, grant: &Grant, before: Option<&Grant>) -> Details {
    let details = details
        .word("scope", grant.scope.as_str())
        .words("verbs", &grant.verbs.words())
        .flag("start_chats", grant.can_start_chats);
    match before {
        Some(before) => details
            .word("scope_before", before.scope.as_str())
            .words("verbs_before", &before.verbs.words())
            .flag("start_chats_before", before.can_start_chats),
        None => details,
    }
}

fn about(target: &Target) -> Details {
    Details::new()
        .name("hermes", &target.name)
        .id("hermes_id", target.hermes)
        .alias("number", &target.alias)
        .id("number_id", target.number)
}

/// Gives the Hermes `name` this access to the number `alias`, whole: the scope, the four verbs
/// and "may start chats" become what is asked. The lines of chats stay. Returns what was
/// before, `None` for a new access. The caller checked the rules of the grant.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn grant(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    alias: &NumberAlias,
    grant: &Grant,
    now: i64,
) -> Result<Changed<Option<Grant>>, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let target = match target(write.connection(), organization, name, alias).await? {
        Ok(target) => target,
        Err(missing) => {
            write.rollback().await?;
            return Ok(Changed::Missing(missing));
        }
    };
    let before = locked_access(write.connection(), organization, &target).await?;
    let (id, action) = match &before {
        Some((_, old)) if old == grant => {
            write.rollback().await?;
            return Ok(Changed::Unchanged);
        }
        Some((id, _)) => {
            sqlx::query(
                "UPDATE access SET scope = $3, can_read = $4, can_send = $5, can_modify = $6, \
                        can_wake = $7, can_start_chats = $8, changed_at = $9 \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(id)
            .bind(grant.scope.as_str())
            .bind(grant.verbs.read)
            .bind(grant.verbs.send)
            .bind(grant.verbs.modify)
            .bind(grant.verbs.wake)
            .bind(grant.can_start_chats)
            .bind(now)
            .execute(write.connection())
            .await?;
            (*id, Action::AccessChanged)
        }
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO access (id, organization_id, hermes_id, number_id, scope, can_read, \
                                     can_send, can_modify, can_wake, can_start_chats, \
                                     created_by, created_at, changed_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)",
            )
            .bind(id)
            .bind(organization.id())
            .bind(target.hermes)
            .bind(target.number)
            .bind(grant.scope.as_str())
            .bind(grant.verbs.read)
            .bind(grant.verbs.send)
            .bind(grant.verbs.modify)
            .bind(grant.verbs.wake)
            .bind(grant.can_start_chats)
            .bind(admin.id)
            .bind(now)
            .execute(write.connection())
            .await?;
            (id, Action::AccessGranted)
        }
    };
    let old = before.map(|(_, old)| old);
    write
        .record(
            now,
            &Event::new(action, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Access, id)
                .with(grant_details(about(&target), grant, old.as_ref())),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", target.hermes).await?;
    write.commit().await?;
    Ok(Changed::Done(old))
}

/// Sets how long the notice of a chat waits, after its last message, before it wakes the
/// Hermes `name` on the number `alias`. Returns the wait before.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn set_wake_wait(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    alias: &NumberAlias,
    wait_ms: i32,
    now: i64,
) -> Result<Changed<i32>, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let target = match target(write.connection(), organization, name, alias).await? {
        Ok(target) => target,
        Err(missing) => {
            write.rollback().await?;
            return Ok(Changed::Missing(missing));
        }
    };
    let Some((id, _)) = locked_access(write.connection(), organization, &target).await? else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Access));
    };
    let before: i32 = sqlx::query_scalar(
        "SELECT wake_wait_ms FROM access WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(id)
    .fetch_one(write.connection())
    .await?;
    if before == wait_ms {
        write.rollback().await?;
        return Ok(Changed::Unchanged);
    }
    sqlx::query(
        "UPDATE access SET wake_wait_ms = $3, changed_at = $4 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(id)
    .bind(wait_ms)
    .bind(now)
    .execute(write.connection())
    .await?;
    write
        .record(
            now,
            &Event::new(Action::AccessChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Access, id)
                .with(
                    about(&target)
                        .number("wake_wait_ms", i64::from(wait_ms))
                        .number("wake_wait_ms_before", i64::from(before)),
                ),
        )
        .await?;
    write.commit().await?;
    Ok(Changed::Done(before))
}

/// Takes the access of the Hermes `name` to the number `alias` away, with its lines. Returns
/// how many lines went with it.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn remove(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    alias: &NumberAlias,
    now: i64,
) -> Result<Changed<u64>, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let target = match target(write.connection(), organization, name, alias).await? {
        Ok(target) => target,
        Err(missing) => {
            write.rollback().await?;
            return Ok(Changed::Missing(missing));
        }
    };
    let Some((id, grant)) = locked_access(write.connection(), organization, &target).await? else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Access));
    };
    let lines = end(&mut write, organization, id).await?;
    write
        .record(
            now,
            &Event::new(Action::AccessRemoved, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Access, id)
                .with(
                    grant_details(about(&target), &grant, None)
                        .number("lines", i64::try_from(lines).unwrap_or(i64::MAX))
                        .word("reason", "admin"),
                ),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", target.hermes).await?;
    write.commit().await?;
    Ok(Changed::Done(lines))
}

/// Deletes an access and its lines. Returns how many lines it had.
async fn end(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    access: Uuid,
) -> Result<u64, RepoError> {
    let lines =
        sqlx::query("DELETE FROM access_chat WHERE organization_id = $1 AND access_id = $2")
            .bind(organization.id())
            .bind(access)
            .execute(write.connection())
            .await?
            .rows_affected();
    sqlx::query("DELETE FROM access WHERE organization_id = $1 AND id = $2")
        .bind(organization.id())
        .bind(access)
        .execute(write.connection())
        .await?;
    Ok(lines)
}

/// Every access of the Hermes `hermes`, or to the number `number`, ends in the write of its
/// removal, each with its event. Returns the Hermes whose accesses ended.
pub(crate) async fn end_all(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    actor: &Actor,
    of: Of,
    now: i64,
) -> Result<Vec<Uuid>, RepoError> {
    let (hermes_id, number_id, reason) = match of {
        Of::Hermes(id) => (Some(id), None, "hermes_removed"),
        Of::Number(id) => (None, Some(id), "number_removed"),
    };
    let rows: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, hermes_id, number_id FROM access \
         WHERE organization_id = $1 AND (hermes_id = $2 OR number_id = $3) \
         ORDER BY id FOR UPDATE",
    )
    .bind(organization.id())
    .bind(hermes_id)
    .bind(number_id)
    .fetch_all(write.connection())
    .await?;
    let mut hermes = Vec::with_capacity(rows.len());
    for (access, owner, number) in rows {
        let lines = end(write, organization, access).await?;
        write
            .record(
                now,
                &Event::new(Action::AccessRemoved, actor.clone())
                    .in_organization(organization)
                    .about(TargetKind::Access, access)
                    .with(
                        Details::new()
                            .id("hermes_id", owner)
                            .id("number_id", number)
                            .number("lines", i64::try_from(lines).unwrap_or(i64::MAX))
                            .word("reason", reason),
                    ),
            )
            .await?;
        hermes.push(owner);
    }
    Ok(hermes)
}

/// Whose accesses [`end_all`] ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Of {
    /// Every access of this Hermes.
    Hermes(Uuid),
    /// Every access to this number.
    Number(Uuid),
}

/// A chat as the admin names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRef {
    /// Its identifier in the engine: the hub finds the chat, or creates it.
    Engine(EngineChatId),
    /// The identifier the hub gave it.
    Hub(Uuid),
}

async fn chat_of(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    chat: &ChatRef,
    now: i64,
) -> Result<Option<Uuid>, RepoError> {
    match chat {
        ChatRef::Engine(engine_chat) => Ok(Some(
            super::chat::resolve(connection, organization, key, number, engine_chat, now).await?,
        )),
        ChatRef::Hub(id) => Ok(sqlx::query_scalar(
            "SELECT id FROM chat WHERE organization_id = $1 AND number_id = $2 AND id = $3",
        )
        .bind(organization.id())
        .bind(number)
        .bind(id)
        .fetch_optional(connection)
        .await?),
    }
}

/// Sets the line of a chat under the access of `name` to `alias`: its verbs replace the
/// verbs of the number in that chat, whole. Returns the chat, and its verbs before.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn set_chat(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    key: &OrganizationKey,
    name: &HermesName,
    alias: &NumberAlias,
    chat: &ChatRef,
    verbs: Verbs,
    now: i64,
) -> Result<Changed<(Uuid, Option<Verbs>)>, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let target = match target(write.connection(), organization, name, alias).await? {
        Ok(target) => target,
        Err(missing) => {
            write.rollback().await?;
            return Ok(Changed::Missing(missing));
        }
    };
    let Some((access, _)) = locked_access(write.connection(), organization, &target).await? else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Access));
    };
    let Some(chat) = chat_of(
        write.connection(),
        organization,
        key,
        target.number,
        chat,
        now,
    )
    .await?
    else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Chat));
    };
    let before: Option<(bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT can_read, can_send, can_modify, can_wake FROM access_chat \
         WHERE organization_id = $1 AND access_id = $2 AND chat_id = $3 FOR UPDATE",
    )
    .bind(organization.id())
    .bind(access)
    .bind(chat)
    .fetch_optional(write.connection())
    .await?;
    let before = before.map(|(read, send, modify, wake)| verbs_of(read, send, modify, wake));
    if before == Some(verbs) {
        write.rollback().await?;
        return Ok(Changed::Unchanged);
    }
    sqlx::query(
        "INSERT INTO access_chat (organization_id, access_id, number_id, chat_id, can_read, \
                                  can_send, can_modify, can_wake, created_at, changed_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9) \
         ON CONFLICT (access_id, chat_id) DO UPDATE SET can_read = $5, can_send = $6, \
                can_modify = $7, can_wake = $8, changed_at = $9",
    )
    .bind(organization.id())
    .bind(access)
    .bind(target.number)
    .bind(chat)
    .bind(verbs.read)
    .bind(verbs.send)
    .bind(verbs.modify)
    .bind(verbs.wake)
    .bind(now)
    .execute(write.connection())
    .await?;
    let mut details = about(&target)
        .id("chat", chat)
        .words("verbs", &verbs.words());
    if let Some(before) = before {
        details = details.words("verbs_before", &before.words());
    }
    write
        .record(
            now,
            &Event::new(Action::AccessChatSet, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Access, access)
                .with(details),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", target.hermes).await?;
    write.commit().await?;
    Ok(Changed::Done((chat, before)))
}

/// Takes the line of a chat away: the verbs of the number hold there again (or, with the
/// scope "list", the chat leaves the scope). Returns its verbs before.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn remove_chat(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    key: &OrganizationKey,
    name: &HermesName,
    alias: &NumberAlias,
    chat: &ChatRef,
    now: i64,
) -> Result<Changed<(Uuid, Verbs)>, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let target = match target(write.connection(), organization, name, alias).await? {
        Ok(target) => target,
        Err(missing) => {
            write.rollback().await?;
            return Ok(Changed::Missing(missing));
        }
    };
    let Some((access, _)) = locked_access(write.connection(), organization, &target).await? else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Access));
    };
    // A chat named by the engine that the hub does not know has no line: nothing is created.
    let chat = match chat {
        ChatRef::Engine(engine_chat) => {
            super::chat::find_in(
                write.connection(),
                organization,
                key,
                target.number,
                engine_chat,
            )
            .await?
        }
        ChatRef::Hub(_) => {
            chat_of(
                write.connection(),
                organization,
                key,
                target.number,
                chat,
                now,
            )
            .await?
        }
    };
    let Some(chat) = chat else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Line));
    };
    let before: Option<(bool, bool, bool, bool)> = sqlx::query_as(
        "DELETE FROM access_chat WHERE organization_id = $1 AND access_id = $2 AND chat_id = $3 \
         RETURNING can_read, can_send, can_modify, can_wake",
    )
    .bind(organization.id())
    .bind(access)
    .bind(chat)
    .fetch_optional(write.connection())
    .await?;
    let Some((read, send, modify, wake)) = before else {
        write.rollback().await?;
        return Ok(Changed::Missing(Missing::Line));
    };
    let before = verbs_of(read, send, modify, wake);
    write
        .record(
            now,
            &Event::new(Action::AccessChatRemoved, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Access, access)
                .with(
                    about(&target)
                        .id("chat", chat)
                        .words("verbs_before", &before.words()),
                ),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", target.hermes).await?;
    write.commit().await?;
    Ok(Changed::Done((chat, before)))
}

/// The other Hermes that may send, or be woken, on the number `number`: what the admin is
/// told when granting (`docs/08-permissoes.md`, section 5: warn, never prevent).
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn others_sending_or_woken(
    pool: &PgPool,
    organization: ResolvedOrg,
    number: &NumberAlias,
    besides: &HermesName,
) -> Result<Vec<HermesName>, RepoError> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT h.name FROM access a \
         JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id \
         JOIN number n ON n.organization_id = a.organization_id AND n.id = a.number_id \
         WHERE a.organization_id = $1 AND n.alias = $2 AND n.removed_at IS NULL \
           AND h.deleted_at IS NULL AND h.name <> $3 \
           AND (a.can_send OR a.can_wake OR EXISTS ( \
                SELECT 1 FROM access_chat c WHERE c.organization_id = a.organization_id \
                  AND c.access_id = a.id AND (c.can_send OR c.can_wake))) \
         ORDER BY h.name",
    )
    .bind(organization.id())
    .bind(number.as_str())
    .bind(besides.as_str())
    .fetch_all(pool)
    .await?;
    names
        .iter()
        .map(|name| {
            HermesName::parse(name)
                .map_err(|_| RepoError::Inconsistent("a Hermes with an invalid name"))
        })
        .collect()
}
