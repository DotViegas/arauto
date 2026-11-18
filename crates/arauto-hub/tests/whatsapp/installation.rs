//! The engine brought by the installation (`docs/decisoes/0052-a-instalacao-traz-o-motor.md`),
//! at the level of the repository: the origin mark, the seed that is safe against a second
//! writer, the refusal of a typed key, and what happens when an admin points the organization
//! at another engine.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use arauto_hub::admin::AdminError;
use arauto_hub::admin::render::whatsapp as render;
use arauto_hub::channel;
use arauto_hub::config::{ClientIpSource, InstallationEngineConfig};
use arauto_hub::db::{self, PoolOptions};
use arauto_hub::engine::installation::{
    self, Ensured, FileOutcome, InstallationEngineError, Settled,
};
use arauto_hub::engine::{ApiKey, Cause, EngineAddress, EngineError, EngineVersion, EventsUrl};
use arauto_hub::mark::Mark;
use arauto_hub::repo::engine::{self, EngineOrigin, Seeded, SetOutcome};
use arauto_hub::vault::Vault;
use arauto_platform::FileLock;
use arauto_protocol::clock::SystemClock;

use crate::fixture::{self, Fixture};
use crate::support;
use crate::world::{KEY, World};

const DRAWN: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const ENGINE_URL: &str = "http://192.0.2.10:3000";
const EVENTS_URL: &str = "http://hub.example:8444";
const OTHER_URL: &str = "http://192.0.2.20:3000";

struct Seed {
    fixture: Fixture,
    vault: Vault,
    mark: Mark,
}

impl Seed {
    /// A hub with no engine row.
    async fn new() -> Self {
        let fixture = Fixture::new(fixture::settings()).await;
        let vault = Vault::open(&fixture.installation.data_dir).unwrap();
        let mark = Mark::new(&fixture.installation.data_dir);
        Self {
            fixture,
            vault,
            mark,
        }
    }

    async fn seed(&self, key: &str) -> Seeded {
        engine::seed_installation(
            &self.fixture.pool,
            &self.mark,
            self.fixture.installation.organization,
            &self.vault,
            &EngineAddress::parse(ENGINE_URL).unwrap(),
            &EventsUrl::parse(EVENTS_URL).unwrap(),
            &ApiKey::parse(key).unwrap(),
            self.fixture.now(),
        )
        .await
        .unwrap()
    }

    async fn row(&self) -> engine::EngineRow {
        engine::get(&self.fixture.pool, self.fixture.installation.organization)
            .await
            .unwrap()
            .unwrap()
    }

    async fn audit_count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM audit WHERE action LIKE 'engine.%'")
            .fetch_one(&self.fixture.pool)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn seeding_writes_one_marked_row_with_the_key_and_two_audit_rows() {
    let seed = Seed::new().await;
    let Seeded::Created(id) = seed.seed(DRAWN).await else {
        panic!("the first seed creates the row");
    };
    let row = seed.row().await;
    assert_eq!(row.id, id);
    assert_eq!(row.origin(), EngineOrigin::Installation);
    assert_eq!(row.address, ENGINE_URL);
    assert_eq!(row.events_url, EVENTS_URL);
    assert_eq!(row.key_refused_since, None);
    // The key opens from the committed row, bound to the id that was inserted.
    let secrets = row.secrets(&seed.vault).unwrap();
    assert_eq!(secrets.key.unwrap().expose(), DRAWN);
    // Never in clear in the database or in Debug.
    let stored: String = sqlx::query_scalar("SELECT row_to_json(e)::text FROM engine e")
        .fetch_one(&seed.fixture.pool)
        .await
        .unwrap();
    assert!(!stored.contains(DRAWN));
    assert!(!format!("{row:?}").contains(DRAWN));
    // Exactly two audit rows, by the system, security events, with the origin and no secret.
    let audit: Vec<(String, String, bool, String)> = sqlx::query_as(
        "SELECT action, actor_kind, security, details::text FROM audit \
         WHERE action LIKE 'engine.%' ORDER BY seq",
    )
    .fetch_all(&seed.fixture.pool)
    .await
    .unwrap();
    assert_eq!(audit.len(), 2, "{audit:?}");
    assert_eq!(audit[0].0, "engine.configured");
    assert_eq!(audit[1].0, "engine.key_changed");
    for (_, actor, security, details) in &audit {
        assert_eq!(actor, "system");
        assert!(*security);
        assert!(details.contains("installation"), "{details}");
        assert!(!details.contains(DRAWN));
    }
    // A second call finds the row and writes nothing, and keeps the first key.
    let before = seed.audit_count().await;
    assert_eq!(
        seed.seed("ffffffffffffffffffffffffffffffff").await,
        Seeded::AlreadyThere
    );
    assert_eq!(seed.audit_count().await, before);
    let again = seed.row().await;
    assert_eq!(again.api_key_enc, row.api_key_enc);
    assert_eq!(again.config_version, 1);
    assert!(
        render::engine_show(Some(&again))
            .text
            .contains("brought by the installation  yes")
    );
}

#[tokio::test]
async fn seeding_does_not_replace_an_engine_an_admin_set() {
    let world = World::new().await;
    let before = world.fixture.admin.engine_show().await.unwrap().unwrap();
    let outcome = engine::seed_installation(
        &world.fixture.pool,
        &Mark::new(&world.fixture.installation.data_dir),
        world.fixture.installation.organization,
        &world.vault,
        &EngineAddress::parse(ENGINE_URL).unwrap(),
        &EventsUrl::parse(EVENTS_URL).unwrap(),
        &ApiKey::parse(DRAWN).unwrap(),
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, Seeded::AlreadyThere);
    let after = world.fixture.admin.engine_show().await.unwrap().unwrap();
    assert_eq!(after, before);
    assert_eq!(after.origin(), EngineOrigin::Admin);
    assert!(
        render::engine_show(Some(&after))
            .text
            .contains("brought by the installation  no")
    );
}

#[tokio::test]
async fn two_seeds_at_once_write_one_row_and_two_audit_rows() {
    let seed = Seed::new().await;
    for round in 0..20_i64 {
        let (a, b) = tokio::join!(
            seed.seed("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            seed.seed("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        let created = [a, b]
            .iter()
            .filter(|outcome| matches!(outcome, Seeded::Created(_)))
            .count();
        assert_eq!(created, 1, "round {round}: {a:?} {b:?}");
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM engine")
            .fetch_one(&seed.fixture.pool)
            .await
            .unwrap();
        assert_eq!(rows, 1, "round {round}");
        // The key that opens is one of the two that were drawn, whole.
        let key = seed.row().await.secrets(&seed.vault).unwrap().key.unwrap();
        assert!(
            key.expose().starts_with("aaaa") || key.expose().starts_with("bbbb"),
            "round {round}"
        );
        assert_eq!(seed.audit_count().await, 2 * (round + 1), "round {round}");
        // Start the next round from an organization with no engine.
        sqlx::query("DELETE FROM engine")
            .execute(&seed.fixture.pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn set_key_on_the_installation_engine_is_refused_and_writes_nothing() {
    let seed = Seed::new().await;
    seed.seed(DRAWN).await;
    let before = seed.row().await;
    let audits = seed.audit_count().await;
    let refused = seed
        .fixture
        .admin
        .engine_set_key(&seed.vault, KEY, seed.fixture.now())
        .await
        .unwrap_err();
    assert!(
        matches!(refused, AdminError::InstallationEngineKey),
        "{refused:?}"
    );
    assert!(!refused.to_string().contains(KEY));
    let after = seed.row().await;
    assert_eq!(after.api_key_enc, before.api_key_enc);
    assert_eq!(after.config_version, before.config_version);
    assert_eq!(after.origin(), EngineOrigin::Installation);
    assert_eq!(seed.audit_count().await, audits);
    assert_eq!(
        after.secrets(&seed.vault).unwrap().key.unwrap().expose(),
        DRAWN
    );
}

#[tokio::test]
async fn the_database_refuses_an_installation_row_without_a_key() {
    let world = World::new().await;
    // The admin row is valid with or without a key; marking it as the installation's is not,
    // while its key is missing.
    sqlx::query("UPDATE engine SET api_key_enc = NULL")
        .execute(&world.fixture.pool)
        .await
        .unwrap();
    let refused = sqlx::query("UPDATE engine SET origin = 'installation'")
        .execute(&world.fixture.pool)
        .await;
    assert!(refused.is_err());
    // And the key cannot be taken from an installation row.
    let seed = Seed::new().await;
    seed.seed(DRAWN).await;
    let refused = sqlx::query("UPDATE engine SET api_key_enc = NULL")
        .execute(&seed.fixture.pool)
        .await;
    assert!(refused.is_err());
    // A word that is neither origin is refused.
    let refused = sqlx::query("UPDATE engine SET origin = 'other'")
        .execute(&seed.fixture.pool)
        .await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn the_database_refuses_a_second_installation_engine() {
    let seed = Seed::new().await;
    seed.seed(DRAWN).await;
    // A second organization with its own engine row marked as the installation's.
    let second = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'second', 'active', '\\x00'::bytea, 0)",
    )
    .bind(second)
    .execute(&seed.fixture.pool)
    .await
    .unwrap();
    let row = seed.row().await;
    let inserted = sqlx::query(
        "INSERT INTO engine (id, organization_id, kind, origin, address, events_url, \
                             api_key_enc, events_secret_enc, config_version, created_at, \
                             changed_at) \
         VALUES ($1, $2, 'waha', 'installation', 'http://192.0.2.30:3000', \
                 'http://hub.example:8444', $3, $4, 1, 0, 0)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(second)
    .bind(row.api_key_enc.clone().unwrap())
    .bind(row.events_secret_enc.clone())
    .execute(&seed.fixture.pool)
    .await;
    assert!(inserted.is_err(), "a second installation engine was taken");
    // The same row marked as an admin's is fine: the index is only about the installation's.
    let inserted = sqlx::query(
        "INSERT INTO engine (id, organization_id, kind, origin, address, events_url, \
                             api_key_enc, events_secret_enc, config_version, created_at, \
                             changed_at) \
         VALUES ($1, $2, 'waha', 'admin', 'http://192.0.2.30:3000', \
                 'http://hub.example:8444', NULL, $3, 1, 0, 0)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(second)
    .bind(row.events_secret_enc.clone())
    .execute(&seed.fixture.pool)
    .await;
    assert!(inserted.is_ok(), "{inserted:?}");
}

#[tokio::test]
async fn setting_another_address_leaves_the_installation_engine_and_clears_its_key() {
    let seed = Seed::new().await;
    seed.seed(DRAWN).await;
    let before = seed.row().await;
    let now = seed.fixture.now();
    let events = format!("http://{}", seed.fixture.running().internal_addr());
    let outcome = seed
        .fixture
        .admin
        .engine_set(&seed.vault, OTHER_URL, &events, now)
        .await
        .unwrap();
    assert!(matches!(outcome, SetOutcome::LeftInstallation(id) if id == before.id));
    let text = render::engine_set(outcome).text;
    assert!(text.contains("set-key"), "{text}");
    let after = seed.row().await;
    assert_eq!(after.origin(), EngineOrigin::Admin);
    assert_eq!(after.api_key_enc, None);
    assert_eq!(after.address, OTHER_URL);
    assert_eq!(after.config_version, before.config_version + 1);
    let flagged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit WHERE action = 'engine.configured' AND actor_kind = 'admin' \
           AND details @> '{\"left_installation\": true}'",
    )
    .fetch_one(&seed.fixture.pool)
    .await
    .unwrap();
    assert_eq!(flagged, 1);
    // The admin now types the key of the new engine.
    seed.fixture
        .admin
        .engine_set_key(&seed.vault, KEY, now)
        .await
        .unwrap();
    assert!(seed.row().await.api_key_enc.is_some());
}

#[tokio::test]
async fn changing_only_the_events_address_keeps_the_installation_engine_and_its_key() {
    let seed = Seed::new().await;
    seed.seed(DRAWN).await;
    let before = seed.row().await;
    let outcome = seed
        .fixture
        .admin
        .engine_set(
            &seed.vault,
            ENGINE_URL,
            "http://other-hub.example:8444",
            seed.fixture.now(),
        )
        .await
        .unwrap();
    assert!(matches!(outcome, SetOutcome::Changed(_)), "{outcome:?}");
    let after = seed.row().await;
    assert_eq!(after.origin(), EngineOrigin::Installation);
    assert_eq!(after.api_key_enc, before.api_key_enc);
    assert_eq!(after.events_url, "http://other-hub.example:8444");
}

#[tokio::test]
async fn a_refusal_of_the_key_is_noted_once_per_episode_and_cleared_by_the_next_answer() {
    let seed = Seed::new().await;
    seed.seed(DRAWN).await;
    let organization = seed.fixture.installation.organization;
    let id = seed.row().await.id;
    let pool = &seed.fixture.pool;
    let now = seed.fixture.now();
    assert_eq!(
        engine::key_refused(pool, organization, id, now)
            .await
            .unwrap(),
        Some(EngineOrigin::Installation)
    );
    assert_eq!(
        engine::key_refused(pool, organization, id, now + 1)
            .await
            .unwrap(),
        None
    );
    assert_eq!(seed.row().await.key_refused_since, Some(now));
    // The first answer that works ends the episode.
    let version = EngineVersion {
        version: "2026.10.1".to_owned(),
        engine: "GOWS".to_owned(),
        tier: "CORE".to_owned(),
    };
    engine::record_version(pool, organization, id, &version, now + 2)
        .await
        .unwrap();
    assert_eq!(seed.row().await.key_refused_since, None);
    assert_eq!(
        engine::key_refused(pool, organization, id, now + 3)
            .await
            .unwrap(),
        Some(EngineOrigin::Installation)
    );
    // A change of the configuration ends it too.
    seed.fixture
        .admin
        .engine_new_events_secret(&seed.vault, now + 4)
        .await
        .unwrap();
    assert_eq!(seed.row().await.key_refused_since, None);
    // A row that is not there notes nothing.
    assert_eq!(
        engine::key_refused(pool, organization, uuid::Uuid::new_v4(), now)
            .await
            .unwrap(),
        None
    );
}

// ---------------------------------------------------------------------------- ensure and the file

/// The folder the hub writes the digest file in, as the volume of the composition is.
fn folder() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn recipe(dir: &Path, url: &str) -> InstallationEngineConfig {
    InstallationEngineConfig {
        address: EngineAddress::parse(url).unwrap(),
        events_url: EventsUrl::parse(EVENTS_URL).unwrap(),
        digest_dir: dir.to_owned(),
    }
}

fn digest_of(key: &str) -> String {
    installation::digest_contents(&ApiKey::parse(key).unwrap())
}

fn file_in(dir: &Path) -> Option<Vec<u8>> {
    std::fs::read(dir.join(installation::DIGEST_FILE)).ok()
}

/// Every name in the folder, sorted.
fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

impl Seed {
    async fn ensure(&self, dir: &Path, url: &str) -> Ensured {
        installation::ensure(
            &self.fixture.pool,
            &self.mark,
            &self.vault,
            &recipe(dir, url),
            self.fixture.now(),
        )
        .await
        .unwrap()
    }

    /// The key of the committed row, opened.
    async fn committed_key(&self) -> String {
        self.row()
            .await
            .secrets(&self.vault)
            .unwrap()
            .key
            .unwrap()
            .expose()
            .to_owned()
    }
}

fn settled(origin: Option<EngineOrigin>, file: FileOutcome) -> Settled {
    Settled { origin, file }
}

#[tokio::test]
async fn an_empty_installation_gets_a_seeded_row_and_the_digest_of_its_committed_key() {
    let seed = Seed::new().await;
    let dir = folder();
    let ensured = seed.ensure(dir.path(), ENGINE_URL).await;
    assert_eq!(
        ensured,
        Ensured::Settled {
            seeded: true,
            settled: settled(Some(EngineOrigin::Installation), FileOutcome::Written),
        }
    );
    let row = seed.row().await;
    assert_eq!(row.origin(), EngineOrigin::Installation);
    assert_eq!(row.address, ENGINE_URL);
    assert_eq!(seed.audit_count().await, 2);
    // The file is the digest of the key that opens from the committed row, and nothing else.
    let key = seed.committed_key().await;
    assert_eq!(key.len(), 64);
    let written = file_in(dir.path()).unwrap();
    assert_eq!(written, digest_of(&key).as_bytes());
    assert!(installation::is_digest_file_contents(&written));
    assert!(!String::from_utf8(written).unwrap().contains(&key));
    // Only the file and the lock are in the folder.
    assert_eq!(
        names_in(dir.path()),
        [installation::DIGEST_LOCK_FILE, installation::DIGEST_FILE]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join(installation::DIGEST_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o644);
    }
    // The next start finds everything right: the row, the key and the file stay.
    let before = seed.row().await;
    let again = seed.ensure(dir.path(), ENGINE_URL).await;
    assert_eq!(
        again,
        Ensured::Settled {
            seeded: false,
            settled: settled(Some(EngineOrigin::Installation), FileOutcome::Unchanged),
        }
    );
    assert_eq!(seed.row().await, before);
    assert_eq!(seed.audit_count().await, 2);
    // A file that was damaged is repaired from the row, not from a new key, and the leftover
    // of an interrupted write goes.
    std::fs::write(dir.path().join(installation::DIGEST_FILE), b"damaged").unwrap();
    let leftover = dir.path().join(".api-key.sha512.0123456789abcdef.tmp");
    std::fs::write(&leftover, b"half a write").unwrap();
    let repaired = seed.ensure(dir.path(), ENGINE_URL).await;
    assert_eq!(
        repaired,
        Ensured::Settled {
            seeded: false,
            settled: settled(Some(EngineOrigin::Installation), FileOutcome::Written),
        }
    );
    assert_eq!(file_in(dir.path()).unwrap(), digest_of(&key).as_bytes());
    assert!(!leftover.exists());
    assert_eq!(seed.committed_key().await, key);
}

#[tokio::test]
async fn an_admin_engine_gets_no_digest_file_and_an_old_one_is_removed() {
    // The world has an engine an admin set, with a key the admin typed.
    let world = World::new().await;
    let before = world.fixture.admin.engine_show().await.unwrap().unwrap();
    assert_eq!(before.origin(), EngineOrigin::Admin);
    let dir = folder();
    let mark = Mark::new(&world.fixture.installation.data_dir);
    // A file left by an earlier life of the installation, holding the digest of that very key.
    std::fs::write(dir.path().join(installation::DIGEST_FILE), digest_of(KEY)).unwrap();
    let config = recipe(dir.path(), ENGINE_URL);
    let ensured = installation::ensure(
        &world.fixture.pool,
        &mark,
        &world.vault,
        &config,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(
        ensured,
        Ensured::Settled {
            seeded: false,
            settled: settled(Some(EngineOrigin::Admin), FileOutcome::Removed),
        }
    );
    assert_eq!(file_in(dir.path()), None);
    // The row of the admin is as it was, to the byte.
    let after = world.fixture.admin.engine_show().await.unwrap().unwrap();
    assert_eq!(after, before);
    // And a start with no file stays with no file.
    for _ in 0..2 {
        let again = installation::ensure(
            &world.fixture.pool,
            &mark,
            &world.vault,
            &config,
            world.now(),
        )
        .await
        .unwrap();
        assert_eq!(
            again,
            Ensured::Settled {
                seeded: false,
                settled: settled(Some(EngineOrigin::Admin), FileOutcome::Absent),
            }
        );
        assert_eq!(file_in(dir.path()), None);
    }
}

#[tokio::test]
async fn the_recipe_address_never_replaces_an_admin_address() {
    let world = World::new().await;
    let before = world.fixture.admin.engine_show().await.unwrap().unwrap();
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(&world.fixture.pool)
        .await
        .unwrap();
    let dir = folder();
    // The recipe names an address that is not the admin's.
    assert_ne!(before.address, OTHER_URL);
    installation::ensure(
        &world.fixture.pool,
        &Mark::new(&world.fixture.installation.data_dir),
        &world.vault,
        &recipe(dir.path(), OTHER_URL),
        world.now(),
    )
    .await
    .unwrap();
    let after = world.fixture.admin.engine_show().await.unwrap().unwrap();
    assert_eq!(after, before);
    let now: i64 = sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(&world.fixture.pool)
        .await
        .unwrap();
    assert_eq!(now, audits);
}

#[tokio::test]
async fn the_recipe_address_never_replaces_the_seeded_one() {
    let seed = Seed::new().await;
    let dir = folder();
    seed.ensure(dir.path(), ENGINE_URL).await;
    let before = seed.row().await;
    let audits = seed.audit_count().await;
    let key = seed.committed_key().await;
    // The recipe now says another address: the row, the key, the file and the audit stay.
    let again = seed.ensure(dir.path(), OTHER_URL).await;
    assert_eq!(
        again,
        Ensured::Settled {
            seeded: false,
            settled: settled(Some(EngineOrigin::Installation), FileOutcome::Unchanged),
        }
    );
    let after = seed.row().await;
    assert_eq!(after, before);
    assert_eq!(after.address, ENGINE_URL);
    assert_eq!(seed.audit_count().await, audits);
    assert_eq!(file_in(dir.path()).unwrap(), digest_of(&key).as_bytes());
}

#[tokio::test]
async fn more_than_one_organization_touches_nothing() {
    let seed = Seed::new().await;
    // A second organization, as the database allows one.
    let second = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at)          VALUES ($1, 'second', 'active', $2, $3)",
    )
    .bind(second)
    .bind(seed.vault.generate_organization_key(second).unwrap())
    .bind(seed.fixture.now())
    .execute(&seed.fixture.pool)
    .await
    .unwrap();
    let dir = folder();
    let stale = dir.path().join(installation::DIGEST_FILE);
    std::fs::write(&stale, b"left alone").unwrap();
    let ensured = seed.ensure(dir.path(), ENGINE_URL).await;
    assert_eq!(ensured, Ensured::Skipped);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM engine")
        .fetch_one(&seed.fixture.pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(std::fs::read(&stale).unwrap(), b"left alone");
    assert_eq!(names_in(dir.path()), [installation::DIGEST_FILE]);
}

#[tokio::test(flavor = "multi_thread")]
async fn two_hubs_starting_at_once_seed_one_engine_and_write_one_file() {
    let seed = Seed::new().await;
    let other = db::connect(
        &seed.fixture.installation.url,
        PoolOptions {
            size: 3,
            acquire_timeout: Duration::from_secs(10),
        },
    )
    .await
    .unwrap();
    let dir = folder();
    let config = recipe(dir.path(), ENGINE_URL);
    for round in 0..50_i64 {
        sqlx::query("DELETE FROM engine")
            .execute(&seed.fixture.pool)
            .await
            .unwrap();
        let _ = std::fs::remove_file(dir.path().join(installation::DIGEST_FILE));
        let now = seed.fixture.now();
        let (a, b) = tokio::join!(
            installation::ensure(&seed.fixture.pool, &seed.mark, &seed.vault, &config, now),
            installation::ensure(&other, &seed.mark, &seed.vault, &config, now),
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        let seeded = [a, b]
            .iter()
            .filter(|ensured| matches!(ensured, Ensured::Settled { seeded: true, .. }))
            .count();
        assert_eq!(seeded, 1, "round {round}: {a:?} {b:?}");
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM engine")
            .fetch_one(&seed.fixture.pool)
            .await
            .unwrap();
        assert_eq!(rows, 1, "round {round}");
        assert_eq!(seed.audit_count().await, 2 * (round + 1), "round {round}");
        let key = seed.committed_key().await;
        assert_eq!(
            file_in(dir.path()).unwrap(),
            digest_of(&key).as_bytes(),
            "round {round}"
        );
        assert_eq!(
            names_in(dir.path()),
            [installation::DIGEST_LOCK_FILE, installation::DIGEST_FILE],
            "round {round}"
        );
    }
}

#[tokio::test]
async fn a_lock_held_longer_than_the_patience_stops_the_start_and_leaves_the_file_alone() {
    let seed = Seed::new().await;
    let dir = folder();
    seed.ensure(dir.path(), ENGINE_URL).await;
    let right = file_in(dir.path()).unwrap();
    // Another process holds the lock of the folder, and the file is damaged meanwhile.
    std::fs::write(dir.path().join(installation::DIGEST_FILE), b"damaged").unwrap();
    let lock_path = dir.path().join(installation::DIGEST_LOCK_FILE);
    let held = FileLock::try_acquire(&lock_path).unwrap();
    let refused = installation::ensure_with(
        &seed.fixture.pool,
        &seed.mark,
        &seed.vault,
        &recipe(dir.path(), ENGINE_URL),
        seed.fixture.now(),
        Duration::from_millis(150),
        std::future::ready(()),
    )
    .await
    .unwrap_err();
    let InstallationEngineError::DigestDirLocked { path } = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(path, &lock_path);
    let shown = refused.to_string();
    assert!(shown.contains("ARAUTO_ENGINE_DIGEST_DIR"), "{shown}");
    assert!(shown.contains(installation::DIGEST_LOCK_FILE), "{shown}");
    assert_eq!(
        file_in(dir.path()).unwrap(),
        b"damaged",
        "the file is as it was"
    );
    // Free again: the next start repairs it.
    drop(held);
    seed.ensure(dir.path(), ENGINE_URL).await;
    assert_eq!(file_in(dir.path()).unwrap(), right);
}

#[tokio::test]
async fn a_missing_folder_stops_the_start_naming_the_variable_and_the_folder() {
    let seed = Seed::new().await;
    let dir = folder();
    let missing = dir.path().join("no-such-folder");
    let error = installation::ensure(
        &seed.fixture.pool,
        &seed.mark,
        &seed.vault,
        &recipe(&missing, ENGINE_URL),
        seed.fixture.now(),
    )
    .await
    .unwrap_err();
    let shown = error.to_string();
    assert!(shown.contains("ARAUTO_ENGINE_DIGEST_DIR"), "{shown}");
    assert!(shown.contains("no-such-folder"), "{shown}");
    // The row was committed before the folder was found missing; the next start, with the
    // folder there, writes the file from that row and draws nothing new.
    assert_eq!(seed.row().await.origin(), EngineOrigin::Installation);
    let key = seed.committed_key().await;
    std::fs::create_dir(&missing).unwrap();
    seed.ensure(&missing, ENGINE_URL).await;
    assert_eq!(file_in(&missing).unwrap(), digest_of(&key).as_bytes());
    assert_eq!(seed.committed_key().await, key);
    assert_eq!(seed.audit_count().await, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_that_changes_between_the_read_and_the_write_ends_in_the_file() {
    let seed = Seed::new().await;
    let dir = folder();
    seed.ensure(dir.path(), ENGINE_URL).await;
    let first = seed.committed_key().await;
    // The file is lost, so the next start has a write to do.
    std::fs::remove_file(dir.path().join(installation::DIGEST_FILE)).unwrap();
    let organization = seed.fixture.installation.organization;
    let id = seed.row().await.id;
    let changed = installation::draw_key().unwrap();
    let (read_tx, read_rx) = tokio::sync::oneshot::channel::<()>();
    let (go_tx, go_rx) = tokio::sync::oneshot::channel::<()>();
    let config = recipe(dir.path(), ENGINE_URL);
    let (ensured, settled_after) = tokio::join!(
        // The start reads the committed row, and waits there with the lock held.
        installation::ensure_with(
            &seed.fixture.pool,
            &seed.mark,
            &seed.vault,
            &config,
            seed.fixture.now(),
            Duration::from_secs(10),
            async move {
                let _ = read_tx.send(());
                let _ = go_rx.await;
            },
        ),
        async {
            read_rx.await.unwrap();
            // Another key commits, as a later command would commit it, and that command then
            // settles the file. It must wait for the start to let the lock go.
            let sealed = seed
                .vault
                .seal_engine_secret(
                    id,
                    arauto_hub::vault::EngineSecretKind::ApiKey,
                    changed.expose().as_bytes(),
                )
                .unwrap();
            sqlx::query("UPDATE engine SET api_key_enc = $1 WHERE id = $2")
                .bind(sealed)
                .bind(id)
                .execute(&seed.fixture.pool)
                .await
                .unwrap();
            let settle = installation::settle_digest_file(
                &seed.fixture.pool,
                &seed.vault,
                organization,
                dir.path(),
                Duration::from_secs(10),
            );
            let release = async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                go_tx.send(()).unwrap();
            };
            let (settled, ()) = tokio::join!(settle, release);
            settled
        }
    );
    // The start wrote what it had read, then the command wrote what it committed.
    assert_eq!(
        ensured.unwrap(),
        Ensured::Settled {
            seeded: false,
            settled: settled(Some(EngineOrigin::Installation), FileOutcome::Written),
        }
    );
    assert_eq!(settled_after.unwrap().file, FileOutcome::Written);
    assert_ne!(first, changed.expose());
    assert_eq!(
        file_in(dir.path()).unwrap(),
        installation::digest_contents(&changed).as_bytes(),
        "the file is the digest of the key that is committed now"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn health_answers_only_after_the_digest_file_is_written() {
    let installed = support::install().await;
    let dir = folder();
    // The internal port is chosen now, so the test can try it while the hub is starting.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let internal = probe.local_addr().unwrap();
    drop(probe);
    let mut config = support::config(&installed, ClientIpSource::Socket);
    config.listen_internal = internal;
    config.installation_engine = Some(recipe(dir.path(), ENGINE_URL));
    // Another process holds the lock of the folder for a while.
    let held = FileLock::try_acquire(&dir.path().join(installation::DIGEST_LOCK_FILE)).unwrap();
    let starting = tokio::spawn(async move {
        channel::start(
            &config,
            channel::Options {
                settings: support::settings(),
                ..channel::Options::default()
            },
            Arc::new(SystemClock),
        )
        .await
    });
    // A refused connection takes seconds to be reported on some systems: wait for a short while
    // and count a connection that does not come as not answered. The hub waits 10 seconds for
    // the lock, so these few rounds end well before it gives up.
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let answered = matches!(
            tokio::time::timeout(
                Duration::from_millis(300),
                tokio::net::TcpStream::connect(internal)
            )
            .await,
            Ok(Ok(_))
        );
        assert!(
            !answered,
            "the internal port answered before the digest file was written"
        );
        assert_eq!(file_in(dir.path()), None);
        if starting.is_finished() {
            let ended = starting.await.unwrap().map(|_| ());
            panic!("the hub started or failed while the folder was locked: {ended:?}");
        }
    }
    drop(held);
    let running = starting.await.unwrap().unwrap();
    // Listening now, and the file is there, whole.
    assert_eq!(running.internal_addr(), internal);
    let written = file_in(dir.path()).unwrap();
    assert!(installation::is_digest_file_contents(&written));
    assert!(tokio::net::TcpStream::connect(internal).await.is_ok());
    running.stop().await;
}

#[tokio::test]
async fn a_hub_without_the_engine_variables_seeds_nothing_and_leaves_a_folder_alone() {
    // A hub started with `installation_engine: None`, as when the three variables are not set,
    // over a folder that holds a file and a leftover: it makes no row and no audit row of an
    // engine, and it does not touch the folder, not even to clean the leftover.
    let installed = support::install().await;
    let dir = folder();
    let stale = dir.path().join(installation::DIGEST_FILE);
    let leftover = dir.path().join(format!(
        ".{}.0123456789abcdef.tmp",
        installation::DIGEST_FILE
    ));
    std::fs::write(&stale, b"left by an engine that is no longer wanted").unwrap();
    std::fs::write(&leftover, b"half").unwrap();
    let pool = db::connect(
        &installed.url,
        PoolOptions {
            size: 2,
            acquire_timeout: Duration::from_secs(10),
        },
    )
    .await
    .unwrap();
    let config = support::config(&installed, ClientIpSource::Socket);
    assert!(config.installation_engine.is_none());
    let running = channel::start(
        &config,
        channel::Options {
            settings: support::settings(),
            ..channel::Options::default()
        },
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    running.stop().await;
    assert_eq!(
        names_in(dir.path()),
        vec![
            format!(".{}.0123456789abcdef.tmp", installation::DIGEST_FILE),
            installation::DIGEST_FILE.to_owned(),
        ]
    );
    assert_eq!(
        std::fs::read(&stale).unwrap(),
        b"left by an engine that is no longer wanted"
    );
    assert_eq!(std::fs::read(&leftover).unwrap(), b"half");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM engine")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM audit WHERE action LIKE 'engine.%'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit, 0);
}

#[test]
fn the_health_of_a_refused_key_says_restart_for_the_installation_engine_and_set_key_for_the_other()
{
    let refused = || Err(EngineError::Unavailable(Cause::KeyRefused));
    let brought = render::engine_health(refused(), Some(EngineOrigin::Installation));
    assert!(!brought.done);
    assert!(
        brought.text.contains("Restart the engine"),
        "{}",
        brought.text
    );
    assert!(!brought.text.contains("set-key"), "{}", brought.text);
    for origin in [Some(EngineOrigin::Admin), None] {
        let typed = render::engine_health(refused(), origin);
        assert!(
            typed.text.contains("arauto-hub admin engine set-key"),
            "{}",
            typed.text
        );
        assert!(!typed.text.contains("Restart the engine"), "{}", typed.text);
    }
    // The other failures say what they said before.
    let down = render::engine_health(
        Err(EngineError::Unavailable(Cause::Connect)),
        Some(EngineOrigin::Installation),
    );
    assert!(!down.text.contains("Restart") && !down.text.contains("set-key"));
    assert!(render::engine_health(Ok(()), None).done);
}
