//! The purge against the database (`docs/15-mvp.md`, section 2, rule 11): what passed its
//! retention goes and what is within stays, at the exact millisecond; in batches of the size it
//! is given; only in the organization it is given; a purge asked to stop, or whose batch
//! fails, leaves whole batches and the next run ends it; a pending proof of a restoration
//! stays; and what happens to an unknown device purged while it presents, or purged and seen
//! again (`docs/14-protocolo-do-canal.md`, section 13).

use std::collections::BTreeSet;

use arauto_hub::core::model::DeviceState;
use arauto_hub::repo::RepoError;
use arauto_hub::repo::purge::{self, Purged, PurgedTarget, Target};
use arauto_hub::repo::swap::{self, Stranger, UnknownOutcome};
use arauto_hub::restore::{self, EnterOutcome, Evidence, UnknownDevice};
use arauto_protocol::limits::DAY_MS;
use arauto_protocol::message::Generation;
use arauto_protocol::noise::PublicKey;
use sqlx::AssertSqlSafe;
use uuid::Uuid;

use crate::common::{self, Confirmed};
use crate::support::{World, address, identity};

/// The rows of one organization, made at the times a test chooses.
struct Rows<'a> {
    world: &'a World,
    /// Two Hermes lines, smaller identifier first: the two sides of a thread.
    hermes: (Uuid, Uuid),
    /// A device of the first one, for the connections.
    device: Uuid,
}

impl<'a> Rows<'a> {
    async fn new(world: &'a World) -> Self {
        let pool = world.pool();
        let one = common::hermes(pool, world.org).await;
        let two = common::hermes(pool, world.org).await;
        let (device, _) = common::device(pool, world.org, one, "revoked", Confirmed::Yes).await;
        Self {
            world,
            hermes: (one.min(two), one.max(two)),
            device,
        }
    }

    fn org(&self) -> Uuid {
        self.world.org.id()
    }

    async fn tool_call(&self, at: i64) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO tool_call (id, organization_id, hermes_id, device_id, tool, result, \
                                    duration_ms, at) \
             VALUES ($1, $2, $3, $4, 'arauto_status', 'ok', 3, $5)",
        )
        .bind(id)
        .bind(self.org())
        .bind(self.hermes.0)
        .bind(self.device)
        .bind(at)
        .execute(self.world.pool())
        .await
        .unwrap();
        id
    }

    /// A notice about a peer, in `state`: a resolved one carries the time it was resolved.
    async fn notice(&self, state: &str, resolved_at: Option<i64>) -> Uuid {
        let id = Uuid::new_v4();
        let created = self.world.now() - 400 * DAY_MS;
        sqlx::query(
            "INSERT INTO notice (id, organization_id, hermes_id, kind, from_hermes_id, count, \
                                 state, not_before, leased_until, created_at, expires_at, \
                                 resolved_at) \
             VALUES ($1, $2, $3, 'peer', $4, 1, $5, $6, $7, $6, $8, $9)",
        )
        .bind(id)
        .bind(self.org())
        .bind(self.hermes.0)
        // Each one from another sender: one pending notice per sender.
        .bind(Uuid::new_v4())
        .bind(state)
        .bind(created)
        .bind((state == "sent").then_some(created + 60_000))
        .bind(created + DAY_MS)
        .bind(resolved_at)
        .execute(self.world.pool())
        .await
        .unwrap();
        id
    }

    async fn connection(&self, opened_at: i64, closed_at: Option<i64>) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO channel_connection (id, organization_id, device_id, address, \
                                             opened_at, closed_at, close_reason) \
             VALUES ($1, $2, $3, '192.0.2.30', $4, $5, $6)",
        )
        .bind(id)
        .bind(self.org())
        .bind(self.device)
        .bind(opened_at)
        .bind(closed_at)
        .bind(closed_at.map(|_| "dropped"))
        .execute(self.world.pool())
        .await
        .unwrap();
        id
    }

    async fn unknown(&self, seen_at: i64, proof_pending: bool) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO unknown_device (id, organization_id, public_key, address, seen_at, \
                                         proof_pending) \
             VALUES ($1, $2, $3, '192.0.2.31', $4, $5)",
        )
        .bind(id)
        .bind(self.org())
        .bind(identity().as_bytes().as_slice())
        .bind(seen_at)
        .bind(proof_pending)
        .execute(self.world.pool())
        .await
        .unwrap();
        id
    }

    async fn thread(&self, last_message_at: i64) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO peer_thread (id, organization_id, hermes_a, hermes_b, created_at, \
                                      last_message_at) \
             VALUES ($1, $2, $3, $4, $5, $5)",
        )
        .bind(id)
        .bind(self.org())
        .bind(self.hermes.0)
        .bind(self.hermes.1)
        .bind(last_message_at)
        .execute(self.world.pool())
        .await
        .unwrap();
        id
    }

    /// A message of `thread`, sent at `sent_at`, to be purged at `sent_at` plus 30 days.
    async fn message(&self, thread: Uuid, sent_at: i64) -> Uuid {
        self.message_until(thread, sent_at, sent_at + 30 * DAY_MS)
            .await
    }

    /// A message of `thread`, sent at `sent_at`, to be purged at `purge_at`.
    async fn message_until(&self, thread: Uuid, sent_at: i64, purge_at: i64) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO peer_message (id, organization_id, thread_id, from_hermes_id, \
                                       to_hermes_id, sent_at, size, nonce, text_enc, purge_at) \
             VALUES ($1, $2, $3, $4, $5, $6, 5, $7, $8, $9)",
        )
        .bind(id)
        .bind(self.org())
        .bind(thread)
        .bind(self.hermes.0)
        .bind(self.hermes.1)
        .bind(sent_at)
        .bind([7u8; 24].as_slice())
        .bind([9u8; 21].as_slice())
        .bind(purge_at)
        .execute(self.world.pool())
        .await
        .unwrap();
        id
    }

    /// One row past its retention of each target, as `(table, id)`.
    async fn one_due_of_each(&self) -> Vec<(&'static str, Uuid)> {
        let now = self.world.now();
        let emptied = self.thread(now - 31 * DAY_MS).await;
        let mut due = vec![
            (
                "peer_message",
                self.message(emptied, now - 31 * DAY_MS).await,
            ),
            ("peer_thread", emptied),
            ("tool_call", self.tool_call(now - 8 * DAY_MS).await),
        ];
        due.extend(self.one_due_after_the_calls().await);
        due
    }

    /// One row past its retention of each target that comes after the calls.
    async fn one_due_after_the_calls(&self) -> Vec<(&'static str, Uuid)> {
        let now = self.world.now();
        vec![
            (
                "notice",
                self.notice("confirmed", Some(now - 8 * DAY_MS)).await,
            ),
            (
                "channel_connection",
                self.connection(now - 92 * DAY_MS, Some(now - 91 * DAY_MS))
                    .await,
            ),
            (
                "unknown_device",
                self.unknown(now - 91 * DAY_MS, false).await,
            ),
        ]
    }
}

async fn exists(world: &World, table: &'static str, id: Uuid) -> bool {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1)"
    )))
    .bind(id)
    .fetch_one(world.pool())
    .await
    .unwrap()
}

/// Which of `rows` are still there, by table and id.
async fn left(world: &World, rows: &[(&'static str, Uuid)]) -> Vec<(&'static str, Uuid)> {
    let mut found = Vec::new();
    for (table, id) in rows {
        if exists(world, table, *id).await {
            found.push((*table, *id));
        }
    }
    found
}

async fn run(world: &World, batch_rows: u32) -> Purged {
    let retention = purge::retention(world.pool()).await.unwrap();
    purge::purge(
        world.pool(),
        world.org,
        &retention,
        world.now(),
        batch_rows,
        || true,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn what_passed_its_retention_goes_and_what_is_within_stays() {
    let world = World::shared().await;
    let rows = Rows::new(&world).await;
    let now = world.now();
    let day = DAY_MS;

    // Calls, 7 days. The deadline has passed when the time reached it.
    let call_due = rows.tool_call(now - 7 * day).await;
    let call_within = rows.tool_call(now - 7 * day + 1).await;
    // Notices, 7 days after they were resolved; an open one stays, however old.
    let confirmed_due = rows.notice("confirmed", Some(now - 7 * day)).await;
    let expired_due = rows.notice("expired", Some(now - 30 * day)).await;
    let discarded_due = rows.notice("discarded", Some(now - 8 * day)).await;
    let confirmed_within = rows.notice("confirmed", Some(now - 7 * day + 1)).await;
    let pending = rows.notice("pending", None).await;
    let sent = rows.notice("sent", None).await;
    // Connections, 90 days after they closed; an open one stays, however old.
    let closed_due = rows.connection(now - 100 * day, Some(now - 90 * day)).await;
    let closed_within = rows
        .connection(now - 100 * day, Some(now - 90 * day + 1))
        .await;
    let open = rows.connection(now - 400 * day, None).await;
    // Unknown devices, 90 days after they were seen; a pending proof stays, however old.
    let unknown_due = rows.unknown(now - 90 * day, false).await;
    let unknown_within = rows.unknown(now - 90 * day + 1, false).await;
    let proof = rows.unknown(now - 900 * day, true).await;
    // Messages, at their own time: 30 days after they were sent, read or not. A thread left
    // without a message goes in the same purge, 30 days after its last one; a thread that
    // still has a message, or whose last one is within, stays.
    let kept_thread = rows.thread(now - 30 * day + 1).await;
    let message_due = rows.message(kept_thread, now - 30 * day).await;
    let message_within = rows.message(kept_thread, now - 30 * day + 1).await;
    let emptied = rows.thread(now - 30 * day).await;
    let last_message = rows.message(emptied, now - 30 * day).await;
    let empty_within = rows.thread(now - 30 * day + 1).await;
    // A thread past the days of the messages after its last one that still has a message
    // whose own time has not come: it stays, and the purge of the tables after it goes on.
    let held_thread = rows.thread(now - 40 * day).await;
    let held = rows
        .message_until(held_thread, now - 40 * day, now + day)
        .await;

    let purged = run(&world, 500).await;
    assert!(!purged.stopped);
    let rows_of = |target| purged.of(target).rows;
    assert_eq!(rows_of(Target::PeerMessages), 2);
    assert_eq!(rows_of(Target::EmptyThreads), 1);
    assert_eq!(rows_of(Target::ToolCalls), 1);
    assert_eq!(rows_of(Target::ResolvedNotices), 3);
    assert_eq!(rows_of(Target::ClosedConnections), 1);
    assert_eq!(rows_of(Target::UnknownDevices), 1);

    let gone = [
        ("tool_call", call_due),
        ("notice", confirmed_due),
        ("notice", expired_due),
        ("notice", discarded_due),
        ("channel_connection", closed_due),
        ("unknown_device", unknown_due),
        ("peer_message", message_due),
        ("peer_message", last_message),
        ("peer_thread", emptied),
    ];
    assert_eq!(left(&world, &gone).await, []);
    let stay = [
        ("tool_call", call_within),
        ("notice", confirmed_within),
        ("notice", pending),
        ("notice", sent),
        ("channel_connection", closed_within),
        ("channel_connection", open),
        ("unknown_device", unknown_within),
        ("unknown_device", proof),
        ("peer_message", message_within),
        ("peer_message", held),
        ("peer_thread", kept_thread),
        ("peer_thread", empty_within),
        ("peer_thread", held_thread),
    ];
    assert_eq!(left(&world, &stay).await, stay);

    // Nothing is due a second time.
    assert_eq!(run(&world, 500).await.rows(), 0);
}

#[tokio::test]
async fn the_purge_goes_in_batches_of_at_most_the_size_it_is_given() {
    let world = World::shared().await;
    let rows = Rows::new(&world).await;
    let now = world.now();
    let within = rows.tool_call(now).await;
    for _ in 0..23 {
        rows.tool_call(now - 8 * DAY_MS).await;
    }
    let retention = purge::retention(world.pool()).await.unwrap();
    let mut asked = 0u32;
    let purged = purge::purge(world.pool(), world.org, &retention, now, 5, || {
        asked += 1;
        true
    })
    .await
    .unwrap();
    // 5, 5, 5, 5 and 3: the batch that finds fewer than its size is the last one.
    assert_eq!(
        purged.of(Target::ToolCalls),
        PurgedTarget {
            target: Target::ToolCalls,
            rows: 23,
            batches: 5
        }
    );
    for target in Target::ALL {
        if target != Target::ToolCalls {
            assert_eq!(
                purged.of(target),
                PurgedTarget {
                    target,
                    rows: 0,
                    batches: 1
                }
            );
        }
    }
    assert_eq!(asked, 10, "asked before each batch");

    // A batch that finds exactly its size is followed by one more.
    for _ in 0..10 {
        rows.tool_call(now - 8 * DAY_MS).await;
    }
    assert_eq!(
        run(&world, 10).await.of(Target::ToolCalls),
        PurgedTarget {
            target: Target::ToolCalls,
            rows: 10,
            batches: 2
        }
    );
    // A size of zero is a size of one.
    for _ in 0..3 {
        rows.tool_call(now - 8 * DAY_MS).await;
    }
    assert_eq!(
        run(&world, 0).await.of(Target::ToolCalls),
        PurgedTarget {
            target: Target::ToolCalls,
            rows: 3,
            batches: 4
        }
    );
    assert!(exists(&world, "tool_call", within).await);
}

#[tokio::test]
async fn the_purge_of_one_organization_never_reaches_another() {
    let world = World::shared().await;
    let elsewhere = World::shared().await;
    elsewhere.clock.set(world.now());
    let here = Rows::new(&world).await;
    let there = Rows::new(&elsewhere).await;
    // The other organization has more rows past their retention than a batch, and older ones:
    // a batch that read them would leave rows of this one behind.
    let mut theirs = Vec::new();
    for _ in 0..3 {
        theirs.extend(there.one_due_of_each().await);
    }
    for _ in 0..4 {
        theirs.push((
            "tool_call",
            there.tool_call(world.now() - 300 * DAY_MS).await,
        ));
    }
    // Threads of the other organization already left without a message, past their
    // retention: only the organization keeps a purge of threads from taking them, since they
    // have no message that keeps them.
    for _ in 0..3 {
        theirs.push((
            "peer_thread",
            there.thread(world.now() - 300 * DAY_MS).await,
        ));
    }
    let mut ours = Vec::new();
    for _ in 0..3 {
        ours.extend(here.one_due_of_each().await);
    }

    let purged = run(&world, 2).await;
    assert_eq!(purged.rows(), 18);
    assert_eq!(left(&world, &ours).await, []);
    assert_eq!(left(&world, &theirs).await, theirs);

    assert_eq!(run(&elsewhere, 2).await.rows(), 25);
    assert_eq!(left(&world, &theirs).await, []);
}

#[tokio::test]
async fn a_purge_asked_to_stop_stops_between_two_batches_and_the_next_run_ends_it() {
    let world = World::shared().await;
    let rows = Rows::new(&world).await;
    let now = world.now();
    let mut calls = Vec::new();
    for _ in 0..12 {
        calls.push(("tool_call", rows.tool_call(now - 8 * DAY_MS).await));
    }
    let later = rows.one_due_after_the_calls().await;
    let within = [
        ("tool_call", rows.tool_call(now).await),
        ("unknown_device", rows.unknown(now, false).await),
    ];
    let retention = purge::retention(world.pool()).await.unwrap();

    // The messages and the threads take one batch each; then two batches of calls, and the
    // question before the third one says stop.
    let mut asked = 0u32;
    let purged = purge::purge(world.pool(), world.org, &retention, now, 5, || {
        asked += 1;
        asked <= 4
    })
    .await
    .unwrap();
    assert!(purged.stopped);
    assert_eq!(
        purged.of(Target::ToolCalls),
        PurgedTarget {
            target: Target::ToolCalls,
            rows: 10,
            batches: 2
        }
    );
    // Whole batches: two of the calls are left, and nothing of the targets after them was
    // touched.
    assert_eq!(left(&world, &calls).await.len(), 2);
    assert_eq!(left(&world, &later).await, later);

    // The next run goes on from there and ends it.
    let purged = run(&world, 5).await;
    assert!(!purged.stopped);
    assert_eq!(left(&world, &calls).await, []);
    assert_eq!(left(&world, &later).await, []);
    assert_eq!(left(&world, &within).await, within);
}

#[tokio::test]
async fn a_batch_that_fails_deletes_nothing_and_the_next_run_ends_it() {
    // A trigger of the test refuses the delete of one row: it needs a database of its own.
    let world = World::own().await;
    let rows = Rows::new(&world).await;
    let now = world.now();
    let mut calls = Vec::new();
    for _ in 0..12 {
        calls.push(("tool_call", rows.tool_call(now - 8 * DAY_MS).await));
    }
    let refused = calls[7].1;
    let later = rows.one_due_after_the_calls().await;
    let within = ("tool_call", rows.tool_call(now).await);
    sqlx::query(AssertSqlSafe(format!(
        "CREATE FUNCTION refuse_one() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF OLD.id = '{refused}' THEN RAISE EXCEPTION 'refused by the test'; END IF; \
         RETURN OLD; END $$"
    )))
    .execute(world.pool())
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER refuse_one BEFORE DELETE ON tool_call \
         FOR EACH ROW EXECUTE FUNCTION refuse_one()",
    )
    .execute(world.pool())
    .await
    .unwrap();

    let retention = purge::retention(world.pool()).await.unwrap();
    let failed = purge::purge(world.pool(), world.org, &retention, now, 5, || true).await;
    assert!(matches!(failed, Err(RepoError::Database(_))), "{failed:?}");
    // The batch with the refused row deleted none of its rows: what went is whole batches.
    let calls_left = left(&world, &calls).await;
    assert!(calls_left.contains(&("tool_call", refused)));
    assert_eq!((calls.len() - calls_left.len()) % 5, 0, "{calls_left:?}");
    assert!(exists(&world, within.0, within.1).await);
    // The targets after it wait for the next run.
    assert_eq!(left(&world, &later).await, later);

    sqlx::query("DROP TRIGGER refuse_one ON tool_call")
        .execute(world.pool())
        .await
        .unwrap();
    let purged = run(&world, 5).await;
    assert!(!purged.stopped);
    assert_eq!(left(&world, &calls).await, []);
    assert_eq!(left(&world, &later).await, []);
    assert!(exists(&world, within.0, within.1).await);
}

#[tokio::test]
async fn a_table_without_its_retention_is_left_as_it_is() {
    let world = World::own().await;
    let rows = Rows::new(&world).await;
    let call = rows.tool_call(world.now() - 300 * DAY_MS).await;
    let unknown = rows.unknown(world.now() - 300 * DAY_MS, false).await;
    sqlx::query("DELETE FROM retention WHERE table_name = 'tool_call'")
        .execute(world.pool())
        .await
        .unwrap();
    let purged = run(&world, 500).await;
    assert_eq!(purged.of(Target::ToolCalls).batches, 0);
    assert!(exists(&world, "tool_call", call).await);
    assert!(!exists(&world, "unknown_device", unknown).await);
}

#[tokio::test]
async fn every_table_of_the_retention_is_purged_and_the_days_are_the_documented_ones() {
    let world = World::shared().await;
    let retention = purge::retention(world.pool()).await.unwrap();
    // `docs/15-mvp.md`, section 2, rule 11: messages 30 days, tool calls 7, resolved notices
    // 7, connections and unknown devices 90. The events of the engine come with their table.
    let days: Vec<(&str, Option<i64>)> = retention
        .tables()
        .map(|table| (table, retention.days(table)))
        .collect();
    assert_eq!(
        days,
        [
            ("channel_connection", Some(90)),
            ("notice", Some(7)),
            ("peer_message", Some(30)),
            ("tool_call", Some(7)),
            ("unknown_device", Some(90)),
        ]
    );
    let followed: BTreeSet<&str> = Target::ALL
        .iter()
        .map(|target| target.retention_row())
        .collect();
    let listed: BTreeSet<&str> = retention.tables().collect();
    assert_eq!(followed, listed, "every row of retention has its target");
}

// ---------------------------------------------------------------------------- the restored mode

/// A valid seal of the organization of `world` for `identity`.
fn seal(world: &World, identity: &PublicKey) -> Vec<u8> {
    world
        .key
        .seal_credential(identity, Generation::new(2).unwrap(), world.now())
        .unwrap()
        .sealed
        .expose()
        .to_vec()
}

/// An identity the database does not know presents a valid seal.
async fn presents(world: &World, identity: &PublicKey) -> (UnknownDevice, Option<Evidence>) {
    let sealed = seal(world, identity);
    let outcome = swap::look_unknown(
        world.pool(),
        world.org,
        &world.key,
        &Stranger {
            identity,
            presented: Some(&sealed),
            connector_version: None,
            address: address(77),
        },
        world.now(),
    )
    .await
    .unwrap();
    let UnknownOutcome::Listed { unknown, evidence } = outcome else {
        panic!("not listed: {outcome:?}");
    };
    (unknown, evidence)
}

#[tokio::test]
async fn a_pending_proof_of_a_restoration_is_never_purged() {
    let world = World::own().await;
    let who = identity();
    // The hub listed it and stopped before it turned the restored mode on.
    let (listed, evidence) = presents(&world, &who).await;
    assert!(listed.newly_listed && listed.proof_pending);
    let evidence = evidence.unwrap();

    world.clock.advance(1_000 * DAY_MS);
    assert_eq!(run(&world, 500).await.of(Target::UnknownDevices).rows, 0);
    // It is still proof: the next presentation gives the same evidence.
    let (again, evidence_again) = presents(&world, &who).await;
    assert_eq!(
        (again.id, again.newly_listed, again.proof_pending),
        (listed.id, false, true)
    );
    assert_eq!(evidence_again, Some(evidence));

    // A restored mode handles it; then it is a listing like any other, seen 1,000 days ago.
    let entered = restore::enter(world.pool(), &world.home.mark, world.now(), evidence)
        .await
        .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    assert_eq!(run(&world, 500).await.of(Target::UnknownDevices).rows, 1);
    assert!(!exists(&world, "unknown_device", listed.id).await);
}

/// The purge may delete a listing between the two statements of a presentation that meets it:
/// the insert finds the listing there, and the read that follows does not. The identity is
/// then listed anew, as if the purge had come a moment earlier, and the presentation does not
/// fail.
#[tokio::test]
async fn an_unknown_device_purged_while_it_presents_is_listed_anew() {
    // A trigger of the test plays the purge: it needs a database of its own.
    let world = World::own().await;
    let who = identity();
    // A listing a restored mode already handled, seen 91 days ago: past its retention.
    let old = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO unknown_device (id, organization_id, public_key, seen_at, proof_pending) \
         VALUES ($1, $2, $3, $4, FALSE)",
    )
    .bind(old)
    .bind(world.org.id())
    .bind(who.as_bytes().as_slice())
    .bind(world.now() - 91 * DAY_MS)
    .execute(world.pool())
    .await
    .unwrap();
    // Once, right after an insert of the presentation, the listing is deleted.
    for statement in [
        "CREATE TABLE purge_between (armed BOOLEAN NOT NULL)",
        "INSERT INTO purge_between VALUES (TRUE)",
        "CREATE FUNCTION purge_between() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF EXISTS (SELECT 1 FROM purge_between) THEN \
         DELETE FROM purge_between; DELETE FROM unknown_device WHERE NOT proof_pending; \
         END IF; RETURN NULL; END $$",
        "CREATE TRIGGER purge_between AFTER INSERT ON unknown_device \
         FOR EACH STATEMENT EXECUTE FUNCTION purge_between()",
    ] {
        sqlx::query(statement).execute(world.pool()).await.unwrap();
    }

    let (anew, evidence) = presents(&world, &who).await;
    let armed: i64 = sqlx::query_scalar("SELECT count(*) FROM purge_between")
        .fetch_one(world.pool())
        .await
        .unwrap();
    assert_eq!(armed, 0, "the purge of the test did not run");
    assert!(!exists(&world, "unknown_device", old).await);
    // Listed anew and, with the restored mode off, proof of a restoration.
    assert_ne!(anew.id, old);
    assert!(anew.newly_listed && anew.proof_pending);
    assert_eq!(
        evidence,
        Some(Evidence::UnknownDeviceSeal {
            unknown_device: anew.id
        })
    );
}

/// The second open point of `docs/14-protocolo-do-canal.md`, section 13, as the documents leave
/// it: the listing is kept 90 days from the first time the identity was seen. After the purge
/// the same identity is listed anew and, with the restored mode off, it is proof of a
/// restoration again: a new restored mode suspends every active device. This test says what
/// happens today; it changes with the decision.
#[tokio::test]
async fn an_unknown_device_purged_and_seen_again_is_proof_of_a_restoration_again() {
    let world = World::own().await;
    let who = identity();
    let (first, evidence) = presents(&world, &who).await;
    let entered = restore::enter(
        world.pool(),
        &world.home.mark,
        world.now(),
        evidence.unwrap(),
    )
    .await
    .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    // Nothing is suspended, so the mode ends at once.
    assert!(
        restore::end_when_done(world.pool(), &world.home.mark, world.now())
            .await
            .unwrap()
    );
    // Seen again with the mode off, a listing a restored mode already handled is no proof:
    // its first time stays.
    world.clock.advance(10 * DAY_MS);
    let (again, evidence) = presents(&world, &who).await;
    assert_eq!(
        (again.id, again.newly_listed, again.proof_pending, evidence),
        (first.id, false, false, None)
    );

    world.clock.advance(80 * DAY_MS - 1);
    assert_eq!(run(&world, 500).await.of(Target::UnknownDevices).rows, 0);
    world.clock.advance(1);
    assert_eq!(run(&world, 500).await.of(Target::UnknownDevices).rows, 1);

    // A Hermes connected meanwhile.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    // The same identity presents its seal again: listed anew, and proof once more.
    let (anew, evidence) = presents(&world, &who).await;
    assert_ne!(anew.id, first.id);
    assert!(anew.newly_listed && anew.proof_pending);
    assert_eq!(
        evidence,
        Some(Evidence::UnknownDeviceSeal {
            unknown_device: anew.id
        })
    );
    let entered = restore::enter(
        world.pool(),
        &world.home.mark,
        world.now(),
        evidence.unwrap(),
    )
    .await
    .unwrap();
    let EnterOutcome::Entered(entered) = entered else {
        panic!("the mode was not turned on: {entered:?}");
    };
    assert_eq!(entered.suspended, 1);
    assert_eq!(
        world.device(active.device).await.state,
        DeviceState::Suspended
    );
    assert!(
        restore::installation(world.pool())
            .await
            .unwrap()
            .unwrap()
            .restored_mode
    );
}
