//! The canary of the key of the engine the installation brings
//! (`docs/decisoes/0052-a-instalacao-traz-o-motor.md`): the hub draws the key, seals it in the
//! row and writes only its digest in a file. The key in clear must not show up in the log of the
//! process, in an audit row, in an alert, in a file of the folder the engine reads, in the
//! output of the admin, or in an error.
//!
//! A binary of its own: the log of the process is captured from its first line, at every
//! level, and no other test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::admin::render::whatsapp as render;
use arauto_hub::channel::{self, Options, Served};
use arauto_hub::config::{ClientIpSource, InstallationEngineConfig};
use arauto_hub::engine::installation::{self, InstallationEngineError};
use arauto_hub::engine::{EngineAddress, EventsUrl};
use arauto_hub::vault::Vault;
use arauto_platform::FileLock;
use arauto_protocol::clock::SystemClock;
use arauto_testkit::hub::fixture::Fixture;
use arauto_testkit::hub::support;
use data_encoding::{BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};

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

/// The ways the key can be written down.
fn spellings(key: &str) -> Vec<String> {
    let bytes = key.as_bytes();
    vec![
        key.to_owned(),
        key.to_uppercase(),
        HEXLOWER.encode(bytes),
        HEXUPPER.encode(bytes),
        BASE64.encode(bytes),
        BASE64_NOPAD.encode(bytes),
        BASE64URL_NOPAD.encode(bytes),
        format!("{bytes:?}"),
        // The raw 32 bytes the 64 characters stand for, as a binary text.
        HEXLOWER
            .decode(bytes)
            .map(|raw| String::from_utf8_lossy(&raw).into_owned())
            .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|spelling| spelling.len() >= 8)
    .collect()
}

/// Where a secret shows up in `text`, if anywhere.
fn leaks(text: &str, secrets: &[String]) -> bool {
    secrets
        .iter()
        .any(|written| text.contains(written.as_str()))
}

#[test]
fn the_detector_sees_a_planted_key() {
    let key = installation::draw_key().unwrap();
    let secrets = spellings(key.expose());
    assert!(leaks(&format!("x {} y", key.expose()), &secrets));
    assert!(leaks(
        &format!("x {} y", BASE64.encode(key.expose().as_bytes())),
        &secrets
    ));
    assert!(leaks(&key.expose().to_uppercase(), &secrets));
    assert!(!leaks("a line of a log with no key", &secrets));
    // The digest is not the key.
    assert!(!leaks(&installation::digest_contents(&key), &secrets));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_plain_key_of_the_engine_of_the_installation_never_reaches_a_log_an_audit_row_or_the_file()
 {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let fixture = Fixture::stopped(
        support::install().await,
        support::settings(),
        Arc::new(SystemClock),
    )
    .await;
    let installed = &fixture.installation;
    let dir = tempfile::tempdir().unwrap();
    let recipe = InstallationEngineConfig {
        address: EngineAddress::parse("http://192.0.2.10:3000").unwrap(),
        events_url: EventsUrl::parse("http://hub.example:8444").unwrap(),
        digest_dir: dir.path().to_owned(),
    };
    let mut config = support::config(installed, ClientIpSource::Socket);
    config.installation_engine = Some(recipe.clone());
    let start = || async {
        channel::start(
            &config,
            Options {
                settings: support::settings(),
                purposes: Served,
                downloads: channel::Downloads::empty(),
            },
            Arc::new(SystemClock),
        )
        .await
        .unwrap()
    };

    // The first start seeds the row and writes the file; the second finds both; a third finds
    // the file lost and writes it again, and one more finds the lock taken.
    let running = start().await;
    running.stop().await;
    let running = start().await;
    running.stop().await;
    std::fs::remove_file(dir.path().join(installation::DIGEST_FILE)).unwrap();
    let running = start().await;
    running.stop().await;
    let mut errors = String::new();
    {
        let pool = arauto_hub::db::connect(&installed.url, arauto_hub::db::PoolOptions::default())
            .await
            .unwrap();
        let vault = Vault::open(&installed.data_dir).unwrap();
        let held = FileLock::try_acquire(&dir.path().join(installation::DIGEST_LOCK_FILE)).unwrap();
        let refused = installation::ensure_with(
            &pool,
            &arauto_hub::mark::Mark::new(&installed.data_dir),
            &vault,
            &recipe,
            1,
            Duration::from_millis(50),
            std::future::ready(()),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            refused,
            InstallationEngineError::DigestDirLocked { .. }
        ));
        errors.push_str(&format!("{refused}\n{refused:?}\n"));
        drop(held);
    }

    // The key, as the committed row holds it.
    let vault = Vault::open(&installed.data_dir).unwrap();
    let row = fixture.admin.engine_show().await.unwrap().unwrap();
    let key = row.secrets(&vault).unwrap().key.unwrap();
    let secrets = spellings(key.expose());
    assert!(secrets.iter().any(|spelling| spelling == key.expose()));

    // What a person or a machine can read.
    let mut readable: Vec<(&str, String)> = vec![
        ("the log", captured.text()),
        ("the errors", errors),
        (
            "the output of admin engine show",
            render::engine_show(Some(&row)).text,
        ),
        ("the debug of the row", format!("{row:?}")),
    ];
    let audit: Vec<String> = sqlx::query_scalar("SELECT row_to_json(a)::text FROM audit a")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    readable.push(("the audit rows", audit.join("\n")));
    let alerts: Vec<String> = sqlx::query_scalar("SELECT row_to_json(a)::text FROM alert a")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    readable.push(("the alert rows", alerts.join("\n")));
    let notifications: Vec<String> =
        sqlx::query_scalar("SELECT row_to_json(e)::text FROM engine e")
            .fetch_all(&fixture.pool)
            .await
            .unwrap();
    readable.push((
        "the engine row as the database prints it",
        notifications.join("\n"),
    ));
    // Every file of the folder the engine reads, as bytes.
    let mut files = 0;
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let entry = entry.unwrap();
        let bytes = std::fs::read(entry.path()).unwrap();
        readable.push((
            "a file of the folder",
            String::from_utf8_lossy(&bytes).into_owned(),
        ));
        files += 1;
    }
    assert_eq!(files, 2, "the digest file and the lock, and nothing else");
    for (what, text) in &readable {
        assert!(!leaks(text, &secrets), "the plain key is in {what}");
    }
    // The log did say something, with fixed words, and the file holds what it should.
    let log = captured.text();
    assert!(
        log.contains("the engine of the installation was checked"),
        "{log}"
    );
    let written = std::fs::read(dir.path().join(installation::DIGEST_FILE)).unwrap();
    assert_eq!(written, installation::digest_contents(&key).as_bytes());
}
