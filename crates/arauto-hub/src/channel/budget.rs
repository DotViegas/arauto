//! The byte budget of the whole hub for messages under reassembly
//! (`docs/14-protocolo-do-canal.md`, section 4: 256 MiB to start with).
//!
//! Every reassembler of the hub takes from it before it keeps a byte and gives back when the
//! message is delivered, when the connection fails and when it is dropped. Without room, the
//! connection that asks for more is closed with AR-205; the others go on.

use std::sync::atomic::{AtomicUsize, Ordering};

use arauto_protocol::frame::ReassemblyBudget;

/// The budget, shared by every connection.
#[derive(Debug)]
pub(crate) struct HubBudget {
    capacity: usize,
    used: AtomicUsize,
}

impl HubBudget {
    pub(crate) const fn new(capacity: usize) -> Self {
        Self {
            capacity,
            used: AtomicUsize::new(0),
        }
    }

    /// Bytes taken right now.
    #[cfg(test)]
    pub(crate) fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }
}

impl ReassemblyBudget for HubBudget {
    fn try_reserve(&self, bytes: usize) -> bool {
        self.used
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(bytes)
                    .filter(|after| *after <= self.capacity)
            })
            .is_ok()
    }

    fn release(&self, bytes: usize) {
        // Never below zero, even if a caller gave back more than it took.
        let _ = self
            .used
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                Some(used.saturating_sub(bytes))
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_does_not_fit_is_refused_and_nothing_is_taken() {
        let budget = HubBudget::new(100);
        assert!(budget.try_reserve(60));
        assert!(!budget.try_reserve(41));
        assert_eq!(budget.used(), 60);
        assert!(budget.try_reserve(40));
        assert!(!budget.try_reserve(1));
        budget.release(100);
        assert_eq!(budget.used(), 0);
        assert!(!budget.try_reserve(usize::MAX));
        budget.release(5);
        assert_eq!(budget.used(), 0);
    }
}
