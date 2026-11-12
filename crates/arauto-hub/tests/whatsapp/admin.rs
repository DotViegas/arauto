//! `arauto-hub admin engine ...` and `arauto-hub admin number ...`: what they refuse, what they
//! write and print, the secrets sealed, and the rules of the restored database.

use arauto_hub::admin::AdminError;
use arauto_hub::admin::render::whatsapp as render;
use arauto_hub::engine::{AddressError, EngineError};
use arauto_hub::repo::engine::SetOutcome;
use arauto_hub::vault::Vault;

use crate::fixture::{self, Fixture};
use crate::world::{KEY, World, phone};

#[tokio::test]
async fn before_the_engine_is_set_the_commands_say_so() {
    let fixture = Fixture::new(fixture::settings()).await;
    let vault = Vault::open(&fixture.installation.data_dir).unwrap();
    let now = fixture.now();
    assert!(matches!(
        fixture.admin.engine_set_key(&vault, KEY, now).await,
        Err(AdminError::NoEngine)
    ));
    assert!(matches!(
        fixture.admin.engine_new_events_secret(&vault, now).await,
        Err(AdminError::NoEngine)
    ));
    assert!(matches!(
        fixture
            .admin
            .number_add(&vault, "vendas", &phone(30), false, now)
            .await,
        Err(AdminError::NoEngine)
    ));
    assert!(matches!(
        fixture.admin.engine_health(&vault).await,
        Err(AdminError::NoEngine)
    ));
    assert!(!render::engine_show(None).done);
    // An address of the events with an underscore is refused: the engine refuses it.
    assert!(matches!(
        fixture
            .admin
            .engine_set(&vault, "http://192.0.2.10:3000", "http://my_hub:9000", now)
            .await,
        Err(AdminError::InvalidAddress(AddressError::Underscore))
    ));
    for (url, events) in [
        ("192.0.2.10:3000", "http://hub:9000"),
        ("http://192.0.2.10:3000/api", "http://hub:9000"),
        ("http://user:pass@192.0.2.10", "http://hub:9000"),
        ("http://192.0.2.10:3000", "http://hub:9000/x"),
    ] {
        assert!(
            matches!(
                fixture.admin.engine_set(&vault, url, events, now).await,
                Err(AdminError::InvalidAddress(_))
            ),
            "{url} {events}"
        );
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM engine")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn the_key_and_the_secret_are_sealed_and_never_shown() {
    let world = World::new().await;
    let row = world.fixture.admin.engine_show().await.unwrap().unwrap();
    let text = render::engine_show(Some(&row)).text;
    assert!(text.contains("key            set"), "{text}");
    let secret = world.engine().await.events.hmac_key().to_string();
    assert!(!text.contains(KEY) && !text.contains(&secret), "{text}");
    assert!(!format!("{row:?}").contains(KEY));
    // Not in clear anywhere in the database.
    let stored: String = sqlx::query_scalar("SELECT row_to_json(e)::text FROM engine e")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert!(
        !stored.contains(KEY) && !stored.contains(&secret),
        "{stored}"
    );
    assert!(!stored.contains(&data_encoding::HEXLOWER.encode(KEY.as_bytes())));
    // A key outside the format is refused without being repeated.
    let refused = world
        .fixture
        .admin
        .engine_set_key(&world.vault, "short", world.now())
        .await
        .unwrap_err();
    assert!(matches!(refused, AdminError::InvalidKey(_)));
    assert!(!refused.to_string().contains("short"));
    // Setting the same addresses again writes nothing.
    let events = format!("http://{}", world.fixture.running().internal_addr());
    assert!(matches!(
        world
            .fixture
            .admin
            .engine_set(&world.vault, &world.fake.address(), &events, world.now())
            .await
            .unwrap(),
        SetOutcome::Unchanged(_)
    ));
    let actions: Vec<(String, String)> = world.audit().await;
    for action in ["engine.configured", "engine.key_changed"] {
        assert!(
            actions.contains(&(action.to_owned(), "admin".to_owned())),
            "{action}"
        );
    }
    let security: Vec<bool> = sqlx::query_scalar(
        "SELECT security FROM audit WHERE action IN ('engine.configured', 'engine.key_changed')",
    )
    .fetch_all(world.hub().pool())
    .await
    .unwrap();
    assert!(security.iter().all(|security| *security));
}

#[tokio::test]
async fn health_and_version_ask_the_engine() {
    let world = World::new().await;
    world
        .fixture
        .admin
        .engine_health(&world.vault)
        .await
        .unwrap();
    let version = world
        .fixture
        .admin
        .engine_version(&world.vault, world.now())
        .await
        .unwrap();
    assert!(
        render::engine_version(&version)
            .text
            .contains("checked against")
    );
    world.fake.set_version("2027.1.0", "GOWS");
    let version = world
        .fixture
        .admin
        .engine_version(&world.vault, world.now())
        .await
        .unwrap();
    assert!(
        render::engine_version(&version)
            .text
            .contains("NOT a version")
    );
    assert_eq!(world.alerts().await, ["engine_version_untested"]);
    // A key the engine does not take.
    world.fake.set_api_key("rotated-key-0002");
    assert!(matches!(
        world.fixture.admin.engine_health(&world.vault).await,
        Err(AdminError::Engine(EngineError::Unavailable(_)))
    ));
}

#[tokio::test]
async fn an_alias_or_a_telephone_outside_the_format_is_refused_before_anything() {
    let world = World::new().await;
    world.fake.forget_received();
    for alias in [
        "Vendas",
        "admin",
        "../x",
        "a b",
        "",
        &"a".repeat(33),
        "vendas\u{202e}",
    ] {
        assert!(
            matches!(
                world
                    .fixture
                    .admin
                    .number_add(&world.vault, alias, &phone(31), false, world.now())
                    .await,
                Err(AdminError::InvalidAlias(_))
            ),
            "{alias:?}"
        );
    }
    for telephone in [
        "",
        "123",
        "+55 11 9876a5432",
        "5511/987654321",
        "00551198765432",
    ] {
        let refused = world
            .fixture
            .admin
            .number_add(&world.vault, "vendas", telephone, false, world.now())
            .await
            .unwrap_err();
        assert!(matches!(refused, AdminError::InvalidPhone), "{telephone:?}");
    }
    assert!(
        world.fake.received().is_empty(),
        "nothing reached the engine"
    );
    assert!(
        world
            .fixture
            .admin
            .number_list(true)
            .await
            .unwrap()
            .is_empty()
    );
    // The same telephone twice is one number.
    world.add("vendas", &phone(32)).await;
    assert!(matches!(
        world
            .fixture
            .admin
            .number_add(&world.vault, "outro", &phone(32), false, world.now())
            .await,
        Err(AdminError::PhoneTaken)
    ));
    // The list shows each number, with its telephone, to the admin on the machine of the hub.
    let number = world.number("vendas").await;
    let telephone = world
        .fixture
        .admin
        .number_phone(&world.vault, &number)
        .await
        .unwrap();
    let text = render::number_list(&[(number, Some(telephone))]).text;
    assert!(
        text.contains("vendas") && text.contains(&phone(32)) && text.contains("unpaired"),
        "{text}"
    );
}

#[tokio::test]
async fn a_database_that_came_back_from_a_copy_gets_nothing_from_the_admin() {
    let mut world = World::new().await;
    world.add("vendas", &phone(33)).await;
    // The hub is down, and the database is behind the mark: the admin only refuses.
    world.fixture.stop().await;
    world.fixture.restore_an_older_copy().await;
    let now = world.now();
    let admin = &world.fixture.admin;
    assert!(matches!(
        admin
            .number_add(&world.vault, "suporte", &phone(34), false, now)
            .await,
        Err(AdminError::Restored(_))
    ));
    assert!(matches!(
        admin.number_disconnect(&world.vault, "vendas", now).await,
        Err(AdminError::Restored(_))
    ));
    assert!(matches!(
        admin.engine_new_events_secret(&world.vault, now).await,
        Err(AdminError::Restored(_))
    ));
    assert!(matches!(
        admin
            .engine_set(
                &world.vault,
                "http://192.0.2.10:3000",
                "http://hub:9000",
                now
            )
            .await,
        Err(AdminError::Restored(_))
    ));
    let numbers: i64 = sqlx::query_scalar("SELECT count(*) FROM number")
        .fetch_one(&world.fixture.pool)
        .await
        .unwrap();
    assert_eq!(numbers, 1);
}
