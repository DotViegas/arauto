//! What a visitor sees (R11 to R14 and R18 of the design of the first access): the entry of each
//! state, the one sentence for every refused code, the checks that come before anything is read,
//! the ceilings per address, the redirect after the first passkey, and the window that checks
//! the installation.

use std::sync::{Arc, Mutex};

use arauto_hub::check;
use arauto_hub::config::{ClientIpSource, Network};
use arauto_hub::first_access::{self, Output};
use arauto_hub::repo::panel::SESSION_IDLE_MS;
use arauto_protocol::clock::{ManualClock, SystemClock};
use arauto_protocol::limits::MINUTE_MS;
use arauto_protocol::message::DeclaredText;
use arauto_testkit::http;
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{self, PANEL_HOST, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;
use uuid::Uuid;

use crate::session::everything;
use crate::{Browser, PATIENCE, REFUSED, audited, code, cookie_of, form_value, hub, rows};

const FORM: (&str, &str) = ("Content-Type", "application/x-www-form-urlencoded");

/// A code of the right shape that no hub issued.
const WRONG: &str = "abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef";

/// What the first-access task wrote: where a test reads the start code.
#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Output for Recorder {
    fn write(&self, text: &str) {
        self.0.lock().unwrap().push(text.to_owned());
    }
}

impl Recorder {
    /// The code of the last block that has one.
    fn last_code(&self) -> String {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("INSTALLATION CODE: "))
                    .map(str::to_owned)
            })
            .expect("a block with a code was written")
    }
}

/// A hub that believes `X-Forwarded-For` from the proxies named. Its clock does not move, so
/// the minute of a ceiling never rolls over in the middle of a test, however slow the machine.
async fn behind(proxies: &str) -> Fixture {
    let mut fixture = Fixture::stopped(
        support::install().await,
        fixture::settings(),
        Arc::new(ManualClock::new(1_800_000_000_000)),
    )
    .await;
    fixture.client_ip = ClientIpSource::Forwarded {
        trusted_proxies: vec![Network::parse(proxies).unwrap()],
    };
    fixture.start().await;
    fixture
}

/// A form post of the panel that says it comes from `address`, through the proxy.
async fn post_from(browser: Browser, address: &str, path: &str, body: &str) -> http::Response {
    let headers = [
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        FORM,
        ("X-Forwarded-For", address),
    ];
    http::post(browser.panel, path, &headers, body.as_bytes(), PATIENCE)
        .await
        .unwrap()
}

async fn get_page(browser: Browser, path: &str, cookie: Option<&str>) -> String {
    let answer = browser.get(path, cookie).await;
    assert_eq!(answer.status, 200, "{path}: {}", answer.text());
    answer.text()
}

/// R11: the entry of a hub whose first access was not done is step 1, with no passkey form at
/// all; once it was done, it is the page of who comes back. Reading it writes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn the_entry_follows_the_state_and_a_read_of_it_writes_nothing() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    let before = everything(&fixture).await;
    let first = get_page(browser, "/sign-in", None).await;
    assert_eq!(everything(&fixture).await, before, "a read wrote");
    assert!(first.contains("Welcome to your Arauto hub"));
    assert!(first.contains("Step 1 · Confirm it is you"));
    assert!(first.contains("class=\"stepper\""));
    assert!(first.contains("name=\"code\""));
    assert!(first.contains("<code>docker compose --env-file arauto.env logs hub</code>"));
    // D3: the compose file refuses to run without its variables, so the bare command fails.
    assert!(!first.contains("<code>docker compose logs hub</code>"));
    assert!(first.contains("<code>arauto-hub admin panel code</code>"));
    // No passkey form at all.
    assert!(!first.contains("data-passkey"));
    assert!(!first.contains("/passkey/"));
    assert!(!first.contains("Good to have you back"));
    assert!(!first.contains("<details"));

    browser.first_access(&fixture).await;
    let before = everything(&fixture).await;
    let back = get_page(browser, "/sign-in", None).await;
    assert_eq!(everything(&fixture).await, before, "a read wrote");
    assert!(back.contains("Good to have you back"));
    assert!(back.contains("data-passkey=\"get\""));
    assert!(back.contains("Lost your passkey?"));
    assert!(back.contains("<details class=\"lost card\"><summary>"));
    assert!(back.contains("valid for 10 minutes"));
    assert!(!back.contains("Welcome to your Arauto hub"));
    assert!(!back.contains("class=\"stepper\""));

    // Losing every passkey later does not bring the first access back: the mark is durable.
    sqlx::query("DELETE FROM panel_passkey")
        .execute(&fixture.pool)
        .await
        .unwrap();
    let lost = get_page(browser, "/sign-in", None).await;
    assert!(lost.contains("Good to have you back"));
    assert!(!lost.contains("Welcome to your Arauto hub"));
}

/// R11: the panel serves the one organization the command resolves; with several it shows the
/// page of who comes back and no word of the first access.
#[tokio::test(flavor = "multi_thread")]
async fn a_hub_with_several_organizations_shows_the_returning_page_with_no_first_access_text() {
    let mut fixture = hub().await;
    fixture.stop().await;
    let id = Uuid::new_v4();
    let vault = arauto_hub::vault::Vault::open(&fixture.installation.data_dir).unwrap();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'second', 'active', $2, $3)",
    )
    .bind(id)
    .bind(vault.generate_organization_key(id).unwrap())
    .bind(fixture.now())
    .execute(&fixture.pool)
    .await
    .unwrap();
    // The hub started before had issued a start code for the only organization it knew.
    sqlx::query("DELETE FROM panel_entry")
        .execute(&fixture.pool)
        .await
        .unwrap();
    fixture.start().await;
    let browser = Browser::of(&fixture);
    let page = get_page(browser, "/sign-in", None).await;
    assert!(page.contains("Good to have you back"));
    for text in [
        "Welcome to your Arauto hub",
        "Step 1 · Confirm it is you",
        "class=\"stepper\"",
        "ARAUTO FIRST ACCESS",
    ] {
        assert!(!page.contains(text), "{text}");
    }
    fixture.stop().await;
}

/// R11: a refused code opens the details element of the returning page and says it inside it; a
/// plain read leaves it closed.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_code_opens_the_details_of_the_lost_passkey() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    browser.first_access(&fixture).await;
    let refused = browser.enter(WRONG).await;
    assert_eq!(refused.status, 200);
    let page = refused.text();
    assert!(page.contains("<details class=\"lost card\" open>"));
    assert!(page.contains(REFUSED));
    assert!(page.contains("Good to have you back"));
    assert!(page.find("<summary>").unwrap() < page.find(REFUSED).unwrap());
    // Step 1 says it above the card, and has no details element.
    let fresh = hub().await;
    let page = Browser::of(&fresh).enter(WRONG).await.text();
    assert!(page.contains(REFUSED));
    assert!(page.contains("Welcome to your Arauto hub"));
    assert!(!page.contains("<details"));
}

/// The status, the type and the body of each answer to the codes typed.
async fn answers(browser: Browser, typed: &[&str]) -> Vec<(u16, Option<String>, Vec<u8>)> {
    let mut pages = Vec::new();
    for typed in typed {
        let answer = browser.enter(typed).await;
        assert_eq!(answer.status, 200, "{typed}");
        assert!(cookie_of(&answer).is_none(), "{typed}");
        assert!(answer.text().contains(REFUSED), "{typed}");
        pages.push((
            answer.status,
            answer.header("content-type").map(str::to_owned),
            answer.body,
        ));
    }
    pages
}

/// R12: every refused code gets the same bytes, on each page of the entry. Before the first
/// access: a wrong one, a start code a command code replaced, a command code another replaced
/// and one past its 10 minutes. After it: a wrong one, a spent one, a replaced one and an
/// expired one.
#[tokio::test(flavor = "multi_thread")]
async fn every_refused_code_gets_the_same_page_byte_for_byte() {
    let clock = Arc::new(ManualClock::new(1_800_000_000_000));
    let fixture = Fixture::with_clock(fixture::settings(), clock.clone()).await;
    let browser = Browser::of(&fixture);

    // Before the first access.
    let written = Arc::new(Recorder::default());
    let task = first_access::start(
        fixture.hub(),
        written.clone(),
        DeclaredText::lossy("tester"),
    )
    .await;
    let start_code = written.last_code();
    let replaced_command = code(&fixture).await;
    let expired = code(&fixture).await;
    clock.advance(10 * MINUTE_MS + 1);
    let before = answers(
        browser,
        &[
            WRONG,
            "not-a-code",
            &start_code,
            &replaced_command,
            &expired,
        ],
    )
    .await;
    for page in &before[1..] {
        assert_eq!(page, &before[0]);
    }
    task.stop().await;

    // After it. A spent code: the one of the first access.
    let spent = code(&fixture).await;
    let entered = browser.enter(&spent).await;
    let cookie = cookie_of(&entered).unwrap();
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&cookie, &mut authenticator).await.status,
        303
    );
    // A replaced one: the first of two.
    let replaced = code(&fixture).await;
    let _newer = code(&fixture).await;
    // An expired one.
    let expired = code(&fixture).await;
    clock.advance(10 * MINUTE_MS + 1);
    let after = answers(browser, &[WRONG, "not-a-code", &spent, &replaced, &expired]).await;
    for page in &after[1..] {
        assert_eq!(page, &after[0]);
    }
    assert_ne!(before[0], after[0], "the two entries are two pages");
}

/// R14: a request that does not name the panel is refused before any read, any lookup and any
/// counted attempt, with the fixed texts.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_host_reads_nothing_and_counts_no_attempt() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;
    let before = everything(&fixture).await;

    let body = format!("code={entry}");
    for _ in 0..40 {
        let headers = [("Host", "evil.test"), ("Origin", PANEL_ORIGIN), FORM];
        let answer = http::post(
            browser.panel,
            "/sign-in",
            &headers,
            body.as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 421);
        assert_eq!(
            answer.text(),
            "This is not the address of the panel.\nOpen the panel at the address set in ARAUTO_PANEL_URL: a passkey works only there.\n"
        );
        let answer = http::post(
            browser.panel,
            "/passkey/sign-in/start",
            &[("Host", "evil.test"), ("Origin", PANEL_ORIGIN)],
            b"",
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 421);
    }
    // No panel session, no audit event, nothing at all changed.
    assert_eq!(everything(&fixture).await, before);
    assert!(rows(&fixture, "panel_session").await.is_empty());
    assert_eq!(audited(&fixture, "panel.signed_in").await, 0);
    // And no attempt was counted: the code still opens a session at once.
    let entered = browser.enter(&entry).await;
    assert_eq!(entered.status, 303);
    // Nor in the bucket of challenges: the 40 refused asks above were more than the ceiling of
    // an address, and a challenge is still given at once, once the first access is done.
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let cookie = cookie_of(&entered).unwrap();
    assert_eq!(
        browser.register(&cookie, &mut authenticator).await.status,
        303
    );
    let asked = browser.post("/passkey/sign-in/start", None, "").await;
    assert_eq!(asked.status, 200, "{}", asked.text());

    // A GET with a wrong Host and the cookie of a live panel session is refused before the
    // session is looked up: it does not even count as a use of the session.
    let clock = Arc::new(ManualClock::new(1_800_000_000_000));
    let timed = Fixture::with_clock(fixture::settings(), clock.clone()).await;
    let browser = Browser::of(&timed);
    let cookie = browser.signed_in(&timed).await;
    let before = everything(&timed).await;
    clock.advance(SESSION_IDLE_MS / 2);
    for path in ["/", "/sign-in", "/installation", "/first-access"] {
        let headers = [("Host", "evil.test"), ("Cookie", cookie.as_str())];
        let answer = http::get(browser.panel, path, &headers, PATIENCE)
            .await
            .unwrap();
        assert_eq!(answer.status, 421, "{path}");
    }
    assert_eq!(everything(&timed).await, before);
    // Had a refused request used the session, it would still be alive here.
    clock.advance(SESSION_IDLE_MS / 2 + 1);
    let late = browser.get("/", Some(&cookie)).await;
    assert_eq!(
        (late.status, late.header("location")),
        (303, Some("/sign-in"))
    );

    // The form without the Origin of the panel is refused with its own text.
    let answer = http::post(
        browser.panel,
        "/sign-in",
        &[("Host", PANEL_HOST), FORM],
        b"code=x",
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 403);
    assert_eq!(
        answer.text(),
        "This request did not come from a page of the panel. Open the panel at its address and try again.\n"
    );
}

/// R18: 100 attempts from one address do not stop a code typed from another, and the bucket of
/// codes is not the bucket of challenges. Before the first access there is no challenge.
#[tokio::test(flavor = "multi_thread")]
async fn one_address_that_floods_does_not_stop_another_and_the_buckets_are_apart() {
    let fixture = behind("127.0.0.1").await;
    let browser = Browser::of(&fixture);

    // While the first access was not done there is no passkey to sign in with.
    let none = post_from(browser, "198.51.100.9", "/passkey/sign-in/start", "").await;
    assert_eq!(none.status, 404);

    let entry = code(&fixture).await;
    let wrong = format!("code={WRONG}");
    let mut stopped = 0;
    for _ in 0..100 {
        if post_from(browser, "198.51.100.1", "/sign-in", &wrong)
            .await
            .status
            == 429
        {
            stopped += 1;
        }
    }
    assert_eq!(stopped, 70, "30 asks an address, a minute");
    // A is stopped even with the right code; B is not.
    let right = format!("code={entry}");
    let held = post_from(browser, "198.51.100.1", "/sign-in", &right).await;
    assert_eq!(held.status, 429);
    assert!(cookie_of(&held).is_none());
    let other = post_from(browser, "198.51.100.2", "/sign-in", &right).await;
    assert_eq!(other.status, 303, "{}", other.text());
    let cookie = cookie_of(&other).unwrap();

    // The passkey is registered, so there are challenges now; A, stopped for codes, still gets one.
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&cookie, &mut authenticator).await.status,
        303
    );
    let asked = post_from(browser, "198.51.100.1", "/passkey/sign-in/start", "").await;
    assert_eq!(asked.status, 200, "{}", asked.text());
    // The ceiling of challenges is its own, per address: A floods it and B still asks.
    let mut refused = 0;
    for _ in 0..100 {
        if post_from(browser, "198.51.100.1", "/passkey/sign-in/start", "")
            .await
            .status
            == 429
        {
            refused += 1;
        }
    }
    // 30 asks an address, a minute: one was spent above.
    assert_eq!(refused, 71);
    let asked = post_from(browser, "198.51.100.2", "/passkey/sign-in/start", "").await;
    assert_eq!(asked.status, 200);
}

/// R18: behind a trusted proxy, a client that spoils its `X-Forwarded-For` is still counted with
/// the ceiling of one address, and does not stop a code typed from another.
#[tokio::test(flavor = "multi_thread")]
async fn a_spoiled_forwarded_header_does_not_escape_the_ceiling_of_an_address() {
    let fixture = behind("127.0.0.1").await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;
    let wrong = format!("code={WRONG}");
    for spoiled in ["x", "x, 198.51.100.1"] {
        let mut stopped = 0;
        for _ in 0..100 {
            if post_from(browser, spoiled, "/sign-in", &wrong).await.status == 429 {
                stopped += 1;
            }
        }
        assert!(stopped >= 60, "{spoiled}: {stopped}");
    }
    let right = format!("code={entry}");
    let other = post_from(browser, "198.51.100.2", "/sign-in", &right).await;
    assert_eq!(other.status, 303, "{}", other.text());
}

/// R18: a flood of passkey challenges stops challenges, and the entry it shows does not open the
/// way back from a lost passkey as if a code had been refused.
#[tokio::test(flavor = "multi_thread")]
async fn too_many_challenges_do_not_open_the_details_of_the_codes() {
    let fixture = behind("127.0.0.1").await;
    let browser = Browser::of(&fixture);
    browser.first_access(&fixture).await;
    let mut page = None;
    for _ in 0..60 {
        let answer = post_from(browser, "198.51.100.5", "/passkey/sign-in", "credential=x").await;
        if answer.status == 429 {
            page = Some(answer.text());
            break;
        }
    }
    let page = page.expect("the ceiling of challenges stops a flood");
    assert!(page.contains("Too many attempts right now. Wait a minute and try again."));
    assert!(page.contains("<details class=\"lost card\"><summary>"));
    assert!(!page.contains("<details class=\"lost card\" open>"));
}

/// R18: everyone together has a larger ceiling, which many addresses reach.
#[tokio::test(flavor = "multi_thread")]
async fn many_addresses_together_reach_the_ceiling_of_everyone() {
    let fixture = behind("127.0.0.1").await;
    let browser = Browser::of(&fixture);
    let wrong = format!("code={WRONG}");
    let mut stopped_at = None;
    for index in 0..800u32 {
        let address = format!("203.0.{}.{}", index / 200, index % 200 + 1);
        if post_from(browser, &address, "/sign-in", &wrong)
            .await
            .status
            == 429
        {
            stopped_at = Some(index);
            break;
        }
    }
    let at = stopped_at.expect("the ceiling of everyone stops a crowd");
    // 600 asks of everyone, a minute, from addresses that are never the same twice.
    assert_eq!(at, 600, "the ceiling of everyone");
}

/// The redirect to the check window is only after the first passkey of the hub: a new one in
/// a session that reaches the whole panel, and one after a lost passkey, go home.
#[tokio::test(flavor = "multi_thread")]
async fn only_the_first_passkey_goes_to_the_check_window() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.entered(&fixture).await;
    let mut first = SoftAuthenticator::new(PANEL_ORIGIN);
    let kept = browser.register(&cookie, &mut first).await;
    assert_eq!(kept.header("location"), Some("/installation?welcome"));
    let mut second = SoftAuthenticator::new(PANEL_ORIGIN);
    let kept = browser.register(&cookie, &mut second).await;
    assert_eq!(kept.status, 303);
    assert_eq!(kept.header("location"), Some("/"));

    // The way back from a lost passkey: a command code, a new passkey, and home.
    sqlx::query("DELETE FROM panel_passkey")
        .execute(&fixture.pool)
        .await
        .unwrap();
    let cookie = browser.entered(&fixture).await;
    let mut third = SoftAuthenticator::new(PANEL_ORIGIN);
    let kept = browser.register(&cookie, &mut third).await;
    assert_eq!(kept.header("location"), Some("/"));
}

/// A registration with no panel session, or in a session whose code is no longer unspent, shows
/// the entry of the state with the sentence that the panel session ended.
#[tokio::test(flavor = "multi_thread")]
async fn a_registration_without_a_valid_session_says_the_panel_session_ended() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.entered(&fixture).await;
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    let made = authenticator.create(&asked.text()).unwrap();
    // A newer code cancels the one this session opened.
    let _newer = code(&fixture).await;
    for cookie in [Some(cookie.as_str()), None] {
        let sent = browser
            .post(
                "/passkey/register",
                cookie,
                &format!("credential={}", form_value(&made)),
            )
            .await;
        assert_eq!(sent.status, 200);
        let page = sent.text();
        assert!(page.contains(
            "Your panel session ended. Type the latest code to pick up where you left off."
        ));
        assert!(page.contains("Welcome to your Arauto hub"));
    }
    assert!(rows(&fixture, "panel_passkey").await.is_empty());
}

/// The window that checks the installation: only for a panel session that reaches the whole
/// panel, only a read, with a row for each thing it looks at and the states of the hub.
#[tokio::test(flavor = "multi_thread")]
async fn the_check_window_shows_each_row_and_is_only_for_the_whole_panel() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    // No session, or one that only reaches the registration of a passkey.
    let none = browser.get("/installation", None).await;
    assert_eq!(
        (none.status, none.header("location")),
        (303, Some("/sign-in"))
    );
    let setup = browser.entered(&fixture).await;
    let early = browser.get("/installation", Some(&setup)).await;
    assert_eq!(
        (early.status, early.header("location")),
        (303, Some("/first-access"))
    );

    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&setup, &mut authenticator).await.status,
        303
    );
    let before = everything(&fixture).await;
    let page = get_page(browser, "/installation?welcome", Some(&setup)).await;
    // The window is a read, and so is the page of the registration: they write nothing, not
    // even in the table of the panel sessions (the idle mark is in the memory of the hub).
    assert_eq!(everything(&fixture).await, before, "the window wrote");
    get_page(browser, "/installation", Some(&setup)).await;
    assert_eq!(everything(&fixture).await, before, "the window wrote");
    let again = browser.get("/first-access", Some(&setup)).await;
    assert_eq!(
        (again.status, again.header("location")),
        (303, Some("/")),
        "a session that reaches the whole panel has no page of registration"
    );
    assert_eq!(everything(&fixture).await, before, "a read wrote");
    assert!(page.contains("Step 3 · Let&#39;s check the installation"));
    assert!(page.contains("All set: your passkey was created!"));
    assert!(page.contains("class=\"stepper\""));
    assert!(page.contains("Connect my first Hermes"));
    assert!(page.contains("Do it later"));
    assert!(page.contains("<code>arauto-hub admin check</code>"));

    // The rows, in order, and the state each shows in this hub.
    let labels = [
        ("Panel address", "ok"),
        ("Connectors address", "check"),
        ("Your address", "check"),
        ("Database password", "check"),
        ("Connector", "failed"),
        ("Data folder", "attention"),
        ("WhatsApp engine", "later"),
    ];
    let mut at = 0;
    for (label, state) in labels {
        let found = page[at..].find(&format!("<th scope=\"row\">{label}</th>"));
        at += found.unwrap_or_else(|| panic!("{label} is not in the window"));
        if !state.is_empty() {
            let row = &page[at..at + page[at..].find("</tr>").unwrap()];
            assert!(row.contains(&format!(">{state}<")), "{label}: {row}");
        }
    }
    assert!(page.contains(PANEL_ORIGIN));
    assert!(page.contains("Your passkey works here."));
    assert!(page.contains("Open it: it should say it is the address of the connectors."));
    assert!(page.contains("target=\"_blank\" rel=\"noopener noreferrer\""));
    assert!(page.contains("The downloads folder is empty"));
    assert!(page.contains("Back it up"));
    // A hub that believes nothing (`socket`) sees the peer, and the window says so without
    // advice it cannot follow: ARAUTO_TRUSTED_PROXIES is refused with `socket`.
    assert!(page.contains("The hub sees you at 127.0.0.1. Is it yours?"));
    assert!(!page.contains("ARAUTO_TRUSTED_PROXIES"));
    assert!(page.contains("Not replaced by the hub"));
    assert!(page.contains("A Hermes can be connected without it; WhatsApp numbers need it."));

    // Without `?welcome` it is only the window, with its title; and the footer links to it.
    let plain = get_page(browser, "/installation", Some(&setup)).await;
    assert!(plain.contains("<h2 id=\"check-title\">Check the installation</h2>"));
    assert!(!plain.contains("All set: your passkey was created!"));
    assert!(!plain.contains("class=\"stepper\""));
    for path in ["/", "/whatsapps", "/accesses"] {
        let home = get_page(browser, path, Some(&setup)).await;
        assert!(home.contains("href=\"/installation\""), "{path}");
        assert!(home.contains("Check the installation"), "{path}");
    }
}

/// The window and `arauto-hub admin check` read the same facts by the same function: each row the
/// command prints has the same state in the window, before and after an engine is set.
#[tokio::test(flavor = "multi_thread")]
async fn the_window_and_the_command_show_the_same_rows() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let config = support::config(&fixture.installation, ClientIpSource::Socket);
    let public = config.public_url.origin();
    let env = |name: &str| match name {
        "ARAUTO_PANEL_URL" => Some(PANEL_ORIGIN.to_owned()),
        "ARAUTO_PUBLIC_URL" => Some(public.clone()),
        _ => None,
    };
    for engine in [false, true] {
        if engine {
            let vault = arauto_hub::vault::Vault::open(&fixture.installation.data_dir).unwrap();
            fixture
                .admin
                .engine_set(
                    &vault,
                    "http://192.0.2.10:3000",
                    "http://hub:9000",
                    fixture.now(),
                )
                .await
                .unwrap();
        }
        let page = get_page(browser, "/installation", Some(&cookie)).await;
        let rows = fixture
            .admin
            .check(&config.storage, &fixture.downloads.listed(), &env)
            .await
            .unwrap();
        let printed = check::print(&rows);
        let mut compared = Vec::new();
        for line in printed.lines() {
            let Some((state, label)) = line
                .strip_prefix('[')
                .and_then(|line| line.split_once("] "))
            else {
                continue;
            };
            let at = page
                .find(&format!("<th scope=\"row\">{label}</th>"))
                .unwrap_or_else(|| panic!("{label} is not in the window"));
            let row = &page[at..at + page[at..].find("</tr>").unwrap()];
            // Who reads the window only the running hub sees.
            if label != "Your address" {
                assert!(
                    row.contains(&format!(">{state}<")),
                    "{label}: the command says [{state}], the window {row}"
                );
            }
            compared.push(label.to_owned());
        }
        assert_eq!(compared.len(), 7, "{printed}");
        let engine_line = if engine {
            "[ok] WhatsApp engine"
        } else {
            "[later] WhatsApp engine"
        };
        assert!(printed.contains(engine_line), "{printed}");
        // Neither says where the engine is.
        assert!(!page.contains("192.0.2.10") && !printed.contains("192.0.2.10"));
    }
}

/// A hub that sees a forwarded address from a connection it does not trust says so in the
/// window, with what to add.
#[tokio::test(flavor = "multi_thread")]
async fn the_window_says_when_a_forwarder_is_not_trusted() {
    let fixture = behind("10.77.0.0/16").await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let headers = [
        ("Host", PANEL_HOST),
        ("Cookie", cookie.as_str()),
        ("X-Forwarded-For", "198.51.100.77"),
    ];
    let answer = http::get(browser.panel, "/installation", &headers, PATIENCE)
        .await
        .unwrap();
    assert_eq!(answer.status, 200);
    let page = answer.text();
    assert!(page.contains("which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address"));
}

/// The first window of the connect guide has its steps, and its title while there is no Hermes.
#[tokio::test(flavor = "multi_thread")]
async fn the_connect_guide_says_it_is_the_first_hermes_while_the_list_is_empty() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let page = get_page(browser, "/hermes/connect", Some(&cookie)).await;
    assert!(page.contains("Let&#39;s connect your first Hermes"));
    assert!(page.contains("How it works:"));
    assert!(page.contains("You give the Hermes a name here."));
    assert!(page.contains("verification code of 6 digits"));
    fixture.hermes().await;
    let page = get_page(browser, "/hermes/connect", Some(&cookie)).await;
    assert!(!page.contains("connect your first Hermes"));
    assert!(page.contains("How it works:"));
}

/// The row of one subject in the check window: its state and what it says.
fn window_row(page: &str, label: &str) -> String {
    let at = page
        .find(&format!("<th scope=\"row\">{label}</th>"))
        .unwrap_or_else(|| panic!("{label} is not in the window"));
    page[at..at + page[at..].find("</tr>").unwrap()].to_owned()
}

/// The script path of a registration: the page's script always asks for the challenge first.
/// When the panel session is gone (here: a newer code replaced its code) that ask answers 401, the
/// script then sends the form with no credential, and the hub answers with the page that says the
/// panel session ended, with the dead cookie dropped. A bare 403 would make the script say the hub
/// did not accept the passkey, again and again.
#[tokio::test(flavor = "multi_thread")]
async fn asking_for_a_challenge_in_a_dead_session_leads_to_the_session_ended_page() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.entered(&fixture).await;
    let _newer = code(&fixture).await;

    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    assert_eq!(asked.status, 401, "{}", asked.text());
    // What the script does next: the form, with no credential.
    let sent = browser
        .post("/passkey/register", Some(&cookie), "credential=")
        .await;
    assert_eq!(sent.status, 200);
    let page = sent.text();
    assert!(
        page.contains(
            "Your panel session ended. Type the latest code to pick up where you left off."
        )
    );
    assert!(page.contains("Welcome to your Arauto hub"));
    assert!(rows(&fixture, "panel_passkey").await.is_empty());

    // The script does that, and says "busy" for a 429: both are written in the script itself.
    let script = browser.get("/assets/panel.js", None).await.text();
    assert!(script.contains("answer.status === 401"));
    assert!(script.contains("HTMLFormElement.prototype.submit.call(form)"));
    assert!(script.contains("answer.status === 429"));
    assert!(script.contains("say('busy')"));
}

/// On the page of who comes back, the message that the panel session ended asks for the latest
/// code, so the field of the code (under "Lost your passkey?") is open beside it.
#[tokio::test(flavor = "multi_thread")]
async fn on_the_returning_page_a_session_that_ended_opens_the_field_of_the_code() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    browser.first_access(&fixture).await;
    // The way back from a lost passkey: a command code and a session it opens, and then another
    // command code, which ends that session before the passkey is registered.
    let cookie = browser.entered(&fixture).await;
    let _newer = code(&fixture).await;
    let sent = browser
        .post("/passkey/register", Some(&cookie), "credential=x")
        .await;
    assert_eq!(sent.status, 200);
    let page = sent.text();
    assert!(page.contains("Good to have you back"));
    assert!(page.contains("Your panel session ended."));
    assert!(page.contains("<details class=\"lost card\" open>"));
    let probe = page.find("Your panel session ended.").unwrap();
    assert!(
        probe > page.find("<summary>").unwrap(),
        "the message is beside the field of the code"
    );
}

/// The row of the address the hub sees, with a proxy it trusts that tells nothing about the
/// client: the address of the proxy is already in the list, so the window must not tell the admin
/// to add it. And the peer that is not trusted and brings nothing still gets the advice.
#[tokio::test(flavor = "multi_thread")]
async fn the_address_row_does_not_ask_for_a_proxy_that_is_already_trusted() {
    let fixture = behind("127.0.0.1").await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    // No X-Forwarded-For at all, from a peer that is a trusted proxy.
    let page = get_page(browser, "/installation", Some(&cookie)).await;
    let row = window_row(&page, "Your address");
    assert!(row.contains(">check<"), "{row}");
    assert!(
        row.contains("The hub sees you at 127.0.0.1. Is it yours?"),
        "{row}"
    );
    assert!(!row.contains("not trusted yet"), "{row}");
    assert!(!row.contains("ARAUTO_TRUSTED_PROXIES"), "{row}");

    // A peer outside the list, no header: the proxy is probably not trusted yet.
    let fixture = behind("10.77.0.0/16").await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let page = get_page(browser, "/installation", Some(&cookie)).await;
    let row = window_row(&page, "Your address");
    assert!(row.contains(">attention<"), "{row}");
    assert!(row.contains("the proxy is not trusted yet"), "{row}");
}

/// The window and the command agree on the connector row when the hub serves a connector, which
/// they read from the same folder by the same function.
#[tokio::test(flavor = "multi_thread")]
async fn the_window_and_the_command_agree_when_a_connector_is_served() {
    let mut fixture = Fixture::stopped(
        support::install().await,
        fixture::settings(),
        Arc::new(SystemClock),
    )
    .await;
    fixture.downloads = arauto_hub::channel::Downloads::empty()
        .with_file(
            "arauto-connector-linux-x86_64",
            bytes::Bytes::from_static(b"abc"),
        )
        .unwrap();
    fixture.start().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let page = get_page(browser, "/installation", Some(&cookie)).await;
    let row = window_row(&page, "Connector");
    assert!(
        row.contains(">ok<") && row.contains("Ready for your Hermes."),
        "{row}"
    );

    let config = support::config(&fixture.installation, ClientIpSource::Socket);
    let env = |_: &str| -> Option<String> { None };
    let rows = fixture
        .admin
        .check(&config.storage, &fixture.downloads.listed(), &env)
        .await
        .unwrap();
    let printed = check::print(&rows);
    assert!(printed.contains("[ok] Connector"), "{printed}");
    assert!(printed.contains("Ready for your Hermes."), "{printed}");
}
