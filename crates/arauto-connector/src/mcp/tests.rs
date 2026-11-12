//! The server of tools against a backend of the test: the conversation of MCP, line by line.
//!
//! Every line the server writes is read back as a message of JSON-RPC: the standard output
//! carries nothing else.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_protocol::errors::{ErrorCode, RetryAfter};
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::tools::{self, ToolName};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use super::{Backend, CALLS_IN_FLIGHT_MAX, Ended, LINE_MAX, serve};

const PATIENCE: Duration = Duration::from_secs(10);

/// How the backend answers a call.
enum Answer {
    Now(CallOutcome),
    Later(oneshot::Receiver<CallOutcome>),
}

/// A backend of the test: a list of tools that the test changes, the calls it received, and
/// the answers it was given for each tool.
struct Fake {
    tools: watch::Sender<Vec<ToolName>>,
    calls: Mutex<Vec<(String, Map<String, Value>)>>,
    answers: Mutex<HashMap<String, VecDeque<Answer>>>,
}

impl Fake {
    fn new(tools: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            tools: watch::Sender::new(names(tools)),
            calls: Mutex::new(Vec::new()),
            answers: Mutex::new(HashMap::new()),
        })
    }

    fn answer(&self, tool: &str, answer: Answer) {
        self.answers
            .lock()
            .unwrap()
            .entry(tool.to_owned())
            .or_default()
            .push_back(answer);
    }

    /// The next call of `tool` waits for the sender this returns.
    fn hold(&self, tool: &str) -> oneshot::Sender<CallOutcome> {
        let (release, held) = oneshot::channel();
        self.answer(tool, Answer::Later(held));
        release
    }

    fn calls(&self) -> Vec<(String, Map<String, Value>)> {
        self.calls.lock().unwrap().clone()
    }
}

fn names(tools: &[&str]) -> Vec<ToolName> {
    tools
        .iter()
        .map(|tool| ToolName::parse(tool).unwrap())
        .collect()
}

fn connected() -> CallOutcome {
    let Value::Object(content) = json!({"connected": true, "hermes": "h-1"}) else {
        unreachable!()
    };
    CallOutcome::Ok(Payload::new(content))
}

impl Backend for Fake {
    fn tools(&self) -> Vec<ToolName> {
        self.tools.borrow().clone()
    }

    fn watch_tools(&self) -> watch::Receiver<Vec<ToolName>> {
        self.tools.subscribe()
    }

    fn call(
        &self,
        tool: ToolName,
        args: Payload,
    ) -> impl std::future::Future<Output = CallOutcome> + Send {
        self.calls
            .lock()
            .unwrap()
            .push((tool.as_str().to_owned(), args.into_map()));
        let answer = self
            .answers
            .lock()
            .unwrap()
            .get_mut(tool.as_str())
            .and_then(VecDeque::pop_front);
        async move {
            match answer {
                None => connected(),
                Some(Answer::Now(outcome)) => outcome,
                Some(Answer::Later(held)) => held.await.unwrap_or_else(|_| connected()),
            }
        }
    }
}

/// The client side of a server running in a task.
struct Client {
    input: Option<DuplexStream>,
    output: tokio::io::Lines<BufReader<DuplexStream>>,
    server: JoinHandle<Ended>,
    stop: Option<oneshot::Sender<()>>,
    next: u64,
}

impl Client {
    fn start(fake: &Arc<Fake>) -> Self {
        let (input, server_input) = tokio::io::duplex(64 * 1024);
        let (server_output, output) = tokio::io::duplex(64 * 1024);
        let (stop, stopped) = oneshot::channel::<()>();
        let fake = Arc::clone(fake);
        let server = tokio::spawn(async move {
            let stop = async {
                let _ = stopped.await;
            };
            serve(&*fake, BufReader::new(server_input), server_output, stop).await
        });
        Self {
            input: Some(input),
            output: BufReader::new(output).lines(),
            server,
            stop: Some(stop),
            next: 0,
        }
    }

    async fn write(&mut self, bytes: &[u8]) {
        let input = self.input.as_mut().unwrap();
        input.write_all(bytes).await.unwrap();
        input.flush().await.unwrap();
    }

    async fn send_line(&mut self, text: &str) {
        self.write(format!("{text}\n").as_bytes()).await;
    }

    async fn send(&mut self, message: &Value) {
        self.send_line(&message.to_string()).await;
    }

    /// The next message the server writes. It must be a message of JSON-RPC 2.0, in one line
    /// that holds no other line break.
    async fn next(&mut self) -> Value {
        let line = tokio::time::timeout(PATIENCE, self.output.next_line())
            .await
            .expect("the server answered in time")
            .unwrap()
            .expect("the output is open");
        for separator in ['\r', '\u{85}', '\u{2028}', '\u{2029}'] {
            assert!(!line.contains(separator), "{line:?}");
        }
        let message: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
        message
    }

    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// Sends a request and returns the next message.
    async fn ask(&mut self, method: &str, params: Value) -> Value {
        let id = self.id();
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        let answer = self.next().await;
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    async fn initialize(&mut self) {
        let answer = self
            .ask(
                "initialize",
                json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}),
            )
            .await;
        assert_eq!(answer["result"]["protocolVersion"], "2025-11-25");
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
    }

    async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.ask("tools/call", json!({"name": tool, "arguments": arguments}))
            .await
    }

    /// Nothing was written but what answers the ping that follows.
    async fn nothing_before_a_ping(&mut self) {
        let id = self.id();
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": "ping"}))
            .await;
        let answer = self.next().await;
        assert_eq!(answer, json!({"jsonrpc": "2.0", "id": id, "result": {}}));
    }

    async fn end(mut self) -> Ended {
        drop(self.input.take());
        tokio::time::timeout(PATIENCE, self.server)
            .await
            .unwrap()
            .unwrap()
    }
}

fn error_of(answer: &Value) -> (i64, String) {
    (
        answer["error"]["code"].as_i64().unwrap(),
        answer["error"]["message"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn initialize_answers_the_version_asked_when_it_is_spoken_here() {
    for (asked, answered) in [
        ("2025-11-25", "2025-11-25"),
        ("2025-06-18", "2025-06-18"),
        ("2025-03-26", "2025-11-25"),
        ("2024-11-05", "2025-11-25"),
        ("2026-07-28", "2025-11-25"),
        ("not a version", "2025-11-25"),
    ] {
        let fake = Fake::new(&[]);
        let mut client = Client::start(&fake);
        let answer = client
            .ask(
                "initialize",
                json!({"protocolVersion": asked, "capabilities": {}}),
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
        assert_eq!(client.end().await, Ended::Input);
    }
}

#[tokio::test]
async fn before_initialize_only_ping_is_answered_and_initialize_comes_once() {
    let fake = Fake::new(&[]);
    let mut client = Client::start(&fake);
    for method in ["tools/list", "tools/call"] {
        let answer = client.ask(method, json!({"name": "arauto_status"})).await;
        assert_eq!(
            error_of(&answer),
            (
                -32_600,
                "Invalid Request: the server is not initialized".to_owned()
            )
        );
    }
    // A client of the versions without handshake probes, and falls back to `initialize`.
    let answer = client.ask("server/discover", json!({})).await;
    assert_eq!(error_of(&answer), (-32_601, "Method not found".to_owned()));
    let answer = client.ask("ping", json!({})).await;
    assert_eq!(answer["result"], json!({}));
    // An `initialize` out of its form changes nothing.
    for params in [json!({}), json!({"protocolVersion": 2025}), json!([1])] {
        let answer = client.ask("initialize", params).await;
        assert_eq!(error_of(&answer), (-32_602, "Invalid params".to_owned()));
    }
    let answer = client.ask("tools/list", json!({})).await;
    assert_eq!(error_of(&answer).0, -32_600);
    client.initialize().await;
    let answer = client
        .ask("initialize", json!({"protocolVersion": "2025-11-25"}))
        .await;
    assert_eq!(
        error_of(&answer),
        (
            -32_600,
            "Invalid Request: the server is already initialized".to_owned()
        )
    );
    assert!(fake.calls().is_empty());
}

/// A notification never gets an answer: known or not, in its form or not, before and after
/// `initialize`.
#[tokio::test]
async fn a_notification_never_gets_an_answer() {
    let fake = Fake::new(&[]);
    let mut client = Client::start(&fake);
    for initialized in [false, true] {
        if initialized {
            client.initialize().await;
        }
        for line in [
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":77}}"#,
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"arauto_status"}}"#,
            r#"{"jsonrpc":"2.0","method":"no/such/method"}"#,
            r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/roots/list_changed","params":5}"#,
            r#"{"jsonrpc":"1.0","method":"notifications/initialized"}"#,
            r#"{"method":"anything","params":[1,2]}"#,
        ] {
            client.send_line(line).await;
        }
        // Before the request `initialize`, the notification `initialize` initialized nothing:
        // the request still works, once.
        client.nothing_before_a_ping().await;
    }
    assert!(fake.calls().is_empty());
}

/// What is not a message gets an error, with the identifier when it could be read, and the
/// server goes on.
#[tokio::test]
async fn lines_out_of_form_get_an_error_and_the_server_goes_on() {
    let fake = Fake::new(&[]);
    let mut client = Client::start(&fake);
    let deep = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{"x":{}{}}}}}"#,
        "[".repeat(80),
        "]".repeat(80)
    );
    let many = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{"x":[{}0]}}}}"#,
        "0,".repeat(100_000)
    );
    for (line, id, code, message) in [
        ("not json", Value::Null, -32_700, "Parse error"),
        (
            r#"{"jsonrpc":"2.0","id":1,"method":"ping""#,
            Value::Null,
            -32_700,
            "Parse error",
        ),
        (
            r#"{"jsonrpc":"2.0","id":1,"method":"ping","id":2}"#,
            Value::Null,
            -32_700,
            "Parse error: a key is repeated",
        ),
        (
            deep.as_str(),
            Value::Null,
            -32_700,
            "Parse error: the message is nested too deep",
        ),
        (
            many.as_str(),
            Value::Null,
            -32_700,
            "Parse error: the message has too many values",
        ),
        (
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#,
            Value::Null,
            -32_600,
            "Invalid Request: batches are not supported",
        ),
        ("17", Value::Null, -32_600, "Invalid Request"),
        (
            r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
            Value::Null,
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":2.5,"method":"ping"}"#,
            Value::Null,
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"1.0","id":3,"method":"ping"}"#,
            json!(3),
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"id":"q","method":"ping"}"#,
            json!("q"),
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":4,"method":{"x":1}}"#,
            json!(4),
            -32_600,
            "Invalid Request",
        ),
        (
            r#"{"jsonrpc":"2.0","id":5,"method":"no/such/method"}"#,
            json!(5),
            -32_601,
            "Method not found",
        ),
    ] {
        client.send_line(line).await;
        let answer = client.next().await;
        assert_eq!(answer["id"], id, "{}", &line[..line.len().min(60)]);
        assert_eq!(
            error_of(&answer),
            (code, message.to_owned()),
            "{}",
            &line[..line.len().min(60)]
        );
        assert_eq!(answer.as_object().unwrap().len(), 3, "{answer}");
    }
    // A line of blanks, and an answer of the client: nothing.
    client.send_line("   \t").await;
    client
        .send_line(r#"{"jsonrpc":"2.0","id":9,"result":{}}"#)
        .await;
    client.nothing_before_a_ping().await;
    // A request in its form that is longer than the ceiling: dropped whole, and answered as
    // too large, without its identifier; the next line is read.
    let pad = "x".repeat(LINE_MAX);
    let large = format!(r#"{{"jsonrpc":"2.0","id":6,"method":"ping","params":{{"pad":"{pad}"}}}}"#);
    client.send_line(&large).await;
    let answer = client.next().await;
    assert_eq!(answer["id"], Value::Null);
    assert_eq!(
        error_of(&answer),
        (-32_700, "Parse error: the message is too large".to_owned())
    );
    // Just below the ceiling, it is read.
    let fits = format!(
        r#"{{"jsonrpc":"2.0","id":7,"method":"ping","params":{{"pad":"{}"}}}}"#,
        "x".repeat(LINE_MAX - 100)
    );
    client.send_line(&fits).await;
    assert_eq!(
        client.next().await,
        json!({"jsonrpc":"2.0","id":7,"result":{}})
    );
    client.nothing_before_a_ping().await;
    // Nothing of what came is repeated in the errors.
    client
        .send_line(r#"{"jsonrpc":"2.0","id":8,"method":"CANARY/method"}"#)
        .await;
    assert!(!client.next().await.to_string().contains("CANARY"));
}

#[tokio::test]
async fn identifiers_come_back_as_they_were_sent() {
    let fake = Fake::new(&[]);
    let mut client = Client::start(&fake);
    for id in [
        json!(0),
        json!(-12),
        json!(u64::MAX),
        json!("text-id"),
        json!(""),
    ] {
        client
            .send(&json!({"jsonrpc": "2.0", "id": id, "method": "ping"}))
            .await;
        assert_eq!(
            client.next().await,
            json!({"jsonrpc": "2.0", "id": id, "result": {}})
        );
    }
}

/// The list comes from the catalog compiled here: the texts the hub cannot change. Names the
/// connector does not know are left out, and `arauto_status` is always there.
#[tokio::test]
async fn the_list_of_tools_has_the_texts_of_the_catalog() {
    let fake = Fake::new(&[]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let answer = client.ask("tools/list", json!({})).await;
    let entry = |name: &str| {
        let spec = tools::find(name).unwrap();
        json!({"name": spec.name, "description": spec.description, "inputSchema": spec.input_schema()})
    };
    assert_eq!(answer["result"], json!({"tools": [entry("arauto_status")]}));

    fake.tools.send_replace(names(&[
        "hermes_read_thread",
        "whatsapp_send_text",
        "arauto_status",
        "hermes_list_peers",
    ]));
    // The change is told first.
    assert_eq!(
        client.next().await,
        json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})
    );
    let answer = client.ask("tools/list", json!({"cursor": null})).await;
    assert_eq!(
        answer["result"],
        json!({"tools": [entry("arauto_status"), entry("hermes_list_peers"), entry("hermes_read_thread")]})
    );
    let answer = client.ask("tools/list", json!({"cursor": "c"})).await;
    assert_eq!(
        error_of(&answer),
        (-32_602, "Invalid params: invalid cursor".to_owned())
    );
    let answer = client.ask("tools/list", json!([])).await;
    assert_eq!(error_of(&answer).0, -32_602);
}

#[tokio::test]
async fn a_call_that_ran_gives_the_structured_content_and_its_text() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let answer = client.call("arauto_status", json!({})).await;
    let result = &answer["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"],
        json!({"connected": true, "hermes": "h-1"})
    );
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    let text: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, result["structuredContent"]);
    // Without arguments, or with `null`, the call is the same.
    client
        .send(&json!({"jsonrpc": "2.0", "id": 50, "method": "tools/call", "params": {"name": "arauto_status"}}))
        .await;
    assert_eq!(client.next().await["result"]["isError"], false);
    client
        .send(&json!({"jsonrpc": "2.0", "id": 51, "method": "tools/call", "params": {"name": "arauto_status", "arguments": null}}))
        .await;
    assert_eq!(client.next().await["result"]["isError"], false);
    assert_eq!(
        fake.calls(),
        vec![
            ("arauto_status".to_owned(), Map::new()),
            ("arauto_status".to_owned(), Map::new()),
            ("arauto_status".to_owned(), Map::new())
        ]
    );
}

/// A refusal of the hub or of the connector is a result with `isError` and the fixed phrase
/// of its code, and nothing else.
#[tokio::test]
async fn a_refusal_is_a_result_with_the_phrase_of_the_catalog() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let wait = RetryAfter::new(30);
    for (code, retry_after) in [
        (ErrorCode::Ar203, None),
        (ErrorCode::Ar207, None),
        (ErrorCode::Ar502, None),
        (ErrorCode::Ar701, None),
        (ErrorCode::Ar710, None),
        (ErrorCode::Ar706, wait),
        (ErrorCode::Ar203, wait),
    ] {
        fake.answer(
            "arauto_status",
            Answer::Now(CallOutcome::Refused { code, retry_after }),
        );
        let answer = client.call("arauto_status", json!({})).await;
        assert_eq!(
            answer["result"],
            json!({
                "content": [{"type": "text", "text": code.phrase(retry_after)}],
                "isError": true,
            }),
            "{code}"
        );
    }
    let phrase = ErrorCode::Ar706.phrase(wait);
    assert!(phrase.contains("30 second(s)"), "{phrase}");
}

/// A tool out of the catalog is an error of the protocol that repeats nothing of what came,
/// and the backend never sees it.
#[tokio::test]
async fn a_tool_out_of_the_catalog_never_reaches_the_backend() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    for name in [
        "CANARY_tool",
        "whatsapp_send_text",
        "../../CANARY",
        "",
        "arauto_status ",
    ] {
        let answer = client.call(name, json!({})).await;
        assert_eq!(
            error_of(&answer),
            (-32_602, "Invalid params: unknown tool".to_owned()),
            "{name}"
        );
        assert!(!answer.to_string().contains("CANARY"), "{answer}");
    }
    for params in [
        json!({}),
        json!({"name": 5}),
        json!({"name": "arauto_status", "arguments": [1]}),
        json!({"name": "arauto_status", "arguments": "x"}),
        json!(["arauto_status"]),
    ] {
        let answer = client.ask("tools/call", params.clone()).await;
        assert_eq!(
            error_of(&answer),
            (-32_602, "Invalid params".to_owned()),
            "{params}"
        );
    }
    assert!(fake.calls().is_empty());
}

/// Arguments out of the schema of the catalog are refused here, with AR-711, before anything
/// leaves.
#[tokio::test]
async fn arguments_out_of_the_catalog_are_refused_here() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    for (tool, arguments) in [
        ("arauto_status", json!({"x": 1})),
        ("hermes_read_thread", json!({})),
        ("hermes_read_thread", json!({"thread": "../../etc"})),
        (
            "hermes_send_message",
            json!({"to": "Not A Name", "text": "hi"}),
        ),
        (
            "hermes_send_message",
            json!({"to": "peer", "text": "a".repeat(16_385)}),
        ),
    ] {
        let answer = client.call(tool, arguments).await;
        assert_eq!(
            answer["result"],
            json!({
                "content": [{"type": "text", "text": ErrorCode::Ar711.phrase(None)}],
                "isError": true,
            }),
            "{tool}"
        );
    }
    assert!(fake.calls().is_empty());
    // In the catalog and in its form: it goes to the backend, released or not.
    let answer = client
        .call(
            "hermes_read_thread",
            json!({"thread": "0b9c5f5e-6f0a-4c3e-9d54-1c2f3a4b5c6d"}),
        )
        .await;
    assert_eq!(answer["result"]["isError"], false);
    assert_eq!(fake.calls().len(), 1);
}

#[tokio::test]
async fn calls_run_at_the_same_time_and_are_answered_as_they_end() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let release = fake.hold("arauto_status");
    client
        .send(&json!({"jsonrpc": "2.0", "id": "slow", "method": "tools/call", "params": {"name": "arauto_status"}}))
        .await;
    client
        .send(&json!({"jsonrpc": "2.0", "id": "fast", "method": "tools/call", "params": {"name": "arauto_status"}}))
        .await;
    assert_eq!(client.next().await["id"], "fast");
    // A ping is answered while a call waits.
    client.nothing_before_a_ping().await;
    release
        .send(CallOutcome::Refused {
            code: ErrorCode::Ar207,
            retry_after: None,
        })
        .unwrap();
    let answer = client.next().await;
    assert_eq!(answer["id"], "slow");
    assert_eq!(answer["result"]["isError"], true);
}

#[tokio::test]
async fn a_cancelled_call_gets_no_answer() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let release = fake.hold("arauto_status");
    client
        .send(&json!({"jsonrpc": "2.0", "id": 30, "method": "tools/call", "params": {"name": "arauto_status"}}))
        .await;
    client
        .send(&json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 30, "reason": "x"}}))
        .await;
    client.nothing_before_a_ping().await;
    release.send(connected()).unwrap();
    client.nothing_before_a_ping().await;
    // The identifier is free again once the call ended.
    client
        .send(&json!({"jsonrpc": "2.0", "id": 30, "method": "tools/call", "params": {"name": "arauto_status"}}))
        .await;
    let answer = client.next().await;
    assert_eq!(answer["id"], 30);
    assert_eq!(answer["result"]["isError"], false);
    assert_eq!(fake.calls().len(), 2);
}

#[tokio::test]
async fn the_identifier_of_a_call_in_progress_is_refused() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let release = fake.hold("arauto_status");
    let call = json!({"jsonrpc": "2.0", "id": 40, "method": "tools/call", "params": {"name": "arauto_status"}});
    client.send(&call).await;
    client.send(&call).await;
    let answer = client.next().await;
    assert_eq!(answer["id"], 40);
    assert_eq!(
        error_of(&answer),
        (
            -32_600,
            "Invalid Request: the identifier is in use by a request in progress".to_owned()
        )
    );
    release.send(connected()).unwrap();
    let answer = client.next().await;
    assert_eq!(answer["id"], 40);
    assert_eq!(answer["result"]["isError"], false);
    assert_eq!(fake.calls().len(), 1);
}

#[tokio::test]
async fn above_the_ceiling_of_calls_in_progress_a_call_is_ar710() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let mut releases = Vec::new();
    for held in 0..CALLS_IN_FLIGHT_MAX {
        releases.push(fake.hold("arauto_status"));
        client
            .send(&json!({"jsonrpc": "2.0", "id": format!("held-{held}"), "method": "tools/call", "params": {"name": "arauto_status"}}))
            .await;
    }
    let answer = client
        .ask("tools/call", json!({"name": "arauto_status"}))
        .await;
    assert_eq!(
        answer["result"]["content"][0]["text"],
        ErrorCode::Ar710.phrase(None).as_ref()
    );
    assert_eq!(answer["result"]["isError"], true);
    assert_eq!(fake.calls().len(), CALLS_IN_FLIGHT_MAX);
    for release in releases {
        release.send(connected()).unwrap();
    }
    for _ in 0..CALLS_IN_FLIGHT_MAX {
        assert_eq!(client.next().await["result"]["isError"], false);
    }
}

/// The client is told of a change of the list once it said it is ready, and only of a change
/// it would see.
#[tokio::test]
async fn a_change_of_the_list_is_told_once_the_client_is_ready() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    fake.tools
        .send_replace(names(&["arauto_status", "hermes_list_peers"]));
    client.nothing_before_a_ping().await;
    let answer = client
        .ask("initialize", json!({"protocolVersion": "2025-06-18"}))
        .await;
    assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
    // Initialized, not ready: a change waits.
    fake.tools.send_replace(names(&["arauto_status"]));
    client.nothing_before_a_ping().await;
    client
        .send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .await;
    let told = json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"});
    assert_eq!(client.next().await, told);
    // The same list again, or a name this connector does not know: nothing to tell.
    fake.tools.send_replace(names(&["arauto_status"]));
    client.nothing_before_a_ping().await;
    fake.tools
        .send_replace(names(&["arauto_status", "whatsapp_read_chat"]));
    client.nothing_before_a_ping().await;
    fake.tools
        .send_replace(names(&["hermes_list_peers", "hermes_list_threads"]));
    assert_eq!(client.next().await, told);
    client.nothing_before_a_ping().await;
}

/// No line break inside a message: a text from the hub with the ones of Unicode is escaped.
#[tokio::test]
async fn a_text_with_line_breaks_stays_in_one_line() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    let text = "one\ntwo\u{2028}three\u{2029}four\u{85}five";
    let Value::Object(content) = json!({"hermes": text}) else {
        unreachable!()
    };
    fake.answer(
        "arauto_status",
        Answer::Now(CallOutcome::Ok(Payload::new(content))),
    );
    let answer = client.call("arauto_status", json!({})).await;
    assert_eq!(answer["result"]["structuredContent"]["hermes"], text);
    let shown: Value =
        serde_json::from_str(answer["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(shown["hermes"], text);
}

#[tokio::test]
async fn the_server_ends_with_the_input_and_with_the_stop() {
    let fake = Fake::new(&["arauto_status"]);
    let mut client = Client::start(&fake);
    client.initialize().await;
    // A call in progress does not hold the end.
    let _release = fake.hold("arauto_status");
    client
        .send(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "arauto_status"}}))
        .await;
    assert_eq!(client.end().await, Ended::Input);

    let mut client = Client::start(&fake);
    client.initialize().await;
    drop(client.stop.take());
    let ended = tokio::time::timeout(PATIENCE, &mut client.server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ended, Ended::Stopped);
}

/// A line without its end, when the input ends, is not a message.
#[tokio::test]
async fn a_last_line_without_its_end_is_not_read() {
    let fake = Fake::new(&[]);
    let mut client = Client::start(&fake);
    client
        .write(br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
        .await;
    drop(client.input.take());
    let nothing = tokio::time::timeout(PATIENCE, client.output.next_line())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(nothing, None);
    assert_eq!(client.end().await, Ended::Input);
}
