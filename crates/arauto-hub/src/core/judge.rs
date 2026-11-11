//! The rule of the credential swap, as pure functions.
//!
//! This is `docs/04-seguranca-conexao.md`, C7 and C11, and the two tables of
//! `docs/14-protocolo-do-canal.md`, section 6. Nothing here touches the database, the network
//! or the clock: a function takes the picture of a device and what was presented, and returns
//! the decision or the new picture. The transactions T1 and T2 of the channel read the device
//! under lock, call these functions and write what they return; the simulator of
//! [`super::swap_model`] calls the same functions tens of thousands of times.
//!
//! ```text
//!  judge      what to do with what a connection presented
//!  rotation   whether the 24-hour swap may issue now
//!  issue      T1: the new credential becomes the pending one (promoting the old pending or not)
//!  confirm    T2: the "saved" of this connection promotes the pending one
//! ```

use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::message::Generation;

use super::model::DeviceState;

/// One sealed credential as the hub knows it: the generation and the SHA-256 of the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credential {
    /// The generation written inside the seal.
    pub generation: Generation,
    /// SHA-256 of the whole block. Two credentials are the same only when this is equal:
    /// after a restoration of the hub a generation number can repeat (C7, rule 2).
    pub digest: CredentialDigest,
}

/// The picture of a device that the rule needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceSnapshot {
    /// The state.
    pub state: DeviceState,
    /// The current credential. `None` at generation zero.
    pub current: Option<Credential>,
    /// The credential that was issued and not confirmed yet.
    pub pending: Option<Credential>,
    /// The largest generation ever issued to this device. Zero when none was.
    pub max_generation: u64,
    /// Whether the device already existed when a restoration was handled (C11).
    pub marked_by_restoration: bool,
}

/// What a connection presented, after the vault looked at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// `null`: the connector has no credential.
    Nothing,
    /// A seal that opened for this device, and says this.
    Opened(Credential),
    /// A seal that does not open: changed, of another device, of another version.
    Unreadable,
}

/// Which conversation the presentation belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The swap: the hub may accept and issue.
    Swap,
    /// The presentation without swap (ended device, or outdated connector): the hub only
    /// looks, and never issues.
    PresentationOnly,
}

/// Why the hub stops trusting the credential of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuarantineReason {
    /// A credential that was already swapped, or none when there should be one.
    Reuse,
    /// A seal that does not open.
    Unreadable,
}

/// What the hub does with a presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Accept, and issue a new credential with [`issue`].
    Accept {
        /// The pending credential was presented: it becomes the current one.
        promote_pending: bool,
    },
    /// Quarantine and alert.
    Quarantine(QuarantineReason),
    /// A valid seal the database does not explain: the hub came back from an older copy.
    /// Turn the restored mode on.
    RestorationProof,
    /// A closed device presented a valid seal older than its last ones: someone used a copy,
    /// and the device was closed before the real one came back. Alert.
    ReuseAfterClosing,
    /// A seal the database does not explain, from a device that no longer counts as proof.
    /// Only an ordinary audit record.
    RecordOnly,
    /// Nothing to do but answer the state of the device.
    Nothing,
}

/// Judges what a connection of a known device presented.
///
/// Call it only for a connection that proved the identity in the handshake **and** answered
/// the check, and that is the newest connection of the device (C7, rule 3): the function
/// cannot see that.
///
/// For an approved or active device in [`Mode::Swap`], it is the table of C7:
///
/// | Presented | Decision |
/// |---|---|
/// | The current one, or nothing at generation zero | accept |
/// | The pending one | accept and promote |
/// | Another one already issued, or nothing past generation zero | quarantine, reuse |
/// | A seal that does not open | quarantine, unreadable |
/// | A valid seal of a generation above every known one | restoration proof |
///
/// In [`Mode::PresentationOnly`], and for a device in any other state, nothing is ever
/// accepted. An approved or active device (an outdated connector) still gets the last three
/// lines. A device that was revoked, replaced or expired after having a confirmed credential,
/// and that presents a valid seal older than its last current and its last pending ones,
/// gets [`Decision::ReuseAfterClosing`].
///
/// A valid seal of a generation above every known one is restoration proof in every state
/// and mode, unless the device is marked by an earlier restoration.
#[must_use]
pub fn judge(device: &DeviceSnapshot, presented: &Presented, mode: Mode) -> Decision {
    let serving = device.state.serves();
    if let Presented::Opened(shown) = presented
        && shown.generation.get() > device.max_generation
    {
        return if !device.marked_by_restoration {
            Decision::RestorationProof
        } else if serving {
            // It is no proof any more, and it is neither the current nor the pending one.
            Decision::Quarantine(QuarantineReason::Reuse)
        } else {
            Decision::RecordOnly
        };
    }
    if !serving {
        return judge_closed(device, presented);
    }
    let verdict = match presented {
        Presented::Nothing if at_generation_zero(device) => Decision::Accept {
            promote_pending: false,
        },
        Presented::Nothing => Decision::Quarantine(QuarantineReason::Reuse),
        Presented::Unreadable => Decision::Quarantine(QuarantineReason::Unreadable),
        Presented::Opened(shown) if device.current.as_ref() == Some(shown) => Decision::Accept {
            promote_pending: false,
        },
        Presented::Opened(shown) if device.pending.as_ref() == Some(shown) => Decision::Accept {
            promote_pending: true,
        },
        Presented::Opened(_) => Decision::Quarantine(QuarantineReason::Reuse),
    };
    match (mode, verdict) {
        (Mode::PresentationOnly, Decision::Accept { .. }) => Decision::Nothing,
        (_, verdict) => verdict,
    }
}

/// Generation zero: approved, and no credential confirmed yet. Only here is the identity
/// alone enough (C7, rule 8).
fn at_generation_zero(device: &DeviceSnapshot) -> bool {
    device.state == DeviceState::Approved && device.current.is_none()
}

fn judge_closed(device: &DeviceSnapshot, presented: &Presented) -> Decision {
    let Presented::Opened(shown) = presented else {
        return Decision::Nothing;
    };
    let closed_after_use = match device.state {
        DeviceState::Revoked | DeviceState::Replaced => true,
        // Expired for inactivity: a request that expired never had a current credential.
        DeviceState::Expired => device.current.is_some(),
        _ => false,
    };
    let older_than = |last: &Option<Credential>| {
        last.as_ref()
            .is_none_or(|last| shown.generation < last.generation)
    };
    let has_last = device.current.is_some() || device.pending.is_some();
    if closed_after_use && has_last && older_than(&device.current) && older_than(&device.pending) {
        Decision::ReuseAfterClosing
    } else {
        Decision::Nothing
    }
}

/// Judges a presentation from an identity the database does not know (C11).
///
/// `proof_pending` is the answer of the list of unknown devices: whether the listing of this
/// identity is proof that no restored mode handled yet. It is so from the moment the identity
/// is listed, with the mode off, until the mode is turned on: a hub that stops between the
/// two finds the proof again at the next presentation. `restored_mode` is whether the mode
/// is already on.
///
/// - `null`, or a seal that opens under no organization key: nothing, and nothing is recorded
///   for each attempt.
/// - A valid seal from an identity whose listing was already handled: only a record.
/// - A valid seal from an identity whose listing is still to be handled: restoration proof,
///   unless the mode is already on. With the mode on, C11 only restarts it for a database
///   behind the mark or for a seal of a device created after the mode began; an unknown
///   device is listed and nothing more.
#[must_use]
pub fn judge_unknown(presented: &Presented, proof_pending: bool, restored_mode: bool) -> Decision {
    match presented {
        Presented::Opened(_) if proof_pending && !restored_mode => Decision::RestorationProof,
        Presented::Opened(_) => Decision::RecordOnly,
        Presented::Nothing | Presented::Unreadable => Decision::Nothing,
    }
}

/// Whether the 24-hour swap may issue now: the device is active, has a current credential
/// and no pending one waiting for its "saved". When it may, the swap goes on as if the
/// current credential had been presented.
#[must_use]
pub fn rotation(device: &DeviceSnapshot) -> Decision {
    if device.state == DeviceState::Active && device.current.is_some() && device.pending.is_none() {
        Decision::Accept {
            promote_pending: false,
        }
    } else {
        Decision::Nothing
    }
}

/// Why a transition was not made. The device is left as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    /// Only an approved or an active device swaps credentials.
    #[error("the device is neither approved nor active")]
    NotServing,
    /// The generations of this device ran out.
    #[error("the device has no generation left")]
    GenerationExhausted,
    /// A new credential must have a generation above every one issued before.
    #[error("the new credential does not have a new generation")]
    GenerationNotNew,
    /// There is no pending credential to promote.
    #[error("the device has no pending credential")]
    NoPending,
    /// The "saved" is not about the pending credential issued on this connection (C7,
    /// rule 7).
    #[error("the confirmation is not about the pending credential of this connection")]
    NotThePending,
}

/// A device after a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    /// The new picture, to be written.
    pub device: DeviceSnapshot,
    /// The device confirmed its first credential in this transition: it became active, and
    /// the device it replaces, if any, is replaced in the same write (C10).
    pub first_confirmation: bool,
}

/// The generation of the next credential of this device: one above the largest ever issued.
///
/// # Errors
/// [`TransitionError::GenerationExhausted`] at the end of the numbers.
pub fn next_generation(device: &DeviceSnapshot) -> Result<Generation, TransitionError> {
    device
        .max_generation
        .checked_add(1)
        .and_then(Generation::new)
        .ok_or(TransitionError::GenerationExhausted)
}

/// T1, after [`Decision::Accept`]: `new` becomes the pending credential and the largest
/// generation. With `promote_pending`, the pending one that was presented becomes the current
/// one first, and the one that was current dies. Without it, a pending one that was waiting
/// is discarded.
///
/// The hub writes this, durably, **before** sending the credential (C7, rule 6).
///
/// # Errors
/// [`TransitionError::NotServing`] when the device is no longer approved or active;
/// [`TransitionError::GenerationNotNew`] when `new` is not above every generation issued;
/// [`TransitionError::NoPending`] when there is nothing to promote.
pub fn issue(
    device: &DeviceSnapshot,
    promote_pending: bool,
    new: Credential,
) -> Result<Transition, TransitionError> {
    if !device.state.serves() {
        return Err(TransitionError::NotServing);
    }
    if new.generation.get() <= device.max_generation {
        return Err(TransitionError::GenerationNotNew);
    }
    let mut after = *device;
    let mut first_confirmation = false;
    if promote_pending {
        let promoted = device.pending.ok_or(TransitionError::NoPending)?;
        first_confirmation = device.state == DeviceState::Approved;
        after.current = Some(promoted);
        after.state = DeviceState::Active;
    }
    after.pending = Some(new);
    after.max_generation = new.generation.get();
    Ok(Transition {
        device: after,
        first_confirmation,
    })
}

/// T2, at the "saved": promotes the pending credential, and the one that was current dies.
///
/// `issued_here` is the credential this very connection received; `saved` is what the
/// connector says it wrote. The promotion only happens when the two are the same and are
/// still the pending credential of the device.
///
/// # Errors
/// [`TransitionError::NotServing`] when the device is no longer approved or active (revoked
/// between the credential and the "saved", for one); [`TransitionError::NotThePending`] when
/// the "saved" is about anything else. In both cases the connection is closed and nothing is
/// written.
pub fn confirm(
    device: &DeviceSnapshot,
    issued_here: &Credential,
    saved: &Credential,
) -> Result<Transition, TransitionError> {
    if !device.state.serves() {
        return Err(TransitionError::NotServing);
    }
    if saved != issued_here || device.pending.as_ref() != Some(issued_here) {
        return Err(TransitionError::NotThePending);
    }
    let mut after = *device;
    after.current = Some(*issued_here);
    after.pending = None;
    after.state = DeviceState::Active;
    Ok(Transition {
        device: after,
        first_confirmation: device.state == DeviceState::Approved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential(generation: u64, mark: u8) -> Credential {
        Credential {
            generation: Generation::new(generation).unwrap(),
            digest: CredentialDigest::from_bytes([mark; 32]),
        }
    }

    /// The device of most cases: active, current 5, pending 6, nothing above 6.
    fn active() -> DeviceSnapshot {
        DeviceSnapshot {
            state: DeviceState::Active,
            current: Some(credential(5, 0x55)),
            pending: Some(credential(6, 0x66)),
            max_generation: 6,
            marked_by_restoration: false,
        }
    }

    /// Approved, at generation zero, with nothing issued yet.
    fn approved() -> DeviceSnapshot {
        DeviceSnapshot {
            state: DeviceState::Approved,
            current: None,
            pending: None,
            max_generation: 0,
            marked_by_restoration: false,
        }
    }

    fn in_state(state: DeviceState) -> DeviceSnapshot {
        DeviceSnapshot { state, ..active() }
    }

    const ACCEPT: Decision = Decision::Accept {
        promote_pending: false,
    };
    const PROMOTE: Decision = Decision::Accept {
        promote_pending: true,
    };
    const REUSE: Decision = Decision::Quarantine(QuarantineReason::Reuse);
    const UNREADABLE: Decision = Decision::Quarantine(QuarantineReason::Unreadable);

    const ALL_STATES: [DeviceState; 9] = [
        DeviceState::Pending,
        DeviceState::Approved,
        DeviceState::Active,
        DeviceState::Quarantined,
        DeviceState::Suspended,
        DeviceState::Denied,
        DeviceState::Expired,
        DeviceState::Replaced,
        DeviceState::Revoked,
    ];

    // ------------------------------------------------------------------ the table of the swap

    /// `docs/14-protocolo-do-canal.md`, section 6, first table: one row per line, and the
    /// variants of each line.
    #[test]
    fn the_judgment_of_an_approved_or_active_device() {
        let approved_with_pending = DeviceSnapshot {
            pending: Some(credential(1, 0x11)),
            max_generation: 1,
            ..approved()
        };
        let no_pending = DeviceSnapshot {
            pending: None,
            max_generation: 5,
            ..active()
        };
        let cases: &[(&str, DeviceSnapshot, Presented, Decision)] = &[
            // Line 1: the current one, or null at generation zero.
            (
                "the current one",
                active(),
                Presented::Opened(credential(5, 0x55)),
                ACCEPT,
            ),
            (
                "the current one, with no pending",
                no_pending,
                Presented::Opened(credential(5, 0x55)),
                ACCEPT,
            ),
            (
                "null at generation zero",
                approved(),
                Presented::Nothing,
                ACCEPT,
            ),
            (
                "null at generation zero, the first one issued and never saved",
                approved_with_pending,
                Presented::Nothing,
                ACCEPT,
            ),
            // Line 2: the pending one.
            (
                "the pending one",
                active(),
                Presented::Opened(credential(6, 0x66)),
                PROMOTE,
            ),
            (
                "the first one, saved and not confirmed",
                approved_with_pending,
                Presented::Opened(credential(1, 0x11)),
                PROMOTE,
            ),
            // Line 3: another one already issued, or null past generation zero.
            (
                "one that was already swapped",
                active(),
                Presented::Opened(credential(4, 0x44)),
                REUSE,
            ),
            (
                "the number of the current one with another digest",
                active(),
                Presented::Opened(credential(5, 0x99)),
                REUSE,
            ),
            (
                "the number of the pending one with another digest",
                active(),
                Presented::Opened(credential(6, 0x99)),
                REUSE,
            ),
            (
                "the digest of the current one under another number",
                active(),
                Presented::Opened(credential(4, 0x55)),
                REUSE,
            ),
            (
                "a pending one that was discarded",
                no_pending,
                Presented::Opened(credential(5, 0x77)),
                REUSE,
            ),
            (
                "null past generation zero",
                active(),
                Presented::Nothing,
                REUSE,
            ),
            (
                "null past generation zero, with no pending",
                no_pending,
                Presented::Nothing,
                REUSE,
            ),
            // Line 4: a seal that does not open, or of another device.
            ("unreadable", active(), Presented::Unreadable, UNREADABLE),
            (
                "unreadable at generation zero",
                approved(),
                Presented::Unreadable,
                UNREADABLE,
            ),
            // Line 5: a valid seal of a generation above every known one.
            (
                "a generation above every known one",
                active(),
                Presented::Opened(credential(7, 0x77)),
                Decision::RestorationProof,
            ),
            (
                "far above",
                active(),
                Presented::Opened(credential(u64::MAX >> 1, 0x77)),
                Decision::RestorationProof,
            ),
            (
                "any seal at all at generation zero with nothing issued",
                approved(),
                Presented::Opened(credential(1, 0x11)),
                Decision::RestorationProof,
            ),
        ];
        for (name, device, presented, expected) in cases {
            assert_eq!(judge(device, presented, Mode::Swap), *expected, "{name}");
        }
    }

    #[test]
    fn the_identity_alone_is_only_enough_at_generation_zero() {
        // Rule 8 of C7. An approved device that somehow has a current credential, and an
        // active one that somehow has none, are both past the point where null is accepted.
        let approved_past_zero = DeviceSnapshot {
            state: DeviceState::Approved,
            ..active()
        };
        let active_without_current = DeviceSnapshot {
            state: DeviceState::Active,
            ..approved()
        };
        assert_eq!(
            judge(&approved_past_zero, &Presented::Nothing, Mode::Swap),
            REUSE
        );
        assert_eq!(
            judge(&active_without_current, &Presented::Nothing, Mode::Swap),
            REUSE
        );
    }

    // ------------------------------------------------------------------ the presentation without swap

    /// Second table, first line: an outdated connector on an approved or active device gets
    /// the last three lines of the swap table, and never a credential.
    #[test]
    fn an_outdated_connector_is_looked_at_and_never_served() {
        let cases: &[(&str, DeviceSnapshot, Presented, Decision)] = &[
            (
                "the current one",
                active(),
                Presented::Opened(credential(5, 0x55)),
                Decision::Nothing,
            ),
            (
                "the pending one is not promoted",
                active(),
                Presented::Opened(credential(6, 0x66)),
                Decision::Nothing,
            ),
            (
                "null at generation zero",
                approved(),
                Presented::Nothing,
                Decision::Nothing,
            ),
            (
                "one already swapped",
                active(),
                Presented::Opened(credential(4, 0x44)),
                REUSE,
            ),
            (
                "null past generation zero",
                active(),
                Presented::Nothing,
                REUSE,
            ),
            ("unreadable", active(), Presented::Unreadable, UNREADABLE),
            (
                "a generation above every known one",
                active(),
                Presented::Opened(credential(7, 0x77)),
                Decision::RestorationProof,
            ),
        ];
        for (name, device, presented, expected) in cases {
            assert_eq!(
                judge(device, presented, Mode::PresentationOnly),
                *expected,
                "{name}"
            );
        }
    }

    /// Second table, second line.
    #[test]
    fn a_closed_device_with_an_older_seal_is_reuse_after_closing() {
        for state in [
            DeviceState::Revoked,
            DeviceState::Replaced,
            DeviceState::Expired,
        ] {
            let device = in_state(state);
            for mode in [Mode::PresentationOnly, Mode::Swap] {
                let judged = |presented: Presented| judge(&device, &presented, mode);
                // Older than both the last current (5) and the last pending (6).
                assert_eq!(
                    judged(Presented::Opened(credential(4, 0x44))),
                    Decision::ReuseAfterClosing,
                    "{state:?}"
                );
                assert_eq!(
                    judged(Presented::Opened(credential(1, 0x01))),
                    Decision::ReuseAfterClosing
                );
                // Its own last ones: the real device coming back, nothing to say.
                assert_eq!(
                    judged(Presented::Opened(credential(5, 0x55))),
                    Decision::Nothing
                );
                assert_eq!(
                    judged(Presented::Opened(credential(6, 0x66))),
                    Decision::Nothing
                );
                // Not older than the last current one.
                assert_eq!(
                    judged(Presented::Opened(credential(5, 0x99))),
                    Decision::Nothing
                );
                assert_eq!(judged(Presented::Nothing), Decision::Nothing);
                assert_eq!(judged(Presented::Unreadable), Decision::Nothing);
            }
        }
    }

    #[test]
    fn reuse_after_closing_looks_at_whichever_last_credentials_exist() {
        let only_current = DeviceSnapshot {
            pending: None,
            max_generation: 5,
            ..in_state(DeviceState::Revoked)
        };
        assert_eq!(
            judge(
                &only_current,
                &Presented::Opened(credential(4, 0x44)),
                Mode::PresentationOnly
            ),
            Decision::ReuseAfterClosing
        );
        // Revoked before confirming anything, with two credentials issued: the first one,
        // discarded, is older than the last pending.
        let only_pending = DeviceSnapshot {
            state: DeviceState::Revoked,
            current: None,
            pending: Some(credential(2, 0x22)),
            max_generation: 2,
            marked_by_restoration: false,
        };
        assert_eq!(
            judge(
                &only_pending,
                &Presented::Opened(credential(1, 0x11)),
                Mode::PresentationOnly
            ),
            Decision::ReuseAfterClosing
        );
        assert_eq!(
            judge(
                &only_pending,
                &Presented::Opened(credential(2, 0x22)),
                Mode::PresentationOnly
            ),
            Decision::Nothing
        );
    }

    #[test]
    fn the_other_closed_states_never_alert_reuse() {
        // Denied, quarantined, suspended and pending are not "revoked, replaced or expired
        // for inactivity"; and a request that expired never had a current credential.
        for state in [
            DeviceState::Pending,
            DeviceState::Quarantined,
            DeviceState::Suspended,
            DeviceState::Denied,
        ] {
            assert_eq!(
                judge(
                    &in_state(state),
                    &Presented::Opened(credential(4, 0x44)),
                    Mode::PresentationOnly
                ),
                Decision::Nothing,
                "{state:?}"
            );
        }
        let expired_request = DeviceSnapshot {
            state: DeviceState::Expired,
            current: None,
            pending: Some(credential(2, 0x22)),
            max_generation: 2,
            marked_by_restoration: false,
        };
        assert_eq!(
            judge(
                &expired_request,
                &Presented::Opened(credential(1, 0x11)),
                Mode::PresentationOnly
            ),
            Decision::Nothing
        );
    }

    /// Second table, third line.
    #[test]
    fn a_seal_above_every_known_generation_is_proof_in_any_state_unless_marked() {
        let above = Presented::Opened(credential(7, 0x77));
        for state in ALL_STATES {
            for mode in [Mode::Swap, Mode::PresentationOnly] {
                let device = in_state(state);
                assert_eq!(
                    judge(&device, &above, mode),
                    Decision::RestorationProof,
                    "{state:?} {mode:?}"
                );
                let marked = DeviceSnapshot {
                    marked_by_restoration: true,
                    ..device
                };
                let expected = if state.serves() {
                    REUSE
                } else {
                    Decision::RecordOnly
                };
                assert_eq!(judge(&marked, &above, mode), expected, "{state:?} {mode:?}");
                assert_ne!(judge(&marked, &above, mode), Decision::RestorationProof);
            }
        }
    }

    #[test]
    fn the_mark_changes_nothing_else() {
        // "The rest still holds for it": the swap table and the reuse after closing.
        let presentations = [
            Presented::Nothing,
            Presented::Unreadable,
            Presented::Opened(credential(4, 0x44)),
            Presented::Opened(credential(5, 0x55)),
            Presented::Opened(credential(6, 0x66)),
        ];
        for state in ALL_STATES {
            for mode in [Mode::Swap, Mode::PresentationOnly] {
                for presented in &presentations {
                    let device = in_state(state);
                    let marked = DeviceSnapshot {
                        marked_by_restoration: true,
                        ..device
                    };
                    assert_eq!(
                        judge(&marked, presented, mode),
                        judge(&device, presented, mode),
                        "{state:?} {mode:?} {presented:?}"
                    );
                }
            }
        }
    }

    /// Second table, fourth and fifth lines.
    #[test]
    fn an_unknown_identity() {
        let seal = Presented::Opened(credential(3, 0x33));
        let cases: &[(&str, Presented, bool, bool, Decision)] = &[
            (
                "a valid seal, listed and not handled yet",
                seal,
                true,
                false,
                Decision::RestorationProof,
            ),
            (
                "a valid seal, of a listing a restored mode already handled",
                seal,
                false,
                false,
                Decision::RecordOnly,
            ),
            (
                "a valid seal, not handled, with the mode already on",
                seal,
                true,
                true,
                Decision::RecordOnly,
            ),
            (
                "a valid seal, handled, mode on",
                seal,
                false,
                true,
                Decision::RecordOnly,
            ),
            ("null", Presented::Nothing, false, false, Decision::Nothing),
            (
                "a seal that does not open",
                Presented::Unreadable,
                false,
                false,
                Decision::Nothing,
            ),
            (
                "null, mode on",
                Presented::Nothing,
                false,
                true,
                Decision::Nothing,
            ),
        ];
        for (name, presented, proof_pending, restored_mode, expected) in cases {
            assert_eq!(
                judge_unknown(presented, *proof_pending, *restored_mode),
                *expected,
                "{name}"
            );
        }
    }

    #[test]
    fn nothing_is_ever_accepted_outside_the_swap_of_a_serving_device() {
        let presentations = [
            Presented::Nothing,
            Presented::Unreadable,
            Presented::Opened(credential(1, 0x11)),
            Presented::Opened(credential(4, 0x44)),
            Presented::Opened(credential(5, 0x55)),
            Presented::Opened(credential(6, 0x66)),
            Presented::Opened(credential(7, 0x77)),
        ];
        let devices = [active(), approved()];
        for state in ALL_STATES {
            for base in &devices {
                for marked in [false, true] {
                    let device = DeviceSnapshot {
                        state,
                        marked_by_restoration: marked,
                        ..*base
                    };
                    for presented in &presentations {
                        let outside = judge(&device, presented, Mode::PresentationOnly);
                        assert!(
                            !matches!(outside, Decision::Accept { .. }),
                            "{device:?} {presented:?}"
                        );
                        if !state.serves() {
                            let swap = judge(&device, presented, Mode::Swap);
                            assert!(
                                !matches!(swap, Decision::Accept { .. } | Decision::Quarantine(_)),
                                "{device:?} {presented:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ the 24-hour swap

    #[test]
    fn the_rotation_only_issues_for_an_active_device_with_nothing_pending() {
        let ready = DeviceSnapshot {
            pending: None,
            max_generation: 5,
            ..active()
        };
        assert_eq!(rotation(&ready), ACCEPT);
        // A pending one is still waiting for its "saved".
        assert_eq!(rotation(&active()), Decision::Nothing);
        assert_eq!(rotation(&approved()), Decision::Nothing);
        for state in ALL_STATES {
            if state != DeviceState::Active {
                let device = DeviceSnapshot { state, ..ready };
                assert_eq!(rotation(&device), Decision::Nothing, "{state:?}");
            }
        }
    }

    // ------------------------------------------------------------------ issuing

    #[test]
    fn the_next_generation_is_one_above_the_largest_ever_issued() {
        assert_eq!(next_generation(&approved()).unwrap().get(), 1);
        assert_eq!(next_generation(&active()).unwrap().get(), 7);
        // The largest ever, not the current one: a discarded pending is not numbered again.
        let discarded = DeviceSnapshot {
            pending: None,
            max_generation: 9,
            ..active()
        };
        assert_eq!(next_generation(&discarded).unwrap().get(), 10);
        let full = DeviceSnapshot {
            max_generation: u64::MAX >> 1,
            ..active()
        };
        assert_eq!(
            next_generation(&full),
            Err(TransitionError::GenerationExhausted)
        );
        let overflowing = DeviceSnapshot {
            max_generation: u64::MAX,
            ..active()
        };
        assert_eq!(
            next_generation(&overflowing),
            Err(TransitionError::GenerationExhausted)
        );
    }

    #[test]
    fn issuing_without_promotion_discards_the_pending_one() {
        let new = credential(7, 0x77);
        let after = issue(&active(), false, new).unwrap();
        assert!(!after.first_confirmation);
        assert_eq!(
            after.device,
            DeviceSnapshot {
                pending: Some(new),
                max_generation: 7,
                ..active()
            }
        );
        // The discarded pending one is now "another one already issued".
        assert_eq!(
            judge(
                &after.device,
                &Presented::Opened(credential(6, 0x66)),
                Mode::Swap
            ),
            REUSE
        );
    }

    #[test]
    fn issuing_with_promotion_kills_the_current_one() {
        let new = credential(7, 0x77);
        let after = issue(&active(), true, new).unwrap();
        assert!(!after.first_confirmation);
        assert_eq!(
            after.device,
            DeviceSnapshot {
                current: Some(credential(6, 0x66)),
                pending: Some(new),
                max_generation: 7,
                ..active()
            }
        );
        assert_eq!(
            judge(
                &after.device,
                &Presented::Opened(credential(5, 0x55)),
                Mode::Swap
            ),
            REUSE
        );
    }

    #[test]
    fn the_first_credential_stays_at_generation_zero_until_confirmed() {
        let first = credential(1, 0x11);
        let issued = issue(&approved(), false, first).unwrap();
        assert!(!issued.first_confirmation);
        assert_eq!(issued.device.state, DeviceState::Approved);
        assert_eq!(issued.device.current, None);
        assert_eq!(issued.device.pending, Some(first));
        // Still generation zero: null is accepted again, and the first one is discarded.
        assert_eq!(
            judge(&issued.device, &Presented::Nothing, Mode::Swap),
            ACCEPT
        );
    }

    #[test]
    fn presenting_the_first_credential_is_its_confirmation() {
        let first = credential(1, 0x11);
        let issued = issue(&approved(), false, first).unwrap().device;
        let second = credential(2, 0x22);
        let after = issue(&issued, true, second).unwrap();
        assert!(after.first_confirmation);
        assert_eq!(after.device.state, DeviceState::Active);
        assert_eq!(after.device.current, Some(first));
        assert_eq!(after.device.pending, Some(second));
        // Past generation zero for good.
        assert_eq!(judge(&after.device, &Presented::Nothing, Mode::Swap), REUSE);
    }

    #[test]
    fn issuing_refuses_what_would_break_the_line() {
        // A generation that is not new.
        for generation in [1, 5, 6] {
            assert_eq!(
                issue(&active(), false, credential(generation, 0x77)),
                Err(TransitionError::GenerationNotNew),
                "{generation}"
            );
        }
        // Nothing to promote.
        let no_pending = DeviceSnapshot {
            pending: None,
            ..active()
        };
        assert_eq!(
            issue(&no_pending, true, credential(7, 0x77)),
            Err(TransitionError::NoPending)
        );
        // A device that stopped serving while the connection waited.
        for state in ALL_STATES {
            if !state.serves() {
                assert_eq!(
                    issue(&in_state(state), false, credential(7, 0x77)),
                    Err(TransitionError::NotServing),
                    "{state:?}"
                );
            }
        }
    }

    // ------------------------------------------------------------------ the "saved"

    #[test]
    fn the_saved_promotes_the_pending_credential_of_this_connection() {
        let pending = credential(6, 0x66);
        let after = confirm(&active(), &pending, &pending).unwrap();
        assert!(!after.first_confirmation);
        assert_eq!(
            after.device,
            DeviceSnapshot {
                current: Some(pending),
                pending: None,
                max_generation: 6,
                ..active()
            }
        );
        // The one that was current is dead.
        assert_eq!(
            judge(
                &after.device,
                &Presented::Opened(credential(5, 0x55)),
                Mode::Swap
            ),
            REUSE
        );
    }

    #[test]
    fn the_first_saved_makes_the_device_active() {
        let first = credential(1, 0x11);
        let issued = issue(&approved(), false, first).unwrap().device;
        let after = confirm(&issued, &first, &first).unwrap();
        assert!(after.first_confirmation);
        assert_eq!(after.device.state, DeviceState::Active);
        assert_eq!(after.device.current, Some(first));
        assert_eq!(after.device.pending, None);
        assert_eq!(after.device.max_generation, 1);
    }

    #[test]
    fn a_saved_that_is_not_about_the_pending_one_is_refused() {
        let pending = credential(6, 0x66);
        let cases: &[(&str, Credential, Credential)] = &[
            // The connector answers about something else than it received.
            ("another digest", pending, credential(6, 0x99)),
            ("another generation", pending, credential(7, 0x66)),
            ("the current one", pending, credential(5, 0x55)),
            // This connection received a credential that is no longer the pending one:
            // another connection was accepted in between and discarded it.
            (
                "a credential discarded since",
                credential(4, 0x44),
                credential(4, 0x44),
            ),
            (
                "the current one issued long ago",
                credential(5, 0x55),
                credential(5, 0x55),
            ),
            (
                "the number of the pending one with another digest",
                credential(6, 0x99),
                credential(6, 0x99),
            ),
        ];
        for (name, issued_here, saved) in cases {
            assert_eq!(
                confirm(&active(), issued_here, saved),
                Err(TransitionError::NotThePending),
                "{name}"
            );
        }
        let no_pending = DeviceSnapshot {
            pending: None,
            ..active()
        };
        assert_eq!(
            confirm(&no_pending, &pending, &pending),
            Err(TransitionError::NotThePending)
        );
    }

    #[test]
    fn a_device_revoked_before_the_saved_does_not_come_back() {
        let pending = credential(6, 0x66);
        for state in ALL_STATES {
            if !state.serves() {
                assert_eq!(
                    confirm(&in_state(state), &pending, &pending),
                    Err(TransitionError::NotServing),
                    "{state:?}"
                );
            }
        }
    }

    #[test]
    fn the_generation_only_grows_along_any_line_of_swaps() {
        // Issue, confirm, issue twice (one discarded), promote by presentation, confirm.
        let mut device = approved();
        let mut seen = Vec::new();
        let mut step = |device: &mut DeviceSnapshot, promote: bool| {
            let generation = next_generation(device).unwrap();
            let new = Credential {
                generation,
                digest: CredentialDigest::from_bytes([generation.get() as u8; 32]),
            };
            *device = issue(device, promote, new).unwrap().device;
            seen.push(generation.get());
            new
        };
        let first = step(&mut device, false);
        device = confirm(&device, &first, &first).unwrap().device;
        step(&mut device, false);
        step(&mut device, false);
        let fourth = step(&mut device, true);
        device = confirm(&device, &fourth, &fourth).unwrap().device;
        assert_eq!(seen, [1, 2, 3, 4]);
        assert_eq!(device.max_generation, 4);
        assert_eq!(device.current, Some(fourth));
        assert_eq!(device.pending, None);
    }
}
