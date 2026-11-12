//! `admin access ...`: granting, changing and taking away, the lines of chats, the rules of
//! `docs/08-permissoes.md` (section 1), the audit with what was before and after, the
//! warnings that never prevent, the end of the accesses with their Hermes or their number,
//! and the restored mode.

use arauto_hub::admin::AdminError;
use arauto_hub::admin::access::Warning;
use arauto_hub::admin::whatsapp::Removal;
use arauto_hub::core::access::{AccessRuleError, Grant, Scope};
use serde_json::{Value, json};

use crate::agent::{ALL_VERBS, NONE, READ, READ_SEND, all, list, verbs};
use crate::world::{World, phone};

async fn access_rows(world: &World) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM access WHERE organization_id = $1")
        .bind(world.organization().id())
        .fetch_one(world.hub().pool())
        .await
        .unwrap()
}

async fn events(world: &World, action: &str) -> Vec<(bool, Value)> {
    sqlx::query_as::<_, (bool, sqlx::types::Json<Value>)>(
        "SELECT security, details FROM audit WHERE organization_id = $1 AND action = $2 \
         ORDER BY seq",
    )
    .bind(world.organization().id())
    .bind(action)
    .fetch_all(world.hub().pool())
    .await
    .unwrap()
    .into_iter()
    .map(|(security, details)| (security, details.0))
    .collect()
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn grants_and_lines_are_written_with_what_was_before_and_after() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(7)).await;
    let hermes = world.fixture.hermes().await;
    let admin = &world.fixture.admin;
    let now = world.now();

    let first = admin
        .access_grant(&hermes, "balcao", all(READ), now)
        .await
        .unwrap();
    assert_eq!(first.before, None);
    assert!(!first.unchanged);
    let again = admin
        .access_grant(&hermes, "balcao", all(READ), now)
        .await
        .unwrap();
    assert!(again.unchanged, "the same access is not written twice");
    let changed = admin
        .access_grant(&hermes, "balcao", list(READ_SEND), now)
        .await
        .unwrap();
    assert_eq!(changed.before, Some(all(READ)));
    let granted = events(&world, "access.granted").await;
    assert_eq!(granted.len(), 1);
    assert!(granted[0].0, "a security event");
    assert_eq!(granted[0].1["verbs"], json!(["read"]));
    assert_eq!(granted[0].1["number"], "balcao");
    let changes = events(&world, "access.changed").await;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].1["scope"], "list");
    assert_eq!(changes[0].1["scope_before"], "all");
    assert_eq!(changes[0].1["verbs"], json!(["read", "send"]));
    assert_eq!(changes[0].1["verbs_before"], json!(["read"]));

    // A line by telephone: the engine gives the exact chat.
    let ana = format!("{}@c.us", phone(58));
    world.fake.register(&phone(58), &ana);
    let line = admin
        .access_chat(
            &world.vault,
            &hermes,
            "balcao",
            &format!("+{}", phone(58)),
            READ,
            now,
        )
        .await
        .unwrap();
    let chat = line.chat.unwrap();
    // A line by the identifier of the engine, of a group, and by the one of the hub.
    admin
        .access_chat(
            &world.vault,
            &hermes,
            "balcao",
            "120363000000000009@g.us",
            NONE,
            now,
        )
        .await
        .unwrap();
    let by_hub = admin
        .access_chat(
            &world.vault,
            &hermes,
            "balcao",
            &chat.to_string(),
            READ_SEND,
            now,
        )
        .await
        .unwrap();
    assert_eq!(by_hub.chat, Some(chat));
    assert_eq!(by_hub.before, Some(READ));
    let set = events(&world, "access_chat.set").await;
    assert_eq!(set.len(), 3);
    assert!(set.iter().all(|(security, _)| *security));
    assert_eq!(set[2].1["verbs_before"], json!(["read"]));
    let listed = admin.access_list(&world.vault, None).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].access.lines.len(), 2);
    assert!(
        listed[0]
            .chats
            .iter()
            .flatten()
            .any(|id| id.as_str() == ana)
    );
    let shown = arauto_hub::admin::render::access::list(&listed).text;
    assert!(
        shown.contains(&ana) && shown.contains("scope list"),
        "{shown}"
    );

    // What is not a chat, a telephone without WhatsApp, a chat of no line.
    for (given, expected) in [
        ("not a chat", "invalid"),
        ("abc@c.us", "invalid"),
        ("5511987600999", "no whatsapp"),
    ] {
        let error = admin
            .access_chat(&world.vault, &hermes, "balcao", given, READ, now)
            .await
            .unwrap_err();
        match (expected, error) {
            ("invalid", AdminError::InvalidChat) | ("no whatsapp", AdminError::NoWhatsapp) => {}
            (_, other) => panic!("{given}: {other:?}"),
        }
    }
    let other_number = world.connected("outro", &phone(8)).await;
    let elsewhere = admin
        .access_chat(&world.vault, &hermes, "outro", &chat.to_string(), READ, now)
        .await
        .unwrap_err();
    assert!(matches!(elsewhere, AdminError::NoAccess), "{elsewhere:?}");
    admin
        .access_grant(&hermes, "outro", list(NONE), now)
        .await
        .unwrap();
    // A chat of another number is not a chat of this one.
    let elsewhere = admin
        .access_chat(&world.vault, &hermes, "outro", &chat.to_string(), READ, now)
        .await
        .unwrap_err();
    assert!(matches!(elsewhere, AdminError::NoSuchChat), "{elsewhere:?}");
    let _ = other_number;

    // Taking the line away, and then the access.
    let removed = admin
        .access_chat_remove(&world.vault, &hermes, "balcao", &chat.to_string(), now)
        .await
        .unwrap();
    assert_eq!(removed.before, Some(READ_SEND));
    let none = admin
        .access_chat_remove(&world.vault, &hermes, "balcao", &chat.to_string(), now)
        .await
        .unwrap_err();
    assert!(matches!(none, AdminError::NoLine), "{none:?}");
    assert_eq!(
        admin.access_remove(&hermes, "balcao", now).await.unwrap(),
        1,
        "the line of the group went with it"
    );
    let gone = events(&world, "access.removed").await;
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].1["reason"], "admin");
    assert_eq!(events(&world, "access_chat.removed").await.len(), 1);
    let _ = number;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_rules_and_the_names_are_checked_before_anything_is_written() {
    let world = World::new().await;
    world.connected("balcao", &phone(9)).await;
    let hermes = world.fixture.hermes().await;
    let admin = &world.fixture.admin;
    let now = world.now();
    for (grant, rule) in [
        (
            all(verbs(false, false, false, true)),
            AccessRuleError::WakeWithoutRead,
        ),
        (
            Grant {
                can_start_chats: true,
                ..all(READ)
            },
            AccessRuleError::StartWithoutSendOrAll,
        ),
        (
            Grant {
                scope: Scope::List,
                can_start_chats: true,
                ..all(READ_SEND)
            },
            AccessRuleError::StartWithoutSendOrAll,
        ),
    ] {
        let error = admin
            .access_grant(&hermes, "balcao", grant, now)
            .await
            .unwrap_err();
        assert!(
            matches!(error, AdminError::AccessRule(found) if found == rule),
            "{error:?}"
        );
    }
    let error = admin
        .access_grant("no-one", "balcao", all(READ), now)
        .await
        .unwrap_err();
    assert!(matches!(error, AdminError::NoSuchHermes), "{error:?}");
    for alias in ["no-number", "Not An Alias"] {
        let error = admin
            .access_grant(&hermes, alias, all(READ), now)
            .await
            .unwrap_err();
        assert!(matches!(error, AdminError::NoSuchNumber), "{error:?}");
    }
    let error = admin
        .access_chat(
            &world.vault,
            &hermes,
            "balcao",
            "120363000000000009@g.us",
            verbs(false, false, false, true),
            now,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AdminError::AccessRule(_)), "{error:?}");
    assert_eq!(access_rows(&world).await, 0);
    // The database refuses a row that breaks the rules too.
    admin
        .access_grant(&hermes, "balcao", all(READ), now)
        .await
        .unwrap();
    for broken in [
        "UPDATE access SET can_wake = TRUE, can_read = FALSE WHERE organization_id = $1",
        "UPDATE access SET can_start_chats = TRUE WHERE organization_id = $1",
    ] {
        let refused = sqlx::query(broken)
            .bind(world.organization().id())
            .execute(world.hub().pool())
            .await;
        assert!(refused.is_err(), "{broken}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn warnings_are_given_and_nothing_is_prevented() {
    let world = World::new().await;
    world
        .fixture
        .admin
        .number_add(&world.vault, "comando", &phone(10), true, world.now())
        .await
        .unwrap();
    let first = world.fixture.hermes().await;
    let second = world.fixture.hermes().await;
    let admin = &world.fixture.admin;
    let granted = admin
        .access_grant(&first, "comando", all(READ_SEND), world.now())
        .await
        .unwrap();
    assert_eq!(granted.warnings, [Warning::CommandNumber]);
    let granted = admin
        .access_grant(&second, "comando", all(ALL_VERBS), world.now())
        .await
        .unwrap();
    assert!(granted.warnings.contains(&Warning::CommandNumber));
    assert!(
        granted
            .warnings
            .iter()
            .any(|warning| matches!(warning, Warning::Shared(names) if names.len() == 1))
    );
    let text = arauto_hub::admin::render::access::granted(&second, "comando", &granted).text;
    assert!(text.contains("WARNING"), "{text}");
    assert_eq!(access_rows(&world).await, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn accesses_end_with_their_number_and_with_their_hermes() {
    let world = World::new().await;
    world.connected("balcao", &phone(11)).await;
    world.connected("outro", &phone(12)).await;
    let first = world.fixture.hermes().await;
    let second = world.fixture.hermes().await;
    let admin = &world.fixture.admin;
    for hermes in [&first, &second] {
        for number in ["balcao", "outro"] {
            admin
                .access_grant(hermes, number, all(READ), world.now())
                .await
                .unwrap();
        }
    }
    assert_eq!(access_rows(&world).await, 4);
    admin
        .number_remove(&world.vault, "balcao", Removal::KeepInEngine, world.now())
        .await
        .unwrap();
    assert_eq!(access_rows(&world).await, 2);
    admin.hermes_remove(&first, world.now()).await.unwrap();
    assert_eq!(access_rows(&world).await, 1);
    let removed = events(&world, "access.removed").await;
    let reasons: Vec<&str> = removed
        .iter()
        .map(|(_, details)| details["reason"].as_str().unwrap())
        .collect();
    assert_eq!(
        reasons,
        ["number_removed", "number_removed", "hermes_removed"]
    );
    assert!(removed.iter().all(|(security, _)| *security));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_database_that_came_back_from_a_copy_gets_no_access_change() {
    let mut world = World::new().await;
    world.connected("balcao", &phone(13)).await;
    let hermes = world.fixture.hermes().await;
    let admin = &world.fixture.admin;
    admin
        .access_grant(&hermes, "balcao", all(READ), world.now())
        .await
        .unwrap();
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    // The hub is down, and the database is behind the mark: the admin only refuses.
    world.fixture.stop().await;
    world.fixture.restore_an_older_copy().await;
    let now = world.now();
    let admin = &world.fixture.admin;
    let error = admin
        .access_grant(&hermes, "balcao", all(READ_SEND), now)
        .await
        .unwrap_err();
    assert!(matches!(error, AdminError::Restored(_)), "{error:?}");
    let error = admin
        .access_remove(&hermes, "balcao", now)
        .await
        .unwrap_err();
    assert!(matches!(error, AdminError::Restored(_)), "{error:?}");
    let error = admin
        .access_chat(
            &world.vault,
            &hermes,
            "balcao",
            "120363000000000009@g.us",
            READ,
            now,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AdminError::Restored(_)), "{error:?}");
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(&world.fixture.pool)
        .await
        .unwrap();
    assert_eq!(after, audited, "nothing was written");
    let verbs: (bool, bool) =
        sqlx::query_as("SELECT can_read, can_send FROM access WHERE organization_id = $1")
            .bind(world.organization().id())
            .fetch_one(&world.fixture.pool)
            .await
            .unwrap();
    assert_eq!(verbs, (true, false));
}
