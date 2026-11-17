//! What `admin hermes whatsapp` prints (decision 0051, a proposal).
//!
//! A telephone is never printed: the record says that one is recorded, and how many give orders.
//! The command never asks, because an agent runs it: the risk is a line, printed while nothing
//! is protected for the Hermes and on any change that protects fewer chats.

use super::Report;
use crate::admin::orders::OrdersOutcome;
use crate::repo::orders::OrdersRecord;

/// The line that says the risk while nothing is recorded for the Hermes.
#[must_use]
pub fn nothing_recorded_line(hermes: &str) -> String {
    format!(
        "Warning: nothing is recorded, so nothing is blocked: any Hermes that can write in the \
         chat where {hermes} takes orders can pass for the person who gives them.\n"
    )
}

/// The line that says the risk of a change that protects fewer chats.
#[must_use]
pub fn fewer_chats_line(hermes: &str) -> String {
    format!(
        "Warning: any Hermes that can write in the chat between each removed telephone and \
         {hermes}'s WhatsApp can now pass for the person who gives it orders.\n"
    )
}

fn summary(hermes: &str, record: &OrdersRecord) -> String {
    let takes = if record.takes_orders_on.is_some() {
        "a recorded telephone"
    } else {
        "no recorded telephone"
    };
    let from = if record.self_chat {
        "itself, self-chat".to_owned()
    } else if record.orders_from.len() == 1 {
        "1 telephone".to_owned()
    } else {
        format!("{} telephones", record.orders_from.len())
    };
    let end = if record.protects_nothing() {
        ".\n"
    } else {
        ". No Hermes can write in that chat through Arauto.\n"
    };
    format!("{hermes}: takes orders on {takes}; orders from {from}{end}")
}

/// `admin hermes whatsapp NAME`, shown or changed. `asked` says that a flag was given.
#[must_use]
pub fn orders(outcome: &OrdersOutcome, asked: bool) -> Report {
    let hermes = outcome.hermes.as_str();
    let mut text = String::new();
    if asked && !outcome.written {
        text.push_str("Nothing changed: the record was already so.\n");
    } else if outcome.written {
        text.push_str("Recorded.\n");
    }
    text.push_str(&summary(hermes, &outcome.after));
    if outcome.after.protects_nothing() {
        text.push_str(&nothing_recorded_line(hermes));
    }
    if outcome.before.loses_a_pair_in(&outcome.after) {
        text.push_str(&fewer_chats_line(hermes));
    }
    Report::done(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Phone;
    use arauto_protocol::names::HermesName;

    fn record(takes: bool, from: &[&str], self_chat: bool) -> OrdersRecord {
        OrdersRecord {
            takes_orders_on: takes.then(|| Phone::parse("+5511900000001").unwrap()),
            self_chat,
            orders_from: from.iter().map(|p| Phone::parse(p).unwrap()).collect(),
        }
    }

    fn outcome(before: OrdersRecord, after: OrdersRecord, written: bool) -> OrdersOutcome {
        OrdersOutcome {
            hermes: HermesName::parse("vendas").unwrap(),
            before,
            after,
            written,
        }
    }

    #[test]
    fn the_risk_is_a_line_while_nothing_is_protected_and_when_a_change_protects_fewer() {
        let none = record(false, &[], false);
        let one = record(true, &["+5511900000002"], false);
        let two = record(true, &["+5511900000002", "+5511900000003"], false);
        // Shown with nothing recorded: the first line.
        let shown = orders(&outcome(none.clone(), none.clone(), false), false).text;
        assert!(shown.contains("Warning: nothing is recorded"), "{shown}");
        // Protecting more: no line.
        let more = orders(&outcome(one.clone(), two.clone(), true), true).text;
        assert!(!more.contains("Warning"), "{more}");
        assert!(more.contains("2 telephones"), "{more}");
        // Protecting fewer, still some: the second line only.
        let fewer = orders(&outcome(two, one.clone(), true), true).text;
        assert!(fewer.contains("can now pass for the person"), "{fewer}");
        assert!(!fewer.contains("nothing is recorded"), "{fewer}");
        // Clearing everything: both.
        let cleared = orders(&outcome(one, none, true), true).text;
        assert!(cleared.contains("nothing is recorded"), "{cleared}");
        assert!(cleared.contains("can now pass for the person"), "{cleared}");
    }

    #[test]
    fn a_telephone_is_never_printed() {
        let own = record(true, &["+5511900000002"], false);
        let text = orders(&outcome(own.clone(), own, false), true).text;
        assert!(!text.contains("55119"), "{text}");
        let me = record(true, &[], true);
        let text = orders(&outcome(me.clone(), me, true), true).text;
        assert!(text.contains("itself, self-chat"), "{text}");
    }
}
