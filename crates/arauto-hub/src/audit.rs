//! The audit: an append-only record of what changes who can do what.
//!
//! There are two ways of writing an event:
//!
//! - [`record`], for an ordinary event: one row, in any transaction or straight on the pool.
//! - [`begin_security`], for a **security event** (`docs/15-mvp.md`, section 2, rule 9). It
//!   reads the audit mark, opens a durable transaction and checks, inside it, that the last
//!   security event of the database is not behind the mark. A database that is behind came
//!   back from an older copy: nothing is written and the caller gets
//!   [`AuditError::Restored`]. Otherwise the caller makes its change and records its events
//!   in that same transaction, and [`SecurityWrite::commit`] commits and then advances the
//!   mark.
//!
//! The catalog of actions is closed ([`Action`]), and so is what an event can carry: numbers,
//! flags, identifiers, names in the restricted format and words fixed in the program
//! ([`Details`]). The only text made at run time is the system user of the local admin, a
//! declared text of one short line. There is no field for anything else, so the content of a
//! message, an invite, a credential, a code or a key cannot end up here.

use std::fmt;
use std::net::IpAddr;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::DeclaredText;
use arauto_protocol::names::{HermesName, NumberAlias};
use sqlx::postgres::{PgConnection, PgPool};
use sqlx::types::Json;
use sqlx::{PgExecutor, Postgres, Transaction};
use uuid::Uuid;

use crate::db::{self, DbError, ResolvedOrg};
use crate::mark::{Mark, MarkError};
use crate::restore::{self, Evidence, Standing};

macro_rules! actions {
    ($( $(#[$meta:meta])* $variant:ident => $text:literal, $security:literal; )+) => {
        /// What happened. The catalog is closed: an event is one of these.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Action {
            $( $(#[$meta])* $variant, )+
        }

        impl Action {
            /// Every action, in the order of the catalog.
            pub const ALL: &'static [Action] = &[$(Action::$variant,)+];

            /// The text stored in `audit.action`.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Action::$variant => $text,)+
                }
            }

            /// Whether this is a security event: one that changes who can do what, and that
            /// the mark outside the database follows (C11).
            #[must_use]
            pub const fn is_security(self) -> bool {
                match self {
                    $(Action::$variant => $security,)+
                }
            }
        }
    };
}

actions! {
    /// `init` created the installation.
    InstallationCreated => "installation.created", true;
    /// An organization was created.
    OrganizationCreated => "organization.created", true;
    /// A key pair of the hub was created.
    HubKeyCreated => "hub_key.created", true;
    /// An admin was added.
    AdminCreated => "admin.created", true;
    /// An admin was removed.
    AdminRemoved => "admin.removed", true;
    /// A Hermes line was created.
    HermesCreated => "hermes.created", true;
    /// A Hermes line was removed.
    HermesRemoved => "hermes.removed", true;
    /// Whether a Hermes can be woken changed.
    HermesWakeChanged => "hermes.wake_changed", true;
    /// An invite was issued.
    InviteIssued => "invite.issued", false;
    /// An invite was used by its first valid enrollment.
    InviteUsed => "invite.used", true;
    /// An invite was cancelled.
    InviteCancelled => "invite.cancelled", true;
    /// An invite expired unused.
    InviteExpired => "invite.expired", false;
    /// Invites that do not hold were presented: one row per origin and per minute.
    InviteRefused => "invite.refused", false;
    /// A used invite was presented by another identity.
    InviteMisused => "invite.misused", false;
    /// The same request enrolled again.
    DeviceEnrolledAgain => "device.enrolled_again", false;
    /// A wrong verification code was typed.
    DeviceCodeRejected => "device.code_rejected", false;
    /// A device was approved.
    DeviceApproved => "device.approved", true;
    /// A request was denied.
    DeviceDenied => "device.denied", true;
    /// A deadline of the device passed.
    DeviceExpired => "device.expired", false;
    /// A device confirmed its first credential and became active.
    DeviceActivated => "device.activated", true;
    /// A device was put in quarantine.
    DeviceQuarantined => "device.quarantined", true;
    /// A device was replaced by a new one of the same Hermes.
    DeviceReplaced => "device.replaced", true;
    /// A device was revoked.
    DeviceRevoked => "device.revoked", true;
    /// A credential swap was confirmed.
    CredentialConfirmed => "credential.confirmed", true;
    /// A connection received a credential and ended without the "saved".
    CredentialUnconfirmed => "credential.unconfirmed", false;
    /// A closed device presented a seal older than its last ones.
    CredentialReuseAfterClosing => "credential.reuse_after_closing", false;
    /// A seal the database does not explain, from a device that no longer counts as proof.
    CredentialUnexplained => "credential.unexplained", false;
    /// A session that was being served was taken by another connection.
    ConnectionTaken => "connection.taken", false;
    /// A link between two Hermes was created.
    PeerLinkCreated => "peer_link.created", true;
    /// A link between two Hermes changed.
    PeerLinkChanged => "peer_link.changed", true;
    /// A link between two Hermes was removed.
    PeerLinkRemoved => "peer_link.removed", true;
    /// A message ceiling was reached: one row per link and per hour.
    PeerCeilingReached => "peer.ceiling_reached", false;
    /// The hub noticed it came back from an older copy.
    RestoredModeEntered => "restored_mode.entered", true;
    /// No suspended device is left: the restored mode ended.
    RestoredModeEnded => "restored_mode.ended", true;
    /// A device the database does not know presented a valid seal and was listed.
    UnknownDeviceListed => "unknown_device.listed", false;
    /// Migrations were applied to a database with data.
    DatabaseMigrated => "database.migrated", false;
    /// The engine was created, or its address changed: where every number is reached.
    EngineConfigured => "engine.configured", true;
    /// The key of the API of the engine was set or changed.
    EngineKeyChanged => "engine.key_changed", true;
    /// A new secret of the events of the engine was drawn.
    EngineSecretChanged => "engine.secret_changed", true;
    /// The hub found a session of the engine configured otherwise and applied its own.
    EngineSessionCorrected => "engine.session_corrected", false;
    /// A number was added.
    NumberAdded => "number.added", true;
    /// A number was removed: every access to it ends.
    NumberRemoved => "number.removed", true;
    /// The admin confirmed the new telephone of a swapped number: its accesses work again.
    NumberConfirmed => "number.confirmed", true;
    /// The admin asked for a QR or a code to pair a number.
    NumberPairingStarted => "number.pairing_started", false;
    /// The admin disconnected, restarted or let go the pairing of a number.
    NumberChanged => "number.changed", false;
    /// A Hermes got an access to a number.
    AccessGranted => "access.granted", true;
    /// The scope or the verbs of an access changed.
    AccessChanged => "access.changed", true;
    /// An access ended: by the admin, or with its Hermes or its number.
    AccessRemoved => "access.removed", true;
    /// The line of a chat was set or changed.
    AccessChatSet => "access_chat.set", true;
    /// The line of a chat was taken away.
    AccessChatRemoved => "access_chat.removed", true;
    /// An entry code of the panel was issued by the admin command. It opens nothing until it
    /// is typed.
    PanelCodeIssued => "panel.code_issued", false;
    /// An entry code was used: a session of the panel is open.
    PanelSignedIn => "panel.signed_in", true;
    /// A session of the panel was closed by whoever held it.
    PanelSignedOut => "panel.signed_out", false;
    /// A passkey was registered for an admin, in a session of the panel.
    PanelPasskeyRegistered => "panel.passkey_registered", true;
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who did it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// The local admin: a person running the admin command on the machine of the hub.
    Admin {
        /// The row of the admin.
        id: Uuid,
        /// The system user that ran the command. Declared, not verified.
        os_user: DeclaredText,
    },
    /// A device, through its channel.
    Device {
        /// The row of the device.
        id: Uuid,
    },
    /// The hub itself: a deadline, the restored mode, the purge.
    System,
}

/// The kind of thing an event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// An organization.
    Organization,
    /// An admin.
    Admin,
    /// A key pair of the hub.
    HubKey,
    /// A Hermes line.
    Hermes,
    /// A device.
    Device,
    /// A device the database does not know.
    UnknownDevice,
    /// An invite.
    Invite,
    /// The engine of WhatsApp.
    Engine,
    /// A number of WhatsApp.
    Number,
    /// An access of a Hermes to a number.
    Access,
}

impl TargetKind {
    /// The text stored in `audit.target_kind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            TargetKind::Organization => "organization",
            TargetKind::Admin => "admin",
            TargetKind::HubKey => "hub_key",
            TargetKind::Hermes => "hermes",
            TargetKind::Device => "device",
            TargetKind::UnknownDevice => "unknown_device",
            TargetKind::Invite => "invite",
            TargetKind::Engine => "engine",
            TargetKind::Number => "number",
            TargetKind::Access => "access",
        }
    }
}

/// How it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// It was done.
    Ok,
    /// It was refused.
    Denied,
    /// It failed.
    Error,
}

impl Outcome {
    /// The text stored in `audit.result`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Denied => "denied",
            Outcome::Error => "error",
        }
    }
}

/// What else an event says. Every value is a number, a flag, an identifier, a name in the
/// restricted format or a word fixed in the program; every key is fixed in the program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Details(serde_json::Map<String, serde_json::Value>);

impl Details {
    /// No details.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A number: a count, a generation, a time in milliseconds.
    #[must_use]
    pub fn number(mut self, key: &'static str, value: i64) -> Self {
        self.0.insert(key.to_owned(), value.into());
        self
    }

    /// A flag.
    #[must_use]
    pub fn flag(mut self, key: &'static str, value: bool) -> Self {
        self.0.insert(key.to_owned(), value.into());
        self
    }

    /// The identifier of a row.
    #[must_use]
    pub fn id(mut self, key: &'static str, value: Uuid) -> Self {
        self.0
            .insert(key.to_owned(), value.as_hyphenated().to_string().into());
        self
    }

    /// The name of a Hermes.
    #[must_use]
    pub fn name(mut self, key: &'static str, value: &HermesName) -> Self {
        self.0.insert(key.to_owned(), value.as_str().into());
        self
    }

    /// The alias of a number, in the restricted format.
    #[must_use]
    pub fn alias(mut self, key: &'static str, value: &NumberAlias) -> Self {
        self.0.insert(key.to_owned(), value.as_str().into());
        self
    }

    /// A list of words fixed in the program, like the verbs of an access.
    #[must_use]
    pub fn words(mut self, key: &'static str, values: &[&'static str]) -> Self {
        self.0.insert(
            key.to_owned(),
            serde_json::Value::Array(values.iter().map(|value| (*value).into()).collect()),
        );
        self
    }

    /// A word fixed in the program: a reason, a kind. Never a text made at run time.
    #[must_use]
    pub fn word(mut self, key: &'static str, value: &'static str) -> Self {
        self.0.insert(key.to_owned(), value.into());
        self
    }

    /// The details as the JSON object that is stored.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Object(self.0.clone())
    }
}

/// One event of the audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The organization it belongs to. `None` for what is of the whole installation.
    pub organization: Option<ResolvedOrg>,
    /// Who did it.
    pub actor: Actor,
    /// What happened.
    pub action: Action,
    /// What it is about.
    pub target: Option<(TargetKind, Uuid)>,
    /// The address it came from.
    pub address: Option<IpAddr>,
    /// How it ended.
    pub outcome: Outcome,
    /// The code that was answered, when one was.
    pub code: Option<ErrorCode>,
    /// What else it says.
    pub details: Details,
}

impl Event {
    /// An event that was done, by `actor`, with nothing else said yet.
    #[must_use]
    pub fn new(action: Action, actor: Actor) -> Self {
        Self {
            organization: None,
            actor,
            action,
            target: None,
            address: None,
            outcome: Outcome::Ok,
            code: None,
            details: Details::new(),
        }
    }

    /// The organization it belongs to.
    #[must_use]
    pub fn in_organization(mut self, organization: ResolvedOrg) -> Self {
        self.organization = Some(organization);
        self
    }

    /// What it is about.
    #[must_use]
    pub fn about(mut self, kind: TargetKind, id: Uuid) -> Self {
        self.target = Some((kind, id));
        self
    }

    /// The address it came from.
    #[must_use]
    pub fn from_address(mut self, address: IpAddr) -> Self {
        self.address = Some(address);
        self
    }

    /// It was refused or failed, with this code.
    #[must_use]
    pub fn ended(mut self, outcome: Outcome, code: Option<ErrorCode>) -> Self {
        self.outcome = outcome;
        self.code = code;
        self
    }

    /// What else it says.
    #[must_use]
    pub fn with(mut self, details: Details) -> Self {
        self.details = details;
        self
    }
}

/// What went wrong while writing the audit.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// The database refused.
    #[error(transparent)]
    Database(#[from] DbError),

    /// The mark could not be read.
    #[error(transparent)]
    Mark(#[from] MarkError),

    /// The database came back from an older copy. **Nothing was written.** The caller turns
    /// the restored mode on ([`restore::enter`]).
    #[error("the database came back from an older copy ({0}); nothing was written")]
    Restored(Evidence),

    /// There is neither a mark nor data: `init` has not run.
    #[error("the hub is not installed: run `arauto-hub init`")]
    NotInstalled,

    /// A security event must be written through [`begin_security`].
    #[error("`{0}` is a security event and is only written by a security write")]
    NeedsSecurityWrite(Action),

    /// The event **is committed**, and the mark could not follow it. The mark is one event
    /// behind until the next security event; that never reads as a restored database.
    #[error("security event {seq} is committed, but the audit mark did not advance: {cause}")]
    MarkNotAdvanced {
        /// The number of the last security event that was committed.
        seq: i64,
        /// Why the mark was not written.
        cause: MarkError,
    },
}

impl From<sqlx::Error> for AuditError {
    fn from(error: sqlx::Error) -> Self {
        AuditError::Database(DbError::Query(error))
    }
}

/// Writes an ordinary event and returns its number.
///
/// # Errors
/// [`AuditError::NeedsSecurityWrite`] when the action is a security event;
/// [`AuditError::Database`] when the database refuses.
pub async fn record<'e>(
    executor: impl PgExecutor<'e>,
    at: i64,
    event: &Event,
) -> Result<i64, AuditError> {
    if event.action.is_security() {
        return Err(AuditError::NeedsSecurityWrite(event.action));
    }
    Ok(insert(executor, at, event).await?)
}

/// The one INSERT of the audit. The number comes from the only sequence of the schema.
pub(crate) async fn insert<'e>(
    executor: impl PgExecutor<'e>,
    at: i64,
    event: &Event,
) -> Result<i64, sqlx::Error> {
    let (actor_kind, actor_id, actor_note) = match &event.actor {
        Actor::Admin { id, os_user } => ("admin", Some(*id), Some(os_user.as_str())),
        Actor::Device { id } => ("device", Some(*id), None),
        Actor::System => ("system", None, None),
    };
    sqlx::query_scalar(
        "INSERT INTO audit (organization_id, at, actor_kind, actor_id, actor_note, action, \
                            target_kind, target_id, address, result, code, security, details) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
         RETURNING seq",
    )
    .bind(event.organization.map(|organization| organization.id()))
    .bind(at)
    .bind(actor_kind)
    .bind(actor_id)
    .bind(actor_note)
    .bind(event.action.as_str())
    .bind(event.target.map(|(kind, _)| kind.as_str()))
    .bind(event.target.map(|(_, id)| id))
    .bind(event.address.map(|address| address.to_string()))
    .bind(event.outcome.as_str())
    .bind(event.code.map(ErrorCode::as_str))
    .bind(event.action.is_security())
    .bind(Json(event.details.to_json()))
    .fetch_one(executor)
    .await
}

/// A durable transaction in which the database was checked against the audit mark. The
/// change and its events go in together; dropping it without [`SecurityWrite::commit`]
/// undoes everything.
#[derive(Debug)]
pub struct SecurityWrite {
    transaction: Transaction<'static, Postgres>,
    mark: Mark,
    last_security_seq: Option<i64>,
}

/// Begins the write of a security event.
///
/// The transaction commits with `synchronous_commit = on` ([`db::begin_durable`]).
///
/// # Errors
/// [`AuditError::Restored`] when the last security event of the database is behind the mark,
/// or the database has data and there is no mark: nothing is written, and the evidence says
/// which. [`AuditError::NotInstalled`] when there is neither mark nor data.
pub async fn begin_security(pool: &PgPool, mark: &Mark) -> Result<SecurityWrite, AuditError> {
    let marked = mark.read().await?;
    let mut transaction = db::begin_durable(pool).await?;
    match restore::standing(&mut transaction, marked).await? {
        Standing::Consistent => Ok(SecurityWrite {
            transaction,
            mark: mark.clone(),
            last_security_seq: None,
        }),
        Standing::Restored(evidence) => Err(AuditError::Restored(evidence)),
        Standing::NotInstalled => Err(AuditError::NotInstalled),
    }
}

impl SecurityWrite {
    /// The connection of the transaction, for the change the events describe.
    pub fn connection(&mut self) -> &mut PgConnection {
        &mut self.transaction
    }

    /// Writes an event in this transaction, security or ordinary, and returns its number.
    ///
    /// # Errors
    /// [`AuditError::Database`] when the database refuses.
    pub async fn record(&mut self, at: i64, event: &Event) -> Result<i64, AuditError> {
        let seq = insert(&mut *self.transaction, at, event).await?;
        if event.action.is_security() {
            self.last_security_seq = Some(self.last_security_seq.map_or(seq, |last| last.max(seq)));
        }
        Ok(seq)
    }

    /// Commits, durably, and then advances the mark to the last security event written here.
    /// Returns that number, if any event was a security event.
    ///
    /// # Errors
    /// [`AuditError::Database`] when the commit fails: nothing was written.
    /// [`AuditError::MarkNotAdvanced`] when the commit went through and the mark did not
    /// follow: the change is done.
    pub async fn commit(self) -> Result<Option<i64>, AuditError> {
        self.transaction.commit().await?;
        if let Some(seq) = self.last_security_seq {
            self.mark
                .advance(seq)
                .await
                .map_err(|cause| AuditError::MarkNotAdvanced { seq, cause })?;
        }
        Ok(self.last_security_seq)
    }

    /// Gives the change up.
    ///
    /// # Errors
    /// [`AuditError::Database`] when the database refuses; the transaction is undone anyway
    /// when the connection goes back to the pool.
    pub async fn rollback(self) -> Result<(), AuditError> {
        Ok(self.transaction.rollback().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_security_events_are_the_ones_that_change_who_can_do_what() {
        let security: BTreeSet<&str> = Action::ALL
            .iter()
            .filter(|action| action.is_security())
            .map(|action| action.as_str())
            .collect();
        // C11: used or cancelled invite, approval and denial, confirmed swap, quarantine,
        // reconnection, revocation, change of access and change of admin; and what creates
        // or ends who can ask for any of that.
        let expected: BTreeSet<&str> = [
            "installation.created",
            "organization.created",
            "hub_key.created",
            "admin.created",
            "admin.removed",
            "hermes.created",
            "hermes.removed",
            "hermes.wake_changed",
            "invite.used",
            "invite.cancelled",
            "device.approved",
            "device.denied",
            "device.activated",
            "device.quarantined",
            "device.replaced",
            "device.revoked",
            "credential.confirmed",
            "peer_link.created",
            "peer_link.changed",
            "peer_link.removed",
            "restored_mode.entered",
            "restored_mode.ended",
            // Where every number is reached and who proves itself to whom; a number that
            // comes or goes, or whose swapped telephone the admin accepts.
            "engine.configured",
            "engine.key_changed",
            "engine.secret_changed",
            "number.added",
            "number.removed",
            "number.confirmed",
            // The matrix of accesses is who can do what on each number (C11: change of access).
            "access.granted",
            "access.changed",
            "access.removed",
            "access_chat.set",
            "access_chat.removed",
            // A session of the panel is someone who can ask for all of the above.
            "panel.signed_in",
            // A passkey is what opens such a session from then on.
            "panel.passkey_registered",
        ]
        .into_iter()
        .collect();
        assert_eq!(security, expected);
    }

    #[test]
    fn every_action_has_its_own_plain_text() {
        let texts: BTreeSet<&str> = Action::ALL.iter().map(|action| action.as_str()).collect();
        assert_eq!(texts.len(), Action::ALL.len());
        for text in texts {
            assert!(
                text.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_' || byte == b'.'),
                "{text}"
            );
            assert_eq!(text.matches('.').count(), 1, "{text}");
        }
    }

    #[test]
    fn the_stored_texts_are_the_ones_the_schema_accepts() {
        const MIGRATION: &str = include_str!("../migrations/0001_core.sql");
        assert!(MIGRATION.contains("CHECK (actor_kind IN ('admin', 'device', 'system'))"));
        assert!(MIGRATION.contains("CHECK (result IN ('ok', 'denied', 'error'))"));
        assert_eq!(Outcome::Ok.as_str(), "ok");
        assert_eq!(Outcome::Denied.as_str(), "denied");
        assert_eq!(Outcome::Error.as_str(), "error");
    }

    #[test]
    fn details_hold_only_what_was_given() {
        let hermes = HermesName::parse("vendas-2").unwrap();
        let id = Uuid::from_u128(0x1234);
        let details = Details::new()
            .number("generation", 7)
            .flag("recognized", false)
            .id("replaced", id)
            .name("hermes", &hermes)
            .word("reason", "reuse");
        assert_eq!(
            details.to_json(),
            serde_json::json!({
                "generation": 7,
                "recognized": false,
                "replaced": "00000000-0000-0000-0000-000000001234",
                "hermes": "vendas-2",
                "reason": "reuse",
            })
        );
        assert_eq!(Details::new().to_json(), serde_json::json!({}));
    }

    #[test]
    fn an_event_is_built_step_by_step() {
        let device = Uuid::from_u128(7);
        let event = Event::new(Action::DeviceQuarantined, Actor::System)
            .about(TargetKind::Device, device)
            .from_address("192.0.2.9".parse().unwrap())
            .ended(Outcome::Denied, Some(ErrorCode::Ar501))
            .with(Details::new().word("reason", "reuse"));
        assert_eq!(event.target, Some((TargetKind::Device, device)));
        assert_eq!(event.outcome, Outcome::Denied);
        assert_eq!(event.code, Some(ErrorCode::Ar501));
        assert_eq!(event.organization, None);
        assert!(event.action.is_security());
    }
}
