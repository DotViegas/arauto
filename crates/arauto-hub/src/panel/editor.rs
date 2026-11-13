//! The editor of one access, in a window over the matrix (`docs/08-permissoes.md`), and the
//! window that confirms the revocation of a Hermes.
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /accesses/edit?hermes=&number=` | the access as it is, in a form |
//! | `POST /accesses/save` | the access, whole: `admin access grant`, `wake-wait` or `remove` |
//! | `GET /hermes/revoke?name=` | asks before revoking |
//! | `POST /hermes/revoke` | `admin revoke` |
//!
//! Nothing here decides. The rules of an access (being woken needs reading; starting chats
//! needs sending and every chat) are the ones of [`Grant::check`], inside the function the
//! admin command calls; this module only says them in the words of the screen.
//!
//! What this editor does not do yet: choose the chats of the scope "only chosen chats". The
//! lines of the chats are set by `arauto-hub admin access chat`; here they are counted.

use arauto_protocol::names::{HermesName, NumberAlias};
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Method, Response, StatusCode};
use hyper::body::Incoming;

use super::html::Html;
use super::view::{self, AccessPage, Frame, HermesPage, Over};
use super::{PanelState, data, http, session_of};
use crate::admin::{self, AdminError};
use crate::channel::Hub;
use crate::core::access::{AccessRuleError, Grant, Scope, Verbs};
use crate::db::ResolvedOrg;
use crate::repo::{self, RepoError};

/// The wait before waking of an access that never had one set, in seconds.
const DEFAULT_WAIT_S: u32 = 6;

const WAKE_NEEDS_READ: &str = "Being woken requires reading. Nothing was saved.";
const START_NEEDS_SEND_AND_ALL: &str =
    "Starting chats requires sending and the scope \"every chat\". Nothing was saved.";
const WAIT_RANGE: &str = "The wait before waking goes from 0 to 60 seconds. Nothing was saved.";
const FAILED: &str =
    "Something failed in the hub. See the hub's log and check the access in the matrix.";
const RESTORED: &str = "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.";

/// An access as the form shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editing {
    /// The Hermes.
    pub hermes: String,
    /// The number.
    pub number: String,
    /// The scope and the verbs in the form.
    pub grant: Grant,
    /// The seconds of silence before a notice wakes the Hermes.
    pub wait_s: u32,
    /// How many chats have a line of their own.
    pub lines: usize,
    /// The access as it is saved now, in one sentence.
    pub today: String,
    /// Why the form came back.
    pub error: Option<&'static str>,
}

/// "a, b and c".
fn joined(parts: &[&str]) -> String {
    match parts {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}

/// What an access means, in one sentence: the same one the editor shows while it is filled.
#[must_use]
pub fn sentence(
    hermes: &str,
    number: &str,
    grant: Option<&Grant>,
    wait_s: u32,
    lines: usize,
) -> String {
    let Some(grant) = grant else {
        return format!("The Hermes {hermes} has no access to the number {number}.");
    };
    if grant.scope == Scope::List {
        return format!(
            "The Hermes {hermes} only reaches the chosen chats of the number {number}: {lines} \
             today. What it may do in each one is in its line."
        );
    }
    let verbs = grant.verbs;
    if !(verbs.read || verbs.send || verbs.modify || verbs.wake) {
        return format!("The Hermes {hermes} has no access to the number {number}.");
    }
    let mut can = Vec::new();
    let mut rest = Vec::new();
    if verbs.read {
        can.push("read");
    } else {
        rest.push("does not read".to_owned());
    }
    if verbs.send {
        can.push("send");
    } else {
        rest.push("does not send".to_owned());
    }
    if verbs.modify {
        can.push("modify");
    } else {
        rest.push("does not modify".to_owned());
    }
    rest.push(if verbs.wake {
        format!("is woken after {wait_s} s of silence in the chat")
    } else {
        "is not woken".to_owned()
    });
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    let rest = joined(&rest);
    let mut text = if can.is_empty() {
        format!("The Hermes {hermes} {rest} in every chat of the number {number}.")
    } else {
        format!(
            "The Hermes {hermes} may {} in every chat of the number {number}. It {rest}.",
            joined(&can)
        )
    };
    if grant.can_start_chats {
        text.push_str(" It may start a chat with someone who never talked to the number.");
    }
    match lines {
        0 => {}
        1 => text.push_str(" 1 chat has its own rule."),
        more => text.push_str(&format!(" {more} chats have their own rule.")),
    }
    text
}

fn option(
    html: &mut Html,
    kind: &'static str,
    name: &'static str,
    value: &'static str,
    on: bool,
    label: &'static str,
) {
    html.raw("<label class=\"option\"><input type=\"")
        .raw(kind)
        .raw("\" name=\"")
        .raw(name)
        .raw("\" value=\"")
        .raw(value)
        .raw(if on { "\" checked><span>" } else { "\"><span>" })
        .raw(label)
        .raw("</span></label>");
}

const CLOSE: &str =
    "<svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg>";

/// The page of the accesses with the editor of one of them over it.
#[must_use]
pub fn page(frame: &Frame, accesses: &AccessPage, editing: &Editing) -> String {
    let mut html = Html::new();
    view::accesses_into(&mut html, frame, accesses, Over::Guide { refresh: None });
    html.raw("<dialog open class=\"modal\" id=\"guide\" aria-labelledby=\"editor-title\"><div class=\"modal-head\"><h2 id=\"editor-title\">")
        .text(&editing.hermes)
        .raw(" on the number ")
        .text(&editing.number)
        .raw("</h2><a class=\"close\" href=\"/accesses\" aria-label=\"Close\">")
        .raw(CLOSE)
        .raw("</a></div><div class=\"modal-body\"><form id=\"editor-form\" method=\"post\" action=\"/accesses/save\" autocomplete=\"off\" data-hermes=\"")
        .text(&editing.hermes)
        .raw("\" data-number=\"")
        .text(&editing.number)
        .raw("\" data-lines=\"")
        .number(editing.lines)
        .raw("\"><input type=\"hidden\" name=\"hermes\" value=\"")
        .text(&editing.hermes)
        .raw("\"><input type=\"hidden\" name=\"number\" value=\"")
        .text(&editing.number)
        .raw("\"><div class=\"fields\"><fieldset><legend>Where it applies</legend>");
    let all = editing.grant.scope == Scope::All;
    option(
        &mut html,
        "radio",
        "scope",
        "all",
        all,
        "Every chat, including the ones that come later",
    );
    option(
        &mut html,
        "radio",
        "scope",
        "list",
        !all,
        "Only chosen chats",
    );
    html.raw("<p class=\"hint\">With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the <code>arauto-hub admin access chat</code> command. Today: ")
        .number(editing.lines)
        .raw(".</p></fieldset><fieldset><legend>What it may do</legend>");
    let verbs = editing.grant.verbs;
    option(
        &mut html,
        "checkbox",
        "read",
        "on",
        verbs.read,
        "<b>Read</b>: see chats and messages, search, download files",
    );
    option(
        &mut html,
        "checkbox",
        "send",
        "on",
        verbs.send,
        "<b>Send</b>: write, react, mark as read",
    );
    option(
        &mut html,
        "checkbox",
        "modify",
        "on",
        verbs.modify,
        "<b>Modify</b>: edit and delete what already exists",
    );
    option(
        &mut html,
        "checkbox",
        "wake",
        "on",
        verbs.wake,
        "<b>Be woken</b>: the hub gives notice when a message arrives (requires reading)",
    );
    html.raw("<div class=\"wait-row\"><label for=\"editor-wait\">Wait</label><input id=\"editor-wait\" name=\"wait\" type=\"number\" min=\"0\" max=\"60\" value=\"")
        .number(usize::try_from(editing.wait_s).unwrap_or(0))
        .raw("\"><span>seconds of silence before waking</span></div>");
    option(
        &mut html,
        "checkbox",
        "start",
        "on",
        editing.grant.can_start_chats,
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
    );
    html.raw("</fieldset></div></form><p class=\"sentence\" id=\"editor-sentence\"><span class=\"no-script\">Today: </span>")
        .text(&editing.today)
        .raw("</p>");
    if let Some(error) = editing.error {
        html.raw("<p class=\"error\" role=\"alert\">")
            .raw(error)
            .raw("</p>");
    }
    html.raw("</div><div class=\"modal-foot\"><button class=\"button primary\" type=\"submit\" form=\"editor-form\">Save</button><a class=\"button\" href=\"/accesses\">Cancel</a><span>It applies from the next call, with nothing to restart. With no verb marked, the access is removed.</span></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

/// The page of the Hermes with the question of a revocation over it.
#[must_use]
pub fn revoke_page(
    frame: &Frame,
    hermes: &HermesPage,
    name: &str,
    error: Option<&'static str>,
) -> String {
    let mut html = Html::new();
    view::hermes_into(&mut html, frame, hermes, Over::Guide { refresh: None });
    html.raw("<dialog open class=\"modal\" id=\"guide\" aria-labelledby=\"revoke-title\"><div class=\"modal-head\"><h2 id=\"revoke-title\">Revoke ")
        .text(name)
        .raw("</h2><a class=\"close\" href=\"/\" aria-label=\"Close\">")
        .raw(CLOSE)
        .raw("</a></div><div class=\"modal-body short\"><p>Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.</p><p>The accesses are kept and apply again when a new device is connected. The only way back is a new invite.</p>");
    if let Some(error) = error {
        html.raw("<p class=\"error\" role=\"alert\">")
            .raw(error)
            .raw("</p>");
    }
    html.raw("</div><div class=\"modal-foot\"><form method=\"post\" action=\"/hermes/revoke\"><input type=\"hidden\" name=\"name\" value=\"")
        .text(name)
        .raw("\"><button class=\"button danger\" type=\"submit\">Revoke</button></form><a class=\"button\" href=\"/\">Cancel</a></div></dialog>")
        .raw(view::TAIL);
    html.finish()
}

/// The access of `hermes` on `number` as it is saved, for the form. `None` when either does
/// not exist.
async fn saved(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: &HermesName,
    number: &NumberAlias,
) -> Result<Option<Editing>, RepoError> {
    if repo::number::by_alias(hub.pool(), organization, number)
        .await?
        .is_none()
    {
        return Ok(None);
    }
    let lines = repo::hermes::list(hub.pool(), organization, false).await?;
    if !lines.iter().any(|line| line.hermes.name == *hermes) {
        return Ok(None);
    }
    let listed = repo::access::list(hub.pool(), organization, Some(hermes)).await?;
    let found = listed.iter().find(|access| access.number == *number);
    let (grant, wait_s, lines) = found.map_or(
        (
            Grant {
                scope: Scope::All,
                verbs: Verbs::default(),
                can_start_chats: false,
            },
            DEFAULT_WAIT_S,
            0,
        ),
        |access| {
            (
                access.grant,
                u32::try_from(access.wake_wait_ms / 1_000).unwrap_or(DEFAULT_WAIT_S),
                access.lines.len(),
            )
        },
    );
    Ok(Some(Editing {
        hermes: hermes.as_str().to_owned(),
        number: number.as_str().to_owned(),
        grant,
        wait_s,
        lines,
        today: sentence(
            hermes.as_str(),
            number.as_str(),
            found.map(|access| &access.grant),
            wait_s,
            lines,
        ),
        error: None,
    }))
}

async fn editor(
    hub: &Hub,
    organization: ResolvedOrg,
    status: StatusCode,
    editing: &Editing,
) -> Result<Response<Body>, RepoError> {
    let frame = data::frame(hub, organization).await?;
    let accesses = data::accesses(hub, organization).await?;
    Ok(http::page(status, page(&frame, &accesses, editing)))
}

async fn revoking(
    hub: &Hub,
    organization: ResolvedOrg,
    status: StatusCode,
    name: &HermesName,
    error: Option<&'static str>,
) -> Result<Response<Body>, RepoError> {
    let frame = data::frame(hub, organization).await?;
    let hermes = data::hermes(hub, organization).await?;
    Ok(http::page(
        status,
        revoke_page(&frame, &hermes, name.as_str(), error),
    ))
}

/// What the admin function refused, as one fixed sentence. The error goes to the log.
fn refusal(error: &AdminError) -> (StatusCode, &'static str) {
    match error {
        AdminError::AccessRule(AccessRuleError::WakeWithoutRead) => {
            (StatusCode::OK, WAKE_NEEDS_READ)
        }
        AdminError::AccessRule(AccessRuleError::StartWithoutSendOrAll) => {
            (StatusCode::OK, START_NEEDS_SEND_AND_ALL)
        }
        AdminError::WakeWaitRange => (StatusCode::OK, WAIT_RANGE),
        AdminError::Restored(_) => (StatusCode::CONFLICT, RESTORED),
        other => {
            tracing::error!(error = %other, "the panel could not do what was asked");
            (StatusCode::INTERNAL_SERVER_ERROR, FAILED)
        }
    }
}

/// The address of the matrix that says what was saved.
fn saved_at(hermes: &HermesName, number: &NumberAlias) -> Response<Body> {
    http::see_other_to(&format!(
        "/accesses?hermes={}&number={}",
        hermes.as_str(),
        number.as_str()
    ))
}

async fn save(
    hub: &Hub,
    organization: ResolvedOrg,
    admin: &admin::Session,
    form: &http::Form,
) -> Result<Response<Body>, RepoError> {
    let field = |name: &str| form.field(name).map(|value| value.trim().to_owned());
    let names = field("hermes")
        .and_then(|name| HermesName::parse(&name).ok())
        .zip(field("number").and_then(|alias| NumberAlias::parse(&alias).ok()));
    let Some((hermes, number)) = names else {
        return Ok(http::see_other("/accesses"));
    };
    let Some(mut editing) = saved(hub, organization, &hermes, &number).await? else {
        return Ok(http::see_other("/accesses"));
    };
    let on = |name: &str| form.field(name).is_some();
    let grant = Grant {
        scope: if field("scope").as_deref() == Some("list") {
            Scope::List
        } else {
            Scope::All
        },
        verbs: Verbs {
            read: on("read"),
            send: on("send"),
            modify: on("modify"),
            wake: on("wake"),
        },
        can_start_chats: on("start"),
    };
    // A wait that is not a number is out of its range: nothing is guessed.
    let wait = field("wait").map_or(Some(editing.wait_s), |text| text.parse::<u32>().ok());
    // The form comes back as it was sent, with why.
    editing.grant = grant;
    editing.wait_s = wait.unwrap_or(editing.wait_s);
    let now = hub.now_ms();
    let nothing = !(grant.verbs.read || grant.verbs.send || grant.verbs.modify || grant.verbs.wake);
    let outcome = if nothing && grant.scope == Scope::All && !grant.can_start_chats {
        // No verb on every chat is no access: the line of the matrix goes.
        match admin
            .access_remove(hermes.as_str(), number.as_str(), now)
            .await
        {
            Ok(_) | Err(AdminError::NoAccess) => Ok(()),
            Err(error) => Err(error),
        }
    } else {
        match wait {
            None | Some(61..) => Err(AdminError::WakeWaitRange),
            Some(seconds) => {
                match admin
                    .access_grant(hermes.as_str(), number.as_str(), grant, now)
                    .await
                {
                    Ok(_) => admin
                        .access_wake_wait(hermes.as_str(), number.as_str(), seconds, now)
                        .await
                        .map(|_| ()),
                    Err(error) => Err(error),
                }
            }
        }
    };
    match outcome {
        Ok(()) => Ok(saved_at(&hermes, &number)),
        Err(AdminError::NoSuchHermes | AdminError::NoSuchNumber) => {
            Ok(http::see_other("/accesses"))
        }
        Err(error) => {
            let (status, sentence) = refusal(&error);
            editing.error = Some(sentence);
            editor(hub, organization, status, &editing).await
        }
    }
}

/// The page of the accesses that says what was just saved: `?hermes=&number=`. Only reads;
/// a name that is not one says nothing.
pub(super) async fn saved_sentence(
    hub: &Hub,
    organization: ResolvedOrg,
    query: Option<&str>,
) -> Result<Option<String>, RepoError> {
    let names = http::query_field(query, "hermes")
        .and_then(|name| HermesName::parse(name.trim()).ok())
        .zip(
            http::query_field(query, "number")
                .and_then(|alias| NumberAlias::parse(alias.trim()).ok()),
        );
    let Some((hermes, number)) = names else {
        return Ok(None);
    };
    Ok(saved(hub, organization, &hermes, &number)
        .await?
        .map(|editing| editing.today))
}

/// Answers a request for an address of the editor or of the revocation. The request already
/// named the panel and, when it writes, came from a page of the panel.
pub(super) async fn answer(
    hub: &Hub,
    panel: &PanelState,
    parts: &Parts,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = session_of(hub, panel, &parts.headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let reads = parts.method == Method::GET;
    let path = parts.uri.path();
    let query = parts.uri.query();
    match (reads, path) {
        (true, "/accesses/edit") => {
            let names = http::query_field(query, "hermes")
                .and_then(|name| HermesName::parse(name.trim()).ok())
                .zip(
                    http::query_field(query, "number")
                        .and_then(|alias| NumberAlias::parse(alias.trim()).ok()),
                );
            let Some((hermes, number)) = names else {
                return Ok(http::see_other("/accesses"));
            };
            return match saved(hub, organization, &hermes, &number).await? {
                Some(editing) => editor(hub, organization, StatusCode::OK, &editing).await,
                None => Ok(http::see_other("/accesses")),
            };
        }
        (true, "/hermes/revoke") => {
            let name = http::query_field(query, "name")
                .and_then(|name| HermesName::parse(name.trim()).ok());
            let Some(name) = name else {
                return Ok(http::see_other("/"));
            };
            let lines = repo::hermes::list(hub.pool(), organization, false).await?;
            if !lines.iter().any(|line| line.hermes.name == name) {
                return Ok(http::see_other("/"));
            }
            return revoking(hub, organization, StatusCode::OK, &name, None).await;
        }
        (false, "/accesses/save" | "/hermes/revoke") => {}
        _ => return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n")),
    }
    let form = match http::form(&parts.headers, body).await {
        Ok(form) => form,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let admin = admin::Session::of_panel(
        hub.pool().clone(),
        hub.mark().clone(),
        organization,
        session.admin,
    );
    if path == "/accesses/save" {
        return save(hub, organization, &admin, &form).await;
    }
    let name = form
        .field("name")
        .and_then(|name| HermesName::parse(name.trim()).ok());
    let Some(name) = name else {
        return Ok(http::see_other("/"));
    };
    match admin.revoke(name.as_str(), hub.now_ms()).await {
        Ok(_) | Err(AdminError::NoSuchHermes) => Ok(http::see_other("/")),
        Err(error) => {
            let (status, sentence) = refusal(&error);
            revoking(hub, organization, status, &name, Some(sentence)).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn grant(
        scope: Scope,
        read: bool,
        send: bool,
        modify: bool,
        wake: bool,
        start: bool,
    ) -> Grant {
        Grant {
            scope,
            verbs: Verbs {
                read,
                send,
                modify,
                wake,
            },
            can_start_chats: start,
        }
    }

    #[test]
    fn an_access_is_said_in_one_sentence() {
        let all =
            |read, send, modify, wake, start| grant(Scope::All, read, send, modify, wake, start);
        assert_eq!(
            sentence("ana", "balcao", None, 6, 0),
            "The Hermes ana has no access to the number balcao."
        );
        assert_eq!(
            sentence(
                "ana",
                "balcao",
                Some(&all(true, true, false, true, false)),
                6,
                0
            ),
            "The Hermes ana may read and send in every chat of the number balcao. It does not modify and is woken after 6 s of silence in the chat."
        );
        assert_eq!(
            sentence(
                "ana",
                "balcao",
                Some(&all(true, true, true, false, true)),
                6,
                2
            ),
            "The Hermes ana may read, send and modify in every chat of the number balcao. It is not woken. It may start a chat with someone who never talked to the number. 2 chats have their own rule."
        );
        assert_eq!(
            sentence(
                "ana",
                "balcao",
                Some(&all(true, false, false, false, false)),
                0,
                1
            ),
            "The Hermes ana may read in every chat of the number balcao. It does not send, does not modify and is not woken. 1 chat has its own rule."
        );
        assert_eq!(
            sentence(
                "ana",
                "balcao",
                Some(&all(true, true, true, true, false)),
                12,
                0
            ),
            "The Hermes ana may read, send and modify in every chat of the number balcao. It is woken after 12 s of silence in the chat."
        );
        // No verb on every chat is no access.
        assert_eq!(
            sentence(
                "ana",
                "balcao",
                Some(&all(false, false, false, false, false)),
                6,
                0
            ),
            "The Hermes ana has no access to the number balcao."
        );
        // With chosen chats, the verbs are the ones of each line.
        assert_eq!(
            sentence(
                "ana",
                "balcao",
                Some(&grant(Scope::List, true, true, false, false, false)),
                6,
                3
            ),
            "The Hermes ana only reaches the chosen chats of the number balcao: 3 today. What it may do in each one is in its line."
        );
    }

    #[test]
    fn what_is_in_the_editor_is_shown_as_text() {
        const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";
        let editing = Editing {
            hermes: HOSTILE.to_owned(),
            number: HOSTILE.to_owned(),
            grant: grant(Scope::List, true, false, false, true, false),
            wait_s: 7,
            lines: 2,
            today: HOSTILE.to_owned(),
            error: Some(WAKE_NEEDS_READ),
        };
        let shown = page(&Frame::default(), &AccessPage::default(), &editing);
        assert!(
            !shown.contains("<script>") && !shown.contains("<img"),
            "{shown}"
        );
        assert!(!shown.contains("\"'>"), "{shown}");
        assert_eq!(shown.matches("<script").count(), 1);
        assert!(shown.contains("<script src=\"/assets/panel.js\"></script>"));
        // The form says what is saved: the scope, the verbs and the wait.
        assert!(shown.contains("name=\"scope\" value=\"list\" checked>"));
        assert!(!shown.contains("name=\"scope\" value=\"all\" checked>"));
        assert!(shown.contains("name=\"read\" value=\"on\" checked>"));
        assert!(shown.contains("name=\"wake\" value=\"on\" checked>"));
        assert!(!shown.contains("name=\"send\" value=\"on\" checked>"));
        assert!(shown.contains("name=\"wait\" type=\"number\" min=\"0\" max=\"60\" value=\"7\""));
        assert!(shown.contains("Being woken requires reading."));

        let asks = revoke_page(&Frame::default(), &HermesPage::default(), HOSTILE, None);
        assert!(
            !asks.contains("<script>") && !asks.contains("<img"),
            "{asks}"
        );
        assert!(asks.contains("action=\"/hermes/revoke\""));
    }
}
