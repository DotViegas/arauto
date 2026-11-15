//! The canary of the org chart (`docs/decisoes/0043-organograma-e-niveis.md`, rule S6): a role
//! and a level name that carry a planted word go through a publish, a stale publish, an invalid
//! publish, a failure of the hub and the command that shows the chart, while two Hermes in
//! session call every peer tool and one of them gets a notice. The word is in no line of the
//! log (every level, captured from the first line), in no message the connectors received, in
//! no column of the audit and in no alert. It is in the rows of the chart, so the detector sees.
//!
//! A binary of its own: the log of the process is captured from its first line, and no other
//! test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::admin::render::org as render;
use arauto_protocol::message::{CallOutcome, Message};
use arauto_testkit::KitError;
use arauto_testkit::device::SessionClient;
use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;
use data_encoding::BASE32_NOPAD;
use serde_json::{Value, json};

const PATIENCE: Duration = Duration::from_secs(15);

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

async fn get(fixture: &Fixture, path: &str, cookie: &str) -> Response {
    let headers = vec![("Host", PANEL_HOST), ("Cookie", cookie)];
    let panel = fixture.running().panel_addr().unwrap();
    http::get(panel, path, &headers, PATIENCE).await.unwrap()
}

/// A session of the panel that reaches all of it.
async fn signed_in(fixture: &Fixture) -> String {
    let code = fixture
        .admin
        .panel_code(fixture.now())
        .await
        .unwrap()
        .code
        .reveal()
        .to_string();
    let answer = post(fixture, "/sign-in", None, &format!("code={code}")).await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    let cookie = answer
        .header("set-cookie")
        .and_then(|set| set.split(';').next())
        .unwrap()
        .to_owned();
    let asked = post(fixture, "/passkey/register/start", Some(&cookie), "").await;
    let made = SoftAuthenticator::new(PANEL_ORIGIN)
        .create(&asked.text())
        .unwrap();
    let kept = post(
        fixture,
        "/passkey/register",
        Some(&cookie),
        &format!("credential={}", form_value(&made)),
    )
    .await;
    assert_eq!(kept.status, 303, "{}", kept.text());
    cookie
}

fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let start = text.find(from).unwrap() + from.len();
    let rest = &text[start..];
    &rest[..rest.find(to).unwrap()]
}

/// What a connector received, as text: every byte of an answer, a notice and a list of tools.
fn written(message: Message) -> String {
    match message {
        Message::Result(result) => match result.outcome {
            CallOutcome::Ok(content) => {
                serde_json::to_string(&Value::Object(content.into_map())).unwrap()
            }
            CallOutcome::Refused { code, .. } => format!("refused {code}"),
        },
        Message::Notice(notice) => serde_json::to_string(&notice).unwrap(),
        Message::Tools(tools) => tools
            .tools
            .as_slice()
            .iter()
            .map(|tool| tool.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(","),
        other => format!("{:?}", other.kind()),
    }
}

async fn call(
    client: &mut SessionClient,
    seen: &mut Vec<String>,
    id: u64,
    tool: &str,
    args: Value,
) {
    client.send_call(id, tool, args).await.unwrap();
    loop {
        match client.pump(PATIENCE).await {
            Ok(Some(Message::Result(result))) if result.id.get() == id => {
                seen.push(written(Message::Result(result)));
                return;
            }
            Ok(Some(message)) => seen.push(written(message)),
            Ok(None) => {}
            Err(error) => panic!("no answer to {tool}: {error:?}"),
        }
    }
}

async fn listen(client: &mut SessionClient, seen: &mut Vec<String>, how_long: Duration) {
    let until = tokio::time::Instant::now() + how_long;
    loop {
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return;
        }
        match client.pump(left).await {
            Ok(Some(message)) => seen.push(written(message)),
            Ok(None) | Err(KitError::TimedOut) => {}
            Err(error) => panic!("{error:?}"),
        }
    }
}

#[test]
fn the_detector_sees_a_planted_word() {
    let canary = "zq-role-canary-0a1b";
    let heard = ["x".to_owned(), format!("a {canary} b")];
    assert!(heard.iter().any(|text| text.contains(canary)));
    assert!(
        !heard
            .iter()
            .any(|text| text.contains("zq-role-canary-zzzz"))
    );
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn no_role_or_level_name_reaches_the_channel_a_notice_a_tool_or_the_log() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let mut random = [0u8; 6];
    getrandom::fill(&mut random).unwrap();
    let suffix = data_encoding::HEXLOWER.encode(&random);
    let role = format!("zq-role-canary-{suffix}");
    let level = format!("zq-level-{suffix}");
    let long_role = format!("{role} {}", BASE32_NOPAD.encode(b"padding"));
    // The text of a note carries a canary of its own: it may only reach the reading of B.
    getrandom::fill(&mut random).unwrap();
    let note = format!("zq-note-{}", data_encoding::HEXLOWER.encode(&random));

    let fixture = Fixture::new(fixture::settings()).await;
    let cookie = signed_in(&fixture).await;
    let (a, mut a_client) = fixture.connected().await;
    let (b, mut b_client) = fixture.connected().await;
    let mut heard: Vec<String> = Vec::new();
    heard.extend(
        a_client
            .session
            .tools
            .as_slice()
            .iter()
            .map(|t| t.as_str().to_owned()),
    );

    // The page, and a publish that carries both words and a link that wakes.
    let page = get(&fixture, "/org", &cookie).await.text();
    let version = between(&page, "id=\"org-data\" data-version=\"", "\"").to_owned();
    let draft = |role: &str| {
        json!({
            "levels": [ { "key": "new-1", "name": level } ],
            "hermes": [
                { "name": a, "level": "new-1", "x": 10, "role": role },
                { "name": b, "level": "new-1", "x": 300, "role": role },
            ],
            "links": [ { "from": a, "to": b, "wake": true } ],
        })
        .to_string()
    };
    let form =
        |version: &str, draft: &str| format!("version={version}&draft={}", form_value(draft));
    let published = post(
        &fixture,
        "/org/publish",
        Some(&cookie),
        &form(&version, &draft(&long_role)),
    )
    .await;
    assert_eq!(published.status, 303, "{}", published.text());

    // A stale publish, an invalid one (a role with a line feed that carries the word), a broken one.
    let stale = post(
        &fixture,
        "/org/publish",
        Some(&cookie),
        &form(&version, &draft(&role)),
    )
    .await;
    assert_eq!(stale.status, 409);
    let fresh = get(&fixture, "/org", &cookie).await.text();
    let version = between(&fresh, "id=\"org-data\" data-version=\"", "\"").to_owned();
    let invalid = post(
        &fixture,
        "/org/publish",
        Some(&cookie),
        &form(&version, &draft(&format!("{role}\nmore"))),
    )
    .await;
    assert_eq!(invalid.status, 422);
    let unreadable = post(
        &fixture,
        "/org/publish",
        Some(&cookie),
        &format!(
            "version={version}&draft={}",
            form_value(&format!("{{\"levels\":\"{role}\""))
        ),
    )
    .await;
    assert_eq!(unreadable.status, 400);

    // A failure of the hub in the middle of a publish: the database refuses the update.
    let function = "org_canary_boom";
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'boom'; END $$"
    )))
    .execute(&fixture.pool)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TRIGGER {function} BEFORE UPDATE ON org_member \
         FOR EACH ROW EXECUTE FUNCTION {function}()"
    )))
    .execute(&fixture.pool)
    .await
    .unwrap();
    let broken = post(
        &fixture,
        "/org/publish",
        Some(&cookie),
        &form(&version, &draft(&format!("{role} changed"))),
    )
    .await;
    assert_eq!(broken.status, 500, "{}", broken.text());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON org_member"
    )))
    .execute(&fixture.pool)
    .await
    .unwrap();

    // The command that shows the chart.
    let chart = fixture.admin.org_chart().await.unwrap();
    let shown = render::show(&chart).text;
    assert!(
        shown.contains(&role),
        "the command shows the role to the admin: {shown}"
    );

    // Both Hermes call every peer tool, and B gets a notice.
    let mut id = 10;
    let thread;
    {
        let mut local = Vec::new();
        a_client
            .send_call(900, "hermes_send_message", json!({ "to": b, "text": note }))
            .await
            .unwrap();
        loop {
            match a_client.pump(PATIENCE).await {
                Ok(Some(Message::Result(result))) if result.id.get() == 900 => {
                    let text = written(Message::Result(result));
                    let parsed: Value = serde_json::from_str(&text).unwrap();
                    thread = parsed["thread"].as_str().unwrap().to_owned();
                    local.push(text);
                    break;
                }
                Ok(Some(message)) => local.push(written(message)),
                Ok(None) => {}
                Err(error) => panic!("{error:?}"),
            }
        }
        heard.extend(local);
    }
    for tool in ["hermes_list_peers", "hermes_list_threads", "arauto_status"] {
        id += 1;
        call(&mut a_client, &mut heard, id, tool, json!({})).await;
        id += 1;
        call(&mut b_client, &mut heard, id, tool, json!({})).await;
    }
    id += 1;
    call(
        &mut b_client,
        &mut heard,
        id,
        "hermes_read_thread",
        json!({ "thread": thread }),
    )
    .await;
    id += 1;
    call(
        &mut a_client,
        &mut heard,
        id,
        "hermes_read_thread",
        json!({ "thread": thread }),
    )
    .await;
    listen(&mut b_client, &mut heard, Duration::from_millis(1_500)).await;
    listen(&mut a_client, &mut heard, Duration::from_millis(500)).await;
    // The link wakes, and B's switch was never turned on: B heard a notice of a note, and
    // the notice carries no text.
    let notices: Vec<&String> = heard
        .iter()
        .filter(|text| text.contains("\"kind\":\"peer\""))
        .collect();
    assert!(
        notices
            .iter()
            .any(|text| text.contains("\"kind\":\"peer\"")),
        "B got a notice of a note: {heard:?}"
    );
    assert!(
        notices.iter().all(|text| !text.contains(&note)),
        "the text of a note is in a notice"
    );
    assert!(
        heard.iter().any(|text| text.contains(&note)),
        "B reads the note in the thread"
    );
    assert!(
        heard.len() > 10,
        "the connectors heard something: {}",
        heard.len()
    );

    // The detector sees what is stored.
    let stored: Vec<String> = sqlx::query_scalar(
        "SELECT (SELECT string_agg(name, ' ') FROM org_level) || ' ' \
         || (SELECT string_agg(role, ' ') FROM org_member)",
    )
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    assert!(stored[0].contains(&level) && stored[0].contains(&role));

    // And nowhere else.
    let log = captured.text();
    let audit: String = sqlx::query_scalar(
        "SELECT coalesce(string_agg(row_to_json(a)::text, E'\\n'), '') FROM audit a",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    let alerts: String = sqlx::query_scalar(
        "SELECT coalesce(string_agg(row_to_json(a)::text, E'\\n'), '') FROM alert a",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert!(audit.contains("org_chart.published"), "the audit was read");
    for text in [&log, &audit, &alerts] {
        assert!(
            !text.contains(&note),
            "the text of a note is stored or logged"
        );
    }
    for planted in [&role, &level, &suffix] {
        assert!(!log.contains(planted.as_str()), "{planted} is in the log");
        assert!(
            !audit.contains(planted.as_str()),
            "{planted} is in the audit"
        );
        assert!(
            !alerts.contains(planted.as_str()),
            "{planted} is in the alerts"
        );
        assert!(
            !heard.iter().any(|text| text.contains(planted.as_str())),
            "{planted} reached a connector"
        );
    }
}
