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
        "<p class=\"status\" role=\"status\">Saved: The Hermes {ana} on the number balcao, in every chat: reads · sends · does not modify · is woken after 9 s of silence.</p>"
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
    assert!(
        page(&browser, &cookie, &saved_at)
            .await
            .contains("is not woken · may start chats.")
    );

    // Only chosen chats: the lines of the chats are of the command, and the page says so.
    let saved = browser
        .post("/accesses/save", Some(&cookie), &form("scope=list&read=on"))
        .await;
    assert_eq!(saved.status, 303);
    assert_eq!(accesses(&world).await[0].0, "list");
    let matrix = page(&browser, &cookie, &saved_at).await;
    assert!(
        matrix.contains(&format!(
            "Saved: The Hermes {ana} only reaches the chosen chats of the number balcao. Chats with a line: 0."
        )),
        "{matrix}"
    );
    assert!(matrix.contains("chosen chats: 0"));

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
    assert!(list.contains(&format!("href=\"/hermes/actions?name={name}\"")));
    assert!(!list.contains("/hermes/revoke?name="));
    let actions = page(&browser, &cookie, &format!("/hermes/actions?name={name}")).await;
    assert!(actions.contains(&format!("<a href=\"/hermes/revoke?name={name}\">")));
    let actions = page(&browser, &cookie, &format!("/hermes/actions?name={alone}")).await;
    assert!(!actions.contains("/hermes/revoke?name="));

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
    let actions = page(&browser, &cookie, &format!("/hermes/actions?name={name}")).await;
    assert!(!actions.contains("/hermes/revoke?name="));
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

/// Gives the access of `alias` a chat with a line of its own that wakes, and returns the chat.
async fn chat_line_that_wakes(world: &World, alias: &str, hermes: &str) -> uuid::Uuid {
    let pool = world.hub().pool();
    let (access, organization, number): (uuid::Uuid, uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT a.id, a.organization_id, a.number_id FROM access a \
         JOIN number n ON n.id = a.number_id JOIN hermes h ON h.id = a.hermes_id \
         WHERE n.alias = $1 AND h.name = $2",
    )
    .bind(alias)
    .bind(hermes)
    .fetch_one(pool)
    .await
    .unwrap();
    let chat = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO chat (id, organization_id, number_id, kind, created_at) \
         VALUES ($1, $2, $3, 'contact', 0)",
    )
    .bind(chat)
    .bind(organization)
    .bind(number)
    .execute(pool)
    .await
    .unwrap();
    set_line_wake(world, access, chat, organization, number, true).await;
    chat
}

async fn set_line_wake(
    world: &World,
    access: uuid::Uuid,
    chat: uuid::Uuid,
    organization: uuid::Uuid,
    number: uuid::Uuid,
    wake: bool,
) {
    sqlx::query(
        "INSERT INTO access_chat (organization_id, access_id, number_id, chat_id, can_read, \
         can_send, can_modify, can_wake, created_at, changed_at) \
         VALUES ($1, $2, $3, $4, TRUE, FALSE, FALSE, $5, 0, 0) \
         ON CONFLICT (access_id, chat_id) DO UPDATE SET can_wake = $5",
    )
    .bind(organization)
    .bind(access)
    .bind(number)
    .bind(chat)
    .bind(wake)
    .execute(world.hub().pool())
    .await
    .unwrap();
}

/// The `can_wake` of the line of `chat`.
async fn line_wakes(world: &World, chat: uuid::Uuid) -> bool {
    sqlx::query_scalar("SELECT can_wake FROM access_chat WHERE chat_id = $1")
        .bind(chat)
        .fetch_one(world.hub().pool())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_editor_has_one_checkbox_for_waking() {
    let world = World::new().await;
    let fixture = &world.fixture;
    let browser = Browser::of(fixture);
    let cookie = browser.signed_in(fixture).await;
    world.add("balcao", &phone(4)).await;
    let ana = fixture.hermes().await;
    let editor = page(
        &browser,
        &cookie,
        &format!("/accesses/edit?hermes={ana}&number=balcao"),
    )
    .await;
    assert!(!editor.contains("whatsapp_wake"), "{editor}");
    assert!(!editor.contains("Woken by WhatsApp"));
    assert!(editor.contains("name=\"wake\" value=\"on\""));
    assert!(editor.contains(
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule."
    ));
    let list = page(&browser, &cookie, "/").await;
    assert!(!list.contains("Woken by WhatsApp"));
}

#[tokio::test(flavor = "multi_thread")]
async fn saving_be_woken_off_turns_off_the_chat_lines_of_that_access_only() {
    let world = World::new().await;
    let fixture = &world.fixture;
    let browser = Browser::of(fixture);
    let cookie = browser.signed_in(fixture).await;
    world.add("balcao", &phone(4)).await;
    world.add("outro", &phone(5)).await;
    let ana = fixture.hermes().await;
    for alias in ["balcao", "outro"] {
        let saved = browser
            .post(
                "/accesses/save",
                Some(&cookie),
                &format!("hermes={ana}&number={alias}&scope=all&read=on&wake=on"),
            )
            .await;
        assert_eq!(saved.status, 303, "{}", saved.text());
    }
    let here = chat_line_that_wakes(&world, "balcao", &ana).await;
    let there = chat_line_that_wakes(&world, "outro", &ana).await;

    // Be woken on: the lines are left as they were.
    let saved = browser
        .post(
            "/accesses/save",
            Some(&cookie),
            &format!("hermes={ana}&number=balcao&scope=all&read=on&send=on&wake=on"),
        )
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    assert!(line_wakes(&world, here).await);
    assert!(line_wakes(&world, there).await);

    // Be woken off: the lines of this access go off, in the same save, with an event each.
    let before = rows(fixture, "audit").await.join("\n");
    let saved = browser
        .post(
            "/accesses/save",
            Some(&cookie),
            &format!("hermes={ana}&number=balcao&scope=all&read=on&send=on"),
        )
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    assert!(!line_wakes(&world, here).await);
    assert!(line_wakes(&world, there).await);
    let audit = rows(fixture, "audit").await.join("\n");
    assert_eq!(
        audit.matches("access_chat.set").count(),
        before.matches("access_chat.set").count() + 1,
        "{audit}"
    );
    let at: Vec<(bool,)> = sqlx::query_as(
        "SELECT can_wake FROM access WHERE number_id = (SELECT id FROM number WHERE alias = 'balcao')",
    )
    .fetch_all(world.hub().pool())
    .await
    .unwrap();
    assert_eq!(at, vec![(false,)]);

    // The access already unwoken and a line that wakes again (a group wakes only by its own
    // line): saving with "be woken" still off leaves the line alone.
    let (access, organization, number): (uuid::Uuid, uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT access_id, organization_id, number_id FROM access_chat WHERE chat_id = $1",
    )
    .bind(here)
    .fetch_one(world.hub().pool())
    .await
    .unwrap();
    set_line_wake(&world, access, here, organization, number, true).await;
    let saved = browser
        .post(
            "/accesses/save",
            Some(&cookie),
            &format!("hermes={ana}&number=balcao&scope=all&read=on&send=on"),
        )
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    assert!(line_wakes(&world, here).await);
    assert!(line_wakes(&world, there).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_hermes_line_has_one_actions_button_and_its_window_lists_the_actions_of_its_state() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    let (name, _session) = fixture.connected().await;
    let alone = fixture.hermes().await;

    let list = page(&browser, &cookie, "/").await;
    for each in [&name, &alone] {
        assert_eq!(
            list.matches(&format!("aria-label=\"Actions: {each}\""))
                .count(),
            1
        );
    }
    // The window is a modal over the list, with the actions of the state and the ending last.
    let with_device = page(&browser, &cookie, &format!("/hermes/actions?name={name}")).await;
    assert!(with_device.contains("<dialog open class=\"modal\" id=\"guide\""));
    assert!(with_device.contains("<div class=\"page\" inert>"));
    assert!(with_device.contains("<span class=\"lbl\">Reconnect</span>"));
    assert!(with_device.contains("<span class=\"lbl\">Who gives it orders</span>"));
    let revoke = with_device.find("/hermes/revoke?name=").unwrap();
    assert!(with_device.find("<ul class=\"acts end\">").unwrap() < revoke);
    assert!(with_device.find("/hermes/whatsapp?name=").unwrap() < revoke);
    let without = page(&browser, &cookie, &format!("/hermes/actions?name={alone}")).await;
    assert!(without.contains("<span class=\"lbl\">Connect</span>"));
    assert!(!without.contains("/hermes/revoke?name=") && !without.contains("acts end"));
    // An unknown name goes back to the list, and a read writes nothing.
    for path in ["/hermes/actions?name=ninguem", "/hermes/actions"] {
        let answer = browser.get(path, Some(&cookie)).await;
        assert_eq!((answer.status, answer.header("location")), (303, Some("/")));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn dismissing_an_alert_marks_it_as_seen_as_the_command_does_and_an_unknown_id_changes_nothing()
 {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;
    arauto_hub::repo::alert::mark_not_advanced(
        &fixture.pool,
        fixture.installation.organization,
        None,
        7,
        fixture.now(),
    )
    .await
    .unwrap();
    let list = page(&browser, &cookie, "/whatsapps").await;
    assert!(list.contains("action=\"/alerts/dismiss\""), "{list}");
    assert!(list.contains("aria-label=\"Dismiss: Audit mark\""));
    let id: String = sqlx::query_scalar("SELECT id::text FROM alert WHERE seen_at IS NULL")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    let audit_before = rows(&fixture, "audit").await;

    // An id that is not there, or not an id, changes nothing.
    let unknown = browser
        .post(
            "/alerts/dismiss",
            Some(&cookie),
            "id=00000000-0000-4000-8000-000000000000",
        )
        .await;
    assert_eq!(unknown.status, 303);
    assert_eq!(
        browser
            .post("/alerts/dismiss", Some(&cookie), "id=not-an-id")
            .await
            .status,
        400
    );
    assert!(page(&browser, &cookie, "/").await.contains("Audit mark"));

    // The form marks it as seen and goes back to the page it came from; only a list is a way back.
    let done = browser
        .post(
            "/alerts/dismiss",
            Some(&cookie),
            &format!("id={id}&return=/whatsapps"),
        )
        .await;
    assert_eq!(
        (done.status, done.header("location")),
        (303, Some("/whatsapps"))
    );
    assert!(!page(&browser, &cookie, "/").await.contains("Audit mark"));
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM alert WHERE seen_at IS NOT NULL")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(seen, 1);
    // The command writes no audit event for it, and neither does the button.
    assert_eq!(rows(&fixture, "audit").await, audit_before);
    let again = browser
        .post(
            "/alerts/dismiss",
            Some(&cookie),
            &format!("id={id}&return=https://example.com"),
        )
        .await;
    assert_eq!((again.status, again.header("location")), (303, Some("/")));
}
