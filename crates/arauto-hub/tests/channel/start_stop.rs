//! The start and the orderly stop of `serve`: the restored mode is on before the first
//! connection is accepted, what the last run left open is closed, and the stop sends AR-205
//! to every connection, waits for them and refuses new ones.

use std::sync::Arc;
use std::time::Duration;

use arauto_hub::channel::{self, Options, Served};
use arauto_hub::config::ClientIpSource;
use arauto_hub::db::{self, PoolOptions};
use arauto_hub::mark::Mark;
use arauto_hub::repo;
use arauto_hub::restore;
use arauto_protocol::clock::SystemClock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{DeclaredText, Message, Purpose};
use arauto_testkit::connector::{TestConnector, identity};
use arauto_testkit::{KitError, RawSocket};

use crate::common::{self, Confirmed};
use crate::support::{self, Behavior, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(15);

#[tokio::test(flavor = "multi_thread")]
async fn the_orderly_stop_sends_ar205_to_every_connection_and_refuses_new_ones() {
    let mut settings = support::settings();
    settings.deadlines.shutdown_grace_ms = 3_000;
    let hub = support::start(settings, TestPurposes(Behavior::Session)).await;
    let addr = hub.addr();
    let key = hub.hub_key;

    let mut session = TestConnector::connect(addr, &identity().unwrap(), &key)
        .await
        .unwrap();
    session.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        session.recv(PATIENCE).await.unwrap(),
        Message::Session(_)
    ));
    let mut before_hello = TestConnector::connect(addr, &identity().unwrap(), &key)
        .await
        .unwrap();
    let mut leaving = TestConnector::connect(addr, &identity().unwrap(), &key)
        .await
        .unwrap();
    leaving.hello(Purpose::Leave).await.unwrap();
    let mut in_handshake = RawSocket::open(addr).await.unwrap();
    assert!(hub.links_become(3, PATIENCE).await);

    let (_, took) = support::timed(hub.running.stop()).await;
    assert!(took < Duration::from_secs(5), "{took:?}");

    for connector in [&mut session, &mut before_hello, &mut leaving] {
        let ended = connector.ended(PATIENCE).await.unwrap();
        assert_eq!(ended.closing, Some(ErrorCode::Ar205));
    }
    // Without a cipher there is no `closing`: it is just closed.
    assert_eq!(in_handshake.ended(PATIENCE).await.unwrap().closing, None);
    // Nobody listens any more.
    match RawSocket::open(addr).await {
        Err(KitError::Io(_) | KitError::Ws(_)) => {}
        other => panic!("a stopped hub still answered: {other:?}"),
    }
}

/// A database behind its mark is put in the restored mode before the hub listens: the first
/// connection already meets the mode.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_over_a_restored_database_turns_the_mode_on_before_accepting_a_connection() {
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
    // A hermes with an active device, then a mark ahead of the database: what a restored
    // copy looks like.
    let hermes = common::hermes(&pool, installation.organization).await;
    let (device, _) = common::device(
        &pool,
        installation.organization,
        hermes,
        "active",
        Confirmed::Yes,
    )
    .await;
    let mark = Mark::new(&installation.data_dir);
    let last: i64 = sqlx::query_scalar("SELECT max(seq) FROM audit WHERE security")
        .fetch_one(&pool)
        .await
        .unwrap();
    mark.advance(last + 10).await.unwrap();
    assert!(matches!(
        restore::check(&pool, &mark).await.unwrap(),
        restore::Standing::Restored(_)
    ));

    let running = channel::start(
        &support::config(&installation, ClientIpSource::Socket),
        Options {
            settings: support::settings(),
            ..Options::default()
        },
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    // Listening now, and the mode is already on.
    let on = restore::installation(&pool).await.unwrap().unwrap();
    assert!(on.restored_mode);
    let state = common::text_of(&pool, "device", "state", device).await;
    assert_eq!(state.as_deref(), Some("suspended"));
    assert_eq!(
        restore::check(&pool, &mark).await.unwrap(),
        restore::Standing::Consistent
    );
    // And it accepts.
    let mut connector = TestConnector::connect(
        running.connectors_addr(),
        &identity().unwrap(),
        &installation.hub_key,
    )
    .await
    .unwrap();
    connector.hello(Purpose::Leave).await.unwrap();
    running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_start_closes_the_connections_the_last_run_left_open_and_refuses_an_empty_install() {
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
    let organization = installation.organization;
    let hermes = common::hermes(&pool, organization).await;
    let (device, _) = common::device(&pool, organization, hermes, "active", Confirmed::Yes).await;
    let left_open = repo::connection::open(
        &pool,
        organization,
        device,
        None,
        &DeclaredText::lossy("0.1.0"),
        1,
        common::NOW,
    )
    .await
    .unwrap();
    let running = channel::start(
        &support::config(&installation, ClientIpSource::Socket),
        Options {
            settings: support::settings(),
            purposes: Served,
            downloads: channel::Downloads::empty(),
        },
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    let closed_at: Option<i64> =
        sqlx::query_scalar("SELECT closed_at FROM channel_connection WHERE id = $1")
            .bind(left_open.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(closed_at.is_some());
    running.stop().await;

    // A database that was never installed is not served.
    let empty = common::TestDb::own().await;
    let mut config = support::config(&installation, ClientIpSource::Socket);
    config.storage.database_url = empty.url();
    config.storage.data_dir = tempfile::tempdir().unwrap().keep().join("data");
    let refused = channel::start(&config, Options::default(), Arc::new(SystemClock)).await;
    assert!(
        matches!(refused, Err(channel::ServeError::NotInstalled)),
        "{refused:?}"
    );
}

fn free_port() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

/// `serve` is what `arauto-hub serve` runs: it holds until the request to stop and then stops
/// in order.
#[tokio::test(flavor = "multi_thread")]
async fn serve_runs_until_the_request_to_stop() {
    let installation = support::shared().await;
    // A free port is free when it is looked at: another hub of this process may bind it
    // before `serve` does. Then `serve` fails to listen, and the test tries other ports.
    for _ in 0..5 {
        let mut config = support::config(installation, ClientIpSource::Socket);
        config.listen = free_port();
        config.listen_internal = free_port();
        let internal = config.listen_internal;
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let serving = tokio::spawn(async move {
            channel::serve(
                &config,
                support::settings(),
                Arc::new(SystemClock),
                async move {
                    let _ = stopped.await;
                },
            )
            .await
        });
        let mut up = false;
        for _ in 0..400 {
            if serving.is_finished() {
                break;
            }
            if let Ok(answer) =
                arauto_testkit::http::get(internal, "/health", &[], Duration::from_secs(1)).await
                && answer.status == 200
                && answer.body
                    == b"ok
"
            {
                up = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        if !up {
            let served = tokio::time::timeout(PATIENCE, serving)
                .await
                .unwrap()
                .unwrap();
            assert!(
                matches!(served, Err(channel::ServeError::Listen { .. })),
                "{served:?}"
            );
            continue;
        }
        assert!(!serving.is_finished());
        stop.send(()).unwrap();
        let served = tokio::time::timeout(PATIENCE, serving)
            .await
            .unwrap()
            .unwrap();
        assert!(served.is_ok(), "{served:?}");
        return;
    }
    panic!("no free pair of ports in 5 tries");
}
