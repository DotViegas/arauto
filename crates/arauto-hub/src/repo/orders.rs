//! Who gives a Hermes orders on WhatsApp, as the admin recorded it (decision 0051, a proposal).
//!
//! Per Hermes, one record: the telephone its own WhatsApp bridge runs on (it **takes** orders
//! there), and either its own WhatsApp in the chat with itself (`self-chat`) or a list of
//! telephones that **give** it orders. The hub reads it at every call that writes in a chat of
//! a number ([`counterparts`]) and refuses the writing in the chat between the two ends.
//!
//! A telephone is personal data. It is sealed ([`WhatsappField::HermesWhatsappPhone`]) and
//! found only by the keyed digest of each of its forms, the same function as
//! `number.phone_digest`, so a recorded telephone compares with a number of the hub without
//! being read. The sealed block is opened only for the admin command; nothing here ever puts
//! a telephone in an audit row, a log or an error.
//!
//! A refusal tells the Hermes whose record it is that the chat is its own command channel
//! (AR-714), on a channel that can read the code; every other Hermes gets AR-705 or AR-712.

use std::collections::BTreeMap;

use arauto_protocol::names::HermesName;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Admin, RepoError, lock_hermes_by_name};
use crate::audit::{self, Action, Details, Event, SecurityWrite, TargetKind};
use crate::db::ResolvedOrg;
use crate::engine::Phone;
use crate::mark::Mark;
use crate::vault::{OrganizationKey, WhatsappField};

/// How many telephones can give one Hermes orders.
pub const MAX_ORDERS_FROM: usize = 16;

/// What the admin recorded for one Hermes.
#[derive(Debug, Clone, Default)]
pub struct OrdersRecord {
    /// The telephone the Hermes's own WhatsApp bridge runs on. `None`: nothing recorded.
    pub takes_orders_on: Option<Phone>,
    /// Its own WhatsApp gives it orders, in the chat with itself (`self-chat`).
    pub self_chat: bool,
    /// The telephones that give it orders (`bot` mode). Empty with `self_chat`.
    pub orders_from: Vec<Phone>,
    /// Its orders come from outside WhatsApp (another conversation platform): no telephone,
    /// nothing refused (18/11/2026).
    pub outside_whatsapp: bool,
}

/// Why a record cannot be kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// A list of telephones, or `self-chat`, without the telephone the bridge runs on: a chat
    /// has two ends.
    NoTelephoneOfTheHermes,
    /// `self-chat` and a list are two answers to the same question.
    SelfChatWithAList,
    /// The telephone the bridge runs on is also in the list: that chat is the self-chat mode.
    BridgeAmongGivers,
    /// More than [`MAX_ORDERS_FROM`] telephones.
    TooMany,
    /// Outside WhatsApp together with a telephone of a WhatsApp, self-chat or a list: two
    /// answers to the same question.
    OutsideWithATelephone,
}

fn same(a: &Phone, b: &Phone) -> bool {
    a.forms().contains(b)
}

impl OrdersRecord {
    /// Whether nothing is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.takes_orders_on.is_none()
            && !self.self_chat
            && self.orders_from.is_empty()
            && !self.outside_whatsapp
    }

    /// The word for the mode, in the audit.
    #[must_use]
    pub const fn mode_word(&self) -> &'static str {
        if self.outside_whatsapp {
            "outside_whatsapp"
        } else if self.self_chat {
            "self_chat"
        } else {
            "bot"
        }
    }

    /// The chats this record protects: each `(telephone the bridge runs on, telephone that
    /// gives it orders)`; with `self-chat`, the telephone with itself.
    #[must_use]
    pub fn pairs(&self) -> Vec<(Phone, Phone)> {
        let Some(takes) = &self.takes_orders_on else {
            return Vec::new();
        };
        if self.self_chat {
            return vec![(takes.clone(), takes.clone())];
        }
        self.orders_from
            .iter()
            .map(|from| (takes.clone(), from.clone()))
            .collect()
    }

    /// Whether this record protects no chat.
    #[must_use]
    pub fn protects_nothing(&self) -> bool {
        self.pairs().is_empty()
    }

    /// Whether the risk of nothing protected is said: nothing is protected and the admin did not
    /// answer that its orders come from outside WhatsApp.
    #[must_use]
    pub fn needs_the_warning(&self) -> bool {
        self.protects_nothing() && !self.outside_whatsapp
    }

    /// The chats of this record that `after` leaves unprotected, in the order of
    /// [`Self::pairs`].
    #[must_use]
    pub fn lost_in(&self, after: &Self) -> Vec<(Phone, Phone)> {
        let kept = after.pairs();
        self.pairs()
            .into_iter()
            .filter(|(a, b)| !kept.iter().any(|(c, d)| same(a, c) && same(b, d)))
            .collect()
    }

    /// Whether `after` leaves unprotected a chat that this record protects.
    #[must_use]
    pub fn loses_a_pair_in(&self, after: &Self) -> bool {
        !self.lost_in(after).is_empty()
    }

    /// The rules of the record.
    ///
    /// # Errors
    /// [`Invalid`] says which one is broken.
    pub fn check(&self) -> Result<(), Invalid> {
        if self.outside_whatsapp
            && (self.takes_orders_on.is_some() || self.self_chat || !self.orders_from.is_empty())
        {
            return Err(Invalid::OutsideWithATelephone);
        }
        if self.self_chat && !self.orders_from.is_empty() {
            return Err(Invalid::SelfChatWithAList);
        }
        if (self.self_chat || !self.orders_from.is_empty()) && self.takes_orders_on.is_none() {
            return Err(Invalid::NoTelephoneOfTheHermes);
        }
        if let Some(takes) = &self.takes_orders_on
            && self.orders_from.iter().any(|from| same(takes, from))
        {
            return Err(Invalid::BridgeAmongGivers);
        }
        if self.orders_from.len() > MAX_ORDERS_FROM {
            return Err(Invalid::TooMany);
        }
        Ok(())
    }

    /// The same record with each telephone once, sorted.
    pub(crate) fn normalized(mut self) -> Self {
        let mut seen: Vec<Phone> = Vec::new();
        for phone in std::mem::take(&mut self.orders_from) {
            if !seen.iter().any(|known| same(known, &phone)) {
                seen.push(phone);
            }
        }
        seen.sort_by(|a, b| a.digits().cmp(b.digits()));
        self.orders_from = seen;
        self
    }

    fn equals(&self, other: &Self) -> bool {
        let a = self.clone().normalized();
        let b = other.clone().normalized();
        a.self_chat == b.self_chat
            && a.outside_whatsapp == b.outside_whatsapp
            && a.takes_orders_on.as_ref().map(Phone::digits)
                == b.takes_orders_on.as_ref().map(Phone::digits)
            && a.orders_from
                .iter()
                .map(Phone::digits)
                .eq(b.orders_from.iter().map(Phone::digits))
    }
}

fn digest_of(key: &OrganizationKey, organization: ResolvedOrg, phone: &Phone) -> [u8; 32] {
    key.whatsapp_digest(
        WhatsappField::Phone,
        organization.id(),
        phone.digits().as_bytes(),
    )
}

/// The digest of each form of a telephone.
fn digests_of(key: &OrganizationKey, organization: ResolvedOrg, phone: &Phone) -> Vec<[u8; 32]> {
    phone
        .forms()
        .iter()
        .map(|form| digest_of(key, organization, form))
        .collect()
}

#[derive(sqlx::FromRow)]
struct PhoneRow {
    id: Uuid,
    role: String,
    whatsapp_mode: Option<String>,
    phone_enc: Vec<u8>,
}

async fn read_in(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    hermes: Uuid,
) -> Result<OrdersRecord, RepoError> {
    let rows: Vec<PhoneRow> = sqlx::query_as(
        "SELECT id, role, whatsapp_mode, phone_enc FROM hermes_whatsapp_phone \
         WHERE organization_id = $1 AND hermes_id = $2",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_all(&mut *connection)
    .await?;
    let any_phone = !rows.is_empty();
    let mut record = OrdersRecord::default();
    for row in rows {
        let opened = key
            .open_whatsapp(WhatsappField::HermesWhatsappPhone, row.id, &row.phone_enc)
            .map_err(|_| RepoError::Inconsistent("a recorded telephone does not open"))?;
        let phone = std::str::from_utf8(&opened)
            .ok()
            .and_then(|text| Phone::parse(text).ok())
            .ok_or(RepoError::Inconsistent("a recorded telephone"))?;
        match row.role.as_str() {
            "takes_orders" => {
                record.self_chat = row.whatsapp_mode.as_deref() == Some("self_chat");
                record.takes_orders_on = Some(phone);
            }
            "gives_orders" => record.orders_from.push(phone),
            _ => return Err(RepoError::Inconsistent("the role of a recorded telephone")),
        }
    }
    let outside: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM hermes_orders_outside_whatsapp \
         WHERE organization_id = $1 AND hermes_id = $2)",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_one(connection)
    .await?;
    // A hand edit that left both: the telephones win, the refusing side.
    record.outside_whatsapp = outside && !any_phone;
    Ok(record.normalized())
}

/// What the admin recorded for the Hermes `hermes` (an empty record when nothing).
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when a sealed
/// telephone does not open.
pub async fn get(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    hermes: Uuid,
) -> Result<OrdersRecord, RepoError> {
    let mut connection = pool.acquire().await?;
    read_in(&mut connection, organization, key, hermes).await
}

/// The Hermes, other than `hermes`, that already takes orders on `takes` (either form).
async fn taken_in(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    hermes: Uuid,
    takes: &Phone,
) -> Result<Option<HermesName>, RepoError> {
    let digests: Vec<Vec<u8>> = digests_of(key, organization, takes)
        .into_iter()
        .map(|digest| digest.to_vec())
        .collect();
    let other: Option<String> = sqlx::query_scalar(
        "SELECT h.name FROM hermes_whatsapp_phone_digest d          JOIN hermes_whatsapp_phone p            ON p.organization_id = d.organization_id AND p.id = d.phone_id          JOIN hermes h ON h.organization_id = p.organization_id AND h.id = p.hermes_id          WHERE d.organization_id = $1 AND d.role = 'takes_orders' AND d.digest = ANY($2)            AND p.hermes_id <> $3 AND h.deleted_at IS NULL          LIMIT 1",
    )
    .bind(organization.id())
    .bind(&digests)
    .bind(hermes)
    .fetch_optional(connection)
    .await?;
    other
        .map(|other| {
            HermesName::parse(&other).map_err(|_| RepoError::Inconsistent("the name of a Hermes"))
        })
        .transpose()
}

/// Read-only: the Hermes, other than `hermes`, that already takes orders on `takes`. The panel
/// says it before the window that asks; [`set`] checks again inside its write.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn bridge_taken_by(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    hermes: Uuid,
    takes: &Phone,
) -> Result<Option<HermesName>, RepoError> {
    let mut connection = pool.acquire().await?;
    taken_in(&mut connection, organization, key, hermes, takes).await
}

/// What [`set`] did.
#[derive(Debug, Clone)]
pub enum SetOutcome {
    /// The record was written.
    Set {
        /// As it was.
        before: OrdersRecord,
        /// As it is now.
        after: OrdersRecord,
    },
    /// The edit gave the record it already was: nothing was written.
    Unchanged(OrdersRecord),
    /// There is no such Hermes (or it was removed).
    NoSuchHermes,
    /// The edit breaks a rule of the record.
    Invalid(Invalid),
    /// This Hermes already takes orders on that telephone: one bridge per telephone.
    TelephoneTaken(HermesName),
}

/// Replaces the whole record of the Hermes `name` with what `edit` makes of the record it
/// has, in one security write (`docs/04`, C11: refused on a restored database).
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn set(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    key: &OrganizationKey,
    name: &HermesName,
    edit: impl FnOnce(&OrdersRecord) -> OrdersRecord,
    now: i64,
) -> Result<SetOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(SetOutcome::NoSuchHermes);
    };
    let hermes = locked.hermes.id;
    let before = read_in(write.connection(), organization, key, hermes).await?;
    let after = edit(&before).normalized();
    if let Err(invalid) = after.check() {
        write.rollback().await?;
        return Ok(SetOutcome::Invalid(invalid));
    }
    if before.equals(&after) {
        write.rollback().await?;
        return Ok(SetOutcome::Unchanged(before));
    }
    if let Some(takes) = &after.takes_orders_on
        && let Some(other) = taken_in(write.connection(), organization, key, hermes, takes).await?
    {
        write.rollback().await?;
        return Ok(SetOutcome::TelephoneTaken(other));
    }
    sqlx::query("DELETE FROM hermes_whatsapp_phone WHERE organization_id = $1 AND hermes_id = $2")
        .bind(organization.id())
        .bind(hermes)
        .execute(write.connection())
        .await?;
    sqlx::query(
        "DELETE FROM hermes_orders_outside_whatsapp WHERE organization_id = $1 AND hermes_id = $2",
    )
    .bind(organization.id())
    .bind(hermes)
    .execute(write.connection())
    .await?;
    if let Some(takes) = &after.takes_orders_on {
        let mode = if after.self_chat { "self_chat" } else { "bot" };
        insert(
            write.connection(),
            organization,
            key,
            hermes,
            "takes_orders",
            Some(mode),
            takes,
            now,
        )
        .await?;
    }
    for from in &after.orders_from {
        insert(
            write.connection(),
            organization,
            key,
            hermes,
            "gives_orders",
            None,
            from,
            now,
        )
        .await?;
    }
    if after.outside_whatsapp {
        sqlx::query(
            "INSERT INTO hermes_orders_outside_whatsapp (organization_id, hermes_id, created_at) \
             VALUES ($1, $2, $3)",
        )
        .bind(organization.id())
        .bind(hermes)
        .bind(now)
        .execute(write.connection())
        .await?;
    }
    write
        .record(
            now,
            &Event::new(Action::HermesWhatsappOrdersChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, hermes)
                .with(change_details(&before, &after)),
        )
        .await?;
    write.commit().await?;
    Ok(SetOutcome::Set { before, after })
}

/// Counts and words only: never a telephone.
fn change_details(before: &OrdersRecord, after: &OrdersRecord) -> Details {
    let count = |record: &OrdersRecord| i64::try_from(record.orders_from.len()).unwrap_or(0);
    Details::new()
        .flag("bridge_telephone_before", before.takes_orders_on.is_some())
        .flag("bridge_telephone_after", after.takes_orders_on.is_some())
        .word("mode_before", before.mode_word())
        .word("mode_after", after.mode_word())
        .number("telephones_before", count(before))
        .number("telephones_after", count(after))
}

#[allow(clippy::too_many_arguments)]
async fn insert(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    hermes: Uuid,
    role: &'static str,
    mode: Option<&'static str>,
    phone: &Phone,
    now: i64,
) -> Result<(), RepoError> {
    let id = Uuid::new_v4();
    let phone_enc = key.seal_whatsapp(
        WhatsappField::HermesWhatsappPhone,
        id,
        phone.digits().as_bytes(),
    )?;
    sqlx::query(
        "INSERT INTO hermes_whatsapp_phone \
             (id, organization_id, hermes_id, role, whatsapp_mode, phone_enc, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(hermes)
    .bind(role)
    .bind(mode)
    .bind(phone_enc)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    for digest in digests_of(key, organization, phone) {
        sqlx::query(
            "INSERT INTO hermes_whatsapp_phone_digest (organization_id, phone_id, role, digest) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(organization.id())
        .bind(id)
        .bind(role)
        .bind(digest.as_slice())
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// The record of a Hermes that is being removed goes with it, in the same write, audited.
pub(crate) async fn end_with_the_hermes(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    admin: &Admin,
    hermes: Uuid,
    now: i64,
) -> Result<(), RepoError> {
    let (rows, givers): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE role = 'gives_orders') \
         FROM hermes_whatsapp_phone WHERE organization_id = $1 AND hermes_id = $2",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_one(write.connection())
    .await?;
    let outside: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hermes_orders_outside_whatsapp \
         WHERE organization_id = $1 AND hermes_id = $2",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_one(write.connection())
    .await?;
    if rows == 0 && outside == 0 {
        return Ok(());
    }
    sqlx::query("DELETE FROM hermes_whatsapp_phone WHERE organization_id = $1 AND hermes_id = $2")
        .bind(organization.id())
        .bind(hermes)
        .execute(write.connection())
        .await?;
    sqlx::query(
        "DELETE FROM hermes_orders_outside_whatsapp WHERE organization_id = $1 AND hermes_id = $2",
    )
    .bind(organization.id())
    .bind(hermes)
    .execute(write.connection())
    .await?;
    let details = if rows == 0 {
        Details::new()
            .flag("bridge_telephone_before", false)
            .flag("bridge_telephone_after", false)
            .word("mode_before", "outside_whatsapp")
            .number("telephones_before", 0)
            .number("telephones_after", 0)
            .word("reason", "hermes_removed")
    } else {
        Details::new()
            .flag("bridge_telephone_before", true)
            .flag("bridge_telephone_after", false)
            .number("telephones_before", givers)
            .number("telephones_after", 0)
            .word("reason", "hermes_removed")
    };
    write
        .record(
            now,
            &Event::new(Action::HermesWhatsappOrdersChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, hermes)
                .with(details),
        )
        .await?;
    Ok(())
}

/// One record row at the other end of the chats of a number: whose record it is, and the digest
/// of each form of its telephone.
#[derive(Debug, Clone)]
struct Counterpart {
    hermes: Uuid,
    digests: Vec<[u8; 32]>,
}

/// A telephone that is a counterpart of the number, as the Hermes that calls sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Matched {
    /// The first record row it matches, in the order of the rows: the key of the hourly memory.
    pub record: Uuid,
    /// Some row it matches is of the record of the Hermes that calls: the chat is that Hermes's
    /// own command channel (decision 0051, 18/11/2026).
    pub own: bool,
}

/// The telephones at the other end of the chats this number must not be written in, read at
/// a call that writes (rule 9: against the current state).
#[derive(Debug, Clone, Default)]
pub struct Counterparts {
    /// The record row each telephone comes from, with the Hermes whose record it is and the
    /// digest of each form.
    by_record: BTreeMap<Uuid, Counterpart>,
}

impl Counterparts {
    /// Whether the number has no recorded counterpart: nothing of it is refused.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_record.is_empty()
    }

    /// The record row whose telephone is `phone`, when it is a counterpart, and whether some
    /// row it matches is of the record of `caller`.
    #[must_use]
    pub fn matching(
        &self,
        key: &OrganizationKey,
        organization: ResolvedOrg,
        phone: &Phone,
        caller: Uuid,
    ) -> Option<Matched> {
        let digest = digest_of(key, organization, phone);
        let mut hits = self
            .by_record
            .iter()
            .filter(|(_, c)| c.digests.contains(&digest));
        let (record, first) = hits.next()?;
        let own = first.hermes == caller || hits.any(|(_, c)| c.hermes == caller);
        Some(Matched {
            record: *record,
            own,
        })
    }
}

/// The counterparts of the number `number`, over the Hermes that were not removed:
///
/// - the telephone the bridge of a Hermes runs on, for each Hermes for which this number is a
///   telephone that gives orders;
/// - the telephones that give orders to the Hermes whose bridge runs on this number;
/// - this number itself, when that Hermes takes orders in `self-chat`.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn counterparts(
    pool: &PgPool,
    organization: ResolvedOrg,
    number: Uuid,
) -> Result<Counterparts, RepoError> {
    let rows: Vec<(Uuid, Uuid, Vec<u8>)> = sqlx::query_as(
        "WITH mine AS ( \
             SELECT p.id, p.hermes_id, p.role, p.whatsapp_mode \
             FROM number n \
             JOIN hermes_whatsapp_phone_digest d \
               ON d.organization_id = n.organization_id AND d.digest = n.phone_digest \
             JOIN hermes_whatsapp_phone p \
               ON p.organization_id = d.organization_id AND p.id = d.phone_id \
             JOIN hermes h \
               ON h.organization_id = p.organization_id AND h.id = p.hermes_id \
              AND h.deleted_at IS NULL \
             WHERE n.organization_id = $1 AND n.id = $2 \
         ) \
         SELECT DISTINCT o.id, o.hermes_id, od.digest \
         FROM mine \
         JOIN hermes_whatsapp_phone o \
           ON o.organization_id = $1 AND o.hermes_id = mine.hermes_id \
          AND ((mine.role = 'gives_orders' AND o.role = 'takes_orders') \
            OR (mine.role = 'takes_orders' \
                AND (o.role = 'gives_orders' \
                     OR (o.id = mine.id AND mine.whatsapp_mode = 'self_chat')))) \
         JOIN hermes_whatsapp_phone_digest od \
           ON od.organization_id = o.organization_id AND od.phone_id = o.id",
    )
    .bind(organization.id())
    .bind(number)
    .fetch_all(pool)
    .await?;
    let mut by_record: BTreeMap<Uuid, Counterpart> = BTreeMap::new();
    for (record, hermes, digest) in rows {
        let digest: [u8; 32] = digest
            .try_into()
            .map_err(|_| RepoError::Inconsistent("a digest of a recorded telephone"))?;
        by_record
            .entry(record)
            .or_insert_with(|| Counterpart {
                hermes,
                digests: Vec::new(),
            })
            .digests
            .push(digest);
    }
    Ok(Counterparts { by_record })
}

/// How a number takes part in the record of a Hermes, for `number list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The number.
    pub number: Uuid,
    /// The Hermes.
    pub hermes: HermesName,
    /// Whether the number is where the Hermes takes orders (else it gives orders to it).
    pub takes_orders_here: bool,
}

/// The tags of the numbers that were not removed, over the Hermes that were not removed.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn tags(pool: &PgPool, organization: ResolvedOrg) -> Result<Vec<Tag>, RepoError> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT DISTINCT n.id, h.name, p.role \
         FROM number n \
         JOIN hermes_whatsapp_phone_digest d \
           ON d.organization_id = n.organization_id AND d.digest = n.phone_digest \
         JOIN hermes_whatsapp_phone p \
           ON p.organization_id = d.organization_id AND p.id = d.phone_id \
         JOIN hermes h \
           ON h.organization_id = p.organization_id AND h.id = p.hermes_id \
          AND h.deleted_at IS NULL \
         WHERE n.organization_id = $1 AND n.removed_at IS NULL \
         ORDER BY h.name, n.id, p.role",
    )
    .bind(organization.id())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(number, hermes, role)| {
            Ok(Tag {
                number,
                hermes: HermesName::parse(&hermes)
                    .map_err(|_| RepoError::Inconsistent("the name of a Hermes"))?,
                takes_orders_here: role == "takes_orders",
            })
        })
        .collect()
}

/// Whether some Hermes that was not removed has an answer that counts: a chat protected (a
/// telephone that gives it orders, or self-chat) or orders from outside WhatsApp. While none
/// has, `number list` says so.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn some_hermes_is_recorded(
    pool: &PgPool,
    organization: ResolvedOrg,
) -> Result<bool, RepoError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS ( \
             SELECT 1 FROM hermes_whatsapp_phone p \
             JOIN hermes h ON h.organization_id = p.organization_id AND h.id = p.hermes_id \
              AND h.deleted_at IS NULL \
             WHERE p.organization_id = $1 \
               AND (p.role = 'gives_orders' OR p.whatsapp_mode = 'self_chat')) \
         OR EXISTS ( \
             SELECT 1 FROM hermes_orders_outside_whatsapp o \
             JOIN hermes h ON h.organization_id = o.organization_id AND h.id = o.hermes_id \
              AND h.deleted_at IS NULL \
             WHERE o.organization_id = $1)",
    )
    .bind(organization.id())
    .fetch_one(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone(text: &str) -> Phone {
        Phone::parse(text).unwrap()
    }

    #[test]
    fn a_record_follows_its_rules() {
        let mut record = OrdersRecord {
            takes_orders_on: Some(phone("+5511900000001")),
            self_chat: false,
            orders_from: vec![phone("+5511900000002")],
            outside_whatsapp: false,
        };
        assert_eq!(record.check(), Ok(()));
        record.self_chat = true;
        assert_eq!(record.check(), Err(Invalid::SelfChatWithAList));
        record.orders_from.clear();
        assert_eq!(record.check(), Ok(()));
        record.takes_orders_on = None;
        assert_eq!(record.check(), Err(Invalid::NoTelephoneOfTheHermes));
        let list = OrdersRecord {
            takes_orders_on: None,
            self_chat: false,
            orders_from: vec![phone("+5511900000002")],
            outside_whatsapp: false,
        };
        assert_eq!(list.check(), Err(Invalid::NoTelephoneOfTheHermes));
        let many = OrdersRecord {
            takes_orders_on: Some(phone("+5511900000001")),
            self_chat: false,
            orders_from: (0..17)
                .map(|n| phone(&format!("+55119000001{n:02}")))
                .collect(),
            outside_whatsapp: false,
        };
        assert_eq!(many.check(), Err(Invalid::TooMany));
        assert!(OrdersRecord::default().protects_nothing());
        let among = OrdersRecord {
            takes_orders_on: Some(phone("+5511987654321")),
            self_chat: false,
            orders_from: vec![phone("+5511987654321")],
            outside_whatsapp: false,
        };
        assert_eq!(among.check(), Err(Invalid::BridgeAmongGivers));
        let other_form = OrdersRecord {
            orders_from: vec![phone("+551187654321")],
            ..among.clone()
        };
        assert_eq!(other_form.check(), Err(Invalid::BridgeAmongGivers));
        let both = OrdersRecord {
            self_chat: true,
            ..among
        };
        assert_eq!(both.check(), Err(Invalid::SelfChatWithAList));
    }

    #[test]
    fn a_telephone_in_both_forms_is_one_pair() {
        let with = phone("+5511987654321");
        let without = phone("+551187654321");
        let before = OrdersRecord {
            takes_orders_on: Some(phone("+5511900000001")),
            self_chat: false,
            orders_from: vec![with],
            outside_whatsapp: false,
        };
        let after = OrdersRecord {
            orders_from: vec![without],
            ..before.clone()
        };
        assert!(!before.loses_a_pair_in(&after));
        let cleared = OrdersRecord {
            orders_from: Vec::new(),
            ..before.clone()
        };
        assert!(before.loses_a_pair_in(&cleared));
        assert!(!cleared.loses_a_pair_in(&before));
    }

    #[test]
    fn the_pairs_that_go_are_named() {
        let bridge = phone("+5511900000001");
        let before = OrdersRecord {
            takes_orders_on: Some(bridge.clone()),
            self_chat: false,
            orders_from: vec![phone("+5511900000002"), phone("+5511900000003")],
            outside_whatsapp: false,
        };
        let after = OrdersRecord {
            orders_from: vec![phone("+5511900000002")],
            ..before.clone()
        };
        let lost = before.lost_in(&after);
        assert_eq!(lost.len(), 1);
        assert_eq!(lost[0].0.digits(), bridge.digits());
        assert_eq!(lost[0].1.digits(), "5511900000003");
        assert!(after.lost_in(&before).is_empty());
    }

    #[test]
    fn orders_from_outside_whatsapp_are_an_answer_alone() {
        let outside = OrdersRecord {
            outside_whatsapp: true,
            ..OrdersRecord::default()
        };
        assert_eq!(outside.check(), Ok(()));
        let bridge = OrdersRecord {
            takes_orders_on: Some(phone("+5511900000001")),
            ..outside.clone()
        };
        assert_eq!(bridge.check(), Err(Invalid::OutsideWithATelephone));
        let own = OrdersRecord {
            self_chat: true,
            ..outside.clone()
        };
        assert_eq!(own.check(), Err(Invalid::OutsideWithATelephone));
        let list = OrdersRecord {
            orders_from: vec![phone("+5511900000002")],
            ..outside.clone()
        };
        assert_eq!(list.check(), Err(Invalid::OutsideWithATelephone));
        let several = OrdersRecord {
            self_chat: true,
            orders_from: vec![phone("+5511900000002")],
            ..outside.clone()
        };
        assert_eq!(several.check(), Err(Invalid::OutsideWithATelephone));
        assert!(outside.protects_nothing());
        assert!(!outside.needs_the_warning());
        assert!(!outside.is_empty());
        assert_eq!(outside.mode_word(), "outside_whatsapp");
        assert!(OrdersRecord::default().needs_the_warning());
        let lone_bridge = OrdersRecord {
            takes_orders_on: Some(phone("+5511900000001")),
            ..OrdersRecord::default()
        };
        assert!(lone_bridge.needs_the_warning());
        let before = OrdersRecord {
            takes_orders_on: Some(phone("+5511900000001")),
            orders_from: vec![phone("+5511900000002")],
            ..OrdersRecord::default()
        };
        let lost = before.lost_in(&outside);
        assert_eq!(lost.len(), 1);
        assert_eq!(lost[0].1.digits(), "5511900000002");
        assert!(outside.equals(&outside.clone()));
        assert!(!outside.equals(&OrdersRecord::default()));
    }
}
