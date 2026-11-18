//! The window of the actions of one line, over the list it came from.
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /hermes/actions?name=` | every action the line of that Hermes offers now |
//! | `GET /whatsapps/actions?alias=` | every action the line of that number offers now |
//!
//! The window offers exactly what the line offered before it had one button: the same links, the
//! same forms with the same fields, the same questions that come after (a revocation, the alias
//! typed to disconnect or remove). It decides nothing and asks nothing new, and a `GET` writes
//! nothing. The actions that end something are in a group of their own, at the end. An unknown
//! name or alias, or one that is not a name, sends back to the list, as the other windows do.

use arauto_protocol::names::{HermesName, NumberAlias};
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Response, StatusCode};

use super::html::{Arg, Html};
use super::text::k;
use super::view::{self, Frame, HermesPage, HermesRow, LineAction, NumberRow, NumbersPage, Over};
use super::{PanelState, data, http, session_of};
use crate::channel::Hub;
use crate::repo::RepoError;

const CLOSE: &str =
    "<svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg>";

// The paths of the icons: 16 by 16, drawn with a line, like the rest of the panel.
const PLAY: &str = "<path d=\"M5 3.5l7 4.5-7 4.5z\"/>";
const REDO: &str = "<path d=\"M13.5 8a5.5 5.5 0 1 1-1.6-3.9\"/><path d=\"M13.5 2.5v3.2h-3.2\"/>";
const CHAT: &str = "<path d=\"M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z\"/>";
const CHECK: &str = "<path d=\"M3 8.5l3.2 3L13 4.5\"/>";
const POWER: &str = "<path d=\"M8 2v5.5\"/><path d=\"M4.6 4.4a5 5 0 1 0 6.8 0\"/>";
const BAN: &str = "<circle cx=\"8\" cy=\"8\" r=\"5.5\"/><path d=\"M4.2 11.8l7.6-7.6\"/>";
const TRASH: &str =
    "<path d=\"M2.5 4.5h11\"/><path d=\"M6 4.5V3h4v1.5\"/><path d=\"M4 4.5l.7 8.5h6.6l.7-8.5\"/>";

const W_CONNECT: &str = k("Issues the first invite for {name}.");
const W_CONTINUE: &str = k("Goes back to the connection of {name} that is in progress.");
const W_RECONNECT: &str = k("Issues an invite. When it is used, {name} moves to the new device.");
const W_WHO: &str = k("Which chat or telephones may give it orders on WhatsApp.");
const W_REVOKE: &str = k("Closes {name} in the hub. Asks first.");
const W_PAIR: &str = k("Opens the window to pair {alias} with a QR code or a pairing code.");
const W_CONTINUE_PAIRING: &str = k("Goes back to the pairing of {alias} that is running.");
const W_SEE_PAIRING: &str = k("Shows the pairing of {alias} that another panel session started.");
const W_DISCONNECT: &str = k("Stops {alias} for every Hermes. Asks for the alias.");
const W_RESTART: &str = k("Starts the failed session of {alias} again.");
const W_DECIDE: &str = k("Another phone paired as {alias}: keep it or undo.");
const W_ORDERS: &str = k("Which Hermes takes orders in the chats of this number.");
const W_REMOVE: &str = k("Takes {alias} out of the hub. Asks for the alias.");

/// Where an action goes.
enum Go<'a> {
    /// A link: the path up to and including `?name=` or `?alias=`, then the value.
    Link(&'static str, &'a str),
    /// The link of a pairing that is running: the alias and the run.
    Run(&'a str, &'a str),
    /// A form that posts one field.
    Post(&'static str, &'static str, &'a str),
}

/// One line of the list of actions.
struct Item<'a> {
    icon: &'static str,
    label: &'static str,
    /// What the action has now, when it has something to show.
    value: Option<&'a str>,
    why: &'static str,
    go: Go<'a>,
}

fn inner(html: &mut Html, one: &Item<'_>, subject: &str) {
    html.raw("<svg class=\"icon\" viewBox=\"0 0 16 16\" aria-hidden=\"true\">")
        .raw(one.icon)
        .raw("</svg><span class=\"lbl\">")
        .t(one.label)
        .raw("</span>");
    match one.value {
        Some(value) => {
            html.raw("<span class=\"val\">").text(value).raw("</span>");
        }
        None => {
            html.raw("<span></span>");
        }
    }
    html.raw("<span class=\"chev\"></span><span class=\"why\">")
        .fill(
            one.why,
            &[("name", Arg::Text(subject)), ("alias", Arg::Text(subject))],
        )
        .raw("</span>");
}

fn entry(html: &mut Html, one: &Item<'_>, subject: &str) {
    html.raw("<li>");
    match one.go {
        Go::Link(path, value) => {
            html.raw("<a href=\"").raw(path).text(value).raw("\">");
            inner(html, one, subject);
            html.raw("</a>");
        }
        Go::Run(alias, run) => {
            html.raw("<a href=\"/whatsapps/pair/wait?alias=")
                .text(alias)
                .raw("&amp;run=")
                .text(run)
                .raw("\">");
            inner(html, one, subject);
            html.raw("</a>");
        }
        Go::Post(action, field, value) => {
            html.raw("<form method=\"post\" action=\"")
                .raw(action)
                .raw("\"><input type=\"hidden\" name=\"")
                .raw(field)
                .raw("\" value=\"")
                .text(value)
                .raw("\"><button type=\"submit\">");
            inner(html, one, subject);
            html.raw("</button></form>");
        }
    }
    html.raw("</li>");
}

/// The window: the title, the head line, the actions, then the ones that end something.
fn window(
    html: &mut Html,
    subject: &str,
    back: &'static str,
    head: impl FnOnce(&mut Html),
    keep: &[Item<'_>],
    end: &[Item<'_>],
) {
    html.raw("<dialog open class=\"modal\" id=\"guide\" aria-labelledby=\"actions-title\"><div class=\"modal-head\"><h2 id=\"actions-title\">")
        .text(subject)
        .raw("</h2><a class=\"close\" href=\"")
        .raw(back)
        .raw("\" aria-label=\"")
        .t(k("Close"))
        .raw("\">")
        .raw(CLOSE)
        .raw("</a></div><div class=\"modal-body\"><div class=\"whohead\">");
    head(html);
    html.raw("</div>");
    if !keep.is_empty() {
        html.raw("<ul class=\"acts\">");
        for one in keep {
            entry(html, one, subject);
        }
        html.raw("</ul>");
    }
    if !end.is_empty() {
        html.raw("<ul class=\"acts end\">");
        for one in end {
            entry(html, one, subject);
        }
        html.raw("</ul>");
    }
    html.raw("</div><div class=\"modal-foot\"><a class=\"button\" href=\"")
        .raw(back)
        .raw("\">")
        .t(k("Close"))
        .raw("</a></div></dialog>")
        .raw(view::TAIL);
}

fn hermes_items(row: &HermesRow) -> (Vec<Item<'_>>, Vec<Item<'_>>) {
    let name = row.name.as_str();
    let first = match row.action {
        view::RowAction::Continue => Item {
            icon: PLAY,
            label: k("Continue"),
            value: None,
            why: W_CONTINUE,
            go: Go::Link("/hermes/wait?name=", name),
        },
        view::RowAction::Connect => Item {
            icon: PLAY,
            label: k("Connect"),
            value: None,
            why: W_CONNECT,
            go: Go::Post("/hermes/invite", "name", name),
        },
        view::RowAction::Reconnect => Item {
            icon: REDO,
            label: k("Reconnect"),
            value: None,
            why: W_RECONNECT,
            go: Go::Post("/hermes/invite", "name", name),
        },
    };
    let keep = vec![
        first,
        Item {
            icon: CHAT,
            label: k("Who gives it orders"),
            value: Some(row.orders.as_str()),
            why: W_WHO,
            go: Go::Link("/hermes/whatsapp?name=", name),
        },
    ];
    let mut end = Vec::new();
    if row.action.has_something() {
        end.push(Item {
            icon: BAN,
            label: k("Revoke"),
            value: None,
            why: W_REVOKE,
            go: Go::Link("/hermes/revoke?name=", name),
        });
    }
    (keep, end)
}

fn number_item<'a>(action: &'a LineAction, alias: &'a str) -> Item<'a> {
    let link = |icon, label, why, path| Item {
        icon,
        label,
        value: None,
        why,
        go: Go::Link(path, alias),
    };
    match action {
        LineAction::Pair => link(PLAY, k("Pair"), W_PAIR, "/whatsapps/pair?alias="),
        LineAction::PairAgain => link(PLAY, k("Pair again"), W_PAIR, "/whatsapps/pair?alias="),
        LineAction::Continue(run) => Item {
            icon: PLAY,
            label: k("Continue pairing"),
            value: None,
            why: W_CONTINUE_PAIRING,
            go: Go::Run(alias, run),
        },
        LineAction::SeePairing => link(
            PLAY,
            k("See the pairing"),
            W_SEE_PAIRING,
            "/whatsapps/pair/wait?alias=",
        ),
        LineAction::Disconnect => link(
            POWER,
            k("Disconnect"),
            W_DISCONNECT,
            "/whatsapps/disconnect?alias=",
        ),
        LineAction::Decide => link(CHECK, k("Decide"), W_DECIDE, "/whatsapps/decide?alias="),
        LineAction::Orders => link(CHAT, k("Orders"), W_ORDERS, "/whatsapps/orders?alias="),
        LineAction::Remove => link(TRASH, k("Remove"), W_REMOVE, "/whatsapps/remove?alias="),
        LineAction::Restart => Item {
            icon: REDO,
            label: k("Restart"),
            value: None,
            why: W_RESTART,
            go: Go::Post("/whatsapps/restart", "alias", alias),
        },
    }
}

fn number_items(row: &NumberRow) -> (Vec<Item<'_>>, Vec<Item<'_>>) {
    let (mut keep, mut end) = (Vec::new(), Vec::new());
    for action in &row.actions {
        let one = number_item(action, row.alias.as_str());
        if matches!(action, LineAction::Disconnect | LineAction::Remove) {
            end.push(one);
        } else {
            keep.push(one);
        }
    }
    (keep, end)
}

/// The list of the Hermes with the window of the actions of `name` over it. `None` when there is
/// no such Hermes in the list.
#[must_use]
pub fn hermes_page(frame: &Frame, page: &HermesPage, name: &str) -> Option<String> {
    let row = page.rows.iter().find(|row| row.name == name)?;
    let mut html = Html::new(frame.lang);
    view::hermes_into(&mut html, frame, page, Over::Guide { refresh: None });
    let (keep, end) = hermes_items(row);
    window(
        &mut html,
        &row.name,
        "/",
        |html| {
            view::state(html, row.tone, row.state);
            html.raw("<span>");
            view::names(html, &row.numbers);
            html.raw("</span>");
        },
        &keep,
        &end,
    );
    Some(html.finish())
}

/// The list of numbers with the window of the actions of `alias` over it. `None` when there is
/// no such number in the list.
#[must_use]
pub fn numbers_page(frame: &Frame, page: &NumbersPage, alias: &str) -> Option<String> {
    let row = page.rows.iter().find(|row| row.alias == alias)?;
    let mut html = Html::new(frame.lang);
    view::numbers_into(&mut html, frame, page, Over::Guide { refresh: None });
    let (keep, end) = number_items(row);
    window(
        &mut html,
        &row.alias,
        "/whatsapps",
        |html| {
            view::state(html, row.tone, row.state);
            html.raw("<span class=\"mono\">");
            match &row.phone {
                Some(phone) => html.text(phone),
                None => html.raw("—"),
            };
            html.raw("</span>");
        },
        &keep,
        &end,
    );
    Some(html.finish())
}

/// Answers `GET /hermes/actions` and `GET /whatsapps/actions`.
pub(super) async fn answer(
    hub: &Hub,
    panel: &PanelState,
    parts: &Parts,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = session_of(hub, panel, &parts.headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let lang = super::lang_of(&parts.headers);
    let query = parts.uri.query();
    if parts.uri.path() == "/hermes/actions" {
        let Some(name) =
            http::query_field(query, "name").and_then(|name| HermesName::parse(name.trim()).ok())
        else {
            return Ok(http::see_other("/"));
        };
        let frame = data::frame(hub, organization, lang).await?;
        let list = data::hermes(hub, organization, lang).await?;
        return Ok(hermes_page(&frame, &list, name.as_str()).map_or_else(
            || http::see_other("/"),
            |html| http::page(StatusCode::OK, html),
        ));
    }
    let Some(alias) =
        http::query_field(query, "alias").and_then(|alias| NumberAlias::parse(alias.trim()).ok())
    else {
        return Ok(http::see_other("/whatsapps"));
    };
    let frame = data::frame(hub, organization, lang).await?;
    let list = data::numbers(hub, organization, lang, session.id).await?;
    Ok(numbers_page(&frame, &list, alias.as_str()).map_or_else(
        || http::see_other("/whatsapps"),
        |html| http::page(StatusCode::OK, html),
    ))
}

/// `POST /alerts/dismiss`: marks one alert of the band as seen, through the function
/// `admin alerts --ack` calls. The command writes no audit event for it and neither does this.
/// An alert that is already gone is not an error. Only the exact paths of the three lists are
/// a way back; anything else goes to the first page.
pub(super) async fn dismiss(
    hub: &Hub,
    panel: &PanelState,
    parts: &Parts,
    body: hyper::body::Incoming,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = session_of(hub, panel, &parts.headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let form = match http::form(&parts.headers, body).await {
        Ok(form) => form,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let Some(id) = form
        .field("id")
        .and_then(|id| uuid::Uuid::parse_str(id.trim()).ok())
    else {
        return Ok(http::plain(
            StatusCode::BAD_REQUEST,
            "The form was not read.\n",
        ));
    };
    let admin = crate::admin::Session::of_panel(
        hub.pool().clone(),
        hub.mark().clone(),
        organization,
        session.admin,
    );
    if let Err(error) = admin.acknowledge(id, hub.now_ms()).await {
        tracing::error!(error = %error, "the panel could not mark an alert as seen");
        return Ok(http::plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The alert was not dismissed.\n",
        ));
    }
    let back = match form.field("return").as_deref().map(String::as_str) {
        Some("/whatsapps") => "/whatsapps",
        Some("/accesses") => "/accesses",
        _ => "/",
    };
    Ok(http::see_other(back))
}
