//! The states of a number (`docs/10-maquinas-de-estado.md`, section 4), as a pure function:
//! the state and what happened give the next state, or nothing when nothing changes.
//!
//! ```text
//! unpaired ─pair─> pairing ─working, the expected telephone─> connected <─> reconnecting
//!    ^               │  └──working, another telephone──> number_swapped ─confirm─> connected
//!    └──expired──────┘                                        └─unpair─> unpaired
//! connected, reconnecting ─the device was removed, or the admin disconnected─> disconnected
//! connected, reconnecting ─failed─> failed ─restart─> reconnecting
//! anything ─remove─> removed
//! ```
//!
//! "Unknown" is not a state: it is what a screen shows when the engine does not answer.

use crate::engine::EngineStatus;

/// The state of a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberState {
    /// Added, never paired, or paired and then let go.
    Unpaired,
    /// A QR or a code of the telephone is being shown.
    Pairing,
    /// Connected to WhatsApp.
    Connected,
    /// The engine lost the connection with WhatsApp and is trying again.
    Reconnecting,
    /// The connected device was removed on the telephone, or the admin disconnected it.
    Disconnected,
    /// The telephone that paired is not the one that was added: the accesses wait for the
    /// admin.
    NumberSwapped,
    /// The engine reported a failure of the session of the engine.
    Failed,
    /// The admin removed it. Final.
    Removed,
}

impl NumberState {
    /// Every state, in the order of the documentation.
    pub const ALL: &'static [NumberState] = &[
        NumberState::Unpaired,
        NumberState::Pairing,
        NumberState::Connected,
        NumberState::Reconnecting,
        NumberState::Disconnected,
        NumberState::NumberSwapped,
        NumberState::Failed,
        NumberState::Removed,
    ];

    /// The text stored in `number.state`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            NumberState::Unpaired => "unpaired",
            NumberState::Pairing => "pairing",
            NumberState::Connected => "connected",
            NumberState::Reconnecting => "reconnecting",
            NumberState::Disconnected => "disconnected",
            NumberState::NumberSwapped => "number_swapped",
            NumberState::Failed => "failed",
            NumberState::Removed => "removed",
        }
    }

    /// Reads the stored text.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|state| state.as_str() == text)
    }

    /// Whether a call that depends on the engine may go to it (`docs/08-permissoes.md`,
    /// section 3, rule 6). Any other state answers AR-702.
    #[must_use]
    pub const fn serves_calls(self) -> bool {
        matches!(self, NumberState::Connected)
    }
}

/// What happened to a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The admin asks for the QR or the code.
    Pair,
    /// The QR or the code expired unused.
    PairingExpired,
    /// The engine reports this status of the session. `expected_phone` says whether the
    /// account that is connected is the telephone that was added (with or without the ninth
    /// digit); `None` when the engine did not say which account.
    Status {
        /// The status.
        status: EngineStatus,
        /// Whether the connected account is the expected telephone.
        expected_phone: Option<bool>,
    },
    /// The admin disconnects it.
    Disconnect,
    /// The admin confirms the new telephone of a swapped number.
    Confirm,
    /// The admin undoes the pairing of a swapped number.
    Unpair,
    /// The admin restarts a failed session of the engine.
    Restart,
    /// The admin removes it.
    Remove,
}

/// The state after `trigger`, or `None` when it does not change the state (or does not apply
/// to it).
#[must_use]
pub fn next(state: NumberState, trigger: Trigger) -> Option<NumberState> {
    use EngineStatus as S;
    use NumberState as N;
    let next = match (state, trigger) {
        (N::Removed, _) => return None,
        (_, Trigger::Remove) => N::Removed,
        (N::Unpaired | N::Disconnected | N::Pairing, Trigger::Pair) => N::Pairing,
        (N::Pairing, Trigger::PairingExpired) => N::Unpaired,
        (
            N::Pairing,
            Trigger::Status {
                status: S::Working,
                expected_phone,
            },
        ) => {
            match expected_phone {
                Some(true) => N::Connected,
                // An account that is another telephone is not taken for the expected one:
                // the admin decides.
                Some(false) => N::NumberSwapped,
                // The engine says it works and does not say which telephone yet: nothing is
                // decided on a telephone nobody read. The pairing goes on until it is told.
                None => return None,
            }
        }
        (
            N::Reconnecting,
            Trigger::Status {
                status: S::Working, ..
            },
        ) => N::Connected,
        (
            N::Connected,
            Trigger::Status {
                status: S::Starting,
                ..
            },
        ) => N::Reconnecting,
        (
            N::Connected | N::Reconnecting,
            Trigger::Status {
                status: S::ScanQrCode | S::PasskeyRequired,
                ..
            },
        ) => N::Disconnected,
        (
            N::Connected | N::Reconnecting,
            Trigger::Status {
                status: S::Failed, ..
            },
        ) => N::Failed,
        (N::Connected | N::Reconnecting, Trigger::Disconnect) => N::Disconnected,
        (N::NumberSwapped, Trigger::Confirm) => N::Connected,
        (N::NumberSwapped, Trigger::Unpair) => N::Unpaired,
        (N::Failed, Trigger::Restart) => N::Reconnecting,
        // The engine recovered by itself, with the telephone the hub expects.
        (
            N::Failed,
            Trigger::Status {
                status: S::Working,
                expected_phone: Some(true),
            },
        ) => N::Connected,
        _ => return None,
    };
    (next != state || next == N::Pairing).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(status: EngineStatus, expected_phone: Option<bool>) -> Trigger {
        Trigger::Status {
            status,
            expected_phone,
        }
    }

    #[test]
    fn every_state_has_its_own_text_and_reads_back() {
        for state in NumberState::ALL {
            assert_eq!(NumberState::parse(state.as_str()), Some(*state));
        }
        assert_eq!(NumberState::parse("CONNECTED"), None);
        const MIGRATION: &str = include_str!("../../migrations/0010_whatsapp.sql");
        for state in NumberState::ALL {
            assert!(
                MIGRATION.contains(&format!("'{}'", state.as_str())),
                "{state:?}"
            );
        }
    }

    #[test]
    fn the_table_of_the_documentation() {
        use EngineStatus as S;
        use NumberState as N;
        let table = [
            (N::Unpaired, Trigger::Pair, Some(N::Pairing)),
            (N::Disconnected, Trigger::Pair, Some(N::Pairing)),
            (
                N::Pairing,
                status(S::Working, Some(true)),
                Some(N::Connected),
            ),
            (
                N::Pairing,
                status(S::Working, Some(false)),
                Some(N::NumberSwapped),
            ),
            (N::Pairing, status(S::Working, None), None),
            (
                N::Failed,
                status(S::Working, Some(true)),
                Some(N::Connected),
            ),
            (N::Pairing, Trigger::PairingExpired, Some(N::Unpaired)),
            (
                N::Connected,
                status(S::Starting, None),
                Some(N::Reconnecting),
            ),
            (
                N::Reconnecting,
                status(S::Working, None),
                Some(N::Connected),
            ),
            (
                N::Connected,
                status(S::ScanQrCode, None),
                Some(N::Disconnected),
            ),
            (
                N::Reconnecting,
                status(S::ScanQrCode, None),
                Some(N::Disconnected),
            ),
            (N::Connected, Trigger::Disconnect, Some(N::Disconnected)),
            (N::NumberSwapped, Trigger::Confirm, Some(N::Connected)),
            (N::NumberSwapped, Trigger::Unpair, Some(N::Unpaired)),
            (N::Connected, status(S::Failed, None), Some(N::Failed)),
            (N::Reconnecting, status(S::Failed, None), Some(N::Failed)),
            (N::Failed, Trigger::Restart, Some(N::Reconnecting)),
        ];
        for (state, trigger, expected) in table {
            assert_eq!(next(state, trigger), expected, "{state:?} + {trigger:?}");
        }
        for state in NumberState::ALL {
            let expected = (*state != N::Removed).then_some(N::Removed);
            assert_eq!(next(*state, Trigger::Remove), expected, "{state:?}");
        }
    }

    #[test]
    fn what_the_documentation_does_not_list_changes_nothing() {
        use EngineStatus as S;
        use NumberState as N;
        for (state, trigger) in [
            (N::Connected, status(S::Working, Some(true))),
            (N::Connected, Trigger::Pair),
            (N::Unpaired, status(S::Working, Some(true))),
            (N::Disconnected, status(S::Working, Some(true))),
            (N::NumberSwapped, status(S::Working, Some(true))),
            (N::Failed, status(S::Working, None)),
            (N::Connected, Trigger::Confirm),
            (N::Unpaired, Trigger::Disconnect),
            (N::Connected, Trigger::Restart),
            (N::Connected, Trigger::Unpair),
            (N::Removed, Trigger::Pair),
            (N::Removed, status(S::Working, Some(true))),
            (N::Pairing, status(S::Stopped, None)),
            (N::Connected, status(S::Stopped, None)),
        ] {
            assert_eq!(next(state, trigger), None, "{state:?} + {trigger:?}");
        }
    }

    #[test]
    fn only_a_connected_number_serves_calls_to_the_engine() {
        for state in NumberState::ALL {
            assert_eq!(
                state.serves_calls(),
                *state == NumberState::Connected,
                "{state:?}"
            );
        }
    }
}
