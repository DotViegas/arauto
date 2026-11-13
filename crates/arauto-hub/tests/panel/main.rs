//! The panel of the admin, against a hub of its own and the test PostgreSQL: the port, the
//! entry by a code of one use, the session and its two deadlines, what a request that reads
//! may not do, where the secrets are not, and the three pages.
//!
//! The tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::time::Duration;

use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};

#[allow(dead_code, unused_imports)]
#[path = "../whatsapp/world.rs"]
mod world;

mod access;
mod connect;
mod entry;
mod pages;
mod session;

/// How long a request of a test may take.
pub const PATIENCE: Duration = Duration::from_secs(15);

/// The name of the cookie of the session, on a panel reached without TLS on this machine.
pub const COOKIE: &str = "arauto-panel";

/// The sentence of every refusal of the entry.
pub const REFUSED: &str = "Invalid or expired code";

/// A browser of a test, pointed at the panel of a hub.
#[derive(Debug, Clone, Copy)]
pub struct Browser {
    pub panel: SocketAddr,
}

impl Browser {
    pub fn of(fixture: &Fixture) -> Self {
        Self {
            panel: fixture
                .running()
                .panel_addr()
                .expect("the hub of the tests has a panel"),
        }
    }

    /// `GET path`, as a browser at the address of the panel sends it.
    pub async fn get(&self, path: &str, cookie: Option<&str>) -> Response {
        let mut headers = vec![("Host", PANEL_HOST)];
        if let Some(cookie) = cookie {
            headers.push(("Cookie", cookie));
        }
        http::get(self.panel, path, &headers, PATIENCE)
            .await
            .unwrap()
    }

    /// `POST path` of a form, as a page of the panel sends it.
    pub async fn post(&self, path: &str, cookie: Option<&str>, body: &str) -> Response {
        let mut headers = vec![
            ("Host", PANEL_HOST),
            ("Origin", PANEL_ORIGIN),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ];
        if let Some(cookie) = cookie {
            headers.push(("Cookie", cookie));
        }
        http::post(self.panel, path, &headers, body.as_bytes(), PATIENCE)
            .await
            .unwrap()
    }

    /// Types an entry code. Returns the answer.
    pub async fn enter(&self, code: &str) -> Response {
        self.post("/sign-in", None, &format!("code={code}")).await
    }

    /// Signs in with a fresh code and returns the cookie, as `name=value`.
    pub async fn signed_in(&self, fixture: &Fixture) -> String {
        let answer = self.enter(&code(fixture).await).await;
        assert_eq!(answer.status, 303, "{}", answer.text());
        cookie_of(&answer).expect("the entry sets the cookie of the session")
    }
}

/// `arauto-hub admin panel code`: the code, as the command shows it.
pub async fn code(fixture: &Fixture) -> String {
    fixture
        .admin
        .panel_code(fixture.now())
        .await
        .unwrap()
        .code
        .reveal()
        .to_string()
}

/// The `name=value` of the cookie an answer sets, when it sets one with a value.
pub fn cookie_of(answer: &Response) -> Option<String> {
    let set = answer.header("set-cookie")?;
    let pair = set.split(';').next()?.trim();
    let (name, value) = pair.split_once('=')?;
    (name == COOKIE && !value.is_empty()).then(|| pair.to_owned())
}

/// How many events of one action the audit has.
pub async fn audited(fixture: &Fixture, action: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit WHERE action = $1")
        .bind(action)
        .fetch_one(&fixture.pool)
        .await
        .unwrap()
}

/// Every row of a table, as the text PostgreSQL makes of it.
pub async fn rows(fixture: &Fixture, table: &str) -> Vec<String> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT row_to_json(t)::text FROM {table} t ORDER BY 1"
    )))
    .fetch_all(&fixture.pool)
    .await
    .unwrap()
}

pub async fn hub() -> Fixture {
    Fixture::new(fixture::settings()).await
}
