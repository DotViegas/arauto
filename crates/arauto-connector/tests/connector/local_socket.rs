//! The local socket of the leader (`docs/14-protocolo-do-canal.md`, section 11): what a hostile
//! process of the same user sends is refused without bringing the leader down, no answer
//! carries a secret, and a restart ends the leader and every copy that forwards to it.

use std::time::Duration;

use arauto_connector::{Connector, Role, Settings};
use arauto_platform::LocalStream;
use data_encoding::{BASE32_NOPAD, BASE64, HEXLOWER};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::support::{self, PATIENCE, connected, status_works};

struct Client {
    lines: tokio::io::Lines<BufReader<tokio::io::ReadHalf<LocalStream>>>,
    writer: tokio::io::WriteHalf<LocalStream>,
}

impl Client {
    async fn connect(home: &arauto_connector::Home) -> Self {
        let stream = LocalStream::connect(home.root()).await.unwrap();
        let (reader, writer) = tokio::io::split(stream);
        Self {
            lines: BufReader::new(reader).lines(),
            writer,
        }
    }

    async fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).await.unwrap();
        self.writer.flush().await.unwrap();
    }

    async fn ask(&mut self, line: &str) -> Value {
        self.send(format!("{line}\n").as_bytes()).await;
        self.answer().await
    }

    async fn answer(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(20), self.lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }

    /// Whether the leader closed this connection.
    async fn closed(&mut self) -> bool {
        matches!(
            tokio::time::timeout(Duration::from_secs(20), self.lines.next_line()).await,
            Ok(Ok(None) | Err(_))
        )
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hostile_lines_on_the_local_socket_are_refused_and_the_leader_goes_on() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s, &mut watch).await;
    let device = folder.current().unwrap();
    let mut client = Client::connect(&folder.home).await;

    for (line, error) in [
        ("not json", "invalid"),
        ("[1,2,3]", "invalid"),
        (r#"{"v":1,"op":"status","op":"leave"}"#, "invalid"),
        (r#"{"v":2,"op":"status"}"#, "unsupported"),
        (r#"{"op":"status"}"#, "unsupported"),
        (r#"{"v":1,"op":"run","command":"rm -rf"}"#, "unsupported"),
        (r#"{"v":1,"op":"call","tool":"arauto_status"}"#, "invalid"),
        (
            r#"{"v":1,"op":"call","id":-3,"tool":"arauto_status"}"#,
            "invalid",
        ),
        (
            r#"{"v":1,"op":"call","id":1,"tool":"../../etc"}"#,
            "invalid",
        ),
    ] {
        let answer = client.ask(line).await;
        assert_eq!(answer["ok"], false, "{line}");
        assert_eq!(answer["error"], error, "{line}");
        // Nothing of what came is repeated.
        let text = answer.to_string();
        assert!(!text.contains("rm -rf") && !text.contains("etc"), "{text}");
    }
    let deep = format!(
        r#"{{"v":1,"op":"status","x":{}{}}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    assert_eq!(client.ask(&deep).await["error"], "invalid");

    // A call that works, by its number, and one of a tool the hub refuses.
    let answer = client
        .ask(r#"{"v":1,"op":"call","id":9,"tool":"arauto_status","args":{}}"#)
        .await;
    assert_eq!(answer["id"], 9);
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["content"]["connected"], true);
    let answer = client
        .ask(r#"{"v":1,"op":"call","id":10,"tool":"hermes_list_peers","args":{}}"#)
        .await;
    assert_eq!(answer, json!({"v":1,"id":10,"ok":false,"code":"AR-701"}));

    // The status says no secret.
    let status = client.ask(r#"{"v":1,"op":"status","id":3}"#).await;
    assert_eq!(status["id"], 3);
    assert_eq!(status["status"]["role"], "leader");
    let text = status.to_string();
    let identity = folder.identity(&device);
    let credential = folder.credential(&device).unwrap();
    for secret in [
        identity.private().expose().as_slice(),
        credential.as_slice(),
    ] {
        for written in [
            HEXLOWER.encode(secret),
            BASE64.encode(secret),
            BASE32_NOPAD.encode(secret),
            BASE32_NOPAD.encode(secret).to_ascii_lowercase(),
        ] {
            assert!(!text.contains(&written), "a secret in the status: {text}");
        }
    }

    // A line far above the ceiling: the connection is dropped, and the leader goes on.
    let mut flooder = Client::connect(&folder.home).await;
    let flood = vec![b'x'; 7 * 1024 * 1024];
    let _ = flooder.writer.write_all(&flood).await;
    assert!(flooder.closed().await);
    assert!(status_works(&connector).await);
    let answer = client.ask(r#"{"v":1,"op":"tools"}"#).await;
    assert_eq!(answer, json!({"v":1,"ok":true,"tools":["arauto_status"]}));
    connector.shutdown().await;
}

/// A restart ends the leader and every copy that forwards to it: Hermes starts each one again
/// on the new program, and none of the old ones takes the place of the leader.
#[tokio::test(flavor = "multi_thread")]
async fn a_restart_ends_the_leader_and_every_copy_that_forwards_to_it() {
    let folder = support::Folder::new();
    let leader = Connector::start(folder.home.clone(), Settings::for_tests())
        .await
        .unwrap();
    assert_eq!(leader.role(), Role::Leader);
    let mut relays = Vec::new();
    for _ in 0..2 {
        let relay = Connector::start(folder.home.clone(), Settings::for_tests())
            .await
            .unwrap();
        assert_eq!(relay.role(), Role::Relay);
        relays.push(relay);
    }
    let mut client = Client::connect(&folder.home).await;
    assert_eq!(
        client.ask(r#"{"v":1,"op":"restart","id":4}"#).await,
        json!({"v":1,"id":4,"ok":true})
    );
    // The last line of every connection.
    assert_eq!(client.answer().await, json!({"v":1,"restart":true}));
    assert!(client.closed().await);
    for connector in std::iter::once(&leader).chain(&relays) {
        tokio::time::timeout(PATIENCE, connector.finished())
            .await
            .expect("every copy ends");
    }
    // None of them took the place of the leader.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(LocalStream::connect(folder.home.root()).await.is_err());
    for connector in std::iter::once(leader).chain(relays) {
        assert_eq!(connector.role(), Role::Starting);
        connector.shutdown().await;
    }
}

/// A copy that forwards lists what the leader lists from the moment it starts: here the list
/// of the last session, which the leader read from `state.json`.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_that_forwards_lists_what_the_leader_lists_from_its_start() {
    let folder = support::Folder::new();
    folder.home.prepare().unwrap();
    std::fs::write(
        folder.root().join("state.json"),
        br#"{"v":1,"tools":["arauto_status","hermes_list_peers"]}"#,
    )
    .unwrap();
    let leader = Connector::start(folder.home.clone(), Settings::for_tests())
        .await
        .unwrap();
    assert_eq!(leader.role(), Role::Leader);
    let listed: Vec<String> = leader
        .tools()
        .iter()
        .map(|tool| tool.as_str().to_owned())
        .collect();
    assert_eq!(listed, ["arauto_status", "hermes_list_peers"]);
    let relay = Connector::start(folder.home.clone(), Settings::for_tests())
        .await
        .unwrap();
    assert_eq!(relay.role(), Role::Relay);
    assert_eq!(relay.tools(), leader.tools());
    relay.shutdown().await;
    leader.shutdown().await;
}
