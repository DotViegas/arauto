//! The guide that connects a Hermes: the steps of `docs/02-como-funciona.md`, section 1, as a
//! window over the list of Hermes, with the path of the steps at its top.
//!
//! Each step is a page of the hub. The page that shows the block is the answer to the form
//! that issued the invite, and the invite is in no other page: opening an address never
//! shows one again. The verification code is typed by the admin and never shown here, not
//! even back to whoever typed it wrong.
//!
//! Like [`super::view`], this module only writes: every value goes through [`Html::text`].

use uuid::Uuid;
use zeroize::Zeroizing;

use super::html::{Arg, Html};
use super::text::k;
use super::view::{self, Frame, HermesPage, Over};
use crate::repo::invite::INVITE_VALIDITY_MS;

/// After how many seconds without a device the Waiting screen lists what may be wrong.
const QUIET_S: i64 = 90;

/// The mascot, drawn by the project frame by frame: no value of anyone goes inside it.
const DANCING: &str = include_str!("assets/mascot-dancing.svg");
const WAVING: &str = include_str!("assets/mascot-waving.svg");

/// The names of the steps, in the path at the top.
const STEPS: [&str; 4] = [k("Name"), k("Message"), k("Code"), k("Done")];

/// Beside the sentence: how to send it. The Hermes may answer a reply with a question.
const SEND_HINT: &str = k(
    "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
);

/// Inside the fallback: the sentence and the command carry the same invite.
/// What the window of an approved line and the screen after the approval say of the
/// confirmation.
pub(super) const APPROVED_WAITING: &str = k(
    "The connector confirms this approval when it runs. If the Hermes said its program was replaced or a key was made, its gateway must restart once. Issuing another invite cancels this approval.",
);
const ONE_OR_OTHER: &str = k("Send one or the other, not both: they carry the same invite.");

/// What the hub observes, one after the other, while a Hermes connects.
const OBSERVED: [&str; 4] = [
    k("Invite issued"),
    k("A device used the invite"),
    k("Code checked and request approved"),
    k("The connector confirmed its first credential"),
];

/// How often a page that waits for the Hermes asks the hub again, in seconds.
const WAITING_REFRESH_S: u32 = 3;

/// What the hub saw and what the device said, of a request that waits for its code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The address the enrollment came from, as the hub saw it.
    pub address: Option<String>,
    /// How long ago, in words.
    pub when: String,
    /// The name of the machine. Said by the device, not verified.
    pub hostname: String,
    /// Its system. Said by the device.
    pub os: String,
    /// The version of its Hermes. Said by the device.
    pub hermes: String,
}

/// The last use of the device a reconnect replaces, as the admin is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastUse {
    /// The device.
    pub device: Uuid,
    /// When, in milliseconds: it goes back with the answer, to say which use was answered.
    pub at: Option<i64>,
    /// The state of the device, as stored.
    pub state: &'static str,
    /// How long ago, in words.
    pub ago: String,
    /// From where. Stored as it came: shown escaped.
    pub address: Option<String>,
}

/// Why a request ended without an approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The fifth wrong code.
    Denied,
    /// The 10 minutes of the code passed.
    Expired,
    /// There is nothing to wait for: no invite and no request.
    Nothing,
    /// The identity was copied before the approval.
    Copied,
}

/// One screen of the guide.
pub enum Screen {
    /// Step 1: which name the Hermes has.
    Name {
        /// What was typed, when the form came back.
        typed: String,
        /// Why it came back.
        error: Option<&'static str>,
    },
    /// Step 2: the connection sentence and, behind it, the block: the only time they appear.
    Block {
        /// The Hermes.
        name: String,
        /// The connection sentence, in the language of the page. **It holds the invite.**
        sentence: Zeroizing<String>,
        /// The block of the command, the fallback. **It holds the invite.** `None` when the hub
        /// holds no Linux connector, the only one the approved block downloads.
        command: Option<Zeroizing<String>>,
        /// For how many seconds the invite is still accepted.
        seconds: i64,
        /// When the invite stops being accepted, in milliseconds.
        expires_at: i64,
        /// The Hermes has a device in use.
        reconnect: bool,
        /// An invite issued before was cancelled by this one.
        cancelled: bool,
        /// A request in progress was closed by this one.
        closed_request: bool,
        /// The reason the message gives, when it is not the one the admin saw in the window
        /// that issued it: the state changed in between.
        changed: Option<String>,
    },
    /// The hub serves no connector, so there is no block to make. Nothing was issued.
    NoConnector {
        /// The Hermes.
        name: String,
    },
    /// Step 3, while no device used the invite.
    Waiting {
        /// The Hermes.
        name: String,
        /// For how many seconds the invite is still accepted.
        seconds: i64,
        /// The Hermes has a device in use.
        reconnect: bool,
        /// What the screen was drawn from: the form that issues another invite carries it.
        replaces: String,
    },
    /// Step 3: a device used the invite and the admin types its code.
    Code {
        /// The Hermes.
        name: String,
        /// What is known of the request.
        facts: Box<Facts>,
        /// On a reconnect, the last use of the device in use.
        last_use: Option<Box<LastUse>>,
        /// What the last answer got wrong, already in the language of the page.
        error: Option<String>,
        /// For how many seconds the code is still accepted.
        seconds: i64,
    },
    /// Step 3 ended without an approval.
    Ended {
        /// The Hermes.
        name: String,
        /// Why.
        why: Ended,
        /// What the screen was drawn from: the form that issues another invite carries it.
        replaces: String,
    },
    /// Step 4: approved, or already connected.
    Done {
        /// The Hermes.
        name: String,
        /// The connector confirmed its first credential.
        connected: bool,
        /// The Hermes had a device in use when this one was approved.
        reconnect: bool,
    },
    /// The hub did not do what was asked.
    Failed {
        /// One fixed sentence.
        sentence: &'static str,
    },
}

/// A screen may hold a block, and a block holds an invite: what is printed of a screen is
/// only which one it is.
impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Screen::Name { .. } => "Screen::Name",
            Screen::Block { .. } => "Screen::Block",
            Screen::NoConnector { .. } => "Screen::NoConnector",
            Screen::Waiting { .. } => "Screen::Waiting",
            Screen::Code { .. } => "Screen::Code",
            Screen::Ended { .. } => "Screen::Ended",
            Screen::Done { .. } => "Screen::Done",
            Screen::Failed { .. } => "Screen::Failed",
        })
    }
}

impl Screen {
    /// Which step of the path the screen is.
    const fn step(&self) -> usize {
        match self {
            Screen::Name { .. } | Screen::Failed { .. } => 0,
            Screen::Block { .. } | Screen::NoConnector { .. } => 1,
            Screen::Waiting { .. } | Screen::Code { .. } | Screen::Ended { .. } => 2,
            Screen::Done { .. } => 3,
        }
    }

    /// The Hermes the screen is about, and whether it had a device in use.
    fn subject(&self) -> Option<(&str, bool)> {
        match self {
            Screen::Name { .. } | Screen::Failed { .. } => None,
            Screen::Block {
                name, reconnect, ..
            }
            | Screen::Waiting {
                name, reconnect, ..
            }
            | Screen::Done {
                name, reconnect, ..
            } => Some((name, *reconnect)),
            Screen::Code { name, last_use, .. } => Some((name, last_use.is_some())),
            Screen::NoConnector { name } | Screen::Ended { name, .. } => Some((name, false)),
        }
    }

    /// A page that waits for something the hub observes asks again by itself.
    const fn refresh(&self) -> Option<u32> {
        match self {
            Screen::Waiting { .. } => Some(WAITING_REFRESH_S),
            // Approved and not connected yet: the connector confirms when Hermes starts it,
            // and only another reading of the page shows it.
            Screen::Done {
                connected: false, ..
            } => Some(WAITING_REFRESH_S),
            _ => None,
        }
    }
}

fn mascot(html: &mut Html, dancing: bool) {
    html.raw(if dancing {
        "<div class=\"mascot dancing\" aria-hidden=\"true\">"
    } else {
        "<div class=\"mascot waving\" aria-hidden=\"true\">"
    })
    .raw(if dancing { DANCING } else { WAVING })
    .raw("</div>");
}

/// The lines that light up as the hub observes each thing. `seen` of them are done and the
/// next one is being waited for.
fn observed(html: &mut Html, seen: usize, address: Option<&str>) {
    html.raw("<ul class=\"steps\">");
    for (index, label) in OBSERVED.into_iter().enumerate() {
        html.raw(if index < seen {
            "<li class=\"ok\">"
        } else if index == seen {
            "<li class=\"waiting\">"
        } else {
            "<li>"
        });
        match address {
            Some(address) if index == 1 && seen > 1 => html.fill(
                k("A device used the invite, from {address}"),
                &[("address", Arg::Text(address))],
            ),
            _ => html.t(label),
        };
        if index == seen {
            html.raw(": ").t(k("waiting…"));
        }
        html.raw("</li>");
    }
    html.raw("</ul>");
}

/// A hidden field with the name of the Hermes.
fn named(html: &mut Html, name: &str) {
    html.raw("<input type=\"hidden\" name=\"name\" value=\"")
        .text(name)
        .raw("\">");
}

fn hint(html: &mut Html, sentence: &'static str) {
    html.raw("<p class=\"hint\">").t(sentence).raw("</p>");
}

#[allow(clippy::too_many_lines)]
fn body(html: &mut Html, screen: &Screen) {
    match screen {
        Screen::Name { typed, error } => {
            html.raw("<div class=\"howto\"><p><b>")
                .t(k("How it works:"))
                .raw("</b></p><ol><li>")
                .t(k("You give the Hermes a name here."))
                .raw("</li><li>")
                .t(k("The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else."))
                .raw("</li><li>")
                .t(k("The Hermes downloads the connector from this hub and answers with a verification code of 6 digits."))
                .raw("</li><li>")
                .t(k("You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms."))
                .raw("</li></ol></div><h3>")
                .t(k("What name will this Hermes have in the panel?"))
                .raw("</h3><form id=\"guide-form\" method=\"post\" action=\"/hermes/connect\" autocomplete=\"off\"><div class=\"field\"><label for=\"guide-name\">")
                .t(k("Name of the Hermes"))
                .raw("</label><input id=\"guide-name\" name=\"name\" type=\"text\" maxlength=\"32\" spellcheck=\"false\" autocapitalize=\"none\" required autofocus value=\"")
                .text(typed)
                .raw("\">");
            hint(
                html,
                k(
                    "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
                ),
            );
            html.raw("</div></form>");
            if let Some(error) = error {
                html.raw("<p class=\"error\" role=\"alert\">")
                    .t(error)
                    .raw("</p>");
            }
        }
        Screen::Block {
            name,
            sentence,
            command,
            seconds,
            expires_at,
            reconnect,
            cancelled,
            closed_request,
            changed,
        } => {
            html.raw("<div class=\"center preparation\">");
            mascot(html, true);
            html.raw("<h3 id=\"guide-preparing\" tabindex=\"-1\">")
                .t(k("Preparing a secure connection"))
                .raw("</h3><p class=\"hint\">")
                .fill(k("The hub drew the invite of {name}."), &[("name", Arg::Text(name))])
                .raw("</p></div><div class=\"after\"><p class=\"deadline\">")
                .fill(
                    k("Invite for {name} · valid for another {time} · until {deadline} · single use"),
                    &[
                        ("name", Arg::Strong(name)),
                        ("time", Arg::Clock(*seconds)),
                        ("deadline", Arg::Time(*expires_at)),
                    ],
                )
                .raw("</p><div class=\"block\" id=\"guide-block\" aria-describedby=\"guide-how\"><pre id=\"guide-text\">")
                .text(sentence)
                .raw("</pre><span class=\"stamp\" aria-hidden=\"true\"><span>")
                .t(k("Copied"))
                .raw("</span></span></div><p class=\"hint\" id=\"guide-how\"><span class=\"with-script\">")
                .t(k("Click the message to copy it."))
                .raw(" </span><span class=\"no-script\">")
                .t(k("Select the whole text and copy it."))
                .raw(" </span>")
                .t(k("It only appears here, once."))
                .raw("</p><p class=\"copied-note\" id=\"guide-copied\" role=\"status\" hidden></p>");
            hint(html, SEND_HINT);
            html.raw("<p class=\"hint\">")
                .fill(
                    k("The Hermes reads the deadline in UTC: {utc}."),
                    &[(
                        "utc",
                        Arg::Text(&format!(
                            "{} UTC",
                            crate::join_page::utc_minute(*expires_at)
                        )),
                    )],
                )
                .raw("</p>");
            // The block of the command stays behind a control: the same invite, the way it
            // always was, for a Hermes the sentence does not move.
            if let Some(command) = command {
                html.raw("<details class=\"fallback\"><summary>")
                    .t(k("Show the command instead"))
                    .raw("</summary><div class=\"block\" id=\"guide-command-block\" aria-describedby=\"guide-command-how\"><pre id=\"guide-command\">")
                    .text(command)
                    .raw("</pre><span class=\"stamp\" aria-hidden=\"true\"><span>")
                    .t(k("Copied"))
                    .raw("</span></span></div><p class=\"hint\" id=\"guide-command-how\"><span class=\"with-script\">")
                    .t(k("Click the block to copy it."))
                    .raw(" </span><span class=\"no-script\">")
                    .t(k("Select the whole text and copy it."))
                    .raw(" </span></p>");
                hint(html, ONE_OR_OTHER);
                html.raw("</details>");
            }
            if let Some(reason) = changed {
                html.raw("<p class=\"hint\">")
                    .fill(
                        k("The state changed after you opened this window. The message gives the reason as it is now: {reason}"),
                        &[("reason", Arg::Text(reason))],
                    )
                    .raw("</p>");
            }
            if *cancelled {
                hint(html, k("The invite issued before was cancelled."));
            }
            if *closed_request {
                hint(html, k("The request that was in progress was closed."));
            }
            if *reconnect {
                hint(
                    html,
                    k(
                        "The device in use keeps working until the new one confirms its first credential.",
                    ),
                );
            }
            html.raw("</div>");
        }
        Screen::NoConnector { name } => {
            html.raw("<h3>")
                .t(k("This hub has no connector to serve"))
                .raw("</h3><p>")
                .fill(
                    k("The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server."),
                    &[
                        ("name", Arg::Text(name)),
                        ("command", Arg::Code("arauto-hub admin invite")),
                    ],
                )
                .raw("</p>");
        }
        Screen::Waiting { name, seconds, .. } => {
            html.raw("<h3>")
                .fill(
                    k("Waiting for the Hermes {name}"),
                    &[("name", Arg::Text(name))],
                )
                .raw("</h3>");
            observed(html, 1, None);
            hint(
                html,
                k(
                    "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
                ),
            );
            hint(
                html,
                k(
                    "The message of this invite cannot be shown again. If it did not reach the Hermes, issue another invite: this one then stops working.",
                ),
            );
            if INVITE_VALIDITY_MS / 1_000 - *seconds >= QUIET_S {
                html.raw("<div class=\"warnings\"><p><b>")
                    .t(k("Nothing has arrived yet"))
                    .raw("</b></p><ul><li>")
                    .t(k(
                        "The Hermes may be waiting for your approval to run the command, in the command channel.",
                    ))
                    .raw("</li><li>")
                    .t(k(
                        "The Hermes may not reach this hub: check that its machine reaches the address of the hub.",
                    ))
                    .raw("</li><li>")
                    .t(k(
                        "The message may have gone to another chat: it only works in the command channel of this Hermes.",
                    ))
                    .raw("</li></ul></div>");
            }
        }
        Screen::Code {
            name,
            facts,
            last_use,
            error,
            ..
        } => {
            let lang = html.lang();
            html.raw("<h3>")
                .fill(
                    k("A device used the invite of {name}"),
                    &[("name", Arg::Text(name))],
                )
                .raw("</h3>");
            observed(html, 2, facts.address.as_deref());
            let address = facts
                .address
                .as_deref()
                .unwrap_or_else(|| super::text::tr(lang, k("unknown")));
            html.raw("<dl class=\"facts\"><dt>")
                .t(k("Seen by the hub"))
                .raw("</dt><dd>")
                .fill(
                    k("address {address}, {when}"),
                    &[("address", Arg::Text(address)), ("when", Arg::Text(&facts.when))],
                )
                .raw("</dd><dt>")
                .t(k("Said by the device"))
                .raw("</dt><dd>")
                .fill(
                    k("machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)"),
                    &[
                        ("host", Arg::Text(&facts.hostname)),
                        ("os", Arg::Text(&facts.os)),
                        ("version", Arg::Text(&facts.hermes)),
                    ],
                )
                .raw("</dd></dl><form id=\"guide-deny\" method=\"post\" action=\"/hermes/deny\" hidden>");
            named(html, name);
            html.raw("</form><form id=\"guide-form\" method=\"post\" action=\"/hermes/approve\" autocomplete=\"off\">");
            named(html, name);
            if let Some(last) = last_use {
                html.raw("<input type=\"hidden\" name=\"seen_device\" value=\"")
                    .text(&last.device.as_hyphenated().to_string())
                    .raw("\"><input type=\"hidden\" name=\"seen_at\" value=\"");
                if let Some(at) = last.at {
                    html.text(&at.to_string());
                }
                html.raw("\"><input type=\"hidden\" name=\"seen_state\" value=\"")
                    .raw(last.state)
                    .raw("\"><fieldset class=\"recognize\"><legend>");
                let never = super::text::tr(lang, k("never"));
                let when = last
                    .at
                    .map_or(Arg::Text(never), |at| Arg::AgoTime(&last.ago, at));
                match &last.address {
                    Some(address) => html.fill(
                        k("Last use of the current device: {when}, from {address}. Do you recognize it?"),
                        &[("when", when), ("address", Arg::Text(address))],
                    ),
                    None => html.fill(
                        k("Last use of the current device: {when}. Do you recognize it?"),
                        &[("when", when)],
                    ),
                };
                html.raw("</legend><label class=\"option\"><input type=\"radio\" name=\"use\" value=\"recognized\" required><span>")
                    .t(k("I recognize it"))
                    .raw("</span></label><label class=\"option\"><input type=\"radio\" name=\"use\" value=\"not-recognized\"><span>")
                    .t(k("I do not recognize it"))
                    .raw("</span></label>");
                hint(
                    html,
                    k("If the Hermes was off and there was use, someone used a copy."),
                );
                html.raw("</fieldset>");
            }
            html.raw("<div class=\"field\"><label for=\"guide-code\">")
                .t(k("Verification code"))
                .raw("</label><input class=\"code\" id=\"guide-code\" name=\"code\" type=\"text\" inputmode=\"numeric\" maxlength=\"7\" placeholder=\"000-000\" spellcheck=\"false\" required autofocus>");
            hint(
                html,
                k(
                    "Type the code the Hermes answered in the command channel. The panel never shows this code.",
                ),
            );
            html.raw("</div></form>");
            if let Some(error) = error {
                html.raw("<p class=\"error\" role=\"alert\">")
                    .text(error)
                    .raw("</p>");
            }
        }
        Screen::Ended { name, why, .. } => {
            let (title, said) = match why {
                Ended::Denied => (
                    k("The request of {name} was denied"),
                    k(
                        "The code was typed wrong for the fifth time. To connect, issue another invite.",
                    ),
                ),
                Ended::Expired => (
                    k("The request of {name} expired"),
                    k("The 10 minutes to type the code passed. To connect, issue another invite."),
                ),
                Ended::Copied => (
                    k("The identity of {name} was copied"),
                    k(
                        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
                    ),
                ),
                Ended::Nothing => (
                    k("{name} has no invite and no request waiting"),
                    k(
                        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
                    ),
                ),
            };
            html.raw("<h3>")
                .fill(title, &[("name", Arg::Text(name))])
                .raw("</h3><p>")
                .t(said)
                .raw("</p>");
        }
        Screen::Done {
            name,
            connected,
            reconnect,
        } => {
            html.raw("<div class=\"center\">");
            mascot(html, false);
            html.raw("<h3>")
                .fill(
                    if *connected {
                        k("{name} is connected")
                    } else {
                        k("{name} was approved")
                    },
                    &[("name", Arg::Text(name))],
                )
                .raw("</h3>");
            observed(html, if *connected { 4 } else { 3 }, None);
            hint(
                html,
                match (*connected, *reconnect) {
                    (true, _) => k("What it may do on each number is in the Accesses tab."),
                    (false, true) => k(
                        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
                    ),
                    (false, false) => k(
                        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
                    ),
                },
            );
            if !*connected {
                hint(html, APPROVED_WAITING);
            }
            html.raw("</div>");
        }
        Screen::Failed { sentence } => {
            html.raw("<h3>")
                .t(k("The hub did not do what was asked"))
                .raw("</h3><p>")
                .t(sentence)
                .raw("</p>");
        }
    }
}

fn footer(html: &mut Html, screen: &Screen) {
    html.raw("<div class=\"modal-foot\">");
    match screen {
        Screen::Name { .. } => {
            html.raw("<button class=\"button primary\" type=\"submit\" form=\"guide-form\">")
                .t(k("Issue invite"))
                .raw("</button><a class=\"button\" href=\"/\">")
                .t(k("Cancel"))
                .raw("</a>");
        }
        Screen::Block { name, .. } => {
            html.raw("<a class=\"button primary after-copy appear\" id=\"guide-next\" href=\"/hermes/wait?name=")
                .text(name)
                .raw("\">")
                .t(k("Next step"))
                .raw("</a><span>")
                .fill(
                    k("After copying, paste it into the command channel of the Hermes {name}. Nowhere else."),
                    &[("name", Arg::Text(name))],
                )
                .raw("</span>");
        }
        Screen::Waiting {
            name,
            seconds,
            replaces,
            ..
        } => {
            html.raw("<form method=\"post\" action=\"/hermes/invite\">");
            named(html, name);
            html.raw("<input type=\"hidden\" name=\"replaces\" value=\"")
                .text(replaces)
                .raw("\"><button class=\"button\" type=\"submit\">")
                .t(k("Issue another invite"))
                .raw("</button></form><span>")
                .fill(
                    k("The invite is valid for another {time}. This screen refreshes by itself."),
                    &[("time", Arg::Clock(*seconds))],
                )
                .raw("</span>");
        }
        Screen::Code { seconds, .. } => {
            html.raw("<button class=\"button primary\" type=\"submit\" form=\"guide-form\">")
                .t(k("Approve"))
                .raw("</button><button class=\"button\" type=\"submit\" form=\"guide-deny\">")
                .t(k("Deny"))
                .raw("</button><span>")
                .fill(
                    k("Confirm within {time}."),
                    &[("time", Arg::Clock(*seconds))],
                )
                .raw("</span>");
        }
        Screen::Ended { name, replaces, .. } => {
            html.raw("<form method=\"post\" action=\"/hermes/invite\">");
            named(html, name);
            html.raw("<input type=\"hidden\" name=\"replaces\" value=\"")
                .text(replaces)
                .raw("\"><button class=\"button primary\" type=\"submit\">")
                .t(k("Issue another invite"))
                .raw("</button></form><a class=\"button\" href=\"/\">")
                .t(k("Close"))
                .raw("</a>");
        }
        Screen::Done { .. } => {
            html.raw("<a class=\"button primary\" href=\"/accesses\">")
                .t(k("See accesses"))
                .raw("</a><a class=\"button\" href=\"/\">")
                .t(k("Finish"))
                .raw("</a>");
        }
        Screen::NoConnector { .. } | Screen::Failed { .. } => {
            html.raw("<a class=\"button primary\" href=\"/\">")
                .t(k("Close"))
                .raw("</a>");
        }
    }
    html.raw("</div>");
}

/// The page of the Hermes with the guide over it.
#[must_use]
pub fn page(frame: &Frame, hermes: &HermesPage, screen: &Screen) -> String {
    let mut html = Html::new(frame.lang);
    view::hermes_into(
        &mut html,
        frame,
        hermes,
        Over::Guide {
            refresh: screen.refresh(),
        },
    );
    // What the script of the panel says when the block is copied comes from here, in the
    // language of the page: the script has no sentence of its own.
    html.raw(if matches!(screen, Screen::Block { .. }) {
        "<dialog open class=\"modal guide preparing\" id=\"guide\" aria-labelledby=\"guide-title\" data-copied=\""
    } else {
        "<dialog open class=\"modal guide\" id=\"guide\" aria-labelledby=\"guide-title\" data-copied=\""
    })
    .t(k("Copied to the clipboard."))
    .raw("\" data-selected=\"")
    .t(k("The text is selected. Copy it with Ctrl+C."))
    .raw("\"><div class=\"modal-head\"><h2 id=\"guide-title\">");
    match screen.subject() {
        Some((name, true)) => html.fill(k("Reconnect {name}"), &[("name", Arg::Text(name))]),
        Some((name, false)) => html.fill(k("Connect {name}"), &[("name", Arg::Text(name))]),
        // The first window of the first Hermes of the organization says so in its title.
        None if hermes.rows.is_empty() => html.t(k("Let's connect your first Hermes")),
        None => html.t(k("Connect Hermes")),
    };
    html.raw("</h2><a class=\"close\" href=\"/\" aria-label=\"")
        .t(k("Close"))
        .raw("\"><svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg></a></div><ol class=\"path\" aria-label=\"")
        .t(k("Steps"))
        .raw("\">");
    let step = screen.step();
    for (index, label) in STEPS.into_iter().enumerate() {
        html.raw(if index < step {
            "<li class=\"done\"><span class=\"dot\">"
        } else if index == step {
            "<li class=\"current\" aria-current=\"step\"><span class=\"dot\">"
        } else {
            "<li><span class=\"dot\">"
        })
        .number(index.saturating_add(1))
        .raw("</span><span class=\"label\">")
        .t(label)
        .raw("</span></li>");
    }
    html.raw("</ol><div class=\"modal-body\">");
    body(&mut html, screen);
    html.raw("</div>");
    footer(&mut html, screen);
    html.raw("</dialog>").raw(view::TAIL);
    html.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    fn shown(screen: &Screen) -> String {
        page(&Frame::default(), &HermesPage::default(), screen)
    }

    fn clean(page: &str) {
        assert!(!page.contains("<script>"), "{page}");
        assert!(!page.contains("<img"), "{page}");
        assert!(!page.contains("\"'>"), "{page}");
        assert!(
            page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{page}"
        );
        // The one script of a page is the file of the panel, by its address.
        assert_eq!(page.matches("<script").count(), 1, "{page}");
        assert!(page.contains("<script src=\"/assets/panel.js\"></script>"));
        assert!(
            !page.contains(" style=\""),
            "the policy of the page blocks inline styles"
        );
        assert!(!page.contains(" onclick=") && !page.contains("javascript:"));
    }

    const TIME: &str = "<time datetime=\"2026-11-14T12:34:56Z\">2026-11-14 12:34 UTC</time>";

    fn block_screen() -> Screen {
        Screen::Block {
            name: "ana".to_owned(),
            sentence: Zeroizing::new("the sentence".to_owned()),
            command: None,
            seconds: 900,
            expires_at: 1_794_659_696_000,
            reconnect: false,
            cancelled: false,
            closed_request: false,
            changed: None,
        }
    }

    fn code_screen() -> Screen {
        Screen::Code {
            name: "ana".to_owned(),
            facts: Box::new(Facts {
                address: None,
                when: "now".to_owned(),
                hostname: "pc".to_owned(),
                os: "linux".to_owned(),
                hermes: "0.22".to_owned(),
            }),
            last_use: Some(Box::new(LastUse {
                device: Uuid::nil(),
                at: Some(1_794_659_696_000),
                state: "active",
                ago: "3 h ago".to_owned(),
                address: None,
            })),
            error: None,
            seconds: 0,
        }
    }

    #[test]
    fn the_block_screen_gives_the_deadline_in_a_time_element_and_in_utc_for_the_hermes() {
        let page = shown(&block_screen());
        assert!(
            page.contains(&format!(
                "valid for another <b data-seconds=\"900\">15:00</b> · until {TIME} · single use"
            )),
            "{page}"
        );
        assert!(
            page.contains(
                "<p class=\"hint\">The Hermes reads the deadline in UTC: 2026-11-14 12:34 UTC.</p>"
            ),
            "{page}"
        );
    }

    #[test]
    fn the_last_use_of_the_current_device_has_a_time_element() {
        let page = shown(&code_screen());
        assert!(
            page.contains(&format!(
                "Last use of the current device: 3 h ago ({TIME}). Do you recognize it?"
            )),
            "{page}"
        );
    }

    #[test]
    fn pages_have_no_absolute_time_outside_a_time_element() {
        let frame = Frame {
            restored: Some(Some(1_794_659_696_000)),
            ..Frame::default()
        };
        for screen in [block_screen(), code_screen()] {
            let page = page(&frame, &HermesPage::default(), &screen);
            assert!(page.contains(TIME), "{page}");
            let bytes = page.as_bytes();
            let mut found = 0;
            for at in 0..bytes.len().saturating_sub(16) {
                let w = &bytes[at..at + 16];
                let shaped = w.iter().enumerate().all(|(i, b)| match i {
                    4 | 7 => *b == b'-',
                    10 => *b == b' ',
                    13 => *b == b':',
                    _ => b.is_ascii_digit(),
                });
                if !shaped {
                    continue;
                }
                found += 1;
                let inside = page[..at].ends_with("UTC\">") || page[..at].ends_with("Z\">");
                let hint = page[..at].ends_with("reads the deadline in UTC: ");
                assert!(
                    inside || hint,
                    "a bare time at {at}: {}",
                    &page[at.saturating_sub(40)..at + 20]
                );
            }
            assert!(found >= 2, "the times were looked for: {found}");
        }
    }

    #[test]
    fn what_came_from_outside_is_shown_as_text_in_every_screen() {
        let hostile = || HOSTILE.to_owned();
        let facts = Facts {
            address: Some(hostile()),
            when: hostile(),
            hostname: hostile(),
            os: hostile(),
            hermes: hostile(),
        };
        let last_use = LastUse {
            device: Uuid::nil(),
            at: Some(7),
            state: "active",
            ago: hostile(),
            address: Some(hostile()),
        };
        for screen in [
            Screen::Name {
                typed: hostile(),
                error: Some("Use only lowercase letters."),
            },
            Screen::Block {
                name: hostile(),
                sentence: Zeroizing::new(hostile()),
                command: Some(Zeroizing::new(hostile())),
                seconds: 900,
                expires_at: 1_794_659_696_000,
                reconnect: true,
                cancelled: true,
                closed_request: true,
                changed: Some(hostile()),
            },
            Screen::NoConnector { name: hostile() },
            Screen::Waiting {
                name: hostile(),
                seconds: 5,
                reconnect: false,
                replaces: "none".to_owned(),
            },
            Screen::Code {
                name: hostile(),
                facts: Box::new(facts),
                last_use: Some(Box::new(last_use)),
                error: Some(hostile()),
                seconds: 600,
            },
            Screen::Ended {
                name: hostile(),
                why: Ended::Nothing,
                replaces: "none".to_owned(),
            },
            Screen::Done {
                name: hostile(),
                connected: false,
                reconnect: true,
            },
        ] {
            clean(&shown(&screen));
        }
    }

    #[test]
    fn the_recognition_question_is_required_in_the_form() {
        let last_use = LastUse {
            device: Uuid::nil(),
            at: Some(7),
            state: "active",
            ago: "now".to_owned(),
            address: None,
        };
        let page = shown(&Screen::Code {
            name: "ana".to_owned(),
            facts: Box::new(Facts {
                address: None,
                when: "now".to_owned(),
                hostname: "pc".to_owned(),
                os: "linux".to_owned(),
                hermes: "0.22".to_owned(),
            }),
            last_use: Some(Box::new(last_use)),
            error: None,
            seconds: 0,
        });
        assert!(
            page.contains("name=\"use\" value=\"recognized\" required"),
            "{page}"
        );
    }

    #[test]
    fn the_waiting_screen_says_the_message_is_gone_and_offers_another_invite() {
        let waiting = |seconds| {
            shown(&Screen::Waiting {
                name: "ana".to_owned(),
                seconds,
                reconnect: false,
                replaces: "invite-1234".to_owned(),
            })
        };
        // Just issued: the hint and the way on, and no list of causes yet.
        let fresh = waiting(900);
        assert!(fresh.contains("The message of this invite cannot be shown again."));
        assert!(fresh.contains("action=\"/hermes/invite\""));
        assert!(fresh.contains("name=\"replaces\" value=\"invite-1234\""));
        assert!(fresh.contains("Issue another invite"));
        assert!(!fresh.contains("Nothing has arrived yet"));
        // 89 seconds in: still quiet. 91 seconds in: the three causes, with no script.
        assert!(!waiting(811).contains("Nothing has arrived yet"));
        let late = waiting(809);
        assert!(late.contains("<div class=\"warnings\"><p><b>Nothing has arrived yet</b></p><ul>"));
        assert!(late.contains("may be waiting for your approval"));
        assert!(late.contains("may not reach this hub"));
        assert!(late.contains("command channel of this Hermes."));
    }

    #[test]
    fn the_path_marks_the_step_and_only_a_page_that_waits_asks_again() {
        let waiting = shown(&Screen::Waiting {
            name: "ana".to_owned(),
            seconds: 61,
            reconnect: false,
            replaces: "none".to_owned(),
        });
        assert!(waiting.contains("<meta http-equiv=\"refresh\" content=\"3\">"));
        assert!(waiting.contains("<b data-seconds=\"61\">01:01</b>"));
        assert_eq!(waiting.matches("<li class=\"done\">").count(), 2);
        assert!(
            waiting.contains(
                "<li class=\"current\" aria-current=\"step\"><span class=\"dot\">3</span>"
            )
        );
        // What is under the guide cannot be reached while it is open.
        assert!(waiting.contains("<div class=\"page\" inert>"));

        let code = shown(&Screen::Code {
            name: "ana".to_owned(),
            facts: Box::new(Facts {
                address: None,
                when: "now".to_owned(),
                hostname: "pc".to_owned(),
                os: "linux".to_owned(),
                hermes: "0.22".to_owned(),
            }),
            last_use: None,
            error: None,
            seconds: 0,
        });
        // A page with a field being typed never reloads by itself.
        assert!(!code.contains("http-equiv"));
        assert!(code.contains("<b data-seconds=\"0\">00:00</b>"));
        assert!(
            !code.contains("name=\"use\""),
            "no question about a use there was not"
        );
        assert!(
            code.contains("action=\"/hermes/approve\"") && code.contains("action=\"/hermes/deny\"")
        );

        let name = shown(&Screen::Name {
            typed: String::new(),
            error: None,
        });
        // While the organization has no Hermes, the window says it is about the first one.
        assert!(name.contains("<h2 id=\"guide-title\">Let&#39;s connect your first Hermes</h2>"));
        assert!(!name.contains("class=\"done\""));
        // "How it works" is on the first window, with its four steps.
        assert!(name.contains("How it works:"));
        assert!(
            name.contains("You give the Hermes a name here.")
                && name.contains("verification code of 6 digits")
        );
        // With a Hermes in the list the window has the plain title again.
        let listed = HermesPage {
            rows: vec![crate::panel::view::HermesRow {
                name: "ana".to_owned(),
                tone: crate::panel::view::Tone::Ok,
                state: "x",
                numbers: vec![],
                connector: None,
                standing: None,
                seen: String::new(),
                explain: None,
                explain_tone: crate::panel::view::Tone::Idle,
                action: crate::panel::view::RowAction::Continue,
                orders: String::new(),
                orders_recorded: false,
            }],
            ..HermesPage::default()
        };
        let with_one = page(
            &Frame::default(),
            &listed,
            &Screen::Name {
                typed: String::new(),
                error: None,
            },
        );
        assert!(with_one.contains("<h2 id=\"guide-title\">Connect Hermes</h2>"));
        assert!(!with_one.contains("connect your first Hermes"));
        let done = shown(&Screen::Done {
            name: "ana".to_owned(),
            connected: true,
            reconnect: false,
        });
        assert_eq!(done.matches("<li class=\"done\">").count(), 3);
        assert_eq!(done.matches("<li class=\"ok\">").count(), 4);
        // Connected is the end: the page stops asking.
        assert!(!done.contains("http-equiv=\"refresh\""));
        // Approved and not connected yet: only another reading shows the connector arriving.
        let approved = shown(&Screen::Done {
            name: "ana".to_owned(),
            connected: false,
            reconnect: false,
        });
        assert!(approved.contains("<meta http-equiv=\"refresh\" content=\"3\">"));
    }

    #[test]
    fn the_block_waits_for_the_mascot_and_the_way_on_waits_for_the_copy() {
        let block = shown(&Screen::Block {
            name: "ana".to_owned(),
            sentence: Zeroizing::new("the sentence".to_owned()),
            command: Some(Zeroizing::new("the command".to_owned())),
            seconds: 900,
            expires_at: 1_794_659_696_000,
            reconnect: false,
            cancelled: false,
            closed_request: false,
            changed: None,
        });
        assert!(block.contains("class=\"modal guide preparing\""));
        assert!(block.contains("<pre id=\"guide-text\">the sentence</pre>"));
        // The command stays behind a control, with the same invite, and the page says so.
        assert!(
            block.contains(
                "<details class=\"fallback\"><summary>Show the command instead</summary>"
            )
        );
        assert!(block.contains("<pre id=\"guide-command\">the command</pre>"));
        let (before, behind) = block.split_once("<details").unwrap();
        assert!(before.contains("the sentence") && !before.contains("the command"));
        assert!(behind.contains("the command") && !behind.contains("the sentence"));
        assert!(block.contains("Send it as a new message, not as a reply to one of its notices."));
        assert!(block.contains("Send one or the other, not both"));
        assert!(block.contains("class=\"button primary after-copy appear\" id=\"guide-next\" href=\"/hermes/wait?name=ana\""));
        assert!(block.contains("<h2 id=\"guide-title\">Connect ana</h2>"));
        assert!(!block.contains("was cancelled") && !block.contains("keeps working"));
        assert!(
            !block.contains("http-equiv"),
            "the block is never asked for again"
        );
        // Without a command (no Linux connector) the control is not there.
        let bare = shown(&Screen::Block {
            name: "ana".to_owned(),
            sentence: Zeroizing::new("the sentence".to_owned()),
            command: None,
            seconds: 900,
            expires_at: 1_794_659_696_000,
            reconnect: false,
            cancelled: false,
            closed_request: false,
            changed: None,
        });
        assert!(bare.contains("the sentence"));
        assert!(!bare.contains("Show the command instead") && !bare.contains("guide-command"));
    }

    #[test]
    fn the_block_says_when_the_reason_changed_and_the_approval_says_when_it_is_confirmed() {
        let screen = |changed| Screen::Block {
            name: "ana".to_owned(),
            sentence: Zeroizing::new("the sentence".to_owned()),
            command: None,
            seconds: 900,
            expires_at: 1_794_659_696_000,
            reconnect: true,
            cancelled: false,
            closed_request: false,
            changed,
        };
        let changed = shown(&screen(Some(
            "the admin asked for a new connection.".to_owned(),
        )));
        assert!(changed.contains(
            "The state changed after you opened this window. The message gives the reason as it is now: the admin asked for a new connection."
        ));
        assert!(!shown(&screen(None)).contains("The state changed after you opened this window."));
        let approved = shown(&Screen::Done {
            name: "ana".to_owned(),
            connected: false,
            reconnect: false,
        });
        assert!(approved.contains("The connector confirms this approval when it runs."));
        let connected = shown(&Screen::Done {
            name: "ana".to_owned(),
            connected: true,
            reconnect: false,
        });
        assert!(!connected.contains("The connector confirms this approval"));
    }
}
