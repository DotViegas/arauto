//! Who gives a Hermes orders on WhatsApp, in the panel (decision 0051, a proposal; part B).
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /hermes/whatsapp?name=` | the window of a Hermes: its telephone, who gives it orders |
//! | `GET /hermes/whatsapp?name=&gives=ALIAS` or `&takes=ALIAS` | the same window with the telephone of that number typed in; nothing is saved |
//! | `POST /hermes/whatsapp` | `admin hermes whatsapp`, or the window that asks first, or the way back |
//! | `GET /whatsapps/orders?alias=` | from a number: which Hermes, and what the number is for it |
//!
//! Nothing here decides. The write is [`crate::admin::Session::hermes_orders_set`], the one the
//! command calls, with the same rules and the same audit; what is of the panel is the order of
//! the screens and the question that comes first. The risk is said where the record is set:
//! one fixed sentence in the window whatever is recorded, and a window over it, with no script,
//! before a save that leaves no chat protected or that unprotects one. Nothing is saved before
//! the answer to that window.
//!
//! A telephone is never put in an address: the link of a number names its alias, and the hub
//! reads the telephone itself. A `GET` writes nothing.

use arauto_protocol::names::{HermesName, NumberAlias};
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Method, Response, StatusCode};
use hyper::body::Incoming;

use super::html::{Arg, Html};
use super::text::{Lang, k};
use super::view::{self, Frame, HermesPage, NumbersPage, Over};
use super::{PanelState, data, http, session_of};
use crate::admin::orders::{OrdersEdit, preview};
use crate::admin::{self, AdminError};
use crate::channel::Hub;
use crate::core::number::NumberState;
use crate::db::ResolvedOrg;
use crate::engine::Phone;
use crate::repo::orders::{Invalid, OrdersRecord};
use crate::repo::panel::PanelSession;
use crate::repo::{self, RepoError};

const CLOSE: &str =
    "<svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg>";

const BAD_PHONE: &str =
    k("Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.");
const NEEDS_BRIDGE: &str = k("Type the telephone {hermes}'s WhatsApp runs on first.");
const TAKEN: &str = k("{other} already takes orders on this telephone.");
const TOO_MANY: &str = k("At most 16 telephones.");
const RESTORED: &str = k(
    "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
);
const FAILED: &str =
    k("Something failed in the hub. See the hub's log and check the record of the Hermes.");

/// What the form has in it, as it was typed: kept as text so that a refusal gives it back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Typed {
    /// The telephone the Hermes's WhatsApp runs on.
    pub takes: String,
    /// "Its own WhatsApp": `self-chat`.
    pub own: bool,
    /// The telephones that give it orders, one per line.
    pub from: String,
}

impl Typed {
    /// What the record says, for the form to start from.
    fn of(record: &OrdersRecord) -> Self {
        Self {
            takes: record
                .takes_orders_on
                .as_ref()
                .map(plus)
                .unwrap_or_default(),
            own: record.self_chat,
            from: record
                .orders_from
                .iter()
                .map(plus)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// What a form sent. A line break is always `\n`, and a choice that did not come is the
    /// list.
    fn of_form(form: &http::Form) -> Self {
        let text = |name: &str| {
            form.field(name)
                .map(|value| value.replace("\r\n", "\n").replace('\r', "\n"))
                .unwrap_or_default()
        };
        Self {
            takes: text("takes").trim().to_owned(),
            own: text("source") == "self",
            from: text("from"),
        }
    }

    /// The edit the command would be given: every field of the form is the whole answer.
    fn edit(&self) -> OrdersEdit {
        let takes = (!self.takes.trim().is_empty()).then(|| self.takes.trim().to_owned());
        if self.own {
            return OrdersEdit {
                takes_orders_on: Some(takes),
                self_chat: true,
                orders_from: None,
            };
        }
        OrdersEdit {
            takes_orders_on: Some(takes),
            self_chat: false,
            orders_from: Some(
                self.from
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ),
        }
    }
}

fn plus(phone: &Phone) -> String {
    format!("+{}", phone.digits())
}

/// Why a form came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// A telephone that is not one.
    BadPhone,
    /// A list or `self-chat` with no telephone of the Hermes.
    NeedsBridge,
    /// More than 16 telephones.
    TooMany,
    /// Another Hermes takes orders on that telephone.
    Taken(String),
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
    fn of(error: &AdminError) -> Self {
        match error {
            AdminError::InvalidPhone => Problem::BadPhone,
            AdminError::OrdersRule(Invalid::NoTelephoneOfTheHermes) => Problem::NeedsBridge,
            AdminError::OrdersRule(Invalid::TooMany) => Problem::TooMany,
            AdminError::OrdersTelephoneTaken(other) => Problem::Taken(other.clone()),
            AdminError::Restored(_) => Problem::Restored,
            other => {
                tracing::error!(error = %other, "the panel could not record who gives a Hermes orders");
                Problem::Failed
            }
        }
    }
}

/// What the window of a Hermes shows.
#[derive(Debug, Clone, Copy)]
pub struct Window<'a> {
    /// The Hermes.
    pub hermes: &'a str,
    /// What is in the form.
    pub typed: &'a Typed,
    /// The alias of the number of the hub whose telephone is the one typed for the Hermes's
    /// WhatsApp.
    pub bridge_alias: Option<&'a str>,
    /// Why it came back.
    pub problem: Option<&'a Problem>,
}

/// The two fields of the form that carry what was typed, for a window that asks first.
fn carry(html: &mut Html, hermes: &str, typed: &Typed) {
    for (name, value) in [("name", hermes), ("takes", typed.takes.as_str())] {
        html.raw("<input type=\"hidden\" name=\"")
            .raw(name)
            .raw("\" value=\"")
            .text(value)
            .raw("\">");
    }
    html.raw("<input type=\"hidden\" name=\"source\" value=\"")
        .raw(if typed.own { "self" } else { "list" })
        .raw("\"><input type=\"hidden\" name=\"from\" value=\"")
        .text(&typed.from)
        .raw("\">");
}

fn say(html: &mut Html, hermes: &str, problem: &Problem) {
    html.raw("<p class=\"error\" role=\"alert\">");
    match problem {
        Problem::BadPhone => html.t(BAD_PHONE),
        Problem::NeedsBridge => html.fill(NEEDS_BRIDGE, &[("hermes", Arg::Text(hermes))]),
        Problem::TooMany => html.t(TOO_MANY),
        Problem::Taken(other) => html.fill(TAKEN, &[("other", Arg::Text(other))]),
        Problem::Restored => html.t(RESTORED),
        Problem::Failed => html.t(FAILED),
    };
    html.raw("</p>");
}

/// The head of a window: its title and the way out.
fn head(html: &mut Html, id: &'static str, title: impl FnOnce(&mut Html), close_to: &'static str) {
    html.raw("<dialog open class=\"modal\" id=\"guide\" aria-labelledby=\"")
        .raw(id)
        .raw("\"><div class=\"modal-head\"><h2 id=\"")
        .raw(id)
        .raw("\">");
    title(html);
    html.raw("</h2><a class=\"close\" href=\"")
        .raw(close_to)
        .raw("\" aria-label=\"")
        .t(k("Close"))
        .raw("\">")
        .raw(CLOSE)
        .raw("</a></div>");
}

/// The window of a Hermes, over the page of the Hermes.
#[must_use]
pub fn window_page(frame: &Frame, page: &HermesPage, window: &Window<'_>) -> String {
    let mut html = Html::new(frame.lang);
    view::hermes_into(&mut html, frame, page, Over::Guide { refresh: None });
    let hermes = window.hermes;
    let who = [("hermes", Arg::Text(hermes))];
    head(
        &mut html,
        "orders-title",
        |html| {
            html.fill(k("Who gives {hermes} orders on WhatsApp"), &who);
        },
        "/",
    );
    html.raw("<div class=\"modal-body\"><form id=\"orders-form\" class=\"orders-form\" method=\"post\" action=\"/hermes/whatsapp\" autocomplete=\"off\"><input type=\"hidden\" name=\"name\" value=\"")
        .text(hermes)
        .raw("\"><div class=\"field\"><label for=\"orders-takes\">")
        .fill(k("The telephone {hermes}'s WhatsApp runs on"), &who)
        .raw("</label><input id=\"orders-takes\" name=\"takes\" type=\"text\" inputmode=\"tel\" value=\"")
        .text(&window.typed.takes)
        .raw("\"><p class=\"hint\">")
        .fill(
            k("{hermes} takes orders here. Empty if it has no WhatsApp."),
            &who,
        )
        .raw("</p>");
    if let Some(alias) = window.bridge_alias {
        html.raw("<p class=\"hint\">")
            .fill(
                k("This is the number {alias} in Arauto."),
                &[("alias", Arg::Strong(alias))],
            )
            .raw("</p>");
    }
    html.raw("</div><fieldset><legend>")
        .t(k("Orders come from"))
        .raw("</legend><label class=\"option\"><input type=\"radio\" name=\"source\" value=\"self\"")
        .raw(if window.typed.own { " checked" } else { "" })
        .raw("><span>")
        .t(k(
            "Its own WhatsApp, in the chat with itself (self-chat mode)",
        ))
        .raw("</span></label><label class=\"option\"><input type=\"radio\" name=\"source\" value=\"list\"")
        .raw(if window.typed.own { "" } else { " checked" })
        .raw("><span>")
        .t(k("These telephones, one per line"))
        .raw("</span></label><div class=\"field\"><textarea name=\"from\" rows=\"4\" spellcheck=\"false\" aria-label=\"")
        .t(k("These telephones, one per line"))
        .raw("\">\n")
        .text(&window.typed.from)
        .raw("</textarea></div></fieldset></form><p class=\"hint\">")
        .fill(
            k("Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses."),
            &who,
        )
        .raw("</p><p class=\"sentence\">")
        .fill(
            k("If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them."),
            &who,
        )
        .raw("</p>");
    if let Some(problem) = window.problem {
        say(&mut html, hermes, problem);
    }
    html.raw("</div><div class=\"modal-foot\"><button class=\"button primary\" type=\"submit\" form=\"orders-form\">")
        .t(k("Save"))
        .raw("</button><a class=\"button\" href=\"/\">")
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

/// The window that asks first, over the page of the Hermes. Nothing is saved yet: its two
/// buttons are two forms that carry what was typed.
#[must_use]
pub fn risk_page(
    frame: &Frame,
    page: &HermesPage,
    hermes: &str,
    typed: &Typed,
    risk: Risk,
) -> String {
    let mut html = Html::new(frame.lang);
    view::hermes_into(&mut html, frame, page, Over::Guide { refresh: None });
    let who = [("hermes", Arg::Text(hermes))];
    head(
        &mut html,
        "orders-risk-title",
        |html| {
            html.fill(k("A Hermes could give {hermes} orders"), &who);
        },
        "/",
    );
    html.raw("<div class=\"modal-body short\">");
    if risk.nothing_protected {
        html.raw("<p>")
            .fill(
                k("Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them."),
                &who,
            )
            .raw("</p>");
    }
    if risk.removes {
        html.raw("<p>")
            .fill(
                k("Arauto will let every Hermes write again in the chat between each removed telephone and {hermes}'s WhatsApp, and pass there for the person who gives {hermes} orders."),
                &who,
            )
            .raw("</p>");
    }
    html.raw("</div><div class=\"modal-foot\"><form method=\"post\" action=\"/hermes/whatsapp\">");
    carry(&mut html, hermes, typed);
    html.raw("<input type=\"hidden\" name=\"confirmed\" value=\"1\"><button class=\"button danger\" type=\"submit\">")
        .t(if risk.removes {
            k("Remove")
        } else {
            k("Save anyway")
        })
        .raw("</button></form><form method=\"post\" action=\"/hermes/whatsapp\">");
    carry(&mut html, hermes, typed);
    html.raw("<input type=\"hidden\" name=\"back\" value=\"1\"><button class=\"button\" type=\"submit\">")
        .t(k("Go back"))
        .raw("</button></form></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

/// What the chooser lists: the Hermes, to pick one for a number.
#[must_use]
pub fn chooser_page(frame: &Frame, page: &NumbersPage, alias: &str, hermes: &[String]) -> String {
    let mut html = Html::new(frame.lang);
    view::numbers_into(&mut html, frame, page, Over::Guide { refresh: None });
    head(
        &mut html,
        "orders-choose-title",
        |html| {
            html.fill(
                k("Orders on WhatsApp for {alias}"),
                &[("alias", Arg::Text(alias))],
            );
        },
        "/whatsapps",
    );
    html.raw("<div class=\"modal-body\">");
    if hermes.is_empty() {
        html.raw("<p>").t(k("Connect a Hermes first.")).raw("</p>");
    } else {
        html.raw("<p>")
            .t(k(
                "Choose the Hermes. Nothing is saved until you save in its window.",
            ))
            .raw("</p><ul class=\"orders-choices\">");
        for name in hermes {
            html.raw("<li><a class=\"button\" href=\"/hermes/whatsapp?name=")
                .text(name)
                .raw("&amp;gives=")
                .text(alias)
                .raw("\">")
                .fill(
                    k("gives orders to {hermes}"),
                    &[("hermes", Arg::Text(name))],
                )
                .raw("</a><a class=\"button\" href=\"/hermes/whatsapp?name=")
                .text(name)
                .raw("&amp;takes=")
                .text(alias)
                .raw("\">")
                .fill(
                    k("{hermes} takes orders here"),
                    &[("hermes", Arg::Text(name))],
                )
                .raw("</a></li>");
        }
        html.raw("</ul>");
    }
    html.raw("</div><div class=\"modal-foot\"><a class=\"button\" href=\"/whatsapps\">")
        .t(k("Close"))
        .raw("</a></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

struct Cx<'a> {
    hub: &'a Hub,
    organization: ResolvedOrg,
    session: PanelSession,
    lang: Lang,
    admin: admin::Session,
}

impl Cx<'_> {
    /// The id of a Hermes by name, when it exists.
    async fn hermes(&self, name: &HermesName) -> Result<Option<uuid::Uuid>, RepoError> {
        Ok(
            repo::hermes::list(self.hub.pool(), self.organization, false)
                .await?
                .into_iter()
                .find(|line| line.hermes.name == *name)
                .map(|line| line.hermes.id),
        )
    }

    async fn record(&self, hermes: uuid::Uuid) -> Result<OrdersRecord, RepoError> {
        let key = self.hub.organization_key(self.organization).await?;
        repo::orders::get(self.hub.pool(), self.organization, &key, hermes).await
    }

    /// The alias of the number of the hub whose telephone is `text`, when `text` is one.
    async fn alias_of(&self, text: &str) -> Result<Option<String>, RepoError> {
        let Ok(phone) = Phone::parse(text) else {
            return Ok(None);
        };
        let Ok(key) = self.hub.organization_key(self.organization).await else {
            return Ok(None);
        };
        let numbers = repo::number::list(self.hub.pool(), self.organization, false).await?;
        Ok(numbers
            .iter()
            .filter(|number| number.state != NumberState::Removed)
            .find(|number| {
                number
                    .phone(&key)
                    .is_ok_and(|own| phone.forms().contains(&own))
            })
            .map(|number| number.alias.as_str().to_owned()))
    }

    /// The telephone of a number of the hub, as the list shows it (`+digits`), by alias.
    async fn phone_of_alias(&self, alias: &str) -> Result<Option<String>, RepoError> {
        let Ok(alias) = NumberAlias::parse(alias.trim()) else {
            return Ok(None);
        };
        let Some(number) =
            repo::number::by_alias(self.hub.pool(), self.organization, &alias).await?
        else {
            return Ok(None);
        };
        if number.state == NumberState::Removed {
            return Ok(None);
        }
        Ok(data::phone_of(self.hub, self.organization, &number).await)
    }

    async fn window(
        &self,
        status: StatusCode,
        hermes: &str,
        typed: &Typed,
        problem: Option<&Problem>,
    ) -> Result<Response<Body>, RepoError> {
        let frame = data::frame(self.hub, self.organization, self.lang).await?;
        let page = data::hermes(self.hub, self.organization, self.lang).await?;
        let bridge_alias = self.alias_of(&typed.takes).await?;
        Ok(http::page(
            status,
            window_page(
                &frame,
                &page,
                &Window {
                    hermes,
                    typed,
                    bridge_alias: bridge_alias.as_deref(),
                    problem,
                },
            ),
        ))
    }

    async fn risk(
        &self,
        hermes: &str,
        typed: &Typed,
        risk: Risk,
    ) -> Result<Response<Body>, RepoError> {
        let frame = data::frame(self.hub, self.organization, self.lang).await?;
        let page = data::hermes(self.hub, self.organization, self.lang).await?;
        Ok(http::page(
            StatusCode::OK,
            risk_page(&frame, &page, hermes, typed, risk),
        ))
    }
}

/// `GET /hermes/whatsapp`: the window, from the record, with a number's telephone typed in
/// when the address names one. Nothing is saved.
async fn open(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(name) =
        http::query_field(query, "name").and_then(|name| HermesName::parse(name.trim()).ok())
    else {
        return Ok(http::see_other("/"));
    };
    let Some(id) = cx.hermes(&name).await? else {
        return Ok(http::see_other("/"));
    };
    let mut typed = Typed::of(&cx.record(id).await?);
    if let Some(alias) = http::query_field(query, "gives") {
        if let Some(phone) = cx.phone_of_alias(&alias).await? {
            typed.own = false;
            if !typed.from.lines().any(|line| line == phone) {
                if !typed.from.is_empty() {
                    typed.from.push('\n');
                }
                typed.from.push_str(&phone);
            }
        }
    } else if let Some(alias) = http::query_field(query, "takes")
        && let Some(phone) = cx.phone_of_alias(&alias).await?
    {
        typed.takes = phone;
    }
    cx.window(StatusCode::OK, name.as_str(), &typed, None).await
}

/// `GET /whatsapps/orders`: which Hermes, for a number.
async fn choose(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(alias) =
        http::query_field(query, "alias").and_then(|alias| NumberAlias::parse(alias.trim()).ok())
    else {
        return Ok(http::see_other("/whatsapps"));
    };
    match repo::number::by_alias(cx.hub.pool(), cx.organization, &alias).await? {
        Some(number) if number.state != NumberState::Removed => {}
        _ => return Ok(http::see_other("/whatsapps")),
    }
    let hermes = repo::hermes::list(cx.hub.pool(), cx.organization, false)
        .await?
        .into_iter()
        .map(|line| line.hermes.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let frame = data::frame(cx.hub, cx.organization, cx.lang).await?;
    let page = data::numbers(cx.hub, cx.organization, cx.lang, cx.session.id).await?;
    Ok(http::page(
        StatusCode::OK,
        chooser_page(&frame, &page, alias.as_str(), &hermes),
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
    let typed = Typed::of_form(form);
    // "Go back": the window again, with what was typed. Nothing is written.
    if form.field("back").is_some() {
        return cx.window(StatusCode::OK, name.as_str(), &typed, None).await;
    }
    let before = cx.record(id).await?;
    let edit = typed.edit();
    let after = match preview(&before, &edit) {
        Ok(after) => after,
        Err(error) => {
            let problem = Problem::of(&error);
            return cx
                .window(problem.status(), name.as_str(), &typed, Some(&problem))
                .await;
        }
    };
    if let Err(rule) = after.check() {
        let problem = Problem::of(&AdminError::OrdersRule(rule));
        return cx
            .window(problem.status(), name.as_str(), &typed, Some(&problem))
            .await;
    }
    // The question comes first: a save that leaves no chat protected, or that unprotects one,
    // is written only after the answer to it.
    let risk = Risk::of(&before, &after);
    if risk.asks() && form.field("confirmed").is_none() {
        return cx.risk(name.as_str(), &typed, risk).await;
    }
    match cx
        .admin
        .hermes_orders_set(cx.hub.vault(), name.as_str(), &edit, cx.hub.now_ms())
        .await
    {
        Ok(_) | Err(AdminError::NoSuchHermes) => Ok(http::see_other("/")),
        Err(error) => {
            let problem = Problem::of(&error);
            cx.window(problem.status(), name.as_str(), &typed, Some(&problem))
                .await
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

    fn typed() -> Typed {
        Typed {
            takes: HOSTILE.to_owned(),
            own: false,
            from: format!("{HOSTILE}\n+5511900000002"),
        }
    }

    #[test]
    fn what_is_typed_or_named_is_shown_as_text_in_every_window() {
        let frame = Frame::default();
        let page = HermesPage::default();
        let typed = typed();
        for problem in [
            Problem::BadPhone,
            Problem::NeedsBridge,
            Problem::TooMany,
            Problem::Taken(HOSTILE.to_owned()),
            Problem::Restored,
            Problem::Failed,
        ] {
            let shown = window_page(
                &frame,
                &page,
                &Window {
                    hermes: HOSTILE,
                    typed: &typed,
                    bridge_alias: Some(HOSTILE),
                    problem: Some(&problem),
                },
            );
            clean(&shown);
            assert!(shown.contains("class=\"error\""), "{problem:?}");
        }
        for risk in [
            Risk {
                nothing_protected: true,
                removes: false,
            },
            Risk {
                nothing_protected: false,
                removes: true,
            },
        ] {
            clean(&risk_page(&frame, &page, HOSTILE, &typed, risk));
        }
        clean(&chooser_page(
            &frame,
            &NumbersPage::default(),
            HOSTILE,
            &[HOSTILE.to_owned()],
        ));
    }

    #[test]
    fn a_choice_of_the_form_is_the_whole_answer() {
        let own = Typed {
            takes: "+5511900000001".to_owned(),
            own: true,
            from: "+5511900000002".to_owned(),
        }
        .edit();
        assert!(own.self_chat);
        assert_eq!(own.orders_from, None, "the list is not sent with self-chat");
        let listed = Typed {
            takes: String::new(),
            own: false,
            from: " +5511900000002 \n\n+5511900000003\n".to_owned(),
        }
        .edit();
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
}
