//! The rows of the connection tables, and what each device state answers.
//!
//! The states and their transitions are in `docs/10-maquinas-de-estado.md`, sections 1 and 2;
//! the columns, in the migration `0001_core.sql`. A row read from the database is checked on
//! the way in: a state or a reason this program does not know is an error, never a guess.

use std::fmt;

use arauto_protocol::code::VerificationCode;
use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::InviteDigest;
use arauto_protocol::limits::DIGEST_LEN;
use arauto_protocol::message::{ChannelState, Declared, DeclaredText, Generation};
use arauto_protocol::names::HermesName;
use arauto_protocol::noise::PublicKey;
use sqlx::Row;
use sqlx::postgres::PgRow;
use uuid::Uuid;

use super::judge::{Credential, DeviceSnapshot};

/// Declares an enum that is stored as a fixed text.
macro_rules! stored_text {
    (
        $(#[$meta:meta])*
        $name:ident, $what:literal {
            $( $(#[$variant_meta:meta])* $variant:ident => $text:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
        }

        impl $name {
            /// Every value, in the order of the declaration.
            pub const ALL: &'static [$name] = &[$($name::$variant,)+];

            /// The text stored in the database.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text,)+
                }
            }

            /// Reads the stored text. `None` for a text this program does not know.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl StoredText for $name {
            const WHAT: &'static str = $what;

            fn parse_stored(text: &str) -> Option<Self> {
                Self::parse(text)
            }
        }
    };
}

/// An enum stored as text, for the reading of rows.
trait StoredText: Sized {
    const WHAT: &'static str;
    fn parse_stored(text: &str) -> Option<Self>;
}

stored_text!(
    /// The state of a device (`docs/10-maquinas-de-estado.md`, section 2).
    DeviceState, "device state" {
        /// Enrolled, waiting for the admin to type the verification code.
        Pending => "pending",
        /// Approved, at generation zero: no credential confirmed yet.
        Approved => "approved",
        /// In use: the first credential was confirmed.
        Active => "active",
        /// The hub stopped trusting its credential. Only reconnecting or revoking gets out.
        Quarantined => "quarantined",
        /// The hub came back from a backup. Only reconnecting or revoking gets out.
        Suspended => "suspended",
        /// The request was denied. Final.
        Denied => "denied",
        /// A deadline passed, or the restored mode closed the request. Final.
        Expired => "expired",
        /// Another device of the same Hermes took its place. Final.
        Replaced => "replaced",
        /// Closed by the admin or at its own request. Final.
        Revoked => "revoked",
    }
);

impl DeviceState {
    /// Denied, expired, replaced and revoked: the row stays as a record and never changes
    /// state again.
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(
            self,
            DeviceState::Denied
                | DeviceState::Expired
                | DeviceState::Replaced
                | DeviceState::Revoked
        )
    }

    /// Approved or active: the only states in which a connection swaps the credential.
    #[must_use]
    pub const fn serves(self) -> bool {
        matches!(self, DeviceState::Approved | DeviceState::Active)
    }
}

stored_text!(
    /// Why a device is in its state. The catalog is closed; the text goes to `state_reason`.
    StateReason, "state reason" {
        /// Quarantine: a credential already swapped was presented, or none when one was due.
        Reuse => "reuse",
        /// Quarantine: a seal that does not open.
        UnreadableCredential => "unreadable_credential",
        /// Quarantine: a presentation that would be accepted while another connection of
        /// the device answered.
        DoubleConnection => "double_connection",
        /// Quarantine: the admin did not recognize the last use when approving a reconnect.
        NotRecognized => "not_recognized",
        /// Denied: five wrong codes.
        CodeAttempts => "code_attempts",
        /// Denied: the sixth enrollment of the same request.
        Enrollments => "enrollments",
        /// Denied: two waiting connections with the same identity.
        DoubleWaiting => "double_waiting",
        /// Denied or revoked: the admin did it.
        Admin => "admin",
        /// Denied or revoked: a new invite for the same Hermes closed this request.
        NewInvite => "new_invite",
        /// Denied or revoked: the device itself asked to leave.
        Left => "left",
        /// Expired: nobody typed the code within 10 minutes.
        PendingTimeout => "pending_timeout",
        /// Expired: approved, and the first credential was not confirmed within 24 hours.
        ApprovedTimeout => "approved_timeout",
        /// Expired: 30 days without connecting.
        Inactivity => "inactivity",
        /// Expired: the restored mode closed a pending request.
        RestoredPending => "restored_pending",
        /// Expired: the restored mode closed an approved request.
        RestoredApproved => "restored_approved",
        /// Suspended: the hub came back from a backup.
        Restored => "restored",
        /// Replaced: a new device of the same Hermes confirmed its first credential.
        Reconnected => "reconnected",
    }
);

impl StateReason {
    /// The request was closed by the restored mode: with a valid seal, such a device is
    /// treated as one the database does not know (C11).
    #[must_use]
    pub const fn closed_by_restoration(self) -> bool {
        matches!(
            self,
            StateReason::RestoredPending | StateReason::RestoredApproved
        )
    }
}

stored_text!(
    /// Which of the two places of a Hermes a device takes.
    Slot, "device slot" {
        /// The device in use: it confirmed a credential (active, suspended or quarantined).
        InUse => "in_use",
        /// The request in progress: it has not confirmed one yet.
        Request => "request",
    }
);

stored_text!(
    /// The state of an invite (`docs/10-maquinas-de-estado.md`, section 1).
    InviteState, "invite state" {
        /// Issued, within its 15 minutes.
        Issued => "issued",
        /// Used by the first valid enrollment.
        Used => "used",
        /// The 15 minutes passed. The digest was erased.
        Expired => "expired",
        /// Cancelled by the admin, by a newer invite, by revoking, or by the restored mode.
        Cancelled => "cancelled",
    }
);

stored_text!(
    /// How the connector of a Hermes runs.
    HermesMode, "hermes mode" {
        /// With the user of the Hermes.
        Fast => "fast",
        /// Apart from the LLM. Not in the MVP.
        Shielded => "shielded",
    }
);

/// The code a connection gets when its device is in `state` for `reason`, or `None` while
/// the device is still on its way (pending, approved or active).
///
/// | State | Code |
/// |---|---|
/// | denied | AR-301 |
/// | expired while pending | AR-302 |
/// | expired while approved | AR-303 |
/// | expired for inactivity | AR-507 |
/// | quarantined | AR-501 |
/// | revoked | AR-502 |
/// | replaced | AR-503 |
/// | suspended | AR-506 |
///
/// An expired device whose reason does not say which of the three it was gets AR-507, the
/// one that never makes a connector discard anything.
#[must_use]
pub const fn ended_code(state: DeviceState, reason: Option<StateReason>) -> Option<ErrorCode> {
    match state {
        DeviceState::Pending | DeviceState::Approved | DeviceState::Active => None,
        DeviceState::Denied => Some(ErrorCode::Ar301),
        DeviceState::Quarantined => Some(ErrorCode::Ar501),
        DeviceState::Revoked => Some(ErrorCode::Ar502),
        DeviceState::Replaced => Some(ErrorCode::Ar503),
        DeviceState::Suspended => Some(ErrorCode::Ar506),
        DeviceState::Expired => Some(match reason {
            Some(StateReason::PendingTimeout | StateReason::RestoredPending) => ErrorCode::Ar302,
            Some(StateReason::ApprovedTimeout | StateReason::RestoredApproved) => ErrorCode::Ar303,
            _ => ErrorCode::Ar507,
        }),
    }
}

/// The `state` message a channel connection gets for a device in `state` for `reason`.
#[must_use]
pub const fn channel_state(state: DeviceState, reason: Option<StateReason>) -> ChannelState {
    match (state, ended_code(state, reason)) {
        (_, Some(code)) => ChannelState::Ended(code),
        (DeviceState::Approved, None) => ChannelState::Approved,
        (DeviceState::Active, None) => ChannelState::Active,
        (_, None) => ChannelState::Waiting,
    }
}

/// A row of `hermes`: the line the admin sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hermes {
    /// The identifier.
    pub id: Uuid,
    /// The organization.
    pub organization_id: Uuid,
    /// The name: restricted, immutable and never reused.
    pub name: HermesName,
    /// How its connector runs.
    pub mode: HermesMode,
    /// Whether a notice may start a run of this Hermes.
    pub can_be_woken: bool,
    /// Why it can be woken, as the admin wrote it.
    pub wake_reason: Option<String>,
    /// When the line was created.
    pub created_at: i64,
    /// When the line was removed. The row is never erased.
    pub deleted_at: Option<i64>,
}

/// A row of `device`: one installation of the connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// The identifier.
    pub id: Uuid,
    /// The organization.
    pub organization_id: Uuid,
    /// The Hermes it belongs to.
    pub hermes_id: Uuid,
    /// The identity: the static public key of its handshakes.
    pub public_key: PublicKey,
    /// The state.
    pub state: DeviceState,
    /// Why it is in that state.
    pub state_reason: Option<StateReason>,
    /// Since when.
    pub state_since: i64,
    /// Which place of the Hermes it takes. `None` in a final state.
    pub slot: Option<Slot>,
    /// When it confirmed its first credential.
    pub first_confirmed_at: Option<i64>,
    /// The current credential. `None` at generation zero.
    pub current: Option<Credential>,
    /// The credential issued and not confirmed yet.
    pub pending: Option<Credential>,
    /// The largest generation ever issued. Kept after the device is closed.
    pub max_generation: u64,
    /// The verification code of the last enrollment, until the request is resolved.
    pub verification_code: Option<VerificationCode>,
    /// Until when the admin may type the code.
    pub confirm_until: Option<i64>,
    /// Until when an approved device may confirm its first credential.
    pub approved_until: Option<i64>,
    /// How many wrong codes were typed.
    pub code_attempts: i32,
    /// How many times this request enrolled.
    pub enrollments: i32,
    /// What the device said about itself. Not verified; whoever shows it escapes it.
    pub declared: Declared,
    /// The address the enrollment came from.
    pub enrolled_from: Option<String>,
    /// The version the connector declared on its last accepted connection.
    pub connector_version: Option<String>,
    /// The protocol version of its last accepted connection.
    pub proto: Option<i32>,
    /// When the row was created.
    pub created_at: i64,
    /// The admin who approved it.
    pub approved_by: Option<Uuid>,
    /// When it was approved.
    pub approved_at: Option<i64>,
    /// The last time a connection of it was accepted.
    pub last_contact_at: Option<i64>,
    /// The address of that connection.
    pub last_address: Option<String>,
    /// Whether it already existed when a restoration was handled (C11).
    pub marked_by_restoration: bool,
}

impl Device {
    /// The picture the swap rule takes.
    #[must_use]
    pub const fn snapshot(&self) -> DeviceSnapshot {
        DeviceSnapshot {
            state: self.state,
            current: self.current,
            pending: self.pending,
            max_generation: self.max_generation,
            marked_by_restoration: self.marked_by_restoration,
        }
    }

    /// The code a connection of this device gets, or `None` while it is on its way.
    #[must_use]
    pub const fn ended_code(&self) -> Option<ErrorCode> {
        ended_code(self.state, self.state_reason)
    }

    /// The `state` message a channel connection of this device gets.
    #[must_use]
    pub const fn channel_state(&self) -> ChannelState {
        channel_state(self.state, self.state_reason)
    }
}

/// A row of `invite`. The invite itself is never stored: only its digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /// The identifier.
    pub id: Uuid,
    /// The organization.
    pub organization_id: Uuid,
    /// The Hermes it was issued for.
    pub hermes_id: Uuid,
    /// SHA-256 of the invite. Erased when it expires unused.
    pub digest: Option<InviteDigest>,
    /// The state.
    pub state: InviteState,
    /// The admin who issued it.
    pub created_by: Option<Uuid>,
    /// When it was issued.
    pub created_at: i64,
    /// When it stops being accepted.
    pub expires_at: i64,
    /// When it was used.
    pub used_at: Option<i64>,
    /// The device it created.
    pub device_id: Option<Uuid>,
}

// ---------------------------------------------------------------------------- reading rows

/// A stored value this program does not accept.
#[derive(Debug, thiserror::Error)]
#[error("{what} is not valid")]
struct InvalidStored {
    what: &'static str,
}

fn invalid(column: &str, what: &'static str) -> sqlx::Error {
    sqlx::Error::ColumnDecode {
        index: column.to_owned(),
        source: Box::new(InvalidStored { what }),
    }
}

fn stored<T: StoredText>(row: &PgRow, column: &str) -> Result<T, sqlx::Error> {
    let text: String = row.try_get(column)?;
    T::parse_stored(&text).ok_or_else(|| invalid(column, T::WHAT))
}

fn stored_optional<T: StoredText>(row: &PgRow, column: &str) -> Result<Option<T>, sqlx::Error> {
    let text: Option<String> = row.try_get(column)?;
    text.map(|text| T::parse_stored(&text).ok_or_else(|| invalid(column, T::WHAT)))
        .transpose()
}

fn digest_bytes(
    row: &PgRow,
    column: &str,
    what: &'static str,
) -> Result<Option<[u8; DIGEST_LEN]>, sqlx::Error> {
    let bytes: Option<Vec<u8>> = row.try_get(column)?;
    bytes
        .map(|bytes| {
            <[u8; DIGEST_LEN]>::try_from(bytes.as_slice()).map_err(|_| invalid(column, what))
        })
        .transpose()
}

/// A generation column and its digest column: both empty (the generation zero or null), or
/// both filled.
fn credential(
    row: &PgRow,
    generation_column: &str,
    digest_column: &str,
) -> Result<Option<Credential>, sqlx::Error> {
    let generation: Option<i64> = row.try_get(generation_column)?;
    let digest = digest_bytes(row, digest_column, "credential digest")?;
    match (generation.unwrap_or(0), digest) {
        (0, None) => Ok(None),
        (generation, Some(digest)) => {
            let generation = u64::try_from(generation)
                .ok()
                .and_then(Generation::new)
                .ok_or_else(|| invalid(generation_column, "credential generation"))?;
            Ok(Some(Credential {
                generation,
                digest: CredentialDigest::from_bytes(digest),
            }))
        }
        (_, None) => Err(invalid(digest_column, "credential digest")),
    }
}

/// What a device declared, from the stored object. A missing or odd field reads as empty, and
/// a text that would not be accepted today is trimmed to what is.
fn declared(row: &PgRow, column: &str) -> Result<Declared, sqlx::Error> {
    let value: serde_json::Value = row.try_get(column)?;
    let field = |name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(DeclaredText::lossy)
            .unwrap_or_default()
    };
    Ok(Declared {
        hostname: field("hostname"),
        os: field("os"),
        hermes_version: field("hermes_version"),
    })
}

impl sqlx::FromRow<'_, PgRow> for Hermes {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let name: String = row.try_get("name")?;
        Ok(Self {
            id: row.try_get("id")?,
            organization_id: row.try_get("organization_id")?,
            name: HermesName::parse(&name).map_err(|_| invalid("name", "Hermes name"))?,
            mode: stored(row, "mode")?,
            can_be_woken: row.try_get("can_be_woken")?,
            wake_reason: row.try_get("wake_reason")?,
            created_at: row.try_get("created_at")?,
            deleted_at: row.try_get("deleted_at")?,
        })
    }
}

impl sqlx::FromRow<'_, PgRow> for Device {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let public_key: Vec<u8> = row.try_get("public_key")?;
        let max_generation: i64 = row.try_get("max_generation")?;
        let verification_code: Option<String> = row.try_get("verification_code")?;
        Ok(Self {
            id: row.try_get("id")?,
            organization_id: row.try_get("organization_id")?,
            hermes_id: row.try_get("hermes_id")?,
            public_key: PublicKey::from_slice(&public_key)
                .ok_or_else(|| invalid("public_key", "device identity"))?,
            state: stored(row, "state")?,
            state_reason: stored_optional(row, "state_reason")?,
            state_since: row.try_get("state_since")?,
            slot: stored_optional(row, "slot")?,
            first_confirmed_at: row.try_get("first_confirmed_at")?,
            current: credential(row, "current_generation", "current_digest")?,
            pending: credential(row, "pending_generation", "pending_digest")?,
            max_generation: u64::try_from(max_generation)
                .map_err(|_| invalid("max_generation", "largest generation"))?,
            verification_code: verification_code
                .map(|code| {
                    VerificationCode::parse(&code)
                        .ok_or_else(|| invalid("verification_code", "verification code"))
                })
                .transpose()?,
            confirm_until: row.try_get("confirm_until")?,
            approved_until: row.try_get("approved_until")?,
            code_attempts: row.try_get("code_attempts")?,
            enrollments: row.try_get("enrollments")?,
            declared: declared(row, "declared")?,
            enrolled_from: row.try_get("enrolled_from")?,
            connector_version: row.try_get("connector_version")?,
            proto: row.try_get("proto")?,
            created_at: row.try_get("created_at")?,
            approved_by: row.try_get("approved_by")?,
            approved_at: row.try_get("approved_at")?,
            last_contact_at: row.try_get("last_contact_at")?,
            last_address: row.try_get("last_address")?,
            marked_by_restoration: row.try_get("marked_by_restoration")?,
        })
    }
}

impl sqlx::FromRow<'_, PgRow> for Invite {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            organization_id: row.try_get("organization_id")?,
            hermes_id: row.try_get("hermes_id")?,
            digest: digest_bytes(row, "digest", "invite digest")?.map(InviteDigest::from_bytes),
            state: stored(row, "state")?,
            created_by: row.try_get("created_by")?,
            created_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
            used_at: row.try_get("used_at")?,
            device_id: row.try_get("device_id")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIGRATION: &str = include_str!("../../migrations/0001_core.sql");

    /// The texts between `IN (` and `)` of the CHECK of `column` in `table`.
    fn allowed_in_schema(table: &str, column: &str) -> Vec<String> {
        let start = MIGRATION
            .find(&format!("CREATE TABLE {table} ("))
            .unwrap_or_else(|| panic!("table {table} is in the migration"));
        let table_sql = &MIGRATION[start..];
        let marker = format!("CHECK ({column} IN (");
        let at = table_sql
            .find(&marker)
            .unwrap_or_else(|| panic!("{table}.{column} has a CHECK"));
        let list = &table_sql[at + marker.len()..];
        let list = &list[..list.find(')').unwrap()];
        list.split(',')
            .map(|item| item.trim().trim_matches('\'').to_owned())
            .collect()
    }

    fn texts<T: Copy>(all: &[T], text: fn(T) -> &'static str) -> Vec<String> {
        all.iter().map(|value| text(*value).to_owned()).collect()
    }

    #[test]
    fn the_stored_texts_are_exactly_the_ones_the_schema_accepts() {
        assert_eq!(
            texts(DeviceState::ALL, DeviceState::as_str),
            allowed_in_schema("device", "state")
        );
        assert_eq!(
            texts(Slot::ALL, Slot::as_str),
            allowed_in_schema("device", "slot")
        );
        assert_eq!(
            texts(InviteState::ALL, InviteState::as_str),
            allowed_in_schema("invite", "state")
        );
        assert_eq!(
            texts(HermesMode::ALL, HermesMode::as_str),
            allowed_in_schema("hermes", "mode")
        );
    }

    #[test]
    fn every_stored_text_reads_back_and_unknown_ones_do_not() {
        for state in DeviceState::ALL {
            assert_eq!(DeviceState::parse(state.as_str()), Some(*state));
            assert_eq!(state.to_string(), state.as_str());
        }
        for reason in StateReason::ALL {
            assert_eq!(StateReason::parse(reason.as_str()), Some(*reason));
        }
        for slot in Slot::ALL {
            assert_eq!(Slot::parse(slot.as_str()), Some(*slot));
        }
        for state in InviteState::ALL {
            assert_eq!(InviteState::parse(state.as_str()), Some(*state));
        }
        for unknown in ["", "ACTIVE", "active ", "liberated", "reactivated"] {
            assert_eq!(DeviceState::parse(unknown), None, "{unknown:?}");
            assert_eq!(StateReason::parse(unknown), None, "{unknown:?}");
        }
        let mut reasons: Vec<&str> = StateReason::ALL.iter().map(|r| r.as_str()).collect();
        reasons.sort_unstable();
        reasons.dedup();
        assert_eq!(reasons.len(), StateReason::ALL.len());
    }

    #[test]
    fn the_final_states_are_the_ones_without_a_slot() {
        // The schema says: slot IS NULL exactly in denied, expired, replaced and revoked.
        let finals: Vec<&str> = DeviceState::ALL
            .iter()
            .filter(|state| state.is_final())
            .map(|state| state.as_str())
            .collect();
        assert_eq!(finals, ["denied", "expired", "replaced", "revoked"]);
        assert!(MIGRATION.contains(
            "CHECK ((slot IS NULL) = (state IN ('denied', 'expired', 'replaced', 'revoked')))"
        ));
        let serving: Vec<&DeviceState> = DeviceState::ALL
            .iter()
            .filter(|state| state.serves())
            .collect();
        assert_eq!(serving, [&DeviceState::Approved, &DeviceState::Active]);
    }

    #[test]
    fn each_state_answers_its_code() {
        use DeviceState as S;
        use StateReason as R;
        let cases: &[(S, Option<R>, Option<&str>)] = &[
            (S::Pending, None, None),
            (S::Approved, None, None),
            (S::Active, None, None),
            (S::Denied, Some(R::CodeAttempts), Some("AR-301")),
            (S::Denied, Some(R::Enrollments), Some("AR-301")),
            (S::Denied, Some(R::Admin), Some("AR-301")),
            (S::Denied, Some(R::DoubleWaiting), Some("AR-301")),
            (S::Denied, None, Some("AR-301")),
            (S::Expired, Some(R::PendingTimeout), Some("AR-302")),
            (S::Expired, Some(R::RestoredPending), Some("AR-302")),
            (S::Expired, Some(R::ApprovedTimeout), Some("AR-303")),
            (S::Expired, Some(R::RestoredApproved), Some("AR-303")),
            (S::Expired, Some(R::Inactivity), Some("AR-507")),
            (S::Quarantined, Some(R::Reuse), Some("AR-501")),
            (
                S::Quarantined,
                Some(R::UnreadableCredential),
                Some("AR-501"),
            ),
            (S::Quarantined, Some(R::DoubleConnection), Some("AR-501")),
            (S::Quarantined, Some(R::NotRecognized), Some("AR-501")),
            (S::Revoked, Some(R::Admin), Some("AR-502")),
            (S::Revoked, Some(R::Left), Some("AR-502")),
            (S::Revoked, Some(R::NewInvite), Some("AR-502")),
            (S::Replaced, Some(R::Reconnected), Some("AR-503")),
            (S::Suspended, Some(R::Restored), Some("AR-506")),
        ];
        for (state, reason, expected) in cases {
            let code = ended_code(*state, *reason).map(ErrorCode::as_str);
            assert_eq!(code, *expected, "{state:?} {reason:?}");
        }
    }

    #[test]
    fn an_expired_device_without_a_telling_reason_discards_nothing() {
        for reason in [None, Some(StateReason::Admin), Some(StateReason::Reuse)] {
            assert_eq!(
                ended_code(DeviceState::Expired, reason),
                Some(ErrorCode::Ar507)
            );
        }
    }

    #[test]
    fn only_the_three_states_on_their_way_have_no_code() {
        for state in DeviceState::ALL {
            for reason in std::iter::once(None).chain(StateReason::ALL.iter().copied().map(Some)) {
                let on_its_way = matches!(
                    state,
                    DeviceState::Pending | DeviceState::Approved | DeviceState::Active
                );
                assert_eq!(ended_code(*state, reason).is_none(), on_its_way);
            }
        }
    }

    #[test]
    fn the_state_message_follows_the_device() {
        assert_eq!(
            channel_state(DeviceState::Pending, None),
            ChannelState::Waiting
        );
        assert_eq!(
            channel_state(DeviceState::Approved, None),
            ChannelState::Approved
        );
        assert_eq!(
            channel_state(DeviceState::Active, None),
            ChannelState::Active
        );
        assert_eq!(
            channel_state(DeviceState::Revoked, Some(StateReason::Admin)),
            ChannelState::Ended(ErrorCode::Ar502)
        );
        assert_eq!(
            channel_state(DeviceState::Suspended, Some(StateReason::Restored)),
            ChannelState::Ended(ErrorCode::Ar506)
        );
        // No state that has a code is ever told to swap.
        for state in DeviceState::ALL {
            let message = channel_state(*state, None);
            assert_eq!(
                matches!(message, ChannelState::Approved | ChannelState::Active),
                state.serves(),
                "{state:?}"
            );
        }
    }

    #[test]
    fn the_requests_closed_by_the_restored_mode_are_told_apart() {
        let closed: Vec<&StateReason> = StateReason::ALL
            .iter()
            .filter(|reason| reason.closed_by_restoration())
            .collect();
        assert_eq!(
            closed,
            [
                &StateReason::RestoredPending,
                &StateReason::RestoredApproved
            ]
        );
    }
}
