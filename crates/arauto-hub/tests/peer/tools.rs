//! The list of tools follows the list of peers, and `arauto_status` says what the Hermes has.

use serde_json::json;

use crate::world::{World, ok};

const PEER_TOOLS: [&str; 4] = [
    "hermes_list_peers",
    "hermes_send_message",
    "hermes_list_threads",
    "hermes_read_thread",
];

fn peer_tools(names: &[String]) -> Vec<&str> {
    PEER_TOOLS
        .into_iter()
        .filter(|tool| names.iter().any(|name| name == tool))
        .collect()
}

#[tokio::test]
async fn the_list_of_tools_changes_with_the_list_of_peers() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    assert!(peer_tools(&a.started_with()).is_empty(), "no link, no tool");

    world.link(&a, &b, false).await;
    // Who may send gets the four; who only receives, the three that read.
    assert_eq!(peer_tools(&a.next_tools().await), PEER_TOOLS);
    assert_eq!(
        peer_tools(&b.next_tools().await),
        [
            "hermes_list_peers",
            "hermes_list_threads",
            "hermes_read_thread"
        ]
    );

    world.unlink(&a, &b).await;
    assert!(peer_tools(&a.next_tools().await).is_empty());
    assert!(peer_tools(&b.next_tools().await).is_empty());

    // A session that starts with a link already has its tools in `session`.
    let name = world.fixture.hermes().await;
    world
        .fixture
        .admin
        .link_add(&name, &a.name, false, false, world.now())
        .await
        .unwrap();
    let device = world.fixture.device();
    let code = world.fixture.enroll(&name, &device).await;
    world.fixture.approve(&name, &code).await;
    let session = device
        .channel(arauto_testkit::device::Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    assert_eq!(
        peer_tools(&crate::world::names(&session.session.tools)),
        PEER_TOOLS
    );
}

#[tokio::test]
async fn the_peers_are_listed_with_what_each_direction_allows() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    let c = world.agent().await;
    let d = world.agent().await;
    let _stranger = world.agent().await;
    world.link(&a, &b, false).await;
    world.link(&c, &a, false).await;
    world.link(&a, &d, false).await;
    world.link(&d, &a, true).await;

    let listed = ok(a.call("hermes_list_peers", json!({})).await);
    let mut expected = vec![
        json!({"name": b.name, "can_send": true, "can_receive": false}),
        json!({"name": c.name, "can_send": false, "can_receive": true}),
        json!({"name": d.name, "can_send": true, "can_receive": true}),
    ];
    expected.sort_by_key(|peer| peer["name"].as_str().unwrap().to_owned());
    assert_eq!(listed["peers"], json!(expected));

    let status = ok(a.call("arauto_status", json!({})).await);
    assert_eq!(
        status["peers"],
        json!({"enabled": true, "can_send": true, "can_receive": true})
    );
}

#[tokio::test]
async fn a_hermes_without_links_is_refused_by_the_database_whatever_list_it_claims() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    // The tools are not in its list; a hostile connector calls them anyway.
    let not_released = Some(arauto_protocol::errors::ErrorCode::Ar701);
    for tool in ["hermes_list_peers", "hermes_list_threads"] {
        assert_eq!(
            crate::world::refused(&a.call(tool, json!({})).await),
            not_released,
            "{tool}"
        );
    }
    let read = a
        .call(
            "hermes_read_thread",
            json!({"thread": uuid::Uuid::new_v4().to_string()}),
        )
        .await;
    assert_eq!(crate::world::refused(&read), not_released);
    let sent = a.send_to(&b.name, "anyway").await;
    assert_eq!(crate::world::refused(&sent), not_released);
    // One that only receives may read, and may not send.
    world.link(&b, &a, false).await;
    assert_eq!(
        ok(a.call("hermes_list_peers", json!({})).await)["peers"][0]["name"],
        b.name.as_str()
    );
    let sent = a.send_to(&b.name, "anyway").await;
    assert_eq!(crate::world::refused(&sent), not_released);
}
