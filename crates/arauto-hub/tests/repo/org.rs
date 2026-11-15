//! The org chart (`docs/decisoes/0043-organograma-e-niveis.md`): Publish writes the levels, the
//! places, the roles and the links in one write, with their events; a draft that started from
//! another version changes nothing; one organization never reaches the chart of another.

use arauto_hub::core::org::{Chart, ChartVersion, Draft, OrgInvalid};
use arauto_hub::repo::hermes::{self, RemoveOutcome};
use arauto_hub::repo::org::{self, PublishOutcome};
use arauto_hub::repo::peer::{self, LinkRemoved, LinkSet};
use arauto_protocol::names::HermesName;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::support::World;

pub fn draft(value: &Value) -> Draft {
    Draft::parse(&value.to_string()).unwrap()
}

/// The draft that keeps the chart as it is: its levels (a first one when it has none), its
/// Hermes (the ones with no place in the first level) and its links.
pub fn keep(chart: &Chart) -> Value {
    let levels: Vec<Value> = if chart.levels.is_empty() {
        vec![json!({ "key": "new-1", "name": "board" })]
    } else {
        chart
            .levels
            .iter()
            .map(|l| json!({ "key": l.id.to_string(), "name": l.name.as_str() }))
            .collect()
    };
    let first = levels[0]["key"].clone();
    let hermes: Vec<Value> = chart
        .members
        .iter()
        .enumerate()
        .map(|(at, member)| match &member.place {
            Some(place) => json!({
                "name": member.name.as_str(),
                "level": place.level.to_string(),
                "x": place.x.get(),
                "role": place.role.as_ref().map_or("", |r| r.as_str()),
            }),
            None => json!({
                "name": member.name.as_str(),
                "level": first,
                "x": at * 200,
                "role": "",
            }),
        })
        .collect();
    let links: Vec<Value> = chart
        .links
        .iter()
        .map(|l| json!({ "from": l.from.as_str(), "to": l.to.as_str(), "wake": l.wake }))
        .collect();
    json!({ "levels": levels, "hermes": hermes, "links": links })
}

pub async fn publish(world: &World, wanted: &Draft, base: ChartVersion) -> PublishOutcome {
    org::publish(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        wanted,
        base,
        world.now(),
    )
    .await
    .unwrap()
}

pub async fn chart(world: &World) -> Chart {
    org::chart(world.pool(), world.org).await.unwrap()
}

/// Every row of the chart, of the links, of the notices and of the audit of one organization.
pub async fn snapshot(world: &World) -> Vec<String> {
    let mut all = Vec::new();
    for (table, key) in [
        ("org_level", "organization_id"),
        ("org_member", "organization_id"),
        ("peer_link", "organization_id"),
        ("notice", "organization_id"),
        ("audit", "organization_id"),
        ("hermes", "organization_id"),
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t WHERE {key} = $1 ORDER BY 1"
        )))
        .bind(world.org.id())
        .fetch_all(world.pool())
        .await
        .unwrap();
        all.push(format!("{table}: {}", rows.join("\n")));
    }
    all
}

async fn actions(world: &World, after: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT action FROM audit WHERE organization_id = $1 AND seq > $2 ORDER BY seq",
    )
    .bind(world.org.id())
    .bind(after)
    .fetch_all(world.pool())
    .await
    .unwrap()
}

async fn last_seq(world: &World) -> i64 {
    sqlx::query_scalar("SELECT coalesce(max(seq), 0) FROM audit WHERE organization_id = $1")
        .bind(world.org.id())
        .fetch_one(world.pool())
        .await
        .unwrap()
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn a_publish_writes_the_whole_chart_in_one_write_with_its_events() {
    let world = World::shared().await;
    let (ana, bia, cleo) = (
        world.hermes().await,
        world.hermes().await,
        world.hermes().await,
    );
    let before = chart(&world).await;
    // Nothing is placed yet and there is no level: every live Hermes is "not placed yet".
    assert!(before.levels.is_empty());
    assert_eq!(before.members.len(), 3);
    assert!(before.members.iter().all(|member| member.place.is_none()));
    let version = before.version();
    assert_eq!(chart(&world).await.version(), version, "a read is stable");

    let after_seq = last_seq(&world).await;
    let wanted = draft(&json!({
        "levels": [
            { "key": "new-1", "name": "board" },
            { "key": "new-2", "name": "floor" },
        ],
        "hermes": [
            { "name": ana.as_str(), "level": "new-1", "x": 300, "role": "  leads the rest " },
            { "name": bia.as_str(), "level": "new-2", "x": 0, "role": "" },
            { "name": cleo.as_str(), "level": "new-2", "x": 200, "role": "answers" },
        ],
        "links": [
            { "from": ana.as_str(), "to": bia.as_str(), "wake": true },
            { "from": bia.as_str(), "to": cleo.as_str(), "wake": false },
        ],
    }));
    let outcome = publish(&world, &wanted, version).await;
    let PublishOutcome::Published(done) = outcome else {
        panic!("not published: {outcome:?}");
    };
    assert_eq!(
        (
            done.levels_added,
            done.levels_removed,
            done.levels_renamed,
            done.levels_moved,
            done.members_changed,
            done.roles_changed,
            done.links_created,
            done.links_changed,
            done.links_removed
        ),
        (2, 0, 0, 0, 3, 2, 2, 0, 0)
    );

    let now = chart(&world).await;
    assert_eq!(done.version, now.version());
    assert_ne!(done.version, version);
    let names: Vec<&str> = now.levels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["board", "floor"]);
    let place = |name: &HermesName| {
        now.members
            .iter()
            .find(|member| member.name == *name)
            .and_then(|member| member.place.clone())
            .unwrap()
    };
    assert_eq!(place(&ana).level, now.levels[0].id);
    assert_eq!(place(&ana).x.get(), 300);
    assert_eq!(place(&ana).role.unwrap().as_str(), "leads the rest");
    assert_eq!(place(&bia).level, now.levels[1].id);
    assert!(place(&bia).role.is_none());
    assert_eq!(now.links.len(), 2);

    // One write: the events of the chart (ordinary) and of the links (security), together.
    let written = actions(&world, after_seq).await;
    let count = |action: &str| written.iter().filter(|a| *a == action).count();
    assert_eq!(count("org_level.changed"), 2);
    assert_eq!(count("org_member.changed"), 3);
    assert_eq!(count("peer_link.created"), 2);
    assert_eq!(count("org_chart.published"), 1);
    assert_eq!(written.len(), 8);
    let security: Vec<(String, bool)> = sqlx::query_as(
        "SELECT action, security FROM audit WHERE organization_id = $1 AND seq > $2",
    )
    .bind(world.org.id())
    .bind(after_seq)
    .fetch_all(world.pool())
    .await
    .unwrap();
    for (action, security) in security {
        assert_eq!(security, action.starts_with("peer_link."), "{action}");
    }
    // The audit holds no level name and no role text.
    let audit = snapshot(&world).await.join("\n");
    for text in ["board", "leads the rest", "answers"] {
        let events: Vec<String> = sqlx::query_scalar(
            "SELECT row_to_json(a)::text FROM audit a \
             WHERE organization_id = $1 AND action LIKE 'org_%'",
        )
        .bind(world.org.id())
        .fetch_all(world.pool())
        .await
        .unwrap();
        assert!(!events.join("\n").contains(text), "{text} is in the audit");
    }
    assert!(audit.contains("org_chart.published"));

    // The same draft again is no change, and writes nothing.
    let seq = last_seq(&world).await;
    let again = publish(&world, &draft(&keep(&now)), now.version()).await;
    assert_eq!(again, PublishOutcome::Unchanged);
    assert_eq!(last_seq(&world).await, seq);

    // A second publish: a rename, a move to the other level, a role, a link taken and one
    // changed, a level taken and a level added.
    let wanted = draft(&json!({
        "levels": [
            { "key": now.levels[1].id.to_string(), "name": "ground" },
            { "key": now.levels[0].id.to_string(), "name": "board" },
            { "key": "new-3", "name": "attic" },
        ],
        "hermes": [
            { "name": ana.as_str(), "level": now.levels[0].id.to_string(), "x": 40, "role": "" },
            { "name": bia.as_str(), "level": now.levels[1].id.to_string(), "x": 0, "role": "" },
            { "name": cleo.as_str(), "level": "new-3", "x": 200, "role": "answers" },
        ],
        "links": [ { "from": ana.as_str(), "to": bia.as_str(), "wake": false } ],
    }));
    let seq = last_seq(&world).await;
    let outcome = publish(&world, &wanted, now.version()).await;
    let PublishOutcome::Published(done) = outcome else {
        panic!("not published: {outcome:?}");
    };
    assert_eq!(
        (
            done.levels_added,
            done.levels_renamed,
            done.levels_moved,
            done.levels_removed
        ),
        (1, 1, 2, 0)
    );
    assert_eq!(
        (done.links_changed, done.links_removed, done.links_created),
        (1, 1, 0)
    );
    assert_eq!(done.roles_changed, 1);
    let now = chart(&world).await;
    let names: Vec<&str> = now.levels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["ground", "board", "attic"]);
    assert_eq!(now.links.len(), 1);
    assert!(!now.links[0].wake);
    let written = actions(&world, seq).await;
    assert!(written.contains(&"peer_link.changed".to_owned()));
    assert!(written.contains(&"peer_link.removed".to_owned()));
}

#[tokio::test]
async fn a_stale_draft_changes_nothing() {
    for source in [
        "another publish",
        "link added",
        "link removed",
        "wake mark changed",
        "hermes added",
        "hermes removed",
    ] {
        let world = World::shared().await;
        let (ana, bia, cleo) = (
            world.hermes().await,
            world.hermes().await,
            world.hermes().await,
        );
        // A chart that exists and a link that can be changed or taken.
        let chart0 = chart(&world).await;
        let mut first = keep(&chart0);
        first["links"] = json!([ { "from": ana.as_str(), "to": bia.as_str(), "wake": false } ]);
        assert!(matches!(
            publish(&world, &draft(&first), chart0.version()).await,
            PublishOutcome::Published(_)
        ));
        let chart1 = chart(&world).await;
        let mine = draft(&{
            let mut value = keep(&chart1);
            value["hermes"][0]["role"] = "my edit".into();
            value
        });

        // Something else changes the chart between the read and the publish.
        match source {
            "another publish" => {
                let mut other = keep(&chart1);
                other["hermes"][1]["x"] = 700.into();
                assert!(matches!(
                    publish(&world, &draft(&other), chart1.version()).await,
                    PublishOutcome::Published(_)
                ));
            }
            "link added" => {
                let set = peer::link_set(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    &world.admin,
                    &bia,
                    &cleo,
                    false,
                    world.now(),
                )
                .await
                .unwrap();
                assert_eq!(set, LinkSet::Created);
            }
            "link removed" => {
                let gone = peer::link_remove(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    &world.admin,
                    &ana,
                    &bia,
                    world.now(),
                )
                .await
                .unwrap();
                assert_eq!(gone, LinkRemoved::Removed);
            }
            "wake mark changed" => {
                let set = peer::link_set(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    &world.admin,
                    &ana,
                    &bia,
                    true,
                    world.now(),
                )
                .await
                .unwrap();
                assert_eq!(set, LinkSet::Changed);
            }
            "hermes added" => {
                world.hermes().await;
            }
            _ => {
                let gone = hermes::remove(
                    world.pool(),
                    &world.home.mark,
                    world.org,
                    &world.admin,
                    &cleo,
                    world.now(),
                )
                .await
                .unwrap();
                assert!(matches!(gone, RemoveOutcome::Removed(_)));
            }
        }
        let before = snapshot(&world).await;
        let outcome = publish(&world, &mine, chart1.version()).await;
        assert_eq!(outcome, PublishOutcome::Stale, "{source}");
        assert_eq!(snapshot(&world).await, before, "{source}");
    }
}

#[tokio::test]
async fn a_draft_that_breaks_a_rule_changes_nothing() {
    let world = World::shared().await;
    let (ana, bia) = (world.hermes().await, world.hermes().await);
    let chart0 = chart(&world).await;
    assert!(matches!(
        publish(&world, &draft(&keep(&chart0)), chart0.version()).await,
        PublishOutcome::Published(_)
    ));
    let chart1 = chart(&world).await;
    let before = snapshot(&world).await;

    // A stored key of no level.
    let mut value = keep(&chart1);
    let strange = Uuid::new_v4().to_string();
    value["levels"][0]["key"] = strange.clone().into();
    value["hermes"][0]["level"] = strange.clone().into();
    value["hermes"][1]["level"] = strange.into();
    assert_eq!(
        publish(&world, &draft(&value), chart1.version()).await,
        PublishOutcome::Invalid(OrgInvalid::Unknown)
    );

    // A live Hermes left out.
    let mut value = keep(&chart1);
    value["hermes"].as_array_mut().unwrap().pop();
    assert_eq!(
        publish(&world, &draft(&value), chart1.version()).await,
        PublishOutcome::Invalid(OrgInvalid::Unknown)
    );

    // A Hermes that was removed, named.
    let gone = world.hermes().await;
    let chart2 = chart(&world).await;
    assert!(matches!(
        publish(&world, &draft(&keep(&chart2)), chart2.version()).await,
        PublishOutcome::Published(_)
    ));
    let removed = hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &gone,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(removed, RemoveOutcome::Removed(_)));
    let chart3 = chart(&world).await;
    let mut value = keep(&chart3);
    value["hermes"].as_array_mut().unwrap().push(json!({
        "name": gone.as_str(), "level": chart3.levels[0].id.to_string(), "x": 0, "role": ""
    }));
    let snapshot_before = snapshot(&world).await;
    assert_eq!(
        publish(&world, &draft(&value), chart3.version()).await,
        PublishOutcome::Invalid(OrgInvalid::Unknown)
    );
    assert_eq!(snapshot(&world).await, snapshot_before);
    let _ = (before, ana, bia);
}

#[tokio::test]
async fn one_organization_never_reaches_the_chart_of_another() {
    let ours = World::shared().await;
    let theirs = World::shared().await;
    let (ana, bia) = (ours.hermes().await, theirs.hermes().await);
    for world in [&ours, &theirs] {
        let chart0 = chart(world).await;
        assert!(matches!(
            publish(world, &draft(&keep(&chart0)), chart0.version()).await,
            PublishOutcome::Published(_)
        ));
    }
    let (ours_chart, theirs_chart) = (chart(&ours).await, chart(&theirs).await);
    assert_eq!(ours_chart.members.len(), 1);
    assert_eq!(theirs_chart.members.len(), 1);
    assert_ne!(ours_chart.levels[0].id, theirs_chart.levels[0].id);

    let theirs_before = snapshot(&theirs).await;
    // Our draft with their level.
    let mut value = keep(&ours_chart);
    value["levels"][0]["key"] = theirs_chart.levels[0].id.to_string().into();
    value["hermes"][0]["level"] = theirs_chart.levels[0].id.to_string().into();
    assert_eq!(
        publish(&ours, &draft(&value), ours_chart.version()).await,
        PublishOutcome::Invalid(OrgInvalid::Unknown)
    );
    // Our draft with their Hermes.
    let mut value = keep(&ours_chart);
    value["hermes"][0]["name"] = bia.as_str().into();
    assert_eq!(
        publish(&ours, &draft(&value), ours_chart.version()).await,
        PublishOutcome::Invalid(OrgInvalid::Unknown)
    );
    assert_eq!(snapshot(&theirs).await, theirs_before);
    assert!(ours_chart.members.iter().all(|m| m.name == ana));
    assert!(theirs_chart.members.iter().all(|m| m.name == bia));
    // The command that takes a Hermes of another organization finds none.
    let gone = peer::link_set(
        ours.pool(),
        &ours.home.mark,
        ours.org,
        &ours.admin,
        &ana,
        &bia,
        false,
        ours.now(),
    )
    .await
    .unwrap();
    assert!(matches!(gone, LinkSet::Missing(_)));
}

#[tokio::test]
async fn a_failure_in_the_middle_leaves_nothing() {
    let world = World::shared().await;
    let (ana, bia, cleo) = (
        world.hermes().await,
        world.hermes().await,
        world.hermes().await,
    );
    let chart0 = chart(&world).await;
    let mut first = keep(&chart0);
    first["links"] = json!([ { "from": ana.as_str(), "to": bia.as_str(), "wake": true } ]);
    assert!(matches!(
        publish(&world, &draft(&first), chart0.version()).await,
        PublishOutcome::Published(_)
    ));
    let chart1 = chart(&world).await;

    // A trigger that refuses the next link of this organization, only.
    let suffix = world.org.id().simple().to_string();
    let function = format!("org_boom_{suffix}");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.organization_id = '{}' THEN RAISE EXCEPTION 'boom'; END IF; RETURN NEW; END $$",
        world.org.id()
    )))
    .execute(world.pool())
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TRIGGER {function} BEFORE INSERT ON peer_link \
         FOR EACH ROW EXECUTE FUNCTION {function}()"
    )))
    .execute(world.pool())
    .await
    .unwrap();

    // A rename, a new level, a move, a role, a link taken, and the link that blows up.
    let wanted = draft(&json!({
        "levels": [
            { "key": chart1.levels[0].id.to_string(), "name": "renamed" },
            { "key": "new-1", "name": "extra" },
        ],
        "hermes": [
            { "name": ana.as_str(), "level": "new-1", "x": 10, "role": "new role" },
            { "name": bia.as_str(), "level": chart1.levels[0].id.to_string(), "x": 20, "role": "" },
            { "name": cleo.as_str(), "level": chart1.levels[0].id.to_string(), "x": 30, "role": "" },
        ],
        "links": [ { "from": bia.as_str(), "to": cleo.as_str(), "wake": false } ],
    }));
    let before = snapshot(&world).await;
    let mark = world.home.mark.read().await.unwrap();
    let failed = org::publish(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &wanted,
        chart1.version(),
        world.now(),
    )
    .await;
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(snapshot(&world).await, before);
    assert_eq!(world.home.mark.read().await.unwrap(), mark);
    assert_eq!(chart(&world).await, chart1);

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON peer_link"
    )))
    .execute(world.pool())
    .await
    .unwrap();
    // Without the trigger the same draft goes through: the failure was the trigger, nothing else.
    assert!(matches!(
        publish(&world, &wanted, chart1.version()).await,
        PublishOutcome::Published(_)
    ));
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP FUNCTION {function}()")))
        .execute(world.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn removing_a_hermes_takes_its_place_and_role_with_it() {
    let world = World::shared().await;
    let (ana, bia) = (world.hermes().await, world.hermes().await);
    let chart0 = chart(&world).await;
    let mut value = keep(&chart0);
    value["hermes"][0]["role"] = "will go".into();
    value["hermes"][1]["role"] = "will stay".into();
    assert!(matches!(
        publish(&world, &draft(&value), chart0.version()).await,
        PublishOutcome::Published(_)
    ));
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM org_member WHERE organization_id = $1")
            .bind(world.org.id())
            .fetch_one(world.pool())
            .await
            .unwrap();
    assert_eq!(rows, 2);
    let removed = hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &ana,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(removed, RemoveOutcome::Removed(_)));
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT hermes_id FROM org_member WHERE organization_id = $1")
            .bind(world.org.id())
            .fetch_all(world.pool())
            .await
            .unwrap();
    assert_eq!(rows, [(world.hermes_id(&bia).await,)]);
    let now = chart(&world).await;
    assert_eq!(now.levels.len(), 1, "the level stays");
    assert_eq!(now.members.len(), 1);
    assert_eq!(now.members[0].name, bia);
}

#[tokio::test]
async fn the_events_of_a_publish_for_a_link_are_the_ones_the_command_writes() {
    // The same changes, once by Publish and once by the command, in two organizations.
    async fn links_events(world: &World) -> Vec<(String, String, String)> {
        sqlx::query_as(
            "SELECT action, target_kind, ((details::jsonb) - 'from_id' - 'to_id' - 'from' - 'to')::text \
             FROM audit WHERE organization_id = $1 AND action LIKE 'peer_link.%' ORDER BY seq",
        )
        .bind(world.org.id())
        .fetch_all(world.pool())
        .await
        .unwrap()
    }
    let (by_publish, by_command) = (World::shared().await, World::shared().await);
    let mut names = Vec::new();
    for world in [&by_publish, &by_command] {
        names.push((world.hermes().await, world.hermes().await));
    }
    // Publish: create with wake, then change the wake, then take it away.
    let (a, b) = &names[0];
    for wake in [Some(true), Some(false), None] {
        let chart0 = chart(&by_publish).await;
        let mut value = keep(&chart0);
        value["links"] = match wake {
            Some(wake) => json!([ { "from": a.as_str(), "to": b.as_str(), "wake": wake } ]),
            None => json!([]),
        };
        assert!(matches!(
            publish(&by_publish, &draft(&value), chart0.version()).await,
            PublishOutcome::Published(_)
        ));
    }
    // The command.
    let (a, b) = &names[1];
    let world = &by_command;
    let args = (world.pool(), &world.home.mark, world.org, &world.admin);
    peer::link_set(args.0, args.1, args.2, args.3, a, b, true, world.now())
        .await
        .unwrap();
    peer::link_set(args.0, args.1, args.2, args.3, a, b, false, world.now())
        .await
        .unwrap();
    peer::link_remove(args.0, args.1, args.2, args.3, a, b, world.now())
        .await
        .unwrap();
    assert_eq!(
        links_events(&by_publish).await,
        links_events(&by_command).await
    );
}
