//! The session of the panel: what needs one, its two deadlines, how it ends, and what a
//! request that reads may not do.

use std::sync::Arc;

use arauto_hub::repo::panel::{SESSION_IDLE_MS, SESSION_VALIDITY_MS};
use arauto_protocol::clock::ManualClock;
use arauto_protocol::limits::MINUTE_MS;
use arauto_testkit::http;
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};

use crate::{Browser, COOKIE, PATIENCE, REFUSED, audited, code, cookie_of, hub, rows};

const PAGES: [&str; 3] = ["/", "/whatsapps", "/accesses"];

#[tokio::test(flavor = "multi_thread")]
async fn a_page_is_only_shown_to_a_session() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let identifier = cookie.split_once('=').unwrap().1;

    // One character of the identifier changed: another identifier, of no session.
    let mut other = identifier.to_owned();
    let last = if other.ends_with('A') { "B" } else { "A" };
    other.replace_range(other.len() - 1.., last);
    let strangers = [
        None,
        Some(format!("{COOKIE}=")),
        Some(format!("{COOKIE}=not-an-identifier")),
        Some(format!("{COOKIE}={other}")),
        Some(format!("{COOKIE}={}", "A".repeat(43))),
        // The right value under another name is not the cookie of the session.
        Some(format!("session={identifier}")),
        Some(format!("__Host-{COOKIE}={identifier}")),
    ];
    for path in PAGES {
        for stranger in &strangers {
            let answer = browser.get(path, stranger.as_deref()).await;
            assert_eq!(answer.status, 303, "{path} {stranger:?}");
            assert_eq!(answer.header("location"), Some("/sign-in"));
            assert!(
                answer.body.is_empty(),
                "nothing of the page goes with the redirect"
            );
        }
        let answer = browser.get(path, Some(&cookie)).await;
        assert_eq!(answer.status, 200, "{path}");
    }
    // Among other cookies of the browser, the one of the session is still found.
    let among = format!("theme=dark; {cookie}; other=1");
    assert_eq!(browser.get("/", Some(&among)).await.status, 200);

    // The sheet and the fonts are of the page of the entry too: they need no session, and
    // only the files of the list exist.
    let sheet = browser.get("/assets/panel.css", None).await;
    assert_eq!(sheet.status, 200);
    assert_eq!(
        sheet.header("content-type"),
        Some("text/css; charset=utf-8")
    );
    let font = browser.get("/assets/poppins-600.woff2", None).await;
    assert_eq!(font.status, 200);
    assert!(font.body.starts_with(b"wOF2"));
    for path in [
        "/assets/",
        "/assets/nothing.css",
        "/assets/../panel.css",
        "/assets/..%2F..%2FCargo.toml",
        "/assets/fonts/poppins-600.woff2",
        "/assets/panel.css/",
        "/nothing",
        "/sign-out",
    ] {
        assert_eq!(browser.get(path, Some(&cookie)).await.status, 404, "{path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn every_answer_tells_the_browser_to_do_no_more_than_show_it() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    let mut answers = vec![
        browser.get("/sign-in", None).await,
        browser.get("/", None).await,
        browser.get("/nothing", None).await,
        browser.get("/assets/panel.css", None).await,
        browser.enter("not-a-code").await,
        http::get(browser.panel, "/", &[("Host", "evil.test")], PATIENCE)
            .await
            .unwrap(),
        http::post(
            browser.panel,
            "/sign-in",
            &[("Host", PANEL_HOST)],
            b"",
            PATIENCE,
        )
        .await
        .unwrap(),
    ];
    for path in PAGES {
        answers.push(browser.get(path, Some(&cookie)).await);
    }
    for answer in &answers {
        let policy = answer.header("content-security-policy").unwrap();
        assert!(policy.starts_with("default-src 'none'"), "{policy}");
        for part in [
            "form-action 'self'",
            "frame-ancestors 'none'",
            "base-uri 'none'",
        ] {
            assert!(policy.contains(part), "{part}: {policy}");
        }
        assert!(!policy.contains("unsafe"), "{policy}");
        // The one script that runs is a file of the panel: none written in a page.
        assert!(policy.contains("script-src 'self';"), "{policy}");
        assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
        assert_eq!(answer.header("x-frame-options"), Some("DENY"));
        assert_eq!(answer.header("referrer-policy"), Some("no-referrer"));
        let cache = answer.header("cache-control").unwrap();
        let is_file = answer.header("content-type") == Some("text/css; charset=utf-8");
        assert_eq!(
            cache == "no-store",
            !is_file,
            "only a file of the panel may be kept"
        );
        assert!(
            !answer.text().contains("<script"),
            "a page without a guide loads no script"
        );
    }
    assert_eq!(
        answers
            .iter()
            .map(|answer| answer.status)
            .collect::<Vec<_>>(),
        [200, 303, 404, 200, 200, 421, 403, 200, 200, 200]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_ends_when_it_sits_idle_and_when_its_hours_pass() {
    let clock = Arc::new(ManualClock::new(1_800_000_000_000));
    let fixture = Fixture::with_clock(fixture::settings(), clock.clone()).await;
    let browser = Browser::of(&fixture);

    // Idle: each request starts the 30 minutes again; 30 minutes without one end it.
    let cookie = browser.signed_in(&fixture).await;
    for _ in 0..3 {
        clock.advance(SESSION_IDLE_MS - 1);
        assert_eq!(browser.get("/", Some(&cookie)).await.status, 200);
    }
    clock.advance(SESSION_IDLE_MS);
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 303);
    // It does not come back by being asked again.
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 303);
    assert_eq!(browser.get("/whatsapps", Some(&cookie)).await.status, 303);

    // Its hours: 12, however much it is used.
    let cookie = browser.signed_in(&fixture).await;
    let step = 20 * MINUTE_MS;
    let mut passed = 0;
    while passed + step < SESSION_VALIDITY_MS {
        clock.advance(step);
        passed += step;
        assert_eq!(
            browser.get("/", Some(&cookie)).await.status,
            200,
            "{passed}"
        );
    }
    clock.advance(SESSION_VALIDITY_MS - passed - 1);
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 200);
    clock.advance(1);
    assert_eq!(
        browser.get("/", Some(&cookie)).await.status,
        303,
        "at 12 hours it is over"
    );
    // Reading did not close its row: the next write does, and erases the digest.
    let open = "\"state\":\"open\"";
    let kept = rows(&fixture, "panel_session").await;
    assert_eq!(kept.iter().filter(|row| row.contains(open)).count(), 2);
    browser.signed_in(&fixture).await;
    let kept = rows(&fixture, "panel_session").await;
    assert_eq!(kept.len(), 3);
    let expired: Vec<&String> = kept
        .iter()
        .filter(|row| row.contains("\"close_reason\":\"expired\""))
        .collect();
    assert_eq!(expired.len(), 2, "{kept:?}");
    assert!(expired.iter().all(|row| row.contains("\"digest\":null")));
    assert_eq!(kept.iter().filter(|row| row.contains(open)).count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn signing_out_ends_that_session_and_no_other() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let first = browser.signed_in(&fixture).await;
    let second = browser.signed_in(&fixture).await;
    assert_ne!(first, second);

    // A link cannot end a session: only the form of the panel does.
    assert_eq!(browser.get("/sign-out", Some(&first)).await.status, 404);
    assert_eq!(browser.get("/", Some(&first)).await.status, 200);

    let answer = browser.post("/sign-out", Some(&first), "").await;
    assert_eq!(answer.status, 303);
    assert_eq!(answer.header("location"), Some("/sign-in"));
    let set = answer.header("set-cookie").unwrap();
    assert!(set.starts_with(&format!("{COOKIE}=;")), "{set}");
    assert!(set.contains("Max-Age=0"), "{set}");

    assert_eq!(browser.get("/", Some(&first)).await.status, 303);
    assert_eq!(browser.get("/", Some(&second)).await.status, 200);
    assert_eq!(audited(&fixture, "panel.signed_out").await, 1);
    let kept = rows(&fixture, "panel_session").await;
    let closed: Vec<&String> = kept
        .iter()
        .filter(|row| row.contains("\"close_reason\":\"signed_out\""))
        .collect();
    assert_eq!(closed.len(), 1, "{kept:?}");
    assert!(closed[0].contains("\"digest\":null"), "{}", closed[0]);

    // Signing out twice, or with no session, says the same and writes nothing.
    for cookie in [Some(first.as_str()), None] {
        let answer = browser.post("/sign-out", cookie, "").await;
        assert_eq!(answer.status, 303);
        assert_eq!(answer.header("location"), Some("/sign-in"));
    }
    assert_eq!(audited(&fixture, "panel.signed_out").await, 1);
    assert_eq!(browser.get("/", Some(&second)).await.status, 200);
}

/// Every row of every table of the hub, as text.
pub async fn everything(fixture: &Fixture) -> Vec<(String, Vec<String>)> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = current_schema() ORDER BY 1",
    )
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    assert!(tables.iter().any(|table| table == "panel_session"));
    assert!(tables.iter().any(|table| table == "audit"));
    let mut all = Vec::with_capacity(tables.len());
    for table in tables {
        let found = rows(fixture, &table).await;
        all.push((table, found));
    }
    all
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_reads_writes_nothing() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    fixture.hermes().await;
    let cookie = browser.signed_in(&fixture).await;
    // A code waits to be typed: reading must not spend it nor count against it.
    let waiting = code(&fixture).await;

    let before = everything(&fixture).await;
    for _ in 0..3 {
        for path in PAGES {
            assert_eq!(browser.get(path, Some(&cookie)).await.status, 200);
            assert_eq!(browser.get(path, None).await.status, 303);
        }
        assert_eq!(browser.get("/sign-in", None).await.status, 200);
        assert_eq!(browser.get("/sign-in", Some(&cookie)).await.status, 303);
        assert_eq!(browser.get("/assets/panel.css", None).await.status, 200);
        // An address with the code in it is only an address: nothing reads it.
        let with_code = format!("/sign-in?code={waiting}");
        let answer = browser.get(&with_code, None).await;
        assert_eq!(answer.status, 200);
        assert!(cookie_of(&answer).is_none());
        let answer = browser.get(&format!("/?code={waiting}"), None).await;
        assert_eq!(answer.status, 303);
        assert!(cookie_of(&answer).is_none());
    }
    let after = everything(&fixture).await;
    assert_eq!(before, after, "a request that reads changed a row");
    // The detector sees a write when one happens.
    assert_eq!(browser.enter(&waiting).await.status, 303);
    assert_ne!(after, everything(&fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hub_that_starts_again_serves_no_session_from_before() {
    let mut fixture = hub().await;
    let cookie = Browser::of(&fixture).signed_in(&fixture).await;
    // A code issued before the stop is still a code: it is in the database, not in memory.
    let waiting = code(&fixture).await;

    fixture.stop().await;
    fixture.start().await;
    let browser = Browser::of(&fixture);
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 303);
    let kept = rows(&fixture, "panel_session").await;
    assert_eq!(kept.len(), 1);
    assert!(
        kept[0].contains("\"close_reason\":\"hub_stopped\""),
        "{}",
        kept[0]
    );
    assert!(kept[0].contains("\"digest\":null"), "{}", kept[0]);

    let again = browser.enter(&waiting).await;
    assert_eq!(again.status, 303);
    let cookie = cookie_of(&again).unwrap();
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_looks_at_entry_codes_have_a_ceiling_and_the_ceiling_spends_no_code() {
    let clock = Arc::new(ManualClock::new(1_800_000_000_000));
    let fixture = Fixture::with_clock(fixture::settings(), clock.clone()).await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;

    for _ in 0..30 {
        let answer = browser
            .enter("abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef")
            .await;
        assert_eq!(answer.status, 200);
        assert!(answer.text().contains(REFUSED));
    }
    // Past the ceiling not even the right code is looked up, and the page says why.
    let held = browser.enter(&entry).await;
    assert_eq!(held.status, 429);
    assert!(held.text().contains("Too many attempts"));
    assert!(cookie_of(&held).is_none());
    assert_eq!(audited(&fixture, "panel.signed_in").await, 0);
    // A request that is refused before the form is read does not count.
    let headers = [("Host", PANEL_HOST), ("Origin", "http://evil.test")];
    let answer = http::post(browser.panel, "/sign-in", &headers, b"code=x", PATIENCE)
        .await
        .unwrap();
    assert_eq!(answer.status, 403);
    let _ = PANEL_ORIGIN;

    clock.advance(MINUTE_MS);
    assert_eq!(
        browser.enter(&entry).await.status,
        303,
        "the code was not spent"
    );
}
