//! The guide that connects a Hermes, from the name to the session: the block that appears
//! once, the code that is typed and never shown, the answer about the last use, and what a
//! request that only reads may not do.

use std::sync::Arc;

use arauto_hub::channel::Downloads;
use arauto_protocol::clock::SystemClock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::message::{Declared, DeclaredText};
use arauto_testkit::device::{self, Device, Enrolled, Faults};
use arauto_testkit::http;
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{self, PANEL_HOST};
use bytes::Bytes;

use crate::session::everything;
use crate::{Browser, PATIENCE, hub, rows};

/// The connector the hub of these tests serves, and the SHA-256 of its three bytes.
const CONNECTOR: &str = "arauto-connector-linux-x86_64";
const CONNECTOR_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// A hub that serves a connector: the only kind the panel makes a block for.
async fn serving() -> Fixture {
    let mut fixture = Fixture::stopped(
        support::install().await,
        fixture::settings(),
        Arc::new(SystemClock),
    )
    .await;
    fixture.downloads = Downloads::empty()
        .with_file(CONNECTOR, Bytes::from_static(b"abc"))
        .unwrap();
    fixture.start().await;
    fixture
}

/// What a page shows as text, with the marks of escaping undone.
fn unescaped(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The block a page shows, as the admin would copy it.
fn block_of(page: &str) -> String {
    let start = page
        .find("<pre id=\"guide-text\">")
        .unwrap_or_else(|| panic!("the page shows no block: {page}"))
        + "<pre id=\"guide-text\">".len();
    let end = start + page[start..].find("</pre>").unwrap();
    unescaped(&page[start..end])
}

/// The invite inside a block: the last thing in quotes on the line of `join`.
fn invite_of(block: &str) -> String {
    let line = block
        .lines()
        .find(|line| line.contains(" join \""))
        .expect("the block has the line of the join");
    let mut quoted = line.split('"');
    let invite = quoted.nth_back(1).unwrap().to_owned();
    assert!(Invite::parse(&invite).is_ok(), "that is an invite");
    invite
}

/// The value of a hidden field of a page.
fn hidden(page: &str, name: &str) -> String {
    let mark = format!("name=\"{name}\" value=\"");
    let start = page
        .find(&mark)
        .unwrap_or_else(|| panic!("the page has no field {name}"))
        + mark.len();
    let end = start + page[start..].find('"').unwrap();
    unescaped(&page[start..end])
}

/// Whether six digits of a verification code show up standing alone, or as `123-456`.
fn has_code(text: &str, code: &str) -> bool {
    let digits: String = code.chars().filter(char::is_ascii_digit).collect();
    text.contains(code)
        || text.match_indices(digits.as_str()).any(|(at, _)| {
            let before = text[..at].chars().next_back();
            let after = text[at + digits.len()..].chars().next();
            !before.is_some_and(|c| c.is_ascii_digit())
                && !after.is_some_and(|c| c.is_ascii_digit())
        })
}

/// A code that is not `code`, and is not the example the field shows either.
fn another(code: &str) -> &'static str {
    if code == "111-111" {
        "222-222"
    } else {
        "111-111"
    }
}

async fn count(fixture: &Fixture, table: &str) -> usize {
    rows(fixture, table).await.len()
}

/// The device enrolls with the invite of a block and returns the code it would show.
async fn enroll(device: &Device, invite: &str, declared: Declared) -> String {
    match device
        .enroll(&Invite::parse(invite).unwrap(), declared)
        .await
        .unwrap()
    {
        Enrolled::Code(code) => code.to_string(),
        Enrolled::Refused(code) => panic!("the enrollment was refused with {code}"),
    }
}

async fn wait_page(browser: &Browser, cookie: &str, name: &str) -> String {
    let answer = browser
        .get(&format!("/hermes/wait?name={name}"), Some(cookie))
        .await;
    assert_eq!(answer.status, 200, "{}", answer.text());
    answer.text()
}

#[allow(clippy::too_many_lines)]
/// The text of the block as `docs/02-como-funciona.md` has it, section 1, "The block": the
/// first fenced text after that title, with its fields still between braces.
fn approved_block() -> String {
    let document = include_str!("../../../../docs/02-como-funciona.md").replace("\r\n", "\n");
    let section = document
        .split("\n### The block\n")
        .nth(1)
        .expect("the document has the section of the block");
    let fenced = section
        .split("\n```\n")
        .nth(1)
        .expect("the section has the text of the block");
    format!("{fenced}\n")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_guide_connects_a_hermes_from_the_name_to_the_session() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    // Step 1: the name. The page under the guide cannot be reached while it is open.
    let asks = browser.get("/hermes/connect", Some(&cookie)).await;
    assert_eq!(asks.status, 200);
    let asks = asks.text();
    assert!(asks.contains("What name will this Hermes have in the panel?"));
    assert!(asks.contains("<div class=\"page\" inert>"));
    assert!(asks.contains("<script src=\"/assets/panel.js\"></script>"));
    assert_eq!(asks.matches("<script").count(), 1);
    assert_eq!(
        count(&fixture, "hermes").await,
        0,
        "asking the name creates nothing"
    );

    // Step 2: the line, its invite, and the block.
    let issued = browser
        .post("/hermes/connect", Some(&cookie), "name=atendimento")
        .await;
    assert_eq!(issued.status, 200, "{}", issued.text());
    assert_eq!(issued.header("cache-control"), Some("no-store"));
    let page = issued.text();
    assert!(page.contains("class=\"modal guide preparing\""));
    assert!(page.contains("<h2 id=\"guide-title\">Connect atendimento</h2>"));
    assert!(page.contains("Preparing a secure connection"));
    let block = block_of(&page);
    assert!(
        block.starts_with("Connect this Hermes to the Arauto hub. Invite for: atendimento.\n"),
        "{block}"
    );
    // The chain of the block: download into the folder of the connector, check the digest
    // of the file this hub serves, and only then run it.
    for line in [
        "mkdir -p \"$HOME/.arauto\" && cd \"$HOME/.arauto\" &&".to_owned(),
        format!("curl -fsSLo {CONNECTOR} \"http://127.0.0.1:1/v1/download/{CONNECTOR}\" &&"),
        format!("echo \"{CONNECTOR_SHA256}  {CONNECTOR}\" | sha256sum -c - &&"),
        format!("chmod 700 {CONNECTOR} &&"),
    ] {
        assert!(block.lines().any(|found| found == line), "{line}: {block}");
    }
    let join = format!(
        "./{CONNECTOR} join \"ws://127.0.0.1:1/v1/channel\" \"{}\" \"",
        fixture.hub_key_text()
    );
    assert!(block.contains(&join), "{block}");
    assert!(block.contains("Then answer me with only the verification code it shows."));
    assert!(
        block.ends_with("do not execute it and tell the operator.\n"),
        "{block}"
    );
    let invite = invite_of(&block);
    // The words are the ones of the document, which only change with the maintainer's
    // approval: the block is that text with its fields filled, and nothing else.
    let approved = approved_block()
        .replace("{NAME}", "atendimento")
        .replace("arauto-connector-{VERSION}", CONNECTOR)
        .replace(
            "{CONNECTOR_ADDRESS}",
            &format!("http://127.0.0.1:1/v1/download/{CONNECTOR}"),
        )
        .replace("{SHA256}", CONNECTOR_SHA256)
        .replace("{HUB}", "ws://127.0.0.1:1/v1/channel")
        .replace("{HUB_KEY}", &fixture.hub_key_text())
        .replace("{INVITE}", &invite);
    assert!(!approved.contains('{'), "a field was left: {approved}");
    assert_eq!(block, approved, "the block is not the text of the document");
    assert_eq!(count(&fixture, "hermes").await, 1);
    // The audit names the panel where the command names the system user.
    let audit = rows(&fixture, "audit").await.join("\n");
    assert!(audit.contains("\"actor_note\":\"panel\""), "{audit}");
    assert!(
        audit.contains("hermes.created") && audit.contains("invite.issued"),
        "{audit}"
    );

    // Step 3, before the Hermes ran anything: the page waits, and asks again by itself.
    let waits = wait_page(&browser, &cookie, "atendimento").await;
    assert!(waits.contains("Waiting for the Hermes atendimento"));
    assert!(waits.contains("<meta http-equiv=\"refresh\" content=\"3\">"));
    assert!(waits.contains("<li class=\"ok\">Invite issued</li>"));
    assert!(waits.contains("<li class=\"waiting\">A device used the invite: waiting…</li>"));
    assert!(
        !waits.contains(&invite),
        "the invite is in no page but the one of the block"
    );

    // The connector presents the invite: the hub observed it, and the page asks for the code.
    let device = fixture.device();
    let code = enroll(&device, &invite, device::declared()).await;
    let asks_code = wait_page(&browser, &cookie, "atendimento").await;
    assert!(asks_code.contains("A device used the invite of atendimento"));
    assert!(asks_code.contains("<li class=\"ok\">A device used the invite, from 127.0.0.1"));
    assert!(asks_code.contains(
        "machine &quot;test-host&quot; · system &quot;test-os&quot; · Hermes &quot;0.0.0-test&quot; (not verified)"
    ));
    assert!(
        !asks_code.contains("http-equiv"),
        "a page being typed in never reloads"
    );
    assert!(
        !asks_code.contains("name=\"use\""),
        "a first connection has no last use"
    );
    assert!(
        !has_code(&asks_code, &code),
        "the panel never shows the code"
    );

    // A wrong code counts; one out of the format does not; neither is shown back.
    let wrong = another(&code);
    let form = |typed: &str| format!("name=atendimento&code={typed}");
    let refused = browser
        .post("/hermes/approve", Some(&cookie), &form(wrong))
        .await;
    assert_eq!(refused.status, 200);
    assert!(
        refused
            .text()
            .contains("The code does not match. Attempt 1 of 5.")
    );
    assert!(!has_code(&refused.text(), wrong) && !has_code(&refused.text(), &code));
    let malformed = browser
        .post("/hermes/approve", Some(&cookie), &form("12"))
        .await;
    assert!(malformed.text().contains("The code has 6 digits"));
    let refused = browser
        .post("/hermes/approve", Some(&cookie), &form(wrong))
        .await;
    assert!(
        refused.text().contains("Attempt 2 of 5."),
        "the malformed one was not counted"
    );

    // The right code: approved, and the guide goes on by an address that can be read again.
    let approved = browser
        .post("/hermes/approve", Some(&cookie), &form(&code))
        .await;
    assert_eq!(approved.status, 303, "{}", approved.text());
    assert_eq!(
        approved.header("location"),
        Some("/hermes/wait?name=atendimento")
    );
    let done = wait_page(&browser, &cookie, "atendimento").await;
    assert!(done.contains("<h3>atendimento was approved</h3>"));
    assert!(done.contains("It has no access to any number yet."));
    assert!(done.contains("class=\"mascot waving\""));
    assert!(!done.contains("http-equiv"));

    // The connector confirms its first credential: connected, in the guide and in the list.
    let session = device
        .channel(Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    let done = wait_page(&browser, &cookie, "atendimento").await;
    assert!(done.contains("<h3>atendimento is connected</h3>"), "{done}");
    assert_eq!(done.matches("<li class=\"ok\">").count(), 4);
    let list = browser.get("/", Some(&cookie)).await.text();
    assert!(list.contains("<span class=\"state\">Connected</span>"));
    assert!(list.contains("Connected: 1 · Waiting: 0 · On alert: 0"));
    drop(session);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_form_of_the_guide_needs_a_session_and_the_origin_of_the_panel() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let name = fixture.hermes().await;
    let before = everything(&fixture).await;

    let forms = [
        ("/hermes/connect", "name=novo".to_owned()),
        ("/hermes/invite", format!("name={name}")),
        ("/hermes/approve", format!("name={name}&code=123-456")),
        ("/hermes/deny", format!("name={name}")),
    ];
    for (path, body) in &forms {
        // No session: the entry, and nothing of the form is done.
        let answer = browser.post(path, None, body).await;
        assert_eq!(answer.status, 303, "{path}");
        assert_eq!(answer.header("location"), Some("/sign-in"));
        // A session, and a form that did not come from a page of the panel.
        for origin in [None, Some("http://evil.test"), Some("null")] {
            let mut headers = vec![
                ("Host", PANEL_HOST),
                ("Cookie", cookie.as_str()),
                ("Content-Type", "application/x-www-form-urlencoded"),
            ];
            if let Some(origin) = origin {
                headers.push(("Origin", origin));
            }
            let answer = http::post(browser.panel, path, &headers, body.as_bytes(), PATIENCE)
                .await
                .unwrap();
            assert_eq!(answer.status, 403, "{path} {origin:?}");
        }
        // What a form does, an address that is only opened never does.
        if *path != "/hermes/connect" {
            let opened = browser.get(&format!("{path}?{body}"), Some(&cookie)).await;
            assert_eq!(opened.status, 404, "{path}");
        }
    }
    // The addresses that read: with every value a form would take, they still only read.
    for path in [
        "/hermes/connect?name=novo".to_owned(),
        format!("/hermes/wait?name={name}"),
        format!("/hermes/wait?name={name}&code=123-456&use=recognized"),
    ] {
        assert_eq!(
            browser.get(&path, Some(&cookie)).await.status,
            200,
            "{path}"
        );
        assert_eq!(browser.get(&path, None).await.status, 303, "{path}");
    }
    for path in [
        "/hermes/wait",
        "/hermes/wait?name=",
        "/hermes/wait?name=ninguem",
        "/hermes/wait?name=%3Cb%3E",
    ] {
        let answer = browser.get(path, Some(&cookie)).await;
        assert_eq!(answer.status, 303, "{path}");
        assert_eq!(answer.header("location"), Some("/"));
    }
    assert_eq!(
        browser.get("/hermes/nothing", Some(&cookie)).await.status,
        404
    );
    assert_eq!(before, everything(&fixture).await, "something was written");

    // The detector sees a write when one happens, and the same form with the session and
    // the origin of the panel is the one that does it.
    let answer = browser
        .post("/hermes/invite", Some(&cookie), &forms[1].1)
        .await;
    assert_eq!(answer.status, 200);
    assert_ne!(before, everything(&fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn reading_the_guide_changes_nothing_while_a_request_waits() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let name = fixture.hermes().await;
    let device = fixture.device();
    let code = fixture.enroll(&name, &device).await;

    let before = everything(&fixture).await;
    for _ in 0..3 {
        let page = wait_page(&browser, &cookie, &name).await;
        assert!(page.contains("Verification code"));
        assert!(!has_code(&page, &code));
        assert_eq!(browser.get("/", Some(&cookie)).await.status, 200);
        assert_eq!(
            browser.get("/hermes/connect", Some(&cookie)).await.status,
            200
        );
    }
    assert_eq!(
        before,
        everything(&fixture).await,
        "reading spent or counted something"
    );
    // The code was not spent by being looked at: it still approves.
    let approved = browser
        .post(
            "/hermes/approve",
            Some(&cookie),
            &format!("name={name}&code={code}"),
        )
        .await;
    assert_eq!(approved.status, 303);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_name_the_hub_refuses_creates_nothing() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let taken = fixture.hermes().await;
    let before = everything(&fixture).await;

    let long = "a".repeat(33);
    for (typed, said) in [
        ("", "The name has from 1 to 32 characters."),
        (long.as_str(), "The name has from 1 to 32 characters."),
        ("Ana", "The name starts with a lowercase letter or a digit."),
        (
            "-ana",
            "The name starts with a lowercase letter or a digit.",
        ),
        (
            "ana+maria",
            "Use only lowercase letters, digits, hyphen and underscore.",
        ),
        (
            "ana%20maria",
            "Use only lowercase letters, digits, hyphen and underscore.",
        ),
        (
            "a%3Cscript%3Ealert(1)%3C%2Fscript%3E",
            "Use only lowercase letters, digits, hyphen and underscore.",
        ),
        ("admin", "That name is reserved. Choose another."),
        (taken.as_str(), "There already is a Hermes with that name."),
    ] {
        let answer = browser
            .post("/hermes/connect", Some(&cookie), &format!("name={typed}"))
            .await;
        assert_eq!(answer.status, 200, "{typed}");
        let page = answer.text();
        assert!(page.contains(said), "{typed}: {page}");
        assert!(
            page.contains("What name will this Hermes have in the panel?"),
            "{typed}"
        );
        assert!(!page.contains("guide-text"), "{typed}: no block");
        assert!(
            !page.contains("<script>alert"),
            "what was typed is shown as text"
        );
    }
    // A form without the field, and one that is not a form.
    let answer = browser
        .post("/hermes/connect", Some(&cookie), "other=1")
        .await;
    assert!(
        answer
            .text()
            .contains("The name has from 1 to 32 characters.")
    );
    assert_eq!(
        before,
        everything(&fixture).await,
        "a refused name wrote something"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_block_appears_once_and_a_new_invite_cancels_the_one_before() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    let first = browser
        .post("/hermes/connect", Some(&cookie), "name=vendas")
        .await;
    let first_invite = invite_of(&block_of(&first.text()));
    assert!(
        !first
            .text()
            .contains("The invite issued before was cancelled.")
    );

    // No page that is read shows it, and the database holds only its digest.
    for path in [
        "/",
        "/hermes/connect",
        "/hermes/wait?name=vendas",
        "/accesses",
    ] {
        let page = browser.get(path, Some(&cookie)).await.text();
        assert!(!page.contains(&first_invite), "{path} shows the invite");
        assert!(!page.contains("guide-text"), "{path} shows a block");
    }
    let mut stored = String::new();
    for table in ["invite", "audit", "alert", "hermes", "device"] {
        stored.push_str(&rows(&fixture, table).await.join("\n"));
    }
    assert!(stored.contains("invite.issued"));
    assert!(
        !stored.contains(&first_invite),
        "the invite is written in the database"
    );

    // The line already exists: another invite comes from its button, and cancels the first.
    let list = browser.get("/", Some(&cookie)).await.text();
    assert!(
        list.contains("<a class=\"button\" href=\"/hermes/wait?name=vendas\">Continue</a>"),
        "{list}"
    );
    let second = browser
        .post("/hermes/invite", Some(&cookie), "name=vendas")
        .await;
    assert_eq!(second.status, 200);
    assert!(
        second
            .text()
            .contains("The invite issued before was cancelled.")
    );
    let second_invite = invite_of(&block_of(&second.text()));
    assert_ne!(first_invite, second_invite);

    let device = fixture.device();
    let refused = device
        .enroll(&Invite::parse(&first_invite).unwrap(), device::declared())
        .await
        .unwrap();
    assert_eq!(refused, Enrolled::Refused(ErrorCode::Ar101));
    enroll(&device, &second_invite, device::declared()).await;

    // An invite for a Hermes that does not exist is not issued.
    let before = count(&fixture, "invite").await;
    let nobody = browser
        .post("/hermes/invite", Some(&cookie), "name=ninguem")
        .await;
    assert_eq!(nobody.status, 303);
    assert_eq!(count(&fixture, "invite").await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hub_that_serves_no_connector_issues_nothing_from_the_panel() {
    // The hub of the other tests of the panel: no file to download.
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let existing = fixture.hermes().await;
    let before = everything(&fixture).await;

    for (path, body) in [
        ("/hermes/connect", "name=novo".to_owned()),
        ("/hermes/invite", format!("name={existing}")),
    ] {
        let answer = browser.post(path, Some(&cookie), &body).await;
        assert_eq!(answer.status, 200, "{path}");
        let page = answer.text();
        assert!(
            page.contains("This hub has no connector to serve"),
            "{path}: {page}"
        );
        assert!(page.contains("Nothing was issued."));
        assert!(!page.contains("guide-text"));
    }
    assert_eq!(
        before,
        everything(&fixture).await,
        "something was created without a block"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reconnect_asks_about_the_last_use_that_is_on_the_screen() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (name, session) = fixture.connected().await;
    let old = session.device().clone();
    drop(session);

    let list = browser.get("/", Some(&cookie)).await.text();
    assert!(
        list.contains("<button class=\"button\" type=\"submit\">Reconnect</button>"),
        "{list}"
    );
    let issued = browser
        .post("/hermes/invite", Some(&cookie), &format!("name={name}"))
        .await;
    assert_eq!(issued.status, 200);
    let page = issued.text();
    assert!(page.contains(&format!("<h2 id=\"guide-title\">Reconnect {name}</h2>")));
    assert!(page.contains("The device in use keeps working until the new one confirms"));
    let invite = invite_of(&block_of(&page));

    let new = fixture.device();
    let code = enroll(&new, &invite, device::declared()).await;
    let asks = wait_page(&browser, &cookie, &name).await;
    assert!(asks.contains("Last use of the current device: "), "{asks}");
    assert!(asks.contains("Do you recognize it?"));
    let seen = format!(
        "seen_device={}&seen_at={}&seen_state={}",
        hidden(&asks, "seen_device"),
        hidden(&asks, "seen_at"),
        hidden(&asks, "seen_state")
    );
    let form = |rest: &str| format!("name={name}&code={code}&{rest}");

    // The right code without the answer approves nothing and counts nothing.
    let unanswered = browser
        .post("/hermes/approve", Some(&cookie), &form(&seen))
        .await;
    assert_eq!(unanswered.status, 200);
    assert!(
        unanswered
            .text()
            .contains("Before the code, say whether you recognize the last use")
    );
    // An answer about a use that is not the one of the device now: nothing either.
    for stale in [
        "use=recognized".to_owned(),
        format!(
            "use=recognized&seen_device={}&seen_at=1&seen_state=active",
            hidden(&asks, "seen_device")
        ),
        format!(
            "use=not-recognized&{}",
            seen.replace("seen_state=active", "seen_state=quarantined")
        ),
    ] {
        let answer = browser
            .post("/hermes/approve", Some(&cookie), &form(&stale))
            .await;
        assert_eq!(answer.status, 200, "{stale}");
        assert!(
            answer.text().contains("Nothing was approved."),
            "{stale}: {}",
            answer.text()
        );
    }
    assert_eq!(fixture.state_of(&new).await.0, "pending");
    assert_eq!(fixture.state_of(&old).await.0, "active");
    let asks = wait_page(&browser, &cookie, &name).await;
    assert!(!asks.contains("Attempt"), "no attempt was counted");

    // The answer about the use on the screen, with the right code.
    let approved = browser
        .post(
            "/hermes/approve",
            Some(&cookie),
            &form(&format!("use=recognized&{seen}")),
        )
        .await;
    assert_eq!(approved.status, 303, "{}", approved.text());
    assert_eq!(fixture.state_of(&new).await.0, "approved");
    let done = wait_page(&browser, &cookie, &name).await;
    assert!(done.contains(&format!("<h3>{name} was approved</h3>")));
    assert!(done.contains("The accesses it already had apply to this device again."));
}

#[tokio::test(flavor = "multi_thread")]
async fn five_wrong_codes_deny_the_request_and_so_does_the_button() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    let name = fixture.hermes().await;
    let device = fixture.device();
    let code = fixture.enroll(&name, &device).await;
    let wrong = format!("name={name}&code={}", another(&code));
    for attempt in 1..=4 {
        let answer = browser.post("/hermes/approve", Some(&cookie), &wrong).await;
        assert!(answer.text().contains(&format!("Attempt {attempt} of 5.")));
    }
    let fifth = browser.post("/hermes/approve", Some(&cookie), &wrong).await;
    assert_eq!(fifth.status, 200);
    let page = fifth.text();
    assert!(
        page.contains(&format!("The request of {name} was denied")),
        "{page}"
    );
    assert!(page.contains("Issue another invite"));
    assert_eq!(fixture.state_of(&device).await.0, "denied");
    // The right code, now: there is nothing to approve, and the guide says what there is.
    let late = browser
        .post(
            "/hermes/approve",
            Some(&cookie),
            &format!("name={name}&code={code}"),
        )
        .await;
    assert_eq!(late.status, 303);
    let ended = wait_page(&browser, &cookie, &name).await;
    assert!(
        ended.contains("has no invite and no request waiting"),
        "{ended}"
    );

    // The button: one form, and the request is denied.
    let other = fixture.hermes().await;
    let asking = fixture.device();
    fixture.enroll(&other, &asking).await;
    let denied = browser
        .post("/hermes/deny", Some(&cookie), &format!("name={other}"))
        .await;
    assert_eq!(denied.status, 303);
    assert_eq!(denied.header("location"), Some("/"));
    assert_eq!(fixture.state_of(&asking).await.0, "denied");
    assert_eq!(fixture.state_of(&device).await.0, "denied");
}

#[tokio::test(flavor = "multi_thread")]
async fn what_a_device_says_of_itself_is_shown_as_text() {
    let fixture = serving().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let device = fixture.device();
    let hostile = Declared {
        hostname: DeclaredText::parse("<script>alert(1)</script>").unwrap(),
        os: DeclaredText::parse("\"><img src=x onerror=alert(2)>").unwrap(),
        hermes_version: DeclaredText::parse("</dd></dl></dialog><h1>x</h1>").unwrap(),
    };
    let Enrolled::Code(_) = device.enroll(&invite, hostile).await.unwrap() else {
        panic!("enrolled");
    };

    let page = wait_page(&browser, &cookie, &name).await;
    assert!(
        page.contains("machine &quot;&lt;script&gt;alert(1)&lt;/script&gt;&quot;"),
        "{page}"
    );
    assert!(page.contains("&quot;&gt;&lt;img src=x onerror=alert(2)&gt;"));
    assert!(!page.contains("<script>alert") && !page.contains("<img"));
    assert_eq!(
        page.matches("</dialog>").count(),
        1,
        "the text did not close the guide"
    );
    assert_eq!(page.matches("<h1>").count(), 1);
}
