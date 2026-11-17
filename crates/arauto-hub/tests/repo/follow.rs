//! The follow mark of a link (`docs/decisoes/0050-etapa-b-marca-de-seguir.md`): only the publish
//! of the org chart sets it, only on a link that goes from a higher level to a lower one and
//! wakes; the command takes it away and cannot set it; the notice reads it when it leaves.

use arauto_hub::admin::link::{LinkChange, Linked};
use arauto_hub::admin::render;
use arauto_hub::core::org::{Draft, OrgInvalid};
use arauto_hub::repo;
use arauto_hub::repo::hermes::{self, RemoveOutcome};
use arauto_hub::repo::org::PublishOutcome;
use arauto_hub::repo::peer::{self, LinkSet, NewMessage, NoticeTarget, Sent};
use arauto_protocol::names::{HermesName, MessageText};
use serde_json::{Value, json};

use crate::org::{chart, draft, keep, publish, snapshot};
use crate::support::World;

/// Two levels, `board` above `floor`: `ana` in the first, `bia` and `cleo` in the second. No
/// link yet.
async fn placed(world: &World) -> (HermesName, HermesName, HermesName) {
    let (ana, bia, cleo) = (
        world.hermes().await,
        world.hermes().await,
        world.hermes().await,
    );
    let before = chart(world).await;
    let wanted = draft(&json!({
        "levels": [
            { "key": "new-1", "name": "board" },
            { "key": "new-2", "name": "floor" },
        ],
        "hermes": [
            { "name": ana.as_str(), "level": "new-1", "x": 100, "role": "" },
            { "name": bia.as_str(), "level": "new-2", "x": 0, "role": "" },
            { "name": cleo.as_str(), "level": "new-2", "x": 300, "role": "" },
        ],
        "links": [],
    }));
    let outcome = publish(world, &wanted, before.version()).await;
    assert!(
        matches!(outcome, PublishOutcome::Published(_)),
        "{outcome:?}"
    );
    (ana, bia, cleo)
}

fn link(from: &HermesName, to: &HermesName, wake: bool, follow: bool) -> Value {
    json!({ "from": from.as_str(), "to": to.as_str(), "wake": wake, "follow": follow })
}

/// The chart as it is, with the links replaced.
async fn with_links(
    world: &World,
    links: Vec<Value>,
) -> (Draft, arauto_hub::core::org::ChartVersion) {
    let now = chart(world).await;
    let mut value = keep(&now);
    value["links"] = Value::Array(links);
    (draft(&value), now.version())
}

async fn publish_links(world: &World, links: Vec<Value>) -> PublishOutcome {
    let (wanted, base) = with_links(world, links).await;
    publish(world, &wanted, base).await
}

async fn marks(world: &World, from: &HermesName, to: &HermesName) -> (bool, bool) {
    sqlx::query_as(
        "SELECT l.wake, l.follow FROM peer_link l \
         JOIN hermes f ON f.id = l.from_hermes_id JOIN hermes t ON t.id = l.to_hermes_id \
         WHERE l.organization_id = $1 AND f.name = $2 AND t.name = $3",
    )
    .bind(world.org.id())
    .bind(from.as_str())
    .bind(to.as_str())
    .fetch_one(world.pool())
    .await
    .unwrap()
}

async fn command(world: &World, from: &HermesName, to: &HermesName, wake: bool) -> LinkSet {
    peer::link_set(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        from,
        to,
        wake,
        world.now(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn the_database_refuses_a_follow_mark_without_the_wake_mark() {
    let world = World::shared().await;
    let (ana, bia, cleo) = placed(&world).await;
    assert_eq!(command(&world, &ana, &bia, false).await, LinkSet::Created);
    assert_eq!(command(&world, &ana, &cleo, true).await, LinkSet::Created);
    let set_follow = |to: &HermesName| {
        sqlx::query(
            "UPDATE peer_link SET follow = TRUE \
             WHERE organization_id = $1 \
               AND from_hermes_id = (SELECT id FROM hermes WHERE organization_id = $1 AND name = $2) \
               AND to_hermes_id = (SELECT id FROM hermes WHERE organization_id = $1 AND name = $3)",
        )
        .bind(world.org.id())
        .bind(ana.as_str().to_owned())
        .bind(to.as_str().to_owned())
        .execute(world.pool())
    };
    // Without the wake mark the CHECK refuses, whatever wrote it.
    let refused = set_follow(&bia).await.unwrap_err();
    assert!(
        refused.to_string().contains("peer_link_follow_needs_wake"),
        "{refused}"
    );
    assert_eq!(marks(&world, &ana, &bia).await, (false, false));
    // With it the table takes it: the levels are for the publish to check.
    set_follow(&cleo).await.unwrap();
    assert_eq!(marks(&world, &ana, &cleo).await, (true, true));
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn a_publish_sets_the_follow_mark_only_on_a_top_down_link_that_wakes() {
    let world = World::shared().await;
    let (ana, bia, cleo) = placed(&world).await;
    let before = snapshot(&world).await;

    // Refused, and nothing written: without the wake mark, upward, and sideways.
    for (what, wanted) in [
        ("no wake", link(&ana, &bia, false, true)),
        ("upward", link(&bia, &ana, true, true)),
        ("sideways", link(&bia, &cleo, true, true)),
    ] {
        let outcome = publish_links(&world, vec![wanted]).await;
        assert_eq!(
            outcome,
            PublishOutcome::Invalid(OrgInvalid::Follow),
            "{what}"
        );
        assert_eq!(snapshot(&world).await, before, "{what}");
    }

    // A top-down link that wakes takes it.
    let base = chart(&world).await.version();
    let outcome = publish_links(&world, vec![link(&ana, &bia, true, true)]).await;
    let PublishOutcome::Published(done) = outcome else {
        panic!("not published: {outcome:?}");
    };
    assert_eq!((done.links_created, done.links_changed), (1, 0));
    assert_eq!(marks(&world, &ana, &bia).await, (true, true));
    let now = chart(&world).await;
    assert!(now.links[0].follow);
    let shown = render::org::show(&now).text;
    assert!(
        shown.contains(&format!(
            "may send and wake; {bia} follows its instructions"
        )),
        "{shown}"
    );
    assert_eq!(done.version, now.version());
    assert_ne!(done.version, base);

    // A change of the mark alone is a change, and a publish.
    let outcome = publish_links(
        &world,
        vec![link(&ana, &bia, true, true), link(&ana, &cleo, true, false)],
    )
    .await;
    assert!(matches!(outcome, PublishOutcome::Published(_)));
    let outcome = publish_links(
        &world,
        vec![link(&ana, &bia, true, true), link(&ana, &cleo, true, true)],
    )
    .await;
    let PublishOutcome::Published(done) = outcome else {
        panic!("a change of only the mark was not published: {outcome:?}");
    };
    assert_eq!((done.links_created, done.links_changed), (0, 1));
    assert_eq!(marks(&world, &ana, &cleo).await, (true, true));
    // The event says the mark before and after, and never a level.
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(a)::text FROM audit a \
         WHERE organization_id = $1 AND action = 'peer_link.changed' ORDER BY seq DESC LIMIT 1",
    )
    .bind(world.org.id())
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert!(events[0].contains("follow_before"), "{}", events[0]);
    assert!(!events[0].contains("board"));

    // The same draft again changes nothing.
    let seq = snapshot(&world).await;
    let outcome = publish_links(
        &world,
        vec![link(&ana, &bia, true, true), link(&ana, &cleo, true, true)],
    )
    .await;
    assert_eq!(outcome, PublishOutcome::Unchanged);
    assert_eq!(snapshot(&world).await, seq);
}

#[tokio::test]
async fn a_publish_that_leaves_a_marked_link_without_a_higher_sender_is_refused() {
    let world = World::shared().await;
    let (ana, bia, _cleo) = placed(&world).await;
    let outcome = publish_links(&world, vec![link(&ana, &bia, true, true)]).await;
    assert!(matches!(outcome, PublishOutcome::Published(_)));
    let before = snapshot(&world).await;
    let now = chart(&world).await;

    // A level moved: `floor` goes above `board`. The link is unchanged in the draft.
    let mut reordered = keep(&now);
    reordered["levels"].as_array_mut().unwrap().reverse();
    let outcome = publish(&world, &draft(&reordered), now.version()).await;
    assert_eq!(outcome, PublishOutcome::Invalid(OrgInvalid::Follow));
    assert_eq!(snapshot(&world).await, before, "a level moved");

    // A card moved: the receiver goes up to the level of the sender.
    let mut moved = keep(&now);
    for member in moved["hermes"].as_array_mut().unwrap() {
        if member["name"] == bia.as_str() {
            member["level"] = now.levels[0].id.to_string().into();
        }
    }
    let outcome = publish(&world, &draft(&moved), now.version()).await;
    assert_eq!(outcome, PublishOutcome::Invalid(OrgInvalid::Follow));
    assert_eq!(snapshot(&world).await, before, "a card moved");

    // The wake mark goes while the follow mark stays.
    let outcome = publish_links(&world, vec![link(&ana, &bia, false, true)]).await;
    assert_eq!(outcome, PublishOutcome::Invalid(OrgInvalid::Follow));
    assert_eq!(snapshot(&world).await, before, "wake taken away");

    // The same moves are published when the draft drops the mark, as the page does.
    let mut moved = keep(&now);
    for member in moved["hermes"].as_array_mut().unwrap() {
        if member["name"] == bia.as_str() {
            member["level"] = now.levels[0].id.to_string().into();
        }
    }
    moved["links"] = json!([link(&ana, &bia, true, false)]);
    let outcome = publish(&world, &draft(&moved), now.version()).await;
    assert!(
        matches!(outcome, PublishOutcome::Published(_)),
        "{outcome:?}"
    );
    assert_eq!(marks(&world, &ana, &bia).await, (true, false));
}

#[tokio::test]
async fn the_command_takes_the_follow_mark_away_and_cannot_set_it() {
    let world = World::shared().await;
    let (ana, bia, cleo) = placed(&world).await;
    let outcome = publish_links(
        &world,
        vec![link(&ana, &bia, true, true), link(&ana, &cleo, true, true)],
    )
    .await;
    assert!(matches!(outcome, PublishOutcome::Published(_)));

    // `admin link add` sets the whole link: the mark goes, and the answer says so.
    assert_eq!(
        command(&world, &ana, &bia, true).await,
        LinkSet::FollowRemoved
    );
    assert_eq!(marks(&world, &ana, &bia).await, (true, false));
    // Asking again is no change. The command has no way to put the mark back.
    assert_eq!(command(&world, &ana, &bia, true).await, LinkSet::Unchanged);
    assert_eq!(marks(&world, &ana, &bia).await, (true, false));
    // With the wake mark taken away too, it is the same.
    assert_eq!(
        command(&world, &ana, &cleo, false).await,
        LinkSet::FollowRemoved
    );
    assert_eq!(marks(&world, &ana, &cleo).await, (false, false));
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(a)::text FROM audit a \
         WHERE organization_id = $1 AND action = 'peer_link.changed' ORDER BY seq DESC LIMIT 2",
    )
    .bind(world.org.id())
    .fetch_all(world.pool())
    .await
    .unwrap();
    for event in events {
        assert!(event.contains("follow_before"), "{event}");
    }

    // The report says it.
    let said = render::link::linked(&Linked {
        changes: vec![LinkChange {
            from: ana.clone(),
            to: bia.clone(),
            wake: true,
            outcome: LinkSet::FollowRemoved,
        }],
        reach: vec![],
    })
    .text;
    assert!(
        said.contains(&format!(
            "The follow mark of {ana} -> {bia} was removed: it is set only on the org chart."
        )),
        "{said}"
    );
}

#[tokio::test]
async fn a_publish_without_the_mark_in_the_file_takes_it_away() {
    let world = World::shared().await;
    let (ana, bia, _cleo) = placed(&world).await;
    let outcome = publish_links(&world, vec![link(&ana, &bia, true, true)]).await;
    assert!(matches!(outcome, PublishOutcome::Published(_)));

    // A file, or an old script of the panel still open in a tab, that sends no "follow" in its
    // links reads as off: the publish takes the mark away (fail-safe), and the audit records
    // `follow_before`. The page never adds a mark and a missing field never keeps one.
    let now = chart(&world).await;
    let mut value = keep(&now);
    value["links"] = json!([{ "from": ana.as_str(), "to": bia.as_str(), "wake": true }]);
    let wanted = draft(&value);
    let dropped = vec![(ana.clone(), bia.clone())];
    let outcome = publish(&world, &wanted, now.version()).await;
    let PublishOutcome::Published(done) = outcome else {
        panic!("not published: {outcome:?}");
    };
    assert_eq!(done.links_changed, 1);
    assert_eq!(marks(&world, &ana, &bia).await, (true, false));
    let event: String = sqlx::query_scalar(
        "SELECT row_to_json(a)::text FROM audit a          WHERE organization_id = $1 AND action = 'peer_link.changed' ORDER BY seq DESC LIMIT 1",
    )
    .bind(world.org.id())
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert!(event.contains("follow_before"), "{event}");
    assert!(event.contains("true"), "{event}");
    let said = render::org::published(&PublishOutcome::Published(done), &dropped).text;
    assert!(
        said.contains(&format!(
            "The follow mark of {ana} -> {bia} was removed: the file does not set it."
        )),
        "{said}"
    );
}

#[tokio::test]
async fn removing_a_hermes_removes_its_followed_links_in_both_directions() {
    let world = World::shared().await;
    let (ana, bia, cleo) = placed(&world).await;
    let outcome = publish_links(
        &world,
        vec![link(&ana, &bia, true, true), link(&ana, &cleo, true, true)],
    )
    .await;
    assert!(matches!(outcome, PublishOutcome::Published(_)));
    let followed = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM peer_link WHERE organization_id = $1 AND follow",
        )
        .bind(world.org.id())
        .fetch_one(world.pool())
        .await
        .unwrap()
    };
    assert_eq!(followed().await, 2);
    // The receiver of one of them.
    let gone = hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &bia,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(gone, RemoveOutcome::Removed(_)));
    assert_eq!(followed().await, 1);
    // The sender of the other.
    let gone = hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &ana,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(gone, RemoveOutcome::Removed(_)));
    assert_eq!(followed().await, 0);
}

#[tokio::test]
async fn a_notice_leaves_with_the_mark_the_link_has_when_it_leaves() {
    let world = World::shared().await;
    let (ana, bia, cleo) = placed(&world).await;
    let outcome = publish_links(
        &world,
        vec![link(&ana, &bia, true, true), link(&ana, &cleo, true, true)],
    )
    .await;
    assert!(matches!(outcome, PublishOutcome::Published(_)));
    let ana_id = world.hermes_id(&ana).await;
    let (bia_id, cleo_id) = (world.hermes_id(&bia).await, world.hermes_id(&cleo).await);
    world.active(&bia).await;
    world.active(&cleo).await;
    let now = world.now();
    let text = MessageText::new("a note".to_owned()).unwrap();
    for to in [&bia, &cleo] {
        let sent = peer::send(
            world.pool(),
            world.org,
            &world.key,
            NewMessage {
                from: ana_id,
                to: Some(to),
                thread: None,
                text: &text,
            },
            now,
        )
        .await
        .unwrap();
        assert!(matches!(sent, Sent::Done { noticed: true, .. }));
    }
    // The mark of the second link goes while its notice waits; the wake mark stays.
    assert_eq!(
        command(&world, &ana, &cleo, true).await,
        LinkSet::FollowRemoved
    );
    let lease = |to| repo::peer::lease(world.pool(), world.org, to, now, 10);
    let due = lease(bia_id).await.unwrap();
    assert_eq!(due.len(), 1);
    assert!(matches!(
        &due[0].target,
        NoticeTarget::Peer { follow: true, .. }
    ));
    let due = lease(cleo_id).await.unwrap();
    assert_eq!(due.len(), 1, "it still leaves: the wake mark is there");
    assert!(matches!(
        &due[0].target,
        NoticeTarget::Peer { follow: false, .. }
    ));
}
