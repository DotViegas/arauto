//! Who may send to whom, and who may read what.

use arauto_protocol::errors::ErrorCode;
use serde_json::json;
use uuid::Uuid;

use crate::world::{World, ok, refused, texts};

#[tokio::test]
async fn without_a_link_nothing_is_sent_and_the_answer_is_the_one_of_a_name_that_does_not_exist() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;

    // With no link at all the tool is not even released, whoever is named.
    let to_stranger = a.send_to(&b.name, "hello").await;
    let to_nobody = a.send_to("nobody-here", "hello").await;
    assert_eq!(refused(&to_stranger), Some(ErrorCode::Ar701));
    assert_eq!(to_stranger, to_nobody, "the two answers are the same");

    // With a link to someone else, the Hermes it has no link to is like a name that does not
    // exist; and the link the other way round releases nothing for A.
    let c = world.agent().await;
    world.link(&a, &c, false).await;
    world.link(&b, &a, false).await;
    let to_stranger = a.send_to(&b.name, "hello").await;
    let to_nobody = a.send_to("nobody-here", "hello").await;
    assert_eq!(refused(&to_stranger), Some(ErrorCode::Ar707));
    assert_eq!(to_stranger, to_nobody, "the two answers are the same");
    // To itself, never.
    let own = a.name.clone();
    assert_eq!(
        refused(&a.send_to(&own, "hello").await),
        Some(ErrorCode::Ar707)
    );
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM peer_message")
        .fetch_one(&world.fixture.pool)
        .await
        .unwrap();
    assert_eq!(kept, 0);
}

#[tokio::test]
async fn a_message_is_kept_listed_and_read_and_reading_marks_it() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.link(&a, &b, false).await;

    let sent = ok(a.send_to(&b.name, "first").await);
    let thread = sent["thread"].as_str().unwrap().to_owned();
    assert!(Uuid::try_parse(sent["message"].as_str().unwrap()).is_ok());
    ok(a.send_in(&thread, "second").await);

    let listed = ok(b.call("hermes_list_threads", json!({})).await);
    let threads = listed["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["thread"], thread.as_str());
    assert_eq!(threads[0]["peer"], a.name.as_str());
    assert_eq!(threads[0]["unread"], 2);
    assert_eq!(listed["more"], false);

    let read = ok(b
        .call("hermes_read_thread", json!({"thread": thread}))
        .await);
    assert_eq!(texts(&read), ["first", "second"]);
    assert_eq!(read["peer"], a.name.as_str());
    let first = &read["messages"][0];
    assert_eq!(first["from"], a.name.as_str());
    assert_eq!(first["mine"], false);

    // Read: nothing is unread, and the same read again shows the most recent ones.
    let listed = ok(b.call("hermes_list_threads", json!({})).await);
    assert_eq!(listed["threads"][0]["unread"], 0);
    assert_eq!(b.read(&thread).await, ["first", "second"]);

    // The sender sees its own thread, and its messages as its own.
    let own = ok(a
        .call("hermes_read_thread", json!({"thread": thread}))
        .await);
    assert_eq!(texts(&own), ["first", "second"]);
    assert_eq!(own["messages"][0]["mine"], true);
    assert_eq!(own["messages"][0]["from"], a.name.as_str());

    // The text is encrypted in the database.
    let stored: Vec<Vec<u8>> = sqlx::query_scalar("SELECT text_enc FROM peer_message")
        .fetch_all(&world.fixture.pool)
        .await
        .unwrap();
    assert_eq!(stored.len(), 2);
    for text in stored {
        assert!(!text.windows(5).any(|window| window == b"first"));
        assert!(!text.windows(6).any(|window| window == b"second"));
    }
}

#[tokio::test]
async fn a_read_starts_at_the_oldest_unread_and_walks_with_before_and_after() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.link(&a, &b, false).await;

    let thread = a.thread_with(&b.name, "m1").await;
    for text in ["m2", "m3", "m4", "m5"] {
        world.advance(10);
        ok(a.send_in(&thread, text).await);
    }
    let first = ok(b
        .call("hermes_read_thread", json!({"thread": thread, "limit": 2}))
        .await);
    assert_eq!(texts(&first), ["m1", "m2"]);
    assert_eq!(first["more"], true);
    // What was not given is still unread: the next read goes on from there.
    let second = ok(b
        .call("hermes_read_thread", json!({"thread": thread, "limit": 2}))
        .await);
    assert_eq!(texts(&second), ["m3", "m4"]);
    let last = second["messages"][1]["id"].as_str().unwrap().to_owned();
    let after = ok(b
        .call(
            "hermes_read_thread",
            json!({"thread": thread, "after": last}),
        )
        .await);
    assert_eq!(texts(&after), ["m5"]);
    assert_eq!(after["more"], false);
    let before = ok(b
        .call(
            "hermes_read_thread",
            json!({"thread": thread, "before": last, "limit": 2}),
        )
        .await);
    assert_eq!(texts(&before), ["m2", "m3"]);
    assert_eq!(before["more"], true);

    // A cursor that is no message of this thread is an invalid argument.
    let stray = b
        .call(
            "hermes_read_thread",
            json!({"thread": thread, "after": Uuid::new_v4().to_string()}),
        )
        .await;
    assert_eq!(refused(&stray), Some(ErrorCode::Ar711));
}

#[tokio::test]
async fn a_thread_of_others_is_neither_read_nor_listed_and_answers_like_one_that_does_not_exist() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    let mut c = world.agent().await;
    world.link(&a, &b, false).await;
    // C is a peer of both, in both directions.
    for other in [&a, &b] {
        world.link(&c, other, false).await;
        world.link(other, &c, false).await;
    }
    let thread = a.thread_with(&b.name, "between a and b").await;

    let of_others = c
        .call("hermes_read_thread", json!({"thread": thread}))
        .await;
    let of_nobody = c
        .call(
            "hermes_read_thread",
            json!({"thread": Uuid::new_v4().to_string()}),
        )
        .await;
    assert_eq!(refused(&of_others), Some(ErrorCode::Ar708));
    assert_eq!(of_others, of_nobody, "the two answers are the same");

    let listed = ok(c.call("hermes_list_threads", json!({})).await);
    assert_eq!(listed["threads"].as_array().unwrap().len(), 0);

    // Nor does C write into it.
    assert_eq!(
        refused(&c.send_in(&thread, "let me in").await),
        Some(ErrorCode::Ar708)
    );
    // And naming a participant does not help: the thread is checked first.
    let named = c
        .call(
            "hermes_send_message",
            json!({"thread": thread, "to": b.name, "text": "let me in"}),
        )
        .await;
    assert_eq!(refused(&named), Some(ErrorCode::Ar708));
}

#[tokio::test]
async fn there_is_no_answer_without_the_link_back() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.link(&a, &b, false).await;
    let thread = a.thread_with(&b.name, "question").await;
    assert_eq!(b.read(&thread).await, ["question"]);

    // B may send to nobody: the tool is not released to it.
    assert_eq!(
        refused(&b.send_in(&thread, "answer").await),
        Some(ErrorCode::Ar701)
    );
    // And once it may send to someone else, A is still out of its reach.
    let other = world.agent().await;
    world.link(&b, &other, false).await;
    assert_eq!(
        refused(&b.send_in(&thread, "answer").await),
        Some(ErrorCode::Ar707)
    );
    assert_eq!(
        refused(&b.send_to(&a.name, "answer").await),
        Some(ErrorCode::Ar707)
    );

    world.link(&b, &a, false).await;
    ok(b.send_in(&thread, "answer").await);
    // The read of A starts at what it had not read; the next one shows the thread.
    assert_eq!(a.read(&thread).await, ["answer"]);
    assert_eq!(a.read(&thread).await, ["question", "answer"]);
    // With `to`, it must be the other participant of the thread.
    let c = world.agent().await;
    world.link(&b, &c, false).await;
    let wrong = b
        .call(
            "hermes_send_message",
            json!({"thread": thread, "to": c.name, "text": "to another"}),
        )
        .await;
    assert_eq!(refused(&wrong), Some(ErrorCode::Ar708));
}

#[tokio::test]
async fn taking_the_link_away_cuts_what_was_not_read() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.link(&a, &b, false).await;
    world.link(&b, &a, false).await;

    let thread = a.thread_with(&b.name, "read in time").await;
    assert_eq!(b.read(&thread).await, ["read in time"]);
    world.advance(10);
    ok(b.send_in(&thread, "from b").await);
    world.advance(10);
    ok(a.send_in(&thread, "never read").await);

    world.unlink(&a, &b).await;
    // The thread is still there, by the link from B to A; what A sent and B had not read is
    // not delivered, and does not count as unread.
    assert_eq!(b.read(&thread).await, ["read in time", "from b"]);
    let listed = ok(b.call("hermes_list_threads", json!({})).await);
    assert_eq!(listed["threads"][0]["unread"], 0);
    // A, who wrote it, still sees it: its read starts at what it had not read.
    assert_eq!(a.read(&thread).await, ["from b", "never read"]);
    assert_eq!(
        a.read(&thread).await,
        ["read in time", "from b", "never read"]
    );

    // With the link back, it is delivered again: it was kept.
    world.link(&a, &b, false).await;
    assert_eq!(b.read(&thread).await, ["never read"]);

    // Without a link in either direction the thread is gone for both.
    world.unlink(&a, &b).await;
    world.unlink(&b, &a).await;
    // Each still has a link with someone else, so its tools are released.
    let other = world.agent().await;
    world.link(&a, &other, false).await;
    world.link(&b, &other, false).await;
    for agent in [&mut a, &mut b] {
        let gone = agent
            .call("hermes_read_thread", json!({"thread": thread}))
            .await;
        assert_eq!(refused(&gone), Some(ErrorCode::Ar708));
        let listed = ok(agent.call("hermes_list_threads", json!({})).await);
        assert_eq!(listed["threads"].as_array().unwrap().len(), 0);
    }
}

#[tokio::test]
async fn a_message_is_kept_for_a_hermes_that_is_not_connected() {
    let world = World::new().await;
    let mut a = world.agent().await;
    // B exists and has never connected.
    let b = world.fixture.hermes().await;
    world
        .fixture
        .admin
        .link_add(&a.name, &b, true, false, world.now())
        .await
        .unwrap();
    ok(a.send_to(&b, "for later").await);
    let (kept, notices): (i64, i64) =
        sqlx::query_as("SELECT (SELECT count(*) FROM peer_message), (SELECT count(*) FROM notice)")
            .fetch_one(&world.fixture.pool)
            .await
            .unwrap();
    assert_eq!((kept, notices), (1, 0), "kept, without a notice");
}

#[tokio::test]
async fn the_threads_are_listed_from_the_most_recent_and_walk_with_before() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.link(&a, &b, false).await;
    let mut threads = Vec::new();
    for text in ["one", "two", "three"] {
        world.advance(1_000);
        threads.push(a.thread_with(&b.name, text).await);
    }
    let page = ok(b.call("hermes_list_threads", json!({"limit": 2})).await);
    let listed: Vec<&str> = page["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|thread| thread["thread"].as_str().unwrap())
        .collect();
    assert_eq!(listed, [threads[2].as_str(), threads[1].as_str()]);
    assert_eq!(page["more"], true);
    let rest = ok(b
        .call("hermes_list_threads", json!({"before": threads[1]}))
        .await);
    assert_eq!(rest["threads"].as_array().unwrap().len(), 1);
    assert_eq!(rest["threads"][0]["thread"], threads[0].as_str());
    assert_eq!(rest["more"], false);

    let stray = b
        .call(
            "hermes_list_threads",
            json!({"before": Uuid::new_v4().to_string()}),
        )
        .await;
    assert_eq!(refused(&stray), Some(ErrorCode::Ar708));
}
