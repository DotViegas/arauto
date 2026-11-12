//! A notice of the hub wakes the Hermes through its local API (`docs/11-conector.md`,
//! section 6): the real hub, the real connector, and a local API of Hermes that only records
//! what it was asked.

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_connector::Connector;
use arauto_connector::wake::WakeTarget;
use arauto_protocol::message::CallOutcome;
use arauto_testkit::hub::fixture::{Fixture, settings as hub_settings};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{PATIENCE, connected, eventually, settings};

const KEY: &str = "CANARY-key-of-the-local-api-5d1e";
const TEXT: &str = "CANARY-text-of-a-message-between-agents-77aa";

/// One request the local API received: its head and its body.
type Asked = (String, String);

/// A local API of Hermes: it records each request and answers with the status it is set to.
struct FakeApi {
    url: String,
    asked: Arc<Mutex<Vec<Asked>>>,
    status: Arc<AtomicU16>,
}

impl FakeApi {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/runs", listener.local_addr().unwrap());
        let asked: Arc<Mutex<Vec<Asked>>> = Arc::default();
        let status = Arc::new(AtomicU16::new(202));
        let (kept, answer) = (Arc::clone(&asked), Arc::clone(&status));
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (kept, answer) = (Arc::clone(&kept), Arc::clone(&answer));
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut chunk = [0u8; 1_024];
                    let (head, body) = loop {
                        let read = stream.read(&mut chunk).await.unwrap_or(0);
                        bytes.extend_from_slice(&chunk[..read]);
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        if let Some((head, body)) = text.split_once("\r\n\r\n") {
                            let length: usize = head
                                .lines()
                                .find_map(|line| line.strip_prefix("Content-Length: "))
                                .and_then(|length| length.trim().parse().ok())
                                .unwrap_or(0);
                            if body.len() >= length || read == 0 {
                                break (head.to_owned(), body.to_owned());
                            }
                        } else if read == 0 {
                            break (text, String::new());
                        }
                    };
                    kept.lock().unwrap().push((head, body));
                    let status = answer.load(Ordering::SeqCst);
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
                    );
                    let _ = stream.write_all(reply.as_bytes()).await;
                });
            }
        });
        Self { url, asked, status }
    }

    fn asked(&self) -> Vec<Asked> {
        self.asked.lock().unwrap().clone()
    }
}

async fn send(from: &Connector, to: &str, text: &str) -> Map<String, Value> {
    let Value::Object(args) = json!({"to": to, "text": text}) else {
        unreachable!()
    };
    match from.call("hermes_send_message", args).await {
        CallOutcome::Ok(content) => content.into_map(),
        CallOutcome::Refused { code, .. } => panic!("refused with {code}"),
    }
}

/// The states of the notices of the hub, the oldest first.
async fn notices(fixture: &Fixture) -> Vec<String> {
    sqlx::query_scalar("SELECT state FROM notice ORDER BY created_at, id")
        .fetch_all(&fixture.pool)
        .await
        .unwrap()
}

/// Two Hermes connected; the second may be woken by the messages of the first, and its
/// connector wakes through `target`.
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
    fixture.admin.wake(&woken, true, now).await.unwrap();
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
