//! The WhatsApp numbers in the panel, on WAHA (`docs/07-telas.md`, T4 and T5;
//! `docs/decisoes/0044-numeros-no-painel-sobre-o-motor.md`): the windows that add a number,
//! pair it, decide a swapped phone, disconnect it, remove it, and the placeholder of the
//! native engine.
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /whatsapps/add` | step 1: alias, phone, and for the first number the acceptance |
//! | `POST /whatsapps/add` | `admin number add`, then on to the method |
//! | `GET /whatsapps/pair` | step 2: QR or pairing code |
//! | `POST /whatsapps/pair/start` | starts the pairing in a task of the hub ([`super::pairing`]) |
//! | `GET /whatsapps/pair/wait` | step 3: the window that shows the QR or the code and how it ended |
//! | `POST /whatsapps/pair/cancel` | stops a pairing |
//! | `GET /whatsapps/decide`, `POST /whatsapps/confirm`, `POST /whatsapps/undo` | another phone paired |
//! | `POST /whatsapps/restart` | restarts a failed session |
//! | `GET`/`POST /whatsapps/disconnect`, `/whatsapps/remove` | with the alias typed |
//! | `GET /whatsapps/native` | fixed text: an engine built into Arauto is only being studied |
//!
//! Nothing here decides: every write is the admin function the command calls, with the same
//! rules, the same audit and the same state machine. A `GET` writes nothing, calls nothing on
//! the engine and starts, extends or removes no pairing. Every value of a form is trimmed,
//! parsed by the same function the command uses, and escaped on its way out; every error is
//! one fixed sentence, and the error itself goes to the log.
//!
//! The passkey is not asked again for a disconnect or a removal: no mechanism for that exists
//! yet (decision 0019 is decided and not built; 0040, pending decision 2). What stands in its
//! place is a session that reaches the whole panel, a `POST` from a page of the panel, the
//! alias typed, and an explicit choice for each thing that cannot be undone.

use arauto_protocol::names::{NameError, NumberAlias};
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Method, Response, StatusCode};
use hyper::body::Incoming;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::html::{Arg, Html};
use super::pairing::{End, NotStarted, Now, PAIRINGS_PER_ORGANIZATION, Refusal, Run};
use super::text::{self, Lang, k, tr};
use super::view::{self, Over};
use super::{PanelState, data, http, pairing, session_of};
use crate::admin::qr::QrCode;
use crate::admin::whatsapp::{Changed, PairMethod, PairOutcome, Removal};
use crate::admin::{self, AdminError};
use crate::channel::Hub;
use crate::core::number::NumberState;
use crate::db::ResolvedOrg;
use crate::engine::{Cause, EngineError};
use crate::repo::number::Number;
use crate::repo::panel::PanelSession;
use crate::repo::{self, RepoError};

use std::sync::Arc;

const ENGINE_SET: &str = "arauto-hub admin engine set --url URL --events-url URL";
pub(super) const KEY_SET: &str = "arauto-hub admin engine set-key";
const ENGINE_HEALTH: &str = "arauto-hub admin engine health";

const N27: &str = k(
    "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
);
const N28: &str = k(
    "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
);
const N29: &str = k("The alias has from 1 to 32 characters.");
const N30: &str = k("The alias starts with a lowercase letter or a digit.");
const N_CHARACTER: &str = k("Use only lowercase letters, digits, hyphen and underscore.");
const N31: &str = k("That alias is reserved. Choose another.");
const N32: &str = k(
    "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
);
const N33: &str =
    k("Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.");
const N34: &str = k("That number is already a WhatsApp of this hub.");
const N35: &str =
    k("Read the sentence about the risk and mark that you accept it. Nothing was added.");
const N45: &str = k("Choose QR code or pairing code. Nothing was started.");
const N46: &str = k(
    "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
);
const N47: &str =
    k("This cannot be done while the number is in the state \"{state}\". Nothing was done.");
const N67: &str =
    k("The hub could not reach WAHA. Check that it is running and on the hub's network.");
const N68: &str = k(
    "WAHA refused the key the hub has. If the installation brought the engine, restart the engine: it reads its key only when it starts. If you set the engine up yourself, set the right key on the hub's server with {command}.",
);
const N69: &str = k(
    "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
);
const N70: &str = k("WhatsApp is limiting this number. Try again later.");
const N71: &str = k("WAHA refused the request. See the hub's log.");
const N72: &str =
    k("Something failed in the hub. See the hub's log and check the number in the list.");
const N79: &str = k("Mark that you understand what undoing erases. Nothing was done.");
const N80: &str = k(
    "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
);
const N84: &str = k("The alias typed does not match. Nothing was done.");
const N91: &str = k("Choose what happens to its session in WAHA. Nothing was done.");
const RESTORED: &str = k(
    "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
);

const CLOSE: &str =
    "<svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg>";

/// The names of the steps of the window that adds and pairs a number.
const STEPS: [&str; 4] = [k("Number"), k("Method"), k("Phone"), k("Done")];

const fn refused(sentence: &'static str) -> Refusal {
    Refusal {
        sentence,
        state: None,
    }
}

/// What an error of the admin function or of the engine says on the screen: one fixed
/// sentence. The error itself goes to the log by whoever calls this.
pub(super) fn refusal_of(error: &AdminError) -> Refusal {
    match error {
        AdminError::NoEngine => refused(N27),
        AdminError::Engine(engine) => match engine {
            EngineError::Unavailable(Cause::NotConfigured) => refused(N28),
            EngineError::Unavailable(Cause::Connect | Cause::Timeout) => refused(N67),
            EngineError::Unavailable(Cause::KeyRefused) => refused(N68),
            EngineError::Unavailable(_)
            | EngineError::NotFound
            | EngineError::NumberDisconnected => refused(N69),
            EngineError::WhatsappLimit => refused(N70),
            EngineError::Refused | EngineError::Invalid(_) => refused(N71),
        },
        AdminError::InvalidAlias(NameError::Length) => refused(N29),
        AdminError::InvalidAlias(NameError::FirstCharacter) => refused(N30),
        AdminError::InvalidAlias(NameError::Character) => refused(N_CHARACTER),
        AdminError::InvalidAlias(NameError::Reserved) => refused(N31),
        AdminError::AliasTaken => refused(N32),
        AdminError::InvalidPhone => refused(N33),
        AdminError::PhoneTaken => refused(N34),
        AdminError::NumberNotNow(state) => Refusal {
            sentence: N47,
            state: NumberState::parse(state),
        },
        AdminError::Restored(_) => refused(RESTORED),
        other => {
            tracing::error!(error = %other, "the panel could not do what was asked of a number");
            refused(N72)
        }
    }
}

fn status_of(refusal: &Refusal) -> StatusCode {
    if refusal.sentence == RESTORED {
        StatusCode::CONFLICT
    } else if refusal.sentence == N72 {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    }
}

/// The command a sentence points to, when it points to one.
fn command_of(sentence: &str) -> &'static str {
    if sentence == N27 {
        ENGINE_SET
    } else if sentence == N28 || sentence == N68 {
        KEY_SET
    } else {
        ENGINE_HEALTH
    }
}

fn state_label(lang: Lang, state: Option<NumberState>) -> &'static str {
    state.map_or("", |state| tr(lang, data::number_state(state).1))
}

/// A refusal, as the plain text of the line above the list.
fn plain(lang: Lang, refusal: &Refusal) -> String {
    text::fill(
        lang,
        refusal.sentence,
        &[
            ("state", state_label(lang, refusal.state)),
            ("command", command_of(refusal.sentence)),
            ("key_command", KEY_SET),
            ("n", &PAIRINGS_PER_ORGANIZATION.to_string()),
        ],
    )
}

/// A refusal, in a window: one line that says it.
fn say(html: &mut Html, refusal: &Refusal) {
    let label = state_label(html.lang(), refusal.state);
    html.raw("<p class=\"error\" role=\"alert\">").fill(
        refusal.sentence,
        &[
            ("state", Arg::Text(label)),
            ("command", Arg::Code(command_of(refusal.sentence))),
            ("key_command", Arg::Code(KEY_SET)),
            ("n", Arg::Number(PAIRINGS_PER_ORGANIZATION)),
        ],
    );
    html.raw("</p>");
}

/// What the page of the numbers says about one number, after a write: `?alias=`. Only reads;
/// a name that is not one, or a number that does not exist, says nothing.
pub(super) async fn said(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    query: Option<&str>,
) -> Result<Option<String>, RepoError> {
    let Some(alias) =
        http::query_field(query, "alias").and_then(|alias| NumberAlias::parse(alias.trim()).ok())
    else {
        return Ok(None);
    };
    Ok(repo::number::by_alias(hub.pool(), organization, &alias)
        .await?
        .map(|number| {
            text::fill(
                lang,
                k("Number {alias}: {state}."),
                &[
                    ("alias", number.alias.as_str()),
                    ("state", tr(lang, data::number_state(number.state).1)),
                ],
            )
        }))
}

/// Everything a handler needs about the request.
struct Cx<'a> {
    hub: &'a Arc<Hub>,
    panel: &'a PanelState,
    organization: ResolvedOrg,
    session: PanelSession,
    lang: Lang,
    admin: admin::Session,
}

impl Cx<'_> {
    fn pool(&self) -> &sqlx::PgPool {
        self.hub.pool()
    }

    async fn number(&self, alias: Option<Zeroizing<String>>) -> Result<Option<Number>, RepoError> {
        let Some(alias) = alias.and_then(|alias| NumberAlias::parse(alias.trim()).ok()) else {
            return Ok(None);
        };
        repo::number::by_alias(self.pool(), self.organization, &alias).await
    }

    /// The page of the numbers, with what a write just said above the table.
    async fn list(
        &self,
        status: StatusCode,
        said: Option<String>,
    ) -> Result<Response<Body>, RepoError> {
        self.over(status, Over::Nothing, said, |_| {}).await
    }

    /// The page of the numbers with a window over it.
    async fn window(
        &self,
        status: StatusCode,
        over: Over,
        write: impl FnOnce(&mut Html),
    ) -> Result<Response<Body>, RepoError> {
        self.over(status, over, None, write).await
    }

    async fn over(
        &self,
        status: StatusCode,
        over: Over,
        said: Option<String>,
        write: impl FnOnce(&mut Html),
    ) -> Result<Response<Body>, RepoError> {
        let frame = data::frame(self.hub, self.organization, self.lang).await?;
        let mut list =
            data::numbers(self.hub, self.organization, self.lang, self.session.id).await?;
        list.said = said;
        let mut html = Html::new(self.lang);
        view::numbers_into(&mut html, &frame, &list, over);
        write(&mut html);
        html.raw(view::TAIL);
        Ok(http::page(status, html.finish()))
    }
}

/// The shell of a window: the title, the way to close it, the steps when it has them, the
/// body and the foot.
fn shell(
    html: &mut Html,
    step: Option<usize>,
    title: impl FnOnce(&mut Html),
    body: impl FnOnce(&mut Html),
    foot: impl FnOnce(&mut Html),
) {
    html.raw("<dialog open class=\"modal guide\" id=\"guide\" aria-labelledby=\"guide-title\"><div class=\"modal-head\"><h2 id=\"guide-title\" tabindex=\"-1\">");
    title(html);
    html.raw("</h2><a class=\"close\" href=\"/whatsapps\" aria-label=\"")
        .t(k("Close"))
        .raw("\">")
        .raw(CLOSE)
        .raw("</a></div>");
    if let Some(current) = step {
        html.raw("<ol class=\"path\" aria-label=\"")
            .t(k("Steps"))
            .raw("\">");
        for (index, label) in STEPS.into_iter().enumerate() {
            html.raw(if index < current {
                "<li class=\"done\"><span class=\"dot\">"
            } else if index == current {
                "<li class=\"current\" aria-current=\"step\"><span class=\"dot\">"
            } else {
                "<li><span class=\"dot\">"
            })
            .number(index.saturating_add(1))
            .raw("</span><span class=\"label\">")
            .t(label)
            .raw("</span></li>");
        }
        html.raw("</ol>");
    }
    html.raw("<div class=\"modal-body\">");
    body(html);
    html.raw("</div><div class=\"modal-foot\">");
    foot(html);
    html.raw("</div></dialog>");
}

fn link(
    html: &mut Html,
    class: &'static str,
    href: &'static str,
    alias: &str,
    label: &'static str,
) {
    html.raw("<a class=\"")
        .raw(class)
        .raw("\" href=\"")
        .raw(href)
        .raw("?alias=")
        .text(alias)
        .raw("\">")
        .t(label)
        .raw("</a>");
}

fn close_link(html: &mut Html) {
    html.raw("<a class=\"button\" href=\"/whatsapps\">")
        .t(k("Close"))
        .raw("</a>");
}

fn cancel_link(html: &mut Html) {
    html.raw("<a class=\"button\" href=\"/whatsapps\">")
        .t(k("Cancel"))
        .raw("</a>");
}

fn hidden(html: &mut Html, name: &'static str, value: &str) {
    html.raw("<input type=\"hidden\" name=\"")
        .raw(name)
        .raw("\" value=\"")
        .text(value)
        .raw("\">");
}

/// One choice of a form: a lead in bold when it has one, and what it means.
fn choice(
    html: &mut Html,
    kind: &'static str,
    name: &'static str,
    value: &'static str,
    checked: bool,
    lead: Option<&'static str>,
    label: &'static str,
) {
    html.raw("<label class=\"option\"><input type=\"")
        .raw(kind)
        .raw("\" name=\"")
        .raw(name)
        .raw("\" value=\"")
        .raw(value)
        .raw(if checked {
            "\" checked><span>"
        } else {
            "\"><span>"
        });
    if let Some(lead) = lead {
        html.raw("<b>").t(lead).raw("</b>: ");
    }
    html.t(label).raw("</span></label>");
}

// ------------------------------------------------------------------ step 1: the number

/// What step 1 shows.
struct AddView {
    alias: String,
    phone: String,
    /// The organization never had a number: the acceptance is asked.
    first: bool,
    engine: EngineState,
    error: Option<Refusal>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EngineState {
    /// No engine row: nothing to add a number to.
    None,
    /// An engine with no key.
    NoKey,
    /// An engine with a key; whether it ever answered.
    Ready { answered: bool },
}

fn add_window(html: &mut Html, view: &AddView) {
    shell(
        html,
        Some(0),
        |html| {
            html.t(k("Connect WhatsApp (WAHA)"));
        },
        |html| match view.engine {
            EngineState::None => {
                say(html, &refused(N27));
            }
            EngineState::NoKey => {
                say(html, &refused(N28));
            }
            EngineState::Ready { answered } => {
                html.raw("<p>")
                    .t(k(
                        "The number is paired through WAHA, the engine set on this hub.",
                    ))
                    .raw("</p>");
                if !answered {
                    html.raw("<p class=\"hint\">")
                        .t(k("WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer."))
                        .raw("</p>");
                }
                html.raw("<form id=\"add-form\" method=\"post\" action=\"/whatsapps/add\" autocomplete=\"off\"><div class=\"field\"><label for=\"add-alias\">")
                    .t(k("Alias"))
                    .raw("</label><input id=\"add-alias\" name=\"alias\" type=\"text\" maxlength=\"32\" spellcheck=\"false\" autocapitalize=\"none\" required autofocus value=\"")
                    .text(&view.alias)
                    .raw("\"><p class=\"hint\">")
                    .t(k("How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused."))
                    .raw("</p></div><div class=\"field\"><label for=\"add-phone\">")
                    .t(k("Number with the country code"))
                    .raw("</label><input id=\"add-phone\" name=\"phone\" type=\"text\" inputmode=\"tel\" maxlength=\"32\" required value=\"")
                    .text(&view.phone)
                    .raw("\"><p class=\"hint\">")
                    .t(k("Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only."))
                    .raw("</p></div>");
                if view.first {
                    html.raw("<fieldset><legend>")
                        .t(k("Before the first number"))
                        .raw("</legend><p>")
                        .t(k("The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose."))
                        .raw("</p><label class=\"option\"><input type=\"checkbox\" name=\"accept\" value=\"on\" required><span>")
                        .t(k("I understand and accept. The hub records this acceptance in the audit."))
                        .raw("</span></label></fieldset>");
                }
                html.raw("</form>");
                if let Some(error) = &view.error {
                    say(html, error);
                }
            }
        },
        |html| {
            if matches!(view.engine, EngineState::Ready { .. }) {
                html.raw("<button class=\"button primary\" type=\"submit\" form=\"add-form\">")
                    .t(k("Add and pair"))
                    .raw("</button>");
                cancel_link(html);
            } else {
                close_link(html);
            }
        },
    );
}

async fn add_view(cx: &Cx<'_>, typed: Option<(String, String)>) -> Result<AddView, RepoError> {
    let engine = match repo::engine::get(cx.pool(), cx.organization).await? {
        None => EngineState::None,
        Some(row) if row.api_key_enc.is_none() => EngineState::NoKey,
        Some(row) => EngineState::Ready {
            answered: row.last_ok_at.is_some(),
        },
    };
    let first = repo::number::list(cx.pool(), cx.organization, true)
        .await?
        .is_empty();
    let (alias, phone) = typed.unwrap_or_default();
    Ok(AddView {
        alias,
        phone,
        first,
        engine,
        error: None,
    })
}

async fn add_page(cx: &Cx<'_>, view: AddView) -> Result<Response<Body>, RepoError> {
    let status = view.error.as_ref().map_or(StatusCode::OK, status_of);
    cx.window(status, Over::Guide { refresh: None }, |html| {
        add_window(html, &view);
    })
    .await
}

async fn add(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let field = |name: &str| {
        form.field(name)
            .map(|value| value.trim().to_owned())
            .unwrap_or_default()
    };
    let (alias, phone) = (field("alias"), field("phone"));
    let accept = form.field("accept").is_some();
    let mut view = add_view(cx, Some((alias.clone(), phone.clone()))).await?;
    if !matches!(view.engine, EngineState::Ready { .. }) {
        return add_page(cx, view).await;
    }
    if view.first && !accept {
        view.error = Some(refused(N35));
        return add_page(cx, view).await;
    }
    let added = cx
        .admin
        .number_add_accepting(
            cx.hub.vault(),
            &alias,
            &phone,
            view.first && accept,
            cx.hub.now_ms(),
        )
        .await;
    match added {
        Ok(added) => Ok(http::see_other_to(&format!(
            "/whatsapps/pair?alias={}",
            added.number.alias.as_str()
        ))),
        Err(error) => {
            view.error = Some(refusal_of(&error));
            add_page(cx, view).await
        }
    }
}

// ------------------------------------------------------------------ step 2: the method

fn method_window(
    html: &mut Html,
    number: &Number,
    phone: &str,
    minutes: usize,
    error: Option<&Refusal>,
    running: Option<&Run>,
) {
    let alias = number.alias.as_str();
    shell(
        html,
        Some(1),
        |html| {
            html.fill(k("Pair {alias}"), &[("alias", Arg::Text(alias))]);
        },
        |html| {
            if let Some(run) = running {
                html.raw("<p>")
                    .t(k("This number is being paired in another window. Only that window shows the QR or the code."))
                    .raw("</p><form method=\"post\" action=\"/whatsapps/pair/cancel\">");
                hidden(html, "alias", alias);
                hidden(html, "run", &run.id.to_string());
                html.raw("<div><button class=\"button danger\" type=\"submit\">")
                    .t(k("Cancel that pairing"))
                    .raw("</button></div></form>");
            } else if let Some(error) = error.filter(|e| e.sentence == N47) {
                say(html, error);
            } else {
                html.raw("<p>")
                    .fill(
                        k("Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes."),
                        &[("phone", Arg::Strong(phone)), ("minutes", Arg::Number(minutes))],
                    )
                    .raw("</p>");
                if number.config_digest.is_none() {
                    html.raw("<p class=\"hint\">")
                        .t(k("WAHA has not created the session of this number yet. Starting the pairing tries again."))
                        .raw("</p>");
                }
                html.raw(
                    "<form id=\"method-form\" method=\"post\" action=\"/whatsapps/pair/start\">",
                );
                hidden(html, "alias", alias);
                html.raw("<fieldset><legend>")
                    .t(k("How will the phone pair?"))
                    .raw("</legend>");
                choice(
                    html,
                    "radio",
                    "method",
                    "qr",
                    true,
                    Some(k("QR code")),
                    k("scan it with the phone's camera, from inside WhatsApp"),
                );
                choice(
                    html,
                    "radio",
                    "method",
                    "code",
                    false,
                    Some(k("Pairing code")),
                    k("type a code on the phone, for when it cannot scan this screen"),
                );
                html.raw("</fieldset></form>");
                if let Some(error) = error {
                    say(html, error);
                }
            }
        },
        |html| {
            if running.is_none() && error.is_none_or(|e| e.sentence != N47) {
                html.raw("<button class=\"button primary\" type=\"submit\" form=\"method-form\">")
                    .t(k("Start pairing"))
                    .raw("</button>");
                cancel_link(html);
            } else {
                close_link(html);
            }
        },
    );
}

/// Whether a number in this state may be paired now (`docs/10-maquinas-de-estado.md`).
const fn may_pair(state: NumberState) -> bool {
    matches!(
        state,
        NumberState::Unpaired | NumberState::Disconnected | NumberState::Pairing
    )
}

async fn method_page(
    cx: &Cx<'_>,
    number: &Number,
    error: Option<Refusal>,
    running: Option<&Run>,
) -> Result<Response<Body>, RepoError> {
    let phone = data::phone_of(cx.hub, cx.organization, number)
        .await
        .unwrap_or_else(|| "—".to_owned());
    let minutes = usize::try_from(cx.hub.engines().settings().pairing_ms / 60_000).unwrap_or(0);
    cx.window(StatusCode::OK, Over::Guide { refresh: None }, |html| {
        method_window(html, number, &phone, minutes, error.as_ref(), running);
    })
    .await
}

async fn method(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(http::query_field(query, "alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    if number.state == NumberState::NumberSwapped {
        return Ok(http::see_other_to(&format!(
            "/whatsapps/decide?alias={}",
            number.alias.as_str()
        )));
    }
    let kept = cx
        .panel
        .pairings
        .of(cx.organization, number.id)
        .filter(|run| run.live());
    if let Some(run) = kept {
        return if run.session == cx.session.id {
            Ok(http::see_other_to(&wait_url(&number, &run)))
        } else {
            method_page(cx, &number, None, Some(&run)).await
        };
    }
    if !may_pair(number.state) {
        let error = Refusal {
            sentence: N47,
            state: Some(number.state),
        };
        return method_page(cx, &number, Some(error), None).await;
    }
    method_page(cx, &number, None, None).await
}

fn wait_url(number: &Number, run: &Run) -> String {
    format!(
        "/whatsapps/pair/wait?alias={}&run={}",
        number.alias.as_str(),
        run.id
    )
}

async fn start(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    let method = match form.field("method").as_deref().map(String::as_str) {
        Some("qr") => PairMethod::Qr,
        Some("code") => PairMethod::Code,
        _ => {
            return method_page(cx, &number, Some(refused(N45)), None).await;
        }
    };
    if !may_pair(number.state) {
        let error = Refusal {
            sentence: N47,
            state: Some(number.state),
        };
        return method_page(cx, &number, Some(error), None).await;
    }
    let started = pairing::start(
        cx.hub,
        cx.organization,
        cx.session.admin,
        cx.session.id,
        number.id,
        &number.alias,
        method,
    )
    .await;
    match started {
        Ok(run) => Ok(http::see_other_to(&wait_url(&number, &run))),
        // The same window again (a double click): it goes where the first one went.
        Err(NotStarted::Running(run)) if run.session == cx.session.id => {
            Ok(http::see_other_to(&wait_url(&number, &run)))
        }
        Err(NotStarted::Running(run)) => method_page(cx, &number, None, Some(&run)).await,
        Err(NotStarted::Full) => method_page(cx, &number, Some(refused(N46)), None).await,
    }
}

// ------------------------------------------------------------------ step 3: the wait

/// What a run shows, copied out of its lock so the page is written with nothing held. The
/// QR is drawn into a code that wipes itself; the pairing code is a copy that wipes itself.
enum Snap {
    Starting,
    Qr {
        drawn: Option<QrCode>,
        serial: u32,
        same: bool,
    },
    Code {
        code: Zeroizing<String>,
        serial: u32,
        same: bool,
    },
    Ended(End),
}

fn snap(run: &Run, shown: Option<u32>) -> Snap {
    run.with_now(|now| match now {
        Now::Starting => Snap::Starting,
        Now::Qr { qr, serial } => {
            let same = shown == Some(*serial);
            Snap::Qr {
                drawn: if same {
                    None
                } else {
                    QrCode::encode(qr.expose().as_bytes()).ok()
                },
                serial: *serial,
                same,
            }
        }
        Now::Code { code, serial } => {
            let same = shown == Some(*serial);
            Snap::Code {
                code: Zeroizing::new(if same {
                    String::new()
                } else {
                    code.expose().to_owned()
                }),
                serial: *serial,
                same,
            }
        }
        Now::Ended(end, _) => Snap::Ended(*end),
    })
}

/// What a pairing that ended says, and where it leads.
fn end_window(html: &mut Html, number: &Number, phone: &str, end: &End) {
    let alias = number.alias.as_str();
    let retry = |html: &mut Html, label: &'static str| {
        link(html, "button primary", "/whatsapps/pair", alias, label);
        close_link(html);
    };
    shell(
        html,
        Some(3),
        |html| {
            html.fill(k("Pair {alias}"), &[("alias", Arg::Text(alias))]);
        },
        |html| {
            html.raw("<div id=\"pair-live\"><div id=\"pair-state\" role=\"status\" data-ended>");
            match end {
                End::Outcome(PairOutcome::Connected) => {
                    html.raw("<p><b>")
                        .fill(k("Paired: {phone}."), &[("phone", Arg::Text(phone))])
                        .raw("</b></p><p>")
                        .fill(
                            k("The number {alias} is connected. Give a Hermes access to it in the Accesses tab."),
                            &[("alias", Arg::Text(alias))],
                        )
                        .raw("</p>");
                }
                End::Outcome(PairOutcome::Swapped) => {
                    html.raw("<p>")
                        .t(k("The phone that paired belongs to a number different from the one given."))
                        .raw("</p><p>")
                        .t(k("Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped."))
                        .raw("</p>");
                }
                End::Outcome(PairOutcome::Expired) => {
                    html.raw("<p>")
                        .t(k("Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand."))
                        .raw("</p>");
                }
                End::Outcome(PairOutcome::Failed) => {
                    html.raw("<p>")
                        .t(k("WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA."))
                        .raw("</p>");
                }
                End::Outcome(PairOutcome::Cancelled) => {
                    html.raw("<p>")
                        .t(k("The pairing was cancelled. The number is not paired."))
                        .raw("</p>");
                }
                End::Refused(refusal) => {
                    html.raw("</div>");
                    say(html, refusal);
                    html.raw("<div>");
                }
            }
            html.raw("</div></div>");
        },
        |html| match end {
            End::Outcome(PairOutcome::Connected) => {
                html.raw("<a class=\"button\" href=\"/accesses\">")
                    .t(k("See accesses"))
                    .raw("</a><a class=\"button primary\" href=\"/whatsapps\">")
                    .t(k("Finish"))
                    .raw("</a>");
            }
            End::Outcome(PairOutcome::Swapped) => {
                html.raw("<form method=\"post\" action=\"/whatsapps/confirm\">");
                hidden(html, "alias", alias);
                html.raw("<button class=\"button primary\" type=\"submit\">")
                    .t(k("Keep this one"))
                    .raw("</button></form>");
                link(html, "button", "/whatsapps/decide", alias, k("Decide"));
            }
            End::Outcome(PairOutcome::Expired | PairOutcome::Failed | PairOutcome::Cancelled) => {
                retry(html, k("Pair again"));
            }
            End::Refused(_) => retry(html, k("Try again")),
        },
    );
}

/// The window of a pairing that runs: the QR or the code in a region the script renews, the
/// state in another, and how to cancel or refresh.
fn running_window(html: &mut Html, number: &Number, run: &Run, snap: &Snap, left_s: i64) {
    let alias = number.alias.as_str();
    let serial = match snap {
        Snap::Qr { serial, .. } | Snap::Code { serial, .. } => *serial,
        _ => 0,
    };
    shell(
        html,
        Some(2),
        |html| {
            html.fill(k("Pair {alias}"), &[("alias", Arg::Text(alias))]);
        },
        |html| {
            html.raw("<div id=\"pair-live\"><div id=\"pair-qr\" data-serial=\"")
                .number(usize::try_from(serial).unwrap_or(0));
            match snap {
                Snap::Starting => {
                    html.raw("\"></div><p id=\"pair-state\" role=\"status\">")
                        .t(k("Starting the pairing…"))
                        .raw("</p>");
                }
                Snap::Qr { same: true, .. } | Snap::Code { same: true, .. } => {
                    html.raw("\" data-same></div><p id=\"pair-state\" role=\"status\"></p>");
                }
                Snap::Qr { drawn, .. } => {
                    html.raw("\"><p>")
                        .t(k(
                            "Scan with the phone: WhatsApp › Linked devices › Link a device.",
                        ))
                        .raw("</p>");
                    match drawn {
                        Some(code) => {
                            html.qr(code, k("QR code to scan with the phone"));
                            html.raw("<p class=\"hint\">")
                                .t(k("The QR changes every few seconds: this window shows the current one."))
                                .raw("</p>");
                        }
                        None => {
                            html.raw("<p class=\"error\" role=\"alert\">")
                                .t(k("The QR is too long to draw here. Cancel, and pair with a pairing code instead."))
                                .raw("</p>");
                        }
                    }
                    html.raw("</div><p id=\"pair-state\" role=\"status\"></p>");
                }
                Snap::Code { code, .. } => {
                    html.raw("\"><p>")
                        .t(k("On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:"))
                        .raw("</p><p class=\"pair-code mono\">")
                        .text(code)
                        .raw("</p></div><p id=\"pair-state\" role=\"status\"></p>");
                }
                Snap::Ended(_) => {
                    html.raw("\"></div>");
                }
            }
            html.raw("</div>");
            if matches!(snap, Snap::Code { .. }) {
                html.raw("<p class=\"hint\">")
                    .fill(
                        k("The code works while this window counts down: {time} left. WhatsApp may end it sooner."),
                        &[("time", Arg::Clock(left_s))],
                    )
                    .raw("</p>");
            }
        },
        |html| {
            if matches!(snap, Snap::Qr { .. }) {
                html.raw("<span>")
                    .fill(
                        k("Time left to scan: {time}."),
                        &[("time", Arg::Clock(left_s))],
                    )
                    .raw("</span>");
            }
            html.raw("<span>")
                .t(k("This window refreshes by itself."))
                .raw("</span><a class=\"button\" href=\"")
                .text(&wait_url(number, run))
                .raw("\">")
                .t(k("Refresh now"))
                .raw("</a><form method=\"post\" action=\"/whatsapps/pair/cancel\">");
            hidden(html, "alias", alias);
            hidden(html, "run", &run.id.to_string());
            html.raw("<button class=\"button danger\" type=\"submit\">")
                .t(k("Cancel the pairing"))
                .raw("</button></form>");
        },
    );
}

/// The window of a pairing, for the session that started it.
async fn own_wait(
    cx: &Cx<'_>,
    number: &Number,
    run: &Arc<Run>,
    shown: Option<u32>,
) -> Result<Response<Body>, RepoError> {
    let snapshot = snap(run, shown);
    let left_s = number
        .pairing_until
        .map_or(0, |until| until.saturating_sub(cx.hub.now_ms()) / 1_000);
    if let Snap::Ended(end) = &snapshot {
        let phone = data::phone_of(cx.hub, cx.organization, number)
            .await
            .unwrap_or_else(|| "—".to_owned());
        return cx
            .window(StatusCode::OK, Over::Guide { refresh: None }, |html| {
                end_window(html, number, &phone, end);
            })
            .await;
    }
    cx.window(StatusCode::OK, Over::Pairing, |html| {
        running_window(html, number, run, &snapshot, left_s);
    })
    .await
}

/// A pairing nobody in this window started, or that left no run: what the number says.
async fn derived_wait(cx: &Cx<'_>, number: &Number) -> Result<Response<Body>, RepoError> {
    match number.state {
        NumberState::Connected => {
            let phone = data::phone_of(cx.hub, cx.organization, number)
                .await
                .unwrap_or_else(|| "—".to_owned());
            cx.window(StatusCode::OK, Over::Guide { refresh: None }, |html| {
                end_window(html, number, &phone, &End::Outcome(PairOutcome::Connected));
            })
            .await
        }
        NumberState::NumberSwapped => Ok(http::see_other_to(&format!(
            "/whatsapps/decide?alias={}",
            number.alias.as_str()
        ))),
        state => {
            let alias = number.alias.as_str();
            cx.window(StatusCode::OK, Over::Guide { refresh: None }, |html| {
                shell(
                    html,
                    Some(3),
                    |html| {
                        html.fill(k("Pair {alias}"), &[("alias", Arg::Text(alias))]);
                    },
                    |html| {
                        let label = state_label(html.lang(), Some(state));
                        html.raw("<p>")
                            .fill(
                                k("This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\"."),
                                &[("state", Arg::Text(label))],
                            )
                            .raw("</p>");
                    },
                    |html| {
                        if may_pair(state) {
                            link(html, "button primary", "/whatsapps/pair", alias, k("Pair again"));
                        }
                        close_link(html);
                    },
                );
            })
            .await
        }
    }
}

async fn wait(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(http::query_field(query, "alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    let asked = http::query_field(query, "run").and_then(|run| Uuid::parse_str(run.trim()).ok());
    let shown =
        http::query_field(query, "shown").and_then(|serial| serial.trim().parse::<u32>().ok());
    match cx.panel.pairings.of(cx.organization, number.id) {
        Some(run) if Some(run.id) == asked && run.session == cx.session.id => {
            own_wait(cx, &number, &run, shown).await
        }
        Some(run) if run.live() => method_page(cx, &number, None, Some(&run)).await,
        _ => derived_wait(cx, &number).await,
    }
}

async fn cancel(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    let back = http::see_other_to(&format!("/whatsapps?alias={}", number.alias.as_str()));
    let run = form
        .field("run")
        .and_then(|run| Uuid::parse_str(run.trim()).ok())
        .and_then(|run| cx.panel.pairings.by_id(cx.organization, number.id, run));
    if let Some(run) = run {
        run.cancel().await;
    }
    Ok(back)
}

// ------------------------------------------------------------------ decide, restart

fn decide_window(html: &mut Html, number: &Number, error: Option<&Refusal>) {
    let alias = number.alias.as_str();
    shell(
        html,
        None,
        |html| {
            html.fill(
                k("{alias}: another phone paired"),
                &[("alias", Arg::Text(alias))],
            );
        },
        |html| {
            html.raw("<p>")
                .t(k("The phone that paired belongs to a number different from the one given."))
                .raw("</p><p>")
                .t(k("Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped."))
                .raw("</p><form method=\"post\" action=\"/whatsapps/confirm\">");
            hidden(html, "alias", alias);
            html.raw("<div><button class=\"button primary\" type=\"submit\">")
                .t(k("Keep this one"))
                .raw("</button></div></form><form method=\"post\" action=\"/whatsapps/undo\"><p>")
                .t(k("Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number."))
                .raw("</p>");
            hidden(html, "alias", alias);
            html.raw("<label class=\"option\"><input type=\"checkbox\" name=\"erase\" value=\"on\" required><span>")
                .t(k("I understand that WAHA erases its history of this number."))
                .raw("</span></label><div><button class=\"button danger\" type=\"submit\">")
                .t(k("Undo"))
                .raw("</button></div></form>");
            if let Some(error) = error {
                say(html, error);
            }
        },
        cancel_link,
    );
}

async fn decide_page(
    cx: &Cx<'_>,
    number: &Number,
    error: Option<Refusal>,
) -> Result<Response<Body>, RepoError> {
    cx.window(StatusCode::OK, Over::Guide { refresh: None }, |html| {
        decide_window(html, number, error.as_ref());
    })
    .await
}

async fn decide(cx: &Cx<'_>, query: Option<&str>) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(http::query_field(query, "alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    if number.state != NumberState::NumberSwapped {
        return Ok(http::see_other_to(&format!(
            "/whatsapps?alias={}",
            number.alias.as_str()
        )));
    }
    decide_page(cx, &number, None).await
}

/// After a change that also went to the engine: the list, and when the engine did not do its
/// part, the line that says so.
async fn changed(cx: &Cx<'_>, done: &Changed) -> Result<Response<Body>, RepoError> {
    if let Err(error) = &done.engine {
        tracing::warn!(%error, "the engine did not do its part of a change of a number");
        return cx
            .list(
                StatusCode::OK,
                Some(text::fill(
                    cx.lang,
                    k("The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server."),
                    &[
                        ("alias", done.number.alias.as_str()),
                        ("command", ENGINE_HEALTH),
                    ],
                )),
            )
            .await;
    }
    Ok(http::see_other_to(&format!(
        "/whatsapps?alias={}",
        done.number.alias.as_str()
    )))
}

async fn confirm(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    match cx
        .admin
        .number_confirm(number.alias.as_str(), cx.hub.now_ms())
        .await
    {
        Ok(number) => Ok(http::see_other_to(&format!(
            "/whatsapps?alias={}",
            number.alias.as_str()
        ))),
        Err(AdminError::NoSuchNumber) => Ok(http::see_other("/whatsapps")),
        Err(AdminError::PhoneTaken) => decide_page(cx, &number, Some(refused(N80))).await,
        Err(error) => decide_page(cx, &number, Some(refusal_of(&error))).await,
    }
}

async fn undo(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    if form.field("erase").is_none() {
        return decide_page(cx, &number, Some(refused(N79))).await;
    }
    match cx
        .admin
        .number_unpair(cx.hub.vault(), number.alias.as_str(), cx.hub.now_ms())
        .await
    {
        Ok(done) => changed(cx, &done).await,
        Err(AdminError::NoSuchNumber) => Ok(http::see_other("/whatsapps")),
        Err(error) => decide_page(cx, &number, Some(refusal_of(&error))).await,
    }
}

async fn restart(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    match cx
        .admin
        .number_restart(cx.hub.vault(), number.alias.as_str(), cx.hub.now_ms())
        .await
    {
        Ok(done) => changed(cx, &done).await,
        Err(AdminError::NoSuchNumber) => Ok(http::see_other("/whatsapps")),
        Err(error) => {
            let refusal = refusal_of(&error);
            cx.list(status_of(&refusal), Some(plain(cx.lang, &refusal)))
                .await
        }
    }
}

// ------------------------------------------------------------------ disconnect, remove

/// The field that asks for the alias typed.
fn typed_field(html: &mut Html, alias: &str) {
    html.raw("<div class=\"field\"><label for=\"typed\">")
        .fill(
            k("Type the alias {alias} to confirm."),
            &[("alias", Arg::Strong(alias))],
        )
        .raw("</label><input id=\"typed\" name=\"typed\" type=\"text\" maxlength=\"64\" autocomplete=\"off\" spellcheck=\"false\" autocapitalize=\"none\" required></div>");
}

fn disconnect_window(html: &mut Html, number: &Number, error: Option<&Refusal>) {
    let alias = number.alias.as_str();
    shell(
        html,
        None,
        |html| {
            html.fill(k("Disconnect {alias}"), &[("alias", Arg::Text(alias))]);
        },
        |html| {
            html.raw("<p>")
                .t(k("Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again."))
                .raw("</p><form id=\"typed-form\" method=\"post\" action=\"/whatsapps/disconnect\" autocomplete=\"off\">");
            hidden(html, "alias", alias);
            typed_field(html, alias);
            html.raw("</form>");
            if let Some(error) = error {
                say(html, error);
            }
        },
        |html| {
            html.raw("<button class=\"button danger\" type=\"submit\" form=\"typed-form\">")
                .t(k("Disconnect"))
                .raw("</button>");
            cancel_link(html);
        },
    );
}

fn remove_window(html: &mut Html, number: &Number, error: Option<&Refusal>) {
    let alias = number.alias.as_str();
    shell(
        html,
        None,
        |html| {
            html.fill(k("Remove {alias}"), &[("alias", Arg::Text(alias))]);
        },
        |html| {
            html.raw("<p>")
                .t(k("Removing ends every access to this number now. Its alias is never used again."))
                .raw("</p><form id=\"typed-form\" method=\"post\" action=\"/whatsapps/remove\" autocomplete=\"off\">");
            hidden(html, "alias", alias);
            html.raw("<fieldset><legend>")
                .t(k("What happens to its session in WAHA"))
                .raw("</legend>");
            // None is checked: the choice is made, never assumed.
            choice(
                html,
                "radio",
                "removal",
                "keep",
                false,
                None,
                k("Stop it and keep it in WAHA, with the history WAHA keeps"),
            );
            choice(
                html,
                "radio",
                "removal",
                "erase",
                false,
                None,
                k("Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps"),
            );
            html.raw("<p class=\"hint\">")
                .t(k(
                    "Arauto has no message archive yet: the history is only what WAHA keeps.",
                ))
                .raw("</p></fieldset>");
            typed_field(html, alias);
            html.raw("</form>");
            if let Some(error) = error {
                say(html, error);
            }
        },
        |html| {
            html.raw("<button class=\"button danger\" type=\"submit\" form=\"typed-form\">")
                .t(k("Remove"))
                .raw("</button>");
            cancel_link(html);
        },
    );
}

async fn typed_window(
    cx: &Cx<'_>,
    number: &Number,
    removing: bool,
    error: Option<Refusal>,
) -> Result<Response<Body>, RepoError> {
    cx.window(StatusCode::OK, Over::Guide { refresh: None }, |html| {
        if removing {
            remove_window(html, number, error.as_ref());
        } else {
            disconnect_window(html, number, error.as_ref());
        }
    })
    .await
}

async fn typed_get(
    cx: &Cx<'_>,
    query: Option<&str>,
    removing: bool,
) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(http::query_field(query, "alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    typed_window(cx, &number, removing, None).await
}

fn typed_matches(form: &http::Form, number: &Number) -> bool {
    form.field("typed")
        .is_some_and(|typed| typed.trim() == number.alias.as_str())
}

async fn disconnect(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    if !typed_matches(form, &number) {
        return typed_window(cx, &number, false, Some(refused(N84))).await;
    }
    match cx
        .admin
        .number_disconnect(cx.hub.vault(), number.alias.as_str(), cx.hub.now_ms())
        .await
    {
        Ok(done) => changed(cx, &done).await,
        Err(AdminError::NoSuchNumber) => Ok(http::see_other("/whatsapps")),
        Err(error) => typed_window(cx, &number, false, Some(refusal_of(&error))).await,
    }
}

async fn remove(cx: &Cx<'_>, form: &http::Form) -> Result<Response<Body>, RepoError> {
    let Some(number) = cx.number(form.field("alias")).await? else {
        return Ok(http::see_other("/whatsapps"));
    };
    if !typed_matches(form, &number) {
        return typed_window(cx, &number, true, Some(refused(N84))).await;
    }
    let removal = match form.field("removal").as_deref().map(String::as_str) {
        Some("keep") => Removal::KeepInEngine,
        Some("erase") => Removal::EraseEngineHistory,
        _ => return typed_window(cx, &number, true, Some(refused(N91))).await,
    };
    match cx
        .admin
        .number_remove(
            cx.hub.vault(),
            number.alias.as_str(),
            removal,
            cx.hub.now_ms(),
        )
        .await
    {
        Ok(done) if done.engine.is_ok() => Ok(http::see_other("/whatsapps")),
        Ok(done) => changed(cx, &done).await,
        Err(AdminError::NoSuchNumber) => Ok(http::see_other("/whatsapps")),
        Err(error) => typed_window(cx, &number, true, Some(refusal_of(&error))).await,
    }
}

// ------------------------------------------------------------------ the native engine

/// The window of the second button: fixed text, no form, no action. An engine built into
/// Arauto is not decided and not part of this version.
fn native_window(html: &mut Html) {
    shell(
        html,
        None,
        |html| {
            html.t(k("Connect WhatsApp (native · in test)"))
                .raw("<span class=\"tag trial\">")
                .t(k("In test"))
                .raw("</span>");
        },
        |html| {
            html.raw("<p>")
                .t(k("A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing."))
                .raw("</p><p>")
                .t(k("For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button."))
                .raw("</p>");
        },
        |html| {
            html.raw("<a class=\"button primary\" href=\"/whatsapps/add\">")
                .t(k("Open the WAHA window"))
                .raw("</a>");
            close_link(html);
        },
    );
}

// ------------------------------------------------------------------ the router

/// Answers a request for an address under `/whatsapps/`. The request already named the panel
/// and, when it writes, came from a page of the panel.
pub(super) async fn answer(
    hub: &Arc<Hub>,
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
    let query = parts.uri.query();
    let path = parts.uri.path();
    if parts.method == Method::GET {
        return match path {
            "/whatsapps/add" => {
                let view = add_view(&cx, None).await?;
                add_page(&cx, view).await
            }
            "/whatsapps/pair" => method(&cx, query).await,
            "/whatsapps/pair/wait" => wait(&cx, query).await,
            "/whatsapps/decide" => decide(&cx, query).await,
            "/whatsapps/disconnect" => typed_get(&cx, query, false).await,
            "/whatsapps/remove" => typed_get(&cx, query, true).await,
            "/whatsapps/native" => {
                cx.window(StatusCode::OK, Over::Guide { refresh: None }, native_window)
                    .await
            }
            _ => Ok(http::plain(StatusCode::NOT_FOUND, "not found\n")),
        };
    }
    if !matches!(
        path,
        "/whatsapps/add"
            | "/whatsapps/pair/start"
            | "/whatsapps/pair/cancel"
            | "/whatsapps/confirm"
            | "/whatsapps/undo"
            | "/whatsapps/restart"
            | "/whatsapps/disconnect"
            | "/whatsapps/remove"
    ) {
        return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n"));
    }
    let form = match http::form(&parts.headers, body).await {
        Ok(form) => form,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    match path {
        "/whatsapps/add" => add(&cx, &form).await,
        "/whatsapps/pair/start" => start(&cx, &form).await,
        "/whatsapps/pair/cancel" => cancel(&cx, &form).await,
        "/whatsapps/confirm" => confirm(&cx, &form).await,
        "/whatsapps/undo" => undo(&cx, &form).await,
        "/whatsapps/restart" => restart(&cx, &form).await,
        "/whatsapps/disconnect" => disconnect(&cx, &form).await,
        _ => remove(&cx, &form).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    fn clean(page: &str) {
        assert!(!page.contains("<script>alert"), "{page}");
        assert!(!page.contains("<img src=x"), "{page}");
        assert!(!page.contains("\"'>"), "{page}");
    }

    #[test]
    fn the_step_one_window_shows_what_was_typed_only_as_text() {
        let view = AddView {
            alias: HOSTILE.to_owned(),
            phone: HOSTILE.to_owned(),
            first: true,
            engine: EngineState::Ready { answered: false },
            error: Some(refused(N33)),
        };
        let mut html = Html::new(Lang::En);
        add_window(&mut html, &view);
        let page = html.finish();
        clean(&page);
        assert!(page.contains("name=\"accept\""));
        assert!(!page.contains("name=\"command\""));
        // With no engine, no form at all, and the command that sets it.
        let mut html = Html::new(Lang::En);
        add_window(
            &mut html,
            &AddView {
                engine: EngineState::None,
                first: false,
                ..view
            },
        );
        let page = html.finish();
        assert!(!page.contains("<form"), "{page}");
        assert!(page.contains("arauto-hub admin engine set --url URL --events-url URL"));
        assert!(page.contains("arauto-hub admin engine set-key"));
    }

    #[test]
    fn the_native_window_has_no_form_no_script_and_says_it_is_in_test() {
        let mut html = Html::new(Lang::En);
        native_window(&mut html);
        let page = html.finish();
        assert!(!page.contains("<form") && !page.contains("method=\"post\""));
        assert!(!page.contains("data-start") && !page.contains("<script"));
        assert!(page.contains("In test") && page.contains("class=\"tag trial\""));
        assert!(page.contains("href=\"/whatsapps/add\""));
    }

    #[test]
    fn each_error_of_the_engine_has_its_sentence_and_the_rest_go_to_the_log() {
        let engine = |error| refusal_of(&AdminError::Engine(error)).sentence;
        assert_eq!(engine(EngineError::Unavailable(Cause::NotConfigured)), N28);
        assert_eq!(engine(EngineError::Unavailable(Cause::Connect)), N67);
        assert_eq!(engine(EngineError::Unavailable(Cause::Timeout)), N67);
        assert_eq!(engine(EngineError::Unavailable(Cause::KeyRefused)), N68);
        assert_eq!(engine(EngineError::Unavailable(Cause::Status(500))), N69);
        assert_eq!(engine(EngineError::Unavailable(Cause::Malformed)), N69);
        assert_eq!(engine(EngineError::NotFound), N69);
        assert_eq!(engine(EngineError::WhatsappLimit), N70);
        assert_eq!(engine(EngineError::Refused), N71);
        assert_eq!(refusal_of(&AdminError::NoEngine).sentence, N27);
        assert_eq!(refusal_of(&AdminError::AliasTaken).sentence, N32);
        assert_eq!(refusal_of(&AdminError::PhoneTaken).sentence, N34);
        let not_now = refusal_of(&AdminError::NumberNotNow("connected"));
        assert_eq!(
            (not_now.sentence, not_now.state),
            (N47, Some(NumberState::Connected))
        );
        assert_eq!(refusal_of(&AdminError::NameTaken).sentence, N72);
        assert_eq!(status_of(&refused(N72)), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(status_of(&refused(RESTORED)), StatusCode::CONFLICT);
        assert_eq!(status_of(&refused(N84)), StatusCode::OK);
        // The refused key is answered for both origins: a restart, or the key typed again.
        let key = plain(Lang::En, &refused(N68));
        assert!(
            key.contains("restart the engine: it reads its key only when it starts"),
            "{key}"
        );
        assert!(key.contains("arauto-hub admin engine set-key"), "{key}");
        // The text of the line above the list fills the state in the language of the page.
        let said = plain(Lang::En, &not_now);
        assert!(said.contains("\"Connected\""), "{said}");
    }
}
