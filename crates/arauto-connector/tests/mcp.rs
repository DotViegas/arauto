//! The server of tools of the program itself, over its standard input and output, as Hermes
//! talks to it (`docs/09-contrato-mcp.md`; `docs/15-mvp.md`, section 4), without a hub:
//!
//! - every line out of its form gets an error, with `null` as the identifier when it could not
//!   be read, and the server goes on; a notification never gets an answer;
//! - the version of MCP is negotiated, and the tools are the texts of the catalog;
//! - with no hub, the list is the one of the last session and the calls get AR-203, the same
//!   through the leader and through a copy that forwards;
//! - the standard output carries only MCP, and the standard error repeats nothing that came;
//! - closing the input ends the program, in order, in less than 2 seconds.
//!
//! A binary of its own: it runs processes and waits for them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::tools;
use arauto_testkit::hermes::{CLOSE_GRACE, FakeHermes, McpCopy, built_example};
use serde_json::{Value, json};

use crate::common::{Folder, PROGRAM, ended_in_order, eventually, leader_process};

const PATIENCE: Duration = Duration::from_secs(20);

/// A copy of `mcp` of the folder `home`, as Hermes starts it.
fn copy_of(hermes: &FakeHermes, home: &Path) -> McpCopy {
    let args: Vec<OsString> = vec!["mcp".into(), "--home".into(), home.into()];
    hermes.start_command(Path::new(PROGRAM), &args)
}

fn fake_hermes() -> FakeHermes {
    FakeHermes::new(&built_example("fake_hermes"))
}

fn error_of(answer: &Value) -> (Value, i64, String) {
    (
        answer["id"].clone(),
        answer["error"]["code"].as_i64().unwrap(),
        answer["error"]["message"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn every_line_out_of_its_form_gets_an_error_and_the_server_goes_on() {
    let hermes = fake_hermes();
    let folder = Folder::new();
    let mut copy = copy_of(&hermes, &folder.home);
    let deep = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{"x":{}{}}}}}"#,
        "[".repeat(100),
        "]".repeat(100)
    );
    let many = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{"x":[{}0]}}}}"#,
        "0,".repeat(100_001)
    );
    let cases: Vec<(String, Value, i64, &str)> = vec![
        (
            "CANARY not json".into(),
            Value::Null,
            -32_700,
            "Parse error",
        ),
        (
            r#"{"jsonrpc":"2.0","id":1,"method":"ping","method":"CANARY"}"#.into(),
            Value::Null,
            -32_700,
            "Parse error: a key is repeated",
        ),
        (
            deep,
            Value::Null,
            -32_700,
            "Parse error: the message is nested too deep",
        ),
        (
            many,
            Value::Null,
            -32_700,
            "Parse error: the message has too many values",
        ),
        (
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#.into(),
            Value::Null,
            -32_600,
            "Invalid Request: batches are not supported",
        ),
        (
            "[]".into(),
            Value::Null,
            -32_600,
            "Invalid Request: batches are not supported",
        ),
        (
            r#""CANARY""#.into(),
            Value::Null,
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":{"CANARY":1},"method":"ping"}"#.into(),
            Value::Null,
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#.into(),
            Value::Null,
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"1.0","id":2,"method":"ping"}"#.into(),
            json!(2),
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":3}"#.into(),
            json!(3),
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#.into(),
            json!(4),
            -32_600,
            "Invalid Request: the server is not initialized",
        ),
        (
            r#"{"jsonrpc":"2.0","id":5,"method":"server/discover","params":{}}"#.into(),
            json!(5),
            -32_601,
            "Method not found",
        ),
        (
            r#"{"jsonrpc":"2.0","id":6,"method":"CANARY/method"}"#.into(),
            json!(6),
            -32_601,
            "Method not found",
        ),
        (
            r#"{"jsonrpc":"2.0","id":7,"method":"initialize","params":{"protocolVersion":7}}"#
                .into(),
            json!(7),
            -32_602,
            "Invalid params",
        ),
    ];
    for (line, id, code, message) in &cases {
        copy.send_line(line).await;
        let answer = copy.next_message(PATIENCE).await.unwrap();
        assert_eq!(
            error_of(&answer),
            (id.clone(), *code, (*message).to_owned()),
            "{}",
            &line[..line.len().min(60)]
        );
        assert!(!answer.to_string().contains("CANARY"), "{answer}");
    }
    // A line far above the ceiling: read to its end, answered as too large, and the server
    // goes on with the next line.
    let pad = "x".repeat(7 * 1024 * 1024);
    copy.send_line(&format!(
        r#"{{"jsonrpc":"2.0","id":8,"method":"ping","params":{{"pad":"{pad}"}}}}"#
    ))
    .await;
    let answer = copy.next_message(PATIENCE).await.unwrap();
    assert_eq!(
        error_of(&answer),
        (
            Value::Null,
            -32_700,
            "Parse error: the message is too large".to_owned()
        )
    );
    // Notifications, in their form or not, and an answer of the client: nothing comes back.
    for line in [
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","method":"CANARY/notification","params":{"x":1}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"arauto_status"}}"#,
        r#"{"jsonrpc":"9","method":"x","params":5}"#,
        r#"{"jsonrpc":"2.0","id":99,"result":{}}"#,
        "",
    ] {
        copy.send_line(line).await;
    }
    copy.send_line(r#"{"jsonrpc":"2.0","id":"after","method":"ping"}"#)
        .await;
    assert_eq!(
        copy.next_message(PATIENCE).await.unwrap(),
        json!({"jsonrpc":"2.0","id":"after","result":{}})
    );
    // In its form, it works: the identifier comes back as it was.
    let result = copy.initialize().await;
    assert_eq!(result["protocolVersion"], "2025-11-25");
    let answer = copy
        .request("initialize", json!({"protocolVersion": "2025-11-25"}))
        .await;
    assert_eq!(
        error_of(&answer).2,
        "Invalid Request: the server is already initialized"
    );
    let answer = copy
        .request(
            "tools/call",
            json!({"name": "CANARY_tool", "arguments": {}}),
        )
        .await;
    let (_, code, message) = error_of(&answer);
    assert_eq!(
        (code, message.as_str()),
        (-32_602, "Invalid params: unknown tool")
    );
    assert!(!answer.to_string().contains("CANARY"));
    let result = copy.call("arauto_status", json!({"CANARY": 1})).await;
    assert_eq!(
        result,
        json!({"content": [{"type": "text", "text": ErrorCode::Ar711.phrase(None)}], "isError": true})
    );
    for id in [json!(-5), json!("text"), json!(u64::MAX)] {
        copy.send(&json!({"jsonrpc": "2.0", "id": id, "method": "ping"}))
            .await;
        assert_eq!(copy.next_message(PATIENCE).await.unwrap()["id"], id);
    }
    assert!(copy.exited().is_none(), "the server is still there");
    let closed = copy.close(CLOSE_GRACE).await;
    ended_in_order(&closed);
    // The log repeats nothing that came.
    let stderr = String::from_utf8_lossy(&closed.stderr);
    assert!(!stderr.contains("CANARY"), "{stderr}");
    assert!(
        !stderr.contains("xxxxxxxx"),
        "the large line reached the log"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_version_is_negotiated_and_the_tools_are_the_texts_of_the_catalog() {
    let hermes = fake_hermes();
    for (asked, answered) in [
        ("2025-06-18", "2025-06-18"),
        ("2025-11-25", "2025-11-25"),
        ("2024-11-05", "2025-11-25"),
    ] {
        let folder = Folder::new();
        let mut copy = copy_of(&hermes, &folder.home);
        let answer = copy
            .request(
                "initialize",
                json!({"protocolVersion": asked, "capabilities": {}, "clientInfo": {"name": "x", "version": "1"}}),
            )
            .await;
        assert_eq!(
            answer["result"],
            json!({
                "protocolVersion": answered,
                "capabilities": {"tools": {"listChanged": true}},
                "serverInfo": {"name": "arauto-connector", "version": env!("CARGO_PKG_VERSION")},
            }),
            "{asked}"
        );
        copy.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        let listed = copy.request("tools/list", json!({})).await;
        let spec = tools::find("arauto_status").unwrap();
        assert_eq!(
            listed["result"],
            json!({"tools": [{
                "name": "arauto_status",
                "description": spec.description,
                "inputSchema": spec.input_schema(),
            }]})
        );
        ended_in_order(&copy.close(CLOSE_GRACE).await);
    }
}

/// With no hub, the list is the one of the last session, kept in `state.json`, and every call
/// gets AR-203: the leader and the copy that forwards to it answer the same.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_hub_the_list_is_the_last_one_and_the_calls_are_ar203() {
    let hermes = fake_hermes();
    let folder = Folder::new();
    std::fs::create_dir_all(&folder.home).unwrap();
    std::fs::write(
        folder.home.join("state.json"),
        br#"{"v":1,"tools":["arauto_status","hermes_list_peers"]}"#,
    )
    .unwrap();
    let mut first = copy_of(&hermes, &folder.home);
    first.initialize().await;
    let leader = first.id();
    eventually("the first copy leads", PATIENCE, || async {
        leader_process(&folder.home).await == leader
    })
    .await;
    let mut second = copy_of(&hermes, &folder.home);
    second.initialize().await;
    let refused = json!({
        "content": [{"type": "text", "text": ErrorCode::Ar203.phrase(None)}],
        "isError": true,
    });
    for copy in [&mut first, &mut second] {
        let names = copy.tools().await;
        assert_eq!(names, ["arauto_status", "hermes_list_peers"]);
        for tool in ["arauto_status", "hermes_list_peers"] {
            assert_eq!(copy.call(tool, json!({})).await, refused, "{tool}");
        }
    }
    assert_eq!(leader_process(&folder.home).await, leader);
    for copy in [first, second] {
        ended_in_order(&copy.close(CLOSE_GRACE).await);
    }
}
