//! The ceiling of connections without a session (`docs/14-protocolo-do-canal.md`, section 4):
//! 503 above the one of the hub, 429 above the one of the origin, both before the websocket
//! opens; the waiting ones and the sessions do not count; the origin is read as
//! `ARAUTO_CLIENT_IP_SOURCE` says.

use std::time::Duration;

use arauto_hub::channel::Downloads;
use arauto_hub::config::{ClientIpSource, Network};
use arauto_protocol::message::{Message, Purpose};
use arauto_testkit::connector::{TestConnector, identity};
use arauto_testkit::http;

use crate::support::{self, Behavior, TestHub, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(10);

fn statuses(upgrades: &[http::Upgrade]) -> Vec<u16> {
    upgrades.iter().map(|upgrade| upgrade.status).collect()
}

fn ceilings(hub: usize, origin: usize) -> arauto_hub::channel::ChannelSettings {
    let mut settings = support::settings();
    settings.without_session_max = hub;
    settings.without_session_per_origin_max = origin;
    settings
}

async fn forwarded_hub(hub_max: usize, origin_max: usize) -> TestHub {
    support::start_with(
        ceilings(hub_max, origin_max),
        TestPurposes(Behavior::Park),
        ClientIpSource::Forwarded {
            trusted_proxies: vec![Network::parse("127.0.0.1").unwrap()],
        },
        Downloads::empty(),
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn above_the_ceiling_of_an_origin_the_answer_is_429_and_a_place_given_back_is_free() {
    let hub = support::start(ceilings(100, 2), TestPurposes(Behavior::Park)).await;
    let mut upgrades = hub.hostile().many_connections(3, None).await.unwrap();
    assert_eq!(statuses(&upgrades), [101, 101, 429]);
    // A connection that ends gives its place back.
    drop(upgrades.remove(0));
    let mut freed = false;
    for _ in 0..100 {
        let upgrade = http::upgrade(hub.addr(), &[], PATIENCE).await.unwrap();
        if upgrade.status == 101 {
            freed = true;
            break;
        }
        assert_eq!(upgrade.status, 429);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(freed);
}

#[tokio::test(flavor = "multi_thread")]
async fn above_the_ceiling_of_the_hub_the_answer_is_503() {
    let hub = forwarded_hub(3, 16).await;
    let mut all = Vec::new();
    for last in 1..=4 {
        let address = format!("192.0.2.{last}");
        let upgrade = hub
            .hostile()
            .many_connections(1, Some(&address))
            .await
            .unwrap();
        all.extend(upgrade);
    }
    assert_eq!(statuses(&all), [101, 101, 101, 503]);
}

/// Behind a trusted proxy, the origin is the address it forwards; from anywhere else, the
/// header is ignored and the socket is the origin.
#[tokio::test(flavor = "multi_thread")]
async fn the_origin_is_the_forwarded_address_only_behind_a_trusted_proxy() {
    let hub = forwarded_hub(100, 2).await;
    let same = hub
        .hostile()
        .many_connections(3, Some("198.51.100.7"))
        .await
        .unwrap();
    assert_eq!(statuses(&same), [101, 101, 429]);
    let other = hub
        .hostile()
        .many_connections(1, Some("198.51.100.8"))
        .await
        .unwrap();
    assert_eq!(statuses(&other), [101]);
    // A trusted proxy that does not say who the client is.
    let unsaid = http::upgrade(hub.addr(), &[], PATIENCE).await.unwrap();
    assert_eq!(unsaid.status, 400);

    // With `socket`, the same header is the client's word and changes nothing.
    let direct = support::start(ceilings(100, 2), TestPurposes(Behavior::Park)).await;
    let mut kept = Vec::new();
    for last in 1..=3 {
        let address = format!("203.0.113.{last}");
        let upgrade = http::upgrade(direct.addr(), &[("X-Forwarded-For", &address)], PATIENCE)
            .await
            .unwrap();
        kept.push(upgrade);
    }
    assert_eq!(statuses(&kept), [101, 101, 429]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_waiting_connections_and_the_sessions_do_not_count() {
    for behavior in [Behavior::Wait { ms: 60_000 }, Behavior::Session] {
        let hub = support::start(ceilings(100, 2), TestPurposes(behavior.clone())).await;
        let mut kept = Vec::new();
        for _ in 0..2 {
            let mut connector =
                TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
                    .await
                    .unwrap();
            connector.hello(Purpose::Channel).await.unwrap();
            if matches!(behavior, Behavior::Session) {
                assert!(matches!(
                    connector.recv(PATIENCE).await.unwrap(),
                    Message::Session(_)
                ));
            }
            kept.push(connector);
        }
        // The waiting ones may have left the count only just now.
        let mut upgrades = Vec::new();
        for _ in 0..200 {
            let upgrade = http::upgrade(hub.addr(), &[], PATIENCE).await.unwrap();
            if upgrade.status == 101 {
                upgrades.push(upgrade);
                if upgrades.len() == 2 {
                    break;
                }
            } else {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        assert_eq!(upgrades.len(), 2, "{behavior:?}");
        let third = http::upgrade(hub.addr(), &[], PATIENCE).await.unwrap();
        assert_eq!(third.status, 429, "{behavior:?}");
        drop(kept);
    }
}
