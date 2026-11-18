//! The WhatsApp numbers in the panel, on the fake engine: who reaches each address, what each
//! form writes (the same as the admin command), how a pairing runs and ends in its window, what
//! the lines of the list offer, and that the placeholder of the native engine does nothing.
//!
//! Never a real engine, never a real number, never a real browser.

use std::collections::BTreeSet;
use std::time::Duration;

use arauto_hub::admin::qr::QrCode;
use arauto_hub::core::number::NumberState;
use arauto_protocol::clock::SystemClock;
use arauto_testkit::engine::Fault;
use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::support::PANEL_HOST;
use arauto_testkit::passkey::SoftAuthenticator;

use crate::world::{World, phone};
use crate::{Browser, PATIENCE, audited, cookie_of, form_value, hub, rows};

const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

/// A hub with the fake engine, a browser and a session that reaches the whole panel.
pub struct Rig {
    pub world: World,
    pub browser: Browser,
    pub cookie: String,
    pub authenticator: SoftAuthenticator,
}

pub async fn rig_with(pairing_ms: i64) -> Rig {
    rig_polling(pairing_ms, 50).await
}

pub async fn rig_polling(pairing_ms: i64, poll_ms: i64) -> Rig {
    let world = World::with(
        |settings| {
            settings.engine.pairing_ms = pairing_ms;
            settings.engine.pairing_poll_ms = poll_ms;
        },
        std::sync::Arc::new(SystemClock),
    )
    .await;
    let browser = Browser::of(&world.fixture);
    let (cookie, authenticator) = browser.first_access(&world.fixture).await;
    Rig {
        world,
        browser,
        cookie,
        authenticator,
    }
}

pub async fn rig() -> Rig {
    rig_with(30_000).await
}

impl Rig {
    pub async fn get(&self, path: &str) -> Response {
        self.browser.get(path, Some(&self.cookie)).await
    }

    pub async fn post(&self, path: &str, body: &str) -> Response {
        self.browser.post(path, Some(&self.cookie), body).await
    }

    /// Another session of the same admin, signed in with the passkey.
    pub async fn other_session(&mut self) -> String {
        let answer = self.browser.pass(&mut self.authenticator).await;
        assert_eq!(answer.status, 303, "{}", answer.text());
        cookie_of(&answer).expect("a sign-in sets the cookie")
    }

    /// Adds a number through the form of the panel. The first one needs the acceptance.
    pub async fn add(&self, alias: &str, digits: &str) {
        let first = self
            .world
            .fixture
            .admin
            .number_list(true)
            .await
            .unwrap()
            .is_empty();
        let accept = if first { "&accept=on" } else { "" };
        let answer = self
            .post(
                "/whatsapps/add",
                &format!("alias={alias}&phone=%2B{digits}{accept}"),
            )
            .await;
        assert_eq!(answer.status, 303, "{}", answer.text());
        assert_eq!(
            answer.header("location"),
            Some(format!("/whatsapps/pair?alias={alias}").as_str())
        );
    }

    /// Starts a pairing from `cookie`. Returns the run id.
    pub async fn start_with(&self, cookie: &str, alias: &str, method: &str) -> Response {
        self.browser
            .post(
                "/whatsapps/pair/start",
                Some(cookie),
                &format!("alias={alias}&method={method}"),
            )
            .await
    }

    pub async fn start(&self, alias: &str, method: &str) -> String {
        let answer = self.start_with(&self.cookie, alias, method).await;
        assert_eq!(answer.status, 303, "{}", answer.text());
        run_of(&answer)
    }

    pub fn wait_path(alias: &str, run: &str) -> String {
        format!("/whatsapps/pair/wait?alias={alias}&run={run}")
    }

    /// Asks the window of a pairing until `text` is in it. Returns the page.
    pub async fn until(&self, cookie: &str, path: &str, text: &str) -> String {
        for _ in 0..200 {
            let page = self.browser.get(path, Some(cookie)).await.text();
            if page.contains(text) {
                return page;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!(
            "never showed {text}: {}",
            self.browser.get(path, Some(cookie)).await.text()
        );
    }

    pub async fn count(&self, table: &str) -> usize {
        rows(&self.world.fixture, table).await.len()
    }
}

fn run_of(answer: &Response) -> String {
    answer
        .header("location")
        .and_then(|location| location.split("run=").nth(1))
        .expect("the answer leads to the window of the run")
        .to_owned()
}

/// The dialog of a page: what stands over the list.
fn dialog(page: &str) -> &str {
    page.split("<dialog").nth(1).expect("the page has a window")
}

/// The dark modules a drawn QR holds, read back from its markup.
fn modules_of(page: &str) -> BTreeSet<(usize, usize)> {
    let svg = page
        .split("<svg class=\"qr\"")
        .nth(1)
        .expect("the page draws a QR");
    let d = svg.split("d=\"").nth(1).unwrap().split('"').next().unwrap();
    let mut found = BTreeSet::new();
    for segment in d.split('M').filter(|segment| !segment.is_empty()) {
        let (at, rest) = segment.split_once('h').unwrap();
        let (x, y) = at.split_once(' ').unwrap();
        let run: usize = rest.split('v').next().unwrap().parse().unwrap();
        let (x, y): (usize, usize) = (x.parse().unwrap(), y.parse().unwrap());
        for dx in 0..run {
            found.insert((x - 4 + dx, y - 4));
        }
    }
    found
}

fn modules_of_value(value: &str) -> BTreeSet<(usize, usize)> {
    let code = QrCode::encode(value.as_bytes()).unwrap();
    (0..code.size())
        .flat_map(|y| (0..code.size()).map(move |x| (x, y)))
        .filter(|(x, y)| code.dark(*x, *y))
        .collect()
}

/// Whether the QR drawn on a page is the one of `2@fake-qr-value-N,session` for some N.
fn draws_a_value_of(page: &str, session: &str) -> bool {
    let drawn = modules_of(page);
    (1..400).any(|n| modules_of_value(&format!("2@fake-qr-value-{n},{session}")) == drawn)
}

async fn snapshot(rig: &Rig) -> (Vec<String>, i64, usize) {
    (
        rows(&rig.world.fixture, "number").await,
        sqlx::query_scalar("SELECT count(*) FROM audit")
            .fetch_one(&rig.world.fixture.pool)
            .await
            .unwrap(),
        // What changes the engine: every request but a read.
        rig.world
            .fake
            .received()
            .iter()
            .filter(|request| request.method != "GET")
            .count(),
    )
}

const GETS: [&str; 8] = [
    "/whatsapps/add",
    "/whatsapps/pair?alias=x",
    "/whatsapps/pair/wait?alias=x",
    "/whatsapps/decide?alias=x",
    "/whatsapps/disconnect?alias=x",
    "/whatsapps/remove?alias=x",
    "/whatsapps/native",
    "/whatsapps/pair/wait?alias=x&run=y&shown=3",
];

const POSTS: [(&str, &str); 8] = [
    ("/whatsapps/add", "alias=x&phone=%2B5511987650000&accept=on"),
    ("/whatsapps/pair/start", "alias=x&method=qr"),
    ("/whatsapps/pair/cancel", "alias=x&run=y"),
    ("/whatsapps/confirm", "alias=x"),
    ("/whatsapps/undo", "alias=x&erase=on"),
    ("/whatsapps/restart", "alias=x"),
    ("/whatsapps/disconnect", "alias=x&typed=x"),
    ("/whatsapps/remove", "alias=x&typed=x&removal=keep"),
];

#[tokio::test(flavor = "multi_thread")]
async fn every_number_route_needs_a_full_panel_session() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let by_code = browser.entered(&fixture).await;
    let before = crate::session::everything(&fixture).await;
    for cookie in [None, Some("arauto-panel=ended"), Some(by_code.as_str())] {
        for path in GETS {
            let answer = browser.get(path, cookie).await;
            assert_eq!(answer.status, 303, "{path} {cookie:?}");
            assert_eq!(answer.header("location"), Some("/sign-in"), "{path}");
            assert!(answer.body.is_empty());
        }
        for (path, body) in POSTS {
            let answer = browser.post(path, cookie, body).await;
            assert_eq!(answer.status, 303, "{path} {cookie:?}");
            assert_eq!(answer.header("location"), Some("/sign-in"), "{path}");
        }
    }
    // Nothing was written: the rows are the ones of before, but for the panel's own use.
    let after = crate::session::everything(&fixture).await;
    for ((table, was), (_, is)) in before.iter().zip(&after) {
        if table != "panel_session" && table != "panel_entry" {
            assert_eq!(was, is, "{table}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_number_forms_are_refused_from_another_origin() {
    let rig = rig().await;
    let before = snapshot(&rig).await;
    let panel = rig.browser.panel;
    for origin in [Some("https://evil.example"), Some("null"), None] {
        for (path, body) in POSTS {
            let mut headers = vec![
                ("Host", PANEL_HOST),
                ("Content-Type", "application/x-www-form-urlencoded"),
                ("Cookie", rig.cookie.as_str()),
            ];
            if let Some(origin) = origin {
                headers.push(("Origin", origin));
            }
            let answer = http::post(panel, path, &headers, body.as_bytes(), PATIENCE)
                .await
                .unwrap();
            assert_eq!(answer.status, 403, "{path} {origin:?}");
        }
    }
    assert_eq!(
        snapshot(&rig).await,
        before,
        "nothing was written or sent to the engine"
    );
    assert_eq!(audited(&rig.world.fixture, "number.added").await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_native_window_has_no_form_and_changes_nothing() {
    let rig = rig().await;
    let list = rig.get("/whatsapps").await.text();
    assert!(list.contains("class=\"button trial\""), "{list}");
    assert!(list.contains("Connect WhatsApp (native · in test)"));
    assert!(list.contains("Connect WhatsApp (WAHA)"));
    let before = snapshot(&rig).await;
    let window = rig.get("/whatsapps/native").await;
    assert_eq!(window.status, 200);
    let page = window.text();
    let window = dialog(&page);
    assert!(
        !window.contains("<form") && !window.contains("method=\"post\""),
        "{window}"
    );
    assert!(!window.contains("data-start") && !window.contains("<script"));
    assert!(window.contains("class=\"tag trial\"") && window.contains("In test"));
    assert!(window.contains("is being studied"));
    assert_eq!(rig.post("/whatsapps/native", "x=1").await.status, 404);
    assert_eq!(snapshot(&rig).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_number_asks_for_the_acceptance_and_later_ones_do_not() {
    let rig = rig().await;
    let window = rig.get("/whatsapps/add").await.text();
    assert!(dialog(&window).contains("name=\"accept\""), "{window}");
    // Without the acceptance, nothing is added.
    let refused = rig
        .post(
            "/whatsapps/add",
            &format!("alias=vendas&phone=%2B{}", phone(60)),
        )
        .await;
    assert_eq!(refused.status, 200);
    assert!(refused.text().contains("Nothing was added."));
    assert_eq!(rig.count("number").await, 0);
    assert_eq!(audited(&rig.world.fixture, "number.added").await, 0);
    // With it: the add, with the acceptance in its audit row and no "command" flag.
    let added = rig
        .post(
            "/whatsapps/add",
            &format!("alias=vendas&phone=%2B{}&accept=on", phone(60)),
        )
        .await;
    assert_eq!(added.status, 303, "{}", added.text());
    let number = rig.world.number("vendas").await;
    assert_eq!(number.state, NumberState::Unpaired);
    assert_eq!(
        rig.world.fake.status(number.session.as_str()).as_deref(),
        Some("STOPPED"),
        "its session was created in the engine"
    );
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT actor_note, details::text FROM audit WHERE action = 'number.added'")
            .fetch_all(&rig.world.fixture.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "panel");
    assert!(
        rows[0].1.contains("risk_accepted") && !rows[0].1.contains("command"),
        "{}",
        rows[0].1
    );
    // The second number asks for nothing and records no acceptance.
    let window = rig.get("/whatsapps/add").await.text();
    assert!(!dialog(&window).contains("name=\"accept\""));
    assert!(!dialog(&window).contains("name=\"command\""));
    rig.add("filial", &phone(61)).await;
    let details: Vec<String> = sqlx::query_scalar(
        "SELECT details::text FROM audit WHERE action = 'number.added' ORDER BY seq",
    )
    .fetch_all(&rig.world.fixture.pool)
    .await
    .unwrap();
    assert_eq!(details.len(), 2);
    assert!(!details[1].contains("risk_accepted"), "{}", details[1]);
    // The command path records no acceptance either.
    rig.world.add("command", &phone(62)).await;
    let last: String = sqlx::query_scalar(
        "SELECT details::text FROM audit WHERE action = 'number.added' ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&rig.world.fixture.pool)
    .await
    .unwrap();
    assert!(!last.contains("risk_accepted"));
    // The same alias, or the same phone, is refused in the window with its sentence.
    for (alias, digits, said) in [
        ("vendas", phone(63), "That alias is taken"),
        ("outro", phone(60), "already a WhatsApp of this hub"),
        ("outro", "12".to_owned(), "from 8 to 15 digits"),
        ("bad.alias", phone(64), "Use only lowercase letters"),
    ] {
        let answer = rig
            .post("/whatsapps/add", &format!("alias={alias}&phone={digits}"))
            .await;
        assert_eq!(answer.status, 200, "{alias} {digits}");
        assert!(answer.text().contains(said), "{alias}: {}", answer.text());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hostile_values_reach_the_number_windows_only_as_text() {
    let rig = rig().await;
    let answer = rig
        .post(
            "/whatsapps/add",
            &format!(
                "alias={}&phone={}&accept=on",
                form_value(HOSTILE),
                form_value(HOSTILE)
            ),
        )
        .await;
    assert_eq!(answer.status, 200);
    let page = answer.text();
    assert!(
        !page.contains("<script>alert") && !page.contains("<img src=x"),
        "{page}"
    );
    assert!(page.contains("&lt;script&gt;"));
    // What the engine serves as a QR is drawn, never written.
    rig.add("vendas", &phone(65)).await;
    let number = rig.world.number("vendas").await;
    let hostile_qr = "\"><script>alert(1)</script>";
    rig.world
        .fake
        .set_qr_value(number.session.as_str(), hostile_qr);
    let run = rig.start("vendas", "qr").await;
    let page = rig
        .until(
            &rig.cookie,
            &Rig::wait_path("vendas", &run),
            "<svg class=\"qr\"",
        )
        .await;
    assert!(!page.contains("alert(1)"), "{page}");
    assert_eq!(modules_of(&page), modules_of_value(hostile_qr));
    // A code that is not a code is refused by the hub: the run ends with a fixed sentence.
    rig.add("filial", &phone(66)).await;
    rig.world.fake.set_pairing_code("<b>x</b>");
    let run = rig.start("filial", "code").await;
    let page = rig
        .until(
            &rig.cookie,
            &Rig::wait_path("filial", &run),
            "answered with an error",
        )
        .await;
    assert!(!page.contains("<b>x</b>"));
    assert_eq!(rig.world.state("filial").await, NumberState::Unpaired);
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_by_qr_from_the_panel_connects_the_expected_phone() {
    let rig = rig().await;
    rig.add("vendas", &phone(70)).await;
    let number = rig.world.number("vendas").await;
    let session = number.session.as_str();
    // The method window names the number and offers both ways.
    let method = rig.get("/whatsapps/pair?alias=vendas").await.text();
    assert!(
        method.contains("name=\"method\" value=\"qr\" checked"),
        "{method}"
    );
    assert!(method.contains(&format!("+{}", phone(70))));
    let run = rig.start("vendas", "qr").await;
    let path = Rig::wait_path("vendas", &run);
    let page = rig.until(&rig.cookie, &path, "<svg class=\"qr\"").await;
    assert!(
        draws_a_value_of(&page, session),
        "the window draws what the engine served"
    );
    assert!(page.contains("pair.js") && page.contains("http-equiv=\"refresh\""));
    // Each look at the window may carry another QR, and the one it already has is not sent
    // again: the hub leaves it out when the window says which one it holds.
    let serial = page
        .split("data-serial=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let same = rig.get(&format!("{path}&shown={serial}")).await.text();
    assert!(
        !same.contains("<svg class=\"qr\"")
            || !same.contains(&format!("data-serial=\"{serial}\""))
            || same.contains("data-same")
    );
    // The telephone scans it.
    rig.world.fake.pairs_as(session, &phone(70));
    let page = rig.until(&rig.cookie, &path, "Paired:").await;
    assert!(page.contains(&format!("+{}", phone(70))));
    assert!(
        !page.contains("<svg class=\"qr\""),
        "an ended window draws no QR"
    );
    assert_eq!(rig.world.state("vendas").await, NumberState::Connected);
    assert_eq!(
        audited(&rig.world.fixture, "number.pairing_started").await,
        1
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_by_code_shows_the_code_in_its_window_only() {
    // A slow poll: the phone "types" the code at the next one, after the other session looked.
    let mut rig = rig_polling(30_000, 1_500).await;
    rig.add("vendas", &phone(71)).await;
    let session = rig.world.number("vendas").await.session;
    let other = rig.other_session().await;
    // The code is requested once, so the phone that will type it is known before it starts.
    rig.world.fake.pairs_as(session.as_str(), &phone(71));
    let run = rig.start("vendas", "code").await;
    let path = Rig::wait_path("vendas", &run);
    let page = rig.until(&rig.cookie, &path, "ABCD-EFGH").await;
    assert!(page.contains("Link with phone number"));
    // Another session never gets it: not with the run, not with the list of the number.
    let seen = rig.browser.get(&path, Some(&other)).await.text();
    assert!(seen.contains("being paired in another window"), "{seen}");
    assert!(!seen.contains("ABCD-EFGH"));
    let list = rig.browser.get("/whatsapps", Some(&other)).await.text();
    assert!(list.contains("See the pairing") && !list.contains("ABCD-EFGH"));
    assert!(
        rig.get("/whatsapps")
            .await
            .text()
            .contains("Continue pairing")
    );
    let page = rig.until(&rig.cookie, &path, "Paired:").await;
    assert!(!page.contains("ABCD-EFGH"), "gone after the end");
}

#[tokio::test(flavor = "multi_thread")]
async fn another_phone_waits_for_the_decision() {
    let rig = rig().await;
    rig.add("keep", &phone(72)).await;
    rig.add("undo", &phone(73)).await;
    for (alias, expected) in [("keep", 72), ("undo", 73)] {
        let session = rig.world.number(alias).await.session;
        rig.world
            .fake
            .pairs_as(session.as_str(), &phone(expected + 20));
        let run = rig.start(alias, "qr").await;
        let page = rig
            .until(
                &rig.cookie,
                &Rig::wait_path(alias, &run),
                "belongs to a number different",
            )
            .await;
        assert!(page.contains("Keep this one"));
        assert_eq!(rig.world.state(alias).await, NumberState::NumberSwapped);
        assert_eq!(
            rig.get(&format!("/whatsapps/decide?alias={alias}"))
                .await
                .status,
            200
        );
        // The pairing screen of a swapped number leads to the decision.
        assert_eq!(
            rig.get(&format!("/whatsapps/pair?alias={alias}"))
                .await
                .header("location"),
            Some(format!("/whatsapps/decide?alias={alias}").as_str())
        );
    }
    let kept = rig.post("/whatsapps/confirm", "alias=keep").await;
    assert_eq!(kept.status, 303, "{}", kept.text());
    assert_eq!(rig.world.state("keep").await, NumberState::Connected);
    // Undo without the checkbox does nothing; with it, the session leaves WhatsApp.
    let session = rig.world.number("undo").await.session;
    let refused = rig.post("/whatsapps/undo", "alias=undo").await;
    assert_eq!(refused.status, 200);
    assert!(refused.text().contains("Mark that you understand"));
    assert_eq!(rig.world.state("undo").await, NumberState::NumberSwapped);
    assert!(rig.world.fake.status(session.as_str()).is_some());
    let undone = rig.post("/whatsapps/undo", "alias=undo&erase=on").await;
    assert_eq!(undone.status, 303, "{}", undone.text());
    assert_eq!(rig.world.state("undo").await, NumberState::Unpaired);
    assert_eq!(
        rig.world.fake.status(session.as_str()),
        None,
        "deleted in the engine"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pairing_nobody_uses_expires_and_stops_the_engine_session() {
    let rig = rig_with(1_200).await;
    rig.add("vendas", &phone(74)).await;
    let session = rig.world.number("vendas").await.session;
    let run = rig.start("vendas", "qr").await;
    let page = rig
        .until(
            &rig.cookie,
            &Rig::wait_path("vendas", &run),
            "Nothing was scanned or typed in time",
        )
        .await;
    assert!(page.contains("Pair again") && !page.contains("<svg class=\"qr\""));
    assert_eq!(rig.world.state("vendas").await, NumberState::Unpaired);
    assert_eq!(
        rig.world.fake.status(session.as_str()).as_deref(),
        Some("STOPPED")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_ends_the_run_stops_the_engine_session_and_unpairs() {
    let mut rig = rig().await;
    rig.add("first", &phone(75)).await;
    rig.add("second", &phone(76)).await;
    let other = rig.other_session().await;
    let (a, b) = (
        rig.world.number("first").await.session,
        rig.world.number("second").await.session,
    );
    // From the session that started it.
    let run = rig.start("first", "qr").await;
    let answer = rig
        .post("/whatsapps/pair/cancel", &format!("alias=first&run={run}"))
        .await;
    assert_eq!(answer.status, 303);
    assert_eq!(answer.header("location"), Some("/whatsapps?alias=first"));
    assert_eq!(rig.world.state("first").await, NumberState::Unpaired);
    assert_eq!(
        rig.world.fake.status(a.as_str()).as_deref(),
        Some("STOPPED")
    );
    let page = rig.get(&Rig::wait_path("first", &run)).await.text();
    assert!(page.contains("The pairing was cancelled"), "{page}");
    // From another session of the same admin.
    let run = rig.start("second", "qr").await;
    let answer = rig
        .browser
        .post(
            "/whatsapps/pair/cancel",
            Some(&other),
            &format!("alias=second&run={run}"),
        )
        .await;
    assert_eq!(answer.status, 303);
    assert_eq!(rig.world.state("second").await, NumberState::Unpaired);
    assert_eq!(
        rig.world.fake.status(b.as_str()).as_deref(),
        Some("STOPPED")
    );
    // A pairing can start again afterwards.
    let run = rig.start("first", "qr").await;
    assert!(!run.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn one_run_per_number_and_three_per_organization() {
    let mut rig = rig().await;
    for (n, alias) in ["a", "b", "c", "d"].into_iter().enumerate() {
        rig.add(alias, &phone(80 + u32::try_from(n).unwrap())).await;
    }
    let other = rig.other_session().await;
    rig.start("a", "qr").await;
    // The same number from another window: it is shown who has it, never the QR.
    let again = rig.start_with(&other, "a", "qr").await;
    assert_eq!(again.status, 200);
    assert!(again.text().contains("being paired in another window"));
    assert!(!again.text().contains("<svg class=\"qr\""));
    // And from the same session: the same run is shown.
    let same = rig.start_with(&rig.cookie, "a", "qr").await;
    assert_eq!(same.status, 303, "{}", same.text());
    rig.start("b", "code").await;
    rig.start("c", "qr").await;
    let fourth = rig.start_with(&rig.cookie, "d", "qr").await;
    assert_eq!(fourth.status, 200);
    assert!(
        fourth
            .text()
            .contains("3 numbers are being paired right now")
    );
    assert_eq!(rig.world.state("d").await, NumberState::Unpaired);
}

/// The row of the list of one number, as markup.
fn line(list: &str, alias: &str) -> String {
    let start = list
        .find(&format!("<tr><td><span class=\"name\">{alias}</span>"))
        .unwrap_or_else(|| panic!("no line for {alias}: {list}"));
    let rest = &list[start..];
    rest[..rest.find("</tr>").unwrap()].to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_lines_offer_what_the_state_allows() {
    let rig = rig().await;
    rig.add("unpaired", &phone(90)).await;
    rig.world.connected("online", &phone(91)).await;
    let failed = rig.world.connected("broken", &phone(92)).await;
    let down = rig.world.connected("away", &phone(93)).await;
    rig.add("busy", &phone(94)).await;
    rig.world.fake.set_status(failed.session.as_str(), "FAILED");
    rig.world.engine().await;
    arauto_hub::engine::supervise(rig.world.hub()).await;
    rig.world
        .fixture
        .admin
        .number_disconnect(&rig.world.vault, "away", rig.world.now())
        .await
        .unwrap();
    assert_eq!(rig.world.state("broken").await, NumberState::Failed);
    assert_eq!(rig.world.state("away").await, NumberState::Disconnected);
    let _ = down;
    let run = rig.start("busy", "qr").await;
    let list = rig.get("/whatsapps").await.text();
    // Each line has one button, and the window behind it lists what the state allows.
    let mut windows = std::collections::HashMap::new();
    for alias in ["unpaired", "online", "broken", "away", "busy"] {
        assert!(line(&list, alias).contains(&format!(
            "href=\"/whatsapps/actions?alias={alias}\" aria-label=\"Actions: {alias}\""
        )));
        assert!(!line(&list, alias).contains("/whatsapps/remove?"));
        windows.insert(
            alias,
            rig.get(&format!("/whatsapps/actions?alias={alias}"))
                .await
                .text(),
        );
    }
    let has = |alias: &str, wanted: &[&str], unwanted: &[&str]| {
        let line = &windows[alias];
        for want in wanted {
            assert!(line.contains(want), "{alias} lacks {want}: {line}");
        }
        for not in unwanted {
            assert!(!line.contains(not), "{alias} has {not}: {line}");
        }
    };
    has(
        "unpaired",
        &[
            "/whatsapps/pair?alias=unpaired",
            "/whatsapps/remove?alias=unpaired",
        ],
        &["disconnect", "restart"],
    );
    has(
        "online",
        &[
            "/whatsapps/disconnect?alias=online",
            "/whatsapps/remove?alias=online",
        ],
        &["/whatsapps/pair?", "restart"],
    );
    has(
        "broken",
        &[
            "action=\"/whatsapps/restart\"",
            "/whatsapps/remove?alias=broken",
        ],
        &["/whatsapps/pair?", "disconnect"],
    );
    has(
        "away",
        &[
            "/whatsapps/pair?alias=away",
            "Pair again",
            "/whatsapps/remove?alias=away",
        ],
        &["restart"],
    );
    has(
        "busy",
        &[
            &format!("/whatsapps/pair/wait?alias=busy&amp;run={run}"),
            "Continue pairing",
            "/whatsapps/remove?alias=busy",
        ],
        &["/whatsapps/pair?"],
    );
    // What ends something comes last, in a group of its own.
    let online = &windows["online"];
    assert!(
        online.find("<ul class=\"acts end\">").unwrap()
            < online.find("/whatsapps/disconnect?alias=online").unwrap()
    );
    assert!(
        online.find("/whatsapps/orders?alias=online").unwrap()
            < online.find("<ul class=\"acts end\">").unwrap()
    );
    assert_eq!(rig.get("/whatsapps/actions?alias=nobody").await.status, 303);
    // A number that is not in a state to pair is told so, with nothing started.
    let answer = rig
        .post("/whatsapps/pair/start", "alias=online&method=qr")
        .await;
    assert_eq!(answer.status, 200);
    assert!(
        answer
            .text()
            .contains("This cannot be done while the number is in the state &quot;Connected&quot;")
    );
    assert_eq!(rig.world.state("online").await, NumberState::Connected);
    let answer = rig
        .post("/whatsapps/pair/start", "alias=unpaired&method=smoke")
        .await;
    assert!(answer.text().contains("Choose QR code or pairing code"));
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_disconnect_and_remove_from_the_panel() {
    let rig = rig().await;
    let broken = rig.world.connected("broken", &phone(95)).await;
    rig.world.connected("online", &phone(96)).await;
    let erased = rig.world.connected("erased", &phone(97)).await;
    let online = rig.world.number("online").await;
    rig.world.fake.set_status(broken.session.as_str(), "FAILED");
    arauto_hub::engine::supervise(rig.world.hub()).await;
    assert_eq!(rig.world.state("broken").await, NumberState::Failed);
    // Restart: one click.
    let answer = rig.post("/whatsapps/restart", "alias=broken").await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    assert_eq!(rig.world.state("broken").await, NumberState::Reconnecting);
    assert!(
        rig.world
            .fake
            .received()
            .iter()
            .any(|r| r.path.ends_with("/restart"))
    );
    // Disconnect needs the alias typed.
    for body in [
        "alias=online&typed=",
        "alias=online&typed=other",
        "alias=online",
    ] {
        let answer = rig.post("/whatsapps/disconnect", body).await;
        assert_eq!(answer.status, 200);
        assert!(answer.text().contains("The alias typed does not match"));
        assert_eq!(rig.world.state("online").await, NumberState::Connected);
    }
    let answer = rig
        .post("/whatsapps/disconnect", "alias=online&typed=online")
        .await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    assert_eq!(rig.world.state("online").await, NumberState::Disconnected);
    assert_eq!(
        rig.world.fake.status(online.session.as_str()).as_deref(),
        Some("STOPPED")
    );
    // Remove needs the alias and a choice, and none is assumed.
    let window = rig.get("/whatsapps/remove?alias=online").await.text();
    assert!(
        !dialog(&window).contains("checked"),
        "no choice is made for the admin"
    );
    let answer = rig
        .post("/whatsapps/remove", "alias=online&typed=online")
        .await;
    assert!(answer.text().contains("Choose what happens to its session"));
    assert_eq!(rig.world.state("online").await, NumberState::Disconnected);
    let answer = rig
        .post("/whatsapps/remove", "alias=online&typed=nope&removal=keep")
        .await;
    assert!(answer.text().contains("The alias typed does not match"));
    let answer = rig
        .post(
            "/whatsapps/remove",
            "alias=online&typed=online&removal=keep",
        )
        .await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    assert!(
        rig.world.fake.status(online.session.as_str()).is_some(),
        "kept in the engine"
    );
    let answer = rig
        .post(
            "/whatsapps/remove",
            "alias=erased&typed=erased&removal=erase",
        )
        .await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    assert_eq!(
        rig.world.fake.status(erased.session.as_str()),
        None,
        "deleted in the engine"
    );
    let list = rig.get("/whatsapps").await.text();
    assert!(!list.contains("<span class=\"name\">online</span>"));
    // The alias is never used again.
    let answer = rig
        .post(
            "/whatsapps/add",
            &format!("alias=online&phone=%2B{}", phone(98)),
        )
        .await;
    assert!(answer.text().contains("That alias is taken"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_engine_that_does_not_answer_is_said_plainly() {
    let rig = rig().await;
    let pool = &rig.world.fixture.pool;
    // A key the engine refuses.
    rig.world.fake.set_api_key("another-key-entirely-0001");
    rig.add("vendas", &phone(100)).await;
    let run = rig.start("vendas", "qr").await;
    let path = Rig::wait_path("vendas", &run);
    rig.until(&rig.cookie, &path, "WAHA refused the key").await;
    assert_eq!(rig.world.state("vendas").await, NumberState::Unpaired);
    rig.world.fake.set_api_key(crate::world::KEY);
    // An engine that answers with an error.
    rig.world.fake.fail("", Fault::ServerError);
    let run = rig.start("vendas", "qr").await;
    rig.until(
        &rig.cookie,
        &Rig::wait_path("vendas", &run),
        "answered with an error",
    )
    .await;
    rig.world.fake.heal();
    // An engine nobody listens at.
    let events = format!("http://{}", rig.world.fixture.running().internal_addr());
    rig.world
        .fixture
        .admin
        .engine_set(
            &rig.world.vault,
            "http://127.0.0.1:1",
            &events,
            rig.world.now(),
        )
        .await
        .unwrap();
    rig.world
        .fixture
        .admin
        .engine_set_key(&rig.world.vault, crate::world::KEY, rig.world.now())
        .await
        .unwrap();
    let run = rig.start("vendas", "qr").await;
    rig.until(
        &rig.cookie,
        &Rig::wait_path("vendas", &run),
        "could not reach WAHA",
    )
    .await;
    assert_eq!(rig.world.state("vendas").await, NumberState::Unpaired);
    // No key: no form, and the command that sets it, never a field for the key.
    sqlx::query("UPDATE engine SET api_key_enc = NULL")
        .execute(pool)
        .await
        .unwrap();
    let window = rig.get("/whatsapps/add").await.text();
    let window = dialog(&window);
    assert!(
        !window.contains("<form") && window.contains("The engine has no key yet"),
        "{window}"
    );
    assert!(window.contains("arauto-hub admin engine set-key"));
    assert!(!window.contains("type=\"password\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn with_no_engine_the_window_points_to_the_command() {
    let rig = rig().await;
    sqlx::query("DELETE FROM engine")
        .execute(&rig.world.fixture.pool)
        .await
        .unwrap();
    let window = rig.get("/whatsapps/add").await.text();
    let window = dialog(&window);
    assert!(
        !window.contains("<form") && window.contains("There is no WhatsApp engine"),
        "{window}"
    );
    assert!(window.contains("arauto-hub admin engine set --url URL --events-url URL"));
    let answer = rig
        .post(
            "/whatsapps/add",
            &format!("alias=vendas&phone=%2B{}&accept=on", phone(99)),
        )
        .await;
    assert_eq!(answer.status, 200);
    assert!(answer.text().contains("There is no WhatsApp engine"));
    assert_eq!(rig.count("number").await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_the_engine_did_not_do_is_said_on_the_list() {
    let rig = rig().await;
    let online = rig.world.connected("online", &phone(101)).await;
    rig.world.fake.fail("stop", Fault::ServerError);
    let answer = rig
        .post("/whatsapps/disconnect", "alias=online&typed=online")
        .await;
    assert_eq!(answer.status, 200, "{}", answer.text());
    assert!(answer.text().contains("but WAHA did not answer"));
    assert!(answer.text().contains("arauto-hub admin engine health"));
    // The change in the hub holds.
    assert_eq!(rig.world.state("online").await, NumberState::Disconnected);
    let _ = online;
}

#[tokio::test(flavor = "multi_thread")]
async fn reading_the_number_windows_changes_nothing() {
    let mut rig = rig().await;
    rig.add("vendas", &phone(102)).await;
    rig.add("filial", &phone(103)).await;
    let other = rig.other_session().await;
    let mut before = snapshot(&rig).await;
    // With no run live: every window, with every query.
    let reads = [
        "/whatsapps",
        "/whatsapps?alias=vendas",
        "/whatsapps/add",
        "/whatsapps/pair?alias=vendas",
        "/whatsapps/pair?alias=nobody",
        "/whatsapps/pair/wait?alias=vendas",
        "/whatsapps/pair/wait?alias=vendas&run=not-a-run&shown=2",
        "/whatsapps/decide?alias=vendas",
        "/whatsapps/disconnect?alias=vendas",
        "/whatsapps/remove?alias=vendas",
        "/whatsapps/native",
    ];
    for cookie in [rig.cookie.clone(), other.clone()] {
        for path in reads {
            let answer = rig.browser.get(path, Some(&cookie)).await;
            assert!(matches!(answer.status, 200 | 303), "{path}");
            assert!(
                answer
                    .header("cache-control")
                    .is_some_and(|value| value.contains("no-store")),
                "{path}"
            );
        }
    }
    assert_eq!(snapshot(&rig).await, before);
    // With a run live: the reads add no request that changes the engine, and the deadline of
    // the pairing does not move.
    let run = rig.start("filial", "qr").await;
    before = snapshot(&rig).await;
    let until = |alias: &'static str| {
        let pool = rig.world.fixture.pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<i64>>(
                "SELECT pairing_until FROM number WHERE alias = $1",
            )
            .bind(alias)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let deadline = until("filial").await;
    let wait = Rig::wait_path("filial", &run);
    for cookie in [rig.cookie.clone(), other.clone()] {
        for path in [
            wait.as_str(),
            "/whatsapps/pair/wait?alias=filial",
            "/whatsapps/pair?alias=filial",
            "/whatsapps",
        ] {
            for _ in 0..3 {
                rig.browser.get(path, Some(&cookie)).await;
            }
        }
    }
    let after = snapshot(&rig).await;
    assert_eq!(after.0, before.0, "the rows did not change");
    assert_eq!(after.1, before.1, "the audit did not change");
    assert_eq!(
        after.2, before.2,
        "no request to change the engine was sent by a read"
    );
    assert_eq!(
        until("filial").await,
        deadline,
        "a read never moves the deadline"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn every_window_of_the_numbers_stands_over_the_list_and_stops_its_refresh() {
    let rig = rig().await;
    rig.add("vendas", &phone(104)).await;
    for path in [
        "/whatsapps/add",
        "/whatsapps/pair?alias=vendas",
        "/whatsapps/disconnect?alias=vendas",
        "/whatsapps/remove?alias=vendas",
        "/whatsapps/native",
    ] {
        let page = rig.get(path).await.text();
        assert!(
            page.contains("<dialog open class=\"modal guide\""),
            "{path}"
        );
        // The list under it does not refresh by itself: its script stops at a page with a window.
        assert!(page.contains("<div class=\"page\" inert>"), "{path}");
    }
    let list = rig.get("/whatsapps").await.text();
    assert!(!list.contains("<dialog"));
    assert!(list.contains("id=\"live-main\" data-live"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_panel_says_what_to_do_about_a_refused_key_by_where_the_engine_came_from() {
    let rig = rig().await;
    let pool = &rig.world.fixture.pool;
    rig.world.fake.set_api_key("another-key-entirely-0001");
    arauto_hub::engine::supervise(rig.world.hub()).await;
    // An engine an admin set up: its key is typed again.
    let page = rig.get("/whatsapps").await.text();
    assert!(page.contains("Set its key again with"), "{page}");
    assert!(page.contains("arauto-hub admin engine set-key"), "{page}");
    assert!(
        !page.contains("it reads its key only when it starts"),
        "{page}"
    );
    // The engine of the installation: its key is drawn, and a restart is what helps.
    sqlx::query("DELETE FROM alert")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE engine SET origin = 'installation', key_refused_since = NULL")
        .execute(pool)
        .await
        .unwrap();
    arauto_hub::engine::supervise(rig.world.hub()).await;
    let page = rig.get("/whatsapps").await.text();
    assert!(
        page.contains("Restart the engine: it reads its key only when it starts."),
        "{page}"
    );
    assert!(!page.contains("Set its key again with"), "{page}");
}
