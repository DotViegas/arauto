//! The numbers of an organization (`docs/06-modelo-de-dados.md`, section 4) and their states
//! (`docs/10-maquinas-de-estado.md`, section 4, the rule in [`crate::core::number`]).
//!
//! The telephone of a number is encrypted with the key of the organization; a keyed digest
//! finds it. The name of its session in the engine is drawn by the hub and never reused. The
//! alias has the restricted format of the name of a Hermes and is never reused either: a
//! Hermes that knew a number by it never meets another one under the same alias.
//!
//! What the admin does goes through [`crate::audit::begin_security`] even when the event is
//! not a security one: a database that came back from an older copy gets nothing from the
//! admin command.

use arauto_protocol::names::NumberAlias;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use super::alert::{AlertDetails, AlertKind, NewAlert, raise};
use super::{Admin, RepoError, notify_hub};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::core::number::{NumberState, Trigger, next};
use crate::db::ResolvedOrg;
use crate::engine::{EngineStatus, Phone, SessionName};
use crate::mark::Mark;
use crate::vault::{OrganizationKey, WhatsappField};

/// A number.
#[derive(Clone, PartialEq, Eq)]
pub struct Number {
    /// The row.
    pub id: Uuid,
    /// Its engine.
    pub engine_id: Uuid,
    /// The name of its session in the engine.
    pub session: SessionName,
    /// The alias the admin gave it.
    pub alias: NumberAlias,
    /// Marked by the admin as a command channel of some Hermes (`docs/08-permissoes.md`, 6).
    pub command: bool,
    /// Its state.
    pub state: NumberState,
    /// Since when.
    pub state_since: i64,
    /// When it was paired last.
    pub paired_at: Option<i64>,
    /// Until when the pairing in progress is valid.
    pub pairing_until: Option<i64>,
    /// The digest of the configuration of its session the hub applied last.
    pub config_digest: Option<Vec<u8>>,
    /// The newest message recorded, in milliseconds of the engine.
    pub last_message_at: Option<i64>,
    /// Up to when the catch-up read everything, in milliseconds of the hub. Only the
    /// catch-up moves it.
    pub caught_up_until: Option<i64>,
    /// When it was added.
    pub created_at: i64,
    phone_enc: Vec<u8>,
}

impl std::fmt::Debug for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Number")
            .field("id", &self.id)
            .field("alias", &self.alias)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Number {
    /// The telephone, opened with the key of the organization.
    ///
    /// # Errors
    /// [`RepoError::Inconsistent`] when it does not open or is not a telephone.
    pub fn phone(&self, key: &OrganizationKey) -> Result<Phone, RepoError> {
        let opened = key
            .open_whatsapp(WhatsappField::Phone, self.id, &self.phone_enc)
            .map_err(|_| RepoError::Inconsistent("the telephone of a number does not open"))?;
        let text = std::str::from_utf8(&opened)
            .map_err(|_| RepoError::Inconsistent("the telephone of a number is not text"))?;
        Phone::parse(text).map_err(|_| RepoError::Inconsistent("the telephone of a number"))
    }
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    engine_id: Uuid,
    session_name: String,
    alias: String,
    command: bool,
    state: String,
    state_since: i64,
    paired_at: Option<i64>,
    pairing_until: Option<i64>,
    config_digest: Option<Vec<u8>>,
    last_message_at: Option<i64>,
    caught_up_until: Option<i64>,
    created_at: i64,
    phone_enc: Vec<u8>,
}

/// The columns of [`Row`], as a literal for the statements.
macro_rules! columns {
    () => {
        "id, engine_id, session_name, alias, command, state, state_since, paired_at, \
         pairing_until, config_digest, last_message_at, caught_up_until, created_at, \
         phone_enc"
    };
}

impl TryFrom<Row> for Number {
    type Error = RepoError;

    fn try_from(row: Row) -> Result<Self, RepoError> {
        Ok(Self {
            id: row.id,
            engine_id: row.engine_id,
            session: SessionName::parse(&row.session_name)
                .map_err(|_| RepoError::Inconsistent("the session name of a number"))?,
            alias: NumberAlias::parse(&row.alias)
                .map_err(|_| RepoError::Inconsistent("the alias of a number"))?,
            command: row.command,
            state: NumberState::parse(&row.state)
                .ok_or(RepoError::Inconsistent("the state of a number"))?,
            state_since: row.state_since,
            paired_at: row.paired_at,
            pairing_until: row.pairing_until,
            config_digest: row.config_digest,
            last_message_at: row.last_message_at,
            caught_up_until: row.caught_up_until,
            created_at: row.created_at,
            phone_enc: row.phone_enc,
        })
    }
}

fn numbers(rows: Vec<Row>) -> Result<Vec<Number>, RepoError> {
    rows.into_iter().map(Number::try_from).collect()
}

/// What [`add`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
// One value at a time, moved once to whoever asked: boxing the number buys nothing.
#[allow(clippy::large_enum_variant)]
pub enum AddOutcome {
    /// The number was added, unpaired.
    Added(Number),
    /// There is no engine yet.
    NoEngine,
    /// The alias is taken, now or by a number that was removed.
    AliasTaken,
    /// The telephone is already a number of the organization.
    PhoneTaken,
}

/// Adds a number: unpaired, with a session name drawn now. The session of the engine is
/// created by whoever calls this, already configured, or by the next check of the hub.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn add(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    key: &OrganizationKey,
    alias: &NumberAlias,
    phone: &Phone,
    command: bool,
    now: i64,
) -> Result<AddOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let engine: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM engine WHERE organization_id = $1")
            .bind(organization.id())
            .fetch_optional(write.connection())
            .await?;
    let Some(engine) = engine else {
        write.rollback().await?;
        return Ok(AddOutcome::NoEngine);
    };
    let alias_taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM number WHERE organization_id = $1 AND alias = $2)",
    )
    .bind(organization.id())
    .bind(alias.as_str())
    .fetch_one(write.connection())
    .await?;
    if alias_taken {
        write.rollback().await?;
        return Ok(AddOutcome::AliasTaken);
    }
    let digest = key.whatsapp_digest(
        WhatsappField::Phone,
        organization.id(),
        phone.digits().as_bytes(),
    );
    let phone_taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM number WHERE organization_id = $1 AND phone_digest = $2 \
                        AND removed_at IS NULL)",
    )
    .bind(organization.id())
    .bind(digest.as_slice())
    .fetch_one(write.connection())
    .await?;
    if phone_taken {
        write.rollback().await?;
        return Ok(AddOutcome::PhoneTaken);
    }
    let id = Uuid::new_v4();
    let session = SessionName::draw().map_err(|_| RepoError::Random)?;
    let phone_enc = key.seal_whatsapp(WhatsappField::Phone, id, phone.digits().as_bytes())?;
    let row: Row = sqlx::query_as(concat!(
        "INSERT INTO number (id, organization_id, engine_id, session_name, phone_enc, \
                             phone_digest, alias, command, state, state_since, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'unpaired', $9, $9) \
         RETURNING ",
        columns!(),
        ""
    ))
    .bind(id)
    .bind(organization.id())
    .bind(engine)
    .bind(session.as_str())
    .bind(phone_enc)
    .bind(digest.as_slice())
    .bind(alias.as_str())
    .bind(command)
    .bind(now)
    .fetch_one(write.connection())
    .await?;
    write
        .record(
            now,
            &Event::new(Action::NumberAdded, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Number, id)
                .with(
                    Details::new()
                        .alias("alias", alias)
                        .flag("command", command),
                ),
        )
        .await?;
    notify_hub(write.connection(), organization, "number", id).await?;
    write.commit().await?;
    Ok(AddOutcome::Added(Number::try_from(row)?))
}

/// The numbers of `organization`, by alias; the removed ones only when asked.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn list(
    pool: &PgPool,
    organization: ResolvedOrg,
    include_removed: bool,
) -> Result<Vec<Number>, RepoError> {
    let rows: Vec<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM number WHERE organization_id = $1 \
           AND ($2 OR removed_at IS NULL) ORDER BY alias"
    ))
    .bind(organization.id())
    .bind(include_removed)
    .fetch_all(pool)
    .await?;
    numbers(rows)
}

/// The number with this alias that was not removed.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn by_alias(
    pool: &PgPool,
    organization: ResolvedOrg,
    alias: &NumberAlias,
) -> Result<Option<Number>, RepoError> {
    let row: Option<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM number WHERE organization_id = $1 AND alias = $2 \
           AND removed_at IS NULL"
    ))
    .bind(organization.id())
    .bind(alias.as_str())
    .fetch_optional(pool)
    .await?;
    row.map(Number::try_from).transpose()
}

/// The number of the session `session` of the engine `engine`, when it was not removed.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn by_session(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    engine: Uuid,
    session: &SessionName,
) -> Result<Option<Number>, RepoError> {
    let row: Option<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM number WHERE organization_id = $1 AND engine_id = $2 \
           AND session_name = $3 AND removed_at IS NULL"
    ))
    .bind(organization.id())
    .bind(engine)
    .bind(session.as_str())
    .fetch_optional(connection)
    .await?;
    row.map(Number::try_from).transpose()
}

/// What a change of state asked by the admin did.
#[derive(Debug, Clone, PartialEq, Eq)]
// One value at a time, moved once to whoever asked: boxing the number buys nothing.
#[allow(clippy::large_enum_variant)]
pub enum ChangeOutcome {
    /// The number went from the first state to the second.
    Changed(Number, NumberState),
    /// There is no number with that alias.
    NotFound,
    /// The change does not apply to the state the number is in.
    NotNow(NumberState),
    /// The telephone that paired is already another number of the organization.
    PhoneTaken,
}

/// Applies `trigger`, asked by the admin, to the number with this alias, under the lock of
/// its row, with its event. `until` is the end of the pairing, for [`Trigger::Pair`].
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn change(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    alias: &NumberAlias,
    trigger: Trigger,
    until: Option<i64>,
    now: i64,
) -> Result<ChangeOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let row: Option<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM number WHERE organization_id = $1 AND alias = $2 \
           AND removed_at IS NULL FOR UPDATE"
    ))
    .bind(organization.id())
    .bind(alias.as_str())
    .fetch_optional(write.connection())
    .await?;
    let Some(row) = row else {
        write.rollback().await?;
        return Ok(ChangeOutcome::NotFound);
    };
    let number = Number::try_from(row)?;
    let Some(to) = next(number.state, trigger) else {
        write.rollback().await?;
        return Ok(ChangeOutcome::NotNow(number.state));
    };
    let pairing_until = (to == NumberState::Pairing).then(|| until.unwrap_or(now));
    // Confirming a swapped number takes the telephone that paired as its telephone.
    if trigger == Trigger::Confirm {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM number o, number n                             WHERE n.organization_id = $1 AND n.id = $2                               AND o.organization_id = $1 AND o.id <> n.id                               AND o.removed_at IS NULL AND o.phone_digest = n.paired_phone_digest)",
        )
        .bind(organization.id())
        .bind(number.id)
        .fetch_one(write.connection())
        .await?;
        if taken {
            write.rollback().await?;
            return Ok(ChangeOutcome::PhoneTaken);
        }
        sqlx::query(
            "UPDATE number SET phone_enc = paired_phone_enc, phone_digest = paired_phone_digest              WHERE organization_id = $1 AND id = $2",
        )
        .bind(organization.id())
        .bind(number.id)
        .execute(write.connection())
        .await?;
    }
    let row: Row = sqlx::query_as(concat!(
        "UPDATE number SET state = $3, state_since = $4, pairing_until = $5, \
                paired_phone_digest = NULL, paired_phone_enc = NULL, \
                paired_at = CASE WHEN $3 = 'connected' THEN $4 ELSE paired_at END, \
                removed_at = CASE WHEN $3 = 'removed' THEN $4 END \
         WHERE organization_id = $1 AND id = $2 RETURNING ",
        columns!(),
        ""
    ))
    .bind(organization.id())
    .bind(number.id)
    .bind(to.as_str())
    .bind(now)
    .bind(pairing_until)
    .fetch_one(write.connection())
    .await?;
    // A number removed takes every access to it along (`docs/08-permissoes.md`): each Hermes
    // that had one is told, and gets its new list of tools.
    if to == NumberState::Removed {
        let ended = super::access::end_all(
            &mut write,
            organization,
            &admin.actor(),
            super::access::Of::Number(number.id),
            now,
        )
        .await?;
        for hermes in ended {
            notify_hub(write.connection(), organization, "hermes", hermes).await?;
        }
    }
    let action = match trigger {
        Trigger::Remove => Action::NumberRemoved,
        Trigger::Confirm => Action::NumberConfirmed,
        Trigger::Pair => Action::NumberPairingStarted,
        _ => Action::NumberChanged,
    };
    write
        .record(
            now,
            &Event::new(action, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Number, number.id)
                .with(
                    Details::new()
                        .alias("alias", &number.alias)
                        .word("from", number.state.as_str())
                        .word("to", to.as_str()),
                ),
        )
        .await?;
    notify_hub(write.connection(), organization, "number", number.id).await?;
    write.commit().await?;
    Ok(ChangeOutcome::Changed(Number::try_from(row)?, number.state))
}

/// What the engine said of a session, as the hub applies it.
#[derive(Debug, Clone)]
pub struct Observed {
    /// The status.
    pub status: EngineStatus,
    /// The telephone of the connected account, when the engine named one.
    pub phone: Option<Phone>,
    /// When the engine says it was, in milliseconds of its clock; `None` when the hub read
    /// it now, by a query.
    pub at: Option<i64>,
}

/// Applies what the engine said of the session of `number`, under the lock of its row: the
/// rule of [`crate::core::number::next`]. A status older than the last one applied is
/// ignored (events may come out of order). A pairing that connects another telephone raises
/// an alert. Returns the change, if any.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn observe(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    observed: &Observed,
    now: i64,
) -> Result<Option<(NumberState, NumberState)>, RepoError> {
    let row: Option<(String, Option<i64>, Vec<u8>)> = sqlx::query_as(
        "SELECT state, status_at, phone_enc FROM number \
         WHERE organization_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(organization.id())
    .bind(number)
    .fetch_optional(&mut *connection)
    .await?;
    let Some((state, status_at, phone_enc)) = row else {
        return Ok(None);
    };
    if let (Some(at), Some(last)) = (observed.at, status_at)
        && at < last
    {
        return Ok(None);
    }
    let state =
        NumberState::parse(&state).ok_or(RepoError::Inconsistent("the state of a number"))?;
    let expected = key
        .open_whatsapp(WhatsappField::Phone, number, &phone_enc)
        .ok()
        .and_then(|opened| {
            std::str::from_utf8(&opened)
                .ok()
                .and_then(|text| Phone::parse(text).ok())
        })
        .ok_or(RepoError::Inconsistent(
            "the telephone of a number does not open",
        ))?;
    let expected_phone = observed.phone.as_ref().map(|phone| expected.same_as(phone));
    let trigger = Trigger::Status {
        status: observed.status,
        expected_phone,
    };
    if let Some(at) = observed.at {
        sqlx::query("UPDATE number SET status_at = $3 WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(number)
            .bind(at)
            .execute(&mut *connection)
            .await?;
    }
    let Some(to) = next(state, trigger) else {
        return Ok(None);
    };
    let digits = observed.phone.as_ref().map_or("", Phone::digits);
    let swapped = (to == NumberState::NumberSwapped)
        .then(|| key.whatsapp_digest(WhatsappField::Phone, organization.id(), digits.as_bytes()));
    let swapped_enc = match swapped {
        Some(_) => Some(key.seal_whatsapp(WhatsappField::Phone, number, digits.as_bytes())?),
        None => None,
    };
    sqlx::query(
        "UPDATE number SET state = $3, state_since = $4, pairing_until = NULL, \
                paired_phone_digest = $5, paired_phone_enc = $7, \
                paired_at = CASE WHEN $3 = 'connected' AND state = 'pairing' THEN $4 ELSE paired_at END, \
                last_message_at = CASE WHEN $3 = 'connected' AND last_message_at IS NULL \
                                       THEN $6 ELSE last_message_at END \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(number)
    .bind(to.as_str())
    .bind(now)
    .bind(swapped.as_ref().map(<[u8; 32]>::as_slice))
    .bind(observed.at.unwrap_or(now))
    .bind(swapped_enc)
    .execute(&mut *connection)
    .await?;
    if to == NumberState::NumberSwapped {
        raise(
            connection,
            organization,
            now,
            &NewAlert {
                kind: AlertKind::NumberSwapped,
                hermes: None,
                device: None,
                details: AlertDetails::new().id("number", number),
            },
        )
        .await?;
    }
    Ok(Some((state, to)))
}

/// Ends the pairings whose QR or code expired unused: back to unpaired. Returns the numbers.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn expire_pairings(
    pool: &PgPool,
    organization: ResolvedOrg,
    now: i64,
) -> Result<Vec<Uuid>, RepoError> {
    Ok(sqlx::query_scalar(
        "UPDATE number SET state = 'unpaired', state_since = $2, pairing_until = NULL \
         WHERE organization_id = $1 AND state = 'pairing' AND pairing_until < $2 \
         RETURNING id",
    )
    .bind(organization.id())
    .bind(now)
    .fetch_all(pool)
    .await?)
}

/// Records that the configuration `digest` was applied to the session of `number`.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn config_applied(
    pool: &PgPool,
    organization: ResolvedOrg,
    number: Uuid,
    digest: &[u8; 32],
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE number SET config_digest = $3, config_applied_at = $4 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(number)
    .bind(digest.as_slice())
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Moves forward the cursor of the catch-up of `number`: up to `until`, in milliseconds of
/// the hub, a whole round read everything. It never goes back.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn caught_up(
    pool: &PgPool,
    organization: ResolvedOrg,
    number: Uuid,
    until: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE number SET caught_up_until = GREATEST(COALESCE(caught_up_until, $3), $3) \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(number)
    .bind(until)
    .execute(pool)
    .await?;
    Ok(())
}

/// Records that a message of the time `at_ms` was recorded for the number: what the screens
/// show as its last message. It never goes back.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn advance_messages(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    number: Uuid,
    at_ms: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE number SET last_message_at = GREATEST(COALESCE(last_message_at, $3), $3) \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(number)
    .bind(at_ms)
    .execute(connection)
    .await?;
    Ok(())
}
