//! The conversation between Hermes (`docs/13-conversa-entre-hermes.md`): the list of peers,
//! the threads, the messages and their ceilings, and the queue of notices of new messages.
//!
//! | Who | What |
//! |---|---|
//! | The admin command | [`link_set`], [`link_remove`], [`links`]: the only writers of the list of peers. Each change is a security event, and the hub is told. |
//! | A Hermes, by its tools | [`peers_of`], [`send`], [`threads`], [`read`]. Every one decides against the links of now. |
//! | The session of the channel | [`lease`] and [`acknowledge`]: a notice goes out with a lease, and comes back when the connector does not confirm it. |
//!
//! **One write for a message.** [`send`] locks the rows of the two Hermes, in the order of
//! their identifiers, before it reads the link and counts the ceilings: two sendings at the
//! same time never pass together, and a link the admin just took away is seen. The ceilings
//! are sliding windows counted in the database, with the time of the hub; only an accepted
//! message counts. They are counted per link and per recipient, never per thread.
//!
//! **A notice is a pointer.** It carries who sent and how many, never the text or the thread.
//! One pending notice per sender: what comes next adds to its count. It is judged again when
//! it leaves: without the link, without its wake mark or with a recipient that may no longer
//! be woken, it is discarded.

use arauto_protocol::errors::RetryAfter;
use arauto_protocol::limits::{
    DAY_MS, HOUR_MS, MINUTE_MS, NOTICE_COUNT_MAX, PEER_BYTES_PER_SENDER_PER_DAY,
    PEER_MESSAGES_PER_LINK_PER_HOUR, PEER_MESSAGES_PER_SENDER_PER_DAY,
    PEER_MESSAGES_PER_SENDER_PER_MINUTE, PEER_NEW_THREADS_PER_LINK_PER_HOUR, PEER_NOTICE_GAP_MS,
    PEER_NOTICES_PER_LINK_PER_HOUR, PEER_NOTICES_PER_RECIPIENT_PER_HOUR, PEER_READ_BUDGET_BYTES,
    PEER_UNREAD_PER_LINK_MAX, SECOND_MS, THROTTLED_RETRY_MS,
};
use arauto_protocol::names::{HermesName, MessageText, NumberAlias};
use arauto_protocol::notice::AckOutcome;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Admin, RepoError, notify_hub};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::db::ResolvedOrg;
use crate::mark::Mark;
use crate::vault::{OrganizationKey, PeerMessageIds};

/// How long a notice may stay in the queue.
pub const NOTICE_LIFE_MS: i64 = DAY_MS;

/// The most open notices of one Hermes: past it, the oldest expires.
pub const NOTICES_PER_HERMES_MAX: i64 = 1_000;

/// How long the hub waits for the `notice_ack` before it sends the notice again.
pub const NOTICE_LEASE_MS: i64 = MINUTE_MS;

/// The days a message is kept when `retention` has no row for it.
const MESSAGE_DAYS_DEFAULT: i64 = 30;

fn name_of(text: &str) -> Result<HermesName, RepoError> {
    HermesName::parse(text).map_err(|_| RepoError::Inconsistent("the name of a Hermes"))
}

// ---------------------------------------------------------------------------- the admin

/// One link, as the admin reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkLine {
    /// Who may send.
    pub from: HermesName,
    /// Who receives.
    pub to: HermesName,
    /// Whether a message of `from` wakes `to`.
    pub wake: bool,
    /// When it last changed, in milliseconds of the hub.
    pub changed_at: i64,
}

/// Which end of a link names no Hermes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkMissing {
    /// The sender.
    From,
    /// The recipient.
    To,
}

/// What [`link_set`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSet {
    /// The link is new.
    Created,
    /// The link was there with the other wake mark.
    Changed,
    /// It already was as asked: nothing was written.
    Unchanged,
    /// One end names no Hermes.
    Missing(LinkMissing),
    /// The two ends are the same Hermes.
    SameHermes,
}

/// What [`link_remove`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRemoved {
    /// The link is gone. The notices it had pending went with it.
    Removed,
    /// There was no such link.
    NoLink,
    /// One end names no Hermes.
    Missing(LinkMissing),
}

/// The two Hermes of a link, locked in the order of their identifiers.
async fn ends(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    from: &HermesName,
    to: &HermesName,
) -> Result<Result<(Uuid, Uuid), LinkMissing>, RepoError> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM hermes \
         WHERE organization_id = $1 AND name = ANY($2) AND deleted_at IS NULL \
         ORDER BY id FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(vec![from.as_str().to_owned(), to.as_str().to_owned()])
    .fetch_all(connection)
    .await?;
    let id_of = |name: &HermesName| {
        rows.iter()
            .find(|(_, stored)| stored == name.as_str())
            .map(|(id, _)| *id)
    };
    let Some(from_id) = id_of(from) else {
        return Ok(Err(LinkMissing::From));
    };
    let Some(to_id) = id_of(to) else {
        return Ok(Err(LinkMissing::To));
    };
    Ok(Ok((from_id, to_id)))
}

fn link_details(from: &HermesName, from_id: Uuid, to: &HermesName, to_id: Uuid) -> Details {
    Details::new()
        .name("from", from)
        .id("from_id", from_id)
        .name("to", to)
        .id("to_id", to_id)
}

/// Creates the link from `from` to `to`, or sets its wake mark.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn link_set(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    from: &HermesName,
    to: &HermesName,
    wake: bool,
    now: i64,
) -> Result<LinkSet, RepoError> {
    if from == to {
        return Ok(LinkSet::SameHermes);
    }
    let mut write = audit::begin_security(pool, mark).await?;
    let (from_id, to_id) = match ends(write.connection(), organization, from, to).await? {
        Ok(ids) => ids,
        Err(missing) => {
            write.rollback().await?;
            return Ok(LinkSet::Missing(missing));
        }
    };
    let before: Option<bool> = sqlx::query_scalar(
        "SELECT wake FROM peer_link \
         WHERE organization_id = $1 AND from_hermes_id = $2 AND to_hermes_id = $3 FOR UPDATE",
    )
    .bind(organization.id())
    .bind(from_id)
    .bind(to_id)
    .fetch_optional(write.connection())
    .await?;
    let details = link_details(from, from_id, to, to_id).flag("wake", wake);
    let (action, details, outcome) = match before {
        Some(old) if old == wake => {
            write.rollback().await?;
            return Ok(LinkSet::Unchanged);
        }
        Some(old) => {
            sqlx::query(
                "UPDATE peer_link SET wake = $4, changed_at = $5 \
                 WHERE organization_id = $1 AND from_hermes_id = $2 AND to_hermes_id = $3",
            )
            .bind(organization.id())
            .bind(from_id)
            .bind(to_id)
            .bind(wake)
            .bind(now)
            .execute(write.connection())
            .await?;
            (
                Action::PeerLinkChanged,
                details.flag("wake_before", old),
                LinkSet::Changed,
            )
        }
        None => {
            sqlx::query(
                "INSERT INTO peer_link (organization_id, from_hermes_id, to_hermes_id, wake, \
                                        created_by, created_at, changed_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $6)",
            )
            .bind(organization.id())
            .bind(from_id)
            .bind(to_id)
            .bind(wake)
            .bind(admin.id)
            .bind(now)
            .execute(write.connection())
            .await?;
            (Action::PeerLinkCreated, details, LinkSet::Created)
        }
    };
    if !wake {
        discard_notices(write.connection(), organization, from_id, to_id, now).await?;
    }
    write
        .record(
            now,
            &Event::new(action, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, from_id)
                .with(details),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", from_id).await?;
    notify_hub(write.connection(), organization, "hermes", to_id).await?;
    write.commit().await?;
    Ok(outcome)
}

/// Takes the link from `from` to `to` away. What `from` had sent and `to` had not read is no
/// longer delivered ([`read`]), and the notices of it are discarded.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn link_remove(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    from: &HermesName,
    to: &HermesName,
    now: i64,
) -> Result<LinkRemoved, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let (from_id, to_id) = match ends(write.connection(), organization, from, to).await? {
        Ok(ids) => ids,
        Err(missing) => {
            write.rollback().await?;
            return Ok(LinkRemoved::Missing(missing));
        }
    };
    let wake: Option<bool> = sqlx::query_scalar(
        "DELETE FROM peer_link \
         WHERE organization_id = $1 AND from_hermes_id = $2 AND to_hermes_id = $3 \
         RETURNING wake",
    )
    .bind(organization.id())
    .bind(from_id)
    .bind(to_id)
    .fetch_optional(write.connection())
    .await?;
    let Some(wake) = wake else {
        write.rollback().await?;
        return Ok(LinkRemoved::NoLink);
    };
    discard_notices(write.connection(), organization, from_id, to_id, now).await?;
    write
        .record(
            now,
            &Event::new(Action::PeerLinkRemoved, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, from_id)
                .with(link_details(from, from_id, to, to_id).flag("wake", wake)),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", from_id).await?;
    notify_hub(write.connection(), organization, "hermes", to_id).await?;
    write.commit().await?;
    Ok(LinkRemoved::Removed)
}

/// The notices of the messages of `from` to `to` that did not reach their end are discarded.
async fn discard_notices(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    from: Uuid,
    to: Uuid,
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE notice SET state = 'discarded', resolved_at = $4, leased_until = NULL \
         WHERE organization_id = $1 AND kind = 'peer' AND hermes_id = $3 \
           AND from_hermes_id = $2 AND state IN ('pending', 'sent')",
    )
    .bind(organization.id())
    .bind(from)
    .bind(to)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// Every link of the organization, by sender and recipient.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn links(pool: &PgPool, organization: ResolvedOrg) -> Result<Vec<LinkLine>, RepoError> {
    let rows: Vec<(String, String, bool, i64)> = sqlx::query_as(
        "SELECT f.name, t.name, l.wake, l.changed_at \
         FROM peer_link l \
         JOIN hermes f ON f.id = l.from_hermes_id \
         JOIN hermes t ON t.id = l.to_hermes_id \
         WHERE l.organization_id = $1 AND f.deleted_at IS NULL AND t.deleted_at IS NULL \
         ORDER BY f.name, t.name",
    )
    .bind(organization.id())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(from, to, wake, changed_at)| {
            Ok(LinkLine {
                from: name_of(&from)?,
                to: name_of(&to)?,
                wake,
                changed_at,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------- the peers

/// One peer of a Hermes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// Its name.
    pub name: HermesName,
    /// There is a link from the Hermes to it.
    pub can_send: bool,
    /// There is a link from it to the Hermes.
    pub can_receive: bool,
}

/// The peers of `hermes`: every other Hermes it has a link with, in either direction.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn peers_of(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
) -> Result<Vec<Peer>, RepoError> {
    let rows: Vec<(String, bool, bool)> = sqlx::query_as(
        "SELECT h.name, \
                EXISTS (SELECT 1 FROM peer_link l \
                        WHERE l.from_hermes_id = $2 AND l.to_hermes_id = h.id), \
                EXISTS (SELECT 1 FROM peer_link l \
                        WHERE l.from_hermes_id = h.id AND l.to_hermes_id = $2) \
         FROM hermes h \
         WHERE h.organization_id = $1 AND h.deleted_at IS NULL AND h.id <> $2 \
           AND EXISTS (SELECT 1 FROM peer_link l \
                       WHERE l.organization_id = $1 \
                         AND ((l.from_hermes_id = $2 AND l.to_hermes_id = h.id) \
                           OR (l.from_hermes_id = h.id AND l.to_hermes_id = $2))) \
         ORDER BY h.name",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(name, can_send, can_receive)| {
            Ok(Peer {
                name: name_of(&name)?,
                can_send,
                can_receive,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------- sending

/// A message to send.
#[derive(Debug, Clone, Copy)]
pub struct NewMessage<'a> {
    /// The Hermes that sends.
    pub from: Uuid,
    /// The peer, by name.
    pub to: Option<&'a HermesName>,
    /// The thread to continue.
    pub thread: Option<Uuid>,
    /// The text.
    pub text: &'a MessageText,
}

/// The ceiling a message stopped at (`docs/13-conversa-entre-hermes.md`, section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ceiling {
    /// Messages of the sender in a minute.
    SenderMinute,
    /// Messages or bytes of the sender in a day.
    SenderDay,
    /// Messages of the link in an hour.
    LinkHour,
    /// New threads of the link in an hour.
    NewThreads,
    /// Unread messages of the link: the box of the recipient is full.
    Unread,
}

impl Ceiling {
    /// The word of the ceiling, for the audit.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Ceiling::SenderMinute => "sender_minute",
            Ceiling::SenderDay => "sender_day",
            Ceiling::LinkHour => "link_hour",
            Ceiling::NewThreads => "new_threads",
            Ceiling::Unread => "unread",
        }
    }
}

/// What [`send`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    /// The message is kept.
    Done {
        /// Its thread.
        thread: Uuid,
        /// The message.
        message: Uuid,
        /// The recipient.
        to: Uuid,
        /// A notice was created or grew: the session of the recipient has something to send.
        noticed: bool,
    },
    /// No peer of that name the sender has a link to: the same answer as a name that does not
    /// exist (AR-707).
    NoPeer,
    /// The thread does not exist, belongs to others, or its other participant is not the
    /// `to` given (AR-708).
    NoThread,
    /// A ceiling was reached (AR-706). Nothing was written.
    Ceiling {
        /// Which one.
        which: Ceiling,
        /// In how long it frees, when time frees it.
        retry_after: Option<RetryAfter>,
        /// The recipient.
        to: Uuid,
    },
}

fn seconds_until(frees_at: i64, now: i64) -> Option<RetryAfter> {
    let wait_ms = frees_at.saturating_sub(now).max(1);
    let seconds = wait_ms.saturating_add(SECOND_MS - 1) / SECOND_MS;
    let seconds = u32::try_from(seconds.max(1)).unwrap_or(u32::MAX);
    Some(RetryAfter::new(seconds).unwrap_or(RetryAfter::MAX))
}

/// The messages `from` sent since `since` (to `to`, when given): how many, their bytes and
/// the time of the oldest.
async fn sent_since(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    from: Uuid,
    to: Option<Uuid>,
    since: i64,
) -> Result<(i64, i64, Option<i64>), RepoError> {
    Ok(sqlx::query_as(
        "SELECT count(*), COALESCE(sum(size), 0)::BIGINT, min(sent_at) FROM peer_message \
         WHERE organization_id = $1 AND from_hermes_id = $2 \
           AND ($3::UUID IS NULL OR to_hermes_id = $3) AND sent_at > $4",
    )
    .bind(organization.id())
    .bind(from)
    .bind(to)
    .bind(since)
    .fetch_one(connection)
    .await?)
}

/// The ceiling this message stops at, if any.
async fn ceiling(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    from: Uuid,
    to: Uuid,
    new_thread: bool,
    size: i64,
    now: i64,
) -> Result<Option<(Ceiling, Option<RetryAfter>)>, RepoError> {
    let (count, _, oldest) =
        sent_since(connection, organization, from, None, now - MINUTE_MS).await?;
    if count >= i64::from(PEER_MESSAGES_PER_SENDER_PER_MINUTE) {
        let frees = oldest.unwrap_or(now).saturating_add(MINUTE_MS);
        return Ok(Some((Ceiling::SenderMinute, seconds_until(frees, now))));
    }
    let (count, bytes, oldest) =
        sent_since(connection, organization, from, None, now - DAY_MS).await?;
    let day_bytes = i64::try_from(PEER_BYTES_PER_SENDER_PER_DAY).unwrap_or(i64::MAX);
    if count >= i64::from(PEER_MESSAGES_PER_SENDER_PER_DAY)
        || bytes.saturating_add(size) > day_bytes
    {
        let frees = oldest.unwrap_or(now).saturating_add(DAY_MS);
        return Ok(Some((Ceiling::SenderDay, seconds_until(frees, now))));
    }
    let (count, _, oldest) =
        sent_since(connection, organization, from, Some(to), now - HOUR_MS).await?;
    if count >= i64::from(PEER_MESSAGES_PER_LINK_PER_HOUR) {
        let frees = oldest.unwrap_or(now).saturating_add(HOUR_MS);
        return Ok(Some((Ceiling::LinkHour, seconds_until(frees, now))));
    }
    if new_thread {
        // A thread is of the link of whoever sent its first message.
        let (count, oldest): (i64, Option<i64>) = sqlx::query_as(
            "SELECT count(*), min(t.created_at) FROM peer_thread t \
             WHERE t.organization_id = $1 AND t.hermes_a = LEAST($2, $3) \
               AND t.hermes_b = GREATEST($2, $3) AND t.created_at > $4 \
               AND EXISTS (SELECT 1 FROM peer_message m \
                           WHERE m.thread_id = t.id AND m.from_hermes_id = $2 \
                             AND m.sent_at = t.created_at)",
        )
        .bind(organization.id())
        .bind(from)
        .bind(to)
        .bind(now - HOUR_MS)
        .fetch_one(&mut *connection)
        .await?;
        if count >= i64::from(PEER_NEW_THREADS_PER_LINK_PER_HOUR) {
            let frees = oldest.unwrap_or(now).saturating_add(HOUR_MS);
            return Ok(Some((Ceiling::NewThreads, seconds_until(frees, now))));
        }
    }
    let unread: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM peer_message \
         WHERE organization_id = $1 AND from_hermes_id = $2 AND to_hermes_id = $3 \
           AND read_at IS NULL",
    )
    .bind(organization.id())
    .bind(from)
    .bind(to)
    .fetch_one(connection)
    .await?;
    if unread >= i64::from(PEER_UNREAD_PER_LINK_MAX) {
        // Only the recipient frees it, by reading.
        return Ok(Some((Ceiling::Unread, None)));
    }
    Ok(None)
}

/// What the notice of a new message does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoticePlan {
    /// No notice: the message is only kept.
    None,
    /// The pending notice of this sender grows by one.
    Grow(Uuid),
    /// A new notice.
    New,
}

/// Decides the notice of a message of `from` to `to`, whose link has the wake mark.
async fn plan_notice(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    from: Uuid,
    to: Uuid,
    now: i64,
) -> Result<NoticePlan, RepoError> {
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM device \
                        WHERE organization_id = $1 AND hermes_id = $2 AND state = 'active')",
    )
    .bind(organization.id())
    .bind(to)
    .fetch_one(&mut *connection)
    .await?;
    if !active {
        return Ok(NoticePlan::None);
    }
    let pending: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM notice \
         WHERE organization_id = $1 AND kind = 'peer' AND hermes_id = $2 \
           AND from_hermes_id = $3 AND state = 'pending' FOR UPDATE",
    )
    .bind(organization.id())
    .bind(to)
    .bind(from)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(id) = pending {
        return Ok(NoticePlan::Grow(id));
    }
    // A notice the ceiling stops is dropped, never put off: put off, the loop would start
    // again by itself an hour later.
    let (of_link, of_recipient): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE from_hermes_id = $3), count(*) FROM peer_message \
         WHERE organization_id = $1 AND to_hermes_id = $2 AND woke AND sent_at > $4",
    )
    .bind(organization.id())
    .bind(to)
    .bind(from)
    .bind(now - HOUR_MS)
    .fetch_one(connection)
    .await?;
    if of_link >= i64::from(PEER_NOTICES_PER_LINK_PER_HOUR)
        || of_recipient >= i64::from(PEER_NOTICES_PER_RECIPIENT_PER_HOUR)
    {
        return Ok(NoticePlan::None);
    }
    Ok(NoticePlan::New)
}

async fn write_notice(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    from: Uuid,
    to: Uuid,
    plan: NoticePlan,
    now: i64,
) -> Result<(), RepoError> {
    match plan {
        NoticePlan::None => {}
        NoticePlan::Grow(id) => {
            sqlx::query("UPDATE notice SET count = LEAST(count + 1, $2) WHERE id = $1")
                .bind(id)
                .bind(i32::from(NOTICE_COUNT_MAX))
                .execute(connection)
                .await?;
        }
        NoticePlan::New => {
            // The queue of a Hermes holds 1,000: past it, the oldest expires.
            sqlx::query(
                "UPDATE notice SET state = 'expired', resolved_at = $3, leased_until = NULL \
                 WHERE id IN (SELECT id FROM notice \
                              WHERE organization_id = $1 AND hermes_id = $2 \
                                AND state IN ('pending', 'sent') \
                              ORDER BY created_at DESC, id OFFSET $4)",
            )
            .bind(organization.id())
            .bind(to)
            .bind(now)
            .bind(NOTICES_PER_HERMES_MAX - 1)
            .execute(&mut *connection)
            .await?;
            sqlx::query(
                "INSERT INTO notice (id, organization_id, hermes_id, kind, from_hermes_id, \
                                     count, state, not_before, created_at, expires_at) \
                 VALUES ($1, $2, $3, 'peer', $4, 1, 'pending', $5, $5, $6)",
            )
            .bind(Uuid::new_v4())
            .bind(organization.id())
            .bind(to)
            .bind(from)
            .bind(now)
            .bind(now.saturating_add(NOTICE_LIFE_MS))
            .execute(connection)
            .await?;
        }
    }
    Ok(())
}

/// Keeps a message of one Hermes for another, and its notice when one fits.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Vault`] when the text
/// cannot be encrypted.
pub async fn send(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    new: NewMessage<'_>,
    now: i64,
) -> Result<Sent, RepoError> {
    let mut tx = pool.begin().await?;
    // Who the other one is: by the thread, or by the name. A name with no link from the
    // sender is the same as a name that does not exist, by one query.
    let other: Uuid = if let Some(thread) = new.thread {
        let pair: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT hermes_a, hermes_b FROM peer_thread WHERE organization_id = $1 AND id = $2",
        )
        .bind(organization.id())
        .bind(thread)
        .fetch_optional(&mut *tx)
        .await?;
        match pair {
            Some((a, b)) if a == new.from => b,
            Some((a, b)) if b == new.from => a,
            _ => return Ok(Sent::NoThread),
        }
    } else if let Some(to) = new.to {
        let found: Option<Uuid> = sqlx::query_scalar(
            "SELECT h.id FROM hermes h \
             JOIN peer_link l ON l.to_hermes_id = h.id AND l.from_hermes_id = $2 \
             WHERE h.organization_id = $1 AND h.name = $3 AND h.deleted_at IS NULL",
        )
        .bind(organization.id())
        .bind(new.from)
        .bind(to.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        match found {
            Some(id) => id,
            None => return Ok(Sent::NoPeer),
        }
    } else {
        return Ok(Sent::NoPeer);
    };
    // The two Hermes, locked in order: nothing else writes about this pair meanwhile.
    let locked: Vec<(Uuid, String, bool, Option<i64>)> = sqlx::query_as(
        "SELECT id, name, can_be_woken, deleted_at FROM hermes \
         WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(vec![new.from, other])
    .fetch_all(&mut *tx)
    .await?;
    let other_row = locked.iter().find(|row| row.0 == other);
    let sender_there = locked
        .iter()
        .any(|row| row.0 == new.from && row.3.is_none());
    let missing = if new.thread.is_some() {
        Sent::NoThread
    } else {
        Sent::NoPeer
    };
    let Some((_, other_name, other_can_be_woken, None)) = other_row else {
        return Ok(missing);
    };
    if !sender_there {
        return Ok(missing);
    }
    if let (Some(_), Some(to)) = (new.thread, new.to)
        && to.as_str() != other_name
    {
        return Ok(Sent::NoThread);
    }
    let wake: Option<bool> = sqlx::query_scalar(
        "SELECT wake FROM peer_link \
         WHERE organization_id = $1 AND from_hermes_id = $2 AND to_hermes_id = $3",
    )
    .bind(organization.id())
    .bind(new.from)
    .bind(other)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(wake) = wake else {
        return Ok(Sent::NoPeer);
    };
    let size = i64::try_from(new.text.as_str().len()).unwrap_or(i64::MAX);
    if let Some((which, retry_after)) = ceiling(
        &mut tx,
        organization,
        new.from,
        other,
        new.thread.is_none(),
        size,
        now,
    )
    .await?
    {
        return Ok(Sent::Ceiling {
            which,
            retry_after,
            to: other,
        });
    }
    let plan = if wake && *other_can_be_woken {
        plan_notice(&mut tx, organization, new.from, other, now).await?
    } else {
        NoticePlan::None
    };
    // The messages of a thread are read in the order they were kept. Two kept in the same
    // millisecond would have no order: the second one is given the next millisecond.
    let mut sent_at = now;
    let thread = match new.thread {
        Some(thread) => {
            sent_at = sqlx::query_scalar(
                "UPDATE peer_thread SET last_message_at = GREATEST(last_message_at + 1, $3) \
                 WHERE organization_id = $1 AND id = $2 RETURNING last_message_at",
            )
            .bind(organization.id())
            .bind(thread)
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;
            thread
        }
        None => {
            let thread = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO peer_thread (id, organization_id, hermes_a, hermes_b, created_at, \
                                          last_message_at) \
                 VALUES ($1, $2, LEAST($3, $4), GREATEST($3, $4), $5, $5)",
            )
            .bind(thread)
            .bind(organization.id())
            .bind(new.from)
            .bind(other)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            thread
        }
    };
    let days: Option<i32> =
        sqlx::query_scalar("SELECT days FROM retention WHERE table_name = 'peer_message'")
            .fetch_optional(&mut *tx)
            .await?;
    let days = days.map_or(MESSAGE_DAYS_DEFAULT, i64::from);
    let message = Uuid::new_v4();
    let sealed = key.seal_peer_text(
        &PeerMessageIds {
            thread,
            message,
            from: new.from,
            to: other,
        },
        new.text,
    )?;
    sqlx::query(
        "INSERT INTO peer_message (id, organization_id, thread_id, from_hermes_id, \
                                   to_hermes_id, sent_at, size, nonce, text_enc, woke, purge_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(message)
    .bind(organization.id())
    .bind(thread)
    .bind(new.from)
    .bind(other)
    .bind(sent_at)
    .bind(i32::try_from(size).unwrap_or(i32::MAX))
    .bind(sealed.nonce.as_slice())
    .bind(sealed.text_enc.as_slice())
    .bind(plan == NoticePlan::New)
    .bind(now.saturating_add(days.saturating_mul(DAY_MS)))
    .execute(&mut *tx)
    .await?;
    write_notice(&mut tx, organization, new.from, other, plan, now).await?;
    tx.commit().await?;
    Ok(Sent::Done {
        thread,
        message,
        to: other,
        noticed: plan != NoticePlan::None,
    })
}

// ---------------------------------------------------------------------------- reading

/// One thread, as its participant lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadLine {
    /// The thread.
    pub id: Uuid,
    /// The other participant.
    pub peer: HermesName,
    /// When its last message was sent.
    pub last_message_at: i64,
    /// The messages for the reader it has not read and may read now.
    pub unread: i64,
}

/// A page of threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Threads {
    /// The threads, the most recent first.
    pub threads: Vec<ThreadLine>,
    /// There are older ones.
    pub more: bool,
}

/// The threads of `hermes` whose two participants still have a link, in either direction.
/// `None` when `before` names no thread the Hermes may see.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn threads(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
    limit: u32,
    before: Option<Uuid>,
) -> Result<Option<Threads>, RepoError> {
    const VISIBLE: &str = "SELECT t.id, o.name, t.last_message_at, \
                (SELECT count(*) FROM peer_message m \
                 WHERE m.thread_id = t.id AND m.to_hermes_id = $2 AND m.read_at IS NULL \
                   AND EXISTS (SELECT 1 FROM peer_link l \
                               WHERE l.from_hermes_id = o.id AND l.to_hermes_id = $2)) \
         FROM peer_thread t \
         JOIN hermes o ON o.id = CASE WHEN t.hermes_a = $2 THEN t.hermes_b ELSE t.hermes_a END \
         WHERE t.organization_id = $1 AND (t.hermes_a = $2 OR t.hermes_b = $2) \
           AND o.deleted_at IS NULL \
           AND EXISTS (SELECT 1 FROM peer_link l \
                       WHERE (l.from_hermes_id = $2 AND l.to_hermes_id = o.id) \
                          OR (l.from_hermes_id = o.id AND l.to_hermes_id = $2))";
    let cursor: Option<(i64, Uuid)> = match before {
        None => None,
        Some(before) => {
            let row: Option<(Uuid, String, i64, i64)> =
                sqlx::query_as(sqlx::AssertSqlSafe(format!("{VISIBLE} AND t.id = $3")))
                    .bind(organization.id())
                    .bind(hermes)
                    .bind(before)
                    .fetch_optional(pool)
                    .await?;
            match row {
                Some((id, _, at, _)) => Some((at, id)),
                None => return Ok(None),
            }
        }
    };
    let take = i64::from(limit).saturating_add(1);
    let rows: Vec<(Uuid, String, i64, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{VISIBLE} AND ($3::BIGINT IS NULL OR (t.last_message_at, t.id) < ($3, $4)) \
         ORDER BY t.last_message_at DESC, t.id DESC LIMIT $5"
    )))
    .bind(organization.id())
    .bind(hermes)
    .bind(cursor.map(|(at, _)| at))
    .bind(cursor.map(|(_, id)| id))
    .bind(take)
    .fetch_all(pool)
    .await?;
    let more = rows.len() > limit as usize;
    let threads = rows
        .into_iter()
        .take(limit as usize)
        .map(|(id, peer, last_message_at, unread)| {
            Ok(ThreadLine {
                id,
                peer: name_of(&peer)?,
                last_message_at,
                unread,
            })
        })
        .collect::<Result<Vec<_>, RepoError>>()?;
    Ok(Some(Threads { threads, more }))
}

/// What to read of a thread.
#[derive(Debug, Clone, Copy)]
pub struct ReadRequest {
    /// The reader.
    pub hermes: Uuid,
    /// The thread.
    pub thread: Uuid,
    /// How many messages at most.
    pub limit: u32,
    /// Only the messages before this one.
    pub before: Option<Uuid>,
    /// Only the messages after this one.
    pub after: Option<Uuid>,
}

/// One message of a thread, open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadMessage {
    /// The message.
    pub id: Uuid,
    /// The reader wrote it.
    pub mine: bool,
    /// When it was sent.
    pub sent_at: i64,
    /// Its text.
    pub text: MessageText,
}

/// What [`read`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    /// The messages, the oldest first. The ones for the reader are now read.
    Messages {
        /// The other participant.
        peer: HermesName,
        /// The messages.
        messages: Vec<ReadMessage>,
        /// There are more in the direction that was read.
        more: bool,
    },
    /// The thread does not exist, belongs to others, or its two Hermes have no link any more
    /// (AR-708).
    NoThread,
    /// `before` or `after` names no message of the thread (AR-711).
    NoCursor,
}

type MessageRow = (Uuid, Uuid, Uuid, i64, Vec<u8>, Vec<u8>, Option<i64>);

const MESSAGE_COLUMNS: &str =
    "m.id, m.from_hermes_id, m.to_hermes_id, m.sent_at, m.nonce, m.text_enc, m.read_at";

/// What the reader may see of the thread `$2`: what it wrote, what it already read, and the
/// unread ones only while the link of the sender to it exists (`$4`).
const DELIVERABLE: &str = "m.organization_id = $1 AND m.thread_id = $2 \
     AND (m.from_hermes_id = $3 OR m.read_at IS NOT NULL OR $4)";

/// Reads messages of a thread and marks as read the ones returned that were for the reader.
///
/// Without `before` and `after` it starts at the oldest unread message; with none unread, it
/// gives the most recent ones. So a read whose answer was lost shows the same messages
/// again. The answer has a budget in bytes: it is cut in whole messages, never below one.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn read(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    request: ReadRequest,
    now: i64,
) -> Result<Read, RepoError> {
    let mut tx = pool.begin().await?;
    let me = request.hermes;
    let seen: Option<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT o.id, o.name, \
                EXISTS (SELECT 1 FROM peer_link l \
                        WHERE l.from_hermes_id = o.id AND l.to_hermes_id = $2) \
         FROM peer_thread t \
         JOIN hermes o ON o.id = CASE WHEN t.hermes_a = $2 THEN t.hermes_b ELSE t.hermes_a END \
         WHERE t.organization_id = $1 AND t.id = $3 AND (t.hermes_a = $2 OR t.hermes_b = $2) \
           AND o.deleted_at IS NULL \
           AND EXISTS (SELECT 1 FROM peer_link l \
                       WHERE (l.from_hermes_id = $2 AND l.to_hermes_id = o.id) \
                          OR (l.from_hermes_id = o.id AND l.to_hermes_id = $2))",
    )
    .bind(organization.id())
    .bind(me)
    .bind(request.thread)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((_, peer, inbound)) = seen else {
        return Ok(Read::NoThread);
    };
    let peer = name_of(&peer)?;
    let take = i64::from(request.limit).saturating_add(1);
    let cursor_of = |id: Uuid| {
        sqlx::query_as::<_, (i64, Uuid)>(
            "SELECT sent_at, id FROM peer_message \
             WHERE organization_id = $1 AND thread_id = $2 AND id = $3",
        )
        .bind(organization.id())
        .bind(request.thread)
        .bind(id)
    };
    // `newest_first`: the rows come from the most recent backwards, and the budget keeps the
    // most recent ones.
    let (rows, newest_first): (Vec<MessageRow>, bool) = if let Some(after) = request.after {
        let Some((at, id)) = cursor_of(after).fetch_optional(&mut *tx).await? else {
            return Ok(Read::NoCursor);
        };
        let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM peer_message m \
             WHERE {DELIVERABLE} AND (m.sent_at, m.id) > ($5, $6) \
             ORDER BY m.sent_at, m.id LIMIT $7"
        )))
        .bind(organization.id())
        .bind(request.thread)
        .bind(me)
        .bind(inbound)
        .bind(at)
        .bind(id)
        .bind(take)
        .fetch_all(&mut *tx)
        .await?;
        (rows, false)
    } else if let Some(before) = request.before {
        let Some((at, id)) = cursor_of(before).fetch_optional(&mut *tx).await? else {
            return Ok(Read::NoCursor);
        };
        let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM peer_message m \
             WHERE {DELIVERABLE} AND (m.sent_at, m.id) < ($5, $6) \
             ORDER BY m.sent_at DESC, m.id DESC LIMIT $7"
        )))
        .bind(organization.id())
        .bind(request.thread)
        .bind(me)
        .bind(inbound)
        .bind(at)
        .bind(id)
        .bind(take)
        .fetch_all(&mut *tx)
        .await?;
        (rows, true)
    } else {
        let first_unread: Option<(i64, Uuid)> = if inbound {
            sqlx::query_as(
                "SELECT sent_at, id FROM peer_message \
                 WHERE organization_id = $1 AND thread_id = $2 AND to_hermes_id = $3 \
                   AND read_at IS NULL ORDER BY sent_at, id LIMIT 1",
            )
            .bind(organization.id())
            .bind(request.thread)
            .bind(me)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            None
        };
        if let Some((at, id)) = first_unread {
            let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {MESSAGE_COLUMNS} FROM peer_message m \
                 WHERE {DELIVERABLE} AND (m.sent_at, m.id) >= ($5, $6) \
                 ORDER BY m.sent_at, m.id LIMIT $7"
            )))
            .bind(organization.id())
            .bind(request.thread)
            .bind(me)
            .bind(inbound)
            .bind(at)
            .bind(id)
            .bind(take)
            .fetch_all(&mut *tx)
            .await?;
            (rows, false)
        } else {
            let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {MESSAGE_COLUMNS} FROM peer_message m \
                 WHERE {DELIVERABLE} ORDER BY m.sent_at DESC, m.id DESC LIMIT $5"
            )))
            .bind(organization.id())
            .bind(request.thread)
            .bind(me)
            .bind(inbound)
            .bind(take)
            .fetch_all(&mut *tx)
            .await?;
            (rows, true)
        }
    };
    let mut more = rows.len() > request.limit as usize;
    let mut messages = Vec::new();
    let mut read_now = Vec::new();
    let mut bytes = 0usize;
    for (id, from, to, sent_at, nonce, text_enc, read_at) in
        rows.into_iter().take(request.limit as usize)
    {
        let ids = PeerMessageIds {
            thread: request.thread,
            message: id,
            from,
            to,
        };
        let Ok(text) = key.open_peer_text(&ids, &nonce, &text_enc) else {
            // A row someone changed in the database: it is not shown, and nothing of it is
            // logged but its identifier.
            tracing::error!(message = %id, "a stored message does not open");
            continue;
        };
        let size = text.as_str().len();
        if !messages.is_empty() && bytes.saturating_add(size) > PEER_READ_BUDGET_BYTES {
            more = true;
            break;
        }
        bytes = bytes.saturating_add(size);
        if to == me && read_at.is_none() {
            read_now.push(id);
        }
        messages.push(ReadMessage {
            id,
            mine: from == me,
            sent_at,
            text,
        });
    }
    if newest_first {
        messages.reverse();
    }
    if !read_now.is_empty() {
        sqlx::query(
            "UPDATE peer_message SET read_at = $4 \
             WHERE organization_id = $1 AND to_hermes_id = $2 AND id = ANY($3) \
               AND read_at IS NULL",
        )
        .bind(organization.id())
        .bind(me)
        .bind(&read_now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Read::Messages {
        peer,
        messages,
        more,
    })
}

// ---------------------------------------------------------------------------- notices

/// A notice that leaves now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueNotice {
    /// Its identifier, which comes back in `notice_ack`.
    pub id: Uuid,
    /// What it points at.
    pub target: NoticeTarget,
    /// How many new messages.
    pub count: u16,
}

/// What a notice points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeTarget {
    /// Messages of another Hermes.
    Peer {
        /// The Hermes that sent them.
        from: HermesName,
    },
    /// Messages in a chat of a number of WhatsApp.
    Whatsapp {
        /// The alias of the number.
        number: NumberAlias,
        /// The chat, by the identifier of the hub.
        chat: Uuid,
    },
}

/// Takes the notices of `hermes` that may leave now, each with a lease: until the lease ends
/// the hub waits for the `notice_ack`. Before that it puts back the ones whose lease ended
/// and expires the ones past their life. Each one is judged again. A notice of messages of a
/// Hermes: without the link, without its wake mark, or with a Hermes that may no longer be
/// woken, it is discarded. A notice of WhatsApp: without the verb "be woken" in that chat
/// (`repo::wake`), with a number that is not connected, or with a Hermes that may no longer
/// be woken, it is discarded.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn lease(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
    now: i64,
    limit: u32,
) -> Result<Vec<DueNotice>, RepoError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE notice SET state = 'expired', resolved_at = $3, leased_until = NULL \
         WHERE organization_id = $1 AND hermes_id = $2 AND state IN ('pending', 'sent') \
           AND expires_at <= $3",
    )
    .bind(organization.id())
    .bind(hermes)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let lapsed: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM notice \
         WHERE organization_id = $1 AND hermes_id = $2 AND state = 'sent' \
           AND leased_until <= $3 ORDER BY created_at, id FOR UPDATE",
    )
    .bind(organization.id())
    .bind(hermes)
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    for id in lapsed {
        back_to_pending(&mut tx, organization, id, now, now).await?;
    }
    let rows: Vec<(Uuid, i32, String, bool)> = sqlx::query_as(
        "SELECT n.id, n.count, f.name, \
                (h.can_be_woken AND f.deleted_at IS NULL \
                 AND EXISTS (SELECT 1 FROM peer_link l \
                             WHERE l.from_hermes_id = n.from_hermes_id \
                               AND l.to_hermes_id = n.hermes_id AND l.wake)) \
         FROM notice n \
         JOIN hermes f ON f.id = n.from_hermes_id \
         JOIN hermes h ON h.id = n.hermes_id \
         WHERE n.organization_id = $1 AND n.hermes_id = $2 AND n.kind = 'peer' \
           AND n.state = 'pending' AND n.not_before <= $3 \
           AND NOT EXISTS (SELECT 1 FROM notice s \
                           WHERE s.hermes_id = n.hermes_id \
                             AND s.from_hermes_id = n.from_hermes_id AND s.id <> n.id \
                             AND s.sent_at > $3 - $4) \
         ORDER BY n.created_at, n.id LIMIT $5 FOR UPDATE OF n SKIP LOCKED",
    )
    .bind(organization.id())
    .bind(hermes)
    .bind(now)
    .bind(PEER_NOTICE_GAP_MS)
    .bind(i64::from(limit))
    .fetch_all(&mut *tx)
    .await?;
    let mut judged: Vec<(Uuid, i32, Option<NoticeTarget>)> = Vec::new();
    for (id, count, from, still) in rows {
        let target = if still {
            Some(NoticeTarget::Peer {
                from: name_of(&from)?,
            })
        } else {
            None
        };
        judged.push((id, count, target));
    }
    let left = i64::from(limit).saturating_sub(i64::try_from(judged.len()).unwrap_or(i64::MAX));
    let rows: Vec<(Uuid, i32, String, Uuid, bool)> = sqlx::query_as(
        "SELECT n.id, n.count, m.alias, n.chat_id, \
                (h.can_be_woken AND m.state = 'connected' \
                 AND COALESCE(c.can_wake, \
                              a.scope = 'all' AND a.can_wake AND ch.kind = 'contact' \
                              AND NOT (ch.hidden_id AND EXISTS (SELECT 1 FROM access_chat x \
                                                                WHERE x.access_id = a.id)), \
                              FALSE)) \
         FROM notice n \
         JOIN hermes h ON h.id = n.hermes_id \
         JOIN number m ON m.id = n.number_id \
         JOIN chat ch ON ch.id = n.chat_id \
         LEFT JOIN access a ON a.hermes_id = n.hermes_id AND a.number_id = n.number_id \
         LEFT JOIN access_chat c ON c.access_id = a.id AND c.chat_id = n.chat_id \
         WHERE n.organization_id = $1 AND n.hermes_id = $2 AND n.kind = 'whatsapp' \
           AND n.state = 'pending' AND n.not_before <= $3 \
           AND m.state NOT IN ('reconnecting', 'failed', 'pairing') \
           AND NOT EXISTS (SELECT 1 FROM notice s \
                           WHERE s.hermes_id = n.hermes_id AND s.chat_id = n.chat_id \
                             AND s.id <> n.id AND s.sent_at > $3 - $4) \
         ORDER BY n.created_at, n.id LIMIT $5 FOR UPDATE OF n SKIP LOCKED",
    )
    .bind(organization.id())
    .bind(hermes)
    .bind(now)
    .bind(PEER_NOTICE_GAP_MS)
    .bind(left.max(0))
    .fetch_all(&mut *tx)
    .await?;
    for (id, count, alias, chat, still) in rows {
        let target = if still {
            Some(NoticeTarget::Whatsapp {
                number: NumberAlias::parse(&alias)
                    .map_err(|_| RepoError::Inconsistent("the alias of a number"))?,
                chat,
            })
        } else {
            None
        };
        judged.push((id, count, target));
    }
    let mut due = Vec::new();
    for (id, count, target) in judged {
        if let Some(target) = target {
            sqlx::query(
                "UPDATE notice SET state = 'sent', leased_until = $2, sent_at = $3, \
                        attempts = attempts + 1 WHERE id = $1",
            )
            .bind(id)
            .bind(now.saturating_add(NOTICE_LEASE_MS))
            .bind(now)
            .execute(&mut *tx)
            .await?;
            due.push(DueNotice {
                id,
                target,
                count: u16::try_from(count).unwrap_or(NOTICE_COUNT_MAX),
            });
        } else {
            sqlx::query(
                "UPDATE notice SET state = 'discarded', resolved_at = $2, leased_until = NULL \
                 WHERE id = $1",
            )
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(due)
}

/// Puts a sent notice back in the queue, to leave again from `not_before`. When its sender
/// already has another pending notice, the two become one: the count is added to that one.
async fn back_to_pending(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    id: Uuid,
    not_before: i64,
    now: i64,
) -> Result<(), RepoError> {
    let merged: Option<Uuid> = sqlx::query_scalar(
        "UPDATE notice p SET count = LEAST(p.count + s.count, $2) \
         FROM notice s \
         WHERE s.id = $1 AND s.organization_id = $3 AND p.organization_id = $3 \
           AND p.kind = s.kind AND p.state = 'pending' AND p.hermes_id = s.hermes_id \
           AND p.from_hermes_id IS NOT DISTINCT FROM s.from_hermes_id \
           AND p.chat_id IS NOT DISTINCT FROM s.chat_id \
         RETURNING p.id",
    )
    .bind(id)
    .bind(i32::from(NOTICE_COUNT_MAX))
    .bind(organization.id())
    .fetch_optional(&mut *connection)
    .await?;
    if merged.is_some() {
        sqlx::query(
            "UPDATE notice SET state = 'discarded', resolved_at = $2, leased_until = NULL \
             WHERE id = $1 AND organization_id = $3",
        )
        .bind(id)
        .bind(now)
        .bind(organization.id())
        .execute(connection)
        .await?;
    } else {
        sqlx::query(
            "UPDATE notice SET state = 'pending', leased_until = NULL, not_before = $2 \
             WHERE id = $1 AND organization_id = $3",
        )
        .bind(id)
        .bind(not_before)
        .bind(organization.id())
        .execute(connection)
        .await?;
    }
    Ok(())
}

/// What the connector answered about the notice `id` of `hermes`. An answer about a notice
/// that is not out (unknown, of another Hermes, already resolved) changes nothing.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn acknowledge(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
    id: Uuid,
    outcome: AckOutcome,
    now: i64,
) -> Result<(), RepoError> {
    let mut tx = pool.begin().await?;
    let attempts: Option<i32> = sqlx::query_scalar(
        "SELECT attempts FROM notice \
         WHERE organization_id = $1 AND hermes_id = $2 AND id = $3 AND state = 'sent' \
         FOR UPDATE",
    )
    .bind(organization.id())
    .bind(hermes)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(attempts) = attempts else {
        return Ok(());
    };
    let resolve = |state: &'static str| {
        sqlx::query(
            "UPDATE notice SET state = $2, resolved_at = $3, leased_until = NULL WHERE id = $1",
        )
        .bind(id)
        .bind(state)
        .bind(now)
    };
    match outcome {
        AckOutcome::Accepted | AckOutcome::Duplicate => {
            resolve("confirmed").execute(&mut *tx).await?;
        }
        AckOutcome::Rejected => {
            resolve("discarded").execute(&mut *tx).await?;
        }
        AckOutcome::Failed => {
            // 5 s, 10 s, 20 s... up to 5 minutes.
            let doublings = u32::try_from(attempts.clamp(1, 7)).unwrap_or(1) - 1;
            let wait = (5 * SECOND_MS)
                .saturating_mul(1i64 << doublings)
                .min(THROTTLED_RETRY_MS);
            back_to_pending(&mut tx, organization, id, now.saturating_add(wait), now).await?;
        }
        AckOutcome::Throttled => {
            back_to_pending(
                &mut tx,
                organization,
                id,
                now.saturating_add(THROTTLED_RETRY_MS),
                now,
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// The Hermes of the organization that have a notice that may leave now, whose lease ended or
/// that passed its life: their sessions look at their queue ([`lease`]).
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn hermes_with_due_notices(
    pool: &PgPool,
    organization: ResolvedOrg,
    now: i64,
) -> Result<Vec<Uuid>, RepoError> {
    Ok(sqlx::query_scalar(
        "SELECT DISTINCT hermes_id FROM notice \
         WHERE organization_id = $1 \
           AND ((state = 'pending' AND not_before <= $2) \
             OR (state = 'sent' AND leased_until <= $2) \
             OR (state IN ('pending', 'sent') AND expires_at <= $2))",
    )
    .bind(organization.id())
    .bind(now)
    .fetch_all(pool)
    .await?)
}

/// Records that the link from `from` to `to` reached a ceiling: the caller sees to it that
/// it is one event per link and per hour.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn record_ceiling(
    pool: &PgPool,
    organization: ResolvedOrg,
    from: Uuid,
    to: Uuid,
    which: Ceiling,
    now: i64,
) -> Result<(), RepoError> {
    audit::record(
        pool,
        now,
        &Event::new(Action::PeerCeilingReached, audit::Actor::System)
            .in_organization(organization)
            .about(TargetKind::Hermes, from)
            .with(
                Details::new()
                    .id("from_id", from)
                    .id("to_id", to)
                    .word("ceiling", which.as_str()),
            ),
    )
    .await
    .map_err(RepoError::Audit)?;
    Ok(())
}

/// Whether each of the Hermes `hermes` has links, in each direction: `(outbound, inbound)`.
/// One query for all of them.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn links_of(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, (bool, bool)>, RepoError> {
    if hermes.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<(Uuid, bool, bool)> = sqlx::query_as(
        "SELECT h.id, \
                EXISTS (SELECT 1 FROM peer_link l \
                        WHERE l.organization_id = $1 AND l.from_hermes_id = h.id), \
                EXISTS (SELECT 1 FROM peer_link l \
                        WHERE l.organization_id = $1 AND l.to_hermes_id = h.id) \
         FROM unnest($2::UUID[]) AS h(id)",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, outbound, inbound)| (id, (outbound, inbound)))
        .collect())
}
