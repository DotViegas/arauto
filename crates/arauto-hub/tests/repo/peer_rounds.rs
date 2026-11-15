//! Random rounds of the notices between Hermes, against the real database
//! (`docs/13-conversa-entre-hermes.md`; decision 0045).
//!
//! Each round has three Hermes with an active device and runs twice, with the same seed and the
//! same explicit clock: the second run ("twin B") never touches the switch "woken by
//! WhatsApp", and the first one flips it at random. A note wakes by the mark of its link and
//! by nothing else, so the two normalized traces must be equal.
//!
//! The steps: a link with or without the mark, a link removed, the switch (twin A only), a
//! note, a lease, an answer (any of the five, none at all, or the same one twice), a repeated
//! lease, and time that moves by 0, 1, 5, 11 or 61 seconds or by an hour and a second.
//!
//! After every lease: a notice that leaves had, at that moment, a live link from its sender
//! with the mark (I1); two different notices of the same pair never leave less than ten
//! seconds apart (I2). At the end, in the table: no pair of Hermes got more than 10 new
//! notices in a sliding hour and no recipient more than 30 (I3); and the twins agree (I4).
//!
//! A failing round prints its seed. `ARAUTO_TEST_SEED=<seed>` moves the run to other rounds
//! and `ARAUTO_PEER_ROUNDS=<count>` runs more of them.

use std::collections::HashMap;

use arauto_hub::repo;
use arauto_hub::repo::peer::{LinkRemoved, LinkSet, NewMessage, NoticeTarget, Sent};
use arauto_protocol::clock::SeededRng;
use arauto_protocol::names::{HermesName, MessageText};
use arauto_protocol::notice::AckOutcome;
use uuid::Uuid;

use crate::support::World;

const DEFAULT_SEED: u64 = 0x5045_4552_0045;
const DEFAULT_ROUNDS: u64 = 30;
const STEPS: u64 = 60;
const HOUR: i64 = 3_600_000;
const GAP: i64 = 10_000;
const TIMES: [i64; 6] = [0, 1_000, 5_000, 11_000, 61_000, HOUR + 1_000];
const OUTCOMES: [AckOutcome; 5] = [
    AckOutcome::Accepted,
    AckOutcome::Failed,
    AckOutcome::Throttled,
    AckOutcome::Rejected,
    AckOutcome::Duplicate,
];

fn number_from(variable: &str) -> Option<u64> {
    let text = std::env::var(variable).ok()?;
    Some(
        text.trim()
            .parse()
            .unwrap_or_else(|_| panic!("{variable} must be a number, and it is {text:?}")),
    )
}

/// One run of a round.
struct Run {
    world: World,
    names: Vec<HermesName>,
    ids: Vec<Uuid>,
    /// The links the test believes there are: `links[from][to]` is the wake mark.
    links: [[Option<bool>; 3]; 3],
    now: i64,
    /// The notices seen, in order of first appearance.
    seen: Vec<Uuid>,
    /// The notices leased and not yet answered, by Hermes.
    out: [Vec<Uuid>; 3],
    /// When each notice last left, with its pair.
    leases: Vec<(Uuid, usize, usize, i64)>,
    trace: Vec<String>,
}

impl Run {
    async fn new() -> Self {
        let world = World::shared().await;
        let mut names = Vec::new();
        let mut ids = Vec::new();
        for _ in 0..3 {
            let name = world.hermes().await;
            world.active(&name).await;
            ids.push(world.hermes_id(&name).await);
            names.push(name);
        }
        let now = world.now();
        Self {
            world,
            names,
            ids,
            links: [[None; 3]; 3],
            now,
            seen: Vec::new(),
            out: [Vec::new(), Vec::new(), Vec::new()],
            leases: Vec::new(),
            trace: Vec::new(),
        }
    }

    fn index(&mut self, id: Uuid) -> usize {
        if let Some(at) = self.seen.iter().position(|seen| *seen == id) {
            at
        } else {
            self.seen.push(id);
            self.seen.len() - 1
        }
    }

    fn who(&self, id: Uuid) -> usize {
        self.ids.iter().position(|one| *one == id).unwrap()
    }

    async fn lease(&mut self, to: usize) {
        let due = repo::peer::lease(
            self.world.pool(),
            self.world.org,
            self.ids[to],
            self.now,
            10,
        )
        .await
        .unwrap();
        let mut line = format!("lease {to}:");
        // Two notices made in the same millisecond have no order of their own: they are told
        // by sender and count, so that the traces of the twins can be compared.
        let mut due = due;
        due.sort_by_key(|notice| {
            let from = match &notice.target {
                NoticeTarget::Peer { from } => self.names.iter().position(|n| n == from),
                NoticeTarget::Whatsapp { .. } => None,
            };
            (from, notice.count)
        });
        for notice in due {
            let NoticeTarget::Peer { from } = &notice.target else {
                panic!("a notice of WhatsApp in a round of notes");
            };
            let from = self.names.iter().position(|name| name == from).unwrap();
            // I1: the mark of the link, at this moment, in this direction.
            assert_eq!(
                self.links[from][to],
                Some(true),
                "a notice left without a live link with the mark ({from} -> {to})"
            );
            // I2: ten seconds between two different notices of the same pair.
            for (other, f, t, at) in &self.leases {
                if *f == from && *t == to && *other != notice.id {
                    assert!(
                        self.now - at >= GAP,
                        "two notices of {from} -> {to} left {} ms apart",
                        self.now - at
                    );
                }
            }
            self.leases.push((notice.id, from, to, self.now));
            let at = self.index(notice.id);
            if !self.out[to].contains(&notice.id) {
                self.out[to].push(notice.id);
            }
            line.push_str(&format!(" n{at}(from {from}, x{})", notice.count));
        }
        self.trace.push(line);
    }

    async fn step(&mut self, rng: &mut SeededRng, switch: bool) {
        let a = usize::try_from(rng.below(3)).unwrap();
        let b = (a + 1 + usize::try_from(rng.below(2)).unwrap()) % 3;
        match rng.below(10) {
            0 | 1 => {
                let wake = rng.below(2) == 0;
                let set = repo::peer::link_set(
                    self.world.pool(),
                    &self.world.home.mark,
                    self.world.org,
                    &self.world.admin,
                    &self.names[a],
                    &self.names[b],
                    wake,
                    self.now,
                )
                .await
                .unwrap();
                assert!(matches!(
                    set,
                    LinkSet::Created | LinkSet::Changed | LinkSet::Unchanged
                ));
                self.links[a][b] = Some(wake);
                self.trace.push(format!("link {a}->{b} wake={wake}"));
            }
            2 => {
                let removed = repo::peer::link_remove(
                    self.world.pool(),
                    &self.world.home.mark,
                    self.world.org,
                    &self.world.admin,
                    &self.names[a],
                    &self.names[b],
                    self.now,
                )
                .await
                .unwrap();
                assert!(matches!(
                    removed,
                    LinkRemoved::Removed | LinkRemoved::NoLink
                ));
                self.links[a][b] = None;
                self.trace.push(format!("unlink {a}->{b}"));
            }
            3 => {
                let on = rng.below(2) == 0;
                if switch {
                    repo::hermes::set_wake(
                        self.world.pool(),
                        &self.world.home.mark,
                        self.world.org,
                        &self.world.admin,
                        &self.names[a],
                        on,
                        self.now,
                    )
                    .await
                    .unwrap();
                }
            }
            4..=6 => {
                let text = MessageText::new(format!("note {}", rng.below(1_000))).unwrap();
                let sent = repo::peer::send(
                    self.world.pool(),
                    self.world.org,
                    &self.world.key,
                    NewMessage {
                        from: self.ids[a],
                        to: Some(&self.names[b]),
                        thread: None,
                        text: &text,
                    },
                    self.now,
                )
                .await
                .unwrap();
                let said = match sent {
                    Sent::Done { noticed, to, .. } => {
                        format!("done to={} noticed={noticed}", self.who(to))
                    }
                    Sent::NoPeer => "no-peer".to_owned(),
                    Sent::NoThread => "no-thread".to_owned(),
                    Sent::Ceiling { which, .. } => format!("ceiling {which:?}"),
                };
                if let Sent::Done { noticed, .. } = sent {
                    // A notice only for a link with the mark, and only through it.
                    assert!(
                        !noticed || self.links[a][b] == Some(true),
                        "a note noticed without the mark ({a} -> {b})"
                    );
                }
                self.trace.push(format!("send {a}->{b} {said}"));
            }
            7 => {
                self.lease(a).await;
                if rng.below(2) == 0 {
                    // A repeated lease.
                    self.lease(a).await;
                }
            }
            8 => {
                let outcome = usize::try_from(rng.below(7)).unwrap();
                if self.out[a].is_empty() {
                    self.trace.push(format!("ack {a} nothing"));
                    return;
                }
                let pick = usize::try_from(rng.below(self.out[a].len() as u64)).unwrap();
                let id = self.out[a][pick];
                let at = self.index(id);
                if outcome == 5 {
                    // Dropped: no answer.
                    self.trace.push(format!("ack {a} n{at} dropped"));
                    return;
                }
                let given = if outcome == 6 {
                    AckOutcome::Accepted
                } else {
                    OUTCOMES[outcome]
                };
                let times = if outcome == 6 { 2 } else { 1 };
                for _ in 0..times {
                    repo::peer::acknowledge(
                        self.world.pool(),
                        self.world.org,
                        self.ids[a],
                        id,
                        given,
                        self.now,
                    )
                    .await
                    .unwrap();
                }
                self.out[a].remove(pick);
                self.trace.push(format!("ack {a} n{at} {given:?} x{times}"));
            }
            _ => {
                let by = TIMES[usize::try_from(rng.below(TIMES.len() as u64)).unwrap()];
                self.now += by;
                self.trace.push(format!("time +{by}"));
            }
        }
    }

    /// I3, in the table: new notices in any sliding hour.
    async fn limits(&self) {
        let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
            "SELECT from_hermes_id, hermes_id, created_at FROM notice \
             WHERE organization_id = $1 AND kind = 'peer' ORDER BY created_at, id",
        )
        .bind(self.world.org.id())
        .fetch_all(self.world.pool())
        .await
        .unwrap();
        let mut per_link: HashMap<(Uuid, Uuid), Vec<i64>> = HashMap::new();
        let mut per_recipient: HashMap<Uuid, Vec<i64>> = HashMap::new();
        for (from, to, at) in rows {
            per_link.entry((from, to)).or_default().push(at);
            per_recipient.entry(to).or_default().push(at);
        }
        let window = |times: &[i64], most: usize, what: &str| {
            for (i, at) in times.iter().enumerate() {
                let inside = times[..=i].iter().filter(|t| **t > at - HOUR).count();
                assert!(inside <= most, "{what}: {inside} notices in an hour");
            }
        };
        for times in per_link.values() {
            window(times, 10, "a link");
        }
        for times in per_recipient.values() {
            window(times, 30, "a recipient");
        }
    }
}

async fn play(seed: u64, switch: bool) -> Vec<String> {
    let mut rng = SeededRng::new(seed);
    let mut run = Run::new().await;
    for _ in 0..STEPS {
        run.step(&mut rng, switch).await;
    }
    run.limits().await;
    run.trace
}

#[tokio::test(flavor = "multi_thread")]
async fn the_notices_of_a_round_follow_the_marks_and_never_the_switch() {
    let seed = number_from("ARAUTO_TEST_SEED").unwrap_or(DEFAULT_SEED);
    let rounds = number_from("ARAUTO_PEER_ROUNDS").unwrap_or(DEFAULT_ROUNDS);
    let mut woke = 0;
    for round in 0..rounds {
        let seed = seed.wrapping_add(round);
        let with_switch = tokio::spawn(play(seed, true)).await;
        let without = tokio::spawn(play(seed, false)).await;
        let (Ok(with_switch), Ok(without)) = (with_switch, without) else {
            panic!(
                "round {round} failed: replay with ARAUTO_TEST_SEED={seed} ARAUTO_PEER_ROUNDS=1"
            );
        };
        let at = with_switch
            .iter()
            .zip(&without)
            .position(|(one, other)| one != other);
        assert!(
            at.is_none() && with_switch.len() == without.len(),
            "the switch changed what a note did at step {at:?}: {:?} / {:?}: ARAUTO_TEST_SEED={seed} ARAUTO_PEER_ROUNDS=1",
            at.map(|i| &with_switch[i]),
            at.map(|i| &without[i])
        );
        woke += with_switch
            .iter()
            .filter(|line| line.starts_with("lease") && line.contains(" n"))
            .count();
    }
    assert!(woke > 0, "the rounds never woke anybody: they test nothing");
}
