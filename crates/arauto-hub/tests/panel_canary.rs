//! The canary of the secrets of the panel (`docs/15-mvp.md`, section 7;
//! `docs/decisoes/0040-painel-dentro-do-hub.md`): entry codes are issued, typed wrong and
//! right, sessions are opened, used, closed and left to end, and a Hermes is connected through
//! the guide, with every log of the process captured. No entry code, no identifier of a
//! session, no invite and no verification code may appear in the log, in the errors the log
//! carries, in the audit, in the alerts or in the rows of the panel and of the invites.
//!
//! A binary of its own: the log of the process is captured from its first line, at every
//! level, and no other test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::channel::Downloads;
use arauto_hub::first_access::{self, Output};
use arauto_hub::repo::panel::SESSION_IDLE_MS;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::invite::Invite;
use arauto_protocol::message::DeclaredText;
use arauto_testkit::device::{self, Enrolled};
use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{self, PANEL_HOST, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;
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

/// Where a verification code shows up in `text`: as `123-456`, or as six digits standing alone,
/// with neither a letter nor a digit beside them. A run of six digits inside a word (a hex
/// digest, an identifier, base64) is not a code someone wrote: the long log of every level holds
/// many of them, and one may match by chance.
fn code_at(text: &str, code: &str) -> Option<(usize, usize)> {
    if let Some(at) = text.find(code) {
        return Some((at, code.len()));
    }
    let digits: String = code.chars().filter(char::is_ascii_digit).collect();
    text.match_indices(digits.as_str())
        .find(|(at, _)| {
            let before = text[..*at].chars().next_back();
            let after = text[at + digits.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
        .map(|(at, _)| (at, digits.len()))
}

fn has_code(text: &str, code: &str) -> bool {
    code_at(text, code).is_some()
}

/// The text around a code that was found, with the code itself masked: a failure says what
/// carried it.
fn around(text: &str, (at, len): (usize, usize)) -> String {
    let mut start = at.saturating_sub(80);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (at + len + 80).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    format!("{}[CODE]{}", &text[start..at], &text[at + len..end])
}

/// The secrets `place` must not hold: the pages of the guide show the invites on purpose.
fn secrets_for(
    place: &str,
    secrets: &[(String, String)],
    not_shown: &[(String, String)],
) -> Vec<(String, String)> {
    if place == "the pages of the guide" {
        not_shown.to_vec()
    } else {
        secrets.to_vec()
    }
}

/// The text of the page of the sentence the guide shows: the address of the instruction page
/// in it, as a path of the port of the connectors, and the bytes of its handle.
fn page_of(page: &str) -> (String, Vec<u8>) {
    let start = page
        .find("<pre id=\"guide-text\">")
        .expect("the page shows a sentence");
    let end = start + page[start..].find("</pre>").unwrap();
    let text = &page[start..end];
    let at = text.find("/v1/join/").expect("the sentence names a page");
    let rest = &text[at..];
    let path: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    let handle = path.rsplit('/').next().unwrap().trim_end_matches('.');
    assert_eq!(handle.len(), 26, "{path}");
    let bytes = BASE32_NOPAD
        .decode(handle.to_ascii_uppercase().as_bytes())
        .unwrap();
    (path.trim_end_matches('.').to_owned(), bytes)
}

/// The invite inside the block of the command a page shows.
fn invite_of(page: &str) -> String {
    let start = page
        .find("<pre id=\"guide-command\">")
        .expect("the page shows a block");
    let end = start + page[start..].find("</pre>").unwrap();
    let text = page[start..end].replace("&quot;", "\"");
    let line = text
        .lines()
        .find(|line| line.contains(" join \""))
        .expect("the block has the line of the join");
    let invite = line.split('"').nth_back(1).unwrap().to_owned();
    assert!(Invite::parse(&invite).is_ok());
    invite
}

#[test]
fn the_detector_sees_a_planted_secret() {
    let secret = [0x5au8; 32];
    for written in spellings(&secret) {
        let secrets = vec![("planted".to_owned(), written.clone())];
        assert_eq!(leaks(&format!("x {written} y"), &secrets), ["planted"]);
    }
    assert!(has_code("the code 028-213 was typed", "028-213"));
    assert!(has_code(r#"{"code":"028213"}"#, "028-213"));
    assert!(!has_code("at 1800000282130", "028-213"));
    assert!(has_code("code=028213&name=x", "028-213"));
    assert!(!has_code("digest 3f028213a9", "028-213"));
    let text = "before code=028213 after";
    assert_eq!(
        around(text, code_at(text, "028-213").unwrap()),
        "before code=[CODE] after"
    );
}

/// Every way the code `shown` could have been written, noted as secrets of `what`.
fn spell_code(shown: &str, what: &str, secrets: &mut Vec<(String, String)>) {
    let plain = shown.replace('-', "");
    let bytes = BASE32_NOPAD
        .decode(plain.to_ascii_uppercase().as_bytes())
        .unwrap();
    assert_eq!(bytes.len(), 20);
    secrets.push((what.to_owned(), shown.to_owned()));
    secrets.push((what.to_owned(), shown.to_ascii_uppercase()));
    for written in spellings(&bytes) {
        secrets.push((what.to_owned(), written));
    }
    // The first groups alone would already help whoever guesses.
    secrets.push((format!("{what} (its start)"), shown[..14].to_owned()));
}

/// An entry code, as the command shows it, with every way it could have been written.
async fn code(fixture: &Fixture, what: &str, secrets: &mut Vec<(String, String)>) -> String {
    let issued = fixture.admin.panel_code(fixture.now()).await.unwrap();
    let shown = issued.code.reveal().to_string();
    spell_code(&shown, what, secrets);
    shown
}

/// What the task of the first access wrote where the operator reads: the one place the start
/// code is allowed to be. It is not the log.
#[derive(Default)]
struct Block(Mutex<String>);

impl Output for Block {
    fn write(&self, text: &str) {
        self.0.lock().unwrap().push_str(text);
    }
}

/// Everything the panel answered during the run, bodies and headers, except the cookie it sets:
/// that one is the identifier of a session on purpose.
static SEEN: Mutex<String> = Mutex::new(String::new());

/// The same for the pages of the guide that connects a Hermes. They show the invite and its
/// block to the admin on purpose, so they are scanned for every other secret.
static GUIDE: Mutex<String> = Mutex::new(String::new());

/// Notes an answer in [`SEEN`], or in [`GUIDE`], and gives it back.
fn seen(path: &str, answer: Response) -> Response {
    let mut all = if path.starts_with("/hermes/") {
        GUIDE.lock().unwrap()
    } else {
        SEEN.lock().unwrap()
    };
    all.push_str(&answer.status.to_string());
    all.push('\n');
    for (name, value) in &answer.headers {
        if name != "set-cookie" {
            all.push_str(&format!("{name}: {value}\n"));
        }
    }
    all.push_str(&answer.text());
    all.push('\n');
    answer
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
    seen(
        path,
        http::post(panel, path, &headers, body.as_bytes(), PATIENCE)
            .await
            .unwrap(),
    )
}

async fn get(fixture: &Fixture, path: &str, cookie: Option<&str>) -> Response {
    let mut headers = vec![("Host", PANEL_HOST)];
    if let Some(cookie) = cookie {
        headers.push(("Cookie", cookie));
    }
    let panel = fixture.running().panel_addr().unwrap();
    seen(
        path,
        http::get(panel, path, &headers, PATIENCE).await.unwrap(),
    )
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

/// Signs in with a code and registers a passkey, so that the session reaches the whole
/// panel. Returns the cookie, with every way its identifier could have been written.
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
    let asked = post(fixture, "/passkey/register/start", Some(&cookie), "").await;
    assert_eq!(asked.status, 200, "{}", asked.text());
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

#[tokio::test(flavor = "multi_thread")]
async fn no_secret_of_the_panel_reaches_the_log_the_audit_or_the_rows() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    // R9 and R4, with the output the hub really uses: the standard error is not a line of the
    // log. A text handed to it, with every level of the log captured, is in no captured line.
    // The test fails when `Stderr::write` is changed to write through `tracing`.
    {
        use first_access::Output as _;
        first_access::Stderr.write("PLANTED-TEXT-FOR-THE-STDERR-WRITER\n");
        assert!(
            !captured
                .text()
                .contains("PLANTED-TEXT-FOR-THE-STDERR-WRITER"),
            "the writer of the block goes through the log"
        );
    }

    let clock = ManualClock::new(SystemClock.now_ms());
    // A hub that serves a connector: the panel makes a block for it.
    let mut fixture = Fixture::stopped(
        support::install().await,
        fixture::settings(),
        Arc::new(clock.clone()),
    )
    .await;
    fixture.downloads = Downloads::empty()
        .with_file(
            "arauto-connector-linux-x86_64",
            bytes::Bytes::from_static(b"abc"),
        )
        .unwrap();
    fixture.start().await;
    let mut secrets: Vec<(String, String)> = Vec::new();
    let mut codes: Vec<String> = Vec::new();

    // The start code: the hub writes it where the operator reads, and nowhere else. It is
    // typed, a passkey is registered with it, and the task writes its closing line. The code is
    // in what the operator reads, and in no line of the log, no row, no page.
    let block = Arc::new(Block::default());
    let task =
        first_access::start(fixture.hub(), block.clone(), DeclaredText::lossy("canary")).await;
    let written = block.0.lock().unwrap().clone();
    let start_code = written
        .lines()
        .find_map(|line| line.strip_prefix("INSTALLATION CODE: "))
        .expect("the block carries the code")
        .to_owned();
    spell_code(&start_code, "start code", &mut secrets);
    let mut pages = String::new();
    for path in ["/sign-in", "/first-access", "/"] {
        pages.push_str(&get(&fixture, path, None).await.text());
    }
    let started = sign_in(&fixture, &start_code, "start session", &mut secrets).await;
    pages.push_str(&get(&fixture, "/first-access", Some(&started)).await.text());
    pages.push_str(&get(&fixture, "/", Some(&started)).await.text());
    assert_eq!(
        post(&fixture, "/sign-out", Some(&started), "").await.status,
        303
    );
    for _ in 0..100 {
        if !task.running() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!task.running());
    assert!(block.0.lock().unwrap().contains("From now on the panel"));

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
    let refused = seen(
        "/sign-in",
        http::post(
            panel,
            "/sign-in",
            &elsewhere,
            format!("code={entry}").as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap(),
    );
    assert_eq!(refused.status, 403);
    let not_a_form = [
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "text/plain"),
    ];
    let refused = seen(
        "/sign-in",
        http::post(
            panel,
            "/sign-in",
            &not_a_form,
            format!("code={entry}").as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap(),
    );
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

    // A Hermes connected through the guide: the block is shown, the invite is used, and a
    // code is typed wrong, out of the format and right. An invite that another one replaces
    // is a secret too.
    let first_page = post(&fixture, "/hermes/connect", Some(&last), "name=canario")
        .await
        .text();
    let second_page = post(&fixture, "/hermes/invite", Some(&last), "name=canario")
        .await
        .text();
    let replaced = invite_of(&first_page);
    let invite = invite_of(&second_page);
    assert_ne!(replaced, invite);
    // The handles of the instruction pages: they are in the sentence, in front of the admin,
    // and in no log, no audit and no row. The page of the old invite is dead, the new one
    // serves; their bodies and `/llms.txt` are checked with the other pages.
    let (replaced_path, replaced_bytes) = page_of(&first_page);
    let (invite_path, invite_bytes) = page_of(&second_page);
    for written in spellings(&replaced_bytes) {
        secrets.push(("replaced page handle".to_owned(), written));
    }
    for written in spellings(&invite_bytes) {
        secrets.push(("page handle".to_owned(), written));
    }
    let connectors = fixture.addr();
    let dead = http::get(connectors, &replaced_path, &[], PATIENCE)
        .await
        .unwrap();
    assert_eq!(dead.status, 404);
    let alive = http::get(connectors, &invite_path, &[], PATIENCE)
        .await
        .unwrap();
    assert_eq!(alive.status, 200);
    let llms = http::get(connectors, "/llms.txt", &[], PATIENCE)
        .await
        .unwrap();
    assert_eq!(llms.status, 200);
    let mut instruction_pages = String::new();
    for answer in [&dead, &alive, &llms] {
        for (name, value) in &answer.headers {
            instruction_pages.push_str(&format!("{name}: {value}\n"));
        }
        instruction_pages.push_str(&answer.text());
        instruction_pages.push('\n');
    }
    secrets.push(("replaced invite".to_owned(), replaced));
    secrets.push(("invite".to_owned(), invite.clone()));
    let device = fixture.device();
    let Enrolled::Code(right) = device
        .enroll(&Invite::parse(&invite).unwrap(), device::declared())
        .await
        .unwrap()
    else {
        panic!("enrolled");
    };
    let right = right.to_string();
    let wrong = if right == "735-194" {
        "735-195"
    } else {
        "735-194"
    };
    for typed in [wrong, "73519", right.as_str()] {
        let form = format!("name=canario&code={typed}");
        let answer = post(&fixture, "/hermes/approve", Some(&last), &form).await;
        assert!(answer.status == 200 || answer.status == 303);
    }
    assert_eq!(fixture.state_of(&device).await.0, "approved");
    get(&fixture, "/hermes/wait?name=canario", Some(&last)).await;
    codes.push(right);
    codes.push(wrong.to_owned());

    let log = captured.text();
    assert!(
        log.len() > 1_000,
        "the log is captured (it has {} bytes)",
        log.len()
    );
    // Every table of the database, not a list of the ones that were thought of.
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY 1",
    )
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    assert!(
        tables.len() > 10 && tables.iter().any(|table| table == "panel_passkey"),
        "{tables:?}"
    );
    let mut stored = String::new();
    for table in &tables {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t"
        )))
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
        stored.push_str(&rows.join("\n"));
        stored.push('\n');
    }
    // Every file under the data folder.
    let mut files = String::new();
    let mut folders = vec![fixture.installation.data_dir.clone()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                files.push_str(&String::from_utf8_lossy(&bytes));
                files.push('\n');
            }
        }
    }
    for action in [
        "panel.code_issued",
        "panel.signed_in",
        "panel.signed_out",
        "invite.issued",
    ] {
        assert!(stored.contains(action), "{action} is in the audit");
    }
    pages.push_str(&SEEN.lock().unwrap());
    pages.push_str(&instruction_pages);
    // What the task wrote where the operator reads holds the start code and no other secret.
    let block_text = block.0.lock().unwrap().clone();
    let others: Vec<(String, String)> = secrets
        .iter()
        .filter(|(what, _)| !what.starts_with("start code"))
        .cloned()
        .collect();
    let found = leaks(&block_text, &others);
    assert!(found.is_empty(), "the block holds: {found:?}");
    // The pages of the guide hold the invites they show, and no other secret.
    let guide = GUIDE.lock().unwrap().clone();
    assert!(
        guide.contains(&invite),
        "the pages of the guide are scanned"
    );
    let not_shown: Vec<(String, String)> = secrets
        .iter()
        .filter(|(what, _)| {
            !matches!(
                what.as_str(),
                "invite" | "replaced invite" | "page handle" | "replaced page handle"
            )
        })
        .cloned()
        .collect();
    for (place, text) in [
        ("log", &log),
        ("the database", &stored),
        ("the files of the data folder", &files),
        ("the pages", &pages),
        ("the pages of the guide", &guide),
    ] {
        let found = leaks(text, &secrets_for(place, &secrets, &not_shown));
        assert!(found.is_empty(), "{place} holds: {found:?}");
        for code in &codes {
            if let Some(at) = code_at(text, code) {
                panic!("{place} holds a verification code: {}", around(text, at));
            }
        }
    }
}
