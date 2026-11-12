//! The purge as the hub runs it (`docs/15-mvp.md`, section 2, rule 11): a task of its own, by
//! the clock of the hub, through every organization; a hub that stops in the middle of a run
//! ends it between two batches, nothing of it runs once the stop returns, and the next start
//! ends the work.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use arauto_hub::channel::settings::PurgeSettings;
use arauto_hub::vault::Vault;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::limits::DAY_MS;
use sqlx::AssertSqlSafe;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{self, Confirmed};
use crate::fixture::{Fixture, PATIENCE, settings};

/// Polls `condition` until it holds, or fails the test.
async fn until<F, Fut>(what: &str, within: Duration, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + within;
    loop {
        if condition().await {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "never: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Another organization in the database of `fixture`, with its key under the master key.
async fn second_organization(fixture: &Fixture) -> Uuid {
    let id = Uuid::new_v4();
    let vault = Vault::open(&fixture.installation.data_dir).unwrap();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'second', 'active', $2, $3)",
    )
    .bind(id)
    .bind(vault.generate_organization_key(id).unwrap())
    .bind(fixture.now())
    .execute(&fixture.pool)
    .await
    .unwrap();
    id
}

/// Calls with these identifiers, all made at `at`.
async fn tool_calls(pool: &PgPool, organization: Uuid, ids: &[Uuid], at: i64) {
    sqlx::query(
        "INSERT INTO tool_call (id, organization_id, hermes_id, device_id, tool, result, \
                                duration_ms, at) \
         SELECT id, $2, $3, $3, 'arauto_status', 'ok', 3, $4 FROM unnest($1::uuid[]) AS id",
    )
    .bind(ids)
    .bind(organization)
    .bind(Uuid::new_v4())
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn unknown(pool: &PgPool, organization: Uuid, seen_at: i64, proof_pending: bool) -> Uuid {
    let id = Uuid::new_v4();
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(id.as_bytes());
    sqlx::query(
        "INSERT INTO unknown_device (id, organization_id, public_key, seen_at, proof_pending) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(organization)
    .bind(key.as_slice())
    .bind(seen_at)
    .bind(proof_pending)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn connection(
    pool: &PgPool,
    organization: Uuid,
    device: Uuid,
    opened_at: i64,
    closed_at: Option<i64>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO channel_connection (id, organization_id, device_id, opened_at, closed_at, \
                                         close_reason) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(organization)
    .bind(device)
    .bind(opened_at)
    .bind(closed_at)
    .bind(closed_at.map(|_| "dropped"))
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Which of `rows` are still there.
async fn left(pool: &PgPool, rows: &[(&'static str, Uuid)]) -> Vec<(&'static str, Uuid)> {
    let mut found = Vec::new();
    for (table, id) in rows {
        let there: bool = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1)"
        )))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
        if there {
            found.push((*table, *id));
        }
    }
    found
}

async fn calls_left(pool: &PgPool, ids: &[Uuid]) -> usize {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tool_call WHERE id = ANY($1)")
        .bind(ids)
        .fetch_one(pool)
        .await
        .unwrap();
    usize::try_from(count).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hub_purges_by_its_own_clock_in_every_organization() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let mut chosen = settings();
    chosen.purge = PurgeSettings {
        interval_ms: 100,
        batch_rows: 2,
    };
    let fixture = Fixture::with_clock(chosen, Arc::new(clock.clone())).await;
    let pool = &fixture.pool;
    let first = fixture.installation.organization;
    let second = second_organization(&fixture).await;
    let hermes = common::hermes(pool, first).await;
    let (device, _) = common::device(pool, first, hermes, "revoked", Confirmed::Yes).await;

    // Rows of a day ago: within their retention now, past it once the hub is 100 days on.
    let yesterday = clock.now_ms() - DAY_MS;
    let mut due = Vec::new();
    for organization in [first.id(), second] {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        tool_calls(pool, organization, &ids, yesterday).await;
        due.extend(ids.into_iter().map(|id| ("tool_call", id)));
        due.push((
            "unknown_device",
            unknown(pool, organization, yesterday, false).await,
        ));
    }
    due.push((
        "channel_connection",
        connection(
            pool,
            first.id(),
            device,
            yesterday - DAY_MS,
            Some(yesterday),
        )
        .await,
    ));
    let kept = vec![
        (
            "unknown_device",
            unknown(pool, second, yesterday, true).await,
        ),
        (
            "channel_connection",
            connection(pool, first.id(), device, yesterday, None).await,
        ),
    ];

    // Several runs go by: by the clock of the hub nothing is due.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(left(pool, &due).await, due);

    // The clock of the hub moves 100 days on; the clock of the system does not.
    clock.advance(100 * DAY_MS);
    until("every row past its retention is gone", PATIENCE, || async {
        left(pool, &due).await.is_empty()
    })
    .await;
    assert_eq!(left(pool, &kept).await, kept);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_in_the_middle_of_a_purge_leaves_whole_batches_and_the_next_start_ends_it() {
    const ROWS: usize = 5_000;
    const BATCH: u32 = 5;
    let mut chosen = settings();
    chosen.purge = PurgeSettings {
        interval_ms: 50,
        batch_rows: BATCH,
    };
    // The stop waits for the purge up to its grace. A purge that went on through the stop
    // would end all of its rows within this one, and the test would see it.
    chosen.deadlines.shutdown_grace_ms = 60_000;
    let mut fixture = Fixture::new(chosen).await;
    let pool = fixture.pool.clone();
    let organization = fixture.installation.organization.id();
    let now = fixture.now();
    let due: Vec<Uuid> = (0..ROWS).map(|_| Uuid::new_v4()).collect();
    tool_calls(&pool, organization, &due, now - 30 * DAY_MS).await;
    let within: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    tool_calls(&pool, organization, &within, now).await;

    until("the purge began", PATIENCE, || async {
        calls_left(&pool, &due).await < ROWS
    })
    .await;
    fixture.stop().await;
    let after_stop = calls_left(&pool, &due).await;
    // The purge saw the stop between two batches and ended there.
    assert!(
        after_stop > 0,
        "the purge went on to the end through the stop of the hub"
    );
    // Whole batches went, and nothing runs once the hub stopped.
    assert_eq!((ROWS - after_stop) % BATCH as usize, 0, "{after_stop}");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(calls_left(&pool, &due).await, after_stop);
    assert_eq!(calls_left(&pool, &within).await, within.len());

    // The next start goes on from there and ends it.
    fixture.start().await;
    until("the purge ended", PATIENCE * 4, || async {
        calls_left(&pool, &due).await == 0
    })
    .await;
    assert_eq!(calls_left(&pool, &within).await, within.len());
}
