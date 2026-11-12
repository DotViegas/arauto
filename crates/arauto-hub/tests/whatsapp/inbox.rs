//! The inbox of events on the internal port: the signature, the ceiling, the shape, the
//! window, the repeated event once, the order of the statuses, and its absence from the port
//! of the connectors.

use arauto_hub::core::number::NumberState;
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_testkit::engine::{post, post_chunked, post_declaring, sign};
use serde_json::json;

use crate::world::{World, message, phone};

const CHAT: &str = "5511900000001@c.us";

fn inbox_path(world: &World, number_session: &str) -> String {
    world.fake.webhook(number_session).unwrap().0
}

#[tokio::test]
async fn an_event_is_written_once_even_when_it_comes_twice() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(1)).await;
    let session = number.session.as_str();
    let payload = message(
        CHAT,
        "3EB0C0FFEE",
        SystemClock.now_ms() / 1_000,
        false,
        "hello",
    );
    let body = world
        .fake
        .envelope(session, "message.any", payload.clone(), Some("evt_01one"))
        .unwrap();
    assert_eq!(world.fake.deliver(session, &body).await.unwrap(), 200);
    assert_eq!(world.fake.deliver(session, &body).await.unwrap(), 200);
    assert_eq!(world.events("message.any").await, 1);
    // The same message in another delivery of the engine, with another event identifier, is
    // still one message.
    let again = world
        .fake
        .envelope(session, "message.any", payload, Some("evt_01two"))
        .unwrap();
    assert_eq!(world.fake.deliver(session, &again).await.unwrap(), 200);
    assert_eq!(world.events("message.any").await, 1);
    // Another kind of event is one row per event of the engine.
    let reaction = json!({"id": "false_x", "reaction": {"text": "", "messageId": "3EB0C0FFEE"}});
    let first = world
        .fake
        .envelope(
            session,
            "message.reaction",
            reaction.clone(),
            Some("evt_01r"),
        )
        .unwrap();
    assert_eq!(world.fake.deliver(session, &first).await.unwrap(), 200);
    assert_eq!(world.fake.deliver(session, &first).await.unwrap(), 200);
    assert_eq!(world.events("message.reaction").await, 1);
    // Nothing of the event is in clear in the row.
    let stored: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(e)::text FROM engine_event e WHERE organization_id = $1",
    )
    .bind(world.organization().id())
    .fetch_all(world.hub().pool())
    .await
    .unwrap();
    for row in stored {
        assert!(
            !row.contains("hello") && !row.contains("5511900000001"),
            "{row}"
        );
    }
}

#[tokio::test]
async fn a_wrong_signature_an_unknown_engine_or_no_signature_is_refused() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(2)).await;
    let session = number.session.as_str();
    let url = inbox_path(&world, session);
    let body = world
        .fake
        .envelope(
            session,
            "message.any",
            message(CHAT, "3EB0AA01", SystemClock.now_ms() / 1_000, false, "x"),
            None,
        )
        .unwrap();
    let headers = |signature: &str| {
        vec![
            ("Content-Type", "application/json".to_owned()),
            ("X-Webhook-Hmac", signature.to_owned()),
        ]
    };
    let send = |url: String, headers: Vec<(&'static str, String)>, body: Vec<u8>| async move {
        let borrowed: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        post(&url, &borrowed, &body).await.unwrap()
    };
    let wrong = sign(b"another-secret", &body);
    assert_eq!(send(url.clone(), headers(&wrong), body.clone()).await, 401);
    assert_eq!(send(url.clone(), headers(""), body.clone()).await, 401);
    assert_eq!(
        send(
            url.clone(),
            vec![("Content-Type", "application/json".to_owned())],
            body.clone()
        )
        .await,
        401
    );
    // The right signature for another body.
    let (_, key) = world.fake.webhook(session).unwrap();
    let other = sign(key.as_bytes(), b"{}");
    assert_eq!(send(url.clone(), headers(&other), body.clone()).await, 401);
    // Another algorithm named.
    let right = sign(key.as_bytes(), &body);
    let mut named = headers(&right);
    named.push(("X-Webhook-Hmac-Algorithm", "sha256".to_owned()));
    assert_eq!(send(url.clone(), named, body.clone()).await, 401);
    // An engine nobody configured gets the same answer as a wrong signature.
    let unknown = url.replace(
        &world.engine().await.id.as_hyphenated().to_string(),
        &uuid::Uuid::new_v4().as_hyphenated().to_string(),
    );
    assert_eq!(send(unknown, headers(&right), body.clone()).await, 401);
    assert_eq!(world.events("message.any").await, 0);
    // And the right one passes.
    assert_eq!(send(url, headers(&right), body).await, 200);
    assert_eq!(world.events("message.any").await, 1);
}

#[tokio::test]
async fn an_enormous_a_mistyped_or_a_malformed_event_is_refused_and_nothing_is_written() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(3)).await;
    let session = number.session.as_str();
    let url = inbox_path(&world, session);
    let (_, key) = world.fake.webhook(session).unwrap();
    let signed = |body: &[u8]| sign(key.as_bytes(), body);

    // A declared length above the ceiling is refused before a byte of the body is read.
    let huge = vec![b' '; 300 * 1024];
    let signature = signed(&huge);
    let headers = [
        ("Content-Type", "application/json"),
        ("X-Webhook-Hmac", signature.as_str()),
    ];
    let status = post_declaring(&url, &headers, 300 * 1024).await.unwrap();
    assert_eq!(status, 413);
    // A body that does not declare its length is counted as it is read.
    let over = vec![b' '; 256 * 1024 + 1];
    let signature = signed(&over);
    let headers = [
        ("Content-Type", "application/json"),
        ("X-Webhook-Hmac", signature.as_str()),
    ];
    let status = post_chunked(&url, &headers, &over).await.unwrap();
    assert_eq!(status, 413);

    let body = world
        .fake
        .envelope(
            session,
            "message.any",
            message(CHAT, "3EB0AA02", SystemClock.now_ms() / 1_000, false, "x"),
            None,
        )
        .unwrap();
    let signature = signed(&body);
    let status = post(
        &url,
        &[
            ("Content-Type", "text/plain"),
            ("X-Webhook-Hmac", &signature),
        ],
        &body,
    )
    .await
    .unwrap();
    assert_eq!(status, 415);

    let now = SystemClock.now_ms();
    let malformed: Vec<Vec<u8>> = vec![
        b"{not json".to_vec(),
        b"[]".to_vec(),
        json!({"id": "evt_1", "timestamp": now, "event": "message.any", "session": session}).to_string().into_bytes(),
        json!({"id": "../x", "timestamp": now, "event": "message.any", "session": session, "payload": {}}).to_string().into_bytes(),
        json!({"id": "evt_1", "timestamp": now, "event": "message.secret", "session": session, "payload": {}}).to_string().into_bytes(),
        json!({"id": "evt_1", "timestamp": now, "event": "message.any", "session": "../other", "payload": {}}).to_string().into_bytes(),
        json!({"id": "evt_1", "timestamp": now, "event": "message.any", "session": "aaaaaaaaaaaaaaaaaaaaaaaaaa1", "payload": message(CHAT, "3EB0AA03", 1, false, "x")}).to_string().into_bytes(),
        json!({"id": "evt_1", "timestamp": now, "event": "message.any", "session": session, "payload": {"id": "../../x"}}).to_string().into_bytes(),
        json!({"id": "evt_1", "timestamp": now, "event": "session.status", "session": session, "payload": {"status": "HACKED"}}).to_string().into_bytes(),
        // Outside the window: a recorded event played again much later.
        json!({"id": "evt_1", "timestamp": now - 13 * 3_600_000, "event": "message.any", "session": session, "payload": message(CHAT, "3EB0AA04", 1, false, "x")}).to_string().into_bytes(),
        json!({"id": "evt_1", "timestamp": "now", "event": "message.any", "session": session, "payload": {}}).to_string().into_bytes(),
    ];
    for body in malformed {
        let signature = signed(&body);
        let status = post(
            &url,
            &[
                ("Content-Type", "application/json"),
                ("X-Webhook-Hmac", &signature),
            ],
            &body,
        )
        .await
        .unwrap();
        assert_eq!(status, 400, "{}", String::from_utf8_lossy(&body));
    }
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM engine_event WHERE organization_id = $1")
            .bind(world.organization().id())
            .fetch_one(world.hub().pool())
            .await
            .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(world.state("vendas").await, NumberState::Connected);
}

#[tokio::test]
async fn the_inbox_is_only_on_the_internal_port() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(4)).await;
    let session = number.session.as_str();
    let url = inbox_path(&world, session);
    let body = world
        .fake
        .envelope(
            session,
            "message.any",
            message(CHAT, "3EB0AA05", SystemClock.now_ms() / 1_000, false, "x"),
            None,
        )
        .unwrap();
    let (_, key) = world.fake.webhook(session).unwrap();
    let signature = sign(key.as_bytes(), &body);
    let internal = format!("http://{}", world.fixture.running().internal_addr());
    let connectors = format!("http://{}", world.fixture.addr());
    let on_connectors = url.replace(&internal, &connectors);
    assert_ne!(on_connectors, url);
    let headers = [
        ("Content-Type", "application/json"),
        ("X-Webhook-Hmac", signature.as_str()),
    ];
    assert_eq!(post(&on_connectors, &headers, &body).await.unwrap(), 404);
    assert_eq!(world.events("message.any").await, 0);
    // A reading of the address of the inbox does nothing either.
    let path = url.strip_prefix(&internal).unwrap();
    let read = arauto_testkit::http::get(
        world.fixture.running().internal_addr(),
        path,
        &[],
        std::time::Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(read.status, 404);
    assert_eq!(post(&url, &headers, &body).await.unwrap(), 200);
}

#[tokio::test]
async fn statuses_out_of_order_do_not_go_back() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(5)).await;
    let session = number.session.as_str();
    let now = SystemClock.now_ms();
    let status = |status: &str| json!({"status": status, "statuses": []});
    let fake = &world.fake;
    let deliver = |event: Vec<u8>| async move { fake.deliver(session, &event).await.unwrap() };
    // The engine lost WhatsApp (later), and an older "starting" arrives after it came back.
    let lost = world
        .fake
        .envelope_at(
            session,
            "session.status",
            status("STARTING"),
            None,
            now + 1_000,
        )
        .unwrap();
    let back = world
        .fake
        .envelope_at(
            session,
            "session.status",
            status("WORKING"),
            None,
            now + 2_000,
        )
        .unwrap();
    let older = world
        .fake
        .envelope_at(
            session,
            "session.status",
            status("STARTING"),
            None,
            now + 1_500,
        )
        .unwrap();
    assert_eq!(deliver(lost).await, 200);
    assert_eq!(world.state("vendas").await, NumberState::Reconnecting);
    assert_eq!(deliver(back).await, 200);
    assert_eq!(world.state("vendas").await, NumberState::Connected);
    assert_eq!(deliver(older).await, 200);
    assert_eq!(
        world.state("vendas").await,
        NumberState::Connected,
        "an older status is ignored"
    );
    // The device removed on the telephone: the session asks for a QR again.
    let removed = world
        .fake
        .envelope_at(
            session,
            "session.status",
            status("SCAN_QR_CODE"),
            None,
            now + 3_000,
        )
        .unwrap();
    assert_eq!(deliver(removed).await, 200);
    assert_eq!(world.state("vendas").await, NumberState::Disconnected);
    // The statuses are handled at once; the messages stay for the queue of notices.
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM engine_event WHERE organization_id = $1 AND kind = 'session.status' \
           AND processed_at IS NULL",
    )
    .bind(world.organization().id())
    .fetch_one(world.hub().pool())
    .await
    .unwrap();
    assert_eq!(open, 0);
}
