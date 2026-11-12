//! The program itself, in processes of its own, against the hub running in this process
//! (`docs/11-conector.md`, sections 1 and 3): `join` prints the code and nothing secret, three
//! copies of `mcp` have one leader and one channel, a copy takes over when the leader dies, and a
//! call in flight when the leader dies comes back with AR-207.
//!
//! A binary of its own: it runs processes and waits for them in seconds.
//!
//! These tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use arauto_connector::{Connector, Home, Role, Settings};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::CallOutcome;
use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::common::{Copy, Folder, code_in, eventually, leader_process, run};

/// Patience for what takes seconds: a process that starts, a copy that takes over.
const PATIENCE: Duration = Duration::from_secs(40);

/// `join` with the program, approved by the admin.
async fn joined(fixture: &Fixture, folder: &Folder) -> String {
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let ran = run(
        &folder.home,
        &[
            "join",
            &fixture.channel_url(),
            &fixture.hub_key_text(),
            &invite.reveal(),
            "--no-register",
        ],
    )
    .await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    assert!(ran.stderr.is_empty(), "{ran:?}");
    let code = code_in(&ran.stdout);
    let approved = fixture.approve(&name, &code).await;
    assert!(
        matches!(
            approved,
            arauto_hub::repo::approval::ApproveOutcome::Approved(_)
        ),
        "{approved:?}"
    );
    name
}

async fn sessions_of(fixture: &Fixture, folder: &Folder, device: &str) -> usize {
    let id = fixture.id_of_key(&folder.public(device)).await;
    fixture
        .hub()
        .sessions_by_device()
        .get(&id)
        .copied()
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn three_copies_have_one_leader_and_another_takes_over_when_it_dies() {
    let fixture = Fixture::new(settings()).await;
    let folder = Folder::new();
    joined(&fixture, &folder).await;
    let device = folder.only_device();
    let identity = folder.public(&device);

    let mut copies: Vec<Copy> = (0..3).map(|_| Copy::start(&folder.home)).collect();
    eventually("the device is in session", PATIENCE, || async {
        sessions_of(&fixture, &folder, &device).await == 1
    })
    .await;
    // One leader, and one channel connection for the hub.
    let first = leader_process(&folder.home).await.unwrap();
    let pids: Vec<u32> = copies.iter().filter_map(Copy::id).collect();
    assert!(pids.contains(&first), "{first} not in {pids:?}");
    assert_eq!(fixture.links_of(&identity), 1);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(fixture.links_of(&identity), 1);

    // The leader dies: another copy takes the lock, the socket and the channel.
    let index = copies
        .iter()
        .position(|copy| copy.id() == Some(first))
        .unwrap();
    let (stdout, _) = copies.remove(index).kill().await;
    assert!(stdout.is_empty(), "the standard output of mcp is only MCP");
    let remaining: Vec<u32> = copies.iter().filter_map(Copy::id).collect();
    eventually("another copy leads", PATIENCE, || async {
        leader_process(&folder.home)
            .await
            .is_some_and(|process| process != first && remaining.contains(&process))
    })
    .await;
    eventually("the device is in session again", PATIENCE, || async {
        sessions_of(&fixture, &folder, &device).await == 1 && fixture.links_of(&identity) == 1
    })
    .await;
    assert_eq!(fixture.state_of_key(&identity).await.0, "active");

    // Hermes closes their input: each one ends, in order, and wrote nothing to its output.
    for copy in copies {
        let (code, stdout, _stderr) = copy.stop().await;
        assert_eq!(code, Some(0));
        assert!(stdout.is_empty());
    }
    eventually("the channel closes", PATIENCE, || async {
        fixture.links_of(&identity) == 0
    })
    .await;
}

/// A call that left when the leader died: AR-207, the result is unknown. Then the copy that
/// forwarded takes over, and the next call works.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_in_flight_when_the_leader_dies_is_ar207() {
    let fixture = Fixture::new(settings()).await;
    let folder = Folder::new();
    joined(&fixture, &folder).await;
    let device = folder.only_device();
    let leader = Copy::start(&folder.home);
    eventually("the device is in session", PATIENCE, || async {
        sessions_of(&fixture, &folder, &device).await == 1
    })
    .await;
    let home = Home::at(&folder.home).unwrap();
    let relay = Arc::new(Connector::start(home, Settings::for_tests()).await.unwrap());
    assert_eq!(relay.role(), Role::Relay);
    // The hub has the session a moment before the leader serves it.
    eventually("a call goes through the leader", PATIENCE, || async {
        matches!(
            relay.call("arauto_status", serde_json::Map::new()).await,
            CallOutcome::Ok(_)
        )
    })
    .await;

    // The hub holds the answer: `arauto_status` waits for the table of the links.
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let call = {
        let relay = Arc::clone(&relay);
        tokio::spawn(async move { relay.call("arauto_status", serde_json::Map::new()).await })
    };
    fixture.until_a_query_waits_for_a_lock().await;
    let _ = leader.kill().await;
    let outcome = tokio::time::timeout(PATIENCE, call).await.unwrap().unwrap();
    assert_eq!(
        outcome,
        CallOutcome::Refused {
            code: ErrorCode::Ar207,
            retry_after: None
        }
    );
    held.rollback().await.unwrap();

    eventually("the copy leads", PATIENCE, || async {
        relay.role() == Role::Leader
    })
    .await;
    eventually("a call works again", PATIENCE, || async {
        matches!(
            relay.call("arauto_status", serde_json::Map::new()).await,
            CallOutcome::Ok(_)
        )
    })
    .await;
    if let Ok(relay) = Arc::try_unwrap(relay) {
        relay.shutdown().await;
    }
}

/// The program prints the fixed texts: the code only in the output of `join`, the usage
/// without the arguments, and `status` without any secret.
#[tokio::test(flavor = "multi_thread")]
async fn the_commands_print_fixed_texts() {
    let fixture = Fixture::new(settings()).await;
    let folder = Folder::new();
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let text = invite.reveal().to_string();
    // A wrong command line: the usage, and not the arguments (one of them is the invite).
    let ran = run(&folder.home, &["join", "x", "y", "z", &text]).await;
    assert_eq!(ran.code, Some(2));
    assert!(
        !ran.stderr.contains(&text) && !ran.stdout.contains(&text),
        "{ran:?}"
    );
    assert!(ran.stderr.starts_with("Usage:"), "{ran:?}");
    // A plain address to another machine: refused, with its exit code.
    let ran = run(
        &folder.home,
        &[
            "join",
            "ws://192.0.2.1/v1/channel",
            &fixture.hub_key_text(),
            &text,
        ],
    )
    .await;
    assert_eq!(ran.code, Some(2), "{ran:?}");
    // An unknown invite: AR-101.
    let unknown = arauto_protocol::invite::Invite::generate().unwrap();
    let ran = run(
        &folder.home,
        &[
            "join",
            &fixture.channel_url(),
            &fixture.hub_key_text(),
            &unknown.reveal(),
            "--no-register",
        ],
    )
    .await;
    assert_eq!(ran.code, Some(3), "{ran:?}");
    assert!(ran.stderr.starts_with("AR-101: "), "{ran:?}");
    // The registration fails: no Hermes there.
    let ran = run(
        &folder.home,
        &[
            "join",
            &fixture.channel_url(),
            &fixture.hub_key_text(),
            &text,
        ],
    )
    .await;
    assert_eq!(ran.code, Some(7), "{ran:?}");
    assert!(ran.stderr.starts_with("AR-601: "), "{ran:?}");
    // The same invite with --no-register: the code.
    let ran = run(
        &folder.home,
        &[
            "join",
            &fixture.channel_url(),
            &fixture.hub_key_text(),
            &text,
            "--no-register",
        ],
    )
    .await;
    assert_eq!(ran.code, Some(0), "{ran:?}");
    let code = code_in(&ran.stdout);
    assert!(ran.stdout.contains("previous code") || ran.stdout.contains("replaces"));
    // The program is at its fixed place.
    let fixed = folder
        .home
        .join(format!("arauto-connector{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        std::fs::read(&fixed).unwrap(),
        std::fs::read(common::PROGRAM).unwrap()
    );
    let ran = run(&folder.home, &["status"]).await;
    assert_eq!(ran.code, Some(0));
    assert!(ran.stdout.contains("waiting for the approval"), "{ran:?}");
    assert!(!ran.stdout.contains(&code) && !ran.stdout.contains(&code.replace('-', "")));
    assert!(!ran.stdout.contains(&text));
}
