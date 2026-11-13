//! Connecting a Hermes from the panel: the addresses of the guide.
//!
//! | Address | What it does |
//! |---|---|
//! | `GET /hermes/connect` | asks the name |
//! | `POST /hermes/connect` | creates the line, issues the invite and shows the block, once |
//! | `POST /hermes/invite` | issues an invite for a line that exists, and shows the block |
//! | `GET /hermes/wait?name=` | what the hub observed so far: waiting, the code, the end |
//! | `POST /hermes/approve` | the code the admin typed, and the answer about the last use |
//! | `POST /hermes/deny` | denies the request |
//!
//! Nothing here decides: every write is one call to the function `arauto-hub admin` calls
//! ([`crate::admin::Session`]), with the same checks, the same audit and the same mark. What
//! is of the panel is only the order of the screens.
//!
//! - **The invite appears once**, in the answer to the form that issued it. No address read
//!   with `GET` issues, shows or spends one.
//! - **The verification code is typed and never shown**, not even back to whoever typed it.
//! - **An answer about the last use counts for the use that was on the screen**: if the
//!   device was used meanwhile, nothing is approved and the new one is shown.

use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Method, Response, StatusCode};
use hyper::body::Incoming;

use super::guide::{self, Ended, Screen};
use super::text::{self, Lang, k};
use super::{PanelState, block, data, http, session_of};
use crate::admin::{self, AdminError};
use crate::channel::Hub;
use crate::core::model::DeviceState;
use crate::db::ResolvedOrg;
use crate::repo::RepoError;
use crate::repo::approval::{ApproveOutcome, LastUse, Recognition, Recognized};

use arauto_protocol::names::{HermesName, NameError};

const NAME_TAKEN: &str = k(
    "There already is a Hermes with that name. To link another device to it, use the button on its line.",
);
const NAME_RETIRED: &str = k(
    "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
);
const USE_NOT_ANSWERED: &str = k(
    "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
);
const USE_CHANGED: &str = k(
    "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
);
const CODE_FORMAT: &str = k("The code has 6 digits, like 123-456. Nothing was counted.");
const RESTORED: &str = k(
    "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
);
const FAILED: &str = k("Something failed in the hub and nothing was done. See the hub's log.");

/// Why a name was refused, in the words of the form.
const fn name_error(error: NameError) -> &'static str {
    match error {
        NameError::Length => k("The name has from 1 to 32 characters."),
        NameError::FirstCharacter => k("The name starts with a lowercase letter or a digit."),
        NameError::Character => k("Use only lowercase letters, digits, hyphen and underscore."),
        NameError::Reserved => k("That name is reserved. Choose another."),
    }
}

/// The guide over the page of the Hermes, read now.
async fn shown(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    status: StatusCode,
    screen: &Screen,
) -> Result<Response<Body>, RepoError> {
    let frame = data::frame(hub, organization, lang).await?;
    let hermes = data::hermes(hub, organization, lang).await?;
    Ok(http::page(status, guide::page(&frame, &hermes, screen)))
}

/// What the admin function refused, as a screen. The error goes to the log of the hub; the
/// page says one fixed sentence.
async fn failed(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    error: AdminError,
) -> Result<Response<Body>, RepoError> {
    let (status, sentence) = match &error {
        AdminError::Restored(_) => (StatusCode::CONFLICT, RESTORED),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, FAILED),
    };
    tracing::error!(%error, "the panel could not do what was asked");
    shown(
        hub,
        organization,
        lang,
        status,
        &Screen::Failed { sentence },
    )
    .await
}

/// Where the guide of a Hermes goes on.
fn wait_of(name: &HermesName) -> Response<Body> {
    http::see_other_to(&format!("/hermes/wait?name={}", name.as_str()))
}

/// Issues the invite of `name` and shows the block: the only time the invite appears.
async fn issue(
    hub: &Hub,
    panel: &PanelState,
    organization: ResolvedOrg,
    lang: Lang,
    admin: &admin::Session,
    name: &HermesName,
) -> Result<Response<Body>, RepoError> {
    let Some(connector) = panel.connector.as_ref() else {
        let screen = Screen::NoConnector {
            name: name.as_str().to_owned(),
        };
        return shown(hub, organization, lang, StatusCode::OK, &screen).await;
    };
    let now = hub.now_ms();
    let issued = match admin.invite(name.as_str(), &panel.hub_url, now).await {
        Ok(issued) => issued,
        Err(AdminError::NoSuchHermes) => return Ok(http::see_other("/")),
        Err(error) => return failed(hub, organization, lang, error).await,
    };
    let Some(text) = block::text(&issued, connector) else {
        return failed(hub, organization, lang, AdminError::Task).await;
    };
    let screen = Screen::Block {
        name: name.as_str().to_owned(),
        text,
        seconds: issued.issued.expires_at.saturating_sub(now).max(0) / 1_000,
        reconnect: issued.issued.reconnect,
        cancelled: issued.issued.cancelled.is_some(),
        closed_request: issued.issued.closed_request.is_some(),
    };
    shown(hub, organization, lang, StatusCode::OK, &screen).await
}

/// The screen of the code again, with what the last answer got wrong. When the request is no
/// longer waiting for a code, the screen is the one of what it became.
async fn code_again(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    name: &HermesName,
    said: String,
) -> Result<Response<Body>, RepoError> {
    let Some(mut screen) = data::guide(hub, organization, lang, name).await? else {
        return Ok(http::see_other("/"));
    };
    if let Screen::Code { error, .. } = &mut screen {
        *error = Some(said);
    }
    shown(hub, organization, lang, StatusCode::OK, &screen).await
}

/// Whether the last use the form says it showed is the one the device has now.
fn same_use(last: &LastUse, form: &http::Form) -> bool {
    let device = form.field("seen_device");
    let at = form.field("seen_at");
    let state = form.field("seen_state");
    device.as_deref().map(String::as_str) == Some(last.device.as_hyphenated().to_string().as_str())
        && at.as_deref().map(String::as_str)
            == Some(
                last.at
                    .map(|at| at.to_string())
                    .unwrap_or_default()
                    .as_str(),
            )
        && state.as_deref().map(String::as_str) == Some(last.state.as_str())
}

async fn approve(
    hub: &Hub,
    organization: ResolvedOrg,
    lang: Lang,
    admin: &admin::Session,
    name: &HermesName,
    form: &http::Form,
) -> Result<Response<Body>, RepoError> {
    // The request as it is now, with the last use of the device it would replace.
    let view = match admin.request(name.as_str()).await {
        Ok(view) => view.filter(|view| view.request.state == DeviceState::Pending),
        Err(error) => return failed(hub, organization, lang, error).await,
    };
    // Nothing waits for a code: the page of the guide says what there is instead.
    let Some(view) = view else {
        return Ok(wait_of(name));
    };
    let answer = match form.field("use").as_deref().map(String::as_str) {
        Some("recognized") => Some(Recognition::Recognized),
        Some("not-recognized") => Some(Recognition::NotRecognized),
        _ => None,
    };
    let recognition = match (&view.replaces, answer) {
        (Some(_), None) => {
            return code_again(
                hub,
                organization,
                lang,
                name,
                text::tr(lang, USE_NOT_ANSWERED).to_owned(),
            )
            .await;
        }
        (Some(last), Some(_)) if !same_use(last, form) => {
            return code_again(
                hub,
                organization,
                lang,
                name,
                text::tr(lang, USE_CHANGED).to_owned(),
            )
            .await;
        }
        (Some(last), Some(answer)) => Some(Recognized {
            answer,
            shown: last.clone(),
        }),
        (None, _) => None,
    };
    let typed = form.field("code").unwrap_or_default();
    let outcome = admin
        .approve(name.as_str(), &typed, recognition, hub.now_ms())
        .await;
    let ended = |why| Screen::Ended {
        name: name.as_str().to_owned(),
        why,
    };
    match outcome {
        Ok(ApproveOutcome::Approved(_) | ApproveOutcome::NoRequest) => Ok(wait_of(name)),
        Ok(ApproveOutcome::NotFound) | Err(AdminError::NoSuchHermes) => Ok(http::see_other("/")),
        Ok(ApproveOutcome::WrongCode { attempts }) => {
            let said = text::fill(
                lang,
                k(
                    "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
                ),
                &[
                    ("attempts", &attempts.to_string()),
                    ("max", &crate::repo::approval::CODE_ATTEMPTS_MAX.to_string()),
                ],
            );
            code_again(hub, organization, lang, name, said).await
        }
        Ok(ApproveOutcome::Denied { .. }) => {
            shown(
                hub,
                organization,
                lang,
                StatusCode::OK,
                &ended(Ended::Denied),
            )
            .await
        }
        Ok(ApproveOutcome::Expired { .. }) => {
            shown(
                hub,
                organization,
                lang,
                StatusCode::OK,
                &ended(Ended::Expired),
            )
            .await
        }
        Ok(ApproveOutcome::NeedsRecognition(_)) => {
            code_again(
                hub,
                organization,
                lang,
                name,
                text::tr(lang, USE_CHANGED).to_owned(),
            )
            .await
        }
        Err(AdminError::InvalidCode) => {
            code_again(
                hub,
                organization,
                lang,
                name,
                text::tr(lang, CODE_FORMAT).to_owned(),
            )
            .await
        }
        Err(error) => failed(hub, organization, lang, error).await,
    }
}

/// Answers a request for an address of the guide. The request already named the panel and,
/// when it writes, came from a page of the panel.
pub(super) async fn answer(
    hub: &Hub,
    panel: &PanelState,
    parts: &Parts,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = session_of(hub, panel, &parts.headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let lang = super::lang_of(&parts.headers);
    let reads = parts.method == Method::GET;
    let path = parts.uri.path();
    match (reads, path) {
        (true, "/hermes/connect") => {
            let screen = Screen::Name {
                typed: String::new(),
                error: None,
            };
            return shown(hub, organization, lang, StatusCode::OK, &screen).await;
        }
        (true, "/hermes/wait") => {
            let name = http::query_field(parts.uri.query(), "name")
                .and_then(|name| HermesName::parse(name.trim()).ok());
            let Some(name) = name else {
                return Ok(http::see_other("/"));
            };
            return match data::guide(hub, organization, lang, &name).await? {
                Some(screen) => shown(hub, organization, lang, StatusCode::OK, &screen).await,
                None => Ok(http::see_other("/")),
            };
        }
        (false, "/hermes/connect" | "/hermes/invite" | "/hermes/approve" | "/hermes/deny") => {}
        _ => return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n")),
    }

    let form = match http::form(&parts.headers, body).await {
        Ok(form) => form,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let typed = form
        .field("name")
        .map(|name| name.trim().to_owned())
        .unwrap_or_default();
    let admin = admin::Session::of_panel(
        hub.pool().clone(),
        hub.mark().clone(),
        organization,
        session.admin,
    );
    let now = hub.now_ms();
    if path == "/hermes/connect" {
        // The line and its first invite, one after the other: a name refused issues nothing.
        let again = |error| Screen::Name {
            typed: typed.clone(),
            error: Some(error),
        };
        let name = match HermesName::parse(&typed) {
            Ok(name) => name,
            Err(error) => {
                let screen = again(name_error(error));
                return shown(hub, organization, lang, StatusCode::OK, &screen).await;
            }
        };
        // Without a connector to serve there is no block: the line is not created either.
        if panel.connector.is_none() {
            return issue(hub, panel, organization, lang, &admin, &name).await;
        }
        return match admin.hermes_add(name.as_str(), now).await {
            Ok(_) => issue(hub, panel, organization, lang, &admin, &name).await,
            Err(AdminError::InvalidName(error)) => {
                shown(
                    hub,
                    organization,
                    lang,
                    StatusCode::OK,
                    &again(name_error(error)),
                )
                .await
            }
            Err(AdminError::NameTaken) => {
                shown(hub, organization, lang, StatusCode::OK, &again(NAME_TAKEN)).await
            }
            Err(AdminError::NameRetired) => {
                shown(
                    hub,
                    organization,
                    lang,
                    StatusCode::OK,
                    &again(NAME_RETIRED),
                )
                .await
            }
            Err(error) => failed(hub, organization, lang, error).await,
        };
    }
    // The other forms are about a Hermes that exists, by its name.
    let Ok(name) = HermesName::parse(&typed) else {
        return Ok(http::see_other("/"));
    };
    match path {
        "/hermes/invite" => issue(hub, panel, organization, lang, &admin, &name).await,
        "/hermes/approve" => approve(hub, organization, lang, &admin, &name, &form).await,
        _ => match admin.deny(name.as_str(), now).await {
            Ok(_) | Err(AdminError::NoSuchHermes) => Ok(http::see_other("/")),
            Err(error) => failed(hub, organization, lang, error).await,
        },
    }
}
