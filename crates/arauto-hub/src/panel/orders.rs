//! Who gives a Hermes orders on WhatsApp, in the panel (decision 0051, a proposal; part B,
//! rebuilt as option B: the numbers of the hub are chosen by alias, only the exception is typed).
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /hermes/whatsapp?name=` | the window of a Hermes, from its record |
//! | `GET /hermes/whatsapp?name=&alias=&role=gives\|takes` | the same window with that number already chosen; nothing is saved |
//! | `POST /hermes/whatsapp` | `admin hermes whatsapp`, or the window that asks first, or the way back |
//! | `GET /whatsapps/orders?alias=` | from a number: which Hermes, and what the number is for it |
//! | `GET /whatsapps/orders?alias=&name=&role=` | the same, answered by a redirect to the window of the Hermes |
//!
//! Only the exact value `return=whatsapps` (query on a `GET`, hidden field on a `POST`) sends the
//! flow back to `/whatsapps`; anything else, or nothing, is `/`.
//!
//! Nothing here decides. The write is [`crate::admin::Session::hermes_orders_set`], the one the
//! command calls, with the same rules and the same audit; what is of the panel is the order of
//! the screens and the questions that come first. The form posts the aliases of the numbers of
//! the hub and the hub turns each into its telephone, so the record keeps telephones. The risk is
//! said where the record is set: one fixed sentence in the window whatever is recorded, and a
//! window over it, with no script, before a save that leaves no chat protected or that
//! unprotects one. Nothing is saved before the answer to that window.
//!
//! A typed telephone that does not start with `+` stops the save once, with a read-back of each
//! line; saving again with the same lines keeps them. What proves it is a keyed digest the hub
//! draws at start, bound to the panel session, the Hermes and the exact lines ([`acknowledgement`]).
//!
//! A telephone is never put in an address: the links name aliases and Hermes names. A `GET` writes
//! nothing.

use std::collections::BTreeSet;

use arauto_protocol::names::{HermesName, NumberAlias};
use arauto_protocol::secret::Secret;
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Method, Response, StatusCode};
use data_encoding::HEXLOWER;
use hkdf::hmac::{Hmac, KeyInit, Mac};
use hyper::body::Incoming;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::data::Pick;
use super::html::{Arg, Html};
use super::text::{Lang, k};
use super::view::{self, Frame, HermesPage, NumbersPage, Over};
use super::{PanelState, data, http, session_of};
use crate::admin::orders::{OrdersEdit, preview};
use crate::admin::{self, AdminError};
use crate::channel::Hub;
use crate::db::ResolvedOrg;
use crate::engine::Phone;
use crate::repo::orders::{Invalid, OrdersRecord};
use crate::repo::panel::PanelSession;
use crate::repo::{self, RepoError};

/// More numbers than this make "Its WhatsApp" a list box.
const PICK_MAX: usize = 8;
/// More lines than this in "Other telephones" are not read.
const LINES_MAX: usize = 64;

const CLOSE: &str =
    "<svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg>";

const ITS_WHATSAPP: &str = k("Its WhatsApp");
const TAKES_HINT: &str =
    k("The telephone {hermes}'s own WhatsApp runs on: {hermes} takes orders there.");
const NO_WHATSAPP: &str = k("It has no WhatsApp");
const A_NUMBER: &str = k("A number of Arauto");
const CHOOSE_A_NUMBER: &str = k("Choose a number");
const ANOTHER_TELEPHONE: &str = k("Another telephone");
const OTHER_HINT: &str = k(
    "Read only when Another telephone is chosen. Start with + and the country code, like +55 11 9XXXX-XXXX.",
);
const THESE_TELEPHONES: &str = k("These telephones");
const ITS_WHATSAPP_TAG: &str = k("its WhatsApp");
const OTHER_TELEPHONES: &str = k("Other telephones");
const OTHER_TELEPHONES_HINT: &str =
    k("Telephones that are not numbers of Arauto, one per line. At most 16 telephones in all.");
const CHOSEN: &str = k("{alias} is chosen. Nothing is saved until you save.");
const CHOSEN_AS_BRIDGE: &str =
    k("{alias} is chosen as its WhatsApp, in place of {before}. Nothing is saved until you save.");

const TAKES_UNCHOSEN: &str = k("Choose an option for its WhatsApp. Nothing was saved.");
const GONE_NUMBER: &str = k("{alias} is not a number of Arauto now. Nothing was saved.");
const NEEDS_OTHER: &str = k(
    "Type the other telephone its WhatsApp runs on, or choose another option. Nothing was saved.",
);
const BAD_TAKES: &str = k(
    "The other telephone of its WhatsApp is not a telephone: 8 to 15 digits, with + and the country code first. Nothing was saved.",
);
const BAD_LINE: &str = k("Line {n} of Other telephones is not a telephone. Nothing was saved.");
const SELF_WITH_LIST: &str = k(
    "Its own chat and a list of telephones are two answers: clear the list, or choose These telephones. Nothing was saved.",
);
const NEEDS_BRIDGE: &str =
    k("Choose {hermes}'s WhatsApp first: a chat has two ends. Nothing was saved.");
const BRIDGE_GIVES: &str = k(
    "Its WhatsApp cannot also give it orders: that chat is the self-chat mode. Nothing was saved.",
);
const TOO_MANY: &str =
    k("At most 16 telephones can give it orders; these are {n}. Nothing was saved.");
const CHECK_ONE: &str = k("1 typed telephone needs a look. Nothing was saved.");
const CHECK_MANY: &str = k("{n} typed telephones need a look. Nothing was saved.");
const TAKEN: &str = k("{other} already takes orders on this telephone.");
const RESTORED: &str = k(
    "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
);
const FAILED: &str =
    k("Something failed in the hub. See the hub's log and check the record of the Hermes.");

const LINE_N: &str = k("Line {n}");
const NOT_A_TELEPHONE: &str =
    k("Not a telephone: 8 to 15 digits, with + and the country code first.");
const NOT_IN_ARAUTO: &str = k("Not a number of Arauto.");
const SAME_AS_LINE: &str = k("The same telephone as line {n}: it counts once.");
const SAME_AS_ALIAS: &str = k("The same telephone as {alias}, chosen above: it counts once.");
const IS_ITS_WHATSAPP: &str = k("This is its WhatsApp: it cannot also give it orders.");
const IS_THE_NUMBER: &str = k("This is the number {alias} in Arauto.");
const NO_PLUS: &str = k(
    "Typed without +, so the first digits were read as the country code. If that is not its country, type + and the country code first. Saving again keeps it as read.",
);
const COUNT: &str = k("{n} of 16 telephones, with the numbers chosen above.");

const CHATS_STOP: &str = k("These chats stop being protected");
const BETWEEN: &str = k("Between {bridge}, its WhatsApp until now, and:");
const ITSELF: &str = k("itself (self-chat mode)");
const REMOVES_SENTENCE: &str = k(
    "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {hermes} orders.",
);
const REMOVE_PROTECTION: &str = k("Remove the protection");
const SAVE_WITHOUT_PROTECTION: &str = k("Save without protection");

const WHAT_IS_IT: &str = k("What {alias} is for it");
const GIVES_ROLE: &str = k("A telephone that gives it orders");
const TAKES_ROLE: &str = k("Its WhatsApp, where it takes orders");
const TAKES_ON_NOW: &str = k("takes orders on {alias} now");
const GIVES_NOW: &str = k("{alias} gives it orders now");
const NOTHING_SAVED_HERE: &str =
    k("Nothing is saved here. The window of the Hermes opens with {alias} chosen.");
const CHOOSE_BOTH: &str = k("Choose a Hermes and what {alias} is for it.");
const GIVER_IS_BRIDGE: &str =
    k("{alias} is the WhatsApp of {hermes}: it cannot also give it orders.");

/// Where the flow goes back to: `/whatsapps` only for the exact value `return=whatsapps`.
const fn target(to_numbers: bool) -> &'static str {
    if to_numbers { "/whatsapps" } else { "/" }
}

fn plus(phone: &Phone) -> String {
    format!("+{}", phone.digits())
}

/// The choice of "Its WhatsApp".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Takes {
    /// It has no WhatsApp.
    None,
    /// A number of the hub, by alias.
    Number(String),
    /// Another telephone, typed.
    Other,
    /// Nothing was chosen.
    Unchosen,
}

/// What the form has in it, as it was sent: kept as text so that a refusal gives it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    /// The choice of "Its WhatsApp".
    pub takes: Takes,
    /// The "Another telephone" field, trimmed; carried whatever `takes` is.
    pub takes_other: String,
    /// "Its own WhatsApp": `self-chat`.
    pub own: bool,
    /// The numbers checked under "These telephones", by alias.
    pub gives: Vec<String>,
    /// "Other telephones", one per line; a line break is always `\n`.
    pub from: String,
    /// The flow came from, and goes back to, the WhatsApps tab.
    pub to_numbers: bool,
}

/// A form of another shape than the window writes: it is not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeError;

impl Typed {
    /// What the record says, for a window opened by `GET` to start from.
    fn of(record: &OrdersRecord, picks: &[Pick], to_numbers: bool) -> Self {
        let takes = match &record.takes_orders_on {
            None => Takes::None,
            Some(recorded) => picks
                .iter()
                .find(|pick| pick.phone.same_as(recorded))
                .map_or(Takes::Other, |pick| Takes::Number(pick.alias.clone())),
        };
        let takes_other = match (&takes, &record.takes_orders_on) {
            (Takes::Other, Some(recorded)) => plus(recorded),
            _ => String::new(),
        };
        let mut gives: Vec<String> = Vec::new();
        let mut from: Vec<String> = Vec::new();
        for giver in &record.orders_from {
            match picks.iter().find(|pick| pick.phone.same_as(giver)) {
                Some(pick) => {
                    if !gives.contains(&pick.alias) {
                        gives.push(pick.alias.clone());
                    }
                }
                None => from.push(plus(giver)),
            }
        }
        Self {
            takes,
            takes_other,
            own: record.self_chat,
            gives,
            from: from.join("\n"),
            to_numbers,
        }
    }

    /// What a form sent.
    ///
    /// # Errors
    /// [`ShapeError`] for a `takes` that is none of the four shapes, an alias that is not one,
    /// or more than 64 numbers checked.
    fn of_form(form: &http::Form) -> Result<Self, ShapeError> {
        let alias = |text: &str| NumberAlias::parse(text).map_err(|_| ShapeError);
        let takes = match form.field("takes").map(|value| value.trim().to_owned()) {
            None => Takes::Unchosen,
            Some(value) if value.is_empty() => Takes::Unchosen,
            Some(value) => match value.as_str() {
                "none" => Takes::None,
                "other" => Takes::Other,
                "number" => {
                    let chosen = form
                        .field("takes_number")
                        .map(|value| value.trim().to_owned())
                        .unwrap_or_default();
                    if chosen.is_empty() {
                        Takes::Unchosen
                    } else {
                        Takes::Number(alias(&chosen)?.as_str().to_owned())
                    }
                }
                other => match other.strip_prefix("number:") {
                    Some(chosen) => Takes::Number(alias(chosen)?.as_str().to_owned()),
                    None => return Err(ShapeError),
                },
            },
        };
        let fields = form.fields("gives");
        if fields.len() > LINES_MAX {
            return Err(ShapeError);
        }
        let mut gives: Vec<String> = Vec::new();
        for value in &fields {
            let one = alias(value)?.as_str().to_owned();
            if !gives.contains(&one) {
                gives.push(one);
            }
        }
        let text = |name: &str| {
            form.field(name)
                .map(|value| value.replace("\r\n", "\n").replace('\r', "\n"))
                .unwrap_or_default()
        };
        Ok(Self {
            takes,
            takes_other: text("takes_other").trim().to_owned(),
            own: text("source") == "self",
            gives,
            from: text("from"),
            to_numbers: text("return") == "whatsapps",
        })
    }
}

/// Why a form came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Nothing was chosen for "Its WhatsApp".
    TakesUnchosen,
    /// An alias that is not a number of the hub now.
    GoneNumber {
        /// The alias.
        alias: String,
        /// Whether it was the choice of "Its WhatsApp" (else a number that gives orders).
        takes: bool,
    },
    /// "Another telephone" is chosen and empty.
    NeedsOther,
    /// "Another telephone" is not a telephone.
    BadTakes,
    /// A line of "Other telephones" is not a telephone (the physical number of the line).
    BadLine(usize),
    /// Its own chat and a list are two answers.
    SelfChatWithAList,
    /// A list or `self-chat` with no telephone of the Hermes.
    NeedsBridge,
    /// The telephone of its WhatsApp is also in the list.
    BridgeAmongGivers {
        /// The checked numbers that are its WhatsApp.
        aliases: Vec<String>,
        /// The lines that are its WhatsApp.
        lines: Vec<usize>,
        /// Its WhatsApp is the typed "Another telephone".
        other: bool,
    },
    /// More than 16 telephones (or more than 64 lines).
    TooMany(usize),
    /// Another Hermes takes orders on that telephone.
    Taken(String),
    /// Typed telephones that need a look: how many.
    Check(usize),
    /// The database came back from a backup.
    Restored,
    /// Anything else: the log has it.
    Failed,
}

impl Problem {
    const fn status(&self) -> StatusCode {
        match self {
            Problem::Restored => StatusCode::CONFLICT,
            Problem::Failed => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::OK,
        }
    }

    /// What an error of the admin function says on the screen. The error goes to the log.
    fn of(error: &AdminError, after: &OrdersRecord, analysis: &Analysis) -> Self {
        match error {
            AdminError::OrdersRule(rule) => Self::of_rule(*rule, after, analysis),
            AdminError::OrdersTelephoneTaken(other) => Problem::Taken(other.clone()),
            AdminError::Restored(_) => Problem::Restored,
            other => {
                tracing::error!(error = %other, "the panel could not record who gives a Hermes orders");
                Problem::Failed
            }
        }
    }

    /// What a rule of the record says on the screen.
    fn of_rule(rule: Invalid, after: &OrdersRecord, analysis: &Analysis) -> Self {
        match rule {
            Invalid::SelfChatWithAList => Problem::SelfChatWithAList,
            Invalid::NoTelephoneOfTheHermes => Problem::NeedsBridge,
            Invalid::TooMany => Problem::TooMany(after.orders_from.len()),
            Invalid::BridgeAmongGivers => analysis.bridge_among_givers(),
        }
    }
}

/// A telephone of "Other telephones", read on its own.
#[derive(Debug, Clone)]
struct LineIn {
    /// The physical number of the line, from 1.
    n: usize,
    /// The line, trimmed.
    text: String,
    /// What the hub read, when the line is a telephone.
    phone: Option<Phone>,
    /// Whether it was typed without `+`.
    warned: bool,
}

/// A number checked under "These telephones", resolved.
#[derive(Debug, Clone)]
struct Given {
    alias: String,
    phone: Phone,
}

/// What each typed thing of a form means: the one place that reads it.
#[derive(Debug)]
struct Analysis {
    /// The problem of the choice of "Its WhatsApp", when it has one.
    takes_pending: Option<Problem>,
    /// The text of the telephone of its WhatsApp, as the edit carries it.
    takes_text: Option<String>,
    /// The telephone of its WhatsApp, when it resolved.
    bridge: Option<Phone>,
    /// The first checked alias that is not a number now.
    gone_gives: Option<String>,
    /// The checked numbers, resolved.
    gives: Vec<Given>,
    /// The non-empty lines of "Other telephones".
    lines: Vec<LineIn>,
    /// More than 64 non-empty lines: how many.
    too_many_lines: Option<usize>,
    /// Whether "Another telephone" is chosen and was typed without `+`.
    other_warned: bool,
    /// Whether "Another telephone" is the choice.
    other_chosen: bool,
    /// The warned readings: `takes:digits` and `gives:digits`, sorted, once each.
    warned: Vec<String>,
    /// How the hub read what was typed.
    readback: Option<Readback>,
}

/// The recorded form of a telephone when the record already holds it (either ninth-digit
/// form), so that choosing it writes nothing.
fn recorded_form(recorded: Option<&Phone>, phone: &Phone) -> Phone {
    match recorded {
        Some(recorded) if recorded.same_as(phone) => recorded.clone(),
        _ => phone.clone(),
    }
}

fn analyse(typed: &Typed, picks: &[Pick], before: &OrdersRecord) -> Analysis {
    // Its WhatsApp.
    let mut takes_pending = None;
    let mut takes_text = None;
    let mut bridge = None;
    let mut other_warned = false;
    match &typed.takes {
        Takes::None => {}
        Takes::Unchosen => takes_pending = Some(Problem::TakesUnchosen),
        Takes::Number(alias) => match picks.iter().find(|pick| pick.alias == *alias) {
            None => {
                takes_pending = Some(Problem::GoneNumber {
                    alias: alias.clone(),
                    takes: true,
                });
            }
            Some(pick) => {
                let phone = recorded_form(before.takes_orders_on.as_ref(), &pick.phone);
                takes_text = Some(plus(&phone));
                bridge = Some(phone);
            }
        },
        Takes::Other => {
            if typed.takes_other.is_empty() {
                takes_pending = Some(Problem::NeedsOther);
            } else if let Ok(phone) = Phone::parse(&typed.takes_other) {
                other_warned = !typed.takes_other.starts_with('+');
                takes_text = Some(typed.takes_other.clone());
                bridge = Some(phone);
            } else {
                takes_pending = Some(Problem::BadTakes);
            }
        }
    }
    // The checked numbers.
    let mut gone_gives = None;
    let mut gives = Vec::new();
    for alias in &typed.gives {
        match picks.iter().find(|pick| pick.alias == *alias) {
            None => {
                gone_gives.get_or_insert_with(|| alias.clone());
            }
            Some(pick) => gives.push(Given {
                alias: alias.clone(),
                phone: recorded_form(
                    before.orders_from.iter().find(|r| r.same_as(&pick.phone)),
                    &pick.phone,
                ),
            }),
        }
    }
    // The other telephones, one line at a time.
    let mut lines = Vec::new();
    let mut count = 0usize;
    for (index, raw) in typed.from.split('\n').enumerate() {
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        count += 1;
        let phone = Phone::parse(text).ok();
        lines.push(LineIn {
            n: index + 1,
            text: text.to_owned(),
            warned: phone.is_some() && !text.starts_with('+'),
            phone,
        });
    }
    let too_many_lines = (count > LINES_MAX).then_some(count);
    if too_many_lines.is_some() {
        lines.clear();
    }
    let other_chosen = typed.takes == Takes::Other;
    let mut warned = BTreeSet::new();
    if other_chosen
        && other_warned
        && let Some(phone) = &bridge
    {
        warned.insert(format!("takes:{}", phone.digits()));
    }
    for line in &lines {
        if let (Some(phone), true) = (&line.phone, line.warned) {
            warned.insert(format!("gives:{}", phone.digits()));
        }
    }
    let mut analysis = Analysis {
        takes_pending,
        takes_text,
        bridge,
        gone_gives,
        gives,
        lines,
        too_many_lines,
        other_warned,
        other_chosen,
        warned: warned.into_iter().collect(),
        readback: None,
    };
    if analysis.too_many_lines.is_none() {
        analysis.readback = Some(analysis.read(typed, picks));
    }
    analysis
}

impl Analysis {
    /// How the hub read what was typed (section "the read-back").
    fn read(&self, typed: &Typed, picks: &[Pick]) -> Readback {
        let in_arauto = |phone: &Phone| {
            picks
                .iter()
                .find(|pick| pick.phone.same_as(phone))
                .map_or(Note::NotInArauto, |pick| Note::IsNumber(pick.alias.clone()))
        };
        let mut lines: Vec<LineRead> = Vec::new();
        let mut distinct_seen: Vec<Phone> = Vec::new();
        for given in &self.gives {
            if !distinct_seen
                .iter()
                .any(|known| known.same_as(&given.phone))
            {
                distinct_seen.push(given.phone.clone());
            }
        }
        let mut earlier: Vec<(usize, &Phone)> = Vec::new();
        for line in &self.lines {
            let Some(phone) = &line.phone else {
                lines.push(LineRead {
                    n: line.n,
                    reading: Reading::Bad,
                });
                continue;
            };
            let note = if self
                .bridge
                .as_ref()
                .is_some_and(|bridge| bridge.same_as(phone))
            {
                Note::ItsWhatsapp
            } else if let Some(given) = self.gives.iter().find(|g| g.phone.same_as(phone)) {
                Note::SameAsAlias(given.alias.clone())
            } else if let Some((n, _)) = earlier.iter().find(|(_, known)| known.same_as(phone)) {
                Note::SameAsLine(*n)
            } else {
                in_arauto(phone)
            };
            if !distinct_seen.iter().any(|known| known.same_as(phone)) {
                distinct_seen.push(phone.clone());
            }
            earlier.push((line.n, phone));
            lines.push(LineRead {
                n: line.n,
                reading: Reading::Good {
                    shown: plus(phone),
                    note,
                    warned: line.warned,
                },
            });
        }
        let other =
            (self.other_chosen && !typed.takes_other.is_empty()).then(|| {
                match Phone::parse(&typed.takes_other) {
                    Err(_) => Reading::Bad,
                    Ok(phone) => Reading::Good {
                        shown: plus(&phone),
                        note: in_arauto(&phone),
                        warned: self.other_warned,
                    },
                }
            });
        Readback {
            lines,
            other,
            count: distinct_seen.len(),
        }
    }

    /// The first problem of what was typed, before any rule of the record (steps 1 to 6 of a
    /// save, in their order).
    fn first_problem(&self) -> Option<Problem> {
        if let Some(problem) = &self.takes_pending {
            return Some(problem.clone());
        }
        if let Some(alias) = &self.gone_gives {
            return Some(Problem::GoneNumber {
                alias: alias.clone(),
                takes: false,
            });
        }
        if let Some(line) = self.lines.iter().find(|line| line.phone.is_none()) {
            return Some(Problem::BadLine(line.n));
        }
        self.too_many_lines.map(Problem::TooMany)
    }

    /// The edit the command would be given: every field of the form is the whole answer. The list
    /// is sent also with its own chat, so that the rule refuses the two together.
    fn edit(&self, own: bool) -> OrdersEdit {
        let mut from: Vec<String> = self.gives.iter().map(|g| plus(&g.phone)).collect();
        from.extend(self.lines.iter().map(|line| line.text.clone()));
        OrdersEdit {
            takes_orders_on: Some(self.takes_text.clone()),
            self_chat: own,
            orders_from: Some(from),
        }
    }

    /// Who is at once its WhatsApp and a giver.
    fn bridge_among_givers(&self) -> Problem {
        let Some(bridge) = &self.bridge else {
            return Problem::BridgeAmongGivers {
                aliases: Vec::new(),
                lines: Vec::new(),
                other: false,
            };
        };
        Problem::BridgeAmongGivers {
            aliases: self
                .gives
                .iter()
                .filter(|given| given.phone.same_as(bridge))
                .map(|given| given.alias.clone())
                .collect(),
            lines: self
                .lines
                .iter()
                .filter(|line| line.phone.as_ref().is_some_and(|p| p.same_as(bridge)))
                .map(|line| line.n)
                .collect(),
            other: self.other_chosen,
        }
    }
}

/// What the hub says of a telephone that is not its WhatsApp.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Note {
    /// It is the telephone of its WhatsApp.
    ItsWhatsapp,
    /// The same as a number checked above.
    SameAsAlias(String),
    /// The same as an earlier line.
    SameAsLine(usize),
    /// It is a number of the hub.
    IsNumber(String),
    /// It is not one.
    NotInArauto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Reading {
    /// Not a telephone.
    Bad,
    /// A telephone, as the hub read it.
    Good {
        shown: String,
        note: Note,
        warned: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineRead {
    n: usize,
    reading: Reading,
}

/// How the hub read what was typed, to show beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readback {
    lines: Vec<LineRead>,
    other: Option<Reading>,
    count: usize,
}

impl Readback {
    fn warned_lines(&self) -> bool {
        self.lines
            .iter()
            .any(|line| matches!(line.reading, Reading::Good { warned: true, .. }))
    }

    fn warned_other(&self) -> bool {
        matches!(self.other, Some(Reading::Good { warned: true, .. }))
    }
}

/// A line above the form that says what happened when the window was opened by an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The number is chosen.
    Chosen(String),
    /// The number is chosen as its WhatsApp, in place of another one.
    ChosenAsBridge {
        /// The number.
        alias: String,
        /// The label of the WhatsApp it replaces.
        before: String,
    },
}

// ---------------------------------------------------------------------------- the acknowledgement

/// What proves that this panel session was shown the read-back of exactly these warned readings
/// for this Hermes: HMAC-SHA-256 under the key the hub drew at start, over the session, the
/// Hermes and each entry (sorted, once). It is not a secret and not a credential: it only lets
/// the second save through the warning.
fn acknowledgement(key: &Secret<32>, session: Uuid, hermes: &str, warned: &[String]) -> String {
    let Ok(mut mac) = <Hmac<Sha256> as KeyInit>::new_from_slice(key.expose()) else {
        return String::new();
    };
    mac.update(b"arauto panel orders acknowledgement\0");
    mac.update(session.as_bytes());
    mac.update(hermes.as_bytes());
    mac.update(&[0]);
    let entries: BTreeSet<&String> = warned.iter().collect();
    for entry in entries {
        mac.update(entry.as_bytes());
        mac.update(&[0]);
    }
    HEXLOWER.encode(&mac.finalize().into_bytes())
}

/// Whether `given` is the acknowledgement of exactly `warned` for this session and Hermes. Anything
/// else (absent, another length, not lowercase hex, another session, Hermes or set) is no.
fn acknowledged(
    key: &Secret<32>,
    session: Uuid,
    hermes: &str,
    warned: &[String],
    given: &str,
) -> bool {
    let Ok(given) = HEXLOWER.decode(given.as_bytes()) else {
        return false;
    };
    let expected = acknowledgement(key, session, hermes, warned);
    let Ok(expected) = HEXLOWER.decode(expected.as_bytes()) else {
        return false;
    };
    given.len() == 32 && bool::from(given.as_slice().ct_eq(expected.as_slice()))
}

/// An acknowledgement carried through "Go back": kept only when it is 64 lowercase hex.
fn well_formed(ack: &str) -> bool {
    ack.len() == 64
        && ack
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// ---------------------------------------------------------------------------- markup

/// The page drawn behind the windows.
pub(super) enum Under {
    /// The page of the Hermes.
    Hermes(HermesPage),
    /// The WhatsApps page, when the flow came from there.
    Numbers(NumbersPage),
}

fn draw_under(html: &mut Html, frame: &Frame, under: &Under) {
    match under {
        Under::Hermes(page) => view::hermes_into(html, frame, page, Over::Guide { refresh: None }),
        Under::Numbers(page) => {
            view::numbers_into(html, frame, page, Over::Guide { refresh: None });
        }
    }
}

/// The way out of the head of a window.
#[derive(Clone, Copy)]
enum Close {
    /// A link to where the flow goes back to.
    To(&'static str),
    /// The submit button of the "Go back" form of the window that asks.
    Back,
}

/// The head of a window: its title and the way out.
fn head(html: &mut Html, id: &'static str, title: impl FnOnce(&mut Html), close: Close) {
    html.raw("<dialog open class=\"modal\" id=\"guide\" aria-labelledby=\"")
        .raw(id)
        .raw("\"><div class=\"modal-head\"><h2 id=\"")
        .raw(id)
        .raw("\">");
    title(html);
    match close {
        Close::To(to) => {
            html.raw("</h2><a class=\"close\" href=\"")
                .raw(to)
                .raw("\" aria-label=\"")
                .t(k("Close"))
                .raw("\">")
                .raw(CLOSE)
                .raw("</a></div>");
        }
        Close::Back => {
            html.raw(
                "</h2><button class=\"close\" type=\"submit\" form=\"orders-back\" aria-label=\"",
            )
            .t(k("Go back"))
            .raw("\">")
            .raw(CLOSE)
            .raw("</button></div>");
        }
    }
}

/// The hidden fields that carry what was typed, for a window that asks first.
fn carry(html: &mut Html, hermes: &str, typed: &Typed, ack: Option<&str>) {
    let hidden = |html: &mut Html, name: &'static str, value: &str| {
        html.raw("<input type=\"hidden\" name=\"")
            .raw(name)
            .raw("\" value=\"")
            .text(value)
            .raw("\">");
    };
    hidden(html, "name", hermes);
    match &typed.takes {
        Takes::None => hidden(html, "takes", "none"),
        Takes::Number(alias) => hidden(html, "takes", &format!("number:{alias}")),
        Takes::Other => hidden(html, "takes", "other"),
        Takes::Unchosen => {}
    }
    hidden(html, "takes_other", &typed.takes_other);
    hidden(html, "source", if typed.own { "self" } else { "list" });
    for alias in &typed.gives {
        hidden(html, "gives", alias);
    }
    hidden(html, "from", &typed.from);
    if let Some(ack) = ack {
        hidden(html, "ack", ack);
    }
    if typed.to_numbers {
        hidden(html, "return", "whatsapps");
    }
}

fn say(html: &mut Html, hermes: &str, problem: &Problem) {
    html.raw(if matches!(problem, Problem::Check(_)) {
        "<p class=\"attention\" id=\"orders-error\" role=\"alert\">"
    } else {
        "<p class=\"error\" id=\"orders-error\" role=\"alert\">"
    });
    match problem {
        Problem::TakesUnchosen => html.t(TAKES_UNCHOSEN),
        Problem::GoneNumber { alias, .. } => html.fill(GONE_NUMBER, &[("alias", Arg::Text(alias))]),
        Problem::NeedsOther => html.t(NEEDS_OTHER),
        Problem::BadTakes => html.t(BAD_TAKES),
        Problem::BadLine(n) => html.fill(BAD_LINE, &[("n", Arg::Number(*n))]),
        Problem::SelfChatWithAList => html.t(SELF_WITH_LIST),
        Problem::NeedsBridge => html.fill(NEEDS_BRIDGE, &[("hermes", Arg::Text(hermes))]),
        Problem::BridgeAmongGivers { .. } => html.t(BRIDGE_GIVES),
        Problem::TooMany(n) => html.fill(TOO_MANY, &[("n", Arg::Number(*n))]),
        Problem::Taken(other) => html.fill(TAKEN, &[("other", Arg::Text(other))]),
        Problem::Check(1) => html.t(CHECK_ONE),
        Problem::Check(n) => html.fill(CHECK_MANY, &[("n", Arg::Number(*n))]),
        Problem::Restored => html.t(RESTORED),
        Problem::Failed => html.t(FAILED),
    };
    html.raw("</p>");
}

/// Which controls the error paragraph is tied to.
#[derive(Default)]
struct Marks {
    takes: bool,
    source: bool,
    other: bool,
    other_invalid: bool,
    from: bool,
    from_invalid: bool,
    boxes: Vec<String>,
}

fn marks(problem: Option<&Problem>, typed: &Typed, readback: Option<&Readback>) -> Marks {
    let mut marks = Marks::default();
    match problem {
        Some(
            Problem::TakesUnchosen | Problem::NeedsBridge | Problem::GoneNumber { takes: true, .. },
        ) => marks.takes = true,
        Some(
            Problem::GoneNumber { takes: false, .. }
            | Problem::SelfChatWithAList
            | Problem::TooMany(_),
        ) => marks.source = true,
        Some(Problem::NeedsOther | Problem::BadTakes) => {
            marks.other = true;
            marks.other_invalid = true;
        }
        Some(Problem::BadLine(_)) => {
            marks.from = true;
            marks.from_invalid = true;
        }
        Some(Problem::BridgeAmongGivers {
            aliases,
            lines,
            other,
        }) => {
            marks.boxes.clone_from(aliases);
            if !lines.is_empty() {
                marks.from = true;
                marks.from_invalid = true;
            }
            if *other {
                marks.other = true;
                marks.other_invalid = true;
            }
        }
        Some(Problem::Taken(_)) => {
            if typed.takes == Takes::Other {
                marks.other = true;
                marks.other_invalid = true;
            } else {
                marks.takes = true;
            }
        }
        Some(Problem::Check(_)) => {
            marks.from = readback.is_some_and(Readback::warned_lines);
            marks.other = readback.is_some_and(Readback::warned_other);
        }
        Some(Problem::Restored | Problem::Failed) | None => {}
    }
    marks
}

/// What the hub says of one read telephone: its note, and the warning.
fn note_into(html: &mut Html, note: &Note) {
    if *note == Note::ItsWhatsapp {
        html.raw("<span class=\"orders-bad\">")
            .t(IS_ITS_WHATSAPP)
            .raw("</span>");
        return;
    }
    html.raw("<span>");
    match note {
        Note::SameAsAlias(alias) => {
            html.fill(SAME_AS_ALIAS, &[("alias", Arg::Strong(alias))]);
        }
        Note::SameAsLine(n) => {
            html.fill(SAME_AS_LINE, &[("n", Arg::Number(*n))]);
        }
        Note::IsNumber(alias) => {
            html.fill(IS_THE_NUMBER, &[("alias", Arg::Strong(alias))]);
        }
        Note::NotInArauto | Note::ItsWhatsapp => {
            html.t(NOT_IN_ARAUTO);
        }
    }
    html.raw("</span>");
}

fn reading_into(html: &mut Html, reading: &Reading) {
    match reading {
        Reading::Bad => {
            html.raw("<span class=\"orders-bad\">")
                .t(NOT_A_TELEPHONE)
                .raw("</span>");
        }
        Reading::Good {
            shown,
            note,
            warned,
        } => {
            html.raw("<span class=\"mono\">").text(shown).raw("</span>");
            note_into(html, note);
            if *warned {
                html.raw("<span class=\"orders-warn\">")
                    .t(NO_PLUS)
                    .raw("</span>");
            }
        }
    }
}

/// What the window of a Hermes shows.
pub(super) struct Window<'a> {
    /// The Hermes.
    pub hermes: &'a str,
    /// What is in the form.
    pub typed: &'a Typed,
    /// The numbers of the hub, offered as choices.
    pub picks: &'a [Pick],
    /// Why it came back.
    pub problem: Option<&'a Problem>,
    /// What an address filled in.
    pub notice: Option<&'a Notice>,
    /// How the hub read what was typed.
    pub readback: Option<&'a Readback>,
    /// The acknowledgement of the warned readings, when there are any.
    pub ack: Option<&'a str>,
}

/// The number that is its WhatsApp in the state the window shows.
fn bridge_alias<'a>(typed: &'a Typed, picks: &'a [Pick]) -> Option<&'a str> {
    match &typed.takes {
        Takes::Number(alias) => picks
            .iter()
            .find(|pick| pick.alias == *alias)
            .map(|pick| pick.alias.as_str()),
        Takes::Other => {
            let phone = Phone::parse(&typed.takes_other).ok()?;
            picks
                .iter()
                .find(|pick| pick.phone.same_as(&phone))
                .map(|pick| pick.alias.as_str())
        }
        Takes::None | Takes::Unchosen => None,
    }
}

fn describedby(html: &mut Html, ids: &[&'static str]) {
    html.raw(" aria-describedby=\"");
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            html.raw(" ");
        }
        html.raw(id);
    }
    html.raw("\"");
}

fn takes_fieldset(html: &mut Html, window: &Window<'_>, marks: &Marks) {
    let hermes = window.hermes;
    let who = [("hermes", Arg::Text(hermes))];
    let typed = window.typed;
    html.raw("<fieldset class=\"orders-takes\"");
    let ids: &[&'static str] = if marks.takes {
        &["orders-takes-hint", "orders-error"]
    } else {
        &["orders-takes-hint"]
    };
    describedby(html, ids);
    html.raw("><legend>")
        .t(ITS_WHATSAPP)
        .raw("</legend><p class=\"hint\" id=\"orders-takes-hint\">")
        .fill(TAKES_HINT, &who)
        .raw("</p><label class=\"option\"><input type=\"radio\" name=\"takes\" value=\"none\"")
        .raw(if typed.takes == Takes::None {
            " checked"
        } else {
            ""
        })
        .raw("><span>")
        .t(NO_WHATSAPP)
        .raw("</span></label>");
    // The hub's own number the Hermes is already given as a taker is never taken from it.
    let taken_by = |pick: &'_ Pick| -> Option<String> {
        pick.takes_for.clone().filter(|other| other != hermes)
    };
    let chosen = |pick: &Pick| typed.takes == Takes::Number(pick.alias.clone());
    if window.picks.len() > PICK_MAX {
        html.raw("<label class=\"option\"><input type=\"radio\" name=\"takes\" value=\"number\"")
            .raw(if window.picks.iter().any(chosen) {
                " checked"
            } else {
                ""
            })
            .raw("><span id=\"orders-takes-number-label\">")
            .t(A_NUMBER)
            .raw("</span></label><div class=\"orders-sub\"><select id=\"orders-takes-number\" name=\"takes_number\" aria-labelledby=\"orders-takes-number-label\"><option value=\"\">")
            .t(CHOOSE_A_NUMBER)
            .raw("</option>");
        for pick in window.picks {
            let taken = taken_by(pick);
            html.raw("<option value=\"")
                .text(&pick.alias)
                .raw("\"")
                .raw(if chosen(pick) { " selected" } else { "" })
                .raw(if taken.is_some() && !chosen(pick) {
                    " disabled"
                } else {
                    ""
                })
                .raw(">")
                .text(&pick.alias)
                .raw(" ")
                .text(&pick.shown);
            if let Some(other) = &taken {
                html.raw(" · ").fill(
                    k("{hermes} takes orders here"),
                    &[("hermes", Arg::Text(other))],
                );
            }
            html.raw("</option>");
        }
        html.raw("</select></div>");
    } else {
        for pick in window.picks {
            let taken = taken_by(pick);
            html.raw(
                "<label class=\"option\"><input type=\"radio\" name=\"takes\" value=\"number:",
            )
            .text(&pick.alias)
            .raw("\"")
            .raw(if chosen(pick) { " checked" } else { "" })
            .raw(if taken.is_some() && !chosen(pick) {
                " disabled"
            } else {
                ""
            })
            .raw("><span class=\"orders-pick\"><span class=\"name\">")
            .text(&pick.alias)
            .raw("</span><span class=\"mono\">")
            .text(&pick.shown)
            .raw("</span>");
            if let Some(other) = &taken {
                html.raw("<span class=\"muted\">")
                    .fill(
                        k("{hermes} takes orders here"),
                        &[("hermes", Arg::Text(other))],
                    )
                    .raw("</span>");
            }
            html.raw("</span></label>");
        }
    }
    html.raw("<label class=\"option\"><input type=\"radio\" name=\"takes\" value=\"other\"")
        .raw(if typed.takes == Takes::Other {
            " checked"
        } else {
            ""
        })
        .raw("><span id=\"orders-takes-other-label\">")
        .t(ANOTHER_TELEPHONE)
        .raw("</span></label><div class=\"field orders-sub orders-other\"><input id=\"orders-takes-other\" class=\"tel\" name=\"takes_other\" type=\"text\" inputmode=\"tel\" maxlength=\"32\" placeholder=\"+55 11 9XXXX-XXXX\" spellcheck=\"false\" aria-labelledby=\"orders-takes-other-label\"");
    let mut ids = vec!["orders-takes-other-hint"];
    if window.readback.is_some_and(|read| read.other.is_some()) {
        ids.push("orders-takes-read");
    }
    if marks.other {
        ids.push("orders-error");
    }
    describedby(html, &ids);
    html.raw(if marks.other_invalid {
        " aria-invalid=\"true\""
    } else {
        ""
    })
    .raw(" value=\"")
    .text(&typed.takes_other)
    .raw("\"><p class=\"hint\" id=\"orders-takes-other-hint\">")
    .t(OTHER_HINT)
    .raw("</p>");
    if let Some(reading) = window.readback.and_then(|read| read.other.as_ref()) {
        html.raw("<p class=\"hint\" id=\"orders-takes-read\">");
        reading_into(html, reading);
        html.raw("</p>");
    }
    html.raw("</div></fieldset>");
}

fn source_fieldset(html: &mut Html, window: &Window<'_>, marks: &Marks) {
    let typed = window.typed;
    let bridge = bridge_alias(typed, window.picks);
    html.raw("<fieldset class=\"orders-source\"");
    if marks.source {
        describedby(html, &["orders-error"]);
    }
    html.raw("><legend>")
        .t(k("Orders come from"))
        .raw("</legend><label class=\"option\"><input type=\"radio\" name=\"source\" value=\"self\"")
        .raw(if typed.own { " checked" } else { "" })
        .raw("><span>")
        .t(k(
            "Its own WhatsApp, in the chat with itself (self-chat mode)",
        ))
        .raw("</span></label><label class=\"option\"><input type=\"radio\" name=\"source\" value=\"list\"")
        .raw(if typed.own { "" } else { " checked" })
        .raw("><span>")
        .t(THESE_TELEPHONES)
        .raw("</span></label><div class=\"orders-sub orders-list\">");
    for pick in window.picks {
        let offender = marks.boxes.contains(&pick.alias);
        let checked = typed.gives.contains(&pick.alias);
        // Its own WhatsApp cannot give it orders (the save refuses it), so its box comes
        // disabled. A box that came back checked, after that refusal, stays enabled so it can
        // be unchecked: a disabled box is not sent, and the choice would vanish unseen.
        let own = bridge == Some(pick.alias.as_str());
        html.raw("<label class=\"option\"><input type=\"checkbox\" name=\"gives\" value=\"")
            .text(&pick.alias)
            .raw("\"")
            .raw(if checked { " checked" } else { "" })
            .raw(if own && !checked { " disabled" } else { "" })
            .raw(if offender {
                " aria-invalid=\"true\" aria-describedby=\"orders-error\""
            } else {
                ""
            })
            .raw("><span class=\"orders-pick\"><span class=\"name\">")
            .text(&pick.alias)
            .raw("</span><span class=\"mono\">")
            .text(&pick.shown)
            .raw("</span>");
        if own {
            html.raw("<span class=\"muted\">")
                .t(ITS_WHATSAPP_TAG)
                .raw("</span>");
        }
        html.raw("</span></label>");
    }
    let listed = window.readback.is_some_and(|read| !read.lines.is_empty());
    let mut ids = vec!["orders-from-hint"];
    if listed {
        ids.push("orders-readback");
    }
    if marks.from {
        ids.push("orders-error");
    }
    // Tall enough to show every telephone recorded, plus a free line, up to 12 lines.
    let rows = (typed.from.lines().count() + 1).clamp(3, 12);
    html.raw("<div class=\"field\"><label for=\"orders-from\">")
        .t(OTHER_TELEPHONES)
        .raw("</label><textarea id=\"orders-from\" name=\"from\" rows=\"")
        .text(&rows.to_string())
        .raw("\" spellcheck=\"false\" placeholder=\"+55 11 9XXXX-XXXX\"");
    describedby(html, &ids);
    html.raw(if marks.from_invalid {
        " aria-invalid=\"true\""
    } else {
        ""
    })
    .raw(">\n")
    .text(&typed.from)
    .raw("</textarea><p class=\"hint\" id=\"orders-from-hint\">")
    .t(OTHER_TELEPHONES_HINT)
    .raw("</p>");
    if let Some(read) = window.readback.filter(|read| !read.lines.is_empty()) {
        html.raw("<ul class=\"orders-readback\" id=\"orders-readback\">");
        for line in &read.lines {
            html.raw("<li id=\"orders-line-")
                .text(&line.n.to_string())
                .raw("\"><span class=\"orders-n\">")
                .fill(LINE_N, &[("n", Arg::Number(line.n))])
                .raw("</span>");
            reading_into(html, &line.reading);
            html.raw("</li>");
        }
        html.raw("</ul><p class=\"hint\" id=\"orders-count\">")
            .fill(COUNT, &[("n", Arg::Number(read.count))])
            .raw("</p>");
    }
    html.raw("</div></div></fieldset>");
}

/// The window of a Hermes, over the page behind it.
#[must_use]
pub(super) fn window_page(frame: &Frame, under: &Under, window: &Window<'_>) -> String {
    let mut html = Html::new(frame.lang);
    draw_under(&mut html, frame, under);
    let hermes = window.hermes;
    let typed = window.typed;
    let who = [("hermes", Arg::Text(hermes))];
    let back = target(typed.to_numbers);
    head(
        &mut html,
        "orders-title",
        |html| {
            html.fill(k("Who gives {hermes} orders on WhatsApp"), &who);
        },
        Close::To(back),
    );
    html.raw("<div class=\"modal-body\">");
    if let Some(problem) = window.problem {
        say(&mut html, hermes, problem);
    }
    match window.notice {
        Some(Notice::Chosen(alias)) => {
            html.raw("<p class=\"hint\">")
                .fill(CHOSEN, &[("alias", Arg::Strong(alias))])
                .raw("</p>");
        }
        Some(Notice::ChosenAsBridge { alias, before }) => {
            html.raw("<p class=\"hint\">")
                .fill(
                    CHOSEN_AS_BRIDGE,
                    &[("alias", Arg::Strong(alias)), ("before", Arg::Text(before))],
                )
                .raw("</p>");
        }
        None => {}
    }
    let marks = marks(window.problem, typed, window.readback);
    html.raw("<form id=\"orders-form\" class=\"orders-form\" method=\"post\" action=\"/hermes/whatsapp\" autocomplete=\"off\"><input type=\"hidden\" name=\"name\" value=\"")
        .text(hermes)
        .raw("\">");
    if typed.to_numbers {
        html.raw("<input type=\"hidden\" name=\"return\" value=\"whatsapps\">");
    }
    if let Some(ack) = window.ack {
        html.raw("<input type=\"hidden\" name=\"ack\" value=\"")
            .text(ack)
            .raw("\">");
    }
    takes_fieldset(&mut html, window, &marks);
    source_fieldset(&mut html, window, &marks);
    html.raw("</form><p class=\"hint\">")
        .fill(
            k("Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses."),
            &who,
        )
        .raw("</p><p class=\"sentence\">")
        .fill(
            k("If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them."),
            &who,
        )
        .raw("</p></div><div class=\"modal-foot\"><button class=\"button primary\" type=\"submit\" form=\"orders-form\">")
        .t(k("Save"))
        .raw("</button><a class=\"button\" href=\"")
        .raw(back)
        .raw("\">")
        .t(k("Cancel"))
        .raw("</a></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

/// Which of the two cases of the risk the save falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Risk {
    /// The save leaves no chat protected for the Hermes (a first save with everything empty
    /// included).
    pub nothing_protected: bool,
    /// The save leaves unprotected a chat that was protected.
    pub removes: bool,
}

impl Risk {
    /// Whether the window that asks first is needed.
    #[must_use]
    pub const fn asks(self) -> bool {
        self.nothing_protected || self.removes
    }

    fn of(before: &OrdersRecord, after: &OrdersRecord) -> Self {
        Self {
            nothing_protected: after.protects_nothing(),
            removes: before.loses_a_pair_in(after),
        }
    }
}

/// How a telephone of the record is named: the number of the hub it is, else `+digits`.
fn label_of(phone: &Phone, picks: &[Pick]) -> String {
    match picks.iter().find(|pick| pick.phone.same_as(phone)) {
        Some(pick) => format!("{} {}", pick.alias, pick.shown),
        None => plus(phone),
    }
}

/// The window that asks first, over the page behind it. Nothing is saved yet: its two buttons are
/// two forms that carry what was typed, and its X goes back too.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(super) fn risk_page(
    frame: &Frame,
    under: &Under,
    hermes: &str,
    typed: &Typed,
    picks: &[Pick],
    before: &OrdersRecord,
    after: &OrdersRecord,
    ack: Option<&str>,
) -> String {
    let risk = Risk::of(before, after);
    let mut html = Html::new(frame.lang);
    draw_under(&mut html, frame, under);
    let who = [("hermes", Arg::Text(hermes))];
    head(
        &mut html,
        "orders-risk-title",
        |html| {
            html.fill(k("A Hermes could give {hermes} orders"), &who);
        },
        Close::Back,
    );
    html.raw("<div class=\"modal-body short\">");
    let lost = before.lost_in(after);
    if let (false, Some(bridge)) = (lost.is_empty(), &before.takes_orders_on) {
        html.raw("<div class=\"warnings\"><p><b>")
            .t(CHATS_STOP)
            .raw("</b></p><p>")
            .fill(BETWEEN, &[("bridge", Arg::Text(&label_of(bridge, picks)))])
            .raw("</p><ul>");
        for (_, giver) in &lost {
            html.raw("<li>");
            if giver.same_as(bridge) {
                html.t(ITSELF);
            } else if let Some(pick) = picks.iter().find(|pick| pick.phone.same_as(giver)) {
                html.raw("<span class=\"name\">")
                    .text(&pick.alias)
                    .raw("</span> <span class=\"mono\">")
                    .text(&pick.shown)
                    .raw("</span>");
            } else {
                html.raw("<span class=\"mono\">")
                    .text(&plus(giver))
                    .raw("</span>");
            }
            html.raw("</li>");
        }
        html.raw("</ul></div>");
    }
    if risk.nothing_protected {
        html.raw("<p>")
            .fill(
                k("Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them."),
                &who,
            )
            .raw("</p>");
    }
    if risk.removes {
        html.raw("<p>").fill(REMOVES_SENTENCE, &who).raw("</p>");
    }
    html.raw("</div><div class=\"modal-foot\"><form method=\"post\" action=\"/hermes/whatsapp\">");
    carry(&mut html, hermes, typed, ack);
    html.raw("<input type=\"hidden\" name=\"confirmed\" value=\"1\"><button class=\"button danger\" type=\"submit\">")
        .t(if risk.removes {
            REMOVE_PROTECTION
        } else {
            SAVE_WITHOUT_PROTECTION
        })
        .raw("</button></form><form id=\"orders-back\" method=\"post\" action=\"/hermes/whatsapp\">");
    carry(&mut html, hermes, typed, ack);
    html.raw("<input type=\"hidden\" name=\"back\" value=\"1\"><button class=\"button\" type=\"submit\">")
        .t(k("Go back"))
        .raw("</button></form></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

/// What a Hermes is for a number now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Relation {
    /// The Hermes takes orders on the number.
    pub takes: bool,
    /// The number gives the Hermes orders.
    pub gives: bool,
}

/// Why the chooser came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChooseProblem {
    /// A Hermes or what the number is for it is missing.
    Both,
    /// Another Hermes already takes orders on the number.
    Taken(String),
    /// The number is the WhatsApp of that Hermes: it cannot also give it orders.
    GiverIsBridge(String),
}

/// What was chosen already, when the chooser comes back: the Hermes and the role.
pub type Chosen<'a> = (Option<&'a str>, Option<&'a str>);

/// The chooser of a number: a short form that checks nothing beforehand and opens the window of
/// the Hermes with the number already chosen.
#[must_use]
pub(super) fn chooser_page(
    frame: &Frame,
    page: &NumbersPage,
    alias: &str,
    hermes: &[(String, Relation)],
    chosen: Chosen<'_>,
    problem: Option<&ChooseProblem>,
) -> String {
    let mut html = Html::new(frame.lang);
    view::numbers_into(&mut html, frame, page, Over::Guide { refresh: None });
    let named = [("alias", Arg::Text(alias))];
    head(
        &mut html,
        "orders-choose-title",
        |html| {
            html.fill(k("Orders on WhatsApp for {alias}"), &named);
        },
        Close::To("/whatsapps"),
    );
    html.raw("<div class=\"modal-body\">");
    if hermes.is_empty() {
        html.raw("<p>")
            .t(k("Connect a Hermes first."))
            .raw("</p><p><a class=\"button\" href=\"/hermes/connect\">")
            .t(k("Connect Hermes"))
            .raw("</a></p></div><div class=\"modal-foot\">");
    } else {
        html.raw("<form id=\"orders-choose\" method=\"get\" action=\"/whatsapps/orders\"><input type=\"hidden\" name=\"alias\" value=\"")
            .text(alias)
            .raw("\">");
        if let Some(problem) = problem {
            html.raw("<p class=\"error\" id=\"orders-error\" role=\"alert\">");
            match problem {
                ChooseProblem::Both => html.fill(CHOOSE_BOTH, &named),
                ChooseProblem::Taken(other) => html.fill(TAKEN, &[("other", Arg::Text(other))]),
                ChooseProblem::GiverIsBridge(name) => html.fill(
                    GIVER_IS_BRIDGE,
                    &[("alias", Arg::Text(alias)), ("hermes", Arg::Text(name))],
                ),
            };
            html.raw("</p>");
        }
        html.raw("<fieldset><legend>Hermes</legend>");
        for (name, relation) in hermes {
            html.raw("<label class=\"option\"><input type=\"radio\" name=\"name\" value=\"")
                .text(name)
                .raw("\" required")
                .raw(if chosen.0 == Some(name.as_str()) {
                    " checked"
                } else {
                    ""
                })
                .raw("><span class=\"orders-pick\"><span class=\"name\">")
                .text(name)
                .raw("</span>");
            if relation.takes {
                html.raw("<span class=\"muted\">")
                    .fill(TAKES_ON_NOW, &named)
                    .raw("</span>");
            }
            if relation.gives {
                html.raw("<span class=\"muted\">")
                    .fill(GIVES_NOW, &named)
                    .raw("</span>");
            }
            html.raw("</span></label>");
        }
        html.raw("</fieldset><fieldset><legend>")
            .fill(WHAT_IS_IT, &named)
            .raw("</legend>");
        for (value, label) in [("gives", GIVES_ROLE), ("takes", TAKES_ROLE)] {
            html.raw("<label class=\"option\"><input type=\"radio\" name=\"role\" value=\"")
                .raw(value)
                .raw("\" required")
                .raw(if chosen.1 == Some(value) {
                    " checked"
                } else {
                    ""
                })
                .raw("><span>")
                .t(label)
                .raw("</span></label>");
        }
        html.raw("</fieldset><p class=\"hint\">")
            .fill(NOTHING_SAVED_HERE, &named)
            .raw("</p></form></div><div class=\"modal-foot\"><button class=\"button primary\" type=\"submit\" form=\"orders-choose\">")
            .t(k("Continue"))
            .raw("</button>");
    }
    html.raw("<a class=\"button\" href=\"/whatsapps\">")
        .t(k("Close"))
        .raw("</a></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

// ---------------------------------------------------------------------------- the handlers

struct Cx<'a> {
    hub: &'a Hub,
    panel: &'a PanelState,
    organization: ResolvedOrg,
    session: PanelSession,
    lang: Lang,
    admin: admin::Session,
}

/// What to show of a window that came back.
struct Answer<'a> {
    hermes: &'a str,
    typed: &'a Typed,
    picks: &'a [Pick],
    problem: Option<&'a Problem>,
    notice: Option<&'a Notice>,
    readback: Option<&'a Readback>,
    ack: Option<&'a str>,
}

impl Cx<'_> {
    /// The id of a Hermes by name, when it exists.
    async fn hermes(&self, name: &HermesName) -> Result<Option<Uuid>, RepoError> {
        Ok(
            repo::hermes::list(self.hub.pool(), self.organization, false)
                .await?
                .into_iter()
                .find(|line| line.hermes.name == *name)
                .map(|line| line.hermes.id),
        )
    }

    async fn record(&self, hermes: Uuid) -> Result<OrdersRecord, RepoError> {
        let key = self.hub.organization_key(self.organization).await?;
        repo::orders::get(self.hub.pool(), self.organization, &key, hermes).await
    }

    /// The acknowledgement of exactly these warned readings, for this session and Hermes.
    fn ack_for(&self, hermes: &str, warned: &[String]) -> Option<String> {
        if warned.is_empty() {
            return None;
        }
        self.panel
            .orders_ack()
            .map(|key| acknowledgement(key, self.session.id, hermes, warned))
    }

    /// Whether `given` is the acknowledgement of exactly `warned`.
    fn acknowledged(&self, hermes: &str, warned: &[String], given: &str) -> bool {
        self.panel
            .orders_ack()
            .is_some_and(|key| acknowledged(key, self.session.id, hermes, warned, given))
    }

    /// The frame and the page behind the windows: the WhatsApps page when the flow came from
    /// there, else the page of the Hermes.
    async fn under(&self, to_numbers: bool) -> Result<(Frame, Under), RepoError> {
        let frame = data::frame(self.hub, self.organization, self.lang).await?;
        let under = if to_numbers {
            Under::Numbers(
                data::numbers(self.hub, self.organization, self.lang, self.session.id).await?,
            )
        } else {
            Under::Hermes(data::hermes(self.hub, self.organization, self.lang).await?)
        };
        Ok((frame, under))
    }

    async fn window(
        &self,
        status: StatusCode,
        answer: &Answer<'_>,
    ) -> Result<Response<Body>, RepoError> {
        let (frame, under) = self.under(answer.typed.to_numbers).await?;
        Ok(http::page(
            status,
            window_page(
                &frame,
                &under,
                &Window {
                    hermes: answer.hermes,
                    typed: answer.typed,
                    picks: answer.picks,
                    problem: answer.problem,
                    notice: answer.notice,
                    readback: answer.readback,
                    ack: answer.ack,
                },
            ),
        ))
    }

    /// The window again, with a problem, the read-back and a fresh acknowledgement.
    async fn refuse(
        &self,
        hermes: &str,
        typed: &Typed,
        picks: &[Pick],
        analysis: &Analysis,
        problem: &Problem,
    ) -> Result<Response<Body>, RepoError> {
        // The read-back is not shown for more than 64 lines: none was made, so no warning was
        // shown either, and an acknowledgement would vouch for readings the admin never saw.
        let ack = analysis
            .readback
            .as_ref()
            .and_then(|_| self.ack_for(hermes, &analysis.warned));
        self.window(
            problem.status(),
            &Answer {
                hermes,
                typed,
                picks,
                problem: Some(problem),
                notice: None,
                readback: analysis.readback.as_ref(),
                ack: ack.as_deref(),
            },
        )
        .await
    }
}

/// `GET /hermes/whatsapp`: the window, from the record, with a number already chosen when the
/// address names one. Nothing is saved.
async fn open(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(name) =
        http::query_field(query, "name").and_then(|name| HermesName::parse(name.trim()).ok())
    else {
        return Ok(http::see_other("/"));
    };
    let Some(id) = cx.hermes(&name).await? else {
        return Ok(http::see_other("/"));
    };
    let to_numbers =
        http::query_field(query, "return").is_some_and(|value| value.as_str() == "whatsapps");
    let picks = data::picks(cx.hub, cx.organization).await?;
    let mut typed = Typed::of(&cx.record(id).await?, &picks, to_numbers);
    let mut problem = None;
    let mut notice = None;
    let alias = http::query_field(query, "alias").and_then(|alias| NumberAlias::parse(&alias).ok());
    let role = http::query_field(query, "role")
        .map(|role| role.trim().to_owned())
        .filter(|role| role == "gives" || role == "takes");
    if let (Some(alias), Some(role)) = (alias, role) {
        let alias = alias.as_str().to_owned();
        let takes = role == "takes";
        if picks.iter().all(|pick| pick.alias != alias) {
            problem = Some(Problem::GoneNumber { alias, takes });
        } else if !takes {
            typed.own = false;
            if !typed.gives.contains(&alias) {
                typed.gives.push(alias.clone());
            }
            notice = Some(Notice::Chosen(alias));
        } else if typed.takes == Takes::Number(alias.clone()) {
            notice = Some(Notice::Chosen(alias));
        } else {
            let before = match &typed.takes {
                Takes::Number(current) => picks
                    .iter()
                    .find(|pick| pick.alias == *current)
                    .map(|pick| format!("{} {}", pick.alias, pick.shown)),
                Takes::Other if !typed.takes_other.is_empty() => Some(typed.takes_other.clone()),
                _ => None,
            };
            typed.takes = Takes::Number(alias.clone());
            notice = Some(match before {
                Some(before) => Notice::ChosenAsBridge { alias, before },
                None => Notice::Chosen(alias),
            });
        }
    }
    cx.window(
        StatusCode::OK,
        &Answer {
            hermes: name.as_str(),
            typed: &typed,
            picks: &picks,
            problem: problem.as_ref(),
            notice: notice.as_ref(),
            readback: None,
            ack: None,
        },
    )
    .await
}

/// `GET /whatsapps/orders`: which Hermes, for a number.
async fn choose(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(alias) =
        http::query_field(query, "alias").and_then(|alias| NumberAlias::parse(alias.trim()).ok())
    else {
        return Ok(http::see_other("/whatsapps"));
    };
    match repo::number::by_alias(cx.hub.pool(), cx.organization, &alias).await? {
        Some(number) if number.state != crate::core::number::NumberState::Removed => {}
        _ => return Ok(http::see_other("/whatsapps")),
    }
    let picks = data::picks(cx.hub, cx.organization).await?;
    let pick = picks.iter().find(|pick| pick.alias == alias.as_str());
    let hermes: Vec<(String, Relation)> = repo::hermes::list(cx.hub.pool(), cx.organization, false)
        .await?
        .into_iter()
        .map(|line| {
            let name = line.hermes.name.as_str().to_owned();
            let relation = Relation {
                takes: pick.is_some_and(|pick| pick.takes_for.as_deref() == Some(name.as_str())),
                gives: pick.is_some_and(|pick| pick.gives_to.contains(&name)),
            };
            (name, relation)
        })
        .collect();
    let name_in = http::query_field(query, "name");
    let role_in = http::query_field(query, "role");
    let name = name_in
        .as_ref()
        .map(|name| name.trim().to_owned())
        .filter(|name| hermes.iter().any(|(known, _)| known == name));
    let role = role_in
        .as_ref()
        .map(|role| role.trim().to_owned())
        .filter(|role| role == "gives" || role == "takes");
    let problem = if name_in.is_none() && role_in.is_none() {
        None
    } else if let (Some(name), Some(role)) = (&name, &role) {
        let taker = pick.and_then(|pick| pick.takes_for.clone());
        match (role.as_str(), taker) {
            ("takes", Some(other)) if other != *name => Some(ChooseProblem::Taken(other)),
            ("gives", Some(taker)) if taker == *name => {
                Some(ChooseProblem::GiverIsBridge(name.clone()))
            }
            _ => {
                return Ok(http::see_other_to(&format!(
                    "/hermes/whatsapp?name={name}&alias={}&role={role}&return=whatsapps",
                    alias.as_str()
                )));
            }
        }
    } else {
        Some(ChooseProblem::Both)
    };
    let frame = data::frame(cx.hub, cx.organization, cx.lang).await?;
    let page = data::numbers(cx.hub, cx.organization, cx.lang, cx.session.id).await?;
    Ok(http::page(
        StatusCode::OK,
        chooser_page(
            &frame,
            &page,
            alias.as_str(),
            &hermes,
            (name.as_deref(), role.as_deref()),
            problem.as_ref(),
        ),
    ))
}

/// `POST /hermes/whatsapp`.
async fn save(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(name) = form
        .field("name")
        .and_then(|name| HermesName::parse(name.trim()).ok())
    else {
        return Ok(http::see_other("/"));
    };
    let Some(id) = cx.hermes(&name).await? else {
        return Ok(http::see_other("/"));
    };
    let Ok(typed) = Typed::of_form(form) else {
        return Ok(http::plain(
            StatusCode::BAD_REQUEST,
            "The form was not read.\n",
        ));
    };
    let hermes = name.as_str();
    let picks = data::picks(cx.hub, cx.organization).await?;
    // "Go back", and the X of the window that asks: the window again, with what was typed.
    // Nothing is written.
    if form.field("back").is_some() {
        let ack = form.field("ack").filter(|ack| well_formed(ack));
        return cx
            .window(
                StatusCode::OK,
                &Answer {
                    hermes,
                    typed: &typed,
                    picks: &picks,
                    problem: None,
                    notice: None,
                    readback: None,
                    ack: ack.as_deref().map(String::as_str),
                },
            )
            .await;
    }
    let before = cx.record(id).await?;
    let analysis = analyse(&typed, &picks, &before);
    if let Some(problem) = analysis.first_problem() {
        return cx.refuse(hermes, &typed, &picks, &analysis, &problem).await;
    }
    let edit = analysis.edit(typed.own);
    let after = match preview(&before, &edit) {
        Ok(after) => after,
        Err(error) => {
            let problem = Problem::of(&error, &OrdersRecord::default(), &analysis);
            return cx.refuse(hermes, &typed, &picks, &analysis, &problem).await;
        }
    };
    if let Err(rule) = after.check() {
        let problem = Problem::of_rule(rule, &after, &analysis);
        return cx.refuse(hermes, &typed, &picks, &analysis, &problem).await;
    }
    // Said before the window that asks: another Hermes already takes orders on that telephone.
    if let Some(takes) = &after.takes_orders_on {
        let key = cx.hub.organization_key(cx.organization).await?;
        if let Some(other) =
            repo::orders::bridge_taken_by(cx.hub.pool(), cx.organization, &key, id, takes).await?
        {
            let problem = Problem::Taken(other.as_str().to_owned());
            return cx.refuse(hermes, &typed, &picks, &analysis, &problem).await;
        }
    }
    // A typed telephone without "+" stops the save once.
    let acknowledged = analysis.warned.is_empty()
        || form
            .field("ack")
            .is_some_and(|given| cx.acknowledged(hermes, &analysis.warned, &given));
    if !acknowledged {
        let problem = Problem::Check(analysis.warned.len());
        return cx.refuse(hermes, &typed, &picks, &analysis, &problem).await;
    }
    // The question comes first: a save that leaves no chat protected, or that unprotects one,
    // is written only after the answer to it.
    if Risk::of(&before, &after).asks() && form.field("confirmed").is_none() {
        let ack = cx.ack_for(hermes, &analysis.warned);
        let (frame, under) = cx.under(typed.to_numbers).await?;
        return Ok(http::page(
            StatusCode::OK,
            risk_page(
                &frame,
                &under,
                hermes,
                &typed,
                &picks,
                &before,
                &after,
                ack.as_deref(),
            ),
        ));
    }
    match cx
        .admin
        .hermes_orders_set(cx.hub.vault(), hermes, &edit, cx.hub.now_ms())
        .await
    {
        // The address names the Hermes, never a telephone: the list says what the save left.
        Ok(_) => Ok(http::see_other_to(&format!(
            "{}?orders={hermes}",
            target(typed.to_numbers)
        ))),
        Err(AdminError::NoSuchHermes) => Ok(http::see_other(target(typed.to_numbers))),
        Err(error) => {
            let problem = Problem::of(&error, &after, &analysis);
            cx.refuse(hermes, &typed, &picks, &analysis, &problem).await
        }
    }
}

/// Answers a request for `/hermes/whatsapp` or `/whatsapps/orders`. The request already named
/// the panel and, when it writes, came from a page of the panel.
pub(super) async fn answer(
    hub: &Hub,
    panel: &PanelState,
    parts: &Parts,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = session_of(hub, panel, &parts.headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let cx = Cx {
        hub,
        panel,
        organization,
        session,
        lang: super::lang_of(&parts.headers),
        admin: admin::Session::of_panel(
            hub.pool().clone(),
            hub.mark().clone(),
            organization,
            session.admin,
        ),
    };
    let path = parts.uri.path();
    if parts.method == Method::GET {
        return match path {
            "/hermes/whatsapp" => open(&cx, parts.uri.query()).await,
            "/whatsapps/orders" => choose(&cx, parts.uri.query()).await,
            _ => Ok(http::plain(StatusCode::NOT_FOUND, "not found\n")),
        };
    }
    if path != "/hermes/whatsapp" {
        return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n"));
    }
    let form = match http::form(&parts.headers, body).await {
        Ok(form) => form,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    save(&cx, &form).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    fn clean(page: &str) {
        assert!(
            !page.contains("<script>") && !page.contains("<img"),
            "{page}"
        );
        assert!(!page.contains("\"'>"), "{page}");
    }

    /// The test telephones: `55119876` and five digits.
    fn phone(n: u32) -> String {
        format!("55119876{n:05}")
    }

    /// The same telephone without the ninth digit.
    fn other_form(n: u32) -> String {
        format!("5511{}", &phone(n)[5..])
    }

    fn pick(alias: &str, n: u32) -> Pick {
        let parsed = Phone::parse(&phone(n)).unwrap();
        Pick {
            alias: alias.to_owned(),
            shown: plus(&parsed),
            phone: parsed,
            takes_for: None,
            gives_to: Vec::new(),
        }
    }

    fn typed() -> Typed {
        Typed {
            takes: Takes::Other,
            takes_other: HOSTILE.to_owned(),
            own: false,
            gives: vec![HOSTILE.to_owned()],
            from: format!("{HOSTILE}\n+5511900000002"),
            to_numbers: false,
        }
    }

    fn form(body: &str) -> http::Form {
        http::Form::of(body.as_bytes())
    }

    #[test]
    fn what_is_typed_or_named_is_shown_as_text_in_every_window() {
        let frame = Frame::default();
        let under = Under::Hermes(HermesPage::default());
        let typed = typed();
        let mut hostile = pick(HOSTILE, 1);
        hostile.shown = HOSTILE.to_owned();
        hostile.takes_for = Some(HOSTILE.to_owned());
        let picks = [hostile, pick("operador", 2)];
        let readback = Readback {
            lines: vec![
                LineRead {
                    n: 1,
                    reading: Reading::Good {
                        shown: HOSTILE.to_owned(),
                        note: Note::SameAsAlias(HOSTILE.to_owned()),
                        warned: true,
                    },
                },
                LineRead {
                    n: 2,
                    reading: Reading::Good {
                        shown: HOSTILE.to_owned(),
                        note: Note::IsNumber(HOSTILE.to_owned()),
                        warned: false,
                    },
                },
                LineRead {
                    n: 3,
                    reading: Reading::Bad,
                },
            ],
            other: Some(Reading::Good {
                shown: HOSTILE.to_owned(),
                note: Note::NotInArauto,
                warned: true,
            }),
            count: 3,
        };
        let notices = [
            Notice::Chosen(HOSTILE.to_owned()),
            Notice::ChosenAsBridge {
                alias: HOSTILE.to_owned(),
                before: HOSTILE.to_owned(),
            },
        ];
        for problem in [
            Problem::TakesUnchosen,
            Problem::GoneNumber {
                alias: HOSTILE.to_owned(),
                takes: true,
            },
            Problem::GoneNumber {
                alias: HOSTILE.to_owned(),
                takes: false,
            },
            Problem::NeedsOther,
            Problem::BadTakes,
            Problem::BadLine(2),
            Problem::SelfChatWithAList,
            Problem::NeedsBridge,
            Problem::BridgeAmongGivers {
                aliases: vec![HOSTILE.to_owned()],
                lines: vec![1],
                other: true,
            },
            Problem::TooMany(17),
            Problem::Taken(HOSTILE.to_owned()),
            Problem::Check(2),
            Problem::Restored,
            Problem::Failed,
        ] {
            let shown = window_page(
                &frame,
                &under,
                &Window {
                    hermes: HOSTILE,
                    typed: &typed,
                    picks: &picks,
                    problem: Some(&problem),
                    notice: notices.first(),
                    readback: Some(&readback),
                    ack: Some(HOSTILE),
                },
            );
            clean(&shown);
            assert!(shown.contains("id=\"orders-error\""), "{problem:?}");
        }
        for notice in &notices {
            clean(&window_page(
                &frame,
                &under,
                &Window {
                    hermes: HOSTILE,
                    typed: &typed,
                    picks: &picks,
                    problem: None,
                    notice: Some(notice),
                    readback: None,
                    ack: None,
                },
            ));
        }
        // More numbers than the radios take: the list box is clean as well.
        let many: Vec<Pick> = (0..=PICK_MAX)
            .map(|n| {
                let mut one = pick(HOSTILE, u32::try_from(n).unwrap());
                one.shown = HOSTILE.to_owned();
                one.takes_for = Some(HOSTILE.to_owned());
                one
            })
            .collect();
        let listed = window_page(
            &frame,
            &under,
            &Window {
                hermes: "ana",
                typed: &typed,
                picks: &many,
                problem: None,
                notice: None,
                readback: None,
                ack: None,
            },
        );
        clean(&listed);
        assert!(listed.contains("name=\"takes_number\""));
        // The window that asks, with chats that stop being protected.
        let bridge = Phone::parse(&phone(2)).unwrap();
        let before = OrdersRecord {
            takes_orders_on: Some(bridge.clone()),
            self_chat: false,
            orders_from: vec![Phone::parse(&phone(1)).unwrap(), bridge.clone()],
        };
        let after = OrdersRecord {
            takes_orders_on: Some(bridge),
            ..OrdersRecord::default()
        };
        let asked = risk_page(
            &frame,
            &under,
            HOSTILE,
            &typed,
            &picks,
            &before,
            &after,
            Some(HOSTILE),
        );
        clean(&asked);
        assert!(asked.contains("These chats stop being protected"));
        assert!(asked.contains("type=\"submit\" form=\"orders-back\""));
        let numbers = Under::Numbers(NumbersPage::default());
        clean(&risk_page(
            &frame, &numbers, HOSTILE, &typed, &picks, &before, &after, None,
        ));
        // The chooser.
        let relation = Relation {
            takes: true,
            gives: true,
        };
        for problem in [
            None,
            Some(ChooseProblem::Both),
            Some(ChooseProblem::Taken(HOSTILE.to_owned())),
            Some(ChooseProblem::GiverIsBridge(HOSTILE.to_owned())),
        ] {
            let chooser = chooser_page(
                &frame,
                &NumbersPage::default(),
                HOSTILE,
                &[(HOSTILE.to_owned(), relation)],
                (Some(HOSTILE), Some("gives")),
                problem.as_ref(),
            );
            clean(&chooser);
        }
        clean(&chooser_page(
            &frame,
            &NumbersPage::default(),
            HOSTILE,
            &[],
            (None, None),
            None,
        ));
    }

    #[test]
    fn its_own_whatsapp_comes_disabled_and_the_telephones_fit_the_box() {
        let frame = Frame::default();
        let under = Under::Hermes(HermesPage::default());
        let picks = [pick("ponte", 1), pick("vendas", 2)];
        let from: Vec<String> = (10..17).map(|n| format!("+{}", phone(n))).collect();
        let mut typed = Typed {
            takes: Takes::Number("ponte".to_owned()),
            takes_other: String::new(),
            own: false,
            gives: Vec::new(),
            from: from.join("\n"),
            to_numbers: false,
        };
        let render = |typed: &Typed| {
            window_page(
                &frame,
                &under,
                &Window {
                    hermes: "ana",
                    typed,
                    picks: &picks,
                    problem: None,
                    notice: None,
                    readback: None,
                    ack: None,
                },
            )
        };
        let shown = render(&typed);
        assert!(shown.contains("name=\"gives\" value=\"ponte\" disabled>"));
        assert!(shown.contains("name=\"gives\" value=\"vendas\">"));
        // Seven telephones and a free line.
        assert!(shown.contains("name=\"from\" rows=\"8\""));
        // Checked after a refusal: it stays enabled, so it can be unchecked.
        typed.gives = vec!["ponte".to_owned()];
        typed.from = String::new();
        let shown = render(&typed);
        assert!(shown.contains("name=\"gives\" value=\"ponte\" checked>"));
        assert!(shown.contains("name=\"from\" rows=\"3\""));
    }

    #[test]
    fn the_edit_sends_the_list_also_with_self_chat() {
        let a = analyse(
            &Typed {
                takes: Takes::Number("ponte".to_owned()),
                takes_other: String::new(),
                own: true,
                gives: Vec::new(),
                from: "+5511900000002".to_owned(),
                to_numbers: false,
            },
            &[pick("ponte", 1)],
            &OrdersRecord::default(),
        );
        let own = a.edit(true);
        assert!(own.self_chat);
        assert_eq!(
            own.orders_from,
            Some(vec!["+5511900000002".to_owned()]),
            "the list is sent, so that the rule refuses the two together"
        );
        assert!(
            preview(&OrdersRecord::default(), &own).is_err(),
            "self-chat with a list is refused, not dropped"
        );
        let listed = analyse(
            &Typed {
                takes: Takes::None,
                takes_other: String::new(),
                own: false,
                gives: Vec::new(),
                from: " +5511900000002 \n\n+5511900000003\n".to_owned(),
                to_numbers: false,
            },
            &[],
            &OrdersRecord::default(),
        )
        .edit(false);
        assert_eq!(listed.takes_orders_on, Some(None));
        assert_eq!(
            listed.orders_from,
            Some(vec![
                "+5511900000002".to_owned(),
                "+5511900000003".to_owned()
            ])
        );
    }

    #[test]
    fn the_risk_is_asked_for_a_save_that_protects_nothing_or_unprotects_a_chat() {
        let record = |takes: bool, from: &[&str]| OrdersRecord {
            takes_orders_on: takes.then(|| Phone::parse("+5511900000001").unwrap()),
            self_chat: false,
            orders_from: from.iter().map(|p| Phone::parse(p).unwrap()).collect(),
        };
        let none = record(false, &[]);
        let one = record(true, &["+5511900000002"]);
        let two = record(true, &["+5511900000002", "+5511900000003"]);
        assert!(Risk::of(&none, &none).asks(), "a first empty save asks");
        assert!(Risk::of(&one, &none).asks());
        assert!(Risk::of(&two, &one).asks(), "removing one telephone asks");
        assert!(!Risk::of(&none, &one).asks());
        assert!(!Risk::of(&one, &two).asks(), "protecting more does not ask");
        assert!(!Risk::of(&one, &one).asks());
    }

    #[test]
    fn an_acknowledgement_binds_the_session_the_hermes_and_the_lines() {
        let key = Secret::<32>::new([7; 32]);
        let session = Uuid::from_u128(1);
        let warned = vec![
            "gives:11987600005".to_owned(),
            "takes:5511987600002".to_owned(),
        ];
        let ack = acknowledgement(&key, session, "ana", &warned);
        assert_eq!(ack.len(), 64);
        assert!(ack.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(ack, acknowledgement(&key, session, "ana", &warned));
        assert!(acknowledged(&key, session, "ana", &warned, &ack));
        // The order of the set does not matter: it is sorted.
        let reversed: Vec<String> = warned.iter().rev().cloned().collect();
        assert!(acknowledged(&key, session, "ana", &reversed, &ack));
        // Another session, another Hermes, another set, another key.
        assert!(!acknowledged(
            &key,
            Uuid::from_u128(2),
            "ana",
            &warned,
            &ack
        ));
        assert!(!acknowledged(&key, session, "bia", &warned, &ack));
        let more = [warned.clone(), vec!["gives:11987600006".to_owned()]].concat();
        assert!(!acknowledged(&key, session, "ana", &more, &ack));
        assert!(!acknowledged(&key, session, "ana", &warned[..1], &ack));
        let changed = vec!["gives:11987600006".to_owned(), warned[1].clone()];
        assert!(!acknowledged(&key, session, "ana", &changed, &ack));
        assert!(!acknowledged(
            &Secret::<32>::new([8; 32]),
            session,
            "ana",
            &warned,
            &ack
        ));
        // What is not the value.
        assert!(!acknowledged(&key, session, "ana", &warned, ""));
        assert!(!acknowledged(&key, session, "ana", &warned, &ack[..63]));
        assert!(!acknowledged(
            &key,
            session,
            "ana",
            &warned,
            &format!("{ack}0")
        ));
        assert!(!acknowledged(
            &key,
            session,
            "ana",
            &warned,
            &ack.to_ascii_uppercase()
        ));
        assert!(!acknowledged(
            &key,
            session,
            "ana",
            &warned,
            &"z".repeat(64)
        ));
        assert!(!acknowledged(
            &key,
            session,
            "ana",
            &warned,
            &"0".repeat(64)
        ));
    }

    #[test]
    fn a_form_of_another_shape_is_not_read() {
        let shape = |body: &str| Typed::of_form(&form(body));
        assert_eq!(shape("takes=whatever"), Err(ShapeError));
        assert_eq!(
            shape(&format!("takes=number:{}", form_text(HOSTILE))),
            Err(ShapeError)
        );
        assert_eq!(
            shape(&format!("takes=none&gives={}", form_text(HOSTILE))),
            Err(ShapeError)
        );
        assert_eq!(
            shape(&format!("takes=number&takes_number={}", form_text(HOSTILE))),
            Err(ShapeError)
        );
        let sixty_five: String = (0..65).map(|n| format!("&gives=n{n}")).collect();
        assert_eq!(shape(&format!("takes=none{sixty_five}")), Err(ShapeError));
        let sixty_four: String = (0..64).map(|n| format!("&gives=n{n}")).collect();
        assert!(shape(&format!("takes=none{sixty_four}")).is_ok());
        assert_eq!(shape("name=ana").unwrap().takes, Takes::Unchosen);
        assert_eq!(
            shape("takes=number&takes_number=").unwrap().takes,
            Takes::Unchosen
        );
        assert_eq!(shape("takes=number").unwrap().takes, Takes::Unchosen);
        let two = shape("takes=number:ponte&gives=a&gives=b&gives=a&source=self&return=whatsapps")
            .unwrap();
        assert_eq!(two.takes, Takes::Number("ponte".to_owned()));
        assert_eq!(two.gives, ["a", "b"], "a repeated alias is kept once");
        assert!(two.own && two.to_numbers);
        assert!(!shape("takes=none&return=elsewhere").unwrap().to_numbers);
    }

    fn form_text(text: &str) -> String {
        text.bytes().map(|b| format!("%{b:02X}")).collect()
    }

    #[test]
    fn each_typed_line_is_read_on_its_own() {
        let picks = [pick("operador", 1)];
        let typed = Typed {
            takes: Takes::None,
            takes_other: String::new(),
            own: false,
            gives: Vec::new(),
            from: format!(
                "+{}\n\nabc\n+{}\n11987600005\n{}\nnone",
                phone(3),
                other_form(3),
                phone(1)
            ),
            to_numbers: false,
        };
        let analysis = analyse(&typed, &picks, &OrdersRecord::default());
        let read = analysis.readback.as_ref().unwrap();
        let numbers: Vec<usize> = read.lines.iter().map(|line| line.n).collect();
        assert_eq!(numbers, [1, 3, 4, 5, 6, 7]);
        let bad: Vec<usize> = read
            .lines
            .iter()
            .filter(|line| line.reading == Reading::Bad)
            .map(|line| line.n)
            .collect();
        assert_eq!(bad, [3, 7]);
        let note = |n: usize| match &read.lines.iter().find(|l| l.n == n).unwrap().reading {
            Reading::Good { note, warned, .. } => (note.clone(), *warned),
            Reading::Bad => panic!("line {n} is bad"),
        };
        assert_eq!(note(1), (Note::NotInArauto, false));
        assert_eq!(note(4), (Note::SameAsLine(1), false));
        assert_eq!(note(5), (Note::NotInArauto, true));
        assert_eq!(note(6), (Note::IsNumber("operador".to_owned()), true));
        assert_eq!(read.count, 3);
        assert_eq!(
            analysis.warned,
            [
                "gives:11987600005".to_owned(),
                format!("gives:{}", phone(1))
            ]
        );
        assert_eq!(analysis.first_problem(), Some(Problem::BadLine(3)));
        // More than 64 lines are not read at all.
        let many = Typed {
            from: (0..65).map(|n| format!("+{}\n", phone(n + 10))).collect(),
            ..typed
        };
        let analysis = analyse(&many, &picks, &OrdersRecord::default());
        assert!(analysis.readback.is_none() && analysis.lines.is_empty());
        assert_eq!(analysis.first_problem(), Some(Problem::TooMany(65)));
    }
}
