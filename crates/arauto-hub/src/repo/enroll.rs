//! The enrollment (`docs/04-seguranca-conexao.md`, C3 and C4; `docs/14-protocolo-do-canal.md`,
//! section 5.2).
//!
//! A connector that finished the handshake presents an invite. The hub looks for the digest
//! of the invite and decides, line by line of the table of C3:
//!
//! | Situation | Answer |
//! |---|---|
//! | Invite issued and within its time | The device is created as pending, the invite becomes used. |
//! | Used, same identity, request still pending | The same request: the code of this enrollment replaces the one before. The sixth enrollment denies the request. |
//! | Used, same identity, request already resolved | The exact situation: AR-104, AR-301, AR-302 or AR-303. Nothing changes. |
//! | Used, another identity, request still pending | AR-102 and an alert. The first request is not touched. |
//! | Anything else | AR-101, without saying why. |
//!
//! The verification code is computed here, from the nonce the hub drew and the handshake
//! hash, and stored in the row of the device until the request is resolved. It is never sent.
//!
//! Refusals do not get a row each: anyone with the public key of the hub can ask, and the
//! audit is never erased. They are counted in memory ([`RefusedEnrollments`]) and written as
//! one row per origin and per minute ([`flush_refused`]).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};

use arauto_protocol::code::{CodeError, EnrollNonce, VerificationCode};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::InviteDigest;
use arauto_protocol::limits::{ENROLLMENTS_PER_REQUEST_MAX, MINUTE_MS, SECOND_MS};
use arauto_protocol::message::{ConfirmWithin, Declared, DeclaredText, Enrolled, EnrollmentNumber};
use arauto_protocol::noise::{HandshakeHash, PublicKey};
use sqlx::postgres::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use super::alert::{self, AlertDetails, AlertKind, NewAlert};
use super::sweep::{expire_device, has_passed};
use super::{RepoError, lock_hermes};
use crate::audit::{self, Action, Actor, Details, Event, Outcome, TargetKind};
use crate::core::model::{Device, DeviceState, Invite, InviteState, StateReason};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// How long the admin has to type the code, counted from the first enrollment.
pub const CONFIRM_WINDOW_MS: i64 = 10 * MINUTE_MS;

/// How many times [`draw_nonce`] draws before giving up. One draw fails about once in
/// 10^18 times.
const NONCE_DRAWS: u32 = 8;

/// Draws the nonce of one enrollment: one from which a code can be taken without bias
/// (`docs/14-protocolo-do-canal.md`, section 8).
///
/// # Errors
/// [`RepoError::Random`] when the generator of the system fails.
pub fn draw_nonce(handshake_hash: &HandshakeHash) -> Result<EnrollNonce, RepoError> {
    for _ in 0..NONCE_DRAWS {
        let nonce = EnrollNonce::random().map_err(|_| RepoError::Random)?;
        match VerificationCode::derive(&nonce, handshake_hash) {
            Ok(_) => return Ok(nonce),
            Err(CodeError::NoUsableWord) => {}
            Err(CodeError::Derivation) => return Err(RepoError::Random),
        }
    }
    Err(RepoError::Random)
}

/// What a connector brought to its enrollment, besides the invite.
#[derive(Debug, Clone, Copy)]
pub struct Enrollment<'a> {
    /// The identity it proved in the handshake.
    pub identity: &'a PublicKey,
    /// What it says about itself. Not verified.
    pub declared: &'a Declared,
    /// The version of the connector, from `hello`. Not verified.
    pub connector_version: &'a DeclaredText,
    /// The address the connection came from.
    pub origin: Option<IpAddr>,
    /// The nonce the hub drew for this enrollment ([`draw_nonce`]), which goes in `enrolled`.
    pub nonce: &'a EnrollNonce,
    /// The hash of the handshake of this connection.
    pub handshake_hash: &'a HandshakeHash,
}

/// An enrollment that was accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    /// The device of the request.
    pub device: Uuid,
    /// The Hermes it is for.
    pub hermes: Uuid,
    /// The `enrolled` message to send.
    pub reply: Enrolled,
    /// Whether this enrollment replaced the code of an earlier one of the same request.
    pub code_replaced: bool,
}

/// An enrollment that was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    /// The code of the `closing`: AR-101, AR-102, AR-104, AR-301, AR-302 or AR-303.
    pub code: ErrorCode,
    /// The request this enrollment closed: the sixth enrollment denies it, and one that
    /// arrives after the deadline expires it. Its waiting connection must be told.
    pub closed: Option<Uuid>,
}

/// The outcome of [`enroll`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrollOutcome {
    /// The request exists and holds the code of this enrollment.
    Enrolled(Accepted),
    /// The enrollment was refused.
    Refused(Refusal),
}

const fn refused(code: ErrorCode) -> EnrollOutcome {
    EnrollOutcome::Refused(Refusal { code, closed: None })
}

/// What a request that is no longer pending answers to the identity that owns it.
fn resolved_code(device: &Device) -> ErrorCode {
    match (device.state, device.state_reason) {
        (DeviceState::Denied, _) => ErrorCode::Ar301,
        (
            DeviceState::Expired,
            Some(StateReason::PendingTimeout | StateReason::RestoredPending),
        ) => ErrorCode::Ar302,
        (
            DeviceState::Expired,
            Some(StateReason::ApprovedTimeout | StateReason::RestoredApproved),
        ) => ErrorCode::Ar303,
        // Approved, or anything that only comes after an approval.
        _ => ErrorCode::Ar104,
    }
}

/// The seconds left until `deadline`, rounded up, within what `enrolled` can say.
fn confirm_within(deadline: i64, now: i64) -> ConfirmWithin {
    let left_ms = deadline.saturating_sub(now).max(1);
    let seconds = left_ms.saturating_add(SECOND_MS - 1) / SECOND_MS;
    u32::try_from(seconds)
        .ok()
        .and_then(ConfirmWithin::new)
        .unwrap_or(ConfirmWithin::MAX)
}

fn code_text(code: VerificationCode) -> String {
    format!("{:06}", code.value())
}

/// Applies one enrollment.
///
/// `organization` is the one of the invite ([`crate::db::organization_of_invite`]). A digest
/// that no invite has belongs to nobody: the hub answers AR-101 and counts it with
/// [`RefusedEnrollments::note`], without calling this.
///
/// # Errors
/// [`RepoError::Random`] when no code can be taken from `nonce` (draw it with
/// [`draw_nonce`]); [`RepoError::Restored`] when the database is behind the audit mark. In
/// both cases nothing is written.
pub async fn enroll(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    refusals: &RefusedEnrollments,
    invite: &InviteDigest,
    request: &Enrollment<'_>,
    now: i64,
) -> Result<EnrollOutcome, RepoError> {
    let code = VerificationCode::derive(request.nonce, request.handshake_hash)
        .map_err(|_| RepoError::Random)?;
    let mut write = audit::begin_security(pool, mark).await?;

    // The Hermes of an invite never changes: reading it before the lock is safe.
    let hermes: Option<Uuid> = sqlx::query_scalar(
        "SELECT hermes_id FROM invite WHERE organization_id = $1 AND digest = $2",
    )
    .bind(organization.id())
    .bind(invite.as_bytes().as_slice())
    .fetch_optional(write.connection())
    .await?;
    let locked = match hermes {
        Some(hermes) => lock_hermes(write.connection(), organization, hermes).await?,
        None => None,
    };
    let found: Option<Invite> = match &locked {
        Some(_) => {
            sqlx::query_as(
                "SELECT * FROM invite WHERE organization_id = $1 AND digest = $2 FOR UPDATE",
            )
            .bind(organization.id())
            .bind(invite.as_bytes().as_slice())
            .fetch_optional(write.connection())
            .await?
        }
        None => None,
    };
    let (Some(locked), Some(found)) = (locked, found) else {
        write.rollback().await?;
        refusals.note(Some(organization), request.origin, ErrorCode::Ar101, now);
        return Ok(refused(ErrorCode::Ar101));
    };

    // An issued invite past its time expires here, where it is met, as the sweep would do
    // it: the digest is erased, and a clock that goes back does not make it hold again (C1).
    if found.state == InviteState::Issued && has_passed(Some(found.expires_at), now) {
        sqlx::query(
            "UPDATE invite SET state = 'expired', digest = NULL, join_digest = NULL \
             WHERE organization_id = $1 AND id = $2",
        )
        .bind(organization.id())
        .bind(found.id)
        .execute(write.connection())
        .await?;
        write
            .record(
                now,
                &Event::new(Action::InviteExpired, Actor::System)
                    .in_organization(organization)
                    .about(TargetKind::Invite, found.id)
                    .with(Details::new().id("hermes", locked.hermes.id)),
            )
            .await?;
        write.commit().await?;
        refusals.note(Some(organization), request.origin, ErrorCode::Ar101, now);
        return Ok(refused(ErrorCode::Ar101));
    }

    // Line 1: issued and within its time.
    if found.state == InviteState::Issued
        && !has_passed(Some(found.expires_at), now)
        && locked.hermes.deleted_at.is_none()
        && locked.request().is_none()
    {
        let device = Uuid::new_v4();
        let confirm_until = now.saturating_add(CONFIRM_WINDOW_MS);
        let declared = serde_json::to_value(request.declared)
            .map_err(|_| RepoError::Inconsistent("what the device declared is not an object"))?;
        // An identity that already has a row, of any Hermes, is not a new request.
        let created = sqlx::query(
            "INSERT INTO device (id, organization_id, hermes_id, public_key, state, \
                                 state_since, slot, verification_code, confirm_until, \
                                 enrollments, declared, enrolled_from, connector_version, \
                                 created_at) \
             VALUES ($1, $2, $3, $4, 'pending', $5, 'request', $6, $7, 1, $8, $9, $10, $5) \
             ON CONFLICT (public_key) DO NOTHING",
        )
        .bind(device)
        .bind(organization.id())
        .bind(locked.hermes.id)
        .bind(request.identity.as_bytes().as_slice())
        .bind(now)
        .bind(code_text(code))
        .bind(confirm_until)
        .bind(Json(declared))
        .bind(request.origin.map(|origin| origin.to_string()))
        .bind(request.connector_version.as_str())
        .execute(write.connection())
        .await?
        .rows_affected();
        if created == 0 {
            write.rollback().await?;
            refusals.note(Some(organization), request.origin, ErrorCode::Ar101, now);
            return Ok(refused(ErrorCode::Ar101));
        }
        sqlx::query(
            "UPDATE invite SET state = 'used', used_at = $3, device_id = $4 \
             WHERE organization_id = $1 AND id = $2",
        )
        .bind(organization.id())
        .bind(found.id)
        .bind(now)
        .bind(device)
        .execute(write.connection())
        .await?;
        let mut event = Event::new(Action::InviteUsed, Actor::Device { id: device })
            .in_organization(organization)
            .about(TargetKind::Invite, found.id)
            .with(
                Details::new()
                    .id("device", device)
                    .id("hermes", locked.hermes.id),
            );
        if let Some(origin) = request.origin {
            event = event.from_address(origin);
        }
        write.record(now, &event).await?;
        write.commit().await?;
        return Ok(EnrollOutcome::Enrolled(Accepted {
            device,
            hermes: locked.hermes.id,
            reply: Enrolled {
                nonce: *request.nonce,
                confirm_within_s: confirm_within(confirm_until, now),
                enrollment: EnrollmentNumber::MIN,
            },
            code_replaced: false,
        }));
    }

    let first = match (found.state, found.device_id) {
        (InviteState::Used, Some(device)) => locked.device(device),
        _ => None,
    };
    let still_pending = |device: &Device| {
        device.state == DeviceState::Pending && !has_passed(device.confirm_until, now)
    };
    match first {
        // Lines 2 and 3: the identity that owns the request.
        Some(device) if device.public_key == *request.identity => {
            if device.state != DeviceState::Pending {
                write.rollback().await?;
                return Ok(refused(resolved_code(device)));
            }
            if !still_pending(device) {
                expire_device(
                    &mut write,
                    organization,
                    device,
                    StateReason::PendingTimeout,
                    now,
                )
                .await?;
                write.commit().await?;
                return Ok(EnrollOutcome::Refused(Refusal {
                    code: ErrorCode::Ar302,
                    closed: Some(device.id),
                }));
            }
            let actor = Actor::Device { id: device.id };
            if device.enrollments >= i32::from(ENROLLMENTS_PER_REQUEST_MAX) {
                super::closing::close_device(
                    &mut write,
                    organization,
                    device,
                    StateReason::Enrollments,
                    &actor,
                    request.origin,
                    now,
                )
                .await?;
                write.commit().await?;
                return Ok(EnrollOutcome::Refused(Refusal {
                    code: ErrorCode::Ar301,
                    closed: Some(device.id),
                }));
            }
            // The same request: neither the deadline nor the attempts change.
            let enrollments = device.enrollments.saturating_add(1);
            sqlx::query(
                "UPDATE device SET enrollments = $3, verification_code = $4 \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(device.id)
            .bind(enrollments)
            .bind(code_text(code))
            .execute(write.connection())
            .await?;
            let mut event = Event::new(Action::DeviceEnrolledAgain, actor)
                .in_organization(organization)
                .about(TargetKind::Device, device.id)
                .with(Details::new().number("enrollment", i64::from(enrollments)));
            if let Some(origin) = request.origin {
                event = event.from_address(origin);
            }
            write.record(now, &event).await?;
            write.commit().await?;
            let enrollment = u8::try_from(enrollments)
                .ok()
                .and_then(EnrollmentNumber::new)
                .ok_or(RepoError::Inconsistent("the count of enrollments"))?;
            Ok(EnrollOutcome::Enrolled(Accepted {
                device: device.id,
                hermes: locked.hermes.id,
                reply: Enrolled {
                    nonce: *request.nonce,
                    confirm_within_s: confirm_within(device.confirm_until.unwrap_or(now), now),
                    enrollment,
                },
                code_replaced: true,
            }))
        }
        // Line 4: another identity, while the request is still pending. The block went
        // somewhere else. The first request is not touched.
        Some(device) if still_pending(device) => {
            let already = alert::raised_since(
                write.connection(),
                organization,
                AlertKind::InviteMisused,
                device.id,
                i64::MIN,
            )
            .await?;
            if already {
                // One alert and one event per request; the rest is counted.
                write.rollback().await?;
                refusals.note(Some(organization), request.origin, ErrorCode::Ar102, now);
                return Ok(refused(ErrorCode::Ar102));
            }
            alert::raise(
                write.connection(),
                organization,
                now,
                &NewAlert {
                    kind: AlertKind::InviteMisused,
                    hermes: Some(locked.hermes.id),
                    device: Some(device.id),
                    details: AlertDetails::new().address("from", request.origin),
                },
            )
            .await?;
            let mut event = Event::new(Action::InviteMisused, Actor::System)
                .in_organization(organization)
                .about(TargetKind::Invite, found.id)
                .ended(Outcome::Denied, Some(ErrorCode::Ar102))
                .with(
                    Details::new()
                        .id("request", device.id)
                        .id("hermes", locked.hermes.id),
                );
            if let Some(origin) = request.origin {
                event = event.from_address(origin);
            }
            write.record(now, &event).await?;
            write.commit().await?;
            Ok(refused(ErrorCode::Ar102))
        }
        // Line 5: anything else.
        _ => {
            write.rollback().await?;
            refusals.note(Some(organization), request.origin, ErrorCode::Ar101, now);
            Ok(refused(ErrorCode::Ar101))
        }
    }
}

// ---------------------------------------------------------------------------- refusals

/// How many different counters are kept at once. Past it, what is new is counted together,
/// without the origin: a flood from many addresses takes a fixed amount of memory.
const REFUSAL_COUNTERS_MAX: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RefusalKey {
    /// The minute, as its first millisecond.
    minute: i64,
    organization: Option<ResolvedOrg>,
    origin: Option<IpAddr>,
    code: ErrorCode,
}

/// The refused enrollments that were not written yet, counted per origin and per minute.
///
/// The hub keeps one of these for its whole life, counts with [`RefusedEnrollments::note`]
/// and writes the minutes that ended with [`flush_refused`] at every sweep. What was counted
/// in the minute in which the hub stops is lost; these rows change nobody's access.
#[derive(Debug, Default)]
pub struct RefusedEnrollments {
    counters: Mutex<HashMap<RefusalKey, u32>>,
}

const fn minute_of(now: i64) -> i64 {
    now.div_euclid(MINUTE_MS).saturating_mul(MINUTE_MS)
}

impl RefusedEnrollments {
    /// No refusal yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Counts one refused enrollment. `organization` is the one of the invite, when the
    /// digest was of an invite at all.
    pub fn note(
        &self,
        organization: Option<ResolvedOrg>,
        origin: Option<IpAddr>,
        code: ErrorCode,
        now: i64,
    ) {
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        let mut key = RefusalKey {
            minute: minute_of(now),
            organization,
            origin,
            code,
        };
        if counters.len() >= REFUSAL_COUNTERS_MAX && !counters.contains_key(&key) {
            key.organization = None;
            key.origin = None;
        }
        let count = counters.entry(key).or_insert(0);
        *count = count.saturating_add(1);
    }

    /// How many refusals are counted and not written yet.
    #[must_use]
    pub fn waiting(&self) -> u64 {
        self.counters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .map(|count| u64::from(*count))
            .sum()
    }

    /// Takes out the counters of the minutes that ended before `now`.
    fn take_ended(&self, now: i64) -> Vec<(RefusalKey, u32)> {
        let current = minute_of(now);
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        let ended: Vec<RefusalKey> = counters
            .keys()
            .filter(|key| key.minute < current)
            .copied()
            .collect();
        ended
            .into_iter()
            .filter_map(|key| counters.remove(&key).map(|count| (key, count)))
            .collect()
    }

    /// Puts back what could not be written.
    fn put_back(&self, entries: Vec<(RefusalKey, u32)>) {
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        for (key, count) in entries {
            let kept = counters.entry(key).or_insert(0);
            *kept = kept.saturating_add(count);
        }
    }
}

/// Writes the refused enrollments of the minutes that ended before `now`: one row of the
/// audit per origin and per minute, with the count. Returns how many rows it wrote.
///
/// This is the one function of the repositories without a resolved organization: an invite
/// nobody issued belongs to no organization, and its refusals are of the installation.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; the counters are kept for the next
/// call.
pub async fn flush_refused(
    pool: &PgPool,
    refusals: &RefusedEnrollments,
    now: i64,
) -> Result<u64, RepoError> {
    let ended = refusals.take_ended(now);
    if ended.is_empty() {
        return Ok(0);
    }
    let written: Result<(), RepoError> = async {
        let mut transaction = pool.begin().await?;
        for (key, count) in &ended {
            let mut event = Event::new(Action::InviteRefused, Actor::System)
                .ended(Outcome::Denied, Some(key.code))
                .with(
                    Details::new()
                        .number("count", i64::from(*count))
                        .number("minute", key.minute)
                        .flag("origin_known", key.origin.is_some()),
                );
            if let Some(organization) = key.organization {
                event = event.in_organization(organization);
            }
            if let Some(origin) = key.origin {
                event = event.from_address(origin);
            }
            // The row is dated at the end of its minute: it says what happened up to then.
            audit::record(
                &mut *transaction,
                key.minute.saturating_add(MINUTE_MS),
                &event,
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
    .await;
    match written {
        Ok(()) => Ok(u64::try_from(ended.len()).unwrap_or(u64::MAX)),
        Err(error) => {
            refusals.put_back(ended);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000_000;

    fn origin(last: u8) -> Option<IpAddr> {
        Some(IpAddr::from([192, 0, 2, last]))
    }

    #[test]
    fn refusals_are_counted_per_origin_per_minute_and_per_code() {
        let refusals = RefusedEnrollments::new();
        let minute = minute_of(T0);
        for _ in 0..500 {
            refusals.note(None, origin(1), ErrorCode::Ar101, minute + 5);
        }
        refusals.note(None, origin(1), ErrorCode::Ar101, minute + MINUTE_MS - 1);
        refusals.note(None, origin(2), ErrorCode::Ar101, minute + 7);
        refusals.note(None, origin(1), ErrorCode::Ar102, minute + 9);
        refusals.note(None, origin(1), ErrorCode::Ar101, minute + MINUTE_MS);
        assert_eq!(refusals.waiting(), 504);

        // The minute in progress stays.
        assert!(refusals.take_ended(minute + MINUTE_MS - 1).is_empty());
        let mut ended = refusals.take_ended(minute + MINUTE_MS);
        ended.sort_by_key(|(key, count)| (key.origin, key.code, *count));
        let seen: Vec<(Option<IpAddr>, ErrorCode, u32)> = ended
            .iter()
            .map(|(key, count)| (key.origin, key.code, *count))
            .collect();
        assert_eq!(
            seen,
            [
                (origin(1), ErrorCode::Ar101, 501),
                (origin(1), ErrorCode::Ar102, 1),
                (origin(2), ErrorCode::Ar101, 1),
            ]
        );
        assert!(ended.iter().all(|(key, _)| key.minute == minute));
        assert_eq!(refusals.waiting(), 1);

        refusals.put_back(ended);
        assert_eq!(refusals.waiting(), 504);
    }

    #[test]
    fn a_flood_from_many_origins_takes_a_fixed_amount_of_memory() {
        let refusals = RefusedEnrollments::new();
        let total = u32::try_from(REFUSAL_COUNTERS_MAX).unwrap() * 3;
        for n in 0..total {
            let address = IpAddr::from(n.to_be_bytes());
            refusals.note(None, Some(address), ErrorCode::Ar101, T0);
        }
        let counters = refusals.counters.lock().unwrap();
        assert!(counters.len() <= REFUSAL_COUNTERS_MAX + 1);
        assert_eq!(
            counters
                .values()
                .map(|count| u64::from(*count))
                .sum::<u64>(),
            u64::from(total)
        );
        let together = counters
            .iter()
            .find(|(key, _)| key.origin.is_none())
            .map(|(_, count)| *count);
        assert_eq!(
            together,
            Some(total - u32::try_from(REFUSAL_COUNTERS_MAX).unwrap())
        );
    }

    #[test]
    fn the_time_left_is_rounded_up_and_stays_in_range() {
        assert_eq!(confirm_within(T0 + 600_000, T0).get(), 600);
        assert_eq!(confirm_within(T0 + 1, T0).get(), 1);
        assert_eq!(confirm_within(T0 + 1_001, T0).get(), 2);
        assert_eq!(confirm_within(T0, T0).get(), 1);
        assert_eq!(confirm_within(T0 - 5_000, T0).get(), 1);
        assert_eq!(confirm_within(i64::MAX, 0), ConfirmWithin::MAX);
    }

    #[test]
    fn the_code_is_stored_with_its_six_digits() {
        let code = VerificationCode::parse("000-042").unwrap();
        assert_eq!(code_text(code), "000042");
        assert_eq!(VerificationCode::parse(&code_text(code)), Some(code));
    }

    #[test]
    fn a_drawn_nonce_always_gives_a_code() {
        let hash = HandshakeHash::from_bytes([7; 32]);
        for _ in 0..64 {
            let nonce = draw_nonce(&hash).unwrap();
            assert!(VerificationCode::derive(&nonce, &hash).is_ok());
        }
    }
}
