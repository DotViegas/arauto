//! Random rounds of the swap, against the real database.
//!
//! The simulator of `arauto_hub::core::swap_model` runs a round in memory and reports every
//! write its hub made: each T1, each 24-hour swap, each T2 and each double connection. The
//! same steps are then applied to PostgreSQL through the repository functions, and after
//! every one of them the row of the device must be what the model says: the same state, the
//! same generations, and the credentials that correspond.
//!
//! The model decides with the pure rule; the database decides again, under its locks, with
//! what is in the row. A statement that wrote something else than the rule returned, or that
//! read less than the rule needs, makes the two disagree.
//!
//! A failing round prints its seed. `ARAUTO_DB_ROUND=<seed>` replays that one round,
//! `ARAUTO_TEST_SEED=<seed>` moves the whole run to other rounds, and
//! `ARAUTO_DB_ROUNDS=<count>` runs more of them.

use arauto_hub::core::judge::{Credential, Decision, DeviceSnapshot};
use arauto_hub::core::model::DeviceState;
use arauto_hub::core::swap_model::{Chooser, Documented, HubStep, run_round_logged};
use arauto_hub::repo::connection::ChannelConnection;
use arauto_hub::repo::swap::{
    self, ConfirmOutcome, DoubleConnectionOutcome, IssueOutcome, IssuedHere,
};
use arauto_protocol::clock::SeededRng;
use arauto_protocol::message::{Generation, Saved};
use arauto_protocol::noise::PublicKey;
use arauto_protocol::secret::SealedCredential;
use uuid::Uuid;

use crate::common::{self, NOW};
use crate::support::{World, identity};

/// How many rounds go through the database, at least.
const ROUNDS: u64 = 300;

/// How many organizations replay rounds at the same time, each with its own connections.
const WORKERS: u64 = 6;

/// The seed of the run when `ARAUTO_TEST_SEED` does not give another.
const DEFAULT_SEED: u64 = 0x4152_4155_544f;

struct Seeded(SeededRng);

impl Chooser for Seeded {
    fn below(&mut self, bound: u64) -> u64 {
        self.0.below(bound)
    }
}

fn number_from(variable: &str) -> Option<u64> {
    let text = std::env::var(variable).ok()?;
    Some(
        text.trim()
            .parse()
            .unwrap_or_else(|_| panic!("{variable} must be a number, and it is {text:?}")),
    )
}

/// What a replay did, to show that the rounds reach what they are meant to reach.
#[derive(Debug, Default, Clone, Copy)]
struct Replayed {
    rounds: u64,
    first_transactions: u64,
    issued: u64,
    promotions: u64,
    rotations: u64,
    confirmed: u64,
    saved_refused: u64,
    quarantines: u64,
    double_connections: u64,
    started_active: u64,
    ended_active: u64,
}

impl Replayed {
    fn add(&mut self, other: &Replayed) {
        self.rounds += other.rounds;
        self.first_transactions += other.first_transactions;
        self.issued += other.issued;
        self.promotions += other.promotions;
        self.rotations += other.rotations;
        self.confirmed += other.confirmed;
        self.saved_refused += other.saved_refused;
        self.quarantines += other.quarantines;
        self.double_connections += other.double_connections;
        self.started_active += other.started_active;
        self.ended_active += other.ended_active;
    }
}

/// One credential of the model and the real one that stands for it.
struct Pair {
    model: Credential,
    real: Credential,
    sealed: SealedCredential,
    /// What the connection that received it holds, for T2.
    receipt: Option<(IssuedHere, ChannelConnection)>,
}

/// The device of one round: its row, its identity and every credential issued so far.
struct Replay<'a> {
    world: &'a World,
    device: Uuid,
    pairs: Vec<Pair>,
    /// The connection on which the last swap was confirmed: the session of the device. The
    /// 24-hour swap only runs there, and the database refuses it anywhere else.
    session: Option<ChannelConnection>,
}

impl Replay<'_> {
    fn pair(&self, model: &Credential) -> Result<&Pair, String> {
        self.pairs
            .iter()
            .find(|pair| pair.model == *model)
            .ok_or_else(|| format!("the model used a credential nobody issued: {model:?}"))
    }

    fn real(&self, model: Option<Credential>) -> Result<Option<Credential>, String> {
        model
            .map(|model| self.pair(&model).map(|pair| pair.real))
            .transpose()
    }

    /// The row of the device must be what the model has after the same step.
    async fn compare(&self, after: &DeviceSnapshot, step: &HubStep) -> Result<(), String> {
        let row = self.world.device(self.device).await;
        let expected = DeviceSnapshot {
            state: after.state,
            current: self.real(after.current)?,
            pending: self.real(after.pending)?,
            max_generation: after.max_generation,
            marked_by_restoration: after.marked_by_restoration,
        };
        if row.snapshot() != expected {
            return Err(format!(
                "after {step:?}\n  the database has {:?}\n  and the model has {expected:?}",
                row.snapshot()
            ));
        }
        // What the schema ties to the state follows too.
        let in_use = row.slot == Some(arauto_hub::core::model::Slot::InUse);
        if in_use != row.first_confirmed_at.is_some() || in_use != after.current.is_some() {
            return Err(format!(
                "after {step:?} the place of the device is wrong: {row:?}"
            ));
        }
        Ok(())
    }

    fn keep(&mut self, model: Credential, issued: swap::Issued, connection: ChannelConnection) {
        self.pairs.push(Pair {
            model,
            real: issued.receipt.credential(),
            sealed: issued.sealed,
            receipt: Some((issued.receipt, connection)),
        });
    }
}

/// Creates the device of a round as the model starts it: approved at generation zero, or
/// already in use with a current credential of some generation.
async fn start(world: &World, start: &DeviceSnapshot) -> (Uuid, PublicKey, Vec<Pair>) {
    let hermes = common::hermes(world.pool(), world.org).await;
    let who = identity();
    let device = Uuid::new_v4();
    let mut pairs = Vec::new();
    let (state, slot, confirmed_at, approved_until) = match start.state {
        DeviceState::Approved => ("approved", "request", None, Some(NOW + 86_400_000)),
        DeviceState::Active => ("active", "in_use", Some(NOW), None),
        other => panic!("the model starts a round with a device that is {other}"),
    };
    let current = start.current.map(|model| {
        let seal = world
            .key
            .seal_credential(&who, model.generation, NOW)
            .unwrap();
        let real = Credential {
            generation: seal.generation,
            digest: seal.digest,
        };
        pairs.push(Pair {
            model,
            real,
            sealed: seal.sealed,
            receipt: None,
        });
        real
    });
    assert_eq!(start.pending, None, "the model starts with nothing pending");
    sqlx::query(
        "INSERT INTO device (id, organization_id, hermes_id, public_key, state, state_since, \
                             slot, first_confirmed_at, approved_until, current_generation, \
                             current_digest, max_generation, created_at, last_contact_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $6, $8)",
    )
    .bind(device)
    .bind(world.org.id())
    .bind(hermes)
    .bind(who.as_bytes().as_slice())
    .bind(state)
    .bind(NOW)
    .bind(slot)
    .bind(confirmed_at)
    .bind(approved_until)
    .bind(current.map_or(0, |c| c.generation.as_i64()))
    .bind(current.map(|c| c.digest.as_bytes().to_vec()))
    .bind(i64::try_from(start.max_generation).unwrap())
    .execute(world.pool())
    .await
    .unwrap();
    (device, who, pairs)
}

/// Runs the round of `seed` in the model and applies its steps to the database.
async fn replay(world: &World, seed: u64) -> Result<Replayed, String> {
    let (steps, outcome) = run_round_logged(&Documented, &mut Seeded(SeededRng::new(seed)));
    if let Err(failure) = outcome {
        return Err(format!("the model itself broke an invariant:\n{failure}"));
    }
    let mut steps = steps.into_iter();
    let Some(HubStep::Start(first)) = steps.next() else {
        return Err("the round does not begin with the device".to_owned());
    };
    let (device, _, pairs) = start(world, &first).await;
    let mut replay = Replay {
        world,
        device,
        pairs,
        session: None,
    };
    let mut done = Replayed {
        rounds: 1,
        started_active: u64::from(first.state == DeviceState::Active),
        ..Replayed::default()
    };
    replay.compare(&first, &HubStep::Start(first)).await?;
    let mut last = first;

    for step in steps {
        let after = match step {
            HubStep::Start(_) => return Err("a round starts once".to_owned()),
            HubStep::Presented {
                presented,
                decision,
                issued,
                after,
            } => {
                done.first_transactions += 1;
                let sealed = match presented {
                    Some(model) => Some(
                        SealedCredential::from_slice(replay.pair(&model)?.sealed.expose()).unwrap(),
                    ),
                    None => None,
                };
                let connection = world.connect(device).await;
                let outcome = world.issue(&connection, sealed.as_ref()).await;
                match (decision, issued, outcome) {
                    (
                        Decision::Accept { promote_pending },
                        Some(model),
                        IssueOutcome::Issued(real),
                    ) => {
                        if real.promoted != promote_pending {
                            return Err(format!("{step:?}: the database promoted otherwise"));
                        }
                        done.issued += 1;
                        done.promotions += u64::from(promote_pending);
                        replay.keep(model, *real, connection);
                    }
                    (Decision::Quarantine(_), None, IssueOutcome::Quarantined(_)) => {
                        done.quarantines += 1;
                    }
                    (
                        Decision::Nothing | Decision::ReuseAfterClosing | Decision::RecordOnly,
                        None,
                        IssueOutcome::NotSwapped(_),
                    ) => {}
                    (_, _, outcome) => {
                        return Err(format!("{step:?}\n  and the database answered {outcome:?}"));
                    }
                }
                after
            }
            HubStep::Rotated { issued, after } => {
                done.rotations += 1;
                let connection = replay
                    .session
                    .clone()
                    .ok_or_else(|| format!("{step:?}: the model rotated outside a session"))?;
                let outcome = swap::rotate(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    &world.key,
                    &connection,
                    world.now(),
                )
                .await
                .map_err(|error| format!("{step:?}: {error}"))?;
                let IssueOutcome::Issued(real) = outcome else {
                    return Err(format!("{step:?}\n  and the database answered {outcome:?}"));
                };
                replay.keep(issued, *real, connection);
                after
            }
            HubStep::Saved {
                issued_here,
                saved,
                accepted,
                after,
            } => {
                let said = replay.pair(&saved)?.real;
                let (receipt, connection) =
                    replay.pair(&issued_here)?.receipt.as_ref().ok_or_else(|| {
                        format!("{step:?}: no connection received that credential")
                    })?;
                let outcome = swap::confirm(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    connection,
                    receipt,
                    &Saved {
                        generation: said.generation,
                        digest: said.digest,
                    },
                    world.now(),
                )
                .await
                .map_err(|error| format!("{step:?}: {error}"))?;
                let confirmed = matches!(outcome, ConfirmOutcome::Confirmed { .. });
                if confirmed != accepted {
                    return Err(format!("{step:?}\n  and the database answered {outcome:?}"));
                }
                if confirmed {
                    replay.session = Some(connection.clone());
                }
                done.confirmed += u64::from(confirmed);
                done.saved_refused += u64::from(!confirmed);
                after
            }
            HubStep::DoubleConnection { after } => {
                done.double_connections += 1;
                let outcome = swap::quarantine_double_connection(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    device,
                    None,
                    world.now(),
                )
                .await
                .map_err(|error| format!("{step:?}: {error}"))?;
                if outcome != DoubleConnectionOutcome::Quarantined {
                    return Err(format!("{step:?}\n  and the database answered {outcome:?}"));
                }
                after
            }
        };
        replay.compare(&after, &step).await?;
        last = after;
    }
    done.ended_active = u64::from(last.state == DeviceState::Active);
    Ok(done)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn random_rounds_end_in_the_database_as_they_end_in_the_model() {
    if let Some(seed) = number_from("ARAUTO_DB_ROUND") {
        let world = World::shared().await;
        match replay(&world, seed).await {
            Ok(done) => println!("round {seed} ends the same in the database: {done:#?}"),
            Err(why) => panic!("round {seed} differs in the database.\n{why}"),
        }
        return;
    }
    let base = number_from("ARAUTO_TEST_SEED").unwrap_or(DEFAULT_SEED);
    let rounds = number_from("ARAUTO_DB_ROUNDS").map_or(ROUNDS, |asked| asked.max(ROUNDS));

    let mut workers = Vec::new();
    for worker in 0..WORKERS {
        workers.push(tokio::spawn(async move {
            let world = World::shared().await;
            let mut total = Replayed::default();
            // Every worker takes the rounds whose number leaves its remainder.
            for number in (worker..rounds).step_by(WORKERS as usize) {
                let seed = base.wrapping_add(number);
                match replay(&world, seed).await {
                    Ok(done) => total.add(&done),
                    Err(why) => return Err((seed, why)),
                }
            }
            Ok(total)
        }));
    }
    let mut total = Replayed::default();
    for worker in workers {
        match worker.await.expect("a worker does not panic") {
            Ok(done) => total.add(&done),
            Err((seed, why)) => panic!(
                "the round of seed {seed} ends in the database differently from the model; \
                 replay it with ARAUTO_DB_ROUND={seed}\n{why}"
            ),
        }
    }
    println!("{rounds} rounds from seed {base} (ARAUTO_TEST_SEED picks another): {total:#?}");

    // The rounds must reach what they are meant to try.
    assert_eq!(total.rounds, rounds);
    let reached: &[(&str, u64, u64)] = &[
        ("first transactions", total.first_transactions, rounds * 3),
        ("credentials issued", total.issued, rounds * 3),
        ("promotions by presentation", total.promotions, rounds / 2),
        ("24-hour swaps", total.rotations, rounds * 2),
        ("swaps confirmed", total.confirmed, rounds * 4),
        ("quarantines", total.quarantines, rounds / 4),
        (
            "rounds that began with a device in use",
            total.started_active,
            rounds / 4,
        ),
        (
            "rounds that ended with an active device",
            total.ended_active,
            rounds / 4,
        ),
    ];
    for (what, count, floor) in reached {
        assert!(
            count >= floor,
            "only {count} {what} in {rounds} rounds (at least {floor})"
        );
    }
}

#[tokio::test]
async fn a_database_that_writes_something_else_than_the_model_is_caught() {
    // The comparison itself: a row changed behind the back of the replay does not pass.
    let world = World::shared().await;
    let model = DeviceSnapshot {
        state: DeviceState::Active,
        current: Some(Credential {
            generation: Generation::new(2).unwrap(),
            digest: arauto_protocol::digest::CredentialDigest::from_bytes([1; 32]),
        }),
        pending: None,
        max_generation: 2,
        marked_by_restoration: false,
    };
    let (device, _, pairs) = start(&world, &model).await;
    let replay = Replay {
        world: &world,
        device,
        pairs,
        session: None,
    };
    let step = HubStep::Start(model);
    assert_eq!(replay.compare(&model, &step).await, Ok(()));

    for (change, what) in [
        ("max_generation = 3", "a larger generation"),
        (
            "current_digest = decode(repeat('ab', 32), 'hex')",
            "another digest",
        ),
        ("marked_by_restoration = TRUE", "the mark of a restoration"),
        ("state = 'quarantined'", "another state"),
    ] {
        let mut transaction = world.pool().begin().await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE device SET {change} WHERE id = $1"
        )))
        .bind(device)
        .execute(&mut *transaction)
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        assert!(
            replay.compare(&model, &step).await.is_err(),
            "{what} went unnoticed"
        );
        // Back to what the model has.
        let real = replay.pairs[0].real;
        sqlx::query(
            "UPDATE device SET max_generation = 2, current_digest = $2, state = 'active', \
                 marked_by_restoration = FALSE WHERE id = $1",
        )
        .bind(device)
        .bind(real.digest.as_bytes().as_slice())
        .execute(world.pool())
        .await
        .unwrap();
    }
    assert_eq!(replay.compare(&model, &step).await, Ok(()));
}
