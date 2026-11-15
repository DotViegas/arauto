//! The org chart in the panel: who reads it, who publishes it, what a publish writes, what a
//! stale or a broken draft does, and that nothing the admin wrote is ever markup.

use arauto_testkit::http;
use arauto_testkit::hub::fixture::Fixture;
use arauto_testkit::hub::support::PANEL_HOST;
use serde_json::{Value, json};

use crate::session::everything;
use crate::{Browser, PATIENCE, audited, form_value, hub};

const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let start = text
        .find(from)
        .unwrap_or_else(|| panic!("no {from} in {text}"))
        + from.len();
    let rest = &text[start..];
    &rest[..rest.find(to).unwrap()]
}

/// The version the page was written at.
fn version_of(page: &str) -> String {
    between(page, "id=\"org-data\" data-version=\"", "\"").to_owned()
}

fn body(version: &str, draft: &Value) -> String {
    format!("version={version}&draft={}", form_value(&draft.to_string()))
}

async fn page(browser: &Browser, cookie: &str, path: &str) -> String {
    let answer = browser.get(path, Some(cookie)).await;
    assert_eq!(answer.status, 200, "{path}: {}", answer.text());
    answer.text()
}

/// Two Hermes, and the draft that puts them in two levels with a link and a role.
async fn two(fixture: &Fixture) -> (String, String) {
    (fixture.hermes().await, fixture.hermes().await)
}

fn draft(a: &str, b: &str, role: &str) -> Value {
    json!({
        "levels": [ { "key": "new-1", "name": "board" }, { "key": "new-2", "name": "floor" } ],
        "hermes": [
            { "name": a, "level": "new-1", "x": 300, "role": role },
            { "name": b, "level": "new-2", "x": 20, "role": "" },
        ],
        "links": [ { "from": a, "to": b, "wake": false } ],
    })
}

async fn count(fixture: &Fixture, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(&fixture.pool)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_org_chart_needs_a_full_session() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let nobody = browser.get("/org", None).await;
    assert_eq!(nobody.status, 303);
    assert_eq!(nobody.header("location"), Some("/sign-in"));
    assert!(nobody.body.is_empty());
    let stranger = browser.get("/org", Some("arauto-panel=nothing")).await;
    assert_eq!(stranger.status, 303);

    // A session opened by a code only reaches the registration of a passkey.
    let by_code = browser.entered(&fixture).await;
    let answer = browser.get("/org", Some(&by_code)).await;
    assert_eq!(answer.status, 303);
    assert_eq!(answer.header("location"), Some("/first-access"));
    let post = browser
        .post("/org/publish", Some(&by_code), "version=x&draft=x")
        .await;
    assert_eq!(post.status, 303);
    assert_eq!(post.header("location"), Some("/sign-in"));
    let post = browser
        .post("/org/publish", None, "version=x&draft=x")
        .await;
    assert_eq!(post.header("location"), Some("/sign-in"));

    let cookie = browser.signed_in(&fixture).await;
    assert_eq!(browser.get("/org", Some(&cookie)).await.status, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_needs_the_origin_of_the_panel() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (a, b) = two(&fixture).await;
    let version = version_of(&page(&browser, &cookie, "/org").await);
    let form = body(&version, &draft(&a, &b, ""));
    let before = everything(&fixture).await;

    for origin in [None, Some("http://evil.test"), Some("null")] {
        let mut headers = vec![
            ("Host", PANEL_HOST),
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Cookie", cookie.as_str()),
        ];
        if let Some(origin) = origin {
            headers.push(("Origin", origin));
        }
        let answer = http::post(
            browser.panel,
            "/org/publish",
            &headers,
            form.as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 403, "{origin:?}");
    }
    // A GET of the address that publishes is nothing.
    assert_eq!(browser.get("/org/publish", Some(&cookie)).await.status, 404);
    assert_eq!(browser.get("/org/other", Some(&cookie)).await.status, 404);
    assert_eq!(
        everything(&fixture).await,
        before,
        "a refused request wrote something"
    );

    // The same body from a page of the panel publishes: the detector sees a write.
    let answer = browser.post("/org/publish", Some(&cookie), &form).await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    assert_ne!(everything(&fixture).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn reading_the_org_chart_writes_nothing() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    two(&fixture).await;
    let before = everything(&fixture).await;
    for path in ["/org", "/org?published", "/org?published&x=1"] {
        assert_eq!(browser.get(path, Some(&cookie)).await.status, 200);
        let headers = [
            ("Host", PANEL_HOST),
            ("X-Arauto-Refresh", "1"),
            ("Cookie", cookie.as_str()),
        ];
        let answer = http::get(browser.panel, path, &headers, PATIENCE)
            .await
            .unwrap();
        assert_eq!(answer.status, 200);
    }
    assert_eq!(everything(&fixture).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_publish_from_the_panel_is_what_the_command_would_write() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (a, b) = two(&fixture).await;
    let first = page(&browser, &cookie, "/org").await;
    let version = version_of(&first);
    // Nothing is placed: the page says it has no level and shows both as not placed yet.
    assert!(first.contains("not placed yet"));

    let answer = browser
        .post(
            "/org/publish",
            Some(&cookie),
            &body(&version, &draft(&a, &b, "leads")),
        )
        .await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    assert_eq!(answer.header("location"), Some("/org?published"));

    assert_eq!(count(&fixture, "org_level").await, 2);
    assert_eq!(count(&fixture, "org_member").await, 2);
    assert_eq!(count(&fixture, "peer_link").await, 1);
    // The audit names the panel, for the chart and for the link the same way.
    for action in [
        "org_chart.published",
        "org_level.changed",
        "org_member.changed",
        "peer_link.created",
    ] {
        let notes: Vec<Option<String>> =
            sqlx::query_scalar("SELECT actor_note FROM audit WHERE action = $1")
                .bind(action)
                .fetch_all(&fixture.pool)
                .await
                .unwrap();
        assert!(!notes.is_empty(), "{action}");
        assert!(
            notes.iter().all(|note| note.as_deref() == Some("panel")),
            "{action}"
        );
    }
    assert_eq!(audited(&fixture, "org_chart.published").await, 1);

    let next = page(&browser, &cookie, "/org?published").await;
    assert!(next.contains("<p class=\"status\" role=\"status\">Published.</p>"));
    assert_ne!(version_of(&next), version);
    // The same draft from the new version: nothing to do, back to the page.
    let again = browser
        .post("/org/publish", Some(&cookie), &body(&version_of(&next), &json!({
            "levels": [ { "key": between(&next, "&quot;key&quot;:&quot;", "&quot;"), "name": "board" } ],
            "hermes": [],
            "links": [],
        })))
        .await;
    // That draft leaves two Hermes out: it is refused whole, and nothing changed.
    assert_eq!(again.status, 422, "{}", again.text());
    assert_eq!(count(&fixture, "org_level").await, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_publish_keeps_the_draft_and_writes_nothing() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (a, b) = two(&fixture).await;
    let version = version_of(&page(&browser, &cookie, "/org").await);
    // The command changes a link between the read and the publish.
    fixture
        .admin
        .link_add(&b, &a, false, false, fixture.now())
        .await
        .unwrap();
    let before = everything(&fixture).await;
    let answer = browser
        .post(
            "/org/publish",
            Some(&cookie),
            &body(&version, &draft(&a, &b, HOSTILE)),
        )
        .await;
    assert_eq!(answer.status, 409, "{}", answer.text());
    let text = answer.text();
    assert!(text.contains("Someone changed the org chart while you were editing"));
    assert!(text.contains("Nothing was published."));
    // The page is the fresh one, and the draft comes back, escaped.
    assert_ne!(version_of(&text), version);
    let echoed = between(&text, "id=\"org-draft\" hidden data-draft=\"", "\"");
    assert!(
        echoed.contains("&quot;role&quot;:&quot;&lt;script&gt;"),
        "{echoed}"
    );
    assert!(
        !text
            .replace("<script src=\"/assets/panel.js\"></script>", "")
            .replace("<script src=\"/assets/org.js\"></script>", "")
            .contains("<script")
    );
    assert_eq!(everything(&fixture).await, before);
    assert_eq!(count(&fixture, "org_member").await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invalid_or_oversized_draft_is_refused_with_its_sentence() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (a, b) = two(&fixture).await;
    let version = version_of(&page(&browser, &cookie, "/org").await);
    let before = everything(&fixture).await;

    let mut cases: Vec<(String, u16, &str)> = vec![
        (
            "version=zz&draft={}".to_owned(),
            400,
            "The draft could not be read.",
        ),
        (
            format!("version={version}&draft=nonsense"),
            400,
            "The draft could not be read.",
        ),
        (
            format!("version={version}"),
            400,
            "The draft could not be read.",
        ),
    ];
    let mut bad_role = draft(&a, &b, "");
    bad_role["hermes"][0]["role"] = "two\nlines".into();
    cases.push((
        body(&version, &bad_role),
        422,
        "A role has at most 280 characters, in one line",
    ));
    let mut bad_name = draft(&a, &b, "");
    bad_name["levels"][0]["name"] = "Bad Name".into();
    cases.push((
        body(&version, &bad_name),
        422,
        "A level name has 1 to 32 lowercase letters",
    ));
    let mut twice = draft(&a, &b, "");
    twice["levels"][1]["name"] = "board".into();
    cases.push((
        body(&version, &twice),
        422,
        "Two levels have the same name.",
    ));
    let mut place = draft(&a, &b, "");
    place["hermes"][0]["x"] = 5000.into();
    cases.push((
        body(&version, &place),
        422,
        "A place on the map is out of range.",
    ));
    let mut unknown = draft(&a, &b, "");
    unknown["hermes"][0]["level"] = "new-9".into();
    cases.push((
        body(&version, &unknown),
        422,
        "names a Hermes or a level that does not exist",
    ));
    let mut link = draft(&a, &b, "");
    link["links"][0]["to"] = a.clone().into();
    cases.push((
        body(&version, &link),
        422,
        "A link joins two different Hermes",
    ));
    let mut levels = draft(&a, &b, "");
    levels["levels"] = json!([]);
    cases.push((
        body(&version, &levels),
        422,
        "The org chart has from 1 to 12 levels.",
    ));
    for (form, status, said) in cases {
        let answer = browser.post("/org/publish", Some(&cookie), &form).await;
        assert_eq!(answer.status, status, "{said}: {}", answer.text());
        let text = answer.text();
        assert!(text.contains(said), "{said}: {text}");
        assert!(text.contains("Nothing was published."), "{said}");
    }
    // Too large: a megabyte and a byte. The hub answers 413 from the declared length, before it
    // reads the body: a client that is still writing may see the connection closed instead of the
    // page (not guaranteed here), and either way nothing was published.
    let huge = format!("version={version}&draft={}", "a".repeat(1_048_576 + 1));
    let headers = [
        ("Host", PANEL_HOST),
        ("Origin", arauto_testkit::hub::support::PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
        ("Cookie", cookie.as_str()),
    ];
    match http::post(
        browser.panel,
        "/org/publish",
        &headers,
        huge.as_bytes(),
        PATIENCE,
    )
    .await
    {
        Ok(answer) => {
            assert_eq!(answer.status, 413);
            assert!(answer.text().contains("The draft is too large to publish."));
        }
        Err(error) => {
            eprintln!("the hub closed the connection before the body was read: {error:?}")
        }
    }
    assert_eq!(everything(&fixture).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hostile_role_is_escaped_on_every_page() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (a, b) = two(&fixture).await;
    let version = version_of(&page(&browser, &cookie, "/org").await);
    // A role that breaks the rule is refused; the hostile one of a valid shape (no control
    // character) is published.
    let answer = browser
        .post(
            "/org/publish",
            Some(&cookie),
            &body(&version, &draft(&a, &b, HOSTILE)),
        )
        .await;
    assert_eq!(answer.status, 303, "{}", answer.text());

    let text = page(&browser, &cookie, "/org").await;
    let plain = text
        .replace("<script src=\"/assets/panel.js\"></script>", "")
        .replace("<script src=\"/assets/org.js\"></script>", "");
    assert!(!plain.contains("<script"), "{plain}");
    assert!(!plain.contains("<img"), "{plain}");
    assert!(plain.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!text.contains(" style=\""));
    // In the attribute that carries the state, the five characters are entities.
    let state = between(&text, "data-state=\"", "\"");
    assert!(!state.contains('<') && !state.contains('>') && !state.contains('\''));
    // The other pages do not hold the role at all.
    for path in ["/", "/whatsapps", "/accesses"] {
        let other = page(&browser, &cookie, path).await;
        assert!(!other.contains("alert(1)"), "{path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_org_tab_is_listed_and_its_page_loads_its_script() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    two(&fixture).await;
    for path in ["/", "/whatsapps", "/accesses", "/org"] {
        let text = page(&browser, &cookie, path).await;
        let tabs = between(&text, "<nav class=\"tabs\"", "</nav>");
        assert_eq!(tabs.matches("<a href=").count(), 4, "{path}");
        assert_eq!(tabs.matches("aria-current=\"page\"").count(), 1, "{path}");
        if path == "/org" {
            assert!(tabs.contains("<a href=\"/org\" aria-current=\"page\">Org chart</a>"));
            assert_eq!(text.matches("<script").count(), 2);
            assert!(text.contains("<script src=\"/assets/org.js\"></script>"));
            assert!(!text.contains("id=\"live-main\""));
            assert_eq!(
                text.matches("<div id=\"org-live\" data-live hidden>")
                    .count(),
                1
            );
        } else {
            assert!(!text.contains("org.js"), "{path}");
            assert!(text.contains("id=\"live-main\""), "{path}");
        }
    }
    let script = browser.get("/assets/org.js", None).await;
    assert_eq!(script.status, 200);
    assert_eq!(
        script.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert!(script.text().contains("data-hold"));
}
