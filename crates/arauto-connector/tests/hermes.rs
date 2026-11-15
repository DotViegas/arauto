//! The program as Hermes runs it, end to end (`docs/11-conector.md`; `docs/15-mvp.md`, sections
//! 4 and 7): the fake Hermes of the testkit (its command records the registration, and it starts
//! the copies of the server of tools with a filtered environment), the real hub in this process,
//! and the program itself in processes of its own.
//!
//! - **The block:** `join` as a process prints the code; the admin approves it; Hermes starts
//!   three copies, one leader and two that forward; `arauto_status` through each one.
//! - **The leader dies:** the calls go on through the copy that takes its place; a call in
//!   flight at its death is AR-207.
//! - **The hub stops:** the list of the last session and AR-203, the same through each copy; the
//!   hub comes back, and the copies are told the list changed.
//! - **The commands:** `status` without secret; `leave` deletes the files and takes out the
//!   registration; `--home`, `ARAUTO_HOME` and the default folder reach the registration and the
//!   copies; a `join` with a new program replaces the one running, and every copy starts again
//!   on it; a refused registration is AR-601, and the same `join` again registers.
//! - **The canary:** the output and the standard error of every process and the files: no
//!   secret, and the code only in the output of the `join` that shows it.
//! - **The key of the local API:** the entry carries the reference to the setting of Hermes, the
//!   fake Hermes resolves it from its own secrets when it starts a copy, `status` and
//!   `arauto_status` say what the running connector holds, an older Hermes without `--env` is
//!   connected all the same, `register` saves the entry again without touching the connection,
//!   and the command of Hermes never gets the two variables of the key.
//!
//! A binary of its own: it runs processes and waits for them in seconds. Only a few tests run
//! at once, so that a busy machine does not stretch what they measure. They need
//! `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so. The program of the fake
//! command of Hermes is the example `fake_hermes`, which `cargo test` builds when no target is
//! chosen.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use arauto_connector::SERVER_NAME;
use arauto_connector::catalog::{Local, Told};
use arauto_hub::repo::approval::ApproveOutcome;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::limits::{DAY_MS, SECOND_MS};
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use arauto_protocol::secret::Secret;
use arauto_testkit::hermes::{CLOSE_GRACE, FakeHermes, McpCopy, Registered, built_example};
use arauto_testkit::hub::fixture::{Fixture, settings};
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::common::{
    Folder, PROGRAM, Ran, Secrets, ask, code_in, ended_in_order, eventually, files_of,
    invite_bytes, leader_process, only_messages, ran,
};

/// Patience for what takes seconds: a process that starts, a session, a copy that takes over.
const PATIENCE: Duration = Duration::from_secs(60);

/// Tests at once: each one runs a hub and several processes.
static AT_ONCE: Semaphore = Semaphore::const_new(3);

fn fake_hermes() -> FakeHermes {
    FakeHermes::new(&built_example("fake_hermes"))
}

/// The fixed path of the program in the folder `home`.
fn program_at(home: &Path) -> PathBuf {
    home.join(format!("arauto-connector{}", std::env::consts::EXE_SUFFIX))
}

fn devices(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(home.join("devices"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn only_device(home: &Path) -> String {
    let devices = devices(home);
    assert_eq!(devices.len(), 1, "{devices:?}");
    devices[0].clone()
}

fn device_file(home: &Path, device: &str, file: &str) -> PathBuf {
    home.join("devices").join(device).join(file)
}

fn public(home: &Path, device: &str) -> PublicKey {
    let bytes = std::fs::read(device_file(home, device, "identity.key")).unwrap();
    *StaticKeypair::from_private(Secret::from_slice(&bytes).unwrap()).public()
}

fn current(home: &Path) -> Option<String> {
    std::fs::read_to_string(home.join("current"))
        .ok()
        .map(|text| text.trim().to_owned())
}

/// `join`, run by the operator in the terminal of Hermes with `program`.
async fn join(
    hermes: &FakeHermes,
    program: &Path,
    fixture: &Fixture,
    invite: &Invite,
    home: &Path,
) -> Ran {
    let mut command = hermes.terminal(program);
    command
        .arg("join")
        .arg(fixture.channel_url())
        .arg(fixture.hub_key_text())
        .arg(invite.reveal().as_str())
        .arg("--home")
        .arg(home);
    ran(command).await
}

/// Another command of the program, with `--home`, in the terminal of Hermes.
async fn command(hermes: &FakeHermes, home: &Path, arguments: &[&str]) -> Ran {
    let mut command = hermes.terminal(Path::new(PROGRAM));
    command.args(arguments).arg("--home").arg(home);
    ran(command).await
}

async fn approve(fixture: &Fixture, name: &str, code: &str) {
    let outcome = fixture.approve(name, code).await;
    assert!(
        matches!(outcome, ApproveOutcome::Approved(_)),
        "{outcome:?}"
    );
}

/// A Hermes line whose connector joined through `hermes` into `home`, approved.
struct Joined {
    name: String,
    invite: Invite,
    code: String,
}

async fn joined(fixture: &Fixture, hermes: &FakeHermes, home: &Path) -> Joined {
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let ran = join(hermes, Path::new(PROGRAM), fixture, &invite, home).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(ran.stderr.is_empty(), "{ran:?}");
    let code = code_in(&ran.stdout);
    approve(fixture, &name, &code).await;
    Joined { name, invite, code }
}

/// `count` copies of the server registered, started by Hermes, each one initialized.
async fn started(hermes: &FakeHermes, count: usize) -> Vec<McpCopy> {
    let mut copies = Vec::new();
    for _ in 0..count {
        let mut copy = hermes.start(SERVER_NAME);
        let result = copy.initialize().await;
        assert_eq!(result["protocolVersion"], "2025-11-25", "{result}");
        copies.push(copy);
    }
    copies
}

/// Waits for the session of the device of `identity`.
async fn in_session(fixture: &Fixture, identity: &PublicKey) {
    let id = fixture.id_of_key(identity).await;
    eventually("the session", PATIENCE, || async {
        fixture.hub().sessions_by_device().get(&id) == Some(&1)
    })
    .await;
}

/// What `arauto_status` answers for the Hermes `name`, before the second milestone: the answer of
/// the hub, and `local_waking`, which the connector adds: whether it received the key of the
/// local API of Hermes.
fn status_of(name: &str, local_waking: bool) -> Value {
    json!({
        "connected": true,
        "hermes": name,
        "can_be_woken": false,
        "woken_by": {"notes": false, "whatsapp": false},
        "numbers": [],
        "peers": {"enabled": false, "can_send": false, "can_receive": false},
        "local_waking": local_waking,
        "keep_in_memory": arauto_connector::memory::keep_in_memory(),
    })
}

/// [`served_with`], for a Hermes that has no key of the local API.
async fn served(copy: &mut McpCopy, name: &str) -> Value {
    served_with(copy, name, false).await
}

/// Calls `arauto_status` through `copy` until the hub answers it: a copy that just started, or
/// one whose leader just changed, gets AR-203 for a moment. `local_waking` is what the connector
/// is expected to say about the key.
async fn served_with(copy: &mut McpCopy, name: &str, local_waking: bool) -> Value {
    let until = tokio::time::Instant::now() + PATIENCE;
    loop {
        let result = copy.call("arauto_status", json!({})).await;
        if result["isError"] == false
            && result["structuredContent"] == status_of(name, local_waking)
        {
            return result;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "never served: {result}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The result of a refusal with `code`.
fn refused(code: ErrorCode) -> Value {
    json!({
        "content": [{"type": "text", "text": code.phrase(None)}],
        "isError": true,
    })
}

/// Calls `arauto_status` through `copy` until it is refused with `code`.
async fn refused_with(copy: &mut McpCopy, code: ErrorCode) {
    let until = tokio::time::Instant::now() + PATIENCE;
    loop {
        let result = copy.call("arauto_status", json!({})).await;
        if result == refused(code) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "never refused with {code}: {result}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The copy of `copies` that leads `home`.
async fn leader_of(copies: &[McpCopy], home: &Path) -> usize {
    let leader = leader_process(home).await;
    copies
        .iter()
        .position(|copy| copy.id().is_some() && copy.id() == leader)
        .unwrap_or_else(|| panic!("the leader {leader:?} is none of the copies"))
}

/// Waits until `copy` was told that the list of tools changed.
async fn told_of_a_change(copy: &mut McpCopy) {
    let until = tokio::time::Instant::now() + PATIENCE;
    let mut seen = Vec::new();
    loop {
        // A ping reads what came before its answer; notifications are kept aside.
        let _ = copy.request("ping", json!({})).await;
        seen.extend(copy.take_aside());
        if seen
            .iter()
            .any(|message| message["method"] == "notifications/tools/list_changed")
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "never told of a change: {seen:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Every copy closed as Hermes closes it.
async fn close_all(copies: Vec<McpCopy>) {
    for copy in copies {
        ended_in_order(&copy.close(CLOSE_GRACE).await);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_block_reaches_three_copies_and_each_one_answers() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;

    // The registration: the official command, the program at its fixed path, and the folder,
    // which is not the default one.
    let fixed = program_at(&home);
    let args = vec![
        "mcp".to_owned(),
        "--home".to_owned(),
        home.display().to_string(),
    ];
    let mut expected = vec![
        "mcp".to_owned(),
        "add".to_owned(),
        "arauto".to_owned(),
        "--command".to_owned(),
        fixed.display().to_string(),
        "--env".to_owned(),
        "ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}".to_owned(),
        "--args".to_owned(),
    ];
    expected.extend(args.iter().cloned());
    // The registration, and the list that proves it.
    assert_eq!(
        hermes.calls(),
        vec![expected, vec!["mcp".to_owned(), "list".to_owned()]]
    );
    assert_eq!(
        hermes.registered(SERVER_NAME),
        Some(Registered {
            command: fixed.clone(),
            args,
            env: std::collections::BTreeMap::from([(
                "ARAUTO_HERMES_API_KEY".to_owned(),
                "${API_SERVER_KEY}".to_owned()
            )]),
        })
    );
    assert_eq!(
        std::fs::read(&fixed).unwrap(),
        std::fs::read(PROGRAM).unwrap()
    );

    // Hermes starts three copies: one leader, two that forward to it, one channel.
    let mut copies = started(&hermes, 3).await;
    let device = only_device(&home);
    let identity = public(&home, &device);
    in_session(&fixture, &identity).await;
    let leader = leader_process(&home).await;
    assert_eq!(
        copies.iter().filter(|copy| copy.id() == leader).count(),
        1,
        "{leader:?}"
    );
    // `arauto_status` through each one: the answer of the hub, as structured content and as
    // its text, the same through the leader and through the copies that forward.
    let mut answers = Vec::new();
    for copy in &mut copies {
        let result = served(copy, &joined.name).await;
        let text: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, result["structuredContent"]);
        assert_eq!(copy.tools().await, ["arauto_status"]);
        answers.push(result);
    }
    assert!(answers.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(fixture.links_of(&identity), 1);

    // `status`: the leader, the session, and no secret.
    let ran = command(&hermes, &home, &["status"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(ran.stdout.contains("Leader: running"), "{ran:?}");
    assert!(
        ran.stdout.contains(&format!(
            "Channel: in session, as the Hermes {}",
            joined.name
        )),
        "{ran:?}"
    );
    let mut secrets = Secrets::default();
    secrets.add(
        "identity",
        &std::fs::read(device_file(&home, &device, "identity.key")).unwrap(),
    );
    secrets.add(
        "credential",
        &std::fs::read(device_file(&home, &device, "credential.seal")).unwrap(),
    );
    secrets.add("invite", &invite_bytes(&joined.invite));
    secrets.add_text("invite text", &joined.invite.reveal());
    secrets.codes.push(joined.code.clone());
    let shown = format!("{}\n{}", ran.stdout, ran.stderr);
    assert!(secrets.leaks(&shown, true).is_empty(), "{ran:?}");

    // Hermes closes their input: each one ends in order, in less than 2 seconds, and the hub
    // sees the channel close.
    close_all(copies).await;
    eventually("the channel closes", Duration::from_secs(5), || async {
        fixture.links_of(&identity) == 0
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn when_the_leader_dies_the_calls_go_on_through_the_copy_that_takes_its_place() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    let mut copies = started(&hermes, 3).await;
    let identity = public(&home, &only_device(&home));
    in_session(&fixture, &identity).await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }
    let dead = copies.remove(leader_of(&copies, &home).await);
    let dead_process = dead.id();
    let (stdout, _) = dead.kill().await;
    only_messages(&stdout);
    let remaining: Vec<Option<u32>> = copies.iter().map(McpCopy::id).collect();
    eventually("another copy leads", PATIENCE, || async {
        let now = leader_process(&home).await;
        now.is_some() && now != dead_process && remaining.contains(&now)
    })
    .await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }
    // Hermes starts the dead one again: it forwards to the new leader.
    let mut again = hermes.start(SERVER_NAME);
    again.initialize().await;
    served(&mut again, &joined.name).await;
    copies.push(again);
    assert_eq!(fixture.links_of(&identity), 1);
    close_all(copies).await;
}

/// A call that left through a copy, and whose leader died before the answer: AR-207, the result
/// is unknown. The copy then takes the place, and the next call runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_in_flight_when_the_leader_dies_is_ar207() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    let mut copies = started(&hermes, 2).await;
    let identity = public(&home, &only_device(&home));
    in_session(&fixture, &identity).await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }
    let leader = copies.remove(leader_of(&copies, &home).await);
    let mut relay = copies.remove(0);

    // The hub holds the answer: `arauto_status` waits for the table of the links.
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let id = relay
        .send_request(
            "tools/call",
            json!({"name": "arauto_status", "arguments": {}}),
        )
        .await;
    fixture.until_a_query_waits_for_a_lock().await;
    let _ = leader.kill().await;
    let answer = relay.answer(id, PATIENCE).await;
    assert_eq!(answer["result"], refused(ErrorCode::Ar207), "{answer}");
    held.rollback().await.unwrap();

    served(&mut relay, &joined.name).await;
    assert_eq!(leader_process(&home).await, relay.id());
    ended_in_order(&relay.close(CLOSE_GRACE).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn with_the_hub_away_the_list_is_the_last_one_and_the_calls_are_ar203() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let mut fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    let mut copies = started(&hermes, 2).await;
    let identity = public(&home, &only_device(&home));
    in_session(&fixture, &identity).await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }
    // The leader keeps the list of the session when it is not the one it started with.
    if let Ok(bytes) = std::fs::read(home.join("state.json")) {
        let state: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(state, json!({"v": 1, "tools": ["arauto_status"]}));
    }

    // The hub stops: every call is AR-203, through the leader and through the copy.
    let addr = fixture.addr();
    fixture.stop().await;
    for copy in &mut copies {
        refused_with(copy, ErrorCode::Ar203).await;
    }

    // Hermes starts them again while the hub is away. The last session had one more tool:
    // it is in the list, and every call is AR-203, the same through both.
    for copy in copies {
        let (stdout, _) = copy.kill().await;
        only_messages(&stdout);
    }
    std::fs::write(
        home.join("state.json"),
        br#"{"v":1,"tools":["arauto_status","hermes_list_peers"]}"#,
    )
    .unwrap();
    let mut copies = started(&hermes, 2).await;
    for copy in &mut copies {
        assert_eq!(copy.tools().await, ["arauto_status", "hermes_list_peers"]);
        for tool in ["arauto_status", "hermes_list_peers"] {
            assert_eq!(
                copy.call(tool, json!({})).await,
                refused(ErrorCode::Ar203),
                "{tool}"
            );
        }
    }

    // The hub comes back where it was: the session, and its list, told to each copy.
    fixture.start_at(addr).await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }
    for copy in &mut copies {
        told_of_a_change(copy).await;
        assert_eq!(copy.tools().await, ["arauto_status"]);
    }
    close_all(copies).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn leave_deletes_the_files_and_takes_out_the_registration() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    let mut copies = started(&hermes, 2).await;
    let identity = public(&home, &only_device(&home));
    in_session(&fixture, &identity).await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }

    let ran = command(&hermes, &home, &["leave"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(ran.stdout.contains("This device left the hub"), "{ran:?}");
    assert!(
        ran.stdout.contains(Told::NotesStayInMemory.phrase()),
        "{ran:?}"
    );
    assert!(devices(&home).is_empty());
    assert!(!home.join("current").exists());
    assert!(!home.join("state.json").exists());
    assert_eq!(
        hermes.calls().last().unwrap(),
        &["mcp".to_owned(), "remove".to_owned(), "arauto".to_owned()]
    );
    assert_eq!(hermes.registered(SERVER_NAME), None);
    assert_eq!(fixture.state_of_key(&identity).await.0, "revoked");
    // The copies stay, with nothing to serve.
    for copy in &mut copies {
        refused_with(copy, ErrorCode::Ar203).await;
        assert_eq!(copy.tools().await, ["arauto_status"]);
    }
    close_all(copies).await;
}

/// The folder reaches the registration and the copies that Hermes starts with its filtered
/// environment: by `--home`, by `ARAUTO_HOME`, and the default folder of the user without
/// either.
#[tokio::test(flavor = "multi_thread")]
async fn the_folder_reaches_the_registration_and_the_copies() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    for how in ["flag", "variable", "default"] {
        let hermes = fake_hermes();
        let elsewhere = tempfile::tempdir().unwrap();
        let home = if how == "default" {
            hermes.default_home()
        } else {
            elsewhere.path().join("arauto")
        };
        let name = fixture.hermes().await;
        let invite = fixture.invite(&name).await;
        let mut command = hermes.terminal(Path::new(PROGRAM));
        command
            .arg("join")
            .arg(fixture.channel_url())
            .arg(fixture.hub_key_text())
            .arg(invite.reveal().as_str());
        match how {
            "flag" => {
                command.arg("--home").arg(&home);
            }
            "variable" => {
                command.env("ARAUTO_HOME", &home);
            }
            _ => {}
        }
        let ran = ran(command).await;
        assert_eq!(ran.code, Some(0), "{how}: {ran:?}");
        let registered = hermes.registered(SERVER_NAME).unwrap();
        assert_eq!(registered.command, program_at(&home), "{how}");
        // Also for the default folder: the copy Hermes starts may have another HOME than the
        // terminal where `join` ran.
        assert_eq!(
            registered.args,
            [
                "mcp".to_owned(),
                "--home".to_owned(),
                home.display().to_string()
            ],
            "{how}"
        );
        approve(&fixture, &name, &code_in(&ran.stdout)).await;
        // The variable never reaches what Hermes starts: only what the entry carries does.
        assert!(
            !hermes
                .environment()
                .iter()
                .any(|(variable, _)| variable.to_string_lossy().starts_with("ARAUTO")),
            "{how}"
        );
        let mut copy = hermes.start(SERVER_NAME);
        copy.initialize().await;
        served(&mut copy, &name).await;
        assert_eq!(leader_process(&home).await, copy.id(), "{how}");
        ended_in_order(&copy.close(CLOSE_GRACE).await);
    }
}

/// A `join` with a new program copies it over the one that runs at the fixed path, and every
/// copy ends, the leader because the `join` asked it and the others because the leader told
/// them: Hermes starts them again, on the new program, and the new device takes over.
#[tokio::test(flavor = "multi_thread")]
async fn a_join_with_a_new_program_replaces_the_one_running_and_every_copy_starts_again() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    let mut copies = started(&hermes, 3).await;
    let old = only_device(&home);
    let old_identity = public(&home, &old);
    in_session(&fixture, &old_identity).await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }

    // Another build: the same program with bytes after its end.
    let download = tempfile::tempdir().unwrap();
    let newer = download.path().join(format!(
        "arauto-connector-new{}",
        std::env::consts::EXE_SUFFIX
    ));
    let mut bytes = std::fs::read(PROGRAM).unwrap();
    bytes.extend_from_slice(b"\0another build of the tests");
    std::fs::write(&newer, &bytes).unwrap();
    let invite = fixture.invite(&joined.name).await;
    let ran = join(&hermes, &newer, &fixture, &invite, &home).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    let code = code_in(&ran.stdout);
    assert_eq!(std::fs::read(program_at(&home)).unwrap(), bytes);
    assert_eq!(
        hermes.registered(SERVER_NAME).map(|entry| entry.command),
        Some(program_at(&home))
    );

    for copy in copies {
        let (status, stdout, _) = copy.ended(PATIENCE).await;
        assert_eq!(status.and_then(|status| status.code()), Some(0));
        only_messages(&stdout);
    }
    let mut copies = started(&hermes, 3).await;
    eventually("the new request waits for the admin", PATIENCE, || async {
        ask(&home, r#"{"v":1,"op":"status"}"#)
            .await
            .is_some_and(|answer| answer["status"]["request"]["channel"] == "waiting")
    })
    .await;
    approve(&fixture, &joined.name, &code).await;
    eventually("the new device takes over", PATIENCE, || async {
        current(&home).is_some_and(|now| now != old)
    })
    .await;
    for copy in &mut copies {
        served(copy, &joined.name).await;
    }
    eventually("the old folder goes", PATIENCE, || async {
        !devices(&home).contains(&old)
    })
    .await;
    assert_eq!(fixture.state_of_key(&old_identity).await.0, "replaced");
    close_all(copies).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_registration_is_ar601_and_the_same_join_registers_again() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    hermes.refuse(true);
    let ran = join(&hermes, Path::new(PROGRAM), &fixture, &invite, &home).await;
    assert_eq!(ran.code, Some(7), "{ran:?}");
    assert!(ran.stderr.starts_with("AR-601: "), "{ran:?}");
    assert!(!ran.stdout.contains("VERIFICATION CODE"), "{ran:?}");
    // The request stays: the folder, and Hermes was asked twice, with the reference and without
    // it (a refusal is how an older Hermes answers an option it does not know).
    assert_eq!(devices(&home).len(), 1);
    assert_eq!(hermes.calls().len(), 2);
    assert_eq!(hermes.registered(SERVER_NAME), None);

    hermes.refuse(false);
    let ran = join(&hermes, Path::new(PROGRAM), &fixture, &invite, &home).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(
        ran.stdout.contains("replaces the one shown before"),
        "{ran:?}"
    );
    assert!(hermes.registered(SERVER_NAME).is_some());
    approve(&fixture, &name, &code_in(&ran.stdout)).await;
    let mut copies = started(&hermes, 1).await;
    served(&mut copies[0], &name).await;
    close_all(copies).await;
}

/// The canary of secrets, end to end (`docs/15-mvp.md`, section 5): a refused invite and a valid
/// one enrolled twice, the approval, three copies that list and call, `status`, the 24-hour swap,
/// the death of the leader and the swap of the copy that takes its place, `leave`, and the end.
/// The identity, every credential, both invites and both codes are looked for in the output and
/// the standard error of every process, in the files of the folder and in the records of the
/// fake Hermes. The code may appear only in the output of the `join` that shows it.
#[tokio::test(flavor = "multi_thread")]
async fn the_canary_end_to_end() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let clock = ManualClock::new(SystemClock.now_ms());
    let fixture = Fixture::with_clock(settings(), Arc::new(clock.clone())).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let mut secrets = Secrets::default();
    let mut joins: Vec<Ran> = Vec::new();
    let mut outputs: Vec<(String, String)> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();

    // The key of the local API in every place it could leak from: the secrets of Hermes, what its
    // secret source injects into every server, and the terminal of the operator.
    let canaries: Vec<String> = [
        "secret-scope",
        "provider",
        "terminal-key",
        "terminal-ref",
        "other",
    ]
    .iter()
    .map(|label| format!("CANARY-{label}-0123456789abcdef0123456789abcdef0123456789"))
    .collect();
    hermes.set_secret("API_SERVER_KEY", &canaries[0]);
    hermes.inject("PROVIDER_API_KEY", &canaries[1]);
    hermes.set_terminal_env("API_SERVER_KEY", &canaries[2]);
    hermes.set_terminal_env("ARAUTO_HERMES_API_KEY", &canaries[3]);
    hermes.set_terminal_env("OTHER_SECRET", &canaries[4]);
    for (at, canary) in canaries.iter().enumerate() {
        secrets.add_text(&format!("canary {at}"), canary);
    }

    let name = fixture.hermes().await;
    let unknown = Invite::generate().unwrap();
    secrets.add("refused invite", &invite_bytes(&unknown));
    secrets.add_text("refused invite text", &unknown.reveal());
    let ran = join(&hermes, Path::new(PROGRAM), &fixture, &unknown, &home).await;
    assert_eq!(ran.code, Some(3), "{ran:?}");
    outputs.push((
        "join refused".to_owned(),
        format!("{}\n{}", ran.stdout, ran.stderr),
    ));
    let invite = fixture.invite(&name).await;
    secrets.add("invite", &invite_bytes(&invite));
    secrets.add_text("invite text", &invite.reveal());
    for _ in 0..2 {
        let ran = join(&hermes, Path::new(PROGRAM), &fixture, &invite, &home).await;
        assert_eq!(ran.code, Some(0), "{ran:?}");
        secrets.codes.push(code_in(&ran.stdout));
        joins.push(ran);
    }
    let device = only_device(&home);
    let identity = public(&home, &device);
    secrets.add(
        "identity",
        &std::fs::read(device_file(&home, &device, "identity.key")).unwrap(),
    );
    approve(&fixture, &name, secrets.codes.last().unwrap()).await;

    let mut copies = started(&hermes, 3).await;
    in_session(&fixture, &identity).await;
    for copy in &mut copies {
        served_with(copy, &name, true).await;
        copy.tools().await;
    }
    let credential = device_file(&home, &device, "credential.seal");
    let first = std::fs::read(&credential).unwrap();
    secrets.add("credential 1", &first);
    let ran = command(&hermes, &home, &["status"]).await;
    outputs.push((
        "status".to_owned(),
        format!("{}\n{}", ran.stdout, ran.stderr),
    ));
    files.extend(files_of(&home));

    // The 24-hour swap.
    clock.advance(DAY_MS + SECOND_MS);
    eventually("the 24-hour swap", PATIENCE, || async {
        std::fs::read(&credential).is_ok_and(|now| now != first)
    })
    .await;
    let second = std::fs::read(&credential).unwrap();
    secrets.add("credential 2", &second);
    files.extend(files_of(&home));

    // The leader dies; the copy that takes its place swaps the credential again.
    let dead = copies.remove(leader_of(&copies, &home).await);
    let (stdout, stderr) = dead.kill().await;
    outputs.push((
        "killed leader, output".to_owned(),
        String::from_utf8_lossy(&stdout).into_owned(),
    ));
    outputs.push((
        "killed leader, log".to_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    ));
    eventually("the swap of the new leader", PATIENCE, || async {
        std::fs::read(&credential).is_ok_and(|now| now != second)
    })
    .await;
    secrets.add("credential 3", &std::fs::read(&credential).unwrap());
    for copy in &mut copies {
        served_with(copy, &name, true).await;
    }
    let ran = command(&hermes, &home, &["status"]).await;
    outputs.push((
        "status again".to_owned(),
        format!("{}\n{}", ran.stdout, ran.stderr),
    ));
    files.extend(files_of(&home));

    let ran = command(&hermes, &home, &["leave"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    outputs.push((
        "leave".to_owned(),
        format!("{}\n{}", ran.stdout, ran.stderr),
    ));
    let ran = command(&hermes, &home, &["status"]).await;
    outputs.push((
        "status after leave".to_owned(),
        format!("{}\n{}", ran.stdout, ran.stderr),
    ));

    let mut logs = Vec::new();
    for copy in copies {
        let closed = copy.close(CLOSE_GRACE).await;
        ended_in_order(&closed);
        outputs.push((
            "output of mcp".to_owned(),
            String::from_utf8_lossy(&closed.stdout).into_owned(),
        ));
        logs.push(String::from_utf8_lossy(&closed.stderr).into_owned());
    }
    assert!(
        logs.iter().any(|log| log.contains("leader")),
        "no log was captured: {logs:?}"
    );
    for log in logs {
        outputs.push(("log of mcp".to_owned(), log));
    }
    files.extend(files_of(&home));
    for record in [
        arauto_testkit::hermes::registry::CALLS_FILE,
        arauto_testkit::hermes::registry::SERVERS_FILE,
        arauto_testkit::hermes::registry::ENVIRONMENT_FILE,
    ] {
        let text = std::fs::read_to_string(hermes.folder().join(record)).unwrap_or_default();
        files.push((record.to_owned(), text));
    }

    let mut found = Vec::new();
    for ran in &joins {
        // The code of this join is in its output, and nothing else secret.
        for leak in secrets.leaks(&ran.stdout, false) {
            found.push(format!("output of join: {leak}"));
        }
        for leak in secrets.leaks(&ran.stderr, true) {
            found.push(format!("standard error of join: {leak}"));
        }
    }
    for (what, text) in &outputs {
        for leak in secrets.leaks(text, true) {
            found.push(format!("{what}: {leak}"));
        }
    }
    for (file, text) in &files {
        for leak in secrets.leaks(text, true) {
            found.push(format!("{file}: {leak}"));
        }
    }
    assert!(found.is_empty(), "secrets found:\n{}", found.join("\n"));
    // The entry holds the reference, never a value, and the command of Hermes never had the
    // variables of the key in its environment, whatever the terminal had.
    assert_eq!(
        hermes.registered(SERVER_NAME).map(|entry| entry.env),
        None,
        "removed by leave"
    );
    assert_no_key_variables(&hermes);
}

/// No call of the command of Hermes had `API_SERVER_KEY` or `ARAUTO_HERMES_API_KEY` in its
/// environment.
fn assert_no_key_variables(hermes: &FakeHermes) {
    let environments = hermes.command_environments();
    assert!(!environments.is_empty());
    for names in &environments {
        for name in names {
            let upper = name.to_ascii_uppercase();
            assert!(
                upper != "API_SERVER_KEY" && upper != "ARAUTO_HERMES_API_KEY",
                "{name} reached the command of Hermes: {names:?}"
            );
        }
    }
}

fn hub_json(home: &Path, device: &str) -> Value {
    serde_json::from_slice(&std::fs::read(device_file(home, device, "hub.json")).unwrap()).unwrap()
}

fn key_canary() -> String {
    "CANARY-the-key-of-the-local-api-9f8e7d6c5b4a39281706f5e4d3c2b1a0".to_owned()
}

/// Whether `text` holds a line (not a part of one).
fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|found| found == line)
}

const REFERENCED: &str = "Entry in Hermes: saved with a reference to API_SERVER_KEY";
const RECEIVED: &str = "Key of the local API of Hermes: received by the running connector";
const NOT_RECEIVED: &str = "Key of the local API of Hermes: not received by the running connector (no notice wakes this Hermes)";

/// The entry carries the reference, Hermes resolves it from its own secrets when it starts a
/// copy, and `status` (run without the variable) and `arauto_status` say that the running
/// connector holds the key.
#[tokio::test(flavor = "multi_thread")]
async fn join_registers_with_the_reference_and_the_copies_get_the_key() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let key = key_canary();
    hermes.set_secret("API_SERVER_KEY", &key);
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;

    // What Hermes saved is the reference, literally; the value is not in the entry.
    let entry = hermes.registered(SERVER_NAME).unwrap();
    assert_eq!(
        entry.env,
        std::collections::BTreeMap::from([(
            "ARAUTO_HERMES_API_KEY".to_owned(),
            "${API_SERVER_KEY}".to_owned()
        )])
    );
    let device = only_device(&home);
    assert_eq!(hub_json(&home, &device)["key_reference"], json!(true));
    // Only `mcp add` and `mcp list` ran, and the environment of the command had no key variable.
    assert!(hermes.calls().iter().all(|call| call[0] == "mcp"));
    assert_no_key_variables(&hermes);

    let mut copies = started(&hermes, 2).await;
    let identity = public(&home, &device);
    in_session(&fixture, &identity).await;
    for copy in &mut copies {
        served_with(copy, &joined.name, true).await;
    }
    // `status` runs in a terminal that does not have the variable: it says what the leader holds.
    let ran = command(&hermes, &home, &["status"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(has_line(&ran.stdout, REFERENCED), "{ran:?}");
    assert!(has_line(&ran.stdout, RECEIVED), "{ran:?}");
    assert!(
        has_line(&ran.stdout, "Last wake: none since the connector started"),
        "{ran:?}"
    );
    assert!(!ran.stdout.contains("not configured"), "{ran:?}");
    let mut secrets = Secrets::default();
    secrets.add_text("key", &key);
    let shown = format!("{}\n{}", ran.stdout, ran.stderr);
    assert!(secrets.leaks(&shown, false).is_empty(), "{ran:?}");
    close_all(copies).await;
}

/// A Hermes whose local API is off still connects. The connector says what to look for, in
/// Hermes's own words, and `status` tells what the running connector holds, which is not what
/// the terminal of `status` holds.
#[tokio::test(flavor = "multi_thread")]
async fn a_hermes_without_its_key_is_connected_and_told() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    let folder = Folder::new();
    let home = folder.home.clone();
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let ran = join(&hermes, Path::new(PROGRAM), &fixture, &invite, &home).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(
        ran.stdout.contains(Told::WakingNeedsLocalApi.phrase()),
        "{ran:?}"
    );
    assert!(ran.stdout.contains("API_SERVER_KEY"), "{ran:?}");
    assert!(ran.stdout.contains("VERIFICATION CODE"), "{ran:?}");
    approve(&fixture, &name, &code_in(&ran.stdout)).await;

    // The secrets of this Hermes are empty: the reference is left out, as Hermes does.
    let mut copies = started(&hermes, 1).await;
    in_session(&fixture, &public(&home, &only_device(&home))).await;
    served_with(&mut copies[0], &name, false).await;

    // `status` run by an operator who has the variable in the terminal: the line is about the
    // running connector, so it still says "not received", and the canary is nowhere.
    let key = key_canary();
    hermes.set_terminal_env("ARAUTO_HERMES_API_KEY", &key);
    let ran = command(&hermes, &home, &["status"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(has_line(&ran.stdout, REFERENCED), "{ran:?}");
    assert!(has_line(&ran.stdout, NOT_RECEIVED), "{ran:?}");
    assert!(
        ran.stdout.contains(Told::KeyNotReceived.phrase()),
        "{ran:?}"
    );
    assert!(
        ran.stdout.contains(Told::KeyAfterRestart.phrase()),
        "{ran:?}"
    );
    let mut secrets = Secrets::default();
    secrets.add_text("key", &key);
    let shown = format!("{}\n{}", ran.stdout, ran.stderr);
    assert!(secrets.leaks(&shown, false).is_empty(), "{ran:?}");
    for (file, text) in files_of(&home) {
        assert!(secrets.leaks(&text, false).is_empty(), "{file}");
    }
    close_all(copies).await;
}

/// A Hermes that does not know `--env` (its `add` exits 2 before any question) is connected
/// without the reference, and says so.
#[tokio::test(flavor = "multi_thread")]
async fn an_old_hermes_without_env_is_connected_without_the_reference() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    hermes.without_env_option(true);
    let folder = Folder::new();
    let home = folder.home.clone();
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let ran = join(&hermes, Path::new(PROGRAM), &fixture, &invite, &home).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(
        ran.stdout.contains(Told::ReferenceRefused.phrase()),
        "{ran:?}"
    );
    assert!(
        ran.stdout
            .contains("registered in this Hermes without a reference to the key"),
        "{ran:?}"
    );
    assert!(
        !ran.stdout.contains(Told::WakingNeedsLocalApi.phrase()),
        "{ran:?}"
    );
    let calls = hermes.calls();
    assert_eq!(calls.len(), 3, "{calls:?}");
    assert!(calls[0].contains(&"--env".to_owned()));
    assert!(!calls[1].contains(&"--env".to_owned()));
    assert!(hermes.registered(SERVER_NAME).unwrap().env.is_empty());
    let device = only_device(&home);
    assert_eq!(hub_json(&home, &device)["key_reference"], json!(false));
    approve(&fixture, &name, &code_in(&ran.stdout)).await;

    let mut copies = started(&hermes, 1).await;
    in_session(&fixture, &public(&home, &device)).await;
    served_with(&mut copies[0], &name, false).await;
    let ran = command(&hermes, &home, &["status"]).await;
    assert!(
        has_line(
            &ran.stdout,
            "Entry in Hermes: saved without a reference to the key (this version of Hermes refused it)"
        ),
        "{ran:?}"
    );
    assert!(has_line(&ran.stdout, NOT_RECEIVED), "{ran:?}");
    assert!(
        ran.stdout.contains(Told::ReferenceRefused.phrase()),
        "{ran:?}"
    );
    close_all(copies).await;
}

/// `register` saves the entry again and changes nothing of the connection: the same device,
/// the same credential, the same leader. A running copy keeps what it loaded; a copy started
/// after it (once the running one is gone) receives the key.
#[tokio::test(flavor = "multi_thread")]
async fn register_saves_the_entry_again_without_touching_the_connection() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    hermes.without_env_option(true);
    hermes.set_secret("API_SERVER_KEY", &key_canary());
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    let device = only_device(&home);
    let identity = public(&home, &device);
    assert_eq!(hub_json(&home, &device)["key_reference"], json!(false));

    let mut copies = started(&hermes, 1).await;
    in_session(&fixture, &identity).await;
    served_with(&mut copies[0], &joined.name, false).await;
    let before = (
        std::fs::read(device_file(&home, &device, "identity.key")).unwrap(),
        std::fs::read(device_file(&home, &device, "credential.seal")).unwrap(),
        std::fs::read(home.join("current")).unwrap(),
    );
    let leader = leader_process(&home).await;
    let state_before = fixture.state_of_key(&identity).await;

    hermes.without_env_option(false);
    let ran = command(&hermes, &home, &["register"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(
        has_line(
            &ran.stdout,
            "MCP server \"arauto\" saved again in this Hermes, with a reference to the key of its local API."
        ),
        "{ran:?}"
    );
    for told in [
        Told::ConnectionUnchanged,
        Told::RunningKeepsOldEntry,
        Told::WakingNeedsLocalApi,
    ] {
        assert!(ran.stdout.contains(told.phrase()), "{ran:?}");
    }
    assert!(ran.stderr.is_empty(), "{ran:?}");
    assert_eq!(
        hermes.registered(SERVER_NAME).unwrap().env,
        std::collections::BTreeMap::from([(
            "ARAUTO_HERMES_API_KEY".to_owned(),
            "${API_SERVER_KEY}".to_owned()
        )])
    );
    assert_eq!(hub_json(&home, &device)["key_reference"], json!(true));
    assert_eq!(hub_json(&home, &device)["registered"], json!(true));

    // The connection is untouched: the same files, the same device, the same leader and the
    // same state on the hub; the running copy still serves, with what it loaded.
    let after = (
        std::fs::read(device_file(&home, &device, "identity.key")).unwrap(),
        std::fs::read(device_file(&home, &device, "credential.seal")).unwrap(),
        std::fs::read(home.join("current")).unwrap(),
    );
    assert_eq!(before, after);
    assert_eq!(devices(&home), std::slice::from_ref(&device));
    assert_eq!(leader_process(&home).await, leader);
    assert_eq!(fixture.state_of_key(&identity).await, state_before);
    assert_eq!(fixture.links_of(&identity), 1);
    served_with(&mut copies[0], &joined.name, false).await;
    let ran = command(&hermes, &home, &["status"]).await;
    assert!(has_line(&ran.stdout, REFERENCED), "{ran:?}");
    assert!(has_line(&ran.stdout, NOT_RECEIVED), "{ran:?}");

    // Hermes restarts its gateway: the running copy ends, and a copy started afterwards has the
    // new entry and receives the key.
    close_all(copies).await;
    eventually("the channel closes", Duration::from_secs(5), || async {
        fixture.links_of(&identity) == 0
    })
    .await;
    let mut copies = started(&hermes, 1).await;
    in_session(&fixture, &identity).await;
    served_with(&mut copies[0], &joined.name, true).await;
    let ran = command(&hermes, &home, &["status"]).await;
    assert!(has_line(&ran.stdout, RECEIVED), "{ran:?}");

    // The connector only ran `mcp add`, `mcp list` and `mcp remove`: it never touched the
    // secrets, and the environment of the command had no variable of the key.
    assert!(hermes.calls().iter().all(|call| call[0] == "mcp"));
    assert_no_key_variables(&hermes);
    close_all(copies).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn register_with_no_device_runs_nothing() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let hermes = fake_hermes();
    let folder = Folder::new();
    let ran = command(&hermes, &folder.home, &["register"]).await;
    assert_eq!(ran.code, Some(9), "{ran:?}");
    assert!(ran.stdout.is_empty(), "{ran:?}");
    assert_eq!(
        ran.stderr.trim_end(),
        Local::NothingToRegister.phrase(),
        "{ran:?}"
    );
    assert!(hermes.calls().is_empty());
    // A folder that exists with nothing in it: the same.
    std::fs::create_dir_all(folder.home.join("devices")).unwrap();
    let ran = command(&hermes, &folder.home, &["register"]).await;
    assert_eq!(ran.code, Some(9), "{ran:?}");
    assert_eq!(
        ran.stderr.trim_end(),
        Local::NothingToRegister.phrase(),
        "{ran:?}"
    );
    assert!(hermes.calls().is_empty());
}

/// A rejoin leaves the entry alone when it has the reference, and saves it again when it does
/// not.
#[tokio::test(flavor = "multi_thread")]
async fn a_rejoin_registers_again_only_when_the_entry_lacks_the_reference() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    for lacking in [false, true] {
        let hermes = fake_hermes();
        hermes.without_env_option(lacking);
        let folder = Folder::new();
        let home = folder.home.clone();
        let joined = joined(&fixture, &hermes, &home).await;
        let mut copies = started(&hermes, 1).await;
        let identity = public(&home, &only_device(&home));
        in_session(&fixture, &identity).await;
        served_with(&mut copies[0], &joined.name, false).await;
        let adds = |hermes: &FakeHermes| {
            hermes
                .calls()
                .iter()
                .filter(|call| call.get(1).is_some_and(|word| word == "add"))
                .count()
        };
        let before = adds(&hermes);
        assert_eq!(before, if lacking { 2 } else { 1 });

        hermes.without_env_option(false);
        let invite = fixture.invite(&joined.name).await;
        let ran = join(&hermes, Path::new(PROGRAM), &fixture, &invite, &home).await;
        assert_eq!(ran.code, Some(0), "{lacking}: {ran:?}");
        if lacking {
            assert_eq!(adds(&hermes), before + 1, "saved again");
            assert!(
                ran.stdout.contains(Told::RunningKeepsOldEntry.phrase()),
                "{ran:?}"
            );
            assert!(
                ran.stdout
                    .contains("registered in this Hermes, with a reference"),
                "{ran:?}"
            );
            assert_eq!(
                hermes.registered(SERVER_NAME).unwrap().env.len(),
                1,
                "the reference is there now"
            );
        } else {
            assert_eq!(adds(&hermes), before, "no second add");
            assert!(
                ran.stdout.contains("already saved in this Hermes"),
                "{ran:?}"
            );
            assert!(
                !ran.stdout.contains(Told::RunningKeepsOldEntry.phrase()),
                "{ran:?}"
            );
        }
        close_all(copies).await;
    }
}

/// Whatever the terminal of the operator has, the command of Hermes does not get the two
/// variables of the key; the rest of the environment passes, because Hermes needs to find its
/// own folder.
#[tokio::test(flavor = "multi_thread")]
async fn the_command_of_hermes_never_gets_the_key_variables() {
    let _turn = AT_ONCE.acquire().await.unwrap();
    let fixture = Fixture::new(settings()).await;
    let hermes = fake_hermes();
    hermes.set_terminal_env("API_SERVER_KEY", "CANARY-terminal-a-0123456789abcdef");
    hermes.set_terminal_env(
        "ARAUTO_HERMES_API_KEY",
        "CANARY-terminal-b-0123456789abcdef",
    );
    hermes.set_terminal_env("OTHER_VARIABLE", "not a secret");
    let folder = Folder::new();
    let home = folder.home.clone();
    let joined = joined(&fixture, &hermes, &home).await;
    assert_no_key_variables(&hermes);
    assert!(
        hermes
            .command_environments()
            .iter()
            .all(|names| names.iter().any(|name| name == "OTHER_VARIABLE")),
        "the rest of the environment passes"
    );
    // Also `leave` (which runs `mcp remove`) and `register`.
    let ran = command(&hermes, &home, &["register"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    let ran = command(&hermes, &home, &["leave", "--local"]).await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(
        ran.stdout.contains(Told::NotesStayInMemory.phrase()),
        "{ran:?}"
    );
    assert_no_key_variables(&hermes);
    drop(joined);
}
