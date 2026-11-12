//! The states of a number (`docs/10-maquinas-de-estado.md`, section 4), through the admin
//! command and the events of the engine.

use arauto_hub::admin::AdminError;
use arauto_hub::admin::whatsapp::{PairMethod, PairOutcome, Removal, Shown};
use arauto_hub::core::number::NumberState;
use arauto_protocol::clock::{Clock, SystemClock};
use serde_json::json;

use crate::world::{World, phone};

#[tokio::test]
async fn a_number_is_added_unpaired_with_its_session_created_configured() {
    let world = World::new().await;
    let number = world.add("vendas", &format!("+{}", phone(10))).await;
    assert_eq!(number.state, NumberState::Unpaired);
    let session = number.session.as_str();
    assert_eq!(world.fake.status(session).as_deref(), Some("STOPPED"));
    let (url, key) = world.fake.webhook(session).unwrap();
    let engine = world.engine().await;
    assert_eq!(
        url,
        format!(
            "http://{}/v1/engine/{}/events",
            world.fixture.running().internal_addr(),
            engine.id.as_hyphenated()
        )
    );
    assert_eq!(key.as_str(), engine.events.hmac_key().as_str());
    // The telephone is not in clear in the row; the session name was drawn by the hub.
    let row: String = sqlx::query_scalar("SELECT row_to_json(n)::text FROM number n WHERE id = $1")
        .bind(number.id)
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert!(!row.contains(&phone(10)), "{row}");
    assert_eq!(session.len(), 27);
    let audit = world.audit().await;
    assert!(audit.contains(&("number.added".to_owned(), "admin".to_owned())));
}

#[tokio::test]
async fn pairing_by_qr_connects_the_expected_telephone() {
    let world = World::new().await;
    world.add("vendas", &phone(11)).await;
    let number = world.number("vendas").await;
    world.fake.pairs_as(number.session.as_str(), &phone(11));
    let mut shown = Vec::new();
    let settings = world.hub().engines().settings().clone();
    let outcome = world
        .fixture
        .admin
        .number_pair(
            &world.vault,
            "vendas",
            PairMethod::Qr,
            &settings,
            &SystemClock,
            |s| {
                if let Shown::Qr(qr) = s {
                    shown.push(qr.expose().to_owned());
                }
            },
        )
        .await
        .unwrap();
    assert_eq!(outcome, PairOutcome::Connected);
    assert_eq!(shown.len(), 1);
    let number = world.number("vendas").await;
    assert_eq!(number.state, NumberState::Connected);
    assert!(number.paired_at.is_some());
    let actions: Vec<String> = world.audit().await.into_iter().map(|(a, _)| a).collect();
    assert!(actions.contains(&"number.pairing_started".to_owned()));
    // The QR is never stored anywhere.
    let stored: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(a)::text FROM audit a UNION ALL SELECT row_to_json(n)::text FROM number n",
    )
    .fetch_all(world.hub().pool())
    .await
    .unwrap();
    assert!(stored.iter().all(|row| !row.contains(&shown[0])));
}

#[tokio::test]
async fn pairing_by_code_shows_the_code_once() {
    let world = World::new().await;
    world.add("vendas", &phone(12)).await;
    let number = world.number("vendas").await;
    world.fake.pairs_as(number.session.as_str(), &phone(12));
    let mut codes = Vec::new();
    let settings = world.hub().engines().settings().clone();
    let outcome = world
        .fixture
        .admin
        .number_pair(
            &world.vault,
            "vendas",
            PairMethod::Code,
            &settings,
            &SystemClock,
            |s| {
                if let Shown::Code(code) = s {
                    codes.push(code.expose().to_owned());
                }
            },
        )
        .await
        .unwrap();
    assert_eq!(outcome, PairOutcome::Connected);
    assert_eq!(codes, ["ABCD-EFGH"]);
    let asked = world
        .fake
        .received()
        .into_iter()
        .find(|r| r.path.ends_with("/auth/request-code"))
        .unwrap();
    assert_eq!(asked.json().unwrap()["phoneNumber"], json!(phone(12)));
}

#[tokio::test]
async fn another_telephone_waits_for_the_admin() {
    let world = World::new().await;
    world.add("vendas", &phone(13)).await;
    assert_eq!(world.pair("vendas", &phone(14)).await, PairOutcome::Swapped);
    assert_eq!(world.state("vendas").await, NumberState::NumberSwapped);
    assert!(world.alerts().await.contains(&"number_swapped".to_owned()));
    // A Brazilian telephone without the ninth digit is the same telephone.
    world.add("suporte", "5511912345678").await;
    assert_eq!(
        world.pair("suporte", "551112345678").await,
        PairOutcome::Connected
    );
    // Confirming takes the new telephone as the one of the number.
    let confirmed = world
        .fixture
        .admin
        .number_confirm("vendas", world.now())
        .await
        .unwrap();
    assert_eq!(confirmed.state, NumberState::Connected);
    let key = world
        .hub()
        .vault()
        .open_organization_key(
            world.organization().id(),
            &sqlx::query_scalar::<_, Vec<u8>>("SELECT key_enc FROM organization WHERE id = $1")
                .bind(world.organization().id())
                .fetch_one(world.hub().pool())
                .await
                .unwrap(),
        )
        .unwrap();
    assert_eq!(confirmed.phone(&key).unwrap().digits(), phone(14));
    let actions: Vec<String> = world.audit().await.into_iter().map(|(a, _)| a).collect();
    assert!(actions.contains(&"number.confirmed".to_owned()));
    // Confirming applies only to a swapped number.
    assert!(matches!(
        world
            .fixture
            .admin
            .number_confirm("vendas", world.now())
            .await,
        Err(AdminError::NumberNotNow("connected"))
    ));
}

#[tokio::test]
async fn a_pairing_nobody_uses_expires() {
    let world = World::new().await;
    world.add("vendas", &phone(15)).await;
    let mut settings = world.hub().engines().settings().clone();
    settings.pairing_ms = 400;
    let outcome = world
        .fixture
        .admin
        .number_pair(
            &world.vault,
            "vendas",
            PairMethod::Qr,
            &settings,
            &SystemClock,
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(outcome, PairOutcome::Expired);
    let number = world.number("vendas").await;
    assert_eq!(number.state, NumberState::Unpaired);
    assert_eq!(
        world.fake.status(number.session.as_str()).as_deref(),
        Some("STOPPED")
    );
}

#[tokio::test]
async fn disconnect_restart_and_the_events_of_the_engine() {
    let world = World::new().await;
    let number = world.connected("vendas", &phone(16)).await;
    let session = number.session.as_str();
    let changed = world
        .fixture
        .admin
        .number_disconnect(&world.vault, "vendas", world.now())
        .await
        .unwrap();
    assert_eq!(changed.number.state, NumberState::Disconnected);
    assert!(changed.engine.is_ok());
    assert_eq!(world.fake.status(session).as_deref(), Some("STOPPED"));
    // The telephone still has the device: pairing again finds it working.
    assert_eq!(
        world.pair("vendas", &phone(16)).await,
        PairOutcome::Connected
    );

    let now = SystemClock.now_ms();
    let failed = world
        .fake
        .envelope_at(
            session,
            "session.status",
            json!({"status": "FAILED"}),
            None,
            now,
        )
        .unwrap();
    assert_eq!(world.fake.deliver(session, &failed).await.unwrap(), 200);
    assert_eq!(world.state("vendas").await, NumberState::Failed);
    let restarted = world
        .fixture
        .admin
        .number_restart(&world.vault, "vendas", world.now())
        .await
        .unwrap();
    assert_eq!(restarted.number.state, NumberState::Reconnecting);
    let working = world
        .fake
        .envelope_at(
            session,
            "session.status",
            json!({"status": "WORKING"}),
            None,
            now + 1,
        )
        .unwrap();
    assert_eq!(world.fake.deliver(session, &working).await.unwrap(), 200);
    assert_eq!(world.state("vendas").await, NumberState::Connected);
    // What does not apply to the state is refused, and nothing changes.
    assert!(matches!(
        world
            .fixture
            .admin
            .number_restart(&world.vault, "vendas", world.now())
            .await,
        Err(AdminError::NumberNotNow("connected"))
    ));
}

#[tokio::test]
async fn removing_ends_the_number_keeps_or_erases_its_session_and_never_frees_the_alias() {
    let world = World::new().await;
    let kept = world.connected("vendas", &phone(17)).await;
    let erased = world.connected("suporte", &phone(18)).await;
    let removed = world
        .fixture
        .admin
        .number_remove(&world.vault, "vendas", Removal::KeepInEngine, world.now())
        .await
        .unwrap();
    assert_eq!(removed.number.state, NumberState::Removed);
    assert_eq!(
        world.fake.status(kept.session.as_str()).as_deref(),
        Some("STOPPED")
    );
    world
        .fixture
        .admin
        .number_remove(
            &world.vault,
            "suporte",
            Removal::EraseEngineHistory,
            world.now(),
        )
        .await
        .unwrap();
    assert_eq!(world.fake.status(erased.session.as_str()), None);
    // Gone from the hub; the alias is never given to another number; the telephone may come
    // back under another alias.
    assert!(matches!(
        world
            .fixture
            .admin
            .number_disconnect(&world.vault, "vendas", world.now())
            .await,
        Err(AdminError::NoSuchNumber)
    ));
    assert!(matches!(
        world
            .fixture
            .admin
            .number_add(&world.vault, "vendas", &phone(19), false, world.now())
            .await,
        Err(AdminError::AliasTaken)
    ));
    let again = world.add("vendas-2", &phone(17)).await;
    assert_ne!(
        again.session, kept.session,
        "a session name is never reused"
    );
    // An event of the removed number is not written.
    let body = world
        .fake
        .envelope(
            kept.session.as_str(),
            "session.status",
            json!({"status": "WORKING"}),
            None,
        )
        .unwrap();
    assert_eq!(
        world
            .fake
            .deliver(kept.session.as_str(), &body)
            .await
            .unwrap(),
        400
    );
    let actions: Vec<String> = world.audit().await.into_iter().map(|(a, _)| a).collect();
    assert_eq!(actions.iter().filter(|a| *a == "number.removed").count(), 2);
}
