//! Random rounds of Publish against the commands that change the same rows at the same time.
//!
//! Each round builds an organization with a few Hermes, a few links and, half the time, a chart;
//! then it starts a Publish and, with random delays, one to five commands (a link set or taken,
//! a Hermes created or removed, a second Publish from the same base). What must hold, whatever
//! the order the database picked:
//!
//! - every call ends: no deadlock and no lock timeout;
//! - a Publish is `Published`, `Stale` or `Unchanged`, and two Publishes from one base never both
//!   publish;
//! - the audit of the links, replayed in order, gives exactly the links the table has, and stopped
//!   at the summary of a Publish it gives exactly the links of its draft: no half a link set ever
//!   committed;
//! - after a Publish its levels are the levels of the chart, and every Hermes still live and
//!   alive when it ran has the place and the role its draft gave;
//! - a refused Publish wrote no event of the chart: there is one summary per Publish that
//!   published.
//!
//! A failing round prints its seed. `ARAUTO_ORG_ROUND=<seed>` replays that one round,
//! `ARAUTO_TEST_SEED=<seed>` moves the whole run to other rounds, `ARAUTO_ORG_ROUNDS=<count>`
//! runs more of them.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use arauto_hub::core::org::{Chart, Draft};
use arauto_hub::repo::hermes;
use arauto_hub::repo::org::{self, PublishOutcome};
use arauto_hub::repo::peer;
use arauto_protocol::clock::SeededRng;
use arauto_protocol::names::HermesName;
use futures_util::future::join_all;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::org::{chart, draft, keep};
use crate::support::World;

const ROUNDS: u64 = 200;
const WORKERS: u64 = 4;
const DEFAULT_SEED: u64 = 0x4f52_4743_4852;
const ROLES: [&str; 4] = ["", "alpha", "beta gamma", "delta"];

fn number_from(variable: &str) -> Option<u64> {
    let text = std::env::var(variable).ok()?;
    Some(
        text.trim()
            .parse()
            .unwrap_or_else(|_| panic!("{variable} must be a number, and it is {text:?}")),
    )
}

/// A random draft over the chart as read: some levels kept (maybe renamed, maybe their names
/// swapped), some new, the Hermes anywhere, some links.
fn random_draft(chart: &Chart, rng: &mut SeededRng, tag: &str) -> Draft {
    let mut levels: Vec<Value> = Vec::new();
    let mut stored: Vec<_> = chart.levels.iter().collect();
    // Shuffle.
    for at in (1..stored.len()).rev() {
        let other = usize::try_from(rng.below(at as u64 + 1)).unwrap();
        stored.swap(at, other);
    }
    let mut taken = 0;
    for level in stored {
        if rng.below(4) == 0 {
            continue;
        }
        let name = match rng.below(3) {
            0 => format!("{tag}-r{taken}"),
            _ => level.name.as_str().to_owned(),
        };
        levels.push(json!({ "key": level.id.to_string(), "name": name }));
        taken += 1;
    }
    // Two names swapped between kept levels now and then: the unique name is deferred.
    if levels.len() >= 2 && rng.below(3) == 0 {
        let (a, b) = (levels[0]["name"].clone(), levels[1]["name"].clone());
        levels[0]["name"] = b;
        levels[1]["name"] = a;
    }
    let new = usize::try_from(rng.below(3))
        .unwrap()
        .max(usize::from(levels.is_empty()));
    for at in 0..new.min(12 - levels.len()) {
        levels.push(json!({ "key": format!("new-{}", at + 1), "name": format!("{tag}-n{at}") }));
    }
    let keys: Vec<Value> = levels.iter().map(|level| level["key"].clone()).collect();
    let hermes: Vec<Value> = chart
        .members
        .iter()
        .map(|member| {
            json!({
                "name": member.name.as_str(),
                "level": keys[usize::try_from(rng.below(keys.len() as u64)).unwrap()],
                "x": rng.below(4_001),
                "role": ROLES[usize::try_from(rng.below(4)).unwrap()],
            })
        })
        .collect();
    let mut links: Vec<Value> = Vec::new();
    for from in &chart.members {
        for to in &chart.members {
            if from.id != to.id && rng.below(4) == 0 {
                links.push(json!({
                    "from": from.name.as_str(), "to": to.name.as_str(), "wake": rng.below(2) == 0
                }));
            }
        }
    }
    draft(&json!({ "levels": levels, "hermes": hermes, "links": links }))
}

type Call<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Option<(PublishOutcome, Draft)>, String>> + 'a>,
>;

#[derive(Debug)]
enum Op {
    Set(usize, usize, bool),
    Remove(usize, usize),
    /// Was the switch of a Hermes (decision 0049): does nothing, keeps the seeds.
    Nothing,
    Create,
    RemoveHermes(usize),
    SecondPublish(Draft),
}

/// What one round did, to show the rounds reach what they are meant to reach.
#[derive(Debug, Default, Clone, Copy)]
struct Seen {
    rounds: u64,
    published: u64,
    stale: u64,
    unchanged: u64,
}

async fn replay(world: &World, seed: u64) -> Result<Seen, String> {
    let mut rng = SeededRng::new(seed);
    let tag = format!("r{}", seed % 100_000);
    let count = 3 + usize::try_from(rng.below(4)).unwrap();
    let mut names: Vec<HermesName> = Vec::new();
    for _ in 0..count {
        names.push(world.hermes().await);
    }
    let pool = world.pool();
    let (mark, org_, admin) = (&world.home.mark, world.org, &world.admin);
    for from in &names {
        for to in &names {
            if from != to && rng.below(4) == 0 {
                peer::link_set(
                    pool,
                    mark,
                    org_,
                    admin,
                    from,
                    to,
                    rng.below(2) == 0,
                    world.now(),
                )
                .await
                .map_err(|e| format!("setup: {e}"))?;
            }
        }
    }
    let mut seen = Seen {
        rounds: 1,
        ..Seen::default()
    };
    let mut setup_published = 0_u64;
    let mut chart0 = chart(world).await;
    if rng.below(2) == 0 {
        let wanted = random_draft(&chart0, &mut rng, &tag);
        if let PublishOutcome::Published(_) = world_publish(world, &wanted, &chart0).await? {
            setup_published += 1;
        }
        chart0 = chart(world).await;
    }

    let mine = random_draft(&chart0, &mut rng, &format!("{tag}a"));
    let mut ops: Vec<(u64, Op)> = Vec::new();
    let live = chart0.members.len();
    for _ in 0..=usize::try_from(rng.below(5)).unwrap() {
        let delay = rng.below(21);
        let pick = |rng: &mut SeededRng| usize::try_from(rng.below(live as u64)).unwrap();
        let op = match rng.below(6) {
            0 => Op::Set(pick(&mut rng), pick(&mut rng), rng.below(2) == 0),
            1 => Op::Remove(pick(&mut rng), pick(&mut rng)),
            2 => {
                let _ = (pick(&mut rng), rng.below(2));
                Op::Nothing
            }
            3 => Op::Create,
            4 => Op::RemoveHermes(pick(&mut rng)),
            _ => Op::SecondPublish(random_draft(&chart0, &mut rng, &format!("{tag}b"))),
        };
        ops.push((delay, op));
        // Some are repeated.
        if rng.below(5) == 0 {
            let again = ops.last().unwrap();
            if let Op::Set(a, b, w) = again.1 {
                ops.push((rng.below(21), Op::Set(a, b, w)));
            }
        }
    }
    let publish_delay = rng.below(21);

    let names_ref = &chart0.members;
    let chart_ref = &chart0;
    let mut calls: Vec<Call<'_>> = Vec::new();
    calls.push(Box::pin(async move {
        tokio::time::sleep(Duration::from_millis(publish_delay)).await;
        let outcome = world_publish(world, &mine, chart_ref).await?;
        Ok(Some((outcome, mine)))
    }));
    for (delay, op) in ops {
        calls.push(Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let name = |at: usize| names_ref[at].name.clone();
            match op {
                Op::Set(a, b, wake) => {
                    peer::link_set(
                        pool,
                        mark,
                        org_,
                        admin,
                        &name(a),
                        &name(b),
                        wake,
                        world.now(),
                    )
                    .await
                    .map_err(|e| format!("link_set: {e}"))?;
                }
                Op::Remove(a, b) => {
                    peer::link_remove(pool, mark, org_, admin, &name(a), &name(b), world.now())
                        .await
                        .map_err(|e| format!("link_remove: {e}"))?;
                }
                Op::Nothing => {}
                Op::Create => {
                    world.hermes().await;
                }
                Op::RemoveHermes(a) => {
                    hermes::remove(pool, mark, org_, admin, &name(a), world.now())
                        .await
                        .map_err(|e| format!("remove: {e}"))?;
                }
                Op::SecondPublish(second) => {
                    let outcome = world_publish(world, &second, chart_ref).await?;
                    return Ok(Some((outcome, second)));
                }
            }
            Ok(None)
        }));
    }
    let results = tokio::time::timeout(Duration::from_secs(90), join_all(calls))
        .await
        .map_err(|_| "a call did not end: a deadlock or a lock that never freed".to_owned())?;
    let mut publishes: Vec<(PublishOutcome, Draft)> = Vec::new();
    for result in results {
        if let Some((outcome, wanted)) = result? {
            publishes.push((outcome, wanted));
        }
    }

    // (b) Published, Stale or Unchanged; never two Published from one base.
    let published: Vec<&(PublishOutcome, Draft)> = publishes
        .iter()
        .filter(|(outcome, _)| matches!(outcome, PublishOutcome::Published(_)))
        .collect();
    for (outcome, _) in &publishes {
        match outcome {
            PublishOutcome::Published(_) => seen.published += 1,
            PublishOutcome::Stale => seen.stale += 1,
            PublishOutcome::Unchanged => seen.unchanged += 1,
            PublishOutcome::Invalid(reason) => return Err(format!("invalid: {reason}")),
        }
    }
    if published.len() > 1 {
        return Err("two publishes from one base both published".to_owned());
    }

    // (e) One summary per publish that published, and none for the others.
    let summaries: Vec<(i64, Uuid)> = sqlx::query_as(
        "SELECT seq, target_id FROM audit WHERE organization_id = $1 AND action = 'org_chart.published' ORDER BY seq",
    )
    .bind(world.org.id())
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    if u64::try_from(summaries.len()).unwrap()
        != setup_published + u64::try_from(published.len()).unwrap()
    {
        return Err(format!(
            "{} summaries for {} publishes",
            summaries.len(),
            setup_published + published.len() as u64
        ));
    }

    // (c) The audit of the links, replayed in order.
    let ids: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM hermes WHERE organization_id = $1")
            .bind(world.org.id())
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    let id_of: BTreeMap<String, Uuid> = ids.iter().map(|(id, name)| (name.clone(), *id)).collect();
    let events: Vec<(i64, String, Uuid, Value)> = sqlx::query_as(
        "SELECT seq, action, target_id, details::jsonb FROM audit \
         WHERE organization_id = $1 AND action LIKE 'peer_link.%' ORDER BY seq",
    )
    .bind(world.org.id())
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let replay_to = |limit: i64| -> BTreeMap<(Uuid, Uuid), bool> {
        let mut links: BTreeMap<(Uuid, Uuid), bool> = BTreeMap::new();
        for (seq, action, from, details) in &events {
            if *seq > limit {
                break;
            }
            let to = details
                .get("to_id")
                .or_else(|| details.get("to"))
                .and_then(Value::as_str)
                .and_then(|text| Uuid::parse_str(text).ok())
                .expect("a link event names its recipient");
            match action.as_str() {
                "peer_link.created" | "peer_link.changed" => {
                    links.insert((*from, to), details["wake"].as_bool().unwrap());
                }
                _ => {
                    links.remove(&(*from, to));
                }
            }
        }
        links
    };
    let table: Vec<(Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT from_hermes_id, to_hermes_id, wake FROM peer_link WHERE organization_id = $1",
    )
    .bind(world.org.id())
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let table: BTreeMap<(Uuid, Uuid), bool> = table
        .into_iter()
        .map(|(from, to, wake)| ((from, to), wake))
        .collect();
    if replay_to(i64::MAX) != table {
        return Err("the audit of the links does not give the links of the table".to_owned());
    }
    if let Some(&(_, wanted)) = published.first() {
        // The summary of this publish is the last of its transaction: the links at that point are
        // exactly the links of its draft.
        let last = summaries.last().map(|(seq, _)| *seq).unwrap();
        let expected: BTreeMap<(Uuid, Uuid), bool> = wanted
            .links
            .iter()
            .map(|l| ((id_of[l.from.as_str()], id_of[l.to.as_str()]), l.wake))
            .collect();
        if replay_to(last) != expected {
            return Err("at its summary the links are not the links of its draft".to_owned());
        }

        // (d) The chart is the draft, for every Hermes still live.
        let now = chart(world).await;
        let names_now: Vec<&str> = now.levels.iter().map(|l| l.name.as_str()).collect();
        let names_draft: Vec<&str> = wanted.levels.iter().map(|l| l.name.as_str()).collect();
        if names_now != names_draft {
            return Err(format!("levels {names_now:?} instead of {names_draft:?}"));
        }
        let live: BTreeSet<&str> = now.members.iter().map(|m| m.name.as_str()).collect();
        for member in &wanted.hermes {
            if !live.contains(member.name.as_str()) {
                continue;
            }
            let stored = now.members.iter().find(|m| m.name == member.name).unwrap();
            let place = stored
                .place
                .as_ref()
                .ok_or("a Hermes of the draft has no place")?;
            let level = now
                .position_of(place.level)
                .ok_or("a place points to no level")?;
            let wanted_level = wanted
                .levels
                .iter()
                .position(|l| l.key == member.level)
                .unwrap();
            if level != wanted_level
                || place.x != member.x
                || place.role.as_ref().map(|r| r.as_str())
                    != member.role.as_ref().map(|r| r.as_str())
            {
                return Err(format!("{} is not as its draft says", member.name));
            }
        }
    }
    // The positions have no gap.
    let positions: Vec<i32> = sqlx::query_scalar(
        "SELECT position FROM org_level WHERE organization_id = $1 ORDER BY position",
    )
    .bind(world.org.id())
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    if positions != (0..i32::try_from(positions.len()).unwrap()).collect::<Vec<_>>() {
        return Err(format!("positions {positions:?}"));
    }
    Ok(seen)
}

async fn world_publish(
    world: &World,
    wanted: &Draft,
    base: &Chart,
) -> Result<PublishOutcome, String> {
    org::publish(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        wanted,
        base.version(),
        world.now(),
    )
    .await
    .map_err(|e| format!("publish: {e}"))
}

#[allow(dead_code)]
fn unused(_: &Value) {
    let _ = keep;
}

#[tokio::test(flavor = "multi_thread")]
async fn random_rounds_of_publish_and_link_commands_never_leave_half_a_chart() {
    if let Some(seed) = number_from("ARAUTO_ORG_ROUND") {
        let world = World::shared().await;
        let seen = replay(&world, seed).await;
        assert!(seen.is_ok(), "seed {seed}: {seen:?}");
        return;
    }
    let seed = number_from("ARAUTO_TEST_SEED").unwrap_or(DEFAULT_SEED);
    let rounds = number_from("ARAUTO_ORG_ROUNDS").unwrap_or(ROUNDS);
    let workers = (0..WORKERS).map(|worker| async move {
        let mut total = Seen::default();
        let mut round = worker;
        while round < rounds {
            let world = World::shared().await;
            let round_seed = seed.wrapping_add(round.wrapping_mul(0x9E37_79B9));
            match replay(&world, round_seed).await {
                Ok(seen) => {
                    total.rounds += seen.rounds;
                    total.published += seen.published;
                    total.stale += seen.stale;
                    total.unchanged += seen.unchanged;
                }
                Err(why) => panic!(
                    "round {round} failed: {why}
replay it with ARAUTO_ORG_ROUND={round_seed}"
                ),
            }
            round += WORKERS;
        }
        total
    });
    let mut total = Seen::default();
    for seen in join_all(workers).await {
        total.rounds += seen.rounds;
        total.published += seen.published;
        total.stale += seen.stale;
        total.unchanged += seen.unchanged;
    }
    assert_eq!(total.rounds, rounds);
    // The rounds reach both ends: a publish that wins and one that is refused as stale.
    assert!(total.published > 0, "{total:?}");
    assert!(total.stale > 0, "{total:?}");
}
