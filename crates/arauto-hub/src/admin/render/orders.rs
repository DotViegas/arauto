//! What `admin hermes whatsapp` prints (decision 0051, a proposal).
//!
//! A telephone is never printed: the record says that one is recorded, and how many give orders.
//! The command never asks, because an agent runs it: the risk is a line, printed while nothing
//! is protected for the Hermes and on any change that protects fewer chats. A Hermes whose
//! orders come from outside WhatsApp gets a note instead: the hub cannot check that answer.

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

/// The note for a Hermes whose orders come from outside WhatsApp: the answer is the admin's.
#[must_use]
pub fn outside_line(hermes: &str) -> String {
    format!(
        "Note: the hub cannot check this. If {hermes} also takes orders on a WhatsApp, record \
         that WhatsApp instead, with --takes-orders-on and --self-chat or --orders-from.\n"
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

/// The line `admin hermes remove` prints when the Hermes had a record: its protection ends with it.
#[must_use]
pub fn removal_line(hermes: &str) -> String {
    format!(
        "Arauto stops refusing writes in the chat where {hermes} takes orders.
"
    )
}

fn summary(hermes: &str, record: &OrdersRecord) -> String {
    if record.outside_whatsapp {
        return format!(
            "{hermes}: its orders come from outside WhatsApp, as the admin recorded. Arauto \
             refuses nothing for it.\n"
        );
    }
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
    if outcome.after.outside_whatsapp {
        text.push_str(&outside_line(hermes));
    } else if outcome.after.needs_the_warning() {
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
            outside_whatsapp: false,
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

    #[test]
    fn an_answer_of_outside_whatsapp_is_said_without_the_warning() {
        let none = record(false, &[], false);
        let one = record(true, &["+5511900000002"], false);
        let outside = OrdersRecord {
            outside_whatsapp: true,
            ..none.clone()
        };
        let shown = orders(&outcome(outside.clone(), outside.clone(), false), false).text;
        let written = orders(&outcome(none, outside.clone(), true), true).text;
        for text in [&shown, &written] {
            assert!(
                text.contains("its orders come from outside WhatsApp"),
                "{text}"
            );
            assert!(text.contains("Note: the hub cannot check this."), "{text}");
            assert!(!text.contains("Warning: nothing is recorded"), "{text}");
            assert!(!text.contains("55119"), "{text}");
        }
        let switched = orders(&outcome(one, outside, true), true).text;
        assert!(
            switched.contains("can now pass for the person"),
            "{switched}"
        );
        assert!(!switched.contains("nothing is recorded"), "{switched}");
        assert!(!switched.contains("55119"), "{switched}");
    }
}
