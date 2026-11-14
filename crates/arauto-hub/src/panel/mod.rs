//! The panel of the admin: pages served by the hub itself, on a port of their own
//! (`docs/decisoes/0040-painel-dentro-do-hub.md`, `docs/07-telas.md`).
//!
//! | Address | What it answers |
//! |---|---|
//! | `GET /sign-in` | the entry: step 1 of the first access, or the passkey of who comes back |
//! | `POST /sign-in` | spends the code and opens a session |
//! | `POST /sign-out` | closes the session |
//! | `GET /` | the Hermes |
//! | `GET /installation` | the window that checks the installation ([`installation`]) |
//! | `GET /whatsapps` | the numbers |
//! | `GET /accesses` | the matrix of accesses |
//! | `GET /assets/{file}` | the sheet, the fonts and the script of the panel |
//! | `POST /language` | the language the person chose, kept in a cookie ([`text`]) |
//! | `/hermes/...` | the guide that connects a Hermes ([`connect`]) |
//! | `/accesses/edit`, `/accesses/save`, `/hermes/revoke` | the editor of an access and the revocation of a Hermes ([`editor`]) |
//!
//! What holds for every request:
//!
//! - **The panel is off by default.** Without `ARAUTO_LISTEN_PANEL` the hub listens on no
//!   port for it, and no address of the panel exists on the two other ports.
//! - **A request names the panel.** Its `Host` is the one of `ARAUTO_PANEL_URL`; one that
//!   reached this port by another name is refused before anything is read.
//! - **A request that reads writes nothing.** `GET` never spends a code, never touches a
//!   row. How long a session has been idle is kept in memory for that reason.
//! - **A request that writes came from a page of the panel.** `POST` is only accepted with
//!   the `Origin` of the panel, and the cookie of the session is not sent across sites.
//! - **No secret is shown.** The entry code is typed, never displayed; the identifier of the
//!   session lives in a cookie no script can read, and the hub keeps only its SHA-256.
//! - **Every value is escaped** ([`html`]). No page carries a script: a page with a guide
//!   loads the one script file of the panel, which only helps the screen.
//!
//! What the pages show is what `arauto-hub admin` shows, through the same repository
//! functions; what they change, they change by the functions that command calls.

mod attempts;
pub mod block;
mod connect;
mod data;
pub mod editor;
pub mod guide;
mod html;
mod http;
pub mod installation;
pub mod passkey;
pub mod secret;
pub mod text;
pub mod view;

use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, Response, StatusCode, header};
use dashmap::DashMap;
use hyper::body::Incoming;
use uuid::Uuid;

use self::attempts::{Attempts, Kind};
use self::passkey::{RelyingParty, StoredPasskey};
use self::secret::{EntryCode, SessionSecret};
use self::view::Said;
use crate::audit::{Actor, AuditError};
use crate::channel::client_ip::{client_ip, no_trusted_proxy, untrusted_forwarder};
use crate::channel::{Downloads, Hub};
use crate::check;
use crate::config::{PasswordRotation, PublicUrl};
use crate::db::{self, ResolvedOrg, Rotated, RotationError, When};
use crate::download::Listed;
use crate::repo::panel::{
    NewPasskey, PanelSession, Registered, SESSION_IDLE_MS, SESSION_VALIDITY_MS, SignIn,
};
use crate::repo::{self, RepoError};

/// The name a passkey shows for the admin of the panel, in the list of the authenticator.
const PASSKEY_USER: &str = "admin";

/// The files of the panel, written by the project and built into the program.
const ASSETS: &[(&str, &str, &[u8])] = &[
    (
        "panel.css",
        "text/css; charset=utf-8",
        include_bytes!("assets/panel.css"),
    ),
    (
        "panel.js",
        "text/javascript; charset=utf-8",
        include_bytes!("assets/panel.js"),
    ),
    (
        "poppins-400.woff2",
        "font/woff2",
        include_bytes!("assets/fonts/poppins-400.woff2"),
    ),
    (
        "poppins-600.woff2",
        "font/woff2",
        include_bytes!("assets/fonts/poppins-600.woff2"),
    ),
    (
        "poppins-800.woff2",
        "font/woff2",
        include_bytes!("assets/fonts/poppins-800.woff2"),
    ),
    (
        "atkinson-next.woff2",
        "font/woff2",
        include_bytes!("assets/fonts/atkinson-next.woff2"),
    ),
    (
        "atkinson-mono-400.woff2",
        "font/woff2",
        include_bytes!("assets/fonts/atkinson-mono-400.woff2"),
    ),
];

/// What the panel keeps in the memory of the hub.
#[derive(Debug)]
pub struct PanelState {
    /// `https://host[:port]`, as a browser writes it in `Origin`.
    origin: String,
    /// `host[:port]`, as a browser writes it in `Host`.
    authority: String,
    /// The name of the cookie of the session, and whether it is only sent over TLS.
    cookie: &'static str,
    secure: bool,
    /// When each open session was last used. A session that is not here is not served: the
    /// hub that knew it stopped.
    seen: DashMap<Uuid, i64>,
    /// The ceilings on what a visitor with no session asks: per address and for everyone.
    attempts: Attempts,
    /// The organization this panel serves: the one the admin command resolves, found when the
    /// hub started. `None` with zero or several: the entry then says nothing of a first access.
    organization: Option<ResolvedOrg>,
    /// The address of the hub that goes in a block: the one the connectors reach.
    hub_url: PublicUrl,
    /// The connector a block tells a Hermes to download, when the hub serves one.
    connector: Option<Listed>,
    /// The passkey ceremonies of this panel, for its own address. `None` when a browser
    /// would refuse a passkey at that address.
    relying: Option<RelyingParty>,
}

impl PanelState {
    /// The panel reached at `url`, of a hub the connectors reach at `hub_url` and that
    /// serves `downloads`.
    #[must_use]
    pub fn new(url: &PublicUrl, hub_url: &PublicUrl, downloads: &Downloads) -> Self {
        let secure = url.is_secure();
        Self {
            origin: url.origin(),
            authority: url.authority().to_owned(),
            // Over TLS the cookie is bound to this host alone: no subdomain sets it and no
            // path narrows it. On this machine, without TLS, a browser would refuse that name.
            cookie: if secure {
                "__Host-arauto-panel"
            } else {
                "arauto-panel"
            },
            secure,
            seen: DashMap::new(),
            attempts: Attempts::default(),
            organization: None,
            hub_url: hub_url.clone(),
            connector: block::connector(&downloads.listed()).cloned(),
            relying: RelyingParty::new(&url.origin()).ok(),
        }
    }

    /// The organization this panel serves, found when the hub started: the one the first access
    /// is issued for, so the entry and the start code never disagree.
    #[must_use]
    pub(crate) const fn organization(&self) -> Option<ResolvedOrg> {
        self.organization
    }

    /// The organization this panel serves, found when the hub started.
    #[must_use]
    pub(crate) fn with_organization(mut self, organization: Option<ResolvedOrg>) -> Self {
        self.organization = organization;
        self
    }

    /// Counts one ask of a visitor with no session. `false` when it is over a ceiling.
    fn allow(&self, kind: Kind, address: Option<IpAddr>, now: i64) -> bool {
        self.attempts.allow(kind, address, now)
    }

    /// A session was opened now.
    fn opened(&self, session: Uuid, now: i64) {
        // What sat idle past its time is forgotten here; its row is closed by the next write.
        self.seen
            .retain(|_, last| now.saturating_sub(*last) < SESSION_IDLE_MS);
        self.seen.insert(session, now);
    }

    /// A session was used now. `false` when it sat idle for too long, or when this hub never
    /// saw it: it is forgotten and not served again.
    fn used(&self, session: Uuid, now: i64) -> bool {
        let alive = match self.seen.get_mut(&session) {
            Some(mut last) => {
                let alive = now.saturating_sub(*last) < SESSION_IDLE_MS;
                if alive {
                    *last = now;
                }
                alive
            }
            None => false,
        };
        if !alive {
            self.seen.remove(&session);
        }
        alive
    }

    /// `https://host[:port]`, the address of the panel: where the block of the first access
    /// says to open it.
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    fn closed(&self, session: Uuid) {
        self.seen.remove(&session);
    }

    fn set_cookie(&self, value: &str, max_age_s: i64) -> String {
        format!(
            "{}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_s}{}",
            self.cookie,
            if self.secure { "; Secure" } else { "" }
        )
    }
}

/// The organization the panel serves, from what the hub found when it started: the one the
/// admin command resolves. Zero or several give `None`, and the entry then says nothing of a
/// first access; a database that did not answer gives `None` too, said in the log, so the
/// entry of a new hub is not the page of who comes back without a word.
pub(crate) fn served_organization<T: Copy>(found: Result<Vec<T>, db::DbError>) -> Option<T> {
    match found.as_deref() {
        Ok([organization]) => Some(*organization),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(
                %error,
                "the organization of the panel could not be read: until the hub starts again, \
                 its entry shows the page of who comes back"
            );
            None
        }
    }
}

/// The language of a request: the one the person chose in the panel, else the first the
/// browser prefers that the panel speaks, else English.
fn lang_of(headers: &HeaderMap) -> text::Lang {
    text::Lang::negotiate(
        http::cookie(headers, text::COOKIE),
        headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    )
}

/// Keeps the language a person chose: a cookie with the tag of a language of the list, and
/// nothing else. It needs no session (the page of the entry has the same form) and changes
/// nothing in the hub. What is not a language of the list changes nothing at all.
async fn language(
    panel: &PanelState,
    headers: &HeaderMap,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let chosen = match http::form_field(headers, body, "lang").await {
        Ok(chosen) => chosen,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let answer = http::see_other("/");
    let Some(lang) = chosen.and_then(|tag| text::Lang::from_tag(tag.trim())) else {
        return Ok(answer);
    };
    // A year: it is a preference, and no secret.
    let cookie = format!(
        "{}={}; Path=/; SameSite=Lax; Max-Age=31536000{}",
        text::COOKIE,
        lang.tag(),
        if panel.secure { "; Secure" } else { "" }
    );
    Ok(http::with_cookie(answer, &cookie))
}

/// Something of the hub failed while a page was read or a form was handled.
fn failed(error: &RepoError) -> Response<Body> {
    tracing::error!(%error, "the panel could not answer");
    http::plain(
        StatusCode::INTERNAL_SERVER_ERROR,
        "The hub could not answer. See its log.\n",
    )
}

/// The session a request brings, when it reaches the whole panel: a passkey was shown in
/// it. Every page and every form of the panel asks this one. Only reads the database.
async fn session_of(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Option<(ResolvedOrg, PanelSession)>, RepoError> {
    Ok(any_session_of(hub, panel, headers)
        .await?
        .filter(|(_, session)| session.full))
}

/// The session a request brings, when it is one this hub serves: open, in its hours, and
/// used not long ago. One opened by a code only reaches the registration of a passkey: only
/// that registration asks this function. Only reads the database.
async fn any_session_of(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Option<(ResolvedOrg, PanelSession)>, RepoError> {
    let Some(secret) = http::cookie(headers, panel.cookie).and_then(SessionSecret::parse) else {
        return Ok(None);
    };
    let digest = secret.digest();
    let Some(organization) = db::organization_of_panel_session(hub.pool(), &digest).await? else {
        return Ok(None);
    };
    let Some(session) = repo::panel::session(hub.pool(), organization, &digest).await? else {
        return Ok(None);
    };
    let now = hub.now_ms();
    // A deadline passed when the time of the hub reached it.
    if session.expires_at <= now || !panel.used(session.id, now) {
        return Ok(None);
    }
    Ok(Some((organization, session)))
}

/// Answers a request that came in by the port of the panel.
pub(crate) async fn route(
    request: Request<Incoming>,
    peer: SocketAddr,
    hub: Arc<Hub>,
) -> Response<Body> {
    let Some(panel) = hub.panel() else {
        return http::plain(StatusCode::NOT_FOUND, "not found\n");
    };
    let (parts, body) = request.into_parts();
    if !http::header_is(&parts.headers, header::HOST, &panel.authority) {
        return http::plain(
            StatusCode::MISDIRECTED_REQUEST,
            "This is not the address of the panel.\nOpen the panel at the address set in ARAUTO_PANEL_URL: a passkey works only there.\n",
        );
    }
    let path = parts.uri.path();
    let reads = parts.method == Method::GET;
    let writes = parts.method == Method::POST;
    if !reads && !writes {
        return http::plain(StatusCode::METHOD_NOT_ALLOWED, "method not allowed\n");
    }
    if writes && !http::header_is(&parts.headers, header::ORIGIN, &panel.origin) {
        return http::plain(
            StatusCode::FORBIDDEN,
            "This request did not come from a page of the panel. Open the panel at its address and try again.\n",
        );
    }
    let answer = match (reads, path) {
        (true, path) if path.starts_with("/assets/") => {
            // The name is only ever compared with the names of the list: it never becomes a path.
            let name = path.strip_prefix("/assets/").unwrap_or_default();
            return ASSETS
                .iter()
                .find(|(file, _, _)| *file == name)
                .map_or_else(
                    || http::plain(StatusCode::NOT_FOUND, "not found\n"),
                    |(_, kind, bytes)| http::file(kind, bytes),
                );
        }
        (true, "/sign-in") => sign_in_page(&hub, panel, &parts.headers).await,
        (false, "/language") => language(panel, &parts.headers, body).await,
        (false, "/sign-in") => sign_in(&hub, panel, peer, &parts.headers, body).await,
        (false, "/sign-out") => sign_out(&hub, panel, peer, &parts.headers).await,
        (true, "/first-access") => first_access_page(&hub, panel, &parts.headers).await,
        (false, "/passkey/register/start") => register_start(&hub, panel, &parts.headers).await,
        (false, "/passkey/register") => register(&hub, panel, peer, &parts.headers, body).await,
        (false, "/passkey/sign-in/start") => passkey_start(&hub, panel, peer, &parts.headers).await,
        (false, "/passkey/sign-in") => {
            passkey_sign_in(&hub, panel, peer, &parts.headers, body).await
        }
        (true, "/installation") => {
            installation(&hub, panel, peer, &parts.headers, parts.uri.query()).await
        }
        (true, "/" | "/whatsapps" | "/accesses") => {
            page(&hub, panel, &parts.headers, path, parts.uri.query()).await
        }
        (_, path) if path == "/hermes/revoke" || path.starts_with("/accesses/") => {
            editor::answer(&hub, panel, &parts, body).await
        }
        (_, path) if path.starts_with("/hermes/") => {
            connect::answer(&hub, panel, &parts, body).await
        }
        _ => return http::plain(StatusCode::NOT_FOUND, "not found\n"),
    };
    answer.unwrap_or_else(|error| failed(&error))
}

async fn sign_in_page(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    if let Some((_, session)) = any_session_of(hub, panel, headers).await? {
        return Ok(http::see_other(if session.full {
            "/"
        } else {
            "/first-access"
        }));
    }
    entry(hub, panel, headers, StatusCode::OK, None).await
}

/// Whether the organization this panel serves never had its first access. Only reads the
/// durable mark of the first passkey: losing or removing every passkey later does not bring
/// the first access back. With no organization to serve it says no.
async fn first_access_pending(hub: &Hub, panel: &PanelState) -> Result<bool, RepoError> {
    match panel.organization {
        Some(organization) => Ok(!repo::panel::first_access_done(hub.pool(), organization).await?),
        None => Ok(false),
    }
}

/// The page of the entry of the state the hub is in, with what happened to the visitor. It
/// only reads.
async fn entry(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
    status: StatusCode,
    said: Option<Said>,
) -> Result<Response<Body>, RepoError> {
    let pending = first_access_pending(hub, panel).await?;
    Ok(http::page(
        status,
        view::sign_in(lang_of(headers), pending, said),
    ))
}

/// The page of a session opened by a code: the registration of a passkey.
async fn first_access_page(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    match any_session_of(hub, panel, headers).await? {
        None => Ok(http::see_other("/sign-in")),
        Some((_, session)) if session.full => Ok(http::see_other("/")),
        Some(_) => Ok(http::page(
            StatusCode::OK,
            view::first_access(lang_of(headers), panel.origin(), None),
        )),
    }
}

/// The challenge of a registration, for the script of the page: any session of the panel
/// may register a passkey for its admin.
async fn register_start(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = any_session_of(hub, panel, headers).await? else {
        // 401, not 403: the script of the page sends the form with no credential then, and the
        // registration answers with the page that says the panel session ended.
        return Ok(http::plain(
            StatusCode::UNAUTHORIZED,
            "The panel session ended.\n",
        ));
    };
    let Some(relying) = &panel.relying else {
        return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n"));
    };
    let existing =
        repo::panel::passkey_credentials(hub.pool(), organization, session.admin).await?;
    Ok(
        match relying.start_registration(session.admin.as_bytes(), PASSKEY_USER, &existing) {
            Ok(options) => http::json(options),
            Err(_) => http::plain(StatusCode::SERVICE_UNAVAILABLE, "Try again.\n"),
        },
    )
}

/// Keeps the passkey a browser just made, when the hub asked for it in this session. From
/// here on the session reaches the whole panel.
async fn register(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, session)) = any_session_of(hub, panel, headers).await? else {
        return session_ended(hub, panel, headers).await;
    };
    let answered = match http::form_field(headers, body, "credential").await {
        Ok(answered) => answered,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let refused = || {
        http::page(
            StatusCode::OK,
            view::first_access(lang_of(headers), panel.origin(), Some(Said::PasskeyRefused)),
        )
    };
    let (Some(relying), Some(answered)) = (&panel.relying, answered) else {
        return Ok(refused());
    };
    let Ok(made) = relying.finish_registration(session.admin.as_bytes(), answered.as_bytes())
    else {
        return Ok(refused());
    };
    let new = NewPasskey {
        credential_id: made.credential_id,
        user_handle: made.user_handle.to_vec(),
        static_state: made.static_state,
        dynamic_state: made.dynamic_state,
    };
    let now = hub.now_ms();
    let address = client_ip(hub.client_ip_source(), peer, headers).ok();
    let kept = repo::panel::register_passkey(
        hub.pool(),
        hub.mark(),
        organization,
        session,
        &new,
        address,
        now,
    )
    .await;
    match kept {
        Ok(Registered::Kept { first, .. }) => {
            if first {
                if hub.storage().password_rotation == PasswordRotation::AtFirstAccess {
                    replace_database_password(hub, organization).await;
                }
                // The only thing a request does about the first access: tell the task that
                // owns the block of the log. It reads the database and writes the closing line,
                // after what the replacement of the password wrote.
                hub.first_passkey_kept();
            }
            // The first passkey ends in the check of the installation; later ones go home.
            Ok(http::see_other(if first {
                "/installation?welcome"
            } else {
                "/"
            }))
        }
        // The passkey is kept and the audit mark did not follow: the admin is told, as for
        // every security write, and the task still hears of the first passkey now rather than
        // at its next look.
        Ok(Registered::KeptMarkBehind { first, error, .. }) => {
            let shown = failed(&error);
            hub.security_write_failed(organization, None, error).await;
            if first {
                // The mark of the first access is set: no later passkey replaces the password,
                // so it is replaced now, as when the audit mark follows.
                if hub.storage().password_rotation == PasswordRotation::AtFirstAccess {
                    replace_database_password(hub, organization).await;
                }
                hub.first_passkey_kept();
                // The passkey is kept and the session reaches the whole panel: the person lands
                // on the check window like anyone else, where the frame shows the alert that
                // the audit mark did not follow. A bare 500 would say the passkey was lost.
                return Ok(http::see_other("/installation?welcome"));
            }
            Ok(shown)
        }
        // Its code is no longer unspent: the session is closed and nothing was kept.
        Ok(Registered::NoSession) => {
            panel.closed(session.id);
            session_ended(hub, panel, headers).await
        }
        Ok(Registered::Repeated) => Ok(refused()),
        Err(error) => security_failed(hub, organization, error).await,
    }
}

/// The panel session of a registration is gone, or its code is no longer unspent: the entry of
/// the state the hub is in, with the sentence that says so, and the dead cookie dropped.
async fn session_ended(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    let page = entry(
        hub,
        panel,
        headers,
        StatusCode::OK,
        Some(Said::SessionEnded),
    )
    .await?;
    Ok(http::with_cookie(page, &panel.set_cookie("", 0)))
}

/// The first passkey of the installation is kept: when `ARAUTO_DATABASE_PASSWORD` asks for it,
/// the hub replaces the provisional password of its database role with one it draws, which
/// nobody sees ([`db::rotate_password`]). The pools of this hub use it from then on.
///
/// A failure does not undo the registration: it goes to the log, without the password, and
/// `arauto-hub admin database rotate-password` does the same later.
async fn replace_database_password(hub: &Hub, organization: ResolvedOrg) {
    let replaced = db::rotate_password(
        hub.storage(),
        &[hub.pool(), hub.sweep_pool()],
        hub.mark(),
        Actor::System,
        When::FirstAccess,
        hub.now_ms(),
    )
    .await;
    match replaced {
        Ok(Rotated::Replaced) => tracing::info!(
            "the password of the database role was replaced by one the hub drew; it is kept \
             in the data folder"
        ),
        Ok(Rotated::NotAsked | Rotated::AlreadyReplaced) => {}
        // The database came back from a copy, or the audit mark did not follow: what every
        // security write does then.
        Err(RotationError::Audit(
            error @ (AuditError::Restored(_) | AuditError::MarkNotAdvanced { .. }),
        )) => {
            hub.security_write_failed(organization, None, RepoError::from(error))
                .await;
        }
        Err(error) => {
            tracing::error!(%error, "the password of the database role was not replaced");
        }
    }
}

/// A security write of the panel failed. When the row is written and the audit mark did not
/// follow, the hub says so in an alert, as for every security write.
async fn security_failed(
    hub: &Hub,
    organization: ResolvedOrg,
    error: RepoError,
) -> Result<Response<Body>, RepoError> {
    if matches!(error, RepoError::Audit(AuditError::MarkNotAdvanced { .. })) {
        let shown = failed(&error);
        hub.security_write_failed(organization, None, error).await;
        return Ok(shown);
    }
    Err(error)
}

/// The address of the client of a request, as the hub tells it: what the audit records.
fn address_of(hub: &Hub, peer: SocketAddr, headers: &HeaderMap) -> Option<IpAddr> {
    client_ip(hub.client_ip_source(), peer, headers).ok()
}

/// The address the ceilings count a request under: the client, as the hub tells it. When a
/// trusted proxy brought a forwarded address the hub cannot read, the request is counted under
/// the proxy itself, with the ceiling of one address: a header the client spoils never takes
/// it out of the ceiling per address.
fn counted_as(address: Option<IpAddr>, peer: SocketAddr) -> IpAddr {
    address.unwrap_or_else(|| canonical_ip(peer))
}

/// The challenge of a sign-in, for the script of the page of the entry. It needs no session,
/// so it has its own ceiling of challenges, per address and for everyone: a flood does not fill
/// the memory of the ceremonies. While the first access was not done there is no passkey to
/// sign in with, and it answers 404.
async fn passkey_start(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    let address = address_of(hub, peer, headers);
    if !panel.allow(
        Kind::Challenges,
        Some(counted_as(address, peer)),
        hub.now_ms(),
    ) {
        return Ok(http::plain(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many attempts.\n",
        ));
    }
    if first_access_pending(hub, panel).await? {
        return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n"));
    }
    let Some(relying) = &panel.relying else {
        return Ok(http::plain(StatusCode::NOT_FOUND, "not found\n"));
    };
    Ok(match relying.start_authentication() {
        Ok(options) => http::json(options),
        Err(_) => http::plain(StatusCode::SERVICE_UNAVAILABLE, "Try again.\n"),
    })
}

/// Checks what a browser answered to the challenge of a sign-in and opens a session that
/// reaches the whole panel.
async fn passkey_sign_in(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let now = hub.now_ms();
    let address = address_of(hub, peer, headers);
    if !panel.allow(Kind::Challenges, Some(counted_as(address, peer)), now) {
        return entry(
            hub,
            panel,
            headers,
            StatusCode::TOO_MANY_REQUESTS,
            Some(Said::TooManyChallenges),
        )
        .await;
    }
    let answered = match http::form_field(headers, body, "credential").await {
        Ok(answered) => answered,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let (Some(relying), Some(answered)) = (&panel.relying, answered) else {
        return refused_passkey(hub, panel, headers).await;
    };
    // The challenge is spent here, whatever comes next.
    let Ok(pending) = relying.claim_authentication(answered.as_bytes()) else {
        return refused_passkey(hub, panel, headers).await;
    };
    let Some(organization) =
        db::organization_of_panel_passkey(hub.pool(), pending.credential_id()).await?
    else {
        return refused_passkey(hub, panel, headers).await;
    };
    let Some(stored) =
        repo::panel::passkey(hub.pool(), organization, pending.credential_id()).await?
    else {
        return refused_passkey(hub, panel, headers).await;
    };
    let checked = relying.finish_authentication(
        pending,
        Some(StoredPasskey {
            credential_id: &stored.credential_id,
            user_handle: &stored.user_handle,
            static_state: &stored.static_state,
            dynamic_state: &stored.dynamic_state,
        }),
    );
    let Ok(checked) = checked else {
        return refused_passkey(hub, panel, headers).await;
    };
    let opened = repo::panel::sign_in_with_passkey(
        hub.pool(),
        hub.mark(),
        organization,
        &stored,
        checked.updated_dynamic_state.as_deref(),
        address,
        now,
    )
    .await;
    match opened {
        Ok(Some(opened)) => {
            panel.opened(opened.id, now);
            let cookie = panel.set_cookie(&opened.secret.reveal(), SESSION_VALIDITY_MS / 1_000);
            Ok(http::with_cookie(http::see_other("/"), &cookie))
        }
        Ok(None) => refused_passkey(hub, panel, headers).await,
        Err(error) => security_failed(hub, organization, error).await,
    }
}

/// The page of the entry, after the hub did not accept a passkey answer.
async fn refused_passkey(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    entry(
        hub,
        panel,
        headers,
        StatusCode::OK,
        Some(Said::PasskeyRefused),
    )
    .await
}

async fn sign_in(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let now = hub.now_ms();
    let address = address_of(hub, peer, headers);
    if !panel.allow(Kind::Codes, Some(counted_as(address, peer)), now) {
        return entry(
            hub,
            panel,
            headers,
            StatusCode::TOO_MANY_REQUESTS,
            Some(Said::TooManyCodes),
        )
        .await;
    }
    let typed = match http::form_field(headers, body, "code").await {
        Ok(typed) => typed,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    // One page for every refused code: wrong, spent, replaced or past its minutes. They are
    // the same bytes, so the answer never says which one it was.
    let refused = || entry(hub, panel, headers, StatusCode::OK, Some(Said::CodeRefused));
    let Some(code) = typed.as_deref().and_then(|typed| EntryCode::parse(typed)) else {
        return refused().await;
    };
    let digest = code.digest();
    let Some(organization) = db::organization_of_panel_entry(hub.pool(), &digest).await? else {
        return refused().await;
    };
    match repo::panel::sign_in(hub.pool(), hub.mark(), organization, &digest, address, now).await {
        Ok(SignIn::Opened(opened)) => {
            panel.opened(opened.id, now);
            let cookie = panel.set_cookie(&opened.secret.reveal(), SESSION_VALIDITY_MS / 1_000);
            // The session a code opens only reaches the registration of a passkey.
            Ok(http::with_cookie(http::see_other("/first-access"), &cookie))
        }
        Ok(SignIn::Refused) => refused().await,
        // The session may be open with the audit mark behind; typing the same code again closes
        // it and opens another (R7), so no new code is needed.
        Err(error) => security_failed(hub, organization, error).await,
    }
}

async fn sign_out(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    if let Some((organization, session)) = any_session_of(hub, panel, headers).await? {
        let address = client_ip(hub.client_ip_source(), peer, headers).ok();
        repo::panel::sign_out(hub.pool(), organization, session, address, hub.now_ms()).await?;
        panel.closed(session.id);
    }
    // The cookie goes either way: a browser that holds a dead one stops sending it.
    Ok(http::with_cookie(
        http::see_other("/sign-in"),
        &panel.set_cookie("", 0),
    ))
}

/// The window that checks the installation: a read, for a session that reaches the whole
/// panel. `?welcome` is right after the first passkey.
async fn installation(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<Response<Body>, RepoError> {
    let organization = match any_session_of(hub, panel, headers).await? {
        Some((organization, session)) if session.full => organization,
        Some(_) => return Ok(http::see_other("/first-access")),
        None => return Ok(http::see_other("/sign-in")),
    };
    let lang = lang_of(headers);
    let frame = data::frame(hub, organization, lang).await?;
    let hermes = data::hermes(hub, organization, lang).await?;
    // What only the running hub knows: who is reading, as it sees them, told from this
    // request alone.
    let peer_ip = canonical_ip(peer);
    let seen = client_ip(hub.client_ip_source(), peer, headers).unwrap_or(peer_ip);
    let reader = check::Reader {
        seen,
        peer: peer_ip,
        untrusted_forwarder: untrusted_forwarder(hub.client_ip_source(), peer, headers),
        no_trusted_proxy: no_trusted_proxy(hub.client_ip_source(), peer),
    };
    // The rest is read as `arauto-hub admin check` reads it.
    let facts = check::facts(
        check::Sources {
            pool: hub.pool(),
            organization,
            storage: hub.storage(),
            connector: panel.connector.as_ref(),
        },
        Some(panel.origin.clone()),
        Some(panel.hub_url.origin()),
        Some(reader),
    )
    .await?;
    let rows = check::rows(&facts);
    let welcome = query.is_some_and(|query| query.split('&').any(|pair| pair == "welcome"));
    Ok(http::page(
        StatusCode::OK,
        installation::page(&frame, &hermes, &rows, welcome),
    ))
}

fn canonical_ip(peer: SocketAddr) -> IpAddr {
    crate::channel::client_ip::canonical(peer.ip())
}

async fn page(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
    path: &str,
    query: Option<&str>,
) -> Result<Response<Body>, RepoError> {
    let organization = match any_session_of(hub, panel, headers).await? {
        Some((organization, session)) if session.full => organization,
        // A session opened by a code is sent to the one page it reaches.
        Some(_) => return Ok(http::see_other("/first-access")),
        None => return Ok(http::see_other("/sign-in")),
    };
    let lang = lang_of(headers);
    let frame = data::frame(hub, organization, lang).await?;
    let html = match path {
        "/whatsapps" => view::numbers(&frame, &data::numbers(hub, organization, lang).await?),
        "/accesses" => {
            let mut accesses = data::accesses(hub, organization, lang).await?;
            // After a save, the address names the access: the page says what it is now.
            accesses.saved = editor::saved_sentence(hub, organization, lang, query).await?;
            view::accesses(&frame, &accesses)
        }
        _ => view::hermes(&frame, &data::hermes(hub, organization, lang).await?),
    };
    Ok(http::page(StatusCode::OK, html))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(url: &str) -> PanelState {
        PanelState::new(
            &PublicUrl::parse(url).unwrap(),
            &PublicUrl::parse("https://hub.example.com").unwrap(),
            &Downloads::empty(),
        )
    }

    #[test]
    fn a_block_downloads_the_connector_the_hub_serves() {
        let serves = Downloads::empty()
            .with_file(
                "arauto-connector-linux-x86_64",
                bytes::Bytes::from_static(b"abc"),
            )
            .unwrap();
        let panel = PanelState::new(
            &PublicUrl::parse("http://localhost:8445").unwrap(),
            &PublicUrl::parse("https://hub.example.com").unwrap(),
            &serves,
        );
        let connector = panel.connector.unwrap();
        assert_eq!(connector.name, "arauto-connector-linux-x86_64");
        // The SHA-256 of the three bytes the hub serves under that name.
        assert_eq!(
            connector.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            panel.hub_url.channel_url(),
            "wss://hub.example.com/v1/channel"
        );
        // A program for Windows is no connector a block can chain.
        let windows_only = Downloads::empty()
            .with_file("arauto-connector-windows-x86_64.exe", bytes::Bytes::new())
            .unwrap();
        let panel = PanelState::new(
            &PublicUrl::parse("http://localhost:8445").unwrap(),
            &PublicUrl::parse("https://hub.example.com").unwrap(),
            &windows_only,
        );
        assert!(panel.connector.is_none());
    }

    #[test]
    fn the_cookie_of_the_session_is_bound_to_the_panel_and_hidden_from_scripts() {
        let over_tls = panel("https://panel.example.com");
        let cookie = over_tls.set_cookie("abc", 43_200);
        assert_eq!(
            cookie,
            "__Host-arauto-panel=abc; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200; Secure"
        );
        // Through a tunnel, on this machine, the same cookie without what only TLS can keep.
        let local = panel("http://localhost:8445");
        let cookie = local.set_cookie("abc", 43_200);
        assert_eq!(
            cookie,
            "arauto-panel=abc; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200"
        );
        assert_eq!(over_tls.origin, "https://panel.example.com");
        assert_eq!(local.authority, "localhost:8445");
    }

    #[test]
    fn a_session_is_served_while_it_is_used_and_forgotten_when_it_sits_idle() {
        let panel = panel("https://panel.example.com");
        let session = Uuid::new_v4();
        let start = 1_800_000_000_000;
        assert!(!panel.used(session, start), "this hub never saw it");
        panel.opened(session, start);
        assert!(panel.used(session, start + SESSION_IDLE_MS - 1));
        // Each use starts the count again.
        assert!(panel.used(session, start + 2 * SESSION_IDLE_MS - 2));
        // Exactly the time is too long: a deadline passed when the clock reached it.
        let last = start + 2 * SESSION_IDLE_MS - 2;
        assert!(!panel.used(session, last + SESSION_IDLE_MS));
        assert!(!panel.used(session, last + 1), "and it does not come back");
        // Closed, it is gone at once.
        panel.opened(session, start);
        panel.closed(session);
        assert!(!panel.used(session, start));
    }

    #[test]
    fn the_looks_at_entry_codes_have_a_ceiling_per_address_and_one_for_everyone() {
        let panel = panel("https://panel.example.com");
        let start = 1_800_000_000_000;
        let address = Some("198.51.100.1".parse().unwrap());
        for _ in 0..attempts::PER_ADDRESS_PER_MINUTE {
            assert!(panel.allow(Kind::Codes, address, start));
        }
        assert!(!panel.allow(Kind::Codes, address, start));
        // Another address is not stopped by it.
        assert!(panel.allow(Kind::Codes, Some("198.51.100.2".parse().unwrap()), start));
    }

    #[test]
    fn the_panel_serves_one_organization_or_none() {
        let (one, other) = (Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(served_organization(Ok(vec![one])), Some(one));
        assert_eq!(served_organization::<Uuid>(Ok(vec![])), None);
        assert_eq!(served_organization(Ok(vec![one, other])), None);
        assert_eq!(
            served_organization::<Uuid>(Err(db::DbError::Query(sqlx::Error::PoolTimedOut))),
            None
        );
    }

    #[test]
    fn the_files_of_the_panel_are_a_fixed_list() {
        let names: Vec<&str> = ASSETS.iter().map(|(name, _, _)| *name).collect();
        assert_eq!(
            names,
            [
                "panel.css",
                "panel.js",
                "poppins-400.woff2",
                "poppins-600.woff2",
                "poppins-800.woff2",
                "atkinson-next.woff2",
                "atkinson-mono-400.woff2"
            ]
        );
        let sheet = std::str::from_utf8(ASSETS[0].2).unwrap();
        // The script helps the screen and nothing else: it writes no markup, runs no text,
        // reads no cookie and calls no address but the one the page of the panel names for
        // the challenge of a passkey.
        let script = std::str::from_utf8(ASSETS[1].2).unwrap();
        assert_eq!(ASSETS[1].1, "text/javascript; charset=utf-8");
        for forbidden in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "eval(",
            "Function(",
            "document.cookie",
            "XMLHttpRequest",
            "WebSocket",
            "sendBeacon",
            "localStorage",
            "http://",
            "https://",
            "location",
        ] {
            assert!(
                !script.contains(forbidden),
                "the script of the panel has {forbidden}"
            );
        }
        // A dialog the person closed, or one that took too long, is not a refusal: the script
        // says the text of the form for it.
        assert!(script.contains(
            "say(name === 'NotAllowedError' || name === 'AbortError' ? 'cancelled' : 'failed');"
        ));
        // One call, to an address written by the hub in the page, with nothing in it.
        assert_eq!(script.matches("fetch(").count(), 1);
        assert!(script.contains(
            "fetch(form.getAttribute('data-start'), { method: 'POST', credentials: 'same-origin' })"
        ));
        for (name, kind, bytes) in ASSETS.iter().skip(2) {
            assert_eq!(*kind, "font/woff2");
            assert!(bytes.starts_with(b"wOF2"), "{name}");
            assert!(
                sheet.contains(&format!("/assets/{name}")),
                "{name} is used by the sheet"
            );
        }
        // The sheet reaches for nothing outside the panel.
        assert!(!sheet.contains("http://") && !sheet.contains("https://"));
        assert!(!sheet.contains("@import"));
    }
}
