//! The audit of the writes refused in the chat where a Hermes takes orders: one row per Hermes,
//! number and chat per hour, never one per refused call.
//!
//! The memory is the process: a restart forgets it, so at most one extra row per key per
//! restart. It is a map pruned past a few thousand entries, as the ceilings of the messages
//! between Hermes are (`crate::peer`). No table.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use arauto_protocol::limits::HOUR_MS;
use uuid::Uuid;

/// A Hermes, a number and what the refusal is about.
type Key = (Uuid, Uuid, Uuid);

/// When each key was last audited, in milliseconds of the hub.
type Seen = Mutex<HashMap<Key, i64>>;

/// How many keys are remembered before the idle ones are dropped.
const KEYS_BEFORE_PRUNE: usize = 4_096;

/// Whether this is the first refusal of `hermes` on `number`, about `subject`, in this hour.
/// `subject` is the chat of the hub; for a sending by telephone to a chat the hub does not
/// have, the row of the record that matched.
pub(super) fn first_of_the_hour(hermes: Uuid, number: Uuid, subject: Uuid, now: i64) -> bool {
    static SEEN: OnceLock<Seen> = OnceLock::new();
    first_in(
        SEEN.get_or_init(Mutex::default),
        (hermes, number, subject),
        now,
    )
}

fn first_in(seen: &Seen, key: Key, now: i64) -> bool {
    // Plain numbers: a panic elsewhere leaves them usable.
    let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    if seen.len() > KEYS_BEFORE_PRUNE {
        seen.retain(|_, at| now.saturating_sub(*at) < HOUR_MS);
    }
    match seen.get(&key) {
        Some(at) if now.saturating_sub(*at) < HOUR_MS && now >= *at => false,
        _ => {
            seen.insert(key, now);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_is_audited_once_an_hour_per_hermes_number_and_chat() {
        let (hermes, number, chat) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        assert!(first_of_the_hour(hermes, number, chat, 1_000));
        assert!(!first_of_the_hour(hermes, number, chat, 2_000));
        assert!(!first_of_the_hour(
            hermes,
            number,
            chat,
            1_000 + HOUR_MS - 1
        ));
        assert!(first_of_the_hour(hermes, number, chat, 1_000 + HOUR_MS));
        // Another Hermes, another number or another chat is another key.
        assert!(first_of_the_hour(Uuid::new_v4(), number, chat, 2_000));
        assert!(first_of_the_hour(hermes, Uuid::new_v4(), chat, 2_000));
        assert!(first_of_the_hour(hermes, number, Uuid::new_v4(), 2_000));
    }

    #[test]
    fn the_idle_keys_are_dropped_past_the_ceiling_of_the_map() {
        let seen = Mutex::default();
        let first = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        assert!(first_in(&seen, first, 0));
        for _ in 0..=KEYS_BEFORE_PRUNE {
            first_in(&seen, (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()), 0);
        }
        // An hour later the old keys go, and a refusal of the first key is news again.
        assert!(first_in(&seen, first, HOUR_MS + 1));
        assert!(seen.lock().unwrap().len() <= 2);
    }
}
