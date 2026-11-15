//! The canary of the pairing in the panel (`docs/decisoes/0044-numeros-no-painel-sobre-o-motor.md`):
//! the QR and the pairing code are short-lived secrets that the panel shows to the session that
//! started the pairing and to nobody else. A number is added and paired from the panel by QR and
//! by code, one pairing is cancelled and one expires, with the log of the process captured from
//! its first line at every level. No QR value and no pairing code may appear in the log, in any
//! row of the database or in a page that is not the window of its own pairing, and every answer
//! that drew a QR or showed a code is `no-store`.
//!
//! A binary of its own: the log of the process is captured from its first line, and no other
//! test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[allow(dead_code, unused_imports)]
#[path = "channel/support.rs"]
mod support;

use arauto_testkit::hub::fixture;

#[allow(dead_code)]
#[path = "whatsapp/world.rs"]
mod world;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_protocol::clock::SystemClock;
use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;
use data_encoding::{BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};

use crate::world::{World, phone};

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

/// The ways a value can be written down.
fn spellings(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    vec![
        text.to_owned(),
        text.to_ascii_lowercase(),
        text.to_ascii_uppercase(),
        HEXLOWER.encode(bytes),
        HEXUPPER.encode(bytes),
        BASE64.encode(bytes),
        BASE64_NOPAD.encode(bytes),
        BASE64URL_NOPAD.encode(bytes),
        format!("{bytes:?}"),
        format!("{text:?}"),
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
    for written in spellings("2@planted-qr-value,session") {
        let secrets = vec![("planted".to_owned(), written.clone())];
        assert_eq!(leaks(&format!("x {written} y"), &secrets), ["planted"]);
    }
    assert!(
        leaks(
            "nothing here",
            &[("planted".to_owned(), "ABCD-EFGH".to_owned())]
        )
        .is_empty()
    );
}

/// A value as a form field carries it.
fn form_value(text: &str) -> String {
    text.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

struct Panel {
    addr: std::net::SocketAddr,
    cookie: String,
}

impl Panel {
    async fn get(&self, path: &str) -> Response {
        let headers = vec![("Host", PANEL_HOST), ("Cookie", self.cookie.as_str())];
        http::get(self.addr, path, &headers, PATIENCE)
            .await
            .unwrap()
    }

    async fn post(&self, path: &str, body: &str) -> Response {
        let headers = vec![
            ("Host", PANEL_HOST),
            ("Origin", PANEL_ORIGIN),
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Cookie", self.cookie.as_str()),
        ];
        http::post(self.addr, path, &headers, body.as_bytes(), PATIENCE)
            .await
            .unwrap()
    }
}

/// A session that reaches the whole panel: an entry code, then a passkey.
async fn signed_in(world: &World) -> Panel {
    let addr = world.fixture.running().panel_addr().unwrap();
    let issued = world.fixture.admin.panel_code(world.now()).await.unwrap();
    let code = issued.code.reveal().to_string();
    let headers = vec![
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ];
    let answer = http::post(
        addr,
        "/sign-in",
        &headers,
        format!("code={code}").as_bytes(),
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 303);
    let cookie = answer
        .header("set-cookie")
        .and_then(|set| set.split(';').next())
        .unwrap()
        .to_owned();
    let panel = Panel { addr, cookie };
    let asked = panel.post("/passkey/register/start", "").await;
    assert_eq!(asked.status, 200, "{}", asked.text());
    let made = SoftAuthenticator::new(PANEL_ORIGIN)
        .create(&asked.text())
        .unwrap();
    let kept = panel
        .post(
            "/passkey/register",
            &format!("credential={}", form_value(&made)),
        )
        .await;
    assert_eq!(kept.status, 303, "{}", kept.text());
    panel
}

/// Every table of the hub, every row, as the text PostgreSQL makes of it.
async fn everything(world: &World) -> String {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = current_schema() ORDER BY 1",
    )
    .fetch_all(&world.fixture.pool)
    .await
    .unwrap();
    let mut all = String::new();
    for table in tables {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t"
        )))
        .fetch_all(&world.fixture.pool)
        .await
        .unwrap();
        for row in rows {
            all.push_str(&row);
            all.push('\n');
        }
    }
    all
}

async fn until(panel: &Panel, path: &str, text: &str, pages: &mut Vec<Response>) -> String {
    for _ in 0..200 {
        let answer = panel.get(path).await;
        let page = answer.text();
        pages.push(answer);
        if page.contains(text) {
            return page;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("never showed {text}");
}

fn run_of(answer: &Response) -> String {
    answer
        .header("location")
        .and_then(|location| location.split("run=").nth(1))
        .expect("the answer leads to the window of the run")
        .to_owned()
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn no_qr_or_pairing_code_reaches_the_log_the_audit_the_rows_or_a_stored_answer() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let world = World::with(
        |settings| {
            settings.engine.pairing_ms = 2_500;
            settings.engine.pairing_poll_ms = 40;
        },
        Arc::new(SystemClock),
    )
    .await;
    let panel = signed_in(&world).await;
    let mut pages: Vec<Response> = Vec::new();
    let mut secrets: Vec<(String, String)> = Vec::new();

    // Four numbers, from the form of the panel.
    for (n, alias) in ["qr", "code", "cancelled", "expired"]
        .into_iter()
        .enumerate()
    {
        let accept = if n == 0 { "&accept=on" } else { "" };
        let answer = panel
            .post(
                "/whatsapps/add",
                &format!(
                    "alias={alias}&phone=%2B{}{accept}",
                    phone(110 + u32::try_from(n).unwrap())
                ),
            )
            .await;
        assert_eq!(answer.status, 303, "{}", answer.text());
        let number = world.number(alias).await;
        // Every QR value the fake can serve to this session, and the code.
        for served in 1..=400 {
            let value = format!("2@fake-qr-value-{served},{}", number.session.as_str());
            for written in spellings(&value) {
                secrets.push((format!("QR value of {alias}"), written));
            }
        }
    }
    for written in spellings("ABCD-EFGH") {
        secrets.push(("pairing code".to_owned(), written));
    }
    secrets.push(("pairing code".to_owned(), "ABCDEFGH".to_owned()));

    // By QR: several values are served while the window waits, then the phone scans.
    let qr = world.number("qr").await;
    let started = panel
        .post("/whatsapps/pair/start", "alias=qr&method=qr")
        .await;
    assert_eq!(started.status, 303, "{}", started.text());
    let path = format!("/whatsapps/pair/wait?alias=qr&run={}", run_of(&started));
    until(&panel, &path, "<svg class=\"qr\"", &mut pages).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    world.fake.pairs_as(qr.session.as_str(), &phone(110));
    until(&panel, &path, "Paired:", &mut pages).await;

    // By code.
    let code = world.number("code").await;
    world.fake.pairs_as(code.session.as_str(), &phone(111));
    let started = panel
        .post("/whatsapps/pair/start", "alias=code&method=code")
        .await;
    assert_eq!(started.status, 303, "{}", started.text());
    let path = format!("/whatsapps/pair/wait?alias=code&run={}", run_of(&started));
    until(&panel, &path, "Paired:", &mut pages).await;

    // Cancelled while the QR is on the window.
    let started = panel
        .post("/whatsapps/pair/start", "alias=cancelled&method=qr")
        .await;
    let run = run_of(&started);
    let path = format!("/whatsapps/pair/wait?alias=cancelled&run={run}");
    until(&panel, &path, "<svg class=\"qr\"", &mut pages).await;
    let cancel = panel
        .post(
            "/whatsapps/pair/cancel",
            &format!("alias=cancelled&run={run}"),
        )
        .await;
    assert_eq!(cancel.status, 303);
    pages.push(panel.get(&path).await);

    // Nobody scans: it expires.
    let started = panel
        .post("/whatsapps/pair/start", "alias=expired&method=code")
        .await;
    let path = format!(
        "/whatsapps/pair/wait?alias=expired&run={}",
        run_of(&started)
    );
    until(
        &panel,
        &path,
        "Nothing was scanned or typed in time",
        &mut pages,
    )
    .await;
    pages.push(panel.get("/whatsapps").await);

    // The log.
    let log = captured.text();
    assert!(
        log.len() > 1_000,
        "the log was captured ({} bytes)",
        log.len()
    );
    let found = leaks(&log, &secrets);
    assert!(found.is_empty(), "the log holds: {found:?}");
    // The rows of every table: the audit, the alerts, the numbers, the inbox, the panel.
    let rows = everything(&world).await;
    assert!(
        rows.contains("number.pairing_started"),
        "the audit was read"
    );
    let found = leaks(&rows, &secrets);
    assert!(found.is_empty(), "the database holds: {found:?}");
    // Every answer the panel gave: never the value as text, never stored by anything on the way.
    let mut drew = 0;
    for answer in &pages {
        let text = answer.text();
        assert!(
            leaks(&text, &secrets[..secrets.len() - 11]).is_empty(),
            "a page wrote a QR value as text"
        );
        if text.contains("<svg class=\"qr\"") || text.contains("ABCD-EFGH") {
            drew += 1;
            assert!(
                answer
                    .header("cache-control")
                    .is_some_and(|value| value.contains("no-store")),
                "an answer with a QR or a code is stored"
            );
        } else {
            assert!(!text.contains("ABCD-EFGH"));
        }
    }
    assert!(drew >= 3, "the pages that drew a QR were checked ({drew})");
}
