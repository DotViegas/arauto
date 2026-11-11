//! The canary of secrets (`docs/15-mvp.md`, section 7): a whole flow runs with every log of
//! the process captured, from the invite to the session, the 24-hour swap and the leave. No
//! invite, credential, verification code or key may appear in the log, in the errors the log
//! carries, in the audit or in the alerts.
//!
//! A binary of its own: the log of the process is captured from its first line, at every
//! level, and no other test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

#[allow(dead_code)]
#[path = "channel/support.rs"]
mod support;

#[path = "channel/fixture.rs"]
mod fixture;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::vault::MASTER_KEY_FILE;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::message::{CallOutcome, LeftStatus};
use arauto_protocol::tools::ARAUTO_STATUS;
use arauto_testkit::device::{self, Enrolled, Faults};
use data_encoding::{BASE32_NOPAD, BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};
use serde_json::json;

use crate::fixture::Fixture;

/// Every line of the log of the process.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// The ways a secret of bytes can be written down.
fn spellings(bytes: &[u8]) -> Vec<String> {
    vec![
        HEXLOWER.encode(bytes),
        HEXUPPER.encode(bytes),
        BASE64.encode(bytes),
        BASE64_NOPAD.encode(bytes),
        BASE64URL_NOPAD.encode(bytes),
        BASE32_NOPAD.encode(bytes),
        BASE32_NOPAD.encode(bytes).to_ascii_lowercase(),
        format!("{bytes:?}"),
    ]
}

/// Where a secret shows up in `text`, if anywhere.
fn leaks(text: &str, secrets: &[(String, String)]) -> Vec<String> {
    secrets
        .iter()
        .filter(|(_, written)| text.contains(written.as_str()))
        .map(|(what, _)| what.clone())
        .collect()
}

/// A verification code shows up as `123-456`, or as six digits standing alone.
fn has_code(text: &str, code: &str) -> bool {
    let digits: String = code.chars().filter(char::is_ascii_digit).collect();
    if text.contains(code) {
        return true;
    }
    text.match_indices(digits.as_str()).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + digits.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

#[test]
fn the_detector_sees_a_planted_secret() {
    let secret = [0x5au8; 32];
    let mut secrets = Vec::new();
    for written in spellings(&secret) {
        secrets.push(("planted".to_owned(), written.clone()));
        assert_eq!(leaks(&format!("x {written} y"), &secrets), ["planted"]);
        secrets.clear();
    }
    assert!(has_code("the code 028-213 was typed", "028-213"));
    assert!(has_code(r#"{"code":"028213"}"#, "028-213"));
    assert!(!has_code("at 1800000282130", "028-213"));
}

#[tokio::test(flavor = "multi_thread")]
async fn no_secret_reaches_the_log_the_errors_the_audit_or_the_alerts() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let clock = ManualClock::new(SystemClock.now_ms());
    let mut chosen = fixture::settings();
    chosen.sweep_interval_ms = 100;
    let fixture = Fixture::with_clock(chosen, Arc::new(clock.clone())).await;
    let mut secrets: Vec<(String, String)> = Vec::new();
    let mut codes: Vec<String> = Vec::new();

    // The keys: the master key, the key of the hub.
    let master = std::fs::read(fixture.installation.data_dir.join(MASTER_KEY_FILE)).unwrap();
    for written in spellings(&master) {
        secrets.push(("master key".to_owned(), written));
    }
    let vault = arauto_hub::vault::Vault::open(&fixture.installation.data_dir).unwrap();
    let hub = arauto_hub::admin::hub_keypair(&fixture.pool, &vault)
        .await
        .unwrap();
    for written in spellings(hub.private().expose()) {
        secrets.push(("hub key".to_owned(), written));
    }

    // An invite that is refused, then one that works.
    let name = fixture.hermes().await;
    let device = fixture.device();
    for written in spellings(device.identity().private().expose()) {
        secrets.push(("identity".to_owned(), written));
    }
    let stranger = Invite::generate().unwrap();
    secrets.push(("refused invite".to_owned(), stranger.reveal().to_string()));
    assert_eq!(
        device.enroll(&stranger, device::declared()).await.unwrap(),
        Enrolled::Refused(ErrorCode::Ar101)
    );
    let invite = fixture.invite(&name).await;
    secrets.push(("invite".to_owned(), invite.reveal().to_string()));
    let Enrolled::Code(first) = device.enroll(&invite, device::declared()).await.unwrap() else {
        panic!("enrolled");
    };
    // The same request, enrolled again: a new code; the first one is no longer valid.
    let Enrolled::Code(code) = device.enroll(&invite, device::declared()).await.unwrap() else {
        panic!("enrolled again");
    };
    codes.push(first.to_string());
    codes.push(code.to_string());
    // A wrong code typed, then the right one.
    let wrong = if code.value() == 0 {
        "000-001"
    } else {
        "000-000"
    };
    let _ = fixture
        .admin
        .approve(&name, wrong, None, fixture.now())
        .await;
    fixture.approve(&name, &code.to_string()).await;

    let mut session = device
        .channel(Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    let first_credential = device.disk().credential().unwrap();
    for written in spellings(first_credential.expose()) {
        secrets.push(("credential".to_owned(), written));
    }
    assert!(matches!(
        session.call(ARAUTO_STATUS, json!({})).await.unwrap(),
        CallOutcome::Ok(_)
    ));
    // The 24-hour swap.
    clock.advance(24 * 3_600_000);
    session.idle(Duration::from_secs(1)).await.unwrap();
    assert_eq!(session.rotated.len(), 1);
    let second = device.disk().credential().unwrap();
    for written in spellings(second.expose()) {
        secrets.push(("rotated credential".to_owned(), written));
    }
    // A copy presented with the true one on: quarantine, alert.
    let copy = device.copy_files();
    let (_, mut other) = fixture.connected().await;
    for written in spellings(other.device().disk().credential().unwrap().expose()) {
        secrets.push(("other credential".to_owned(), written));
    }
    let left = other.leave().await;
    assert!(matches!(left, Ok(LeftStatus::Closed)), "{left:?}");
    let keep = tokio::spawn(async move { session.ended(Duration::from_secs(30)).await });
    assert_eq!(
        copy.channel(Faults::default()).await.unwrap().closing(),
        Some(ErrorCode::Ar501)
    );
    keep.await.unwrap().unwrap();
    // Let the sweep write the refused enrollments of the minute that ended.
    clock.advance(120_000);
    tokio::time::sleep(Duration::from_millis(600)).await;

    let log = captured.text();
    assert!(
        log.len() > 1_000,
        "the log is captured (it has {} bytes)",
        log.len()
    );
    let mut stored = String::new();
    for table in [
        "audit",
        "alert",
        "unknown_device",
        "channel_connection",
        "invite",
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t"
        )))
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
        stored.push_str(&rows.join("\n"));
        stored.push('\n');
    }
    let refused: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit WHERE action = 'invite.refused'")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert!(
        refused >= 1,
        "the refused invite is in the audit, without itself"
    );
    for (place, text) in [("log", &log), ("audit and alerts", &stored)] {
        let found = leaks(text, &secrets);
        assert!(found.is_empty(), "{place} holds: {found:?}");
        for code in &codes {
            assert!(!has_code(text, code), "{place} holds a verification code");
        }
    }
}
