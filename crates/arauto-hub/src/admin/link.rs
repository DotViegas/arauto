//! `arauto-hub admin link ...`: the list of peers (`docs/13-conversa-entre-hermes.md`,
//! section 2).
//!
//! A link says "from A to B": A may send messages to B, and, with the wake mark, a message of
//! A wakes B. The rules of the other commands hold: every write goes through the
//! repositories, which check the database against the audit mark first; every change is a
//! security event; the actor is the local admin. The hub hears of each change, and the
//! sessions of the two Hermes get their new list of tools.
//!
//! When a link is created the command warns, and never prevents: it shows the numbers of
//! WhatsApp each side reaches and with which verbs, because the link lets a request cross
//! from one number to another, and it reminds that a message is data, never an order.

use arauto_protocol::names::HermesName;

use super::{AdminError, Session};
use crate::repo;
use crate::repo::peer::{LinkLine, LinkMissing, LinkRemoved, LinkSet};

/// What one direction of `admin link add` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkChange {
    /// Who may send.
    pub from: HermesName,
    /// Who receives.
    pub to: HermesName,
    /// The wake mark asked.
    pub wake: bool,
    /// What was done.
    pub outcome: LinkSet,
}

/// The numbers one Hermes reaches: the alias and the verbs, as words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    /// The Hermes.
    pub hermes: HermesName,
    /// For each number, its alias and its verbs.
    pub numbers: Vec<(String, Vec<&'static str>)>,
}

/// What `admin link add` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    /// Each direction asked.
    pub changes: Vec<LinkChange>,
    /// What each side reaches on WhatsApp, shown when a link was created.
    pub reach: Vec<Reach>,
}

/// What one direction of `admin link remove` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlinked {
    /// Who could send.
    pub from: HermesName,
    /// Who received.
    pub to: HermesName,
    /// The link was there and is gone.
    pub removed: bool,
}

fn name(text: &str) -> Result<HermesName, AdminError> {
    HermesName::parse(text).map_err(|_| AdminError::NoSuchHermes)
}

fn missing(_: LinkMissing) -> AdminError {
    AdminError::NoSuchHermes
}

impl Session {
    /// `admin link add FROM TO [--wake] [--both]`: the link from `from` to `to`, and with
    /// `both` the one back too, each with the wake mark asked.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::LinkToItself`]; [`AdminError::Restored`].
    pub async fn link_add(
        &self,
        from: &str,
        to: &str,
        wake: bool,
        both: bool,
        now: i64,
    ) -> Result<Linked, AdminError> {
        let from = name(from)?;
        let to = name(to)?;
        let mut directions = vec![(from.clone(), to.clone())];
        if both {
            directions.push((to.clone(), from.clone()));
        }
        let mut changes = Vec::new();
        for (from, to) in directions {
            let outcome = repo::peer::link_set(
                &self.pool,
                &self.mark,
                self.organization,
                &self.admin,
                &from,
                &to,
                wake,
                now,
            )
            .await?;
            match outcome {
                LinkSet::Missing(which) => return Err(missing(which)),
                LinkSet::SameHermes => return Err(AdminError::LinkToItself),
                LinkSet::Created | LinkSet::Changed | LinkSet::Unchanged => {}
            }
            changes.push(LinkChange {
                from,
                to,
                wake,
                outcome,
            });
        }
        let mut reach = Vec::new();
        if changes
            .iter()
            .any(|change| change.outcome == LinkSet::Created)
        {
            for hermes in [from, to] {
                let numbers =
                    repo::access::reach_of(&self.pool, self.organization, &hermes).await?;
                reach.push(Reach { hermes, numbers });
            }
        }
        Ok(Linked { changes, reach })
    }

    /// `admin link remove FROM TO [--both]`.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::Restored`].
    pub async fn link_remove(
        &self,
        from: &str,
        to: &str,
        both: bool,
        now: i64,
    ) -> Result<Vec<Unlinked>, AdminError> {
        let from = name(from)?;
        let to = name(to)?;
        let mut directions = vec![(from.clone(), to.clone())];
        if both {
            directions.push((to, from));
        }
        let mut done = Vec::new();
        for (from, to) in directions {
            let outcome = repo::peer::link_remove(
                &self.pool,
                &self.mark,
                self.organization,
                &self.admin,
                &from,
                &to,
                now,
            )
            .await?;
            let removed = match outcome {
                LinkRemoved::Removed => true,
                LinkRemoved::NoLink => false,
                LinkRemoved::Missing(which) => return Err(missing(which)),
            };
            done.push(Unlinked { from, to, removed });
        }
        Ok(done)
    }

    /// `admin link list`.
    ///
    /// # Errors
    /// [`AdminError::Repo`] when the database refuses.
    pub async fn link_list(&self) -> Result<Vec<LinkLine>, AdminError> {
        Ok(repo::peer::links(&self.pool, self.organization).await?)
    }
}
