//! The pages of the panel: what each one shows, as HTML.
//!
//! A page is written from plain values that [`super::data`] reads: nothing here touches the
//! database. Every value goes through [`Html::text`]; the markup is the `raw` part; and
//! every sentence of the screen goes through [`Html::t`] or [`Html::fill`], which write it in
//! the language of the page ([`super::text`]).

use super::html::{Arg, Html};
use super::text::{Lang, k, tr};

/// The logo, written by the project: no value of anyone goes inside it.
const LOGO: &str = include_str!("assets/logo.svg");

pub(super) const TAIL: &str = "</body></html>";

/// What stands over a page of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// Nothing: the page is the whole screen.
    Nothing,
    /// A guide, in a window: the page under it cannot be reached while it is open.
    Guide {
        /// Every how many seconds the page asks the hub again, when it waits for something.
        refresh: Option<u32>,
    },
}

/// The head of a page. Only a page with a guide loads the script of the panel, and it loads
/// it by its address: no page carries a script of its own.
fn head(html: &mut Html, over: Over) {
    let lang = html.lang();
    html.raw("<!doctype html><html lang=\"")
        .raw(lang.tag())
        .raw("\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Arauto[IA] · ")
        .t(k("Panel"))
        .raw("</title><link rel=\"stylesheet\" href=\"/assets/panel.css\">");
    if let Over::Guide { refresh } = over {
        if let Some(seconds) = refresh {
            html.raw("<meta http-equiv=\"refresh\" content=\"")
                .number(usize::try_from(seconds).unwrap_or(usize::MAX))
                .raw("\">");
        }
        html.raw("<script src=\"/assets/panel.js\"></script>");
    }
    html.raw("</head><body>");
}

/// The form that changes the language of the panel: every language by its own name.
fn language(html: &mut Html) {
    let current = html.lang();
    html.raw("<form class=\"language\" method=\"post\" action=\"/language\"><label for=\"lang\">")
        .t(k("Language"))
        .raw("</label><select id=\"lang\" name=\"lang\">");
    for lang in Lang::ALL {
        html.raw("<option value=\"")
            .raw(lang.tag())
            .raw(if lang == current {
                "\" selected>"
            } else {
                "\">"
            })
            .raw(lang.name())
            .raw("</option>");
    }
    html.raw("</select><button class=\"button\" type=\"submit\">")
        .t(k("Change"))
        .raw("</button></form>");
}

/// How a state reads at a glance: the color of the dot, never the only sign of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// It works.
    Ok,
    /// It waits for someone.
    Wait,
    /// It stopped and needs the admin.
    Bad,
    /// It is not in use.
    Idle,
}

impl Tone {
    const fn state_class(self) -> &'static str {
        match self {
            Tone::Ok => "state",
            Tone::Wait => "state wait",
            Tone::Bad => "state bad",
            Tone::Idle => "state idle",
        }
    }
}

/// One alert of the band at the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// Whether it is of the kind that stops a Hermes or a number.
    pub grave: bool,
    /// What it is about, in a few fixed words: a sentence of the screens.
    pub lead: &'static str,
    /// The sentence, already in the language of the page. It may carry a name or an address
    /// that came from outside.
    pub text: String,
}

/// Which page is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The Hermes.
    Hermes,
    /// The numbers.
    Numbers,
    /// The matrix of accesses.
    Accesses,
}

/// What every page of a session shows around its own content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frame {
    /// The language of the page.
    pub lang: Lang,
    /// The sentence of the restored mode, while it is on (C11).
    pub restored: Option<String>,
    /// The alerts nobody marked as seen, the newest first.
    pub notices: Vec<Notice>,
    /// How many more there are than the ones shown.
    pub more_notices: usize,
    /// How many devices the hub does not know presented a valid credential.
    pub unknown_devices: usize,
}

/// One line of the list of Hermes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesRow {
    /// The name.
    pub name: String,
    /// The color of its state.
    pub tone: Tone,
    /// Its state: a sentence of the screens.
    pub state: &'static str,
    /// The aliases of the numbers it reaches.
    pub numbers: Vec<String>,
    /// Whether it can be woken.
    pub woken: bool,
    /// The version its connector declared. Not verified.
    pub connector: Option<String>,
    /// When the hub last saw it, in words.
    pub seen: String,
    /// What the admin needs to know about a Hermes that stopped.
    pub explain: Option<String>,
    /// What the button of its line does.
    pub action: RowAction,
}

/// What the admin can do from the line of a Hermes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    /// It has no device and no invite: issue its first invite.
    Connect,
    /// An invite or a request is in progress: go back to the guide.
    Continue,
    /// It has a device: issue an invite for another one.
    Reconnect,
}

impl RowAction {
    /// Whether the Hermes has something a revocation would close: a device, a request or an
    /// invite.
    const fn has_something(self) -> bool {
        !matches!(self, RowAction::Connect)
    }
}

/// The page of the Hermes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HermesPage {
    /// The lines, by name.
    pub rows: Vec<HermesRow>,
    /// How many are in a session of the channel now.
    pub connected: usize,
    /// How many wait for the Hermes, for the code or for the connector.
    pub waiting: usize,
    /// How many are in quarantine or suspended.
    pub stopped: usize,
}

/// One line of the list of numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberRow {
    /// The alias.
    pub alias: String,
    /// The telephone, when the key of the organization opened it.
    pub phone: Option<String>,
    /// The color of its state.
    pub tone: Tone,
    /// Its state: a sentence of the screens.
    pub state: &'static str,
    /// Whether the admin marked it as a command channel.
    pub command: bool,
    /// The Hermes with an access to it.
    pub hermes: Vec<String>,
    /// When its last message came, in words. Never what it said.
    pub last_message: String,
}

/// The engine, as the page of the numbers says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineLine {
    /// The color of the line.
    pub tone: Tone,
    /// The sentence. The version in it came from the engine.
    pub text: String,
}

/// The page of the numbers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NumbersPage {
    /// The lines, by alias.
    pub rows: Vec<NumberRow>,
    /// The engine, when there is one.
    pub engine: Option<EngineLine>,
}

/// What a Hermes may do on a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Read, send, modify, be woken: in this order.
    pub verbs: [bool; 4],
    /// Which chats, in words.
    pub scope: String,
}

/// One line of the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRow {
    /// The Hermes.
    pub hermes: String,
    /// The state of a Hermes whose device does not serve, as a sentence of the screens: its
    /// accesses are kept and none applies.
    pub stopped: Option<&'static str>,
    /// One cell per number, in the order of the columns. `None`: no access.
    pub cells: Vec<Option<Cell>>,
}

/// The page of the accesses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccessPage {
    /// The columns: the alias and the telephone of each number.
    pub numbers: Vec<(String, Option<String>)>,
    /// The lines, by name of Hermes.
    pub rows: Vec<AccessRow>,
    /// What the admin is told and nothing prevents.
    pub warnings: Vec<String>,
    /// The access that was just saved, in one sentence.
    pub saved: Option<String>,
}

/// The letters of the four verbs in the matrix: the initials of read, send, modify and be
/// woken, in each language.
pub(super) const VERB_LETTERS: [&str; 4] = [k("R"), k("S"), k("M"), k("W")];

/// The page where the entry code is typed. `refusal` is a sentence of the screens.
#[must_use]
pub fn sign_in(lang: Lang, refusal: Option<&'static str>) -> String {
    let mut html = Html::new(lang);
    head(&mut html, Over::Nothing);
    html.raw("<main class=\"entry\">")
        .raw(LOGO)
        .raw("<h1>")
        .t(k("Sign in to the panel"))
        .raw("</h1><form class=\"card\" method=\"post\" action=\"/sign-in\" autocomplete=\"off\"><div class=\"field\"><label for=\"code\">")
        .t(k("Entry code"))
        .raw("</label><input id=\"code\" name=\"code\" type=\"password\" inputmode=\"text\" autocapitalize=\"none\" spellcheck=\"false\" maxlength=\"64\" required autofocus></div>");
    if let Some(refusal) = refusal {
        html.raw("<p class=\"error\" role=\"alert\">")
            .t(refusal)
            .raw("</p>");
    }
    html.raw("<p class=\"hint\">")
        .fill(
            k("Issue the code on the hub's server, with {command}. It is valid for 10 minutes and for one sign-in."),
            &[("command", Arg::Code("arauto-hub admin panel code"))],
        )
        .raw("</p><div><button class=\"button primary\" type=\"submit\">")
        .t(k("Sign in"))
        .raw("</button></div></form>");
    language(&mut html);
    html.raw("</main>").raw(TAIL);
    html.finish()
}

fn open(html: &mut Html, frame: &Frame, tab: Tab, over: Over) {
    head(html, over);
    html.raw(if over == Over::Nothing {
        "<div class=\"page\"><header class=\"top\">"
    } else {
        "<div class=\"page\" inert><header class=\"top\">"
    })
    .raw(LOGO)
    .raw("<div class=\"who\"><span>admin</span><form method=\"post\" action=\"/sign-out\"><button class=\"sign-out\" type=\"submit\">")
    .t(k("Sign out"))
    .raw("</button></form></div></header>");
    let alerts = frame.restored.is_some() || !frame.notices.is_empty() || frame.unknown_devices > 0;
    if alerts {
        html.raw("<ul class=\"alerts\" aria-label=\"")
            .t(k("Alerts"))
            .raw("\">");
        if let Some(restored) = &frame.restored {
            html.raw("<li class=\"alert restored\">")
                .text(restored)
                .raw("</li>");
        }
        for notice in &frame.notices {
            html.raw(if notice.grave {
                "<li class=\"alert bad\"><span class=\"state bad\">"
            } else {
                "<li class=\"alert\"><span class=\"state wait\">"
            })
            .t(notice.lead)
            .raw(":</span> ")
            .text(&notice.text)
            .raw("</li>");
        }
        if frame.unknown_devices > 0 {
            html.raw("<li class=\"alert bad\"><span class=\"state bad\">")
                .t(k("Unknown devices"))
                .raw(":</span> ")
                .fill(
                    k("{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}."),
                    &[
                        ("n", Arg::Number(frame.unknown_devices)),
                        ("command", Arg::Code("arauto-hub admin alerts")),
                    ],
                )
                .raw("</li>");
        }
        if frame.more_notices > 0 {
            html.raw("<li class=\"alert\"><span class=\"more\">")
                .fill(
                    k("More alerts: {n}. See them all with {command}."),
                    &[
                        ("n", Arg::Number(frame.more_notices)),
                        ("command", Arg::Code("arauto-hub admin alerts")),
                    ],
                )
                .raw("</span></li>");
        }
        html.raw("</ul>");
    }
    html.raw("<nav class=\"tabs\" aria-label=\"")
        .t(k("Panel"))
        .raw("\">");
    // Hermes and WhatsApp are names: they are the same in every language.
    for (this, address, label) in [
        (Tab::Hermes, "<a href=\"/\"", None),
        (Tab::Numbers, "<a href=\"/whatsapps\"", None),
        (Tab::Accesses, "<a href=\"/accesses\"", Some(k("Accesses"))),
    ] {
        html.raw(address).raw(if this == tab {
            " aria-current=\"page\">"
        } else {
            ">"
        });
        match (label, this) {
            (Some(label), _) => html.t(label),
            (None, Tab::Numbers) => html.raw("WhatsApps"),
            (None, _) => html.raw("Hermes"),
        };
        html.raw("</a>");
    }
    html.raw("</nav><main>");
}

/// Ends the page, up to where a window over it would go.
fn close(html: &mut Html) {
    html.raw("</main><p class=\"footer\">")
        .fill(
            k("In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server."),
            &[("command", Arg::Code("arauto-hub admin"))],
        )
        .raw("</p>");
    language(html);
    html.raw("</div>");
}

fn state(html: &mut Html, tone: Tone, label: &'static str) {
    html.raw("<span class=\"")
        .raw(tone.state_class())
        .raw("\">")
        .t(label)
        .raw("</span>");
}

fn names(html: &mut Html, list: &[String]) {
    if list.is_empty() {
        html.t(k("none"));
        return;
    }
    for (index, name) in list.iter().enumerate() {
        if index > 0 {
            html.raw(", ");
        }
        html.text(name);
    }
}

/// The page of the Hermes: the same list `arauto-hub admin hermes list` gives, and whether
/// each one is connected now, which only the running hub knows.
#[must_use]
pub fn hermes(frame: &Frame, page: &HermesPage) -> String {
    let mut html = Html::new(frame.lang);
    hermes_into(&mut html, frame, page, Over::Nothing);
    html.raw(TAIL);
    html.finish()
}

/// Writes the page of the Hermes up to where a window over it would go.
pub(super) fn hermes_into(html: &mut Html, frame: &Frame, page: &HermesPage, over: Over) {
    open(html, frame, Tab::Hermes, over);
    html.raw("<div class=\"heading\"><div><h1>")
        .bracketed(k("Your Hermes and their [numbers]"))
        .raw("</h1><p class=\"counters\">")
        .fill(
            k("Connected: {connected} · Waiting: {waiting} · On alert: {stopped}"),
            &[
                ("connected", Arg::Number(page.connected)),
                ("waiting", Arg::Number(page.waiting)),
                ("stopped", Arg::Number(page.stopped)),
            ],
        )
        .raw("</p></div><a class=\"button primary\" href=\"/hermes/connect\">")
        .t(k("Connect Hermes"))
        .raw("</a></div><div class=\"frame\"><table class=\"list\"><thead><tr><th scope=\"col\">Hermes</th><th scope=\"col\">")
        .t(k("State"))
        .raw("</th><th scope=\"col\">")
        .t(k("Numbers"))
        .raw("</th><th scope=\"col\">")
        .t(k("Can be woken"))
        .raw("</th><th scope=\"col\">")
        .t(k("Connector"))
        .raw("</th><th scope=\"col\">")
        .t(k("Seen"))
        .raw("</th><th scope=\"col\"><span class=\"sr-only\">")
        .t(k("Actions"))
        .raw("</span></th></tr></thead><tbody>");
    if page.rows.is_empty() {
        html.raw("<tr class=\"empty\"><td colspan=\"7\">")
            .t(k(
                "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
            ))
            .raw("</td></tr>");
    }
    for row in &page.rows {
        html.raw("<tr><td><span class=\"name\">")
            .text(&row.name)
            .raw("</span></td><td>");
        state(html, row.tone, row.state);
        html.raw("</td><td class=\"second-line\">");
        names(html, &row.numbers);
        html.raw("</td><td class=\"wide-only\">")
            .t(if row.woken { k("yes") } else { k("no") })
            .raw("</td><td class=\"wide-only mono\">");
        match &row.connector {
            Some(version) => html.text(version),
            None => html.raw("—"),
        };
        html.raw("</td><td class=\"wide-only num\">")
            .text(&row.seen)
            .raw("</td><td class=\"actions-cell\"><div class=\"actions\">");
        match row.action {
            RowAction::Continue => {
                html.raw("<a class=\"button\" href=\"/hermes/wait?name=")
                    .text(&row.name)
                    .raw("\">")
                    .t(k("Continue"))
                    .raw("</a>");
            }
            RowAction::Connect | RowAction::Reconnect => {
                html.raw("<form method=\"post\" action=\"/hermes/invite\"><input type=\"hidden\" name=\"name\" value=\"")
                    .text(&row.name)
                    .raw("\"><button class=\"button\" type=\"submit\">")
                    .t(if row.action == RowAction::Connect {
                        k("Connect")
                    } else {
                        k("Reconnect")
                    })
                    .raw("</button></form>");
            }
        }
        if row.action.has_something() {
            html.raw("<a class=\"button danger\" href=\"/hermes/revoke?name=")
                .text(&row.name)
                .raw("\">")
                .t(k("Revoke"))
                .raw("</a>");
        }
        html.raw("</div></td></tr>");
        if let Some(explain) = &row.explain {
            html.raw("<tr class=\"explain\"><td colspan=\"7\">")
                .text(explain)
                .raw("</td></tr>");
        }
    }
    html.raw("</tbody></table></div><p class=\"note\">")
        .t(k("\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified."))
        .raw("</p>");
    close(html);
}

/// The page of the numbers.
#[must_use]
pub fn numbers(frame: &Frame, page: &NumbersPage) -> String {
    let mut html = Html::new(frame.lang);
    open(&mut html, frame, Tab::Numbers, Over::Nothing);
    html.raw("<div class=\"heading\"><h1>WhatsApps</h1><p class=\"counters\">");
    match &page.engine {
        Some(engine) => {
            html.raw("<span class=\"")
                .raw(engine.tone.state_class())
                .raw("\">")
                .t(k("Engine"))
                .raw("</span> ")
                .text(&engine.text);
        }
        None => {
            html.fill(
                k("There is no engine yet. Set it with {command}."),
                &[("command", Arg::Code("arauto-hub admin engine set"))],
            );
        }
    }
    html.raw("</p></div><div class=\"frame\"><table class=\"list\"><thead><tr><th scope=\"col\">")
        .t(k("Alias"))
        .raw("</th><th scope=\"col\">")
        .t(k("Number"))
        .raw("</th><th scope=\"col\">")
        .t(k("State"))
        .raw("</th><th scope=\"col\">")
        .t(k("Hermes with access"))
        .raw("</th><th scope=\"col\">")
        .t(k("Last message"))
        .raw("</th></tr></thead><tbody>");
    if page.rows.is_empty() {
        html.raw("<tr class=\"empty\"><td colspan=\"5\">")
            .fill(
                k("No number yet. Add one with {command}."),
                &[(
                    "command",
                    Arg::Code("arauto-hub admin number add ALIAS PHONE"),
                )],
            )
            .raw("</td></tr>");
    }
    for row in &page.rows {
        html.raw("<tr><td><span class=\"name\">")
            .text(&row.alias)
            .raw("</span>");
        if row.command {
            html.raw("<span class=\"tag\">")
                .t(k("command"))
                .raw("</span>");
        }
        html.raw("</td><td class=\"wide-only mono\">");
        match &row.phone {
            Some(phone) => html.text(phone),
            None => html.raw("—"),
        };
        html.raw("</td><td>");
        state(&mut html, row.tone, row.state);
        html.raw("</td><td class=\"second-line\">");
        names(&mut html, &row.hermes);
        html.raw("</td><td class=\"wide-only num\">")
            .text(&row.last_message)
            .raw("</td></tr>");
    }
    html.raw("</tbody></table></div><p class=\"note\">")
        .t(k("The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone."))
        .raw("</p>");
    close(&mut html);
    html.raw(TAIL);
    html.finish()
}

/// The page of the accesses: the matrix. Each cell leads to the editor of that access.
#[must_use]
pub fn accesses(frame: &Frame, page: &AccessPage) -> String {
    let mut html = Html::new(frame.lang);
    accesses_into(&mut html, frame, page, Over::Nothing);
    html.raw(TAIL);
    html.finish()
}

/// Writes the page of the accesses up to where a window over it would go.
pub(super) fn accesses_into(html: &mut Html, frame: &Frame, page: &AccessPage, over: Over) {
    let lang = html.lang();
    open(html, frame, Tab::Accesses, over);
    html.raw("<div class=\"heading\"><div><h1>")
        .t(k("Accesses"))
        .raw("</h1><p class=\"counters\">")
        .t(k(
            "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        ))
        .raw("</p></div></div>");
    if page.rows.is_empty() || page.numbers.is_empty() {
        html.raw("<div class=\"frame\"><table class=\"list\"><tbody><tr class=\"empty\"><td>")
            .t(if page.rows.is_empty() {
                k("Connect a Hermes first.")
            } else {
                k("Add a WhatsApp first.")
            })
            .raw("</td></tr></tbody></table></div>");
        close(html);
        return;
    }
    html.raw("<div class=\"frame\"><div class=\"scroll\"><table class=\"matrix\"><thead><tr><th scope=\"col\">Hermes</th>");
    for (alias, phone) in &page.numbers {
        html.raw("<th scope=\"col\">").text(alias);
        if let Some(phone) = phone {
            html.raw("<span class=\"phone\">")
                .text(phone)
                .raw("</span>");
        }
        html.raw("</th>");
    }
    html.raw("</tr></thead><tbody>");
    for row in &page.rows {
        html.raw("<tr><th scope=\"row\">")
            .text(&row.hermes)
            .raw("</th>");
        if let Some(stopped) = row.stopped {
            html.raw("<td colspan=\"")
                .number(page.numbers.len())
                .raw("\"><span class=\"state bad\">")
                .t(stopped)
                .raw("</span><span class=\"kept\"> ")
                .t(k("(accesses kept)"))
                .raw("</span></td></tr>");
            continue;
        }
        for (cell, (alias, _)) in row.cells.iter().zip(&page.numbers) {
            html.raw("<td><a class=\"cell\" href=\"/accesses/edit?hermes=")
                .text(&row.hermes)
                .raw("&amp;number=")
                .text(alias)
                .raw("\"><span class=\"verbs\">");
            let verbs = cell.as_ref().map_or([false; 4], |cell| cell.verbs);
            for (granted, letter) in verbs.iter().zip(VERB_LETTERS) {
                html.raw(if *granted {
                    "<span>"
                } else {
                    "<span class=\"off\">"
                })
                .t(letter)
                .raw("</span>");
            }
            html.raw("</span><span class=\"scope\">");
            match cell {
                Some(cell) => html.text(&cell.scope),
                None => html.t(k("no access")),
            };
            html.raw("</span></a></td>");
        }
        html.raw("</tr>");
    }
    html.raw("</tbody></table></div></div><p class=\"note\">")
        .fill(
            k("{r} read · {s} send · {m} modify · {w} be woken."),
            &[
                ("r", Arg::Strong(tr(lang, VERB_LETTERS[0]))),
                ("s", Arg::Strong(tr(lang, VERB_LETTERS[1]))),
                ("m", Arg::Strong(tr(lang, VERB_LETTERS[2]))),
                ("w", Arg::Strong(tr(lang, VERB_LETTERS[3]))),
            ],
        )
        .raw("</p>");
    if let Some(saved) = &page.saved {
        html.raw("<p class=\"status\" role=\"status\">")
            .fill(k("Saved: {what}"), &[("what", Arg::Text(saved))])
            .raw("</p>");
    }
    if !page.warnings.is_empty() {
        html.raw("<div class=\"warnings\"><b>")
            .t(k("Warnings that block nothing"))
            .raw("</b><ul>");
        for warning in &page.warnings {
            html.raw("<li>").text(warning).raw("</li>");
        }
        html.raw("</ul></div>");
    }
    close(html);
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    /// What a page may contain besides what the program wrote: nothing that opens markup.
    fn clean(page: &str) {
        assert!(!page.contains("<script"), "{page}");
        assert!(!page.contains("<img"), "{page}");
        // The quotes of the value did not close an attribute nor an element.
        assert!(!page.contains("\"'>"), "{page}");
        assert!(
            page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{page}"
        );
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.ends_with("</html>"));
    }

    fn hostile_frame() -> Frame {
        Frame {
            lang: Lang::En,
            restored: Some(HOSTILE.to_owned()),
            notices: vec![Notice {
                grave: true,
                lead: k("Connection taken"),
                text: HOSTILE.to_owned(),
            }],
            more_notices: 2,
            unknown_devices: 1,
        }
    }

    #[test]
    fn what_a_device_or_the_engine_wrote_is_shown_as_text_on_every_page() {
        let frame = hostile_frame();
        let hermes_page = HermesPage {
            rows: vec![HermesRow {
                name: HOSTILE.to_owned(),
                tone: Tone::Bad,
                state: k("In quarantine"),
                numbers: vec![HOSTILE.to_owned(), HOSTILE.to_owned()],
                woken: true,
                connector: Some(HOSTILE.to_owned()),
                seen: HOSTILE.to_owned(),
                explain: Some(HOSTILE.to_owned()),
                action: RowAction::Reconnect,
            }],
            connected: 0,
            waiting: 0,
            stopped: 1,
        };
        let page = hermes(&frame, &hermes_page);
        clean(&page);
        // The values of the row (its name three times: in the list, in the form of its button
        // and in the address of its revocation), and the ones of the frame: all escaped.
        assert_eq!(page.matches("&lt;script&gt;").count(), 10);

        let numbers_page = NumbersPage {
            rows: vec![NumberRow {
                alias: HOSTILE.to_owned(),
                phone: Some(HOSTILE.to_owned()),
                tone: Tone::Ok,
                state: k("Connected"),
                command: true,
                hermes: vec![HOSTILE.to_owned()],
                last_message: HOSTILE.to_owned(),
            }],
            engine: Some(EngineLine {
                tone: Tone::Wait,
                text: HOSTILE.to_owned(),
            }),
        };
        clean(&numbers(&frame, &numbers_page));

        let access_page = AccessPage {
            numbers: vec![(HOSTILE.to_owned(), Some(HOSTILE.to_owned()))],
            rows: vec![AccessRow {
                hermes: HOSTILE.to_owned(),
                stopped: None,
                cells: vec![Some(Cell {
                    verbs: [true, false, false, true],
                    scope: HOSTILE.to_owned(),
                })],
            }],
            warnings: vec![HOSTILE.to_owned()],
            saved: Some(HOSTILE.to_owned()),
        };
        clean(&accesses(&frame, &access_page));
    }

    #[test]
    fn the_pages_say_when_there_is_nothing_and_carry_no_script_of_their_own() {
        let empty = Frame::default();
        for page in [
            hermes(&empty, &HermesPage::default()),
            numbers(&empty, &NumbersPage::default()),
            accesses(&empty, &AccessPage::default()),
            sign_in(Lang::En, None),
            sign_in(
                Lang::En,
                Some(k(
                    "Invalid or expired code. Issue another one on the hub's server.",
                )),
            ),
        ] {
            assert!(
                !page.contains("<script"),
                "a page without a guide loads no script"
            );
            assert!(
                !page.contains(" style=\""),
                "the policy of the page blocks inline styles"
            );
            assert!(!page.contains("javascript:"));
            assert!(page.contains("/assets/panel.css"));
            assert!(page.starts_with("<!doctype html><html lang=\"en\">"));
            // Every page lets the person change the language, with each one by its own name.
            assert!(
                page.contains("<form class=\"language\" method=\"post\" action=\"/language\">")
            );
            assert!(page.contains("<option value=\"en\" selected>English</option>"));
            assert!(page.contains("<option value=\"pt-BR\">Português</option>"));
            assert!(page.contains("<option value=\"ja\">日本語</option>"));
        }
        assert!(hermes(&empty, &HermesPage::default()).contains("No Hermes yet"));
        assert!(numbers(&empty, &NumbersPage::default()).contains("No number yet"));
        assert!(accesses(&empty, &AccessPage::default()).contains("Connect a Hermes first"));
        assert!(
            hermes(&empty, &HermesPage::default())
                .contains("Connected: 0 · Waiting: 0 · On alert: 0")
        );
        // The band of alerts only exists when there is something to say.
        assert!(!hermes(&empty, &HermesPage::default()).contains("class=\"alerts\""));
        assert!(hermes(&hostile_frame(), &HermesPage::default()).contains("class=\"alerts\""));
        // The word between brackets in the title is the one the brand colors.
        assert!(
            hermes(&empty, &HermesPage::default())
                .contains("<h1>Your Hermes and their <b>[numbers]</b></h1>")
        );
    }

    #[test]
    fn the_matrix_shows_each_verb_and_keeps_a_stopped_hermes_apart() {
        let page = AccessPage {
            numbers: vec![("balcao".to_owned(), None), ("compras".to_owned(), None)],
            rows: vec![
                AccessRow {
                    hermes: "ana".to_owned(),
                    stopped: None,
                    cells: vec![
                        Some(Cell {
                            verbs: [true, true, false, true],
                            scope: "all".to_owned(),
                        }),
                        None,
                    ],
                },
                AccessRow {
                    hermes: "antigo".to_owned(),
                    stopped: Some(k("In quarantine")),
                    cells: Vec::new(),
                },
            ],
            warnings: Vec::new(),
            saved: None,
        };
        let html = accesses(&Frame::default(), &page);
        // Each cell leads to the editor of its Hermes and its number.
        assert!(
            html.contains(
                "<a class=\"cell\" href=\"/accesses/edit?hermes=ana&amp;number=balcao\">"
            )
        );
        assert!(html.contains("href=\"/accesses/edit?hermes=ana&amp;number=compras\""));
        assert!(
            !html.contains("hermes=antigo"),
            "a stopped Hermes has no cell to click"
        );
        assert!(!html.contains("class=\"status\""));
        assert!(
            html.contains("<span>R</span><span>S</span><span class=\"off\">M</span><span>W</span>")
        );
        assert!(html.contains("no access"));
        assert!(html.contains("<td colspan=\"2\"><span class=\"state bad\">In quarantine</span>"));
        assert!(
            html.contains("<b>R</b> read · <b>S</b> send · <b>M</b> modify · <b>W</b> be woken.")
        );
        assert!(!html.contains("class=\"warnings\""));
    }
}
