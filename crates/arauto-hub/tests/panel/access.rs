//! The editor of an access and the revocation of a Hermes: what the forms save is what the
//! admin command saves, the rules are the ones of the command, and reading saves nothing.

use arauto_testkit::http;
use arauto_testkit::hub::support::PANEL_HOST;

use crate::session::everything;
use crate::world::{World, phone};
use crate::{Browser, PATIENCE, hub, rows};

async fn page(browser: &Browser, cookie: &str, path: &str) -> String {
    let answer = browser.get(path, Some(cookie)).await;
    assert_eq!(answer.status, 200, "{path}: {}", answer.text());
    answer.text()
}

/// The accesses of the database, as `(scope, read, send, modify, wake, start, wait)`.
async fn accesses(world: &World) -> Vec<(String, bool, bool, bool, bool, bool, i32)> {
    sqlx::query_as(
        "SELECT scope, can_read, can_send, can_modify, can_wake, can_start_chats, wake_wait_ms \
         FROM access WHERE organization_id = $1 ORDER BY changed_at",
    )
    .bind(world.organization().id())
    .fetch_all(world.hub().pool())
    .await
    .unwrap()
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn the_editor_saves_an_access_by_the_functions_of_the_command() {
    let world = World::new().await;
    let fixture = &world.fixture;
    let browser = Browser::of(fixture);
    let cookie = browser.signed_in(fixture).await;
    world.add("balcao", &phone(1)).await;
    let ana = fixture.hermes().await;
    let edit = format!("/accesses/edit?hermes={ana}&number=balcao");
    let saved_at = format!("/accesses?hermes={ana}&number=balcao");
    let form = |rest: &str| format!("hermes={ana}&number=balcao&{rest}");

    // The matrix leads to the editor, and the editor starts from what is saved: nothing.
    let matrix = page(&browser, &cookie, "/accesses").await;
    assert!(matrix.contains(&format!(
        "<a class=\"cell\" href=\"/accesses/edit?hermes={ana}&amp;number=balcao\">"
    )));
    let editor = page(&browser, &cookie, &edit).await;
    assert!(editor.contains(&format!(
        "<h2 id=\"editor-title\">{ana} on the number balcao</h2>"
    )));
    assert!(editor.contains("<div class=\"page\" inert>"));
    assert!(editor.contains("name=\"scope\" value=\"all\" checked>"));
    assert!(
        !editor.contains("value=\"on\" checked>"),
        "no verb is marked"
    );
    assert!(editor.contains(&format!(
        "The Hermes {ana} has no access to the number balcao."
    )));
    assert!(
        accesses(&world).await.is_empty(),
        "opening the editor saves nothing"
    );

    // Saving: the grant and the wait, and the page says what the access is now.
    let saved = browser
        .post(
            "/accesses/save",
            Some(&cookie),
            &form("scope=all&read=on&send=on&wake=on&wait=9"),
        )
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    assert_eq!(saved.header("location"), Some(saved_at.as_str()));
    assert_eq!(
        accesses(&world).await,
        [("all".to_owned(), true, true, false, true, false, 9_000)]
    );
    let matrix = page(&browser, &cookie, &saved_at).await;
    assert!(matrix.contains(&format!(
        "<p class=\"status\" role=\"status\">Saved: The Hermes {ana} may read and send in every chat of the number balcao. It does not modify and is woken after 9 s of silence in the chat.</p>"
    )), "{matrix}");
    assert!(
        matrix.contains("<span>R</span><span>S</span><span class=\"off\">M</span><span>W</span>")
    );
    // The audit has what the command would have written, and names the panel.
    let audit = rows(fixture, "audit").await.join("\n");
    assert!(
        audit.contains("access.granted") && audit.contains("\"actor_note\":\"panel\""),
        "{audit}"
    );

    // The editor shows what was saved.
    let editor = page(&browser, &cookie, &edit).await;
    for name in ["read", "send", "wake"] {
        assert!(
            editor.contains(&format!("name=\"{name}\" value=\"on\" checked>")),
            "{name}"
        );
    }
    assert!(!editor.contains("name=\"modify\" value=\"on\" checked>"));
    assert!(editor.contains("name=\"wait\" type=\"number\" min=\"0\" max=\"60\" value=\"9\""));

    // Changing it: every chat, with the option of starting chats; the wait stays when the
    // form does not send it (a field the screen greyed out).
    let saved = browser
        .post(
            "/accesses/save",
            Some(&cookie),
            &form("scope=all&read=on&send=on&modify=on&start=on"),
        )
        .await;
    assert_eq!(saved.status, 303);
    assert_eq!(
        accesses(&world).await,
        [("all".to_owned(), true, true, true, false, true, 9_000)]
    );
    assert!(page(&browser, &cookie, &saved_at).await.contains(
        "It is not woken. It may start a chat with someone who never talked to the number."
    ));

    // Only chosen chats: the lines of the chats are of the command, and the page says so.
    let saved = browser
        .post("/accesses/save", Some(&cookie), &form("scope=list&read=on"))
        .await;
    assert_eq!(saved.status, 303);
    assert_eq!(accesses(&world).await[0].0, "list");
    let matrix = page(&browser, &cookie, &saved_at).await;
    assert!(
        matrix.contains(&format!(
            "Saved: The Hermes {ana} only reaches the chosen chats of the number balcao: 0 today."
        )),
        "{matrix}"
    );
    assert!(matrix.contains("no chat"));

    // No verb on every chat: the access goes.
    let saved = browser
        .post("/accesses/save", Some(&cookie), &form("scope=all"))
        .await;
    assert_eq!(saved.status, 303);
    assert!(accesses(&world).await.is_empty());
    let matrix = page(&browser, &cookie, &saved_at).await;
    assert!(matrix.contains(&format!(
        "Saved: The Hermes {ana} has no access to the number balcao."
    )));
    assert!(
        rows(fixture, "audit")
            .await
            .join("\n")
            .contains("access.removed")
    );
    // Taking away what is not there is no error.
    let again = browser
        .post("/accesses/save", Some(&cookie), &form("scope=all"))
        .await;
    assert_eq!(again.status, 303);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_rules_of_an_access_are_the_ones_of_the_command() {
    let world = World::new().await;
    let fixture = &world.fixture;
    let browser = Browser::of(fixture);
    let cookie = browser.signed_in(fixture).await;
    world.add("balcao", &phone(2)).await;
    let ana = fixture.hermes().await;
    let before = everything(fixture).await;

    for (rest, said) in [
        (
            "scope=all&wake=on",
            "Being woken requires reading. Nothing was saved.",
        ),
        (
            "scope=all&send=on&wake=on&wait=5",
            "Being woken requires reading. Nothing was saved.",
        ),
        (
            "scope=all&read=on&start=on",
            "Starting chats requires sending",
        ),
        (
            "scope=list&read=on&send=on&start=on",
            "Starting chats requires sending",
        ),
        ("scope=all&start=on", "Starting chats requires sending"),
        (
            "scope=all&read=on&wake=on&wait=61",
            "The wait before waking goes from 0 to 60 seconds.",
        ),
        (
            "scope=all&read=on&wait=abc",
            "The wait before waking goes from 0 to 60 seconds.",
        ),
        (
            "scope=all&read=on&wait=-1",
            "The wait before waking goes from 0 to 60 seconds.",
        ),
    ] {
        let body = format!("hermes={ana}&number=balcao&{rest}");
        let answer = browser.post("/accesses/save", Some(&cookie), &body).await;
        assert_eq!(answer.status, 200, "{rest}");
        let text = answer.text();
        assert!(text.contains(said), "{rest}: {text}");
        // The form comes back as it was sent, for the admin to fix it.
        assert!(text.contains("<form id=\"editor-form\""), "{rest}");
    }
    // A Hermes or a number that does not exist: nothing to edit and nothing to save.
    for body in [
        "hermes=ninguem&number=balcao&scope=all&read=on".to_owned(),
        format!("hermes={ana}&number=nenhum&scope=all&read=on"),
        format!("hermes={ana}&scope=all&read=on"),
        "number=balcao&scope=all&read=on".to_owned(),
        "hermes=%3Cb%3E&number=balcao&scope=all&read=on".to_owned(),
    ] {
        let answer = browser.post("/accesses/save", Some(&cookie), &body).await;
        assert_eq!(answer.status, 303, "{body}");
        assert_eq!(answer.header("location"), Some("/accesses"), "{body}");
    }
    assert_eq!(
        before,
        everything(fixture).await,
        "a refused form saved something"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_editor_needs_a_session_and_the_origin_and_reading_saves_nothing() {
    let world = World::new().await;
    let fixture = &world.fixture;
    let browser = Browser::of(fixture);
    let cookie = browser.signed_in(fixture).await;
    world.add("balcao", &phone(3)).await;
    let ana = fixture.hermes().await;
    let before = everything(fixture).await;
    let body = format!("hermes={ana}&number=balcao&scope=all&read=on&send=on");

    let no_session = browser.post("/accesses/save", None, &body).await;
    assert_eq!(no_session.status, 303);
    assert_eq!(no_session.header("location"), Some("/sign-in"));
    for origin in [None, Some("http://evil.test")] {
        let mut headers = vec![
            ("Host", PANEL_HOST),
            ("Cookie", cookie.as_str()),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ];
        if let Some(origin) = origin {
            headers.push(("Origin", origin));
        }
        let answer = http::post(
            browser.panel,
            "/accesses/save",
            &headers,
            body.as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 403, "{origin:?}");
    }
    // An address that is opened, with every field a form would send, saves nothing.
    assert_eq!(
        browser
            .get(&format!("/accesses/save?{body}"), Some(&cookie))
            .await
            .status,
        404
    );
    let opened = browser
        .get(&format!("/accesses/edit?{body}"), Some(&cookie))
        .await;
    assert_eq!(opened.status, 200);
    assert!(
        !opened.text().contains("value=\"on\" checked>"),
        "the address marks nothing"
    );
    assert_eq!(
        browser
            .get(&format!("/accesses/edit?{body}"), None)
            .await
            .status,
        303
    );
    for path in [
        "/accesses/edit".to_owned(),
        "/accesses/edit?hermes=ninguem&number=balcao".to_owned(),
        format!("/accesses/edit?hermes={ana}&number=nenhum"),
        format!("/accesses/edit?hermes={ana}"),
    ] {
        let answer = browser.get(&path, Some(&cookie)).await;
        assert_eq!(answer.status, 303, "{path}");
        assert_eq!(answer.header("location"), Some("/accesses"));
    }
    // The address that says what was saved only reads, and says nothing of what is not there.
    let says = page(
        &browser,
        &cookie,
        &format!("/accesses?hermes={ana}&number=balcao"),
    )
    .await;
    assert!(says.contains(&format!(
        "Saved: The Hermes {ana} has no access to the number balcao."
    )));
    for path in [
        "/accesses?hermes=ninguem&number=balcao",
        "/accesses?hermes=%3Cb%3E&number=x",
        "/accesses?number=balcao",
    ] {
        assert!(
            !page(&browser, &cookie, path)
                .await
                .contains("class=\"status\""),
            "{path}"
        );
    }
    assert_eq!(before, everything(fixture).await, "reading saved something");

    // The detector sees a write: the same form, with the session and the origin, saves.
    assert_eq!(
        browser
            .post("/accesses/save", Some(&cookie), &body)
            .await
            .status,
        303
    );
    assert_ne!(before, everything(fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn revoking_asks_first_and_only_the_form_revokes() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (name, mut session) = fixture.connected().await;
    let device = session.device().clone();
    let alone = fixture.hermes().await;
    let asks = format!("/hermes/revoke?name={name}");

    // The line of a Hermes with a device has the button; one with nothing to close has not.
    let list = page(&browser, &cookie, "/").await;
    assert!(list.contains(&format!(
        "<a class=\"button danger\" href=\"/hermes/revoke?name={name}\">Revoke</a>"
    )));
    assert!(!list.contains(&format!("/hermes/revoke?name={alone}")));

    // Asking revokes nothing, however many times.
    for _ in 0..3 {
        let question = page(&browser, &cookie, &asks).await;
        assert!(question.contains(&format!("<h2 id=\"revoke-title\">Revoke {name}</h2>")));
        assert!(question.contains("The accesses are kept"));
        assert!(question.contains("<div class=\"page\" inert>"));
    }
    assert_eq!(fixture.state_of(&device).await.0, "active");
    assert_eq!(browser.get(&asks, None).await.status, 303);
    for path in [
        "/hermes/revoke",
        "/hermes/revoke?name=ninguem",
        "/hermes/revoke?name=%3Cb%3E",
    ] {
        let answer = browser.get(path, Some(&cookie)).await;
        assert_eq!(answer.status, 303, "{path}");
        assert_eq!(answer.header("location"), Some("/"));
    }
    // A form without the session or without the origin of the panel revokes nothing.
    let body = format!("name={name}");
    assert_eq!(
        browser
            .post("/hermes/revoke", None, &body)
            .await
            .header("location"),
        Some("/sign-in")
    );
    let headers = [
        ("Host", PANEL_HOST),
        ("Cookie", cookie.as_str()),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ];
    let answer = http::post(
        browser.panel,
        "/hermes/revoke",
        &headers,
        body.as_bytes(),
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 403);
    assert_eq!(fixture.state_of(&device).await.0, "active");

    // The form: revoked, the channel falls, and the list says so.
    let revoked = browser.post("/hermes/revoke", Some(&cookie), &body).await;
    assert_eq!(revoked.status, 303, "{}", revoked.text());
    assert_eq!(revoked.header("location"), Some("/"));
    assert_eq!(fixture.state_of(&device).await.0, "revoked");
    assert!(
        session.ended(PATIENCE).await.is_ok(),
        "the session of the Hermes ended"
    );
    let list = page(&browser, &cookie, "/").await;
    assert!(list.contains("<span class=\"state idle\">No device</span>"));
    assert!(!list.contains(&format!("/hermes/revoke?name={name}")));
    let audit = rows(&fixture, "audit").await.join("\n");
    assert!(audit.contains("\"actor_note\":\"panel\""));
    // Revoking again, or someone who does not exist, changes nothing.
    for body in [body.as_str(), "name=ninguem", "other=1"] {
        assert_eq!(
            browser
                .post("/hermes/revoke", Some(&cookie), body)
                .await
                .status,
            303
        );
    }
}
