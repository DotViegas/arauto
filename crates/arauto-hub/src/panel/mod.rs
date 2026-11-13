//! The panel of the admin: pages served by the hub itself, on a port of their own
//! (`docs/decisoes/0040-painel-dentro-do-hub.md`, `docs/07-telas.md`).
//!
//! | Address | What it answers |
//! |---|---|
//! | `GET /sign-in` | the page where the entry code is typed |
//! | `POST /sign-in` | spends the code and opens a session |
//! | `POST /sign-out` | closes the session |
//! | `GET /` | the Hermes |
//! | `GET /whatsapps` | the numbers |
//! | `GET /accesses` | the matrix of accesses |
//! | `GET /assets/{file}` | the sheet and the fonts of the panel |
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
//! - **Every value is escaped** ([`html`]), and no script runs: the pages have none.
//!
//! This first delivery only reads. What it shows is what `arauto-hub admin` shows, through
//! the same repository functions.

mod data;
mod html;
mod http;
pub mod secret;
pub mod view;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use arauto_protocol::limits::MINUTE_MS;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, Response, StatusCode, header};
use dashmap::DashMap;
use hyper::body::Incoming;
use uuid::Uuid;

use self::secret::{EntryCode, SessionSecret};
use crate::audit::AuditError;
use crate::channel::Hub;
use crate::channel::client_ip::client_ip;
use crate::config::PublicUrl;
use crate::db::{self, ResolvedOrg};
use crate::repo::panel::{PanelSession, SESSION_IDLE_MS, SESSION_VALIDITY_MS, SignIn};
use crate::repo::{self, RepoError};

/// How many times a minute the hub looks an entry code up, from everyone together. A code
/// has 160 bits: this does not protect the code, it keeps a flood away from the database.
const ATTEMPTS_PER_MINUTE: u32 = 30;

/// What the page of the entry says when it refuses. One sentence for every reason.
const REFUSED: &str = "Código inválido ou vencido. Gere outro no servidor do hub.";

const TOO_MANY: &str = "Muitas tentativas agora. Espere um minuto e tente de novo.";

/// The files of the panel, written by the project and built into the program.
const ASSETS: &[(&str, &str, &[u8])] = &[
    (
        "panel.css",
        "text/css; charset=utf-8",
        include_bytes!("assets/panel.css"),
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
    /// The minute being counted and how many entry codes were looked up in it.
    attempts: Mutex<(i64, u32)>,
}

impl PanelState {
    /// The panel reached at `url`.
    #[must_use]
    pub fn new(url: &PublicUrl) -> Self {
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
            attempts: Mutex::new((0, 0)),
        }
    }

    /// Counts one look at an entry code. `false` when this minute already had its share.
    fn attempt(&self, now: i64) -> bool {
        let mut window = self.attempts.lock().unwrap_or_else(PoisonError::into_inner);
        if now.saturating_sub(window.0) >= MINUTE_MS || now < window.0 {
            *window = (now, 0);
        }
        if window.1 >= ATTEMPTS_PER_MINUTE {
            return false;
        }
        window.1 = window.1.saturating_add(1);
        true
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

/// Something of the hub failed while a page was read or a form was handled.
fn failed(error: &RepoError) -> Response<Body> {
    tracing::error!(%error, "the panel could not answer");
    http::plain(
        StatusCode::INTERNAL_SERVER_ERROR,
        "The hub could not answer. See its log.\n",
    )
}

/// The session a request brings, when it is one this hub serves: open, in its hours, and
/// used not long ago. Only reads the database.
async fn session_of(
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
            "This is not the address of the panel.\n",
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
            "This request did not come from a page of the panel.\n",
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
        (false, "/sign-in") => sign_in(&hub, panel, peer, &parts.headers, body).await,
        (false, "/sign-out") => sign_out(&hub, panel, peer, &parts.headers).await,
        (true, "/" | "/whatsapps" | "/accesses") => page(&hub, panel, &parts.headers, path).await,
        _ => return http::plain(StatusCode::NOT_FOUND, "not found\n"),
    };
    answer.unwrap_or_else(|error| failed(&error))
}

async fn sign_in_page(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    if session_of(hub, panel, headers).await?.is_some() {
        return Ok(http::see_other("/"));
    }
    Ok(http::page(StatusCode::OK, view::sign_in(None)))
}

async fn sign_in(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
    body: Incoming,
) -> Result<Response<Body>, RepoError> {
    let now = hub.now_ms();
    if !panel.attempt(now) {
        return Ok(http::page(
            StatusCode::TOO_MANY_REQUESTS,
            view::sign_in(Some(TOO_MANY)),
        ));
    }
    let typed = match http::form_field(headers, body, "code").await {
        Ok(typed) => typed,
        Err(error) => return Ok(http::plain(error.status(), "The form was not read.\n")),
    };
    let refused = || http::page(StatusCode::OK, view::sign_in(Some(REFUSED)));
    let Some(code) = typed.as_deref().and_then(|typed| EntryCode::parse(typed)) else {
        return Ok(refused());
    };
    let digest = code.digest();
    let Some(organization) = db::organization_of_panel_entry(hub.pool(), &digest).await? else {
        return Ok(refused());
    };
    let address = client_ip(hub.client_ip_source(), peer, headers).ok();
    match repo::panel::sign_in(hub.pool(), hub.mark(), organization, &digest, address, now).await {
        Ok(SignIn::Opened(opened)) => {
            panel.opened(opened.id, now);
            let cookie = panel.set_cookie(&opened.secret.reveal(), SESSION_VALIDITY_MS / 1_000);
            Ok(http::with_cookie(http::see_other("/"), &cookie))
        }
        Ok(SignIn::Refused) => Ok(refused()),
        Err(error) => {
            // The session may be open with the audit mark behind: the hub says so in an
            // alert, as for every security write, and the admin asks for another code.
            if matches!(error, RepoError::Audit(AuditError::MarkNotAdvanced { .. })) {
                let shown = failed(&error);
                hub.security_write_failed(organization, None, error).await;
                return Ok(shown);
            }
            Err(error)
        }
    }
}

async fn sign_out(
    hub: &Hub,
    panel: &PanelState,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<Response<Body>, RepoError> {
    if let Some((organization, session)) = session_of(hub, panel, headers).await? {
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

async fn page(
    hub: &Hub,
    panel: &PanelState,
    headers: &HeaderMap,
    path: &str,
) -> Result<Response<Body>, RepoError> {
    let Some((organization, _)) = session_of(hub, panel, headers).await? else {
        return Ok(http::see_other("/sign-in"));
    };
    let frame = data::frame(hub, organization).await?;
    let html = match path {
        "/whatsapps" => view::numbers(&frame, &data::numbers(hub, organization).await?),
        "/accesses" => view::accesses(&frame, &data::accesses(hub, organization).await?),
        _ => view::hermes(&frame, &data::hermes(hub, organization).await?),
    };
    Ok(http::page(StatusCode::OK, html))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(url: &str) -> PanelState {
        PanelState::new(&PublicUrl::parse(url).unwrap())
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
    fn the_looks_at_entry_codes_have_a_ceiling_per_minute() {
        let panel = panel("https://panel.example.com");
        let start = 1_800_000_000_000;
        for _ in 0..ATTEMPTS_PER_MINUTE {
            assert!(panel.attempt(start));
        }
        assert!(!panel.attempt(start));
        assert!(!panel.attempt(start + MINUTE_MS - 1));
        assert!(
            panel.attempt(start + MINUTE_MS),
            "a new minute, a new share"
        );
        // A clock that went back does not lock the entry forever.
        assert!(panel.attempt(start - 10 * MINUTE_MS));
    }

    #[test]
    fn the_files_of_the_panel_are_a_fixed_list() {
        let names: Vec<&str> = ASSETS.iter().map(|(name, _, _)| *name).collect();
        assert_eq!(
            names,
            [
                "panel.css",
                "poppins-400.woff2",
                "poppins-600.woff2",
                "poppins-800.woff2",
                "atkinson-next.woff2",
                "atkinson-mono-400.woff2"
            ]
        );
        let sheet = std::str::from_utf8(ASSETS[0].2).unwrap();
        for (name, kind, bytes) in ASSETS.iter().skip(1) {
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
