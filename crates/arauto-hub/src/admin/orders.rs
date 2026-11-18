//! `arauto-hub admin hermes whatsapp NAME ...`: who gives a Hermes orders on WhatsApp
//! (decision 0051, a proposal).
//!
//! The admin records the telephone the Hermes's own WhatsApp bridge runs on, and who gives it
//! orders there: its own WhatsApp in the chat with itself, or a list of telephones. The hub
//! then refuses every Hermes writing in that chat, for as long as the record is there. The
//! rules of the other commands hold: the write goes through the repository, which checks the
//! database against the audit mark first, and the actor is the local admin.
//!
//! The command never asks: an agent runs it. It prints the risk instead (`render::orders`).
//! A telephone typed here is never printed back.

use arauto_protocol::names::HermesName;

use super::{AdminError, Session};
use crate::engine::Phone;
use crate::repo;
use crate::repo::orders::{Invalid, OrdersRecord, SetOutcome};
use crate::vault::Vault;

/// What the flags of the command ask for. A flag left out changes nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrdersEdit {
    /// `--takes-orders-on PHONE|none`: `Some(None)` clears it.
    pub takes_orders_on: Option<Option<String>>,
    /// `--self-chat`.
    pub self_chat: bool,
    /// `--orders-from PHONE` (repeated; `none` clears the list): `Some(vec![])` clears it.
    pub orders_from: Option<Vec<String>>,
    /// `--orders-outside-whatsapp`.
    pub outside_whatsapp: bool,
}

impl OrdersEdit {
    /// Whether no flag was given: the command only shows the record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.takes_orders_on.is_none()
            && !self.self_chat
            && self.orders_from.is_none()
            && !self.outside_whatsapp
    }
}

/// What the command did, for what it prints.
#[derive(Debug, Clone)]
pub struct OrdersOutcome {
    /// The Hermes.
    pub hermes: HermesName,
    /// The record before the command.
    pub before: OrdersRecord,
    /// The record now.
    pub after: OrdersRecord,
    /// Whether something was written (false when only shown, or when nothing changed).
    pub written: bool,
}

fn is_none(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("none")
}

fn phone(text: &str) -> Result<Phone, AdminError> {
    Phone::parse(text).map_err(|_| AdminError::InvalidPhone)
}

/// The edit with its telephones parsed: a text that is not one refuses the whole command.
/// The telephones of an edit, parsed: the bridge (`Some(None)` clears it) and the list.
type Parsed = (Option<Option<Phone>>, Option<Vec<Phone>>);

fn parsed(edit: &OrdersEdit) -> Result<Parsed, AdminError> {
    let takes = match &edit.takes_orders_on {
        None => None,
        Some(None) => Some(None),
        Some(Some(text)) if is_none(text) => Some(None),
        Some(Some(text)) => Some(Some(phone(text)?)),
    };
    let from = match &edit.orders_from {
        None => None,
        Some(texts) => {
            let mut list = Vec::new();
            for text in texts {
                if is_none(text) {
                    continue;
                }
                list.push(phone(text)?);
            }
            Some(list)
        }
    };
    Ok((takes, from))
}

/// Outside WhatsApp with a telephone, self-chat or a list is two answers: refused before
/// anything else.
fn check_outside(
    edit: &OrdersEdit,
    takes: Option<&Option<Phone>>,
    from: Option<&[Phone]>,
) -> Result<(), AdminError> {
    if edit.outside_whatsapp
        && (matches!(takes, Some(Some(_)))
            || edit.self_chat
            || from.is_some_and(|list| !list.is_empty()))
    {
        return Err(AdminError::OrdersRule(Invalid::OutsideWithATelephone));
    }
    Ok(())
}

/// What an edit makes of the record a Hermes has. The one place that says it: the write and
/// the preview of the panel both go through it.
fn applied(
    before: &OrdersRecord,
    takes: Option<&Option<Phone>>,
    from: Option<&[Phone]>,
    self_chat: bool,
    outside: bool,
) -> OrdersRecord {
    let mut after = before.clone();
    if let Some(takes) = takes {
        after.takes_orders_on.clone_from(takes);
        if takes.is_some() {
            after.outside_whatsapp = false;
        }
    }
    if let Some(from) = from {
        after.self_chat = false;
        after.outside_whatsapp = false;
        after.orders_from = from.to_vec();
    }
    if self_chat {
        after.self_chat = true;
        after.orders_from.clear();
        after.outside_whatsapp = false;
    }
    if outside {
        after.takes_orders_on = None;
        after.self_chat = false;
        after.orders_from.clear();
        after.outside_whatsapp = true;
    }
    after
}

/// The record `edit` would make of `before`, without writing anything, normalized as the write
/// checks it: what the panel needs to count, check and know whether a save protects fewer
/// chats. Rules of the record are checked by the caller (`OrdersRecord::check`).
///
/// # Errors
/// [`AdminError::InvalidPhone`]; [`AdminError::OrdersRule`] for `self-chat` with a list.
pub fn preview(before: &OrdersRecord, edit: &OrdersEdit) -> Result<OrdersRecord, AdminError> {
    let (takes, from) = parsed(edit)?;
    check_outside(edit, takes.as_ref(), from.as_deref())?;
    if edit.self_chat && from.as_ref().is_some_and(|list| !list.is_empty()) {
        return Err(AdminError::OrdersRule(Invalid::SelfChatWithAList));
    }
    Ok(applied(
        before,
        takes.as_ref(),
        from.as_deref(),
        edit.self_chat,
        edit.outside_whatsapp,
    )
    .normalized())
}

impl Session {
    /// `admin hermes whatsapp NAME`: the record of a Hermes.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`].
    pub async fn hermes_orders_show(
        &self,
        vault: &Vault,
        name: &str,
    ) -> Result<OrdersOutcome, AdminError> {
        let name = HermesName::parse(name).map_err(|_| AdminError::NoSuchHermes)?;
        let line = repo::hermes::list(&self.pool, self.organization, false)
            .await?
            .into_iter()
            .find(|line| line.hermes.name == name)
            .ok_or(AdminError::NoSuchHermes)?;
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        let record = repo::orders::get(&self.pool, self.organization, &key, line.hermes.id).await?;
        Ok(OrdersOutcome {
            hermes: name,
            before: record.clone(),
            after: record,
            written: false,
        })
    }

    /// `admin hermes whatsapp NAME --takes-orders-on ... --self-chat | --orders-from ...`: the
    /// record of a Hermes, replaced whole in one security write.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::InvalidPhone`];
    /// [`AdminError::OrdersRule`]; [`AdminError::OrdersTelephoneTaken`];
    /// [`AdminError::Restored`].
    pub async fn hermes_orders_set(
        &self,
        vault: &Vault,
        name: &str,
        edit: &OrdersEdit,
        now: i64,
    ) -> Result<OrdersOutcome, AdminError> {
        let name = HermesName::parse(name).map_err(|_| AdminError::NoSuchHermes)?;
        let (takes, from) = parsed(edit)?;
        check_outside(edit, takes.as_ref(), from.as_deref())?;
        let self_chat = edit.self_chat;
        let outside = edit.outside_whatsapp;
        if self_chat && from.as_ref().is_some_and(|list| !list.is_empty()) {
            return Err(AdminError::OrdersRule(Invalid::SelfChatWithAList));
        }
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        let outcome = repo::orders::set(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &key,
            &name,
            |before| applied(before, takes.as_ref(), from.as_deref(), self_chat, outside),
            now,
        )
        .await?;
        match outcome {
            SetOutcome::Set { before, after } => Ok(OrdersOutcome {
                hermes: name,
                before,
                after,
                written: true,
            }),
            SetOutcome::Unchanged(record) => Ok(OrdersOutcome {
                hermes: name,
                before: record.clone(),
                after: record,
                written: false,
            }),
            SetOutcome::NoSuchHermes => Err(AdminError::NoSuchHermes),
            SetOutcome::Invalid(rule) => Err(AdminError::OrdersRule(rule)),
            SetOutcome::TelephoneTaken(other) => {
                Err(AdminError::OrdersTelephoneTaken(other.as_str().to_owned()))
            }
        }
    }

    /// For `admin number list`: how each number takes part in the records, and whether some
    /// Hermes protects a chat at all.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn orders_for_the_list(&self) -> Result<OrdersOfNumbers, AdminError> {
        Ok(OrdersOfNumbers {
            tags: repo::orders::tags(&self.pool, self.organization).await?,
            some_hermes_is_recorded: repo::orders::some_hermes_is_recorded(
                &self.pool,
                self.organization,
            )
            .await?,
        })
    }
}

/// What `admin number list` says about orders.
#[derive(Debug, Clone, Default)]
pub struct OrdersOfNumbers {
    /// Each number that takes part in a record.
    pub tags: Vec<repo::orders::Tag>,
    /// Whether some Hermes has an answer that counts.
    pub some_hermes_is_recorded: bool,
}

/// The text of a rule of the record, for the admin.
#[must_use]
pub const fn rule_text(rule: Invalid) -> &'static str {
    match rule {
        Invalid::NoTelephoneOfTheHermes => {
            "A list of telephones, or self-chat, needs the telephone the Hermes's WhatsApp runs \
             on: record it with --takes-orders-on (or clear the list with --orders-from none)"
        }
        Invalid::SelfChatWithAList => {
            "self-chat and a list of telephones are two answers to the same question: give one"
        }
        Invalid::BridgeAmongGivers => {
            "The telephone the Hermes's WhatsApp runs on cannot also give it orders: that chat is the self-chat mode (use --self-chat), or leave it out of --orders-from"
        }
        Invalid::TooMany => "At most 16 telephones can give a Hermes orders",
        Invalid::OutsideWithATelephone => {
            "Outside WhatsApp and a telephone of a WhatsApp are two answers: give --orders-outside-whatsapp alone (it clears the telephones), or the telephones without it"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_of_the_bridge_is_said_to_the_command() {
        let text = AdminError::OrdersRule(Invalid::BridgeAmongGivers).to_string();
        assert!(text.contains("--self-chat"), "{text}");
        assert!(text.contains("--orders-from"), "{text}");
    }

    #[test]
    fn a_preview_is_normalized_as_the_write_checks_it() {
        let before = OrdersRecord::default();
        let mut lines: Vec<String> = (0..16).map(|n| format!("+55119000001{n:02}")).collect();
        lines.push("+5511987654321".to_owned());
        lines.push("+551187654321".to_owned());
        let edit = OrdersEdit {
            takes_orders_on: Some(Some("+5511900000001".to_owned())),
            self_chat: false,
            orders_from: Some(lines[1..].to_vec()),
            outside_whatsapp: false,
        };
        let after = preview(&before, &edit).unwrap();
        assert_eq!(after.orders_from.len(), 16, "one in both forms counts once");
        assert_eq!(after.check(), Ok(()));
    }

    #[test]
    fn orders_from_outside_whatsapp_take_no_telephone() {
        let before = OrdersRecord::default();
        let phone_text = "+5511900000001".to_owned();
        let outside = OrdersEdit {
            outside_whatsapp: true,
            ..OrdersEdit::default()
        };
        let with_takes = OrdersEdit {
            takes_orders_on: Some(Some(phone_text.clone())),
            ..outside.clone()
        };
        assert!(matches!(
            preview(&before, &with_takes),
            Err(AdminError::OrdersRule(Invalid::OutsideWithATelephone))
        ));
        let with_list = OrdersEdit {
            orders_from: Some(vec!["+5511900000002".to_owned()]),
            ..outside.clone()
        };
        assert!(matches!(
            preview(&before, &with_list),
            Err(AdminError::OrdersRule(Invalid::OutsideWithATelephone))
        ));
        let with_self = OrdersEdit {
            self_chat: true,
            ..outside.clone()
        };
        assert!(matches!(
            preview(&before, &with_self),
            Err(AdminError::OrdersRule(Invalid::OutsideWithATelephone))
        ));
        let with_none = OrdersEdit {
            takes_orders_on: Some(Some("none".to_owned())),
            orders_from: Some(Vec::new()),
            ..outside.clone()
        };
        assert!(preview(&before, &with_none).unwrap().outside_whatsapp);
        assert!(preview(&before, &outside).unwrap().outside_whatsapp);

        let bridge = Phone::parse(&phone_text).unwrap();
        let other = Phone::parse("+5511900000002").unwrap();
        let full = OrdersRecord {
            takes_orders_on: Some(bridge.clone()),
            orders_from: vec![other],
            ..OrdersRecord::default()
        };
        let cleared = applied(&full, None, None, false, true);
        assert!(cleared.outside_whatsapp);
        assert!(cleared.takes_orders_on.is_none() && cleared.orders_from.is_empty());
        let own = OrdersRecord {
            takes_orders_on: Some(bridge.clone()),
            self_chat: true,
            ..OrdersRecord::default()
        };
        assert!(!applied(&own, None, None, false, true).self_chat);
        let outside_record = OrdersRecord {
            outside_whatsapp: true,
            ..OrdersRecord::default()
        };
        let takes = Some(Some(bridge));
        assert!(!applied(&outside_record, takes.as_ref(), None, false, false).outside_whatsapp);
        assert!(!applied(&outside_record, None, Some(&[]), false, false).outside_whatsapp);
        assert!(!applied(&outside_record, None, None, true, false).outside_whatsapp);
        let none = Some(None);
        assert!(applied(&outside_record, none.as_ref(), None, false, false).outside_whatsapp);
    }
}
