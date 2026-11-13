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

use super::html::Html;
use super::view::{self, Frame, HermesPage, Over};

/// The mascot, drawn by the project frame by frame: no value of anyone goes inside it.
const DANCING: &str = include_str!("assets/mascot-dancing.svg");
const WAVING: &str = include_str!("assets/mascot-waving.svg");

/// The names of the steps, in the path at the top.
const STEPS: [&str; 4] = ["Name", "Block", "Code", "Done"];

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
    /// When, in words.
    pub when: String,
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
    /// Step 2: the block, the only time it appears.
    Block {
        /// The Hermes.
        name: String,
        /// The text of the block. **It holds the invite.**
        text: Zeroizing<String>,
        /// For how many seconds the invite is still accepted.
        seconds: i64,
        /// The Hermes has a device in use.
        reconnect: bool,
        /// An invite issued before was cancelled by this one.
        cancelled: bool,
        /// A request in progress was closed by this one.
        closed_request: bool,
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
    },
    /// Step 3: a device used the invite and the admin types its code.
    Code {
        /// The Hermes.
        name: String,
        /// What is known of the request.
        facts: Box<Facts>,
        /// On a reconnect, the last use of the device in use.
        last_use: Option<Box<LastUse>>,
        /// What the last answer got wrong.
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
            _ => None,
        }
    }
}

/// `MM:SS`, for a time left that the script of the panel then counts down.
fn clock(html: &mut Html, seconds: i64) {
    let seconds = usize::try_from(seconds.max(0)).unwrap_or(0);
    html.raw("<b data-seconds=\"").number(seconds).raw("\">");
    let (minutes, rest) = (seconds / 60, seconds % 60);
    if minutes < 10 {
        html.raw("0");
    }
    html.number(minutes).raw(":");
    if rest < 10 {
        html.raw("0");
    }
    html.number(rest).raw("</b>");
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
    for (index, label) in [
        "Invite issued",
        "A device used the invite",
        "Code checked and request approved",
        "The connector confirmed its first credential",
    ]
    .into_iter()
    .enumerate()
    {
        html.raw(if index < seen {
            "<li class=\"ok\">"
        } else if index == seen {
            "<li class=\"waiting\">"
        } else {
            "<li>"
        })
        .raw(label);
        if index == 1
            && seen > 1
            && let Some(address) = address
        {
            html.raw(", from ").text(address);
        }
        if index == seen {
            html.raw(": waiting…");
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

fn wait_address(html: &mut Html, name: &str) {
    html.raw("/hermes/wait?name=").text(name);
}

fn body(html: &mut Html, screen: &Screen) {
    match screen {
        Screen::Name { typed, error } => {
            html.raw("<h3>What name will this Hermes have in the panel?</h3>")
                .raw("<form id=\"guide-form\" method=\"post\" action=\"/hermes/connect\" autocomplete=\"off\">")
                .raw("<div class=\"field\"><label for=\"guide-name\">Name of the Hermes</label>")
                .raw("<input id=\"guide-name\" name=\"name\" type=\"text\" maxlength=\"32\" spellcheck=\"false\" autocapitalize=\"none\" required autofocus value=\"")
                .text(typed)
                .raw("\"><p class=\"hint\">Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.</p></div></form>");
            if let Some(error) = error {
                html.raw("<p class=\"error\" role=\"alert\">")
                    .raw(error)
                    .raw("</p>");
            }
        }
        Screen::Block {
            name,
            text,
            seconds,
            reconnect,
            cancelled,
            closed_request,
        } => {
            html.raw("<div class=\"center preparation\">");
            mascot(html, true);
            html.raw("<h3 id=\"guide-preparing\" tabindex=\"-1\">Preparing a secure connection</h3><p class=\"hint\">The hub drew the invite of ")
                .text(name)
                .raw(".</p></div><div class=\"after\"><p class=\"deadline\">Invite for <b>")
                .text(name)
                .raw("</b> · valid for another ");
            clock(html, *seconds);
            html.raw(" · single use</p><div class=\"block\" id=\"guide-block\" aria-describedby=\"guide-how\"><pre id=\"guide-text\">")
                .text(text)
                .raw("</pre><span class=\"stamp\" aria-hidden=\"true\"><span>Copied</span></span></div>")
                .raw("<p class=\"hint\" id=\"guide-how\"><span class=\"with-script\">Click the block to copy it. </span><span class=\"no-script\">Select the whole text and copy it. </span>It only appears here, once.</p>")
                .raw("<p class=\"copied-note\" id=\"guide-copied\" role=\"status\" hidden></p>");
            if *cancelled {
                html.raw("<p class=\"hint\">The invite issued before was cancelled.</p>");
            }
            if *closed_request {
                html.raw("<p class=\"hint\">The request that was in progress was closed.</p>");
            }
            if *reconnect {
                html.raw("<p class=\"hint\">The device in use keeps working until the new one confirms its first credential.</p>");
            }
            html.raw("</div>");
        }
        Screen::NoConnector { name } => {
            html.raw("<h3>This hub has no connector to serve</h3><p>The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of ")
                .text(name)
                .raw(" with the <code>arauto-hub admin invite</code> command, on the hub's server.</p>");
        }
        Screen::Waiting { name, .. } => {
            html.raw("<h3>Waiting for the Hermes ")
                .text(name)
                .raw("</h3>");
            observed(html, 1, None);
            html.raw("<p class=\"hint\">Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.</p>");
        }
        Screen::Code {
            name,
            facts,
            last_use,
            error,
            ..
        } => {
            html.raw("<h3>A device used the invite of ")
                .text(name)
                .raw("</h3>");
            observed(html, 2, facts.address.as_deref());
            html.raw("<dl class=\"facts\"><dt>Seen by the hub</dt><dd>address ");
            match &facts.address {
                Some(address) => html.text(address),
                None => html.raw("unknown"),
            };
            html.raw(", ")
                .text(&facts.when)
                .raw("</dd><dt>Said by the device</dt><dd>machine \"")
                .text(&facts.hostname)
                .raw("\" · system \"")
                .text(&facts.os)
                .raw("\" · Hermes \"")
                .text(&facts.hermes)
                .raw("\" (not verified)</dd></dl>")
                .raw("<form id=\"guide-deny\" method=\"post\" action=\"/hermes/deny\" hidden>");
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
                    .raw(
                        "\"><fieldset class=\"recognize\"><legend>Last use of the current device: ",
                    )
                    .text(&last.when);
                if let Some(address) = &last.address {
                    html.raw(", from ").text(address);
                }
                html.raw(". Do you recognize it?</legend>")
                    .raw("<label class=\"option\"><input type=\"radio\" name=\"use\" value=\"recognized\"><span>I recognize it</span></label>")
                    .raw("<label class=\"option\"><input type=\"radio\" name=\"use\" value=\"not-recognized\"><span>I do not recognize it</span></label>")
                    .raw("<p class=\"hint\">If the Hermes was off and there was use, someone used a copy.</p></fieldset>");
            }
            html.raw("<div class=\"field\"><label for=\"guide-code\">Verification code</label>")
                .raw("<input class=\"code\" id=\"guide-code\" name=\"code\" type=\"text\" inputmode=\"numeric\" maxlength=\"7\" placeholder=\"000-000\" spellcheck=\"false\" required autofocus>")
                .raw("<p class=\"hint\">Type the code the Hermes answered in the command channel. The panel never shows this code.</p></div></form>");
            if let Some(error) = error {
                html.raw("<p class=\"error\" role=\"alert\">")
                    .text(error)
                    .raw("</p>");
            }
        }
        Screen::Ended { name, why } => {
            html.raw("<h3>");
            match why {
                Ended::Denied => html
                    .raw("The request of ")
                    .text(name)
                    .raw(" was denied</h3><p>The code was typed wrong for the fifth time. To connect, issue another invite.</p>"),
                Ended::Expired => html
                    .raw("The request of ")
                    .text(name)
                    .raw(" expired</h3><p>The 10 minutes to type the code passed. To connect, issue another invite.</p>"),
                Ended::Copied => html
                    .raw("The identity of ")
                    .text(name)
                    .raw(" was copied</h3><p>Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.</p>"),
                Ended::Nothing => html
                    .text(name)
                    .raw(" has no invite and no request waiting</h3><p>The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.</p>"),
            };
        }
        Screen::Done {
            name,
            connected,
            reconnect,
        } => {
            html.raw("<div class=\"center\">");
            mascot(html, false);
            html.raw("<h3>").text(name).raw(if *connected {
                " is connected</h3>"
            } else {
                " was approved</h3>"
            });
            observed(html, if *connected { 4 } else { 3 }, None);
            html.raw(match (*connected, *reconnect) {
                (true, _) => "<p class=\"hint\">What it may do on each number is in the Accesses tab.</p>",
                (false, true) => "<p class=\"hint\">As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.</p>",
                (false, false) => "<p class=\"hint\">As soon as the connector confirms, it shows as connected. It has no access to any number yet.</p>",
            })
            .raw("</div>");
        }
        Screen::Failed { sentence } => {
            html.raw("<h3>The hub did not do what was asked</h3><p>")
                .raw(sentence)
                .raw("</p>");
        }
    }
}

fn footer(html: &mut Html, screen: &Screen) {
    html.raw("<div class=\"modal-foot\">");
    match screen {
        Screen::Name { .. } => {
            html.raw("<button class=\"button primary\" type=\"submit\" form=\"guide-form\">Issue invite</button><a class=\"button\" href=\"/\">Cancel</a>");
        }
        Screen::Block { name, .. } => {
            html.raw("<a class=\"button primary after-copy appear\" id=\"guide-next\" href=\"");
            wait_address(html, name);
            html.raw(
                "\">Next step</a><span>After copying, paste it into the command channel of the Hermes ",
            )
            .text(name)
            .raw(". Nowhere else.</span>");
        }
        Screen::Waiting { seconds, .. } => {
            html.raw("<span>The invite is valid for another ");
            clock(html, *seconds);
            html.raw(". This screen refreshes by itself.</span>");
        }
        Screen::Code { seconds, .. } => {
            html.raw("<button class=\"button primary\" type=\"submit\" form=\"guide-form\">Approve</button><button class=\"button\" type=\"submit\" form=\"guide-deny\">Deny</button><span>Confirm within ");
            clock(html, *seconds);
            html.raw(".</span>");
        }
        Screen::Ended { name, .. } => {
            html.raw("<form method=\"post\" action=\"/hermes/invite\">");
            named(html, name);
            html.raw("<button class=\"button primary\" type=\"submit\">Issue another invite</button></form><a class=\"button\" href=\"/\">Close</a>");
        }
        Screen::Done { .. } => {
            html.raw("<a class=\"button primary\" href=\"/accesses\">See accesses</a><a class=\"button\" href=\"/\">Finish</a>");
        }
        Screen::NoConnector { .. } | Screen::Failed { .. } => {
            html.raw("<a class=\"button primary\" href=\"/\">Close</a>");
        }
    }
    html.raw("</div>");
}

/// The page of the Hermes with the guide over it.
#[must_use]
pub fn page(frame: &Frame, hermes: &HermesPage, screen: &Screen) -> String {
    let mut html = Html::new();
    view::hermes_into(
        &mut html,
        frame,
        hermes,
        Over::Guide {
            refresh: screen.refresh(),
        },
    );
    html.raw(if matches!(screen, Screen::Block { .. }) {
        "<dialog open class=\"modal guide preparing\" id=\"guide\" aria-labelledby=\"guide-title\">"
    } else {
        "<dialog open class=\"modal guide\" id=\"guide\" aria-labelledby=\"guide-title\">"
    })
    .raw("<div class=\"modal-head\"><h2 id=\"guide-title\">");
    match screen.subject() {
        Some((name, true)) => html.raw("Reconnect ").text(name),
        Some((name, false)) => html.raw("Connect ").text(name),
        None => html.raw("Connect Hermes"),
    };
    html.raw("</h2><a class=\"close\" href=\"/\" aria-label=\"Close\"><svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg></a></div>")
        .raw("<ol class=\"path\" aria-label=\"Steps\">");
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
        .raw(label)
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
            when: hostile(),
            address: Some(hostile()),
        };
        for screen in [
            Screen::Name {
                typed: hostile(),
                error: Some("Use only lowercase letters."),
            },
            Screen::Block {
                name: hostile(),
                text: Zeroizing::new(hostile()),
                seconds: 900,
                reconnect: true,
                cancelled: true,
                closed_request: true,
            },
            Screen::NoConnector { name: hostile() },
            Screen::Waiting {
                name: hostile(),
                seconds: 5,
                reconnect: false,
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
    fn the_path_marks_the_step_and_only_a_page_that_waits_asks_again() {
        let waiting = shown(&Screen::Waiting {
            name: "ana".to_owned(),
            seconds: 61,
            reconnect: false,
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
        assert!(name.contains("<h2 id=\"guide-title\">Connect Hermes</h2>"));
        assert!(!name.contains("class=\"done\""));
        let done = shown(&Screen::Done {
            name: "ana".to_owned(),
            connected: true,
            reconnect: false,
        });
        assert_eq!(done.matches("<li class=\"done\">").count(), 3);
        assert_eq!(done.matches("<li class=\"ok\">").count(), 4);
    }

    #[test]
    fn the_block_waits_for_the_mascot_and_the_way_on_waits_for_the_copy() {
        let block = shown(&Screen::Block {
            name: "ana".to_owned(),
            text: Zeroizing::new("the block".to_owned()),
            seconds: 900,
            reconnect: false,
            cancelled: false,
            closed_request: false,
        });
        assert!(block.contains("class=\"modal guide preparing\""));
        assert!(block.contains("<pre id=\"guide-text\">the block</pre>"));
        assert!(block.contains("class=\"button primary after-copy appear\" id=\"guide-next\" href=\"/hermes/wait?name=ana\""));
        assert!(block.contains("<h2 id=\"guide-title\">Connect ana</h2>"));
        assert!(!block.contains("was cancelled") && !block.contains("keeps working"));
        assert!(
            !block.contains("http-equiv"),
            "the block is never asked for again"
        );
    }
}
