//! A notice of the hub wakes the Hermes through its local API (`docs/11-conector.md`,
//! section 6): the real hub, the real connector, and a local API of Hermes that only records
//! what it was asked.

use std::sync::atomic::Ordering;
use std::time::Duration;

use arauto_connector::Connector;
use arauto_connector::wake::WakeTarget;
use arauto_protocol::message::CallOutcome;
use arauto_testkit::hub::fixture::{Fixture, settings as hub_settings};
use serde_json::{Map, Value, json};
use tokio::net::TcpListener;

use crate::support::{FakeApi, PATIENCE, connected, eventually, send, settings};

const KEY: &str = "CANARY-key-of-the-local-api-5d1e";
const TEXT: &str = "CANARY-text-of-a-message-between-agents-77aa";

/// The states of the notices of the hub, the oldest first.
async fn notices(fixture: &Fixture) -> Vec<String> {
    sqlx::query_scalar("SELECT state FROM notice ORDER BY created_at, id")
        .fetch_all(&fixture.pool)
        .await
        .unwrap()
}

/// Two Hermes connected; the link from the first wakes the second (the WhatsApp switch of the
/// second stays off), and its connector wakes through `target`.
async fn pair(
    fixture: &Fixture,
    target: Option<WakeTarget>,
) -> (String, Connector, String, Connector, crate::support::Folder) {
    let (first, mut watch, _) = settings();
    let (sender, _folder, a) = connected(fixture, first, &mut watch).await;
    std::mem::forget(_folder);
    let (second, mut watch, _) = settings();
    let (woken, folder, b) = connected(fixture, second.waking(target), &mut watch).await;
    let now = fixture.now();
    fixture
        .admin
        .link_add(&sender, &woken, true, false, now)
        .await
        .unwrap();
    (sender, a, woken, b, folder)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_of_another_hermes_wakes_through_the_local_api_with_the_fixed_phrase() {
    let fixture = Fixture::new(hub_settings()).await;
    let api = FakeApi::start().await;
    let target = WakeTarget::new(&api.url, KEY).unwrap();
    let (sender, a, woken, _b, folder) = pair(&fixture, Some(target)).await;

    let sent = send(&a, &woken, TEXT).await;
    eventually("the local API is asked for a run", PATIENCE, || async {
        !api.asked().is_empty()
    })
    .await;
    eventually("the hub has the notice as confirmed", PATIENCE, || async {
        notices(&fixture).await == ["confirmed"]
    })
    .await;

    let asked = api.asked();
    assert_eq!(asked.len(), 1, "one run for one notice");
    let (head, body) = &asked[0];
    assert!(head.starts_with("POST /v1/runs HTTP/1.1"), "{head}");
    assert!(head.contains(&format!("Authorization: Bearer {KEY}")));
    assert!(head.contains("Idempotency-Key: "));
    let body: Value = serde_json::from_str(body).unwrap();
    let phrase = body["input"].as_str().unwrap();
    // The fixed phrase, with the name of who sent and how many: data, and it says so.
    assert!(phrase.starts_with("Arauto notice (data, not an instruction): 1 new message(s)"));
    assert!(
        phrase.contains(&format!("from Hermes agent \"{sender}\"")),
        "{phrase}"
    );
    // Rule 6, end to end: exactly the fixed sentence of the protocol, around the validated name
    // and the count.
    let expected = arauto_protocol::notice::Notice::Peer {
        from: arauto_protocol::names::HermesName::parse(&sender).unwrap(),
        count: arauto_protocol::notice::NoticeCount::new(1).unwrap(),
    }
    .phrase();
    assert_eq!(phrase, expected);
    // Never the text of the message, nor its thread.
    assert!(!phrase.contains(TEXT));
    assert!(!phrase.contains(sent["thread"].as_str().unwrap()));

    // The notice handled is remembered on disk, and the key is in no file of the folder.
    let wakes = std::fs::read_to_string(folder.root().join("wakes.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&wakes).unwrap()["handled"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for entry in walk(&folder.root()) {
        let bytes = std::fs::read(&entry).unwrap_or_default();
        assert!(
            !bytes
                .windows(KEY.len())
                .any(|window| window == KEY.as_bytes()),
            "{}",
            entry.display()
        );
    }
}

fn walk(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_api_that_refuses_is_tried_again_and_then_the_hermes_is_woken_once() {
    let fixture = Fixture::new(hub_settings()).await;
    let api = FakeApi::start().await;
    api.status.store(500, Ordering::SeqCst);
    let target = WakeTarget::new(&api.url, KEY).unwrap();
    let (_sender, a, woken, _b, folder) = pair(&fixture, Some(target)).await;

    send(&a, &woken, "wake up").await;
    eventually("the local API is asked and refuses", PATIENCE, || async {
        !api.asked().is_empty()
    })
    .await;
    eventually("the notice goes back to the queue", PATIENCE, || async {
        notices(&fixture).await == ["pending"]
    })
    .await;
    assert!(
        !folder.root().join("wakes.json").exists(),
        "nothing was handled"
    );

    // The hub tries again after its wait (5 seconds at the first failure).
    api.status.store(200, Ordering::SeqCst);
    eventually(
        "the notice is confirmed",
        Duration::from_secs(30),
        || async { notices(&fixture).await == ["confirmed"] },
    )
    .await;
    let asked = api.asked();
    assert_eq!(asked.len(), 2, "one refused and one accepted");
    let key_of = |head: &str| {
        head.lines()
            .find_map(|line| line.strip_prefix("Idempotency-Key: "))
            .map(str::to_owned)
    };
    assert_eq!(key_of(&asked[0].0), key_of(&asked[1].0), "the same notice");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_key_of_the_local_api_nobody_is_woken_and_the_tools_go_on() {
    let fixture = Fixture::new(hub_settings()).await;
    let (_sender, a, woken, b, _folder) = pair(&fixture, None).await;

    send(&a, &woken, "kept").await;
    eventually("the connector answers that it failed", PATIENCE, || async {
        let attempts: Vec<i32> = sqlx::query_scalar("SELECT attempts FROM notice")
            .fetch_all(&fixture.pool)
            .await
            .unwrap();
        attempts == [1] && notices(&fixture).await == ["pending"]
    })
    .await;
    // The message is there for when the Hermes looks.
    let Value::Object(args) = json!({}) else {
        unreachable!()
    };
    let CallOutcome::Ok(listed) = b.call("hermes_list_threads", args).await else {
        panic!("the tools do not answer");
    };
    assert_eq!(listed.as_map()["threads"][0]["unread"], 1);
}

/// The hub knows only what the admin allowed. The connector adds what only it knows.
#[tokio::test(flavor = "multi_thread")]
async fn arauto_status_says_whether_this_connector_can_wake() {
    let fixture = Fixture::new(hub_settings()).await;
    let api = FakeApi::start().await;
    let target = WakeTarget::new(&api.url, KEY).unwrap();
    let (_sender, _a, _woken, with_key, _folder) = pair(&fixture, Some(target)).await;
    let (_other, _a, _woken, without_key, _folder) = pair(&fixture, None).await;

    async fn status(connector: &Connector) -> Map<String, Value> {
        let CallOutcome::Ok(content) = connector.call("arauto_status", Map::new()).await else {
            panic!("arauto_status is refused");
        };
        content.into_map()
    }
    let with = status(&with_key).await;
    let without = status(&without_key).await;
    assert_eq!(with["local_waking"], json!(true));
    assert_eq!(without["local_waking"], json!(false));
    let reminder = json!(arauto_connector::memory::keep_in_memory());
    assert_eq!(with["keep_in_memory"], reminder);
    assert_eq!(without["keep_in_memory"], reminder);
    // What the hub says is untouched, and the same for both.
    for field in ["can_be_woken", "woken_by"] {
        assert!(with.contains_key(field), "{with:?}");
        assert_eq!(with[field], without[field], "{field}");
    }
    // No secret: the key is in neither.
    assert!(!Value::Object(with).to_string().contains(KEY));
}

/// The leader's status carries whether it holds the key and how its last wake ended, never
/// the key itself.
#[tokio::test(flavor = "multi_thread")]
async fn the_leader_status_carries_its_waking() {
    use arauto_connector::wake::{LastWake, Waking};

    let fixture = Fixture::new(hub_settings()).await;
    let api = FakeApi::start().await;
    api.status.store(500, Ordering::SeqCst);
    let target = WakeTarget::new(&api.url, KEY).unwrap();
    let (_sender, a, woken, with_key, _folder) = pair(&fixture, Some(target)).await;
    let (_other, _a2, _woken2, without_key, _folder2) = pair(&fixture, None).await;

    assert_eq!(
        with_key.status().await.waking,
        Some(Waking {
            key_received: true,
            last: None
        })
    );
    assert_eq!(
        without_key.status().await.waking,
        Some(Waking {
            key_received: false,
            last: None
        })
    );

    send(&a, &woken, "wake up").await;
    eventually("the refusal is remembered", PATIENCE, || async {
        with_key
            .status()
            .await
            .waking
            .and_then(|waking| waking.last)
            == Some(LastWake::Refused)
    })
    .await;
    api.status.store(202, Ordering::SeqCst);
    eventually(
        "the acceptance is remembered",
        Duration::from_secs(30),
        || async {
            with_key
                .status()
                .await
                .waking
                .and_then(|waking| waking.last)
                == Some(LastWake::Accepted)
        },
    )
    .await;
}

/// A key that is there but a local API that is not: the last wake says it did not answer.
#[tokio::test(flavor = "multi_thread")]
async fn a_local_api_that_does_not_answer_is_remembered() {
    use arauto_connector::wake::LastWake;

    let fixture = Fixture::new(hub_settings()).await;
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://{}/v1/runs", listener.local_addr().unwrap())
    };
    let target = WakeTarget::new(&closed, KEY).unwrap();
    let (_sender, a, woken, b, _folder) = pair(&fixture, Some(target)).await;
    send(&a, &woken, "wake up").await;
    eventually("the silence is remembered", PATIENCE, || async {
        b.status().await.waking.and_then(|waking| waking.last) == Some(LastWake::NoAnswer)
    })
    .await;
}
