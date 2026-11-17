//! `await-reply`, from the command to the leader and the hub (decision 0048;
//! `docs/11-conector.md`, section 6): the real hub, real connectors, the fake local API of
//! Hermes of `support.rs`, and the clients of the local socket.
//!
//! The times are short (`with_await_times`): a minute of waiting is a couple of seconds, the poll
//! 200 ms, the grace check 1.5 s. Where a test needs the notice of the hub to come before the
//! poll, the poll is long.

use std::ffi::OsString;
use std::time::Duration;

use arauto_connector::cli;
use arauto_connector::testing::Event;
use arauto_connector::wake::WakeTarget;
use arauto_connector::{Awaited, Connector, Settings, await_reply, reply};
use arauto_platform::LocalStream;
use arauto_protocol::message::CallOutcome;
use arauto_protocol::names::HermesName;
use arauto_protocol::notice::{Notice, NoticeCount};
use arauto_testkit::hub::fixture::{Fixture, settings as hub_settings};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

use crate::support::{FakeApi, Folder, PATIENCE, Watch, connected, eventually, send, settings};

pub(crate) const KEY: &str = "CANARY-key-of-the-local-api-77e1";
const CANARY: &str = "CANARY-text-of-the-answer-9c41e7";

/// A minute of 2 s, a poll of 200 ms, a grace check of 1.5 s, a retry of 100 ms.
const FAST: (i64, i64, i64, i64) = (2_000, 200, 1_500, 100);
/// The same, with a poll that never comes before the notice of the hub.
const NOTICE_FIRST: (i64, i64, i64, i64) = (2_000, 5_000, 1_500, 100);
/// A long minute, for the tests that hold many waits.
const LONG: (i64, i64, i64, i64) = (6_000, 5_000, 1_500, 100);

pub(crate) fn settings_with(
    times: (i64, i64, i64, i64),
    target: Option<WakeTarget>,
) -> (Settings, Watch) {
    let (settings, watch, _) = settings();
    let settings = settings
        .with_await_times(times.0, times.1, times.2, times.3)
        .waking(target);
    (settings, watch)
}

/// Two Hermes: the asking `a`, which wakes through the fake local API, and `b`, linked to `a`
/// with or without the wake mark.
pub(crate) struct Duo {
    pub(crate) api: FakeApi,
    pub(crate) a: Connector,
    pub(crate) a_name: String,
    pub(crate) a_folder: Folder,
    pub(crate) a_watch: Watch,
    /// What the events of `a` said before its last restart.
    carried: Vec<Event>,
    pub(crate) b: Connector,
    pub(crate) b_name: String,
    _b_folder: Folder,
    pub(crate) times: (i64, i64, i64, i64),
}

impl Duo {
    pub(crate) async fn new(
        fixture: &Fixture,
        wake_mark: bool,
        times: (i64, i64, i64, i64),
    ) -> Self {
        let api = FakeApi::start().await;
        let target = WakeTarget::new(&api.url, KEY).unwrap();
        let (a_settings, mut a_watch) = settings_with(times, Some(target));
        let (a_name, a_folder, a) = connected(fixture, a_settings, &mut a_watch).await;
        let (b_settings, mut b_watch) = settings_with(times, None);
        let (b_name, b_folder, b) = connected(fixture, b_settings, &mut b_watch).await;
        let now = fixture.now();
        fixture
            .admin
            .link_add(&a_name, &b_name, false, false, now)
            .await
            .unwrap();
        fixture
            .admin
            .link_add(&b_name, &a_name, wake_mark, false, now)
            .await
            .unwrap();
        Self {
            api,
            a,
            a_name,
            a_folder,
            a_watch,
            carried: Vec::new(),
            b,
            b_name,
            _b_folder: b_folder,
            times,
        }
    }

    /// The settings of the command run in the folder of `a`.
    pub(crate) fn client_settings(&self) -> Settings {
        settings_with(self.times, None).0
    }

    /// `a` asks `b` something: the thread and the whole result.
    pub(crate) async fn ask(&self) -> (Uuid, Map<String, Value>) {
        let sent = send(&self.a, &self.b_name, "a question").await;
        let thread = reply::parse_thread(sent["thread"].as_str().unwrap()).unwrap();
        (thread, sent)
    }

    /// `b` answers in `thread` with `text`.
    pub(crate) async fn answer(&self, thread: Uuid, text: &str) {
        let Value::Object(args) = json!({"thread": thread.to_string(), "text": text}) else {
            unreachable!()
        };
        match self.b.call("hermes_send_message", args).await {
            CallOutcome::Ok(_) => {}
            CallOutcome::Refused { code, .. } => panic!("the answer was refused with {code}"),
        }
    }

    /// The unread notes of `thread`, for `a`.
    pub(crate) async fn unread(&self, thread: Uuid) -> u64 {
        let CallOutcome::Ok(listed) = self.a.call("hermes_list_threads", Map::new()).await else {
            panic!("the listing was refused");
        };
        listed.as_map()["threads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["thread"] == json!(thread.to_string()))
            .map_or(0, |row| row["unread"].as_u64().unwrap())
    }

    /// `a` reads `thread`.
    pub(crate) async fn read(&self, thread: Uuid) {
        let Value::Object(args) = json!({"thread": thread.to_string()}) else {
            unreachable!()
        };
        assert!(matches!(
            self.a.call("hermes_read_thread", args).await,
            CallOutcome::Ok(_)
        ));
    }

    /// The command, as a task.
    pub(crate) fn wait(&self, thread: Uuid, minutes: u32) -> tokio::task::JoinHandle<Awaited> {
        let home = self.a_folder.home.clone();
        let settings = self.client_settings();
        tokio::spawn(async move { await_reply(&home, thread, minutes, &settings).await })
    }
}

impl Duo {
    /// Every event of `a`, from before and after its restarts.
    pub(crate) fn events(&mut self) -> Vec<Event> {
        let mut all = self.carried.clone();
        all.extend(self.a_watch.drain().iter().cloned());
        all
    }

    /// The leader of `a` restarts, as when a `join` asks it: it ends, and a new one serves the
    /// same folder, with a registry of its own.
    pub(crate) async fn restarted(mut self) -> Duo {
        let mut client = Client::connect(&self.a_folder).await;
        client.send(r#"{"v":1,"op":"restart","id":4}"#).await;
        assert_eq!(
            client.line_within(PATIENCE).await.unwrap(),
            json!({"v":1,"id":4,"ok":true})
        );
        tokio::time::timeout(PATIENCE, self.a.finished())
            .await
            .expect("the leader ends");
        let mut carried = self.events();
        let Duo {
            api,
            a,
            a_name,
            a_folder,
            b,
            b_name,
            _b_folder,
            times,
            ..
        } = self;
        a.shutdown().await;
        let target = WakeTarget::new(&api.url, KEY).unwrap();
        let (fresh, mut watch) = settings_with(times, Some(target));
        let a = Connector::start(a_folder.home.clone(), fresh)
            .await
            .unwrap();
        watch
            .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
            .await;
        carried.extend(watch.drain().iter().cloned());
        Duo {
            api,
            a,
            a_name,
            a_folder,
            a_watch: watch,
            carried: carried
                .into_iter()
                .filter(|event| !matches!(event, Event::Serving { .. } | Event::Role { .. }))
                .collect(),
            b,
            b_name,
            _b_folder,
            times,
        }
    }
}

/// The states of the notices of the hub.
pub(crate) async fn notices(fixture: &Fixture) -> Vec<String> {
    sqlx::query_scalar("SELECT state FROM notice ORDER BY created_at, id")
        .fetch_all(&fixture.pool)
        .await
        .unwrap()
}

pub(crate) async fn notice_ids(fixture: &Fixture) -> Vec<String> {
    sqlx::query_scalar("SELECT id::text FROM notice ORDER BY created_at, id")
        .fetch_all(&fixture.pool)
        .await
        .unwrap()
}

/// A client of the local socket, line by line.
pub(crate) struct Client {
    lines: tokio::io::Lines<BufReader<tokio::io::ReadHalf<LocalStream>>>,
    writer: tokio::io::WriteHalf<LocalStream>,
    /// Every raw line it received.
    pub(crate) raw: Vec<String>,
}

impl Client {
    pub(crate) async fn connect(folder: &Folder) -> Self {
        let stream = LocalStream::connect(folder.home.root()).await.unwrap();
        let (reader, writer) = tokio::io::split(stream);
        Self {
            lines: BufReader::new(reader).lines(),
            writer,
            raw: Vec::new(),
        }
    }

    pub(crate) async fn send(&mut self, line: &str) {
        self.writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .unwrap();
        self.writer.flush().await.unwrap();
    }

    pub(crate) async fn await_thread(&mut self, id: u64, thread: &str, minutes: u64) {
        self.send(
            &json!({"v":1,"op":"await","id":id,"thread":thread,"minutes":minutes}).to_string(),
        )
        .await;
    }

    /// The next line, within `within`; `None` when nothing came.
    pub(crate) async fn line_within(&mut self, within: Duration) -> Option<Value> {
        match tokio::time::timeout(within, self.lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                self.raw.push(line.clone());
                Some(serde_json::from_str(&line).unwrap())
            }
            _ => None,
        }
    }
}

async fn alone(fixture: &Fixture) -> (String, Folder, Connector, Watch) {
    let (settings, mut watch) = settings_with(FAST, None);
    let (name, folder, connector) = connected(fixture, settings, &mut watch).await;
    (name, folder, connector, watch)
}

// ---------------------------------------------------------------------------------------------
// What the output carries (rule 1)
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_answer_of_a_waiting_hermes_is_announced_without_its_text() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let (thread, _) = duo.ask().await;

    let waiting = duo.wait(thread, 3);
    let mut raw = Client::connect(&duo.a_folder).await;
    raw.await_thread(1, &thread.to_string(), 3).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    duo.answer(thread, CANARY).await;

    let awaited = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        awaited,
        Awaited::Arrived {
            thread,
            peer: HermesName::parse(&duo.b_name).unwrap()
        }
    );
    assert!(!awaited.text().contains(CANARY));
    assert!(awaited.text().contains(&duo.b_name));
    let line = raw.line_within(PATIENCE).await.unwrap();
    assert_eq!(line["arrived"], json!(true));
    assert_eq!(line["peer"], json!(duo.b_name));
    assert_eq!(line["thread"], json!(thread.to_string()));
    for text in &raw.raw {
        assert!(!text.contains(CANARY), "{text}");
    }
    // The waiter never read the thread: the note is still unread for the asking Hermes.
    assert_eq!(duo.unread(thread).await, 1);
}

// ---------------------------------------------------------------------------------------------
// How the wait ends (rule 3, and the notice path)
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_on_a_link_without_the_wake_mark_ends_by_polling_and_wakes_nobody() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let (thread, _) = duo.ask().await;
    let waiting = duo.wait(thread, 5);
    tokio::time::sleep(Duration::from_millis(500)).await;
    duo.answer(thread, "an answer").await;
    let awaited = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(awaited, Awaited::Arrived { .. }), "{awaited:?}");
    assert_eq!(duo.api.asked().len(), 0, "at the moment of the arrival");
    // The grace time, two polls and a second more: still nothing.
    tokio::time::sleep(Duration::from_millis(1_500 + 400 + 1_000)).await;
    assert_eq!(duo.api.asked().len(), 0, "after the grace check");
    assert!(
        notices(&fixture).await.is_empty(),
        "no link to wake, no notice"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reply_that_wakes_ends_the_wait_and_the_local_api_is_not_called() {
    let fixture = Fixture::new(hub_settings()).await;
    let mut duo = Duo::new(&fixture, true, NOTICE_FIRST).await;
    let (thread, _) = duo.ask().await;
    let waiting = duo.wait(thread, 5);
    tokio::time::sleep(Duration::from_millis(500)).await;
    duo.answer(thread, "an answer").await;
    let awaited = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(awaited, Awaited::Arrived { .. }), "{awaited:?}");
    eventually("the hub has the notice as confirmed", PATIENCE, || async {
        notices(&fixture).await == ["confirmed"]
    })
    .await;
    duo.a_watch
        .until(PATIENCE, |event| {
            matches!(event, Event::WakeSuppressed { .. })
        })
        .await;
    assert_eq!(duo.api.asked().len(), 0);
    // The Hermes reads the answer, and the grace check finds nothing to wake.
    duo.read(thread).await;
    tokio::time::sleep(Duration::from_millis(1_500 + 1_000)).await;
    assert_eq!(duo.api.asked().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_still_unread_after_the_grace_time_wakes_through_the_local_api_once() {
    let fixture = Fixture::new(hub_settings()).await;
    let mut duo = Duo::new(&fixture, true, NOTICE_FIRST).await;
    let (thread, _) = duo.ask().await;
    let waiting = duo.wait(thread, 5);
    tokio::time::sleep(Duration::from_millis(500)).await;
    duo.answer(thread, "an answer").await;
    let awaited = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(awaited, Awaited::Arrived { .. }), "{awaited:?}");
    assert_eq!(duo.api.asked().len(), 0, "not before the grace time");

    eventually("the grace check wakes the Hermes", PATIENCE, || async {
        !duo.api.asked().is_empty()
    })
    .await;
    duo.a_watch
        .until(PATIENCE, |event| matches!(event, Event::GraceWake { .. }))
        .await;
    // Once, and never again.
    tokio::time::sleep(Duration::from_millis(1_500 + 1_000)).await;
    let asked = duo.api.asked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    let (head, body) = &asked[0];
    let ids = notice_ids(&fixture).await;
    assert_eq!(ids.len(), 1);
    assert!(
        head.contains(&format!("Idempotency-Key: {}", ids[0])),
        "{head}"
    );
    let body: Value = serde_json::from_str(body).unwrap();
    let expected = Notice::Peer {
        from: HermesName::parse(&duo.b_name).unwrap(),
        count: NoticeCount::new(1).unwrap(),
        follow: false,
    }
    .phrase();
    assert_eq!(body["input"], json!(expected));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_note_from_another_peer_while_waiting_still_wakes_through_the_local_api() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, true, NOTICE_FIRST).await;
    let (thread, _) = duo.ask().await;
    let (c_settings, mut c_watch) = settings_with(FAST, None);
    let (c_name, _c_folder, c) = connected(&fixture, c_settings, &mut c_watch).await;
    fixture
        .admin
        .link_add(&c_name, &duo.a_name, true, false, fixture.now())
        .await
        .unwrap();

    let waiting = duo.wait(thread, 5);
    tokio::time::sleep(Duration::from_millis(500)).await;
    send(&c, &duo.a_name, "hello from a third Hermes").await;
    eventually("the local API is asked for a run", PATIENCE, || async {
        !duo.api.asked().is_empty()
    })
    .await;
    let asked = duo.api.asked();
    assert_eq!(asked.len(), 1);
    let body: Value = serde_json::from_str(&asked[0].1).unwrap();
    assert!(
        body["input"]
            .as_str()
            .unwrap()
            .contains(&format!("from Hermes agent \"{c_name}\""))
    );
    // The wait goes on: nobody answered it.
    assert!(!waiting.is_finished());
    waiting.abort();
}

// ---------------------------------------------------------------------------------------------
// The places (rule 5)
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_ninth_wait_is_busy_and_a_closed_connection_frees_its_place() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, LONG).await;
    let (thread, _) = duo.ask().await;
    let mut clients = Vec::new();
    for id in 1..=8u64 {
        let mut client = Client::connect(&duo.a_folder).await;
        client.await_thread(id, &thread.to_string(), 1).await;
        clients.push(client);
    }
    tokio::time::sleep(Duration::from_millis(1_000)).await;
    let mut ninth = Client::connect(&duo.a_folder).await;
    ninth.await_thread(9, &thread.to_string(), 1).await;
    let busy = ninth.line_within(PATIENCE).await.unwrap();
    assert_eq!(busy, json!({"v":1,"id":9,"ok":false,"error":"busy"}));

    // A connection that closes frees its place.
    drop(clients.remove(0));
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut tenth = Client::connect(&duo.a_folder).await;
    tenth.await_thread(10, &thread.to_string(), 1).await;
    assert!(
        tenth
            .line_within(Duration::from_millis(1_000))
            .await
            .is_none(),
        "the place was free: the wait is held"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_wait_on_one_connection_is_busy() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, LONG).await;
    let (thread, _) = duo.ask().await;
    let mut client = Client::connect(&duo.a_folder).await;
    client.await_thread(1, &thread.to_string(), 1).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    client.await_thread(2, &thread.to_string(), 1).await;
    let busy = client.line_within(PATIENCE).await.unwrap();
    assert_eq!(busy, json!({"v":1,"id":2,"ok":false,"error":"busy"}));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_in_another_thread_of_the_same_peer_ends_the_wait_with_that_thread() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let (first, _) = duo.ask().await;
    let (second, _) = duo.ask().await;
    assert_ne!(first, second);
    let waiting = duo.wait(first, 3);
    tokio::time::sleep(Duration::from_millis(500)).await;
    duo.answer(second, "an answer in the other thread").await;
    let awaited = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        awaited,
        Awaited::Arrived {
            thread: second,
            peer: HermesName::parse(&duo.b_name).unwrap()
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hostile_or_unknown_thread_is_refused_on_the_local_socket() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let mut client = Client::connect(&duo.a_folder).await;
    for (id, thread) in [(1u64, "$(id)"), (2, "00000000000000000000000000000000")] {
        client.await_thread(id, thread, 1).await;
        let line = client.line_within(PATIENCE).await.unwrap();
        assert_eq!(line, json!({"v":1,"id":id,"ok":false,"error":"invalid"}));
    }
    client
        .await_thread(3, "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d", 1)
        .await;
    let line = client.line_within(PATIENCE).await.unwrap();
    assert_eq!(line, json!({"v":1,"id":3,"ok":false,"code":"AR-708"}));
}

// ---------------------------------------------------------------------------------------------
// The field
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_successful_send_carries_the_field_with_its_thread() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let (thread, sent) = duo.ask().await;
    assert_eq!(
        sent[reply::FIELD],
        json!(reply::to_get_the_reply(thread)),
        "{sent:?}"
    );
    assert!(
        sent[reply::FIELD]
            .as_str()
            .unwrap()
            .contains(&format!("await-reply {thread}"))
    );
    // A refusal is not touched: no field.
    let Value::Object(args) = json!({"to": "nobody-here", "text": "x"}) else {
        unreachable!()
    };
    assert!(matches!(
        duo.a.call("hermes_send_message", args).await,
        CallOutcome::Refused { .. }
    ));
}

// ---------------------------------------------------------------------------------------------
// A restart of the leader
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_goes_on_across_a_restart_of_the_leader() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, LONG).await;
    let (thread, _) = duo.ask().await;
    // A fresh leader will serve the same folder: it must not share the registry of the old one.
    let waiting = {
        let home = duo.a_folder.home.clone();
        let settings = settings_with(LONG, None).0;
        tokio::spawn(async move { await_reply(&home, thread, 2, &settings).await })
    };
    tokio::time::sleep(Duration::from_millis(700)).await;

    let mut client = Client::connect(&duo.a_folder).await;
    client.send(r#"{"v":1,"op":"restart","id":4}"#).await;
    assert_eq!(
        client.line_within(PATIENCE).await.unwrap(),
        json!({"v":1,"id":4,"ok":true})
    );
    tokio::time::timeout(PATIENCE, duo.a.finished())
        .await
        .expect("the leader ends");
    let Duo {
        a,
        a_folder,
        b,
        b_name,
        _b_folder,
        ..
    } = duo;
    a.shutdown().await;
    assert!(
        !waiting.is_finished(),
        "the client waits for the new leader"
    );

    let (fresh, mut watch) = settings_with(LONG, None);
    let a = Connector::start(a_folder.home.clone(), fresh)
        .await
        .unwrap();
    watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    // The answer comes after the restart: the wait registered again and sees it.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let Value::Object(args) = json!({"thread": thread.to_string(), "text": "after the restart"})
    else {
        unreachable!()
    };
    assert!(matches!(
        b.call("hermes_send_message", args).await,
        CallOutcome::Ok(_)
    ));
    let awaited = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        awaited,
        Awaited::Arrived {
            thread,
            peer: HermesName::parse(&b_name).unwrap()
        }
    );
    a.shutdown().await;
    drop(a_folder);
}

// ---------------------------------------------------------------------------------------------
// The command: exit codes
// ---------------------------------------------------------------------------------------------

fn arguments(folder: &Folder, rest: &[&str]) -> Vec<OsString> {
    let mut all = vec![
        OsString::from("arauto-connector"),
        OsString::from("--home"),
        folder.home.root().as_os_str().to_owned(),
    ];
    all.extend(rest.iter().map(OsString::from));
    all
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_0_when_the_answer_arrives() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let (thread, _) = duo.ask().await;
    let thread_text = thread.to_string();
    let args = arguments(
        &duo.a_folder,
        &["await-reply", &thread_text, "--minutes", "3"],
    );
    let running = tokio::spawn(cli::run_with_settings(args, duo.client_settings()));
    tokio::time::sleep(Duration::from_millis(500)).await;
    duo.answer(thread, "an answer").await;
    let exit = tokio::time::timeout(PATIENCE, running)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.code(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_11_when_no_answer_arrives_in_time() {
    let fixture = Fixture::new(hub_settings()).await;
    // A minute of a second.
    let duo = Duo::new(&fixture, false, (1_000, 200, 1_500, 100)).await;
    let (thread, _) = duo.ask().await;
    let thread_text = thread.to_string();
    let args = arguments(
        &duo.a_folder,
        &["await-reply", &thread_text, "--minutes", "1"],
    );
    let exit = tokio::time::timeout(
        PATIENCE,
        cli::run_with_settings(args, duo.client_settings()),
    )
    .await
    .unwrap();
    assert_eq!(exit.code(), 11);
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_12_for_an_unknown_thread() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, FAST).await;
    let unknown = Uuid::new_v4().to_string();
    let args = arguments(&duo.a_folder, &["await-reply", &unknown]);
    let exit = tokio::time::timeout(
        PATIENCE,
        cli::run_with_settings(args, duo.client_settings()),
    )
    .await
    .unwrap();
    assert_eq!(exit.code(), 12);
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_12_when_eight_already_wait() {
    let fixture = Fixture::new(hub_settings()).await;
    let duo = Duo::new(&fixture, false, LONG).await;
    let (thread, _) = duo.ask().await;
    let mut clients = Vec::new();
    for id in 1..=8u64 {
        let mut client = Client::connect(&duo.a_folder).await;
        client.await_thread(id, &thread.to_string(), 1).await;
        clients.push(client);
    }
    tokio::time::sleep(Duration::from_millis(1_000)).await;
    let thread_text = thread.to_string();
    let args = arguments(&duo.a_folder, &["await-reply", &thread_text]);
    let exit = tokio::time::timeout(
        PATIENCE,
        cli::run_with_settings(args, duo.client_settings()),
    )
    .await
    .unwrap();
    assert_eq!(exit.code(), 12);
    drop(clients);
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_12_with_the_code_when_the_listing_is_refused() {
    let fixture = Fixture::new(hub_settings()).await;
    // A Hermes with no link at all: the tools of peers are refused with AR-701.
    let (_name, folder, _connector, _watch) = alone(&fixture).await;
    let thread = Uuid::new_v4().to_string();
    let args = arguments(&folder, &["await-reply", &thread]);
    let settings = settings_with(FAST, None).0;
    let exit = tokio::time::timeout(PATIENCE, cli::run_with_settings(args, settings))
        .await
        .unwrap();
    assert_eq!(exit.code(), 12);
    // And the text says which code.
    let home = folder.home.clone();
    let awaited = await_reply(&home, Uuid::new_v4(), 1, &settings_with(FAST, None).0).await;
    assert_eq!(
        awaited,
        Awaited::Refused(arauto_protocol::errors::ErrorCode::Ar701)
    );
    assert!(awaited.text().starts_with("Arauto: cannot wait: AR-701 "));
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_9_when_no_leader_runs() {
    let folder = Folder::new();
    folder.home.prepare().unwrap();
    let thread = Uuid::new_v4().to_string();
    let args = arguments(&folder, &["await-reply", &thread, "--minutes", "1"]);
    let settings = Settings::for_tests().with_await_times(300, 100, 100, 50);
    let exit = tokio::time::timeout(PATIENCE, cli::run_with_settings(args, settings))
        .await
        .unwrap();
    assert_eq!(exit.code(), 9);
}

#[tokio::test(flavor = "multi_thread")]
async fn await_reply_exits_2_for_a_thread_that_is_not_a_uuid() {
    let folder = Folder::new();
    for bad in ["not-a-uuid", "$(id)", "CANARY-thread-4d2a"] {
        let args = arguments(&folder, &["await-reply", bad]);
        let exit = cli::run_with_settings(args, Settings::for_tests()).await;
        assert_eq!(exit.code(), 2, "{bad}");
    }
}
