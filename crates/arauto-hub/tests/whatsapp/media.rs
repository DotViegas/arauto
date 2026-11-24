//! Files by address, both ways (decision 0055), through a Hermes in session and the fake
//! engine: `whatsapp_media_link` gives a short-lived address only to a Hermes that may read the
//! chat; the token has 43 base64url characters and only its digest is kept; the address is on
//! the public address of the hub; a token out of its format, unknown or past its end gets one
//! identical 404; a `GET` or a `HEAD` writes no row and does not extend the life; the purge
//! removes an address a day after its end. `whatsapp_send_file` with `url` refuses what is not
//! a public https address with AR-715 on protocol version 3, and another code below it.
//!
//! The download that goes through is tested in the unit tests of `fetch`: a hub of a test can
//! reach no public address, and its client has no allowance outside those tests.

use std::time::Duration;

use arauto_hub::media::{MEDIA_PREFIX, MediaToken, TOKEN_TEXT_LEN};
use arauto_hub::repo::purge;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::{DAY_MS, WHATSAPP_MEDIA_LINK_VALIDITY_MS};
use arauto_testkit::http::{self, Response};
use serde_json::{Value, json};

use crate::agent::{Agent, NONE, READ, READ_SEND, all, list, ok, refused};
use crate::world::{World, message, phone};

const NOW_S: i64 = 1_800_000_000;

/// The bytes of the file of the message.
const FILE: &[u8] = b"%PDF-1.4 the bill of the contact";

/// A world with `balcao` connected, a chat with Ana, a message of hers with a PDF, and a Hermes
/// that reads everything: the world, the Hermes, the chat of the hub and the message.
async fn with_a_file() -> (World, Agent, String, Value) {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let ana = format!("{}@c.us", phone(50));
    world.fake.add_chat(&session, &ana, "Ana");
    world.fake.register(&phone(50), &ana);
    let mut with_file = message(&ana, "3EB0D0000001", NOW_S, false, "");
    with_file["hasMedia"] = json!(true);
    let stored = format!("false_{ana}_3EB0D0000001.pdf");
    with_file["media"] =
        json!({"file": stored, "mimetype": "application/pdf", "filename": "bill.pdf"});
    world.fake.add_message(&session, &ana, with_file);
    world
        .fake
        .add_file(&stored, "application/pdf", FILE.to_vec());
    // Another chat, of Bruno.
    let bruno = format!("{}@c.us", phone(51));
    world.fake.add_chat(&session, &bruno, "Bruno");
    world.fake.register(&phone(51), &bruno);
    world.fake.add_message(
        &session,
        &bruno,
        message(&bruno, "3EB0D0000002", NOW_S, false, "yo"),
    );
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ)).await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let chat = listed["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|chat| chat["name"] == "Ana")
        .unwrap()["chat"]
        .as_str()
        .unwrap()
        .to_owned();
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let message_id = read["messages"][0]["message_id"].clone();
    (world, agent, chat, message_id)
}

/// The token of an address the hub gave.
fn token_of(url: &str) -> String {
    url.rsplit('/').next().unwrap().to_owned()
}

async fn get(world: &World, path: &str) -> Response {
    http::get(
        world.fixture.running().connectors_addr(),
        path,
        &[],
        Duration::from_secs(10),
    )
    .await
    .unwrap()
}

async fn head(world: &World, path: &str) -> Response {
    http::send(
        "HEAD",
        world.fixture.running().connectors_addr(),
        path,
        &[],
        &[],
        Duration::from_secs(10),
    )
    .await
    .unwrap()
}

/// Every row of a table, as text, in a fixed order.
async fn rows(world: &World, table: &str) -> Vec<String> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT row_to_json(t)::text FROM {table} t ORDER BY 1"
    )))
    .fetch_all(world.hub().pool())
    .await
    .unwrap()
}

async fn count(world: &World, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(world.hub().pool())
        .await
        .unwrap()
}

/// An answer, without the `date` header the server stamps on each one.
fn without_date(response: &Response) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let headers = response
        .headers
        .iter()
        .filter(|(name, _)| name != "date")
        .cloned()
        .collect();
    (response.status, headers, response.body.clone())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_link_serves_the_file_and_a_read_changes_nothing() {
    let (world, mut agent, chat, message_id) = with_a_file().await;
    let before_s = world.now().div_euclid(1_000);
    let link = ok(agent
        .call("whatsapp_media_link", json!({"message_id": message_id}))
        .await);
    assert_eq!(link["number"], "balcao");
    // The address is on the public address of the hub.
    let url = link["url"].as_str().unwrap().to_owned();
    let origin = world.hub().public_url().origin();
    assert!(url.starts_with(&format!("{origin}{MEDIA_PREFIX}")), "{url}");
    let token = token_of(&url);
    assert_eq!(token.len(), TOKEN_TEXT_LEN);
    assert_eq!(token.len(), 43);
    assert!(
        token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "{token}"
    );
    assert_eq!(url, format!("{origin}/v1/media/{token}"));
    // Ten minutes.
    let expires_at = link["expires_at"].as_i64().unwrap();
    let validity_s = WHATSAPP_MEDIA_LINK_VALIDITY_MS / 1_000;
    assert!(
        (before_s + validity_s..=before_s + validity_s + 5).contains(&expires_at),
        "{expires_at}"
    );
    // Only the digest is kept, with the Hermes, the number and the chat; never the token.
    let kept = rows(&world, "media_link").await;
    assert_eq!(kept.len(), 1);
    assert!(!kept[0].contains(&token), "{}", kept[0]);
    let digest: Vec<u8> = sqlx::query_scalar("SELECT token_digest FROM media_link")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    let parsed = MediaToken::parse(&token).unwrap();
    assert_eq!(digest, parsed.digest().as_bytes().to_vec());
    let chat_id: uuid::Uuid = sqlx::query_scalar("SELECT chat_id FROM media_link")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert_eq!(chat_id.as_hyphenated().to_string(), chat);
    // The audit says it was issued, never the address.
    let audited = rows(&world, "audit").await;
    let issued: Vec<&String> = audited
        .iter()
        .filter(|row| row.contains("media_link.issued"))
        .collect();
    assert_eq!(issued.len(), 1);
    assert!(
        issued[0].contains(&chat),
        "the chat is named: {}",
        issued[0]
    );
    assert!(audited.iter().all(|row| !row.contains(&token)));

    // GET and HEAD serve the file, as many times as asked.
    let path = format!("{MEDIA_PREFIX}{token}");
    let media_before = rows(&world, "media_link").await;
    let audit_before = rows(&world, "audit").await;
    for _ in 0..2 {
        let got = get(&world, &path).await;
        assert_eq!(got.status, 200);
        assert_eq!(got.body, FILE);
        assert_eq!(got.header("content-type"), Some("application/pdf"));
        assert_eq!(got.header("content-disposition"), Some("attachment"));
        assert_eq!(got.header("cache-control"), Some("no-store"));
        let headed = head(&world, &path).await;
        assert_eq!(headed.status, 200);
        assert!(headed.body.is_empty());
        assert_eq!(
            headed.header("content-length"),
            Some(FILE.len().to_string().as_str())
        );
    }
    // A read changes nothing: the row and its end are as they were, and no row was written.
    assert_eq!(rows(&world, "media_link").await, media_before);
    assert_eq!(rows(&world, "audit").await, audit_before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_link_needs_reading_in_the_chat_at_the_call_and_at_each_read() {
    let (world, mut agent, chat, message_id) = with_a_file().await;
    // Another Hermes, with the scope "list" and a line in Bruno's chat only: it does not read
    // Ana's.
    let mut other = world.agent().await;
    world.grant(&other.name, "balcao", list(NONE)).await;
    world
        .line(&other.name, "balcao", &format!("+{}", phone(51)), READ)
        .await;
    let outcome = other
        .call("whatsapp_media_link", json!({"message_id": message_id}))
        .await;
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar712));
    // A Hermes with no access at all.
    let mut stranger = world.agent().await;
    let outcome = stranger
        .call("whatsapp_media_link", json!({"message_id": message_id}))
        .await;
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar701));
    // Reading blocked in the chat by its line.
    world.line(&agent.name, "balcao", &chat, NONE).await;
    let outcome = agent
        .call("whatsapp_media_link", json!({"message_id": message_id}))
        .await;
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar712));
    assert_eq!(count(&world, "SELECT count(*) FROM media_link").await, 0);

    // Given, then the line taken away: the address stops serving (rule 9).
    world.line(&agent.name, "balcao", &chat, READ).await;
    let link = ok(agent
        .call("whatsapp_media_link", json!({"message_id": message_id}))
        .await);
    let path = format!("{MEDIA_PREFIX}{}", token_of(link["url"].as_str().unwrap()));
    assert_eq!(get(&world, &path).await.status, 200);
    world.line(&agent.name, "balcao", &chat, NONE).await;
    assert_eq!(get(&world, &path).await.status, 404);
    world.line(&agent.name, "balcao", &chat, READ).await;
    assert_eq!(get(&world, &path).await.status, 200);
    world.take_away(&agent.name, "balcao").await;
    assert_eq!(get(&world, &path).await.status, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn what_does_not_serve_gets_one_identical_404() {
    let (world, mut agent, _, message_id) = with_a_file().await;
    let link = ok(agent
        .call("whatsapp_media_link", json!({"message_id": message_id}))
        .await);
    let token = token_of(link["url"].as_str().unwrap());
    let unknown = MediaToken::generate().unwrap().reveal().to_string();
    let mut answers = Vec::new();
    for path in [
        MEDIA_PREFIX.to_owned(),
        format!("{MEDIA_PREFIX}abc"),
        format!("{MEDIA_PREFIX}{}", &token[..42]),
        format!("{MEDIA_PREFIX}{token}A"),
        format!("{MEDIA_PREFIX}{}+", &token[..42]),
        format!("{MEDIA_PREFIX}{}%41", &token[..42]),
        format!("{MEDIA_PREFIX}{token}/x"),
        format!("{MEDIA_PREFIX}{unknown}"),
    ] {
        let answer = get(&world, &path).await;
        assert_eq!(answer.status, 404, "{path}");
        answers.push(without_date(&answer));
    }
    // Past its end: the same answer.
    let path = format!("{MEDIA_PREFIX}{token}");
    assert_eq!(get(&world, &path).await.status, 200);
    sqlx::query("UPDATE media_link SET expires_at = $1, created_at = $1 - 1")
        .bind(world.now() - 1)
        .execute(world.hub().pool())
        .await
        .unwrap();
    let expired = get(&world, &path).await;
    assert_eq!(expired.status, 404);
    answers.push(without_date(&expired));
    assert!(
        answers.windows(2).all(|pair| pair[0] == pair[1]),
        "one answer for all of them"
    );
    assert_eq!(head(&world, &path).await.status, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_purge_removes_an_address_a_day_after_its_end() {
    let (world, mut agent, _, message_id) = with_a_file().await;
    for _ in 0..2 {
        ok(agent
            .call("whatsapp_media_link", json!({"message_id": message_id}))
            .await);
    }
    let now = world.now();
    let ids: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM media_link ORDER BY id")
        .fetch_all(world.hub().pool())
        .await
        .unwrap();
    // One ended a day ago, to the millisecond; the other a moment less.
    for (id, ended) in [(ids[0], now - DAY_MS), (ids[1], now - DAY_MS + 1)] {
        sqlx::query("UPDATE media_link SET expires_at = $2, created_at = $2 - 1 WHERE id = $1")
            .bind(id)
            .bind(ended)
            .execute(world.hub().pool())
            .await
            .unwrap();
    }
    let retention = purge::retention(world.hub().pool()).await.unwrap();
    assert_eq!(retention.days("media_link"), Some(1));
    let purged = purge::purge(
        world.hub().pool(),
        world.organization(),
        &retention,
        now,
        100,
        || true,
    )
    .await
    .unwrap();
    assert_eq!(purged.of(purge::Target::MediaLinks).rows, 1);
    let left: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM media_link")
        .fetch_all(world.hub().pool())
        .await
        .unwrap();
    assert_eq!(left, [ids[1]]);
}

/// `whatsapp_send_file` with `url`: what is not a public https address is refused with AR-715
/// before anything goes to the engine, and the refusals take no place in the send limit.
#[tokio::test(flavor = "multi_thread")]
async fn a_file_by_address_that_is_not_public_https_is_refused_with_ar715() {
    let (world, mut agent, chat, _) = with_a_file().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    world.fake.forget_received();
    let refused_addresses = [
        "http://files.example.org/bill.pdf",
        "https://127.0.0.1/bill.pdf",
        "https://10.0.0.1/bill.pdf",
        "https://169.254.169.254/latest/meta-data",
        "https://[::1]/bill.pdf",
        "https://[::ffff:192.168.0.1]/bill.pdf",
        "https://user@files.example.org/bill.pdf",
        "ftp://files.example.org/bill.pdf",
    ];
    // More refusals than the send limit allows in a minute: none of them takes a place.
    for round in 0..3 {
        for url in refused_addresses {
            let outcome = agent
                .call(
                    "whatsapp_send_file",
                    json!({"to": chat, "kind": "document", "url": url}),
                )
                .await;
            assert_eq!(refused(&outcome), Some(ErrorCode::Ar715), "{round}: {url}");
        }
    }
    assert!(
        world.fake.sent().is_empty(),
        "nothing reached the engine: {:?}",
        world.fake.sent()
    );
    ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "still here"}),
        )
        .await);
    // The form is still the protocol's: both, or a voice message with a caption.
    let both = agent
        .call(
            "whatsapp_send_file",
            json!({"to": chat, "kind": "document", "mimetype": "application/pdf",
                   "url": "https://files.example.org/a.pdf",
                   "data": data_encoding::BASE64.encode(b"%PDF")}),
        )
        .await;
    assert_eq!(refused(&both), Some(ErrorCode::Ar711));
    let voice = agent
        .call(
            "whatsapp_send_file",
            json!({"to": chat, "kind": "voice", "url": "https://10.0.0.1/a.mp3",
                   "caption": "hi"}),
        )
        .await;
    assert_eq!(refused(&voice), Some(ErrorCode::Ar711));

    // A connector of protocol version 2 cannot read AR-715: it gets AR-711.
    let mut older = world.agent_of_version_2().await;
    world.grant(&older.name, "balcao", all(READ_SEND)).await;
    let outcome = older
        .call(
            "whatsapp_send_file",
            json!({"to": chat, "kind": "document", "url": "https://10.0.0.1/bill.pdf"}),
        )
        .await;
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar711));
}

/// A voice message sent by content: `audio/mpeg` is converted by the engine, `audio/ogg` is
/// not (decision 0055).
#[tokio::test(flavor = "multi_thread")]
async fn a_voice_message_that_is_not_ogg_is_converted() {
    let (world, mut agent, chat, _) = with_a_file().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    world.fake.forget_received();
    for (mimetype, convert) in [("audio/mpeg", true), ("audio/ogg; codecs=opus", false)] {
        ok(agent
            .call(
                "whatsapp_send_file",
                json!({"to": chat, "kind": "voice", "mimetype": mimetype,
                       "data": data_encoding::BASE64.encode(b"ID3 audio")}),
            )
            .await);
        let (action, body) = world.fake.sent().last().cloned().unwrap();
        assert_eq!(action, "sendVoice");
        assert_eq!(body["convert"], json!(convert), "{mimetype}");
    }
}
