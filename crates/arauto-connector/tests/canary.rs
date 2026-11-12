//! The canary of secrets of the connector (`docs/15-mvp.md`, section 5): a whole flow runs with
//! the program itself, in processes of its own, and every secret it touches is looked for in
//! everything it writes outside the two files that hold them.
//!
//! - **The flow:** an invite refused and a valid one, two enrollments (two codes), the approval,
//!   the session, `status`, the 24-hour swap, a copy of the folder that sends the device to
//!   quarantine, and `leave`.
//! - **Where:** the standard output and the standard error of every command, the log of every
//!   copy of `mcp`, the answers of the local socket, and every file of the folder but
//!   `identity.key`, `credential.seal` and the program.
//! - **What:** the identity, every credential, both invites (the text and the bytes) and both
//!   codes, written in hexadecimal, base64 and base32. The code may appear in one place only:
//!   the output of the `join` that shows it.
//!
//! A binary of its own: it runs processes and waits for them in seconds.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::invite::Invite;
use arauto_protocol::limits::{DAY_MS, SECOND_MS};
use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::common::{
    Copy, Folder, Ran, Secrets, ask, code_in, eventually, files_of, invite_bytes, run, spellings,
};

const PATIENCE: Duration = Duration::from_secs(40);

#[test]
fn the_detector_sees_a_planted_secret() {
    let mut secrets = Secrets::default();
    secrets.add("planted", &[0x5a; 32]);
    for written in spellings(&[0x5a; 32]) {
        assert_eq!(
            secrets.leaks(&format!("x {written} y"), false),
            ["planted"],
            "{written}"
        );
    }
    secrets.codes.push("028-213".to_owned());
    assert!(!secrets.leaks("the code 028213 was typed", true).is_empty());
    assert!(secrets.leaks("at 1800000282130", true).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn no_secret_reaches_the_log_the_outputs_the_files_or_the_socket() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let fixture = Fixture::with_clock(settings(), Arc::new(clock.clone())).await;
    let folder = Folder::new();
    let mut secrets = Secrets::default();
    let mut outputs: Vec<(String, Ran)> = Vec::new();
    let mut logs: Vec<String> = Vec::new();
    let mut answers: Vec<String> = Vec::new();
    let mut snapshots: Vec<(String, String)> = Vec::new();
    let join = |invite: &str| {
        vec![
            "join".to_owned(),
            fixture.channel_url(),
            fixture.hub_key_text(),
            invite.to_owned(),
            "--no-register".to_owned(),
        ]
    };

    // An invite refused, then a valid one, enrolled twice.
    let name = fixture.hermes().await;
    let refused = Invite::generate().unwrap();
    secrets.add("refused invite", &invite_bytes(&refused));
    secrets.add_text("refused invite text", &refused.reveal());
    let arguments = join(&refused.reveal());
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let ran = run(&folder.home, &arguments).await;
    assert_eq!(ran.code, Some(3), "{ran:?}");
    outputs.push(("join refused".to_owned(), ran));

    let invite = fixture.invite(&name).await;
    secrets.add("invite", &invite_bytes(&invite));
    secrets.add_text("invite text", &invite.reveal());
    let arguments = join(&invite.reveal());
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let mut codes = Vec::new();
    for _ in 0..2 {
        let ran = run(&folder.home, &arguments).await;
        assert_eq!(ran.code, Some(0), "{ran:?}");
        let code = code_in(&ran.stdout);
        // The code is said once, in this output, and the stdout carries nothing else secret.
        assert!(secrets.leaks(&ran.stdout, false).is_empty(), "{ran:?}");
        assert!(secrets.leaks(&ran.stderr, true).is_empty(), "{ran:?}");
        codes.push(code);
    }
    secrets.codes.extend(codes.iter().cloned());
    let device = folder.only_device();
    let identity = std::fs::read(folder.device_file(&device, "identity.key")).unwrap();
    secrets.add("identity", &identity);
    let approved = fixture.approve(&name, codes.last().unwrap()).await;
    assert!(
        matches!(
            approved,
            arauto_hub::repo::approval::ApproveOutcome::Approved(_)
        ),
        "{approved:?}"
    );

    // The session.
    let leader = Copy::start(&folder.home);
    let public = folder.public(&device);
    eventually("the session", PATIENCE, || async {
        fixture.state_of_key(&public).await.0 == "active" && fixture.links_of(&public) == 1
    })
    .await;
    let first = std::fs::read(folder.device_file(&device, "credential.seal")).unwrap();
    secrets.add("first credential", &first);
    eventually("the leader answers", PATIENCE, || async {
        ask(&folder.home, r#"{"v":1,"op":"status"}"#)
            .await
            .is_some()
    })
    .await;
    answers.push(
        ask(&folder.home, r#"{"v":1,"op":"status"}"#)
            .await
            .unwrap()
            .to_string(),
    );
    answers.push(
        ask(
            &folder.home,
            r#"{"v":1,"op":"call","id":1,"tool":"arauto_status","args":{}}"#,
        )
        .await
        .unwrap()
        .to_string(),
    );
    outputs.push(("status".to_owned(), run(&folder.home, &["status"]).await));
    snapshots.extend(files_of(&folder.home));

    // The 24-hour swap.
    clock.advance(DAY_MS + SECOND_MS);
    eventually("the 24-hour swap", PATIENCE, || async {
        std::fs::read(folder.device_file(&device, "credential.seal")).is_ok_and(|now| now != first)
    })
    .await;
    let second = std::fs::read(folder.device_file(&device, "credential.seal")).unwrap();
    secrets.add("second credential", &second);
    snapshots.extend(files_of(&folder.home));

    // A copy of the folder, used while the true one is on: quarantine.
    let copy = Folder::new();
    std::fs::create_dir_all(copy.home.join("devices").join(&device)).unwrap();
    for file in ["identity.key", "credential.seal", "hub.json"] {
        std::fs::copy(
            folder.device_file(&device, file),
            copy.device_file(&device, file),
        )
        .unwrap();
    }
    std::fs::copy(folder.home.join("current"), copy.home.join("current")).unwrap();
    let copied = Copy::start(&copy.home);
    eventually("the quarantine", PATIENCE, || async {
        fixture.state_of_key(&public).await.0 == "quarantined"
    })
    .await;
    outputs.push((
        "status in quarantine".to_owned(),
        run(&folder.home, &["status"]).await,
    ));
    snapshots.extend(files_of(&folder.home));
    snapshots.extend(files_of(&copy.home));

    // The leave, through the leader: confirmed, the files go; Hermes is not there to remove
    // the registration.
    let ran = run(&folder.home, &["leave"]).await;
    assert_eq!(ran.code, Some(10), "{ran:?}");
    outputs.push(("leave".to_owned(), ran));
    outputs.push((
        "status after leave".to_owned(),
        run(&folder.home, &["status"]).await,
    ));

    for copy in [leader, copied] {
        let (_, stdout, stderr) = copy.stop().await;
        assert!(stdout.is_empty(), "the standard output of mcp is only MCP");
        logs.push(String::from_utf8_lossy(&stderr).into_owned());
    }
    // The log was really captured.
    assert!(
        logs.iter().any(|log| log.contains("leader")),
        "no log was captured: {logs:?}"
    );

    let mut found = Vec::new();
    for (what, ran) in &outputs {
        for leak in secrets.leaks(&format!("{}\n{}", ran.stdout, ran.stderr), true) {
            found.push(format!("{what}: {leak}"));
        }
    }
    for log in &logs {
        for leak in secrets.leaks(log, true) {
            found.push(format!("log: {leak}"));
        }
    }
    for answer in &answers {
        for leak in secrets.leaks(answer, true) {
            found.push(format!("socket: {leak}"));
        }
    }
    for (file, text) in &snapshots {
        for leak in secrets.leaks(text, true) {
            found.push(format!("{file}: {leak}"));
        }
    }
    assert!(found.is_empty(), "secrets found:\n{}", found.join("\n"));
}
