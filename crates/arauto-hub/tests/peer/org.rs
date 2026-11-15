//! The org chart gives the channel nothing in stage A (`docs/decisoes/0043-organograma-e-niveis.md`):
//! a level or the kind of an arrow decides no permission, and what a Hermes sees through its
//! tools and its notices is the same before and after a publish of levels, places and roles.
//! The links of a publish follow the rules of the command.

use arauto_hub::admin::Session;
use arauto_hub::core::org::{Draft, Kind, kind};
use arauto_hub::repo::org::PublishOutcome;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::notice::AckOutcome;
use serde_json::{Value, json};

use crate::world::{Agent, World, ok, peer_notice, refused};

/// Publishes the chart with these levels (by name, top first), these Hermes in them and, when
/// `links` is given, these links; otherwise the links stay as they are.
async fn arrange(
    world: &World,
    levels: &[&str],
    place: &[(&Agent, usize, &str)],
    links: Option<&[(&Agent, &Agent, bool)]>,
) -> PublishOutcome {
    let admin = &world.fixture.admin;
    let chart = admin.org_chart().await.unwrap();
    let wanted_levels: Vec<Value> = levels
        .iter()
        .enumerate()
        .map(|(at, name)| match chart.levels.get(at) {
            Some(stored) => json!({ "key": stored.id.to_string(), "name": name }),
            None => json!({ "key": format!("new-{}", at + 1), "name": name }),
        })
        .collect();
    let hermes: Vec<Value> = place
        .iter()
        .enumerate()
        .map(|(at, (agent, level, role))| {
            json!({
                "name": agent.name,
                "level": wanted_levels[*level]["key"],
                "x": at * 200,
                "role": role,
            })
        })
        .collect();
    let links: Vec<Value> = match links {
        Some(links) => links
            .iter()
            .map(|(from, to, wake)| json!({ "from": from.name, "to": to.name, "wake": wake }))
            .collect(),
        None => chart
            .links
            .iter()
            .map(|l| json!({ "from": l.from.as_str(), "to": l.to.as_str(), "wake": l.wake }))
            .collect(),
    };
    let draft = Draft::parse(
        &json!({ "levels": wanted_levels, "hermes": hermes, "links": links }).to_string(),
    )
    .unwrap();
    admin
        .org_publish(&draft, chart.version(), world.now())
        .await
        .unwrap()
}

#[tokio::test]
async fn levels_and_kinds_give_no_permission() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;

    // A on top, B below, no link: no tool, and sending is refused as it is without a level (AR-701, nothing to send with).
    let published = arrange(
        &world,
        &["top", "bottom"],
        &[(&a, 0, "leads"), (&b, 1, "serves")],
        None,
    )
    .await;
    assert!(matches!(published, PublishOutcome::Published(_)));
    let to_b = a.send_to(&b.name, "hello").await;
    let to_nobody = a.send_to("nobody-here", "hello").await;
    assert_eq!(refused(&to_b), Some(ErrorCode::Ar701));
    assert_eq!(
        to_b, to_nobody,
        "a higher level reaches nobody by being higher"
    );
    let from_b = b.send_to(&a.name, "hello").await;
    assert_eq!(refused(&from_b), Some(ErrorCode::Ar701));

    // Only A -> B is a link: an "instruction" in the eyes of the admin, a permission for A only.
    let chart = world.fixture.admin.org_chart().await.unwrap();
    let published = arrange(
        &world,
        &["top", "bottom"],
        &[(&a, 0, "leads"), (&b, 1, "serves")],
        Some(&[(&a, &b, false)]),
    )
    .await;
    assert!(matches!(published, PublishOutcome::Published(_)));
    let after = world.fixture.admin.org_chart().await.unwrap();
    assert_eq!(
        kind(
            after.position_of_member(&a.name.parse().unwrap()),
            after.position_of_member(&b.name.parse().unwrap())
        ),
        Some(Kind::Instruction)
    );
    assert!(chart.links.is_empty());
    a.next_tools().await;
    b.next_tools().await;
    ok(a.send_to(&b.name, "hello").await);
    assert_eq!(
        refused(&b.send_to(&a.name, "and back").await),
        Some(ErrorCode::Ar701),
        "the way back of an instruction is no request: there is no link"
    );

    // B moved above A, no link touched: the same answers, and no new list of tools.
    let published = arrange(
        &world,
        &["top", "bottom"],
        &[(&a, 1, "serves"), (&b, 0, "leads")],
        None,
    )
    .await;
    assert!(matches!(published, PublishOutcome::Published(_)));
    let after = world.fixture.admin.org_chart().await.unwrap();
    assert_eq!(
        kind(
            after.position_of_member(&a.name.parse().unwrap()),
            after.position_of_member(&b.name.parse().unwrap())
        ),
        Some(Kind::Request)
    );
    ok(a.send_to(&b.name, "still allowed").await);
    assert_eq!(
        refused(&b.send_to(&a.name, "still not").await),
        Some(ErrorCode::Ar701)
    );
}

#[tokio::test]
async fn the_channel_says_the_same_after_a_publish_of_levels_and_roles() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.may_be_woken(&b, true).await;
    world.link(&a, &b, true).await;
    a.next_tools().await;
    b.next_tools().await;
    let thread = a.thread_with(&b.name, "m1").await;
    let first = b.next_notice().await;
    let before_notice = peer_notice(&first);
    b.acknowledge(&first, AckOutcome::Accepted).await;

    async fn seen(agent: &mut Agent) -> Vec<Value> {
        let mut all = Vec::new();
        for tool in ["hermes_list_peers", "hermes_list_threads", "arauto_status"] {
            all.push(Value::Object(ok(agent.call(tool, json!({})).await)));
        }
        all
    }
    let (a0, b0) = (seen(&mut a).await, seen(&mut b).await);
    let (tools_a, tools_b) = (a.tools.len(), b.tools.len());

    // Levels, places and roles change; no link does.
    let published = arrange(
        &world,
        &["board", "floor", "attic"],
        &[(&a, 2, "one role"), (&b, 0, "another")],
        None,
    )
    .await;
    assert!(matches!(published, PublishOutcome::Published(_)));
    let published = arrange(
        &world,
        &["board", "floor", "attic"],
        &[(&a, 1, ""), (&b, 1, "third")],
        None,
    )
    .await;
    assert!(matches!(published, PublishOutcome::Published(_)));

    assert_eq!(seen(&mut a).await, a0);
    assert_eq!(seen(&mut b).await, b0);
    assert_eq!(a.notices_while_quiet().await, 0);
    assert_eq!(b.notices_while_quiet().await, 0);
    assert_eq!(
        (a.tools.len(), b.tools.len()),
        (tools_a, tools_b),
        "no new list of tools"
    );

    // A notice after the publish is the same pointer as before it.
    world.advance(30_000);
    ok(a.send_in(&thread, "m2").await);
    let second = b.next_notice().await;
    assert_eq!(peer_notice(&second), before_notice);
    let travelled = serde_json::to_string(&second).unwrap();
    for text in ["board", "floor", "attic", "one role", "another", "third"] {
        assert!(!travelled.contains(text), "{text}");
    }
}

#[tokio::test]
async fn the_links_of_a_publish_follow_the_rules_of_the_command() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.may_be_woken(&b, true).await;
    world.link(&a, &b, true).await;
    a.next_tools().await;
    b.next_tools().await;
    let thread = a.thread_with(&b.name, "unread").await;
    let notice = b.next_notice().await;
    assert_eq!(world.notice_states(&a, &b).await, [("sent".to_owned(), 1)]);
    let _ = notice;

    // The wake mark goes off by a publish: the notice that was out is discarded at once.
    arrange(
        &world,
        &["top"],
        &[(&a, 0, ""), (&b, 0, "")],
        Some(&[(&a, &b, false)]),
    )
    .await;
    assert_eq!(
        world.notice_states(&a, &b).await,
        [("discarded".to_owned(), 1)]
    );

    // The link goes by a publish: what A had sent and B had not read is no longer delivered, and the
    // tools follow, as with the command.
    arrange(&world, &["top"], &[(&a, 0, ""), (&b, 0, "")], Some(&[])).await;
    assert!(
        refused(
            &b.call("hermes_read_thread", json!({ "thread": thread }))
                .await
        )
        .is_some()
            || b.read(&thread).await.is_empty()
    );
    let tools = b.next_tools().await;
    assert!(!tools.iter().any(|tool| tool == "hermes_list_peers"));
    assert!(
        !a.next_tools()
            .await
            .iter()
            .any(|tool| tool == "hermes_send_message")
    );
}

#[tokio::test]
async fn the_command_shows_and_publishes_the_same_chart() {
    let world = World::new().await;
    let a = world.agent().await;
    let b = world.agent().await;
    let admin = &world.fixture.admin;
    let chart = admin.org_chart().await.unwrap();
    // Through the session of the panel, the same chart and the same version.
    let hub = world.fixture.hub();
    let admin_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM admin WHERE kind = 'local' LIMIT 1")
            .fetch_one(&world.fixture.pool)
            .await
            .unwrap();
    let panel = Session::of_panel(
        hub.pool().clone(),
        hub.mark().clone(),
        admin.organization(),
        admin_id,
    );
    assert_eq!(panel.org_chart().await.unwrap(), chart);

    let wanted = Draft::parse(
        &json!({
            "levels": [ { "key": "new-1", "name": "board" } ],
            "hermes": [
                { "name": a.name, "level": "new-1", "x": 1, "role": "" },
                { "name": b.name, "level": "new-1", "x": 2, "role": "" },
            ],
            "links": [ { "from": a.name, "to": b.name, "wake": false } ],
        })
        .to_string(),
    )
    .unwrap();
    let by_panel = panel
        .org_publish(&wanted, chart.version(), world.now())
        .await
        .unwrap();
    assert!(matches!(by_panel, PublishOutcome::Published(_)));
    // The command sees the result, and publishing it again is no change.
    let now = admin.org_chart().await.unwrap();
    let again = admin.org_publish(&Draft::parse(&{
        let mut value = json!({
            "levels": [ { "key": now.levels[0].id.to_string(), "name": "board" } ],
            "hermes": [
                { "name": a.name, "level": now.levels[0].id.to_string(), "x": 1, "role": "" },
                { "name": b.name, "level": now.levels[0].id.to_string(), "x": 2, "role": "" },
            ],
            "links": [ { "from": a.name, "to": b.name, "wake": false } ],
        });
        value["links"][0]["wake"] = false.into();
        value.to_string()
    }).unwrap(), now.version(), world.now()).await.unwrap();
    assert_eq!(again, PublishOutcome::Unchanged);
    // Only the actor note tells the two apart in the audit.
    let notes: Vec<Option<String>> =
        sqlx::query_scalar("SELECT actor_note FROM audit WHERE action = 'org_chart.published'")
            .fetch_all(&world.fixture.pool)
            .await
            .unwrap();
    assert_eq!(notes, [Some("panel".to_owned())]);
}
