//! The canary of the secrets of the panel (`docs/15-mvp.md`, section 7;
//! `docs/decisoes/0040-painel-dentro-do-hub.md`): entry codes are issued, typed wrong and
//! right, sessions are opened, used, closed and left to end, with every log of the process
//! captured. No entry code and no identifier of a session may appear in the log, in the
//! errors the log carries, in the audit, in the alerts or in the rows of the panel.
//!
//! A binary of its own: the log of the process is captured from its first line, at every
//! level, and no other test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::repo::panel::SESSION_IDLE_MS;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};
use data_encoding::{BASE32_NOPAD, BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};

const PATIENCE: Duration = Duration::from_secs(15);

/// Every line of the log of the process.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// The ways a secret of bytes can be written down.
fn spellings(bytes: &[u8]) -> Vec<String> {
    vec![
        HEXLOWER.encode(bytes),
        HEXUPPER.encode(bytes),
        BASE64.encode(bytes),
        BASE64_NOPAD.encode(bytes),
        BASE64URL_NOPAD.encode(bytes),
        BASE32_NOPAD.encode(bytes),
        BASE32_NOPAD.encode(bytes).to_ascii_lowercase(),
        format!("{bytes:?}"),
    ]
}

/// Where a secret shows up in `text`, if anywhere.
fn leaks(text: &str, secrets: &[(String, String)]) -> Vec<String> {
    secrets
        .iter()
        .filter(|(_, written)| text.contains(written.as_str()))
        .map(|(what, _)| what.clone())
        .collect()
}

#[test]
fn the_detector_sees_a_planted_secret() {
    let secret = [0x5au8; 32];
    for written in spellings(&secret) {
        let secrets = vec![("planted".to_owned(), written.clone())];
        assert_eq!(leaks(&format!("x {written} y"), &secrets), ["planted"]);
    }
}

/// An entry code, as the command shows it, with every way it could have been written.
async fn code(fixture: &Fixture, what: &str, secrets: &mut Vec<(String, String)>) -> String {
    let issued = fixture.admin.panel_code(fixture.now()).await.unwrap();
    let shown = issued.code.reveal().to_string();
    let plain = shown.replace('-', "");
    let bytes = BASE32_NOPAD
        .decode(plain.to_ascii_uppercase().as_bytes())
        .unwrap();
    assert_eq!(bytes.len(), 20);
    secrets.push((what.to_owned(), shown.clone()));
    secrets.push((what.to_owned(), shown.to_ascii_uppercase()));
    for written in spellings(&bytes) {
        secrets.push((what.to_owned(), written));
    }
    // The first groups alone would already help whoever guesses.
    secrets.push((format!("{what} (its start)"), shown[..14].to_owned()));
    shown
}

async fn post(fixture: &Fixture, path: &str, cookie: Option<&str>, body: &str) -> Response {
    let mut headers = vec![
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ];
    if let Some(cookie) = cookie {
        headers.push(("Cookie", cookie));
    }
    let panel = fixture.running().panel_addr().unwrap();
    http::post(panel, path, &headers, body.as_bytes(), PATIENCE)
        .await
        .unwrap()
}

async fn get(fixture: &Fixture, path: &str, cookie: Option<&str>) -> Response {
    let mut headers = vec![("Host", PANEL_HOST)];
    if let Some(cookie) = cookie {
        headers.push(("Cookie", cookie));
    }
    let panel = fixture.running().panel_addr().unwrap();
    http::get(panel, path, &headers, PATIENCE).await.unwrap()
}

/// Signs in and returns the cookie, with every way its identifier could have been written.
async fn sign_in(
    fixture: &Fixture,
    code: &str,
    what: &str,
    secrets: &mut Vec<(String, String)>,
) -> String {
    let answer = post(fixture, "/sign-in", None, &format!("code={code}")).await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    let cookie = answer
        .header("set-cookie")
        .and_then(|set| set.split(';').next())
        .unwrap()
        .to_owned();
    let identifier = cookie.split_once('=').unwrap().1;
    let bytes = BASE64URL_NOPAD.decode(identifier.as_bytes()).unwrap();
    assert_eq!(bytes.len(), 32);
    for written in spellings(&bytes) {
        secrets.push((what.to_owned(), written));
    }
    secrets.push((format!("{what} (its start)"), identifier[..16].to_owned()));
    cookie
}

#[tokio::test(flavor = "multi_thread")]
async fn no_secret_of_the_panel_reaches_the_log_the_audit_or_the_rows() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let clock = ManualClock::new(SystemClock.now_ms());
    let mut fixture = Fixture::with_clock(fixture::settings(), Arc::new(clock.clone())).await;
    let mut secrets: Vec<(String, String)> = Vec::new();

    // A code that is cancelled by the next one, typed after it died; a code typed wrong, in
    // an address, by a request that is not of the panel; and then typed right.
    let cancelled = code(&fixture, "cancelled code", &mut secrets).await;
    let entry = code(&fixture, "entry code", &mut secrets).await;
    assert_eq!(
        post(&fixture, "/sign-in", None, &format!("code={cancelled}"))
            .await
            .status,
        200
    );
    let mut wrong = entry.clone();
    wrong.replace_range(0..1, if entry.starts_with('a') { "b" } else { "a" });
    secrets.push(("mistyped code".to_owned(), wrong[1..].to_owned()));
    assert_eq!(
        post(&fixture, "/sign-in", None, &format!("code={wrong}"))
            .await
            .status,
        200
    );
    let in_the_address = format!("/sign-in?code={entry}");
    assert_eq!(get(&fixture, &in_the_address, None).await.status, 200);
    let panel = fixture.running().panel_addr().unwrap();
    let elsewhere = [("Host", PANEL_HOST), ("Origin", "http://evil.test")];
    let refused = http::post(
        panel,
        "/sign-in",
        &elsewhere,
        format!("code={entry}").as_bytes(),
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(refused.status, 403);
    let not_a_form = [
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "text/plain"),
    ];
    let refused = http::post(
        panel,
        "/sign-in",
        &not_a_form,
        format!("code={entry}").as_bytes(),
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(refused.status, 415);

    // A session used, then closed by its owner.
    let first = sign_in(&fixture, &entry, "session", &mut secrets).await;
    for path in ["/", "/whatsapps", "/accesses", "/sign-in", "/nothing"] {
        get(&fixture, path, Some(&first)).await;
    }
    assert_eq!(
        post(&fixture, "/sign-out", Some(&first), "").await.status,
        303
    );
    assert_eq!(get(&fixture, "/", Some(&first)).await.status, 303);

    // A session left idle, presented again; and one that the stop of the hub ends.
    let another = code(&fixture, "second code", &mut secrets).await;
    let idle = sign_in(&fixture, &another, "idle session", &mut secrets).await;
    clock.advance(SESSION_IDLE_MS);
    assert_eq!(get(&fixture, "/", Some(&idle)).await.status, 303);
    let third = code(&fixture, "third code", &mut secrets).await;
    let kept = sign_in(&fixture, &third, "session across a stop", &mut secrets).await;
    // A code that waits while the hub stops, and is typed after.
    let waiting = code(&fixture, "waiting code", &mut secrets).await;
    fixture.stop().await;
    fixture.start().await;
    assert_eq!(get(&fixture, "/", Some(&kept)).await.status, 303);
    let last = sign_in(&fixture, &waiting, "last session", &mut secrets).await;
    assert_eq!(get(&fixture, "/", Some(&last)).await.status, 200);
    // One more is issued and never typed: its digest stays in the database.
    code(&fixture, "untyped code", &mut secrets).await;

    let log = captured.text();
    assert!(
        log.len() > 1_000,
        "the log is captured (it has {} bytes)",
        log.len()
    );
    let mut stored = String::new();
    for table in ["panel_entry", "panel_session", "audit", "alert"] {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t"
        )))
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
        stored.push_str(&rows.join("\n"));
        stored.push('\n');
    }
    for action in ["panel.code_issued", "panel.signed_in", "panel.signed_out"] {
        assert!(stored.contains(action), "{action} is in the audit");
    }
    for (place, text) in [("log", &log), ("the database", &stored)] {
        let found = leaks(text, &secrets);
        assert!(found.is_empty(), "{place} holds: {found:?}");
    }
}
