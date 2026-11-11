//! `hello` and the request to leave by a short connection, from the websocket to the database
//! (`docs/14-protocolo-do-canal.md`, sections 5.1 and 5.4; `docs/04-seguranca-conexao.md`,
//! C10).

use std::time::Duration;

use std::sync::Arc;

use arauto_hub::channel::{self, ChannelSettings, Downloads, Options};
use arauto_hub::config::ClientIpSource;
use arauto_hub::db::{self, PoolOptions, ResolvedOrg};
use arauto_hub::mark::Mark;
use arauto_hub::restore;
use arauto_protocol::clock::SystemClock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{HelloOk, Left, LeftStatus, Message, Purpose};
use arauto_protocol::noise::StaticKeypair;
use arauto_testkit::connector::{TestConnector, hello, identity};
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{self, Confirmed};
use crate::support::{self, Behavior, TestHub, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(10);

async fn leave(hub: &TestHub, identity: &StaticKeypair) -> LeftStatus {
    let mut connector = TestConnector::connect(hub.addr(), identity, &hub.hub_key)
        .await
        .unwrap();
    assert_eq!(
        connector.hello(Purpose::Leave).await.unwrap(),
        HelloOk::Agreed(1)
    );
    connector.send(&Message::Leave).await.unwrap();
    let Message::Left(Left { status }) = connector.recv(PATIENCE).await.unwrap() else {
        panic!("no left");
    };
    // The short connection ends after the answer, without a `closing`.
    assert_eq!(connector.ended(PATIENCE).await.unwrap().closing, None);
    status
}

async fn pool() -> PgPool {
    db::connect(
        &support::shared().await.url,
        PoolOptions {
            size: 2,
            acquire_timeout: Duration::from_secs(10),
        },
    )
    .await
    .unwrap()
}

/// A device of `state` whose identity is `identity`.
async fn device_of(pool: &PgPool, identity: &StaticKeypair, state: &str) -> Uuid {
    device_in(pool, support::shared().await.organization, identity, state).await
}

async fn device_in(
    pool: &PgPool,
    organization: ResolvedOrg,
    identity: &StaticKeypair,
    state: &str,
) -> Uuid {
    let hermes = common::hermes(pool, organization).await;
    let confirmed = if state == "pending" {
        Confirmed::Never
    } else {
        Confirmed::Yes
    };
    let (device, _) = common::device(pool, organization, hermes, state, confirmed).await;
    sqlx::query("UPDATE device SET public_key = $1 WHERE id = $2")
        .bind(identity.public().as_bytes().as_slice())
        .bind(device)
        .execute(pool)
        .await
        .unwrap();
    device
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_leaves_by_a_short_connection_and_the_database_says_so() {
    let pool = pool().await;
    let hub = support::start(support::settings(), TestPurposes(Behavior::Park)).await;

    let active = identity().unwrap();
    let device = device_of(&pool, &active, "active").await;
    assert_eq!(leave(&hub, &active).await, LeftStatus::Closed);
    let row: (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT state, state_reason, slot FROM device WHERE id = $1")
            .bind(device)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, ("revoked".to_owned(), Some("left".to_owned()), None));
    // Every closing asked by a device raises the alert, with the address it came from.
    let alert: (String, serde_json::Value) =
        sqlx::query_as("SELECT kind, details FROM alert WHERE device_id = $1")
            .bind(device)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(alert.0, "device_left");
    assert_eq!(alert.1["from"], "127.0.0.1");
    // Asked again: already closed, and nothing changes.
    assert_eq!(leave(&hub, &active).await, LeftStatus::AlreadyClosed);

    // A pending request that asks to leave is denied.
    let pending = identity().unwrap();
    let request = device_of(&pool, &pending, "pending").await;
    assert_eq!(leave(&hub, &pending).await, LeftStatus::Closed);
    assert_eq!(
        common::text_of(&pool, "device", "state", request)
            .await
            .as_deref(),
        Some("denied")
    );

    // An identity nobody knows.
    assert_eq!(leave(&hub, &identity().unwrap()).await, LeftStatus::Unknown);
}

#[tokio::test(flavor = "multi_thread")]
async fn hello_agrees_on_the_highest_common_version_or_closes_with_ar402() {
    let hub = support::start(support::settings(), TestPurposes(Behavior::Park)).await;
    for purpose in [Purpose::Enroll, Purpose::Channel, Purpose::Leave] {
        // Newer than the hub.
        let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
            .await
            .unwrap();
        connector
            .send(&hello((2, 3), purpose).unwrap())
            .await
            .unwrap();
        let ended = connector.ended(PATIENCE).await.unwrap();
        assert_eq!(ended.closing, Some(ErrorCode::Ar402), "{purpose:?}");
        // A range that includes the version of the hub.
        let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
            .await
            .unwrap();
        connector
            .send(&hello((1, 5), purpose).unwrap())
            .await
            .unwrap();
        assert!(
            matches!(
                connector.recv(PATIENCE).await.unwrap(),
                Message::HelloOk(HelloOk::Agreed(1))
            ),
            "{purpose:?}"
        );
    }
}

/// The database falls behind the mark while the hub runs: the request to leave writes
/// nothing, the hub turns the restored mode on, and every connection is told to come back.
#[tokio::test(flavor = "multi_thread")]
async fn a_leave_that_finds_the_database_restored_turns_the_mode_on_and_writes_nothing() {
    let installation = support::install().await;
    let pool = db::connect(
        &installation.url,
        PoolOptions {
            size: 2,
            acquire_timeout: Duration::from_secs(10),
        },
    )
    .await
    .unwrap();
    let running = channel::start(
        &support::config(&installation, ClientIpSource::Socket),
        Options {
            // Neither the sweep nor the listener: the request to leave is what finds the
            // restored database here.
            settings: ChannelSettings {
                sweep_interval_ms: 600_000,
                listen_admin: false,
                ..support::settings()
            },
            downloads: Downloads::empty(),
            purposes: TestPurposes(Behavior::Park),
        },
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    let addr = running.connectors_addr();
    let leaving = identity().unwrap();
    let device = device_in(&pool, installation.organization, &leaving, "active").await;
    let mut bystander = TestConnector::connect(addr, &identity().unwrap(), &installation.hub_key)
        .await
        .unwrap();
    bystander.hello(Purpose::Channel).await.unwrap();

    let last: i64 = sqlx::query_scalar("SELECT max(seq) FROM audit WHERE security")
        .fetch_one(&pool)
        .await
        .unwrap();
    Mark::new(&installation.data_dir)
        .advance(last + 10)
        .await
        .unwrap();

    let mut connector = TestConnector::connect(addr, &leaving, &installation.hub_key)
        .await
        .unwrap();
    connector.hello(Purpose::Leave).await.unwrap();
    connector.send(&Message::Leave).await.unwrap();
    let ended = connector.ended(PATIENCE).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar205));
    assert_eq!(
        bystander.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar205)
    );
    assert!(
        restore::installation(&pool)
            .await
            .unwrap()
            .unwrap()
            .restored_mode
    );
    // The leave was not written: the mode suspended the device instead.
    assert_eq!(
        common::text_of(&pool, "device", "state", device)
            .await
            .as_deref(),
        Some("suspended")
    );
    running.stop().await;
}
