//! Random rounds of the engine the installation brings: the seed of the row, the key the hub
//! draws and the digest file, against the real database and a real folder.
//!
//! Each round draws a starting database (no engine, an engine an admin set with or without a
//! key, an engine the installation brought), a starting folder (empty, the right file, another
//! valid digest, a damaged file, leftovers of an interrupted write, the lock taken for a while),
//! a crash point (none, after the commit and before the file, in the middle of the write, after
//! the rename), one to three starts, two of them at once now and then, a key that commits
//! between the read of the row and the write of the file (a second writer settles the file while
//! the first still holds the lock), starts of a hub with the three variables unset over the same
//! folder and database, and changes of the address in the configuration between the starts. A
//! final clean start then ends the round, and these must hold:
//!
//! - I1: exactly one engine row.
//! - I2: a round that began with no engine ends with an engine of the installation, at the
//!   address of the configuration the first start had, and with a key.
//! - I3: an engine an admin set is as it was, to the byte.
//! - I4: the digest file exists if and only if the row is the installation's; it is 135 bytes of
//!   the right shape and it is the digest of the key that opens from the committed row.
//! - I5: no leftover of a write in the folder.
//! - I6: the audit has one `engine.configured` and one `engine.key_changed`, both by the system
//!   and about the installation, for an engine seeded in the round, and none for a repeat.
//! - I7: the key is 64 lowercase hexadecimal characters and is in no file of the folder, no audit
//!   row and no alert.
//! - I8: no two rounds drew the same key.
//! - I9: a start with the variables unset leaves the folder (every name and every byte) and the
//!   engine row as they were.
//!
//! A failing round prints its seed. `ARAUTO_ENGINE_ROUND=<seed>` replays that one round,
//! `ARAUTO_TEST_SEED=<seed>` moves the whole run to other rounds, and
//! `ARAUTO_ENGINE_ROUNDS=<count>` runs more of them.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::Duration;

use arauto_hub::channel;
use arauto_hub::config::{ClientIpSource, InstallationEngineConfig};
use arauto_hub::db::{self, PoolOptions, ResolvedOrg};
use arauto_hub::engine::installation::{self, DIGEST_FILE, DIGEST_LOCK_FILE};
use arauto_hub::engine::{ApiKey, EngineAddress, EventsUrl};
use arauto_hub::mark::Mark;
use arauto_hub::repo::engine::{self, EngineOrigin};
use arauto_hub::vault::{EngineSecretKind, Vault};
use arauto_platform::FileLock;
use arauto_protocol::clock::{Clock, SeededRng, SystemClock};
use arauto_testkit::hub::fixture::Fixture;
use arauto_testkit::hub::support;
use sqlx::postgres::PgPool;

/// How many rounds run, at least.
const ROUNDS: u64 = 300;

/// The seed of the run when `ARAUTO_TEST_SEED` does not give another.
const DEFAULT_SEED: u64 = 0x454e_4749_4e45;

const URLS: [&str; 2] = ["http://192.0.2.10:3000", "http://192.0.2.20:3000"];
const EVENTS: &str = "http://hub.example:8444";

fn number_from(variable: &str) -> Option<u64> {
    let text = std::env::var(variable).ok()?;
    Some(
        text.trim()
            .parse()
            .unwrap_or_else(|_| panic!("{variable} must be a number, and it is {text:?}")),
    )
}

/// What the rounds reached, to show that they reach what they are meant to reach.
#[derive(Debug, Default, Clone, Copy)]
struct Reached {
    rounds: u64,
    began_empty: u64,
    began_admin_with_key: u64,
    began_admin_without_key: u64,
    began_installation: u64,
    crashed_after_commit: u64,
    crashed_in_write: u64,
    crashed_after_rename: u64,
    lock_held: u64,
    leftovers: u64,
    damaged: u64,
    repeats: u64,
    concurrent: u64,
    address_changed: u64,
    file_written: u64,
    file_removed: u64,
    key_changed_while_locked: u64,
    started_without_variables: u64,
}

impl Reached {
    fn add(&mut self, other: &Self) {
        self.rounds += other.rounds;
        self.began_empty += other.began_empty;
        self.began_admin_with_key += other.began_admin_with_key;
        self.began_admin_without_key += other.began_admin_without_key;
        self.began_installation += other.began_installation;
        self.crashed_after_commit += other.crashed_after_commit;
        self.crashed_in_write += other.crashed_in_write;
        self.crashed_after_rename += other.crashed_after_rename;
        self.lock_held += other.lock_held;
        self.leftovers += other.leftovers;
        self.damaged += other.damaged;
        self.repeats += other.repeats;
        self.concurrent += other.concurrent;
        self.address_changed += other.address_changed;
        self.file_written += other.file_written;
        self.file_removed += other.file_removed;
        self.key_changed_while_locked += other.key_changed_while_locked;
        self.started_without_variables += other.started_without_variables;
    }
}

struct Arena {
    fixture: Fixture,
    other: PgPool,
    vault: Vault,
    mark: Mark,
    organization: ResolvedOrg,
}

impl Arena {
    async fn new() -> Self {
        let fixture = Fixture::stopped(
            support::install().await,
            support::settings(),
            std::sync::Arc::new(SystemClock),
        )
        .await;
        let other = db::connect(
            &fixture.installation.url,
            PoolOptions {
                size: 3,
                acquire_timeout: Duration::from_secs(10),
            },
        )
        .await
        .unwrap();
        let vault = Vault::open(&fixture.installation.data_dir).unwrap();
        let mark = Mark::new(&fixture.installation.data_dir);
        let organization = fixture.installation.organization;
        Self {
            fixture,
            other,
            vault,
            mark,
            organization,
        }
    }
}

fn recipe(dir: &Path, url: &str) -> InstallationEngineConfig {
    InstallationEngineConfig {
        address: EngineAddress::parse(url).unwrap(),
        events_url: EventsUrl::parse(EVENTS).unwrap(),
        digest_dir: dir.to_owned(),
    }
}

fn random_bytes(rng: &mut SeededRng, most: u64) -> Vec<u8> {
    let length = rng.below(most + 1);
    (0..length)
        .map(|_| u8::try_from(rng.below(256)).unwrap())
        .collect()
}

fn leftover_name(rng: &mut SeededRng) -> String {
    format!(".{DIGEST_FILE}.{:016x}.tmp", rng.next_u64())
}

/// The names in the folder.
fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn digest_of_text(key: &str) -> String {
    installation::digest_contents(&ApiKey::parse(key).unwrap())
}

/// Every file of the folder with its bytes.
fn contents_of(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

/// The second writer: it waits for the start to read the row, commits another key on the row of
/// the installation, as a later command would, and then settles the file as that command must.
/// It cannot get the lock until `delay_ms` has passed and the start lets `go` loose. Returns
/// whether it committed a key. The start may fail before it reads the row, and then `read` is
/// dropped and this does nothing.
async fn change_the_key_when_read(
    arena: &Arena,
    dir: &Path,
    read: tokio::sync::oneshot::Receiver<()>,
    go: tokio::sync::oneshot::Sender<()>,
    delay_ms: u64,
) -> Result<bool, String> {
    if read.await.is_err() {
        return Ok(false);
    }
    let pool = &arena.other;
    let committed: Result<bool, String> = async {
        let Some(row) = engine::get(pool, arena.organization)
            .await
            .map_err(|error| format!("{error:?}"))?
            .filter(|row| row.origin() == EngineOrigin::Installation)
        else {
            return Ok(false);
        };
        let key = installation::draw_key().map_err(|error| error.to_string())?;
        let sealed = arena
            .vault
            .seal_engine_secret(row.id, EngineSecretKind::ApiKey, key.expose().as_bytes())
            .map_err(|error| format!("{error:?}"))?;
        sqlx::query("UPDATE engine SET api_key_enc = $1 WHERE id = $2")
            .bind(sealed)
            .bind(row.id)
            .execute(pool)
            .await
            .map_err(|error| format!("{error:?}"))?;
        Ok(true)
    }
    .await;
    if !matches!(committed, Ok(true)) {
        let _ = go.send(());
        return committed;
    }
    let settle = installation::settle_digest_file(
        pool,
        &arena.vault,
        arena.organization,
        dir,
        Duration::from_secs(10),
    );
    let release = async {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        let _ = go.send(());
    };
    let (settled, ()) = tokio::join!(settle, release);
    settled.map_err(|error| format!("the second writer failed: {error}"))?;
    Ok(true)
}

/// A start of the hub with the three engine variables unset, over the database and the folder of
/// the round: it must touch neither.
async fn start_without_the_variables(arena: &Arena, folder: &Path) -> Result<(), String> {
    let pool = &arena.fixture.pool;
    let row_before = engine::get(pool, arena.organization)
        .await
        .map_err(|error| format!("{error:?}"))?;
    let folder_before = contents_of(folder);
    let config = support::config(&arena.fixture.installation, ClientIpSource::Socket);
    assert!(config.installation_engine.is_none());
    let running = channel::start(
        &config,
        channel::Options {
            settings: support::settings(),
            ..channel::Options::default()
        },
        std::sync::Arc::new(SystemClock),
    )
    .await
    .map_err(|error| format!("a start without the variables failed: {error}"))?;
    running.stop().await;
    let row_after = engine::get(pool, arena.organization)
        .await
        .map_err(|error| format!("{error:?}"))?;
    if row_after != row_before {
        return Err("I9: a start without the variables changed the engine row".to_owned());
    }
    if contents_of(folder) != folder_before {
        return Err("I9: a start without the variables changed the folder".to_owned());
    }
    Ok(())
}

/// I4 at this moment: the digest file exists if and only if the committed row is the
/// installation's, and then it is the digest of the key that opens from that row.
async fn file_follows_the_row(arena: &Arena, folder: &Path) -> Result<(), String> {
    let row = engine::get(&arena.fixture.pool, arena.organization)
        .await
        .map_err(|error| format!("{error:?}"))?;
    let on_disk = std::fs::read(folder.join(DIGEST_FILE)).ok();
    match row.filter(|row| row.origin() == EngineOrigin::Installation) {
        Some(row) => {
            let key = row.secrets(&arena.vault).unwrap().key.unwrap();
            let wanted = installation::digest_contents(&key);
            if on_disk.as_deref() != Some(wanted.as_bytes()) {
                return Err("I4: the file is not the digest of the committed key".to_owned());
            }
        }
        None => {
            if on_disk.is_some() {
                return Err("I4: a file for an engine that is not the installation's".to_owned());
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn round(arena: &Arena, seed: u64, drawn: &mut HashSet<String>) -> Result<Reached, String> {
    let mut rng = SeededRng::new(seed);
    let mut reached = Reached {
        rounds: 1,
        ..Reached::default()
    };
    let pool = &arena.fixture.pool;
    let now = SystemClock.now_ms();
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path();
    sqlx::query("DELETE FROM engine")
        .execute(pool)
        .await
        .unwrap();
    let audit_before: i64 = sqlx::query_scalar("SELECT coalesce(max(seq), 0) FROM audit")
        .fetch_one(pool)
        .await
        .unwrap();

    // The starting database.
    let began = rng.below(4);
    let mut first_url = URLS[0];
    match began {
        0 => reached.began_empty += 1,
        1 | 2 => {
            arena
                .fixture
                .admin
                .engine_set(&arena.vault, URLS[1], EVENTS, now)
                .await
                .map_err(|error| format!("{error:?}"))?;
            if began == 1 {
                reached.began_admin_with_key += 1;
                arena
                    .fixture
                    .admin
                    .engine_set_key(&arena.vault, &format!("typed-key-{seed}"), now)
                    .await
                    .map_err(|error| format!("{error:?}"))?;
            } else {
                reached.began_admin_without_key += 1;
            }
        }
        _ => {
            reached.began_installation += 1;
            let key = installation::draw_key().unwrap();
            engine::seed_installation(
                pool,
                &arena.mark,
                arena.organization,
                &arena.vault,
                &recipe(folder, URLS[1]).address,
                &recipe(folder, URLS[1]).events_url,
                &key,
                now,
            )
            .await
            .map_err(|error| format!("{error:?}"))?;
            first_url = URLS[1];
        }
    }
    let admin_row = if began == 1 || began == 2 {
        engine::get(pool, arena.organization)
            .await
            .map_err(|error| format!("{error:?}"))?
    } else {
        None
    };

    // The crash point, as the state it leaves behind.
    match rng.below(4) {
        0 => {}
        1 => {
            // After the commit of the row, before the file: only the row exists.
            reached.crashed_after_commit += 1;
            if began == 0 {
                let key = installation::draw_key().unwrap();
                let config = recipe(folder, URLS[0]);
                engine::seed_installation(
                    pool,
                    &arena.mark,
                    arena.organization,
                    &arena.vault,
                    &config.address,
                    &config.events_url,
                    &key,
                    now,
                )
                .await
                .map_err(|error| format!("{error:?}"))?;
            }
        }
        2 => {
            // In the middle of the write: a leftover, and an old or absent file.
            reached.crashed_in_write += 1;
            std::fs::write(
                folder.join(leftover_name(&mut rng)),
                random_bytes(&mut rng, 200),
            )
            .unwrap();
            reached.leftovers += 1;
            if rng.below(2) == 0 {
                std::fs::write(folder.join(DIGEST_FILE), random_bytes(&mut rng, 60)).unwrap();
            }
        }
        _ => {
            // After the rename: the file the row calls for, if the row exists by now.
            reached.crashed_after_rename += 1;
            if began == 0 {
                // Seeded in the crash, then the file written, then the process died.
                let key = installation::draw_key().unwrap();
                let config = recipe(folder, URLS[0]);
                engine::seed_installation(
                    pool,
                    &arena.mark,
                    arena.organization,
                    &arena.vault,
                    &config.address,
                    &config.events_url,
                    &key,
                    now,
                )
                .await
                .map_err(|error| format!("{error:?}"))?;
            }
            if let Some(row) = engine::get(pool, arena.organization)
                .await
                .map_err(|error| format!("{error:?}"))?
                .filter(|row| row.origin() == EngineOrigin::Installation)
            {
                let key = row.secrets(&arena.vault).unwrap().key.unwrap();
                std::fs::write(
                    folder.join(DIGEST_FILE),
                    installation::digest_contents(&key),
                )
                .unwrap();
            }
        }
    }

    // The starting folder, on top of that.
    match rng.below(5) {
        0 => {}
        1 => {
            // Another valid digest.
            let other = installation::draw_key().unwrap();
            std::fs::write(folder.join(DIGEST_FILE), digest_of_text(other.expose())).unwrap();
        }
        2 => {
            reached.damaged += 1;
            std::fs::write(folder.join(DIGEST_FILE), random_bytes(&mut rng, 200)).unwrap();
        }
        3 => {
            reached.damaged += 1;
            // Too long, with the right start.
            let mut long = digest_of_text("the-long-one-00000").into_bytes();
            long.extend(vec![b'0'; 1 + usize::try_from(rng.below(300)).unwrap()]);
            std::fs::write(folder.join(DIGEST_FILE), long).unwrap();
        }
        _ => {}
    }
    for _ in 0..rng.below(4) {
        reached.leftovers += 1;
        std::fs::write(
            folder.join(leftover_name(&mut rng)),
            random_bytes(&mut rng, 200),
        )
        .unwrap();
    }

    // One to three starts, and the address in the configuration changing between them.
    let repeats = 1 + rng.below(3);
    for repeat in 0..repeats {
        reached.repeats += 1;
        let url = if repeat == 0 {
            first_url
        } else if rng.below(2) == 0 {
            reached.address_changed += 1;
            URLS[usize::try_from(rng.below(2)).unwrap()]
        } else {
            first_url
        };
        // A start of a hub with the variables unset touches nothing. Never the first start, which
        // fixes the address that a round that began empty ends with.
        if repeat > 0 && rng.below(4) == 0 {
            reached.started_without_variables += 1;
            start_without_the_variables(arena, folder).await?;
            continue;
        }
        // The lock taken by somebody else for a short while.
        let held = if rng.below(3) == 0 {
            reached.lock_held += 1;
            let lock = FileLock::try_acquire(&folder.join(DIGEST_LOCK_FILE)).unwrap();
            let for_ms = rng.below(300);
            Some(std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(for_ms));
                drop(lock);
            }))
        } else {
            None
        };
        let config = recipe(folder, url);
        match rng.below(6) {
            0 | 1 => {
                reached.concurrent += 1;
                let (a, b) = tokio::join!(
                    installation::ensure(pool, &arena.mark, &arena.vault, &config, now),
                    installation::ensure(&arena.other, &arena.mark, &arena.vault, &config, now),
                );
                a.map_err(|error| format!("a start failed: {error}"))?;
                b.map_err(|error| format!("a start failed: {error}"))?;
            }
            2 | 3 => {
                // A key commits between the read of the row and the write of the file.
                let delay_ms = rng.below(200);
                let (read_tx, read_rx) = tokio::sync::oneshot::channel::<()>();
                let (go_tx, go_rx) = tokio::sync::oneshot::channel::<()>();
                let (started, changed) = tokio::join!(
                    installation::ensure_with(
                        pool,
                        &arena.mark,
                        &arena.vault,
                        &config,
                        now,
                        installation::LOCK_PATIENCE,
                        async move {
                            let _ = read_tx.send(());
                            let _ = go_rx.await;
                        },
                    ),
                    change_the_key_when_read(arena, folder, read_rx, go_tx, delay_ms),
                );
                started.map_err(|error| format!("a start failed: {error}"))?;
                if changed? {
                    reached.key_changed_while_locked += 1;
                }
            }
            _ => {
                installation::ensure(pool, &arena.mark, &arena.vault, &config, now)
                    .await
                    .map_err(|error| format!("a start failed: {error}"))?;
            }
        }
        if let Some(held) = held {
            held.join().unwrap();
        }
        // I4 right after every start, before any later start could repair what this one broke:
        // the final clean start would hide a file left behind a key that committed meanwhile.
        file_follows_the_row(arena, folder)
            .await
            .map_err(|why| format!("after start {repeat}: {why}"))?;
    }

    // The final, clean start.
    let after = installation::ensure(
        pool,
        &arena.mark,
        &arena.vault,
        &recipe(folder, first_url),
        now,
    )
    .await
    .map_err(|error| format!("the final start failed: {error}"))?;
    let _ = after;

    // I1.
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM engine")
        .fetch_one(pool)
        .await
        .unwrap();
    if rows != 1 {
        return Err(format!("I1: {rows} engine rows"));
    }
    let row = engine::get(pool, arena.organization)
        .await
        .unwrap()
        .unwrap();
    // I2 and I3.
    match began {
        1 | 2 => {
            if Some(&row) != admin_row.as_ref() {
                return Err("I3: the engine of the admin changed".to_owned());
            }
        }
        _ => {
            if row.origin() != EngineOrigin::Installation {
                return Err("I2: the engine is not the installation's".to_owned());
            }
            if row.api_key_enc.is_none() {
                return Err("I2: the engine of the installation has no key".to_owned());
            }
            if began == 0 && (row.address != URLS[0] || row.events_url != EVENTS) {
                return Err("I2: the engine has not the address of the first start".to_owned());
            }
        }
    }
    // I4, I5 and I7.
    let names = names_in(folder);
    let on_disk = std::fs::read(folder.join(DIGEST_FILE)).ok();
    let mut secrets = String::new();
    match row.origin() {
        EngineOrigin::Installation => {
            let key = row.secrets(&arena.vault).unwrap().key.unwrap();
            if key.expose().len() != 64
                || !key
                    .expose()
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            {
                return Err("I7: the key is not 64 lowercase hexadecimal characters".to_owned());
            }
            let wanted = installation::digest_contents(&key);
            match &on_disk {
                Some(bytes)
                    if bytes == wanted.as_bytes()
                        && installation::is_digest_file_contents(bytes) => {}
                other => {
                    return Err(format!(
                        "I4: the file is {other:?}, not the digest of the key"
                    ));
                }
            }
            reached.file_written += 1;
            // I8.
            if !drawn.insert(key.expose().to_owned()) {
                return Err("I8: a key was drawn twice".to_owned());
            }
            secrets.push_str(key.expose());
        }
        EngineOrigin::Admin => {
            if on_disk.is_some() {
                return Err("I4: a file for an engine that is not the installation's".to_owned());
            }
            reached.file_removed += 1;
        }
    }
    for name in &names {
        if name != DIGEST_FILE && name != DIGEST_LOCK_FILE {
            return Err(format!("I5: {name} is in the folder"));
        }
    }
    if !secrets.is_empty() {
        for name in &names {
            let bytes = std::fs::read(folder.join(name)).unwrap();
            if String::from_utf8_lossy(&bytes).contains(&secrets) {
                return Err(format!("I7: the key is in {name}"));
            }
        }
        let texts: Vec<String> = sqlx::query_scalar(
            "SELECT row_to_json(a)::text FROM audit a UNION ALL \
             SELECT row_to_json(l)::text FROM alert l",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        if texts.iter().any(|text| text.contains(&secrets)) {
            return Err("I7: the key is in an audit row or an alert".to_owned());
        }
    }
    // I6.
    let system: Vec<(String, String)> = sqlx::query_as(
        "SELECT action, details::text FROM audit \
         WHERE seq > $1 AND actor_kind = 'system' AND action LIKE 'engine.%' ORDER BY seq",
    )
    .bind(audit_before)
    .fetch_all(pool)
    .await
    .unwrap();
    let seeded = began == 0 || began == 3;
    let expected = if seeded { 2 } else { 0 };
    if system.len() != expected {
        return Err(format!(
            "I6: {} audit rows by the system: {system:?}",
            system.len()
        ));
    }
    if seeded {
        let actions: Vec<&str> = system.iter().map(|(action, _)| action.as_str()).collect();
        if actions != ["engine.configured", "engine.key_changed"] {
            return Err(format!("I6: {actions:?}"));
        }
        if system
            .iter()
            .any(|(_, details)| !details.contains("installation"))
        {
            return Err(format!("I6: {system:?}"));
        }
    }
    Ok(reached)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn random_rounds_of_the_engine_of_the_installation_keep_the_row_and_the_file_together() {
    let arena = Arena::new().await;
    let mut drawn = HashSet::new();

    // A start that stops before the commit leaves what the database held before: the
    // transaction rolls back, and the next start seeds as if nothing had happened.
    {
        let mut open = arena.fixture.pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO engine (id, organization_id, kind, origin, address, events_url, \
                                 api_key_enc, events_secret_enc, config_version, created_at, \
                                 changed_at) \
             VALUES (gen_random_uuid(), $1, 'waha', 'installation', 'http://192.0.2.10:3000', \
                     'http://hub.example:8444', '\\x00', '\\x00', 1, 1, 1)",
        )
        .bind(arena.organization.id())
        .execute(&mut *open)
        .await
        .unwrap();
        drop(open);
        assert!(
            engine::get(&arena.fixture.pool, arena.organization)
                .await
                .unwrap()
                .is_none()
        );
    }

    if let Some(seed) = number_from("ARAUTO_ENGINE_ROUND") {
        match round(&arena, seed, &mut drawn).await {
            Ok(reached) => println!("round {seed} holds: {reached:#?}"),
            Err(why) => panic!("round {seed} breaks an invariant.\n{why}"),
        }
        return;
    }
    let base = number_from("ARAUTO_TEST_SEED").unwrap_or(DEFAULT_SEED);
    let rounds = number_from("ARAUTO_ENGINE_ROUNDS").map_or(ROUNDS, |asked| asked.max(ROUNDS));
    let mut total = Reached::default();
    for number in 0..rounds {
        let seed = base.wrapping_add(number);
        match round(&arena, seed, &mut drawn).await {
            Ok(reached) => total.add(&reached),
            Err(why) => panic!(
                "the round of seed {seed} breaks an invariant; replay it with \
                 ARAUTO_ENGINE_ROUND={seed}\n{why}"
            ),
        }
    }
    println!("{rounds} rounds from seed {base} (ARAUTO_TEST_SEED picks another): {total:#?}");

    // The rounds must reach what they are meant to try.
    assert_eq!(total.rounds, rounds);
    let reached: &[(&str, u64, u64)] = &[
        (
            "rounds that began with no engine",
            total.began_empty,
            rounds / 8,
        ),
        (
            "rounds that began with an engine of an admin with a key",
            total.began_admin_with_key,
            rounds / 8,
        ),
        (
            "rounds that began with an engine of an admin without a key",
            total.began_admin_without_key,
            rounds / 8,
        ),
        (
            "rounds that began with an engine of the installation",
            total.began_installation,
            rounds / 8,
        ),
        (
            "crashes after the commit",
            total.crashed_after_commit,
            rounds / 8,
        ),
        ("crashes in the write", total.crashed_in_write, rounds / 8),
        (
            "crashes after the rename",
            total.crashed_after_rename,
            rounds / 8,
        ),
        ("starts with the lock taken", total.lock_held, rounds / 8),
        ("leftovers", total.leftovers, rounds / 4),
        ("damaged files", total.damaged, rounds / 8),
        ("repeats", total.repeats, rounds),
        ("starts at once", total.concurrent, rounds / 8),
        (
            "keys that committed between the read and the write",
            total.key_changed_while_locked,
            rounds / 8,
        ),
        (
            "starts of a hub without the engine variables",
            total.started_without_variables,
            rounds / 8,
        ),
        ("changes of the address", total.address_changed, rounds / 8),
        (
            "rounds that ended with a file",
            total.file_written,
            rounds / 4,
        ),
        (
            "rounds that ended with no file",
            total.file_removed,
            rounds / 4,
        ),
    ];
    for (what, got, at_least) in reached {
        assert!(
            got >= at_least,
            "the rounds did not reach enough: {what}: {got}, at least {at_least} expected"
        );
    }
}
