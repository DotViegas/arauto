//! `arauto-hub admin link ...`: what it refuses, what it writes and what it audits.

use arauto_hub::admin::AdminError;
use arauto_hub::admin::render::link as render;
use arauto_hub::repo::peer::LinkSet;

use crate::world::World;

#[tokio::test]
async fn a_link_is_created_changed_listed_and_removed_and_each_change_is_audited() {
    let world = World::new().await;
    let admin = &world.fixture.admin;
    let a = world.fixture.hermes().await;
    let b = world.fixture.hermes().await;
    let now = world.now();

    let linked = admin.link_add(&a, &b, false, false, now).await.unwrap();
    assert_eq!(linked.changes.len(), 1);
    assert_eq!(linked.changes[0].outcome, LinkSet::Created);
    // A new link shows what each side reaches, and the reminder that a message is data.
    assert_eq!(linked.reach.len(), 2);
    let text = render::linked(&linked).text;
    assert!(text.contains("never an order"), "{text}");

    let same = admin.link_add(&a, &b, false, false, now).await.unwrap();
    assert_eq!(same.changes[0].outcome, LinkSet::Unchanged);
    assert!(same.reach.is_empty());
    let woken = admin.link_add(&a, &b, true, false, now).await.unwrap();
    assert_eq!(woken.changes[0].outcome, LinkSet::Changed);

    // `--both`: the link back is created, and the one that was there is left as it is.
    let both = admin.link_add(&a, &b, true, true, now).await.unwrap();
    assert_eq!(
        both.changes
            .iter()
            .map(|change| change.outcome)
            .collect::<Vec<_>>(),
        [LinkSet::Unchanged, LinkSet::Created]
    );

    let listed = admin.link_list().await.unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|link| link.wake));

    let removed = admin.link_remove(&a, &b, true, now).await.unwrap();
    assert!(removed.iter().all(|one| one.removed));
    let again = admin.link_remove(&a, &b, false, now).await.unwrap();
    assert!(!again[0].removed);
    assert!(!render::unlinked(&again).done);
    assert!(admin.link_list().await.unwrap().is_empty());

    let actions: Vec<(String, bool)> = sqlx::query_as(
        "SELECT action, security FROM audit WHERE action LIKE 'peer_link.%' ORDER BY seq",
    )
    .fetch_all(&world.fixture.pool)
    .await
    .unwrap();
    let names: Vec<&str> = actions.iter().map(|(action, _)| action.as_str()).collect();
    assert_eq!(
        names,
        [
            "peer_link.created",
            "peer_link.changed",
            "peer_link.created",
            "peer_link.removed",
            "peer_link.removed",
        ]
    );
    assert!(actions.iter().all(|(_, security)| *security));
}

#[tokio::test]
async fn a_link_needs_two_different_hermes_that_exist() {
    let world = World::new().await;
    let admin = &world.fixture.admin;
    let a = world.fixture.hermes().await;
    let now = world.now();

    assert!(matches!(
        admin.link_add(&a, &a, false, false, now).await,
        Err(AdminError::LinkToItself)
    ));
    for (from, to) in [
        (a.as_str(), "nobody"),
        ("nobody", a.as_str()),
        ("Not A Name", a.as_str()),
    ] {
        assert!(matches!(
            admin.link_add(from, to, false, false, now).await,
            Err(AdminError::NoSuchHermes)
        ));
        assert!(matches!(
            admin.link_remove(from, to, false, now).await,
            Err(AdminError::NoSuchHermes)
        ));
    }
    assert!(admin.link_list().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_database_that_came_back_from_a_copy_gets_no_link() {
    let mut world = World::new().await;
    let a = world.fixture.hermes().await;
    let b = world.fixture.hermes().await;
    world.fixture.stop().await;
    world.fixture.restore_an_older_copy().await;
    let now = world.now();
    assert!(matches!(
        world
            .fixture
            .admin
            .link_add(&a, &b, false, false, now)
            .await,
        Err(AdminError::Restored(_))
    ));
}
