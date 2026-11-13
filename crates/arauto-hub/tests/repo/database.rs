//! The password of the database role: replaced by one the hub draws, kept in the data folder,
//! used by every opening of the database, never shown, and settled after a crash.
//!
//! Each test connects as a role of its own ([`crate::role`]): the role of the test server is
//! shared by every test. The test server trusts the connections of this machine, so a wrong
//! password is not refused there: what a password opens is checked against the verifier
//! PostgreSQL keeps, and the branch of the opening that follows a refused password is covered
//! by the tests of `db_password::next_step`, without a database.

use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::admin::{self, render};
use arauto_hub::audit::Actor;
use arauto_hub::config::{PasswordRotation, StorageConfig};
use arauto_hub::db::{self, DbError, PoolOptions, Rotated, RotationError, When};
use arauto_hub::db_password::{
    DATABASE_SECRET_FILE, DATABASE_SECRET_NEXT_FILE, DatabasePassword, DatabasePasswordError,
};
use arauto_hub::mark::Mark;
use arauto_hub::restore::{self, Standing};
use arauto_protocol::message::DeclaredText;
use data_encoding::{BASE32_NOPAD, BASE64, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};
use sqlx::ConnectOptions;
use sqlx::postgres::PgPool;

use crate::common::{NOW, TestDb};
use crate::role::{self, Role};

/// The password the database starts with: the provisional one of an installation.
/// Letters only: the options of a connection give it back percent-encoded.
const PROVISIONAL: &str = "ProvisionalPasswordOfTheInstallation";

const OPTIONS: PoolOptions = PoolOptions {
    size: 2,
    acquire_timeout: Duration::from_secs(10),
};

/// An installed hub whose database role is a role of its own.
struct Installed {
    db: TestDb,
    _dir: tempfile::TempDir,
    data_dir: PathBuf,
    role: Role,
    storage: StorageConfig,
}

impl Installed {
    async fn new(rotation: PasswordRotation) -> Self {
        let db = TestDb::empty().await;
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        admin::init(
            &db.pool,
            &data_dir,
            &DeclaredText::lossy("operator"),
            false,
            NOW,
        )
        .await
        .unwrap();
        let role = role::create(&db.pool, PROVISIONAL).await;
        let storage = StorageConfig {
            database_url: role.url(&db.url(), Some(PROVISIONAL)),
            data_dir: data_dir.clone(),
            allow_unsafe_db: false,
            password_rotation: rotation,
        };
        Self {
            db,
            _dir: dir,
            data_dir,
            role,
            storage,
        }
    }

    fn mark(&self) -> Mark {
        Mark::new(&self.data_dir)
    }

    fn kept(&self) -> Option<String> {
        std::fs::read_to_string(self.data_dir.join(DATABASE_SECRET_FILE)).ok()
    }

    fn next_exists(&self) -> bool {
        self.data_dir.join(DATABASE_SECRET_NEXT_FILE).exists()
    }

    async fn verifier(&self) -> String {
        self.role.verifier(&self.db.pool).await
    }

    async fn audit(&self) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT row_to_json(a)::text FROM audit a \
             WHERE action = 'database.password_replaced' ORDER BY seq",
        )
        .fetch_all(&self.db.pool)
        .await
        .unwrap()
    }

    async fn rotate(
        &self,
        storage: &StorageConfig,
        pools: &[&PgPool],
        when: When,
    ) -> Result<Rotated, RotationError> {
        db::rotate_password(storage, pools, &self.mark(), Actor::System, when, NOW).await
    }

    async fn end(self, pools: Vec<PgPool>) {
        for pool in pools {
            pool.close().await;
        }
        self.role.drop(&self.db.pool).await;
    }
}

/// The password a pool opens its next connections with.
fn password_of(pool: &PgPool) -> Option<String> {
    pool.connect_options()
        .to_url_lossy()
        .password()
        .map(str::to_owned)
}

/// The user a pool is connected as.
async fn user_of(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Every way the password could be written down.
fn spellings(password: &str) -> Vec<String> {
    let bytes = HEXLOWER.decode(password.as_bytes()).unwrap();
    vec![
        password.to_owned(),
        HEXUPPER.encode(&bytes),
        BASE64.encode(&bytes),
        BASE64URL_NOPAD.encode(&bytes),
        BASE32_NOPAD.encode(&bytes),
        BASE64.encode(password.as_bytes()),
        format!("{bytes:?}"),
        // Half of it would already help whoever guesses.
        password[..32].to_owned(),
        password[32..].to_owned(),
    ]
}

fn shows(text: &str, password: &str) -> bool {
    spellings(password)
        .iter()
        .any(|written| text.contains(written.as_str()))
}

fn assert_is_a_password(kept: &str) {
    assert_eq!(kept.len(), 64, "{} characters", kept.len());
    assert!(
        kept.bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "64 lowercase hexadecimal characters"
    );
}

#[tokio::test]
async fn the_password_of_the_role_is_replaced_by_one_nobody_sees_and_kept_in_the_data_folder() {
    let installed = Installed::new(PasswordRotation::Never).await;
    let pool = db::open(&installed.storage, OPTIONS).await.unwrap();
    assert_eq!(user_of(&pool).await, installed.role.name);
    let before = installed.verifier().await;
    assert!(role::opens(&before, PROVISIONAL));
    assert_eq!(password_of(&pool).as_deref(), Some(PROVISIONAL));

    let rotated = installed
        .rotate(&installed.storage, &[&pool], When::Asked)
        .await
        .unwrap();
    assert_eq!(rotated, Rotated::Replaced);

    // The file: 64 hexadecimal characters, the one of the next step gone.
    let kept = installed.kept().expect("the password is kept");
    assert_is_a_password(&kept);
    assert!(!installed.next_exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(installed.data_dir.join(DATABASE_SECRET_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o7777, 0o600);
    }
    // PostgreSQL has it, and no longer the provisional one.
    let after = installed.verifier().await;
    assert_ne!(after, before);
    assert!(role::opens(&after, &kept));
    assert!(!role::opens(&after, PROVISIONAL));

    // The pool opens its next connections with it, and so does every later opening.
    assert_eq!(password_of(&pool).as_deref(), Some(kept.as_str()));
    let target = installed.storage.database_target().unwrap();
    assert_eq!(
        target.options().to_url_lossy().password(),
        Some(kept.as_str())
    );
    let reopened = db::open(&installed.storage, OPTIONS).await.unwrap();
    assert_eq!(password_of(&reopened).as_deref(), Some(kept.as_str()));
    assert_eq!(user_of(&reopened).await, installed.role.name);

    // One security event, the mark following it; nothing of the password in it.
    let audit = installed.audit().await;
    assert_eq!(audit.len(), 1, "{audit:?}");
    assert!(audit[0].contains("\"security\":true"), "{}", audit[0]);
    assert!(audit[0].contains("\"asked\""), "{}", audit[0]);
    assert!(!shows(&audit[0], &kept), "{}", audit[0]);
    assert_eq!(
        restore::check(&installed.db.pool, &installed.mark())
            .await
            .unwrap(),
        Standing::Consistent
    );

    // Asked again, it is replaced again, by another one.
    let rotated = installed
        .rotate(&installed.storage, &[&pool, &reopened], When::Asked)
        .await
        .unwrap();
    assert_eq!(rotated, Rotated::Replaced);
    let again = installed.kept().unwrap();
    assert_is_a_password(&again);
    assert_ne!(again, kept);
    let verifier = installed.verifier().await;
    assert!(role::opens(&verifier, &again));
    assert!(!role::opens(&verifier, &kept));
    for pool in [&pool, &reopened] {
        assert_eq!(password_of(pool).as_deref(), Some(again.as_str()));
    }
    assert_eq!(installed.audit().await.len(), 2);

    installed.end(vec![pool, reopened]).await;
}

#[tokio::test]
async fn by_itself_the_hub_replaces_it_once_and_only_when_the_configuration_asks() {
    let installed = Installed::new(PasswordRotation::Never).await;
    let pool = db::open(&installed.storage, OPTIONS).await.unwrap();
    let before = installed.verifier().await;

    // Without the variable, the first passkey changes nothing.
    let rotated = installed
        .rotate(&installed.storage, &[&pool], When::FirstAccess)
        .await
        .unwrap();
    assert_eq!(rotated, Rotated::NotAsked);
    assert_eq!(installed.kept(), None);
    assert!(!installed.next_exists());
    assert_eq!(installed.verifier().await, before);
    assert!(installed.audit().await.is_empty());

    // With it, the first one replaces the password.
    let asked = StorageConfig {
        password_rotation: PasswordRotation::AtFirstAccess,
        ..installed.storage.clone()
    };
    let rotated = installed
        .rotate(&asked, &[&pool], When::FirstAccess)
        .await
        .unwrap();
    assert_eq!(rotated, Rotated::Replaced);
    let kept = installed.kept().unwrap();
    let replaced = installed.verifier().await;
    assert!(role::opens(&replaced, &kept));
    let audit = installed.audit().await;
    assert_eq!(audit.len(), 1);
    assert!(audit[0].contains("\"first_access\""), "{}", audit[0]);
    assert!(
        audit[0].contains("\"actor_kind\":\"system\""),
        "{}",
        audit[0]
    );

    // And never twice: the data folder already keeps one.
    for _ in 0..2 {
        let rotated = installed
            .rotate(&asked, &[&pool], When::FirstAccess)
            .await
            .unwrap();
        assert_eq!(rotated, Rotated::AlreadyReplaced);
    }
    assert_eq!(installed.kept().as_deref(), Some(kept.as_str()));
    assert_eq!(installed.verifier().await, replaced);
    assert_eq!(installed.audit().await.len(), 1);
    assert_eq!(password_of(&pool).as_deref(), Some(kept.as_str()));

    installed.end(vec![pool]).await;
}

#[tokio::test]
async fn a_replacement_the_database_never_had_is_dropped_at_the_next_opening() {
    let installed = Installed::new(PasswordRotation::Never).await;
    let before = installed.verifier().await;

    // A crash after the first step: the new password waits, and the database never had it.
    let never_given = DatabasePassword::generate().unwrap();
    std::fs::write(
        installed.data_dir.join(DATABASE_SECRET_NEXT_FILE),
        never_given.expose(),
    )
    .unwrap();
    // Until an opening settles it, no replacement writes over it: the database may have it.
    let early = db::connect(&installed.storage.database_url, OPTIONS)
        .await
        .unwrap();
    let refused = installed
        .rotate(&installed.storage, &[&early], When::Asked)
        .await
        .unwrap_err();
    assert!(matches!(refused, RotationError::Unsettled), "{refused:?}");
    assert_eq!(
        std::fs::read_to_string(installed.data_dir.join(DATABASE_SECRET_NEXT_FILE)).unwrap(),
        never_given.expose()
    );
    assert_eq!(installed.verifier().await, before);
    assert!(installed.audit().await.is_empty());
    early.close().await;

    let pool = db::open(&installed.storage, OPTIONS).await.unwrap();
    assert!(!installed.next_exists(), "the leftover went away");
    assert_eq!(installed.kept(), None);
    assert_eq!(password_of(&pool).as_deref(), Some(PROVISIONAL));
    assert_eq!(installed.verifier().await, before);

    // The same after a first replacement: the kept password stays the one in use.
    installed
        .rotate(&installed.storage, &[&pool], When::Asked)
        .await
        .unwrap();
    let kept = installed.kept().unwrap();
    let replaced = installed.verifier().await;
    std::fs::write(
        installed.data_dir.join(DATABASE_SECRET_NEXT_FILE),
        DatabasePassword::generate().unwrap().expose(),
    )
    .unwrap();
    let reopened = db::open(&installed.storage, OPTIONS).await.unwrap();
    assert!(!installed.next_exists());
    assert_eq!(installed.kept().as_deref(), Some(kept.as_str()));
    assert_eq!(password_of(&reopened).as_deref(), Some(kept.as_str()));
    assert_eq!(installed.verifier().await, replaced);

    installed.end(vec![pool, reopened]).await;
}

#[tokio::test]
async fn a_replacement_refused_by_the_security_write_leaves_the_password_in_use() {
    let installed = Installed::new(PasswordRotation::Never).await;
    let pool = db::open(&installed.storage, OPTIONS).await.unwrap();
    let before = installed.verifier().await;
    // The database reads as restored: the mark is ahead of every security event in it.
    installed.mark().advance(1_000_000).await.unwrap();

    let refused = installed
        .rotate(&installed.storage, &[&pool], When::Asked)
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            RotationError::Audit(arauto_hub::audit::AuditError::Restored(_))
        ),
        "{refused:?}"
    );
    // Nothing was done: no file, the same password, the pool as it was.
    assert_eq!(installed.kept(), None);
    assert!(!installed.next_exists());
    assert_eq!(installed.verifier().await, before);
    assert_eq!(password_of(&pool).as_deref(), Some(PROVISIONAL));
    assert!(installed.audit().await.is_empty());

    installed.end(vec![pool]).await;
}

#[tokio::test]
async fn a_damaged_password_file_stops_the_opening_without_being_shown() {
    let installed = Installed::new(PasswordRotation::Never).await;
    let damaged = "s3cr3t-written-by-hand-and-wrong";
    std::fs::write(installed.data_dir.join(DATABASE_SECRET_FILE), damaged).unwrap();
    let refused = db::open(&installed.storage, OPTIONS).await.unwrap_err();
    assert!(
        matches!(
            refused,
            DbError::Password(DatabasePasswordError::Damaged { .. })
        ),
        "{refused:?}"
    );
    let text = format!("{refused} {refused:?}");
    assert!(text.contains(DATABASE_SECRET_FILE), "{text}");
    assert!(!text.contains(damaged), "{text}");
    assert!(!text.contains(&damaged[..10]), "{text}");
    // The replacement refuses it too, before anything is done.
    let pool = db::connect(&installed.storage.database_url, OPTIONS)
        .await
        .unwrap();
    let before = installed.verifier().await;
    let refused = installed
        .rotate(&installed.storage, &[&pool], When::Asked)
        .await
        .unwrap_err();
    assert!(!refused.to_string().contains(damaged));
    assert!(!installed.next_exists());
    assert_eq!(installed.verifier().await, before);

    installed.end(vec![pool]).await;
}

/// Every line of the log of the thread of a test.
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

/// The log is captured at every level, statements of the database included, on the thread of
/// the test: the runtime of the test runs every task there. It starts after the role of the
/// test is created, whose statement carries the provisional password.
#[tokio::test(flavor = "current_thread")]
async fn no_log_line_and_no_audit_row_holds_the_password() {
    let installed = Installed::new(PasswordRotation::AtFirstAccess).await;

    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    // With one dispatcher only, tracing decides whether a place of the code logs by the
    // dispatcher of the thread that reaches it first, and the other tests of this process,
    // which log nowhere, reach the statements of the database too. A second dispatcher makes
    // it ask every one, this one included; the decisions already made are made again.
    let _every_dispatcher_is_asked =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    tracing::callsite::rebuild_interest_cache();

    let pool = db::open(&installed.storage, OPTIONS).await.unwrap();
    installed
        .rotate(&installed.storage, &[&pool], When::FirstAccess)
        .await
        .unwrap();
    let first = installed.kept().unwrap();
    // A replacement stopped halfway, settled by the next opening.
    std::fs::write(
        installed.data_dir.join(DATABASE_SECRET_NEXT_FILE),
        DatabasePassword::generate().unwrap().expose(),
    )
    .unwrap();
    let reopened = db::open(&installed.storage, OPTIONS).await.unwrap();
    sqlx::query("SELECT 1").execute(&reopened).await.unwrap();
    installed
        .rotate(&installed.storage, &[&pool, &reopened], When::Asked)
        .await
        .unwrap();
    let second = installed.kept().unwrap();
    // A refused one, in the log as an error.
    installed.mark().advance(1_000_000).await.unwrap();
    let refused = installed
        .rotate(&installed.storage, &[&pool], When::Asked)
        .await
        .unwrap_err();
    tracing::error!(error = %refused, "the test logs the refusal as the panel does");

    let log = captured.text();
    // The statements of the database reach this log: one carrying the password would be seen.
    assert!(log.contains("SET lock_timeout"), "{log}");
    assert!(
        !log.contains("ALTER ROLE"),
        "the statement that sets the password was logged"
    );
    let audit = installed.audit().await.join("\n");
    assert_eq!(installed.audit().await.len(), 2);
    for password in [&first, &second] {
        assert!(!shows(&log, password), "the log holds the password");
        assert!(!shows(&audit, password), "the audit holds the password");
    }
    assert!(
        !log.contains(PROVISIONAL),
        "the log holds the provisional password"
    );

    installed.end(vec![pool, reopened]).await;
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

#[test]
fn the_screen_of_the_command_takes_no_password() {
    let report = render::database_password(std::path::Path::new("/var/lib/arauto/database.secret"));
    assert!(report.text.contains("database.secret"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_command_replaces_the_password_and_never_shows_it() {
    let installed = Installed::new(PasswordRotation::Never).await;
    let before = installed.verifier().await;
    let ran = tokio::task::block_in_place(|| {
        Command::new(env!("CARGO_BIN_EXE_arauto-hub"))
            .args(["admin", "database", "rotate-password"])
            .env_remove("ARAUTO_ALLOW_UNSAFE_DB")
            .env_remove("ARAUTO_DATABASE_PASSWORD")
            .env_remove("ARAUTO_PANEL_URL")
            .env_remove("RUST_LOG")
            .env(
                "ARAUTO_DATABASE_URL",
                installed.storage.database_url.expose(),
            )
            .env("ARAUTO_DATA_DIR", &installed.data_dir)
            .env("USER", "tester")
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    });
    let out = String::from_utf8(ran.stdout).unwrap();
    let err = String::from_utf8(ran.stderr).unwrap();
    assert!(ran.status.success(), "{out}\n{err}");

    let kept = installed.kept().expect("the command keeps the password");
    assert_is_a_password(&kept);
    let after = installed.verifier().await;
    assert_ne!(after, before);
    assert!(role::opens(&after, &kept));
    assert!(out.contains(DATABASE_SECRET_FILE), "{out}");
    assert!(out.contains("never shown"), "{out}");
    for text in [&out, &err] {
        assert!(
            !shows(text, &kept),
            "the command showed the password: {text}"
        );
        assert!(!text.contains(PROVISIONAL), "{text}");
    }
    let audit = installed.audit().await;
    assert_eq!(audit.len(), 1);
    assert!(
        audit[0].contains("\"actor_kind\":\"admin\""),
        "{}",
        audit[0]
    );
    assert!(
        audit[0].contains("\"actor_note\":\"tester\""),
        "{}",
        audit[0]
    );
    assert!(!shows(&audit[0], &kept));

    // The next command opens the database with the kept password.
    let pool = db::open(&installed.storage, OPTIONS).await.unwrap();
    assert_eq!(password_of(&pool).as_deref(), Some(kept.as_str()));
    installed.end(vec![pool]).await;
}
