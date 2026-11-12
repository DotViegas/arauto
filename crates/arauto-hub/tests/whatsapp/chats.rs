//! The chats of a number (`repo::chat`): the opaque identifier of the hub for an identifier of
//! the engine, created once even when asked twice at the same time, never found through
//! another number; and the body of an event, read back for the queue of notices.

use arauto_hub::engine::EngineChatId;
use arauto_hub::repo::{chat, device, engine_event};
use arauto_protocol::clock::{Clock, SystemClock};

use crate::world::{World, message, phone};

const CHAT: &str = "5511900000001@c.us";

#[tokio::test]
async fn a_chat_of_the_engine_is_one_chat_of_the_hub_and_only_of_its_number() {
    let world = World::new().await;
    let ours = world.connected("vendas", &phone(60)).await;
    let other = world.connected("suporte", &phone(61)).await;
    let pool = world.hub().pool();
    let organization = world.organization();
    let key = device::organization_key(pool, organization, &world.vault)
        .await
        .unwrap();
    let engine_chat = EngineChatId::parse(CHAT).unwrap();
    assert_eq!(
        chat::find(pool, organization, &key, ours.id, &engine_chat)
            .await
            .unwrap(),
        None
    );

    // Two at the same time get the same chat.
    let (mut first, mut second) = (pool.acquire().await.unwrap(), pool.acquire().await.unwrap());
    let now = world.now();
    let (a, b) = tokio::join!(
        chat::resolve(&mut first, organization, &key, ours.id, &engine_chat, now),
        chat::resolve(&mut second, organization, &key, ours.id, &engine_chat, now),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a, b);
    let chats: i64 = sqlx::query_scalar("SELECT count(*) FROM chat WHERE number_id = $1")
        .bind(ours.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(chats, 1, "no empty chat is left behind");
    assert_eq!(
        chat::find(pool, organization, &key, ours.id, &engine_chat)
            .await
            .unwrap(),
        Some(a)
    );
    assert_eq!(
        chat::engine_ids(pool, organization, &key, ours.id, a)
            .await
            .unwrap(),
        vec![engine_chat.clone()]
    );

    // The same identifier in another number is another chat, and one number never reaches
    // the chat of the other.
    assert_eq!(
        chat::find(pool, organization, &key, other.id, &engine_chat)
            .await
            .unwrap(),
        None
    );
    assert!(
        chat::engine_ids(pool, organization, &key, other.id, a)
            .await
            .unwrap()
            .is_empty()
    );
    let theirs = chat::resolve(&mut first, organization, &key, other.id, &engine_chat, now)
        .await
        .unwrap();
    assert_ne!(theirs, a);

    // Nothing of the identifier of the engine is in clear.
    let stored: Vec<String> =
        sqlx::query_scalar("SELECT row_to_json(c)::text FROM chat_engine_id c")
            .fetch_all(pool)
            .await
            .unwrap();
    for row in stored {
        assert!(!row.contains("5511900000001"), "{row}");
    }
}

#[tokio::test]
async fn the_body_of_an_event_is_read_back_whole_and_a_message_is_handled_as_it_arrives() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(62)).await;
    let session = number.session.as_str();
    let payload = message(
        CHAT,
        "3EB0B0D1",
        SystemClock.now_ms() / 1_000,
        false,
        "read me back",
    );
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", payload)
            .await
            .unwrap(),
        200
    );
    let pool = world.hub().pool();
    // The message became its notices in the write that recorded it: nothing is left open.
    let open = engine_event::open(pool, world.organization(), 10)
        .await
        .unwrap();
    assert!(open.is_empty(), "{open:?}");
    let (event, of_number): (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT id, number_id FROM engine_event \
         WHERE organization_id = $1 AND kind = 'message.any' AND processed_at IS NOT NULL",
    )
    .bind(world.organization().id())
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(of_number, number.id);
    let key = device::organization_key(pool, world.organization(), &world.vault)
        .await
        .unwrap();
    let body = engine_event::body(pool, world.organization(), &key, event)
        .await
        .unwrap()
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["payload"]["body"], "read me back");
    assert_eq!(body["event"], "message.any");
}
