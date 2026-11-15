//! The org chart of the Hermes in the panel (`docs/decisoes/0043-organograma-e-niveis.md`,
//! stage A): levels, the place and the role of each Hermes, and the links between them, edited
//! on a map as a draft and applied by Publish in one write.
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /org` | the page: the stored state for the script, and the same data as plain tables |
//! | `POST /org/publish` | the whole wanted state, in one write ([`crate::repo::org::publish`]) |
//!
//! **Proposal, not run in a real browser.** The page is written by the hub; the map is drawn by
//! the script `org.js` from the state the page carries. What the map shows is for the admin:
//! no Hermes is told about levels, roles or the kind of an arrow, and a note is still data,
//! never an order. A read of this page writes nothing; a publish is a form of the panel with
//! the same session and `Origin` rules as every other.

use axum::body::Body;
use axum::http::{HeaderMap, Response, StatusCode};
use hyper::body::Incoming;
use uuid::Uuid;

use super::html::{Arg, Html};
use super::text::{self, Lang, k};
use super::view::{self, Frame, Over, Tab, Tone};
use super::{PanelState, data, http, session_of};
use crate::admin::{self, AdminError};
use crate::audit::AuditError;
use crate::channel::Hub;
use crate::core::access::Scope;
use crate::core::org::{
    Chart, ChartVersion, Draft, Kind, LEVELS_MAX, MAP_X_MAX, OrgInvalid, PUBLISH_FORM_MAX,
    ROLE_MAX_CHARS, kind, reserved_words,
};
use crate::db::ResolvedOrg;
use crate::repo::org::PublishOutcome;
use crate::repo::{self, RepoError};

const PUBLISHED: &str = k("Published.");
const STALE: &str = k(
    "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
);
const NOTHING: &str = k("Nothing was published.");
const FAILED: &str = k("Something failed in the hub. Nothing was published. See the hub's log.");
const RESTORED: &str = k(
    "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
);
const NO_HERMES: &str = k("No Hermes yet. Connect the first one in the Hermes tab.");
const NO_SCRIPT: &str = k(
    "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
);
const HINT: &str = k(
    "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
);
const STAGE_A: &str = k(
    "In this version the levels, the roles and the kinds of arrow are only shown here: no Hermes is told about them, and a note is still data, never an order. Only a link lets one Hermes send a note to another.",
);
const LEGEND: [&str; 3] = [
    k("From a higher level to a lower one: instruction"),
    k("From a lower level to a higher one: request"),
    k("Same level: note"),
];

/// Every sentence the script shows, by the name of its attribute on `#org`. The script has no
/// sentence of its own: it reads these, and the page writes them in the language of the page.
/// A sentence with `{name}` in it is filled by the script.
const SAY: &[(&str, &str)] = &[
    ("org-chart", k("Org chart")),
    ("view", k("View")),
    ("map", k("Map")),
    ("list", k("List")),
    ("add-level", k("+ Level")),
    ("undo", k("Undo")),
    ("publish", k("Publish")),
    ("draft-one", k("Draft: 1 change")),
    ("draft-many", k("Draft: {n} changes")),
    ("kind-instruction", k("instruction")),
    ("kind-request", k("request")),
    ("kind-note", k("note")),
    ("level-name", k("Name of level {n}")),
    ("remove-level", k("Remove empty level")),
    ("move-up", k("Move up")),
    ("move-down", k("Move down")),
    (
        "rule-level-name",
        k(
            "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        ),
    ),
    ("level-taken", k("Another level already has this name.")),
    (
        "rule-levels",
        k("The org chart has from 1 to {max} levels."),
    ),
    ("handle", k("Drag to another Hermes to create a link")),
    ("chip-answers", k("{alias} · answers")),
    ("chip-reads", k("{alias} · reads only")),
    ("no-number", k("no number")),
    ("not-placed", k("not placed yet")),
    ("card-label", k("{name}, level {level}, {state}")),
    (
        "unplaced",
        k(
            "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        ),
    ),
    ("kind-of", k("Kind: {kind}")),
    ("wakes-name", k("Wakes {name}")),
    (
        "not-wakeable",
        k("{name} cannot be woken now: this mark only applies once it can."),
    ),
    ("way-back-create", k("Create the way back")),
    ("way-back-remove", k("Remove the way back")),
    ("remove", k("Remove")),
    ("link-exists", k("This link already exists.")),
    ("role-label", k("Role of this Hermes")),
    (
        "role-hint",
        k(
            "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        ),
    ),
    ("level", k("Level")),
    ("move-left", k("Move left")),
    ("move-right", k("Move right")),
    ("change-in-accesses", k("Change them in Accesses")),
    ("no-access", k("No access to any number.")),
    ("wake-command", k("Change it with the command {command}.")),
    ("link-to", k("Link to")),
    ("add-link", k("Add link")),
    ("links-of", k("Links of this Hermes")),
    ("levels", k("Levels")),
    ("links", k("Links")),
    ("name", k("Name")),
    ("role", k("Role")),
    ("from", k("From")),
    ("to", k("To")),
    ("kind", k("Kind")),
    ("wakes", k("Wakes")),
    ("nobody", k("nobody")),
    ("no-link", k("No link.")),
    ("publish-title", k("Publish these changes?")),
    (
        "publish-note",
        k("Links are security rules. They only apply after you publish."),
    ),
    (
        "cross-warning",
        k(
            "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        ),
    ),
    ("back", k("Back")),
    ("close", k("Close")),
    ("numbers", k("Numbers")),
    ("can-be-woken", k("Can be woken")),
    ("yes", k("yes")),
    ("no", k("no")),
    ("change-link-new", k("New link: {from} → {to} ({kind})")),
    (
        "change-link-new-wake",
        k("New link: {from} → {to} ({kind}), wakes {to}"),
    ),
    ("change-wake-on", k("Link {from} → {to}: now wakes {to}")),
    (
        "change-wake-off",
        k("Link {from} → {to}: no longer wakes {to}"),
    ),
    ("change-link-removed", k("Link removed: {from} → {to}")),
    (
        "change-level",
        k("{name} changes level: {before} → {after}"),
    ),
    ("change-placed", k("{name} is placed in level {after}")),
    ("change-role", k("Role of {name} changed")),
    ("change-places", k("Places on the map changed: {n}")),
    ("change-level-new", k("New level: {name}")),
    (
        "change-level-renamed",
        k("Level renamed: {before} → {after}"),
    ),
    ("change-level-removed", k("Level removed: {name}")),
    ("change-level-order", k("Order of the levels: {names}")),
    (
        "rule-role",
        k(
            "A role has at most 280 characters, in one line, without control or direction characters.",
        ),
    ),
];

/// What the arrows are called, and the line drawn for each.
const KINDS: [(Kind, &str, &str); 3] = [
    (Kind::Instruction, "instruction", k("instruction")),
    (Kind::Request, "request", k("request")),
    (Kind::Note, "note", k("note")),
];

/// What a Hermes may do on a number, for the chip of its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// It may send somewhere on the number.
    Answers,
    /// It may only read.
    Reads,
    /// Something else: the alias only.
    Other,
}

impl Mode {
    const fn word(self) -> &'static str {
        match self {
            Mode::Answers => "answers",
            Mode::Reads => "reads",
            Mode::Other => "other",
        }
    }
}

/// One live Hermes of the page.
#[derive(Debug, Clone)]
struct Card {
    name: String,
    /// The level it sits in and its place, when it has one.
    place: Option<(Uuid, u16)>,
    role: String,
    tone: Tone,
    state: &'static str,
    woken: bool,
    numbers: Vec<(String, Mode)>,
}

/// What the page shows.
#[derive(Debug, Clone)]
struct OrgPage {
    chart: Chart,
    cards: Vec<Card>,
}

/// What the page says above the chart.
#[derive(Debug, Clone, Copy)]
enum Said {
    Nothing,
    Published,
    Stale,
    Invalid(OrgInvalid),
    TooLarge,
    Restored,
    Failed,
}

fn tone_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Ok => "ok",
        Tone::Wait => "wait",
        Tone::Bad => "bad",
        Tone::Idle => "idle",
    }
}

/// Reads what the page shows. Only reads.
async fn read_page(hub: &Hub, organization: ResolvedOrg) -> Result<OrgPage, RepoError> {
    let chart = repo::org::chart(hub.pool(), organization).await?;
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    let accesses = repo::access::list(hub.pool(), organization, None).await?;
    let sessions = hub.sessions_by_device();
    let cards = chart
        .members
        .iter()
        .map(|member| {
            let line = lines.iter().find(|line| line.hermes.id == member.id);
            let (tone, state, woken) = match line {
                Some(line) => {
                    let in_session = line
                        .in_use
                        .as_ref()
                        .is_some_and(|device| sessions.contains_key(&device.id));
                    let (tone, state) = data::standing(line, in_session);
                    (tone, state, line.hermes.can_be_woken)
                }
                // Created or removed between the two reads: the version check catches it.
                None => (Tone::Idle, k("No device"), false),
            };
            let numbers = accesses
                .iter()
                .filter(|access| access.hermes == member.name)
                .map(|access| {
                    let sends = (access.grant.scope == Scope::All && access.grant.verbs.send)
                        || access.lines.iter().any(|(_, verbs)| verbs.send);
                    let reads = (access.grant.scope == Scope::All && access.grant.verbs.read)
                        || access.lines.iter().any(|(_, verbs)| verbs.read);
                    let mode = if sends {
                        Mode::Answers
                    } else if reads {
                        Mode::Reads
                    } else {
                        Mode::Other
                    };
                    (access.number.as_str().to_owned(), mode)
                })
                .collect();
            Card {
                name: member.name.as_str().to_owned(),
                place: member
                    .place
                    .as_ref()
                    .map(|place| (place.level, place.x.get())),
                role: member
                    .place
                    .as_ref()
                    .and_then(|place| place.role.as_ref())
                    .map_or_else(String::new, |role| role.as_str().to_owned()),
                tone,
                state,
                woken,
                numbers,
            }
        })
        .collect();
    Ok(OrgPage { chart, cards })
}

/// The stored state, for the script: Hermes by name, links by `(from, to)`.
fn state_json(page: &OrgPage, lang: Lang) -> String {
    let levels: Vec<_> = page
        .chart
        .levels
        .iter()
        .map(|level| {
            serde_json::json!({
                "key": level.id.as_hyphenated().to_string(),
                "name": level.name.as_str(),
            })
        })
        .collect();
    let hermes: Vec<_> = page
        .cards
        .iter()
        .map(|card| {
            serde_json::json!({
                "name": card.name,
                "level": card.place.map(|(level, _)| level.as_hyphenated().to_string()),
                "x": card.place.map(|(_, x)| x),
                "role": card.role,
                "tone": tone_class(card.tone),
                "state": text::tr(lang, card.state),
                "woken": card.woken,
                "numbers": card
                    .numbers
                    .iter()
                    .map(|(alias, mode)| serde_json::json!({ "alias": alias, "mode": mode.word() }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let links: Vec<_> = page
        .chart
        .links
        .iter()
        .map(|link| {
            serde_json::json!({
                "from": link.from.as_str(),
                "to": link.to.as_str(),
                "wake": link.wake,
            })
        })
        .collect();
    serde_json::json!({ "levels": levels, "hermes": hermes, "links": links }).to_string()
}

/// The sentence of a refusal.
fn rule(html: &mut Html, invalid: OrgInvalid) {
    match invalid {
        OrgInvalid::Unreadable => html.t(k("The draft could not be read.")),
        OrgInvalid::LevelCount => html.fill(
            k("The org chart has from 1 to {max} levels."),
            &[("max", Arg::Number(LEVELS_MAX))],
        ),
        OrgInvalid::LevelName => html.t(k(
            "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        )),
        OrgInvalid::LevelNameTwice => html.t(k("Two levels have the same name.")),
        OrgInvalid::Role => html.t(k(
            "A role has at most 280 characters, in one line, without control or direction characters.",
        )),
        OrgInvalid::Place => html.t(k("A place on the map is out of range.")),
        OrgInvalid::Unknown => html.t(k(
            "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        )),
        OrgInvalid::Link => html.t(k(
            "A link joins two different Hermes, once in each direction.",
        )),
        OrgInvalid::TooLarge => html.t(k("The draft is too large to publish.")),
    };
}

fn said_line(html: &mut Html, said: Said) {
    match said {
        Said::Nothing => {}
        Said::Published => {
            html.raw("<p class=\"status\" role=\"status\">")
                .t(PUBLISHED)
                .raw("</p>");
        }
        Said::Stale => {
            html.raw("<p class=\"error\" role=\"alert\">")
                .t(STALE)
                .raw("</p>");
        }
        Said::Invalid(invalid) => {
            html.raw("<p class=\"error\" role=\"alert\">");
            rule(html, invalid);
            html.raw(" ").t(NOTHING).raw("</p>");
        }
        Said::TooLarge => {
            html.raw("<p class=\"error\" role=\"alert\">");
            rule(html, OrgInvalid::TooLarge);
            html.raw(" ").t(NOTHING).raw("</p>");
        }
        Said::Restored => {
            html.raw("<p class=\"error\" role=\"alert\">")
                .t(RESTORED)
                .raw("</p>");
        }
        Said::Failed => {
            html.raw("<p class=\"error\" role=\"alert\">")
                .t(FAILED)
                .raw("</p>");
        }
    }
}

fn chip(html: &mut Html, alias: &str, mode: Mode) {
    html.raw("<span class=\"chip\">");
    match mode {
        Mode::Answers => html.fill(k("{alias} · answers"), &[("alias", Arg::Text(alias))]),
        Mode::Reads => html.fill(k("{alias} · reads only"), &[("alias", Arg::Text(alias))]),
        Mode::Other => html.text(alias),
    };
    html.raw("</span> ");
}

/// The three plain tables: what the page shows with no script, and what a screen reader finds
/// beside the map.
fn tables(html: &mut Html, page: &OrgPage) {
    html.raw("<section class=\"org-static\"><p class=\"note no-script\">")
        .t(NO_SCRIPT)
        .raw("</p>");
    // Levels.
    html.raw("<h2 class=\"org-title\">")
        .t(k("Levels"))
        .raw("</h2><div class=\"frame\"><table class=\"list\"><thead><tr><th scope=\"col\">")
        .t(k("Level"))
        .raw("</th><th scope=\"col\">")
        .t(k("Name"))
        .raw("</th><th scope=\"col\">Hermes</th></tr></thead><tbody>");
    for (position, level) in page.chart.levels.iter().enumerate() {
        html.raw("<tr><td class=\"num\">")
            .number(position.saturating_add(1))
            .raw("</td><td>")
            .text(level.name.as_str())
            .raw("</td><td>");
        let here: Vec<&Card> = page
            .cards
            .iter()
            .filter(|card| card.place.is_some_and(|(id, _)| id == level.id))
            .collect();
        if here.is_empty() {
            html.t(k("nobody"));
        }
        for (index, card) in here.iter().enumerate() {
            if index > 0 {
                html.raw(", ");
            }
            html.text(&card.name);
        }
        html.raw("</td></tr>");
    }
    let unplaced: Vec<&Card> = page
        .cards
        .iter()
        .filter(|card| card.place.is_none())
        .collect();
    if !unplaced.is_empty() {
        html.raw("<tr><td class=\"num\">—</td><td>")
            .t(k("not placed yet"))
            .raw("</td><td>");
        for (index, card) in unplaced.iter().enumerate() {
            if index > 0 {
                html.raw(", ");
            }
            html.text(&card.name);
        }
        html.raw("</td></tr>");
    }
    html.raw("</tbody></table></div>");
    // Hermes.
    html.raw("<h2 class=\"org-title\">Hermes</h2><div class=\"frame\"><table class=\"list\"><thead><tr><th scope=\"col\">Hermes</th><th scope=\"col\">")
        .t(k("Level"))
        .raw("</th><th scope=\"col\">")
        .t(k("Role"))
        .raw("</th><th scope=\"col\">")
        .t(k("Numbers"))
        .raw("</th></tr></thead><tbody>");
    for card in &page.cards {
        html.raw("<tr><td><span class=\"name\">")
            .text(&card.name)
            .raw("</span></td><td>");
        match card
            .place
            .and_then(|(id, _)| page.chart.levels.iter().find(|level| level.id == id))
        {
            Some(level) => html.text(level.name.as_str()),
            None => html.t(k("not placed yet")),
        };
        html.raw("</td><td>").text(&card.role).raw("</td><td>");
        if card.numbers.is_empty() {
            html.t(k("no number"));
        }
        for (alias, mode) in &card.numbers {
            chip(html, alias, *mode);
        }
        html.raw("</td></tr>");
    }
    html.raw("</tbody></table></div>");
    // Links.
    html.raw("<h2 class=\"org-title\">")
        .t(k("Links"))
        .raw("</h2><div class=\"frame\"><table class=\"list\"><thead><tr><th scope=\"col\">")
        .t(k("From"))
        .raw("</th><th scope=\"col\">")
        .t(k("To"))
        .raw("</th><th scope=\"col\">")
        .t(k("Kind"))
        .raw("</th><th scope=\"col\">")
        .t(k("Wakes"))
        .raw("</th></tr></thead><tbody>");
    if page.chart.links.is_empty() {
        html.raw("<tr class=\"empty\"><td colspan=\"4\">")
            .t(k("No link."))
            .raw("</td></tr>");
    }
    for link in &page.chart.links {
        let arrow = kind(
            page.chart.position_of_member(&link.from),
            page.chart.position_of_member(&link.to),
        );
        html.raw("<tr><td>")
            .text(link.from.as_str())
            .raw("</td><td>")
            .text(link.to.as_str())
            .raw("</td><td>");
        match arrow.and_then(|arrow| KINDS.iter().find(|(of, _, _)| *of == arrow)) {
            Some((_, _, word)) => html.t(word),
            None => html.raw("—"),
        };
        html.raw("</td><td>")
            .t(if link.wake { k("yes") } else { k("no") })
            .raw("</td></tr>");
    }
    html.raw("</tbody></table></div></section>");
}

/// The page, with `said` above the chart and, after a refusal, the draft to put back.
fn write(frame: &Frame, page: &OrgPage, said: Said, echo: Option<&str>) -> String {
    let mut html = Html::new(frame.lang);
    view::open(&mut html, frame, Tab::Org, Over::Nothing);
    html.raw("<div class=\"heading\"><div><h1>")
        .bracketed(k("Who talks to [whom]"))
        .raw("</h1><p class=\"counters org-hint with-script\">")
        .t(HINT)
        .raw("</p></div><div id=\"org-bar\" class=\"org-bar\"></div></div>");
    said_line(&mut html, said);
    if page.cards.is_empty() {
        html.raw("<div class=\"frame\"><table class=\"list\"><tbody><tr class=\"empty\"><td>")
            .t(NO_HERMES)
            .raw("</td></tr></tbody></table></div>");
    } else {
        html.raw("<div id=\"org\" class=\"frame org\" hidden data-reserved=\"")
            .text(&reserved_words())
            .raw("\" data-max-levels=\"")
            .number(LEVELS_MAX)
            .raw("\" data-max-role=\"")
            .number(ROLE_MAX_CHARS)
            .raw("\" data-max-x=\"")
            .number(usize::from(MAP_X_MAX))
            .raw("\" data-wake-command=\"arauto-hub admin wake {name} on\" data-accesses=\"/accesses\"");
        for (name, key) in SAY {
            html.raw(" data-say-").raw(name).raw("=\"").t(key).raw("\"");
        }
        // The script reads the namespace of its SVG from this element, which the browser parses
        // in the right one: no address is written in the script.
        html.raw("><svg id=\"org-svg-seed\" hidden aria-hidden=\"true\"></svg></div><div id=\"org-live\" data-live hidden><div id=\"org-data\" data-version=\"")
            .text(&page.chart.version().to_hex())
            .raw("\" data-state=\"")
            .text(&state_json(page, frame.lang))
            .raw("\"></div></div>");
        if let Some(echo) = echo {
            html.raw("<div id=\"org-draft\" hidden data-draft=\"")
                .text(echo)
                .raw("\"></div>");
        }
        tables(&mut html, page);
    }
    html.raw("<div class=\"legend\">");
    for ((_, class, _), sentence) in KINDS.iter().zip(LEGEND) {
        html.raw("<span><i class=\"wire-sample ")
            .raw(class)
            .raw("\"></i>")
            .t(sentence)
            .raw("</span>");
    }
    html.raw("</div><p class=\"note\">")
        .t(STAGE_A)
        .raw("</p><form id=\"org-form\" method=\"post\" action=\"/org/publish\" hidden><input type=\"hidden\" name=\"version\"><input type=\"hidden\" name=\"draft\"></form>");
    view::close(&mut html);
    html.raw(view::TAIL);
    html.finish()
}

/// The page and its answer after a refusal: the fresh state, what happened, and the draft.
async fn answered(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    status: StatusCode,
    said: Said,
    echo: Option<&str>,
) -> Result<Response<Body>, RepoError> {
    let frame = data::frame(hub, organization, lang).await?;
    let page = read_page(hub, organization).await?;
    Ok(http::page(status, write(&frame, &page, said, echo)))
}

/// `GET /org`. Only reads.
pub(super) async fn page(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<Response<Body>, RepoError> {
    let organization = match super::any_session_of(hub, panel, headers).await? {
        Some((organization, session)) if session.full => organization,
        Some(_) => return Ok(http::see_other("/first-access")),
        None => return Ok(http::see_other("/sign-in")),
    };
    let lang = super::lang_of(headers);
    let published = query.is_some_and(|query| query.split('&').any(|pair| pair == "published"));
    answered(
        hub,
        organization,
        lang,
        StatusCode::OK,
        if published {
            Said::Published
        } else {
            Said::Nothing
        },
        None,
    )
    .await
}

/// `POST /org/publish`. The request already named the panel and came from a page of the panel.
pub(super) async fn publish(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = session_of(hub, panel, headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let lang = super::lang_of(headers);
    let form = match http::form_up_to(headers, body, PUBLISH_FORM_MAX).await {
        Ok(form) => form,
        Err(http::FormError::TooLarge) => {
            return answered(
                hub,
                organization,
                lang,
                StatusCode::PAYLOAD_TOO_LARGE,
                Said::TooLarge,
                None,
            )
            .await;
        }
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let base = form
        .field("version")
        .and_then(|text| ChartVersion::parse_hex(text.trim()));
    let draft = form.field("draft");
    let (Some(base), Some(draft)) = (base, draft) else {
        return answered(
            hub,
            organization,
            lang,
            StatusCode::BAD_REQUEST,
            Said::Invalid(OrgInvalid::Unreadable),
            None,
        )
        .await;
    };
    let draft = match Draft::parse(&draft) {
        Ok(draft) => draft,
        Err(OrgInvalid::Unreadable) => {
            return answered(
                hub,
                organization,
                lang,
                StatusCode::BAD_REQUEST,
                Said::Invalid(OrgInvalid::Unreadable),
                None,
            )
            .await;
        }
        Err(invalid) => {
            return answered(
                hub,
                organization,
                lang,
                StatusCode::UNPROCESSABLE_ENTITY,
                Said::Invalid(invalid),
                None,
            )
            .await;
        }
    };
    let echo = draft.to_json();
    let admin = admin::Session::of_panel(
        hub.pool().clone(),
        hub.mark().clone(),
        organization,
        session.admin,
    );
    match admin.org_publish(&draft, base, hub.now_ms()).await {
        Ok(PublishOutcome::Published(_)) => Ok(http::see_other("/org?published")),
        Ok(PublishOutcome::Unchanged) => Ok(http::see_other("/org")),
        Ok(PublishOutcome::Stale) => {
            answered(
                hub,
                organization,
                lang,
                StatusCode::CONFLICT,
                Said::Stale,
                Some(&echo),
            )
            .await
        }
        Ok(PublishOutcome::Invalid(invalid)) => {
            answered(
                hub,
                organization,
                lang,
                StatusCode::UNPROCESSABLE_ENTITY,
                Said::Invalid(invalid),
                Some(&echo),
            )
            .await
        }
        Err(AdminError::Restored(_)) => {
            answered(
                hub,
                organization,
                lang,
                StatusCode::CONFLICT,
                Said::Restored,
                Some(&echo),
            )
            .await
        }
        // Committed: the mark did not follow. The frame then shows the alert of the mark.
        Err(AdminError::Repo(error @ RepoError::Audit(AuditError::MarkNotAdvanced { .. }))) => {
            hub.security_write_failed(organization, None, error).await;
            Ok(http::see_other("/org?published"))
        }
        Err(error) => {
            // The error can carry no value of the draft: `OrgInvalid` has unit variants and a
            // repository error holds the text of the database or a fixed word.
            tracing::error!(%error, "the panel could not publish the org chart");
            answered(
                hub,
                organization,
                lang,
                StatusCode::INTERNAL_SERVER_ERROR,
                Said::Failed,
                Some(&echo),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::org::{LevelName, MapX, Place, RoleText, StoredLevel, StoredMember};
    use arauto_protocol::names::HermesName;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    fn page_with(role: &str) -> OrgPage {
        let level = Uuid::from_u128(5);
        let ana = HermesName::parse("ana").unwrap();
        OrgPage {
            chart: Chart {
                levels: vec![StoredLevel {
                    id: level,
                    name: LevelName::parse("board").unwrap(),
                }],
                members: vec![StoredMember {
                    id: Uuid::from_u128(1),
                    name: ana,
                    place: Some(Place {
                        level,
                        x: MapX::new(10).unwrap(),
                        role: RoleText::parse(role).unwrap(),
                    }),
                }],
                links: vec![],
            },
            cards: vec![Card {
                name: "ana".to_owned(),
                place: Some((level, 10)),
                role: role.to_owned(),
                tone: Tone::Ok,
                state: k("Connected"),
                woken: true,
                numbers: vec![(HOSTILE.to_owned(), Mode::Answers)],
            }],
        }
    }

    /// The entities the page writes for the five characters, back to the characters.
    fn unescape(text: &str) -> String {
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }

    #[test]
    fn what_a_hermes_or_the_admin_wrote_is_escaped_on_the_org_chart() {
        let page = page_with(HOSTILE);
        let frame = Frame::default();
        let echo = Draft::parse(
            &serde_json::json!({
                "levels": [ { "key": "new-1", "name": "board" } ],
                "hermes": [ { "name": "ana", "level": "new-1", "x": 1, "role": HOSTILE } ],
                "links": [],
            })
            .to_string(),
        )
        .unwrap()
        .to_json();
        let html = write(&frame, &page, Said::Stale, Some(&echo));
        let plain = html
            .replace("<script src=\"/assets/panel.js\"></script>", "")
            .replace("<script src=\"/assets/org.js\"></script>", "");
        assert!(!plain.contains("<script"), "{plain}");
        assert!(!plain.contains("<img"), "{plain}");
        assert!(plain.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains(" style=\""));
        // The stored state sits in an attribute: no raw quote, angle bracket or apostrophe, and
        // what the script reads back is what was given.
        let attribute = html
            .split("data-state=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        for raw in ['<', '>', '\'', '"'] {
            assert!(!attribute.contains(raw), "{raw}");
        }
        let state: serde_json::Value = serde_json::from_str(&unescape(attribute)).unwrap();
        assert_eq!(state["hermes"][0]["role"], HOSTILE);
        assert_eq!(state["hermes"][0]["numbers"][0]["alias"], HOSTILE);
        assert_eq!(state["levels"][0]["name"], "board");
        // The echo of a refused draft is the same.
        let echoed = html
            .split("data-draft=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let echoed: serde_json::Value = serde_json::from_str(&unescape(echoed)).unwrap();
        assert_eq!(echoed["hermes"][0]["role"], HOSTILE);
    }

    #[test]
    fn without_the_script_the_page_shows_the_lists_and_says_why() {
        let html = write(
            &Frame::default(),
            &page_with("keeps the books"),
            Said::Nothing,
            None,
        );
        assert!(html.contains("<section class=\"org-static\">"));
        assert!(html.contains("Editing the org chart needs the script of the panel."));
        assert!(html.contains("keeps the books"));
        // The editor is hidden until the script shows it.
        assert!(html.contains("<div id=\"org\" class=\"frame org\" hidden "));
        assert_eq!(html.matches("<table class=\"list\">").count(), 3);
        // The stage-A sentence is always there.
        assert!(html.contains("only shown here"));
        // Nobody on the chart: the empty state, and no editor.
        let empty = OrgPage {
            chart: Chart::default(),
            cards: vec![],
        };
        let html = write(&Frame::default(), &empty, Said::Nothing, None);
        assert!(html.contains("No Hermes yet. Connect the first one in the Hermes tab."));
        assert!(!html.contains("id=\"org\""));
    }

    #[test]
    fn the_org_page_is_a_tab_with_one_live_region_of_its_own_and_two_script_files() {
        let html = write(&Frame::default(), &page_with(""), Said::Nothing, None);
        // The map is not swapped under the admin: the main is no live region, the alerts and the
        // stored state are.
        assert!(!html.contains("live-main"));
        assert!(html.contains("<main id=\"org-main\">"));
        assert_eq!(
            html.matches("<div id=\"live-alerts\" data-live>").count(),
            1
        );
        assert_eq!(html.matches(" data-live").count(), 2);
        assert_eq!(html.matches("<script").count(), 2);
        assert!(html.contains(
            "<script src=\"/assets/panel.js\"></script><script src=\"/assets/org.js\"></script>"
        ));
        // Four tabs, the fourth is this one.
        assert_eq!(html.matches("<nav class=\"tabs\"").count(), 1);
        assert!(html.contains("<a href=\"/org\" aria-current=\"page\">Org chart</a>"));
        assert!(html.contains("<a href=\"/accesses\">Accesses</a>"));
        // The data of the script sits in an attribute: no script block of data.
        assert!(!html.contains("<script>"));
        assert!(html.contains("<svg id=\"org-svg-seed\" hidden aria-hidden=\"true\"></svg>"));
        // A refused draft comes back outside the live region.
        let echo = Draft::parse(
            &serde_json::json!({
                "levels": [ { "key": "new-1", "name": "board" } ],
                "hermes": [ { "name": "ana", "level": "new-1", "x": 1, "role": "" } ],
                "links": [],
            })
            .to_string(),
        )
        .unwrap()
        .to_json();
        let html = write(&Frame::default(), &page_with(""), Said::Stale, Some(&echo));
        let draft_at = html.find("id=\"org-draft\"").unwrap();
        let live_at = html.find("id=\"org-live\"").unwrap();
        let data_at = html.find("id=\"org-data\"").unwrap();
        assert!(draft_at > data_at && live_at < data_at);
        assert!(html.contains("role=\"alert\""));
    }

    #[test]
    fn the_page_gives_the_script_every_sentence_and_the_reserved_words() {
        let html = write(&Frame::default(), &page_with(""), Said::Nothing, None);
        for (name, _) in SAY {
            assert!(
                html.contains(&format!(" data-say-{name}=\"")),
                "no data-say-{name}"
            );
        }
        assert!(html.contains(&format!(" data-reserved=\"{}\"", reserved_words())));
        assert!(html.contains(" data-max-levels=\"12\""));
        assert!(html.contains(" data-max-role=\"280\""));
        assert!(html.contains(" data-max-x=\"4000\""));
        // The sentence of a placeholder reaches the script with the placeholder.
        assert!(html.contains(" data-say-draft-many=\"Draft: {n} changes\""));
        assert!(html.contains("<div id=\"org-live\" data-live hidden>"));
        assert!(
            html.contains("<form id=\"org-form\" method=\"post\" action=\"/org/publish\" hidden>")
        );
    }
}
