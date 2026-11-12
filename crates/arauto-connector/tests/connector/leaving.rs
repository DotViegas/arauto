//! `leave` and each of its cases (`docs/11-conector.md`, section 1; `docs/04-seguranca-conexao.md`,
//! C10).

use arauto_connector::catalog::Said;
use arauto_connector::testing::Event;
use arauto_connector::{ChannelStatus, Connector, DeviceStatus, Hermes, Left, leave};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::noise::StaticKeypair;
use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::support::{
    self, Folder, PATIENCE, approved, code_of, connected, eventually, join, refusal,
};

fn removed(registrations: &support::Registrations) -> bool {
    registrations
        .all()
        .iter()
        .any(|call| call == &["mcp", "remove", "arauto"])
}

/// The leader has the channel open: the request goes over it, and with the answer everything
/// is deleted and the registration removed. The leader stays alive.
#[tokio::test(flavor = "multi_thread")]
async fn leave_goes_through_the_leader_with_the_channel_open() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let device = folder.current().unwrap();
    let identity = folder.public(&device);
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert_eq!(
        left,
        Ok(Left::Done {
            identities: 1,
            unregistered: true
        })
    );
    assert!(folder.devices().is_empty());
    assert_eq!(folder.current(), None);
    assert!(!folder.root().join("state.json").exists());
    assert!(removed(&registrations));
    assert_eq!(fixture.state_of_key(&identity).await.0, "revoked");
    // The leader is still there, with nothing to serve.
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar203));
    assert_eq!(connector.status().await.device, None);
    connector.shutdown().await;
}

/// Two identities (a reconnection under way): each one is asked to close, and nothing is
/// deleted before both answers.
#[tokio::test(flavor = "multi_thread")]
async fn leave_asks_for_each_identity_of_the_folder() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (name, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    eventually("the request waits", PATIENCE, || async {
        matches!(
            connector.status().await.request,
            Some(DeviceStatus {
                channel: ChannelStatus::Waiting,
                ..
            })
        )
    })
    .await;
    let identities: Vec<_> = folder
        .devices()
        .iter()
        .map(|device| folder.public(device))
        .collect();
    assert_eq!(identities.len(), 2);
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert!(
        matches!(left, Ok(Left::Done { identities: 2, .. })),
        "{left:?}"
    );
    assert!(folder.devices().is_empty());
    for identity in &identities {
        let state = fixture.state_of_key(identity).await.0;
        assert!(state == "revoked" || state == "denied", "{state}");
    }
    connector.shutdown().await;
}

/// Only a pending request: the hub is asked to close it, it becomes denied, and it is deleted.
/// Without a leader, by a short connection.
#[tokio::test(flavor = "multi_thread")]
async fn leave_of_a_pending_request_denies_it() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, registrations) = support::settings();
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    let identity = folder.public(&folder.only_device());
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert!(
        matches!(left, Ok(Left::Done { identities: 1, .. })),
        "{left:?}"
    );
    assert!(folder.devices().is_empty());
    assert_eq!(fixture.state_of_key(&identity).await.0, "denied");
    assert!(removed(&registrations));
}

/// No channel: the connector is stopped, or the device is in quarantine. A short connection
/// proves the identity and asks.
#[tokio::test(flavor = "multi_thread")]
async fn leave_without_a_channel_uses_a_short_connection() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    connector.shutdown().await;
    let identity = folder.public(&folder.current().unwrap());
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert!(matches!(left, Ok(Left::Done { .. })), "{left:?}");
    assert!(folder.devices().is_empty());
    assert_eq!(fixture.state_of_key(&identity).await.0, "revoked");

    // In quarantine: the leader is alive and stopped using the channel; the request goes by a
    // short connection.
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let identity = folder.public(&folder.current().unwrap());
    let copy = folder.copy();
    connector.shutdown().await;
    // A copy is used, and then the true device comes back: reuse, quarantine.
    let (copy_settings, mut copy_watch, _) = support::settings();
    let copied = Connector::start(copy.home.clone(), copy_settings)
        .await
        .unwrap();
    copy_watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    copied.shutdown().await;
    watch.forget();
    let back = Connector::start(folder.home.clone(), s.clone())
        .await
        .unwrap();
    watch
        .until(PATIENCE, |event| {
            matches!(
                event,
                Event::Stopped {
                    code: ErrorCode::Ar501,
                    ..
                }
            )
        })
        .await;
    assert_eq!(refusal(&back).await, Some(ErrorCode::Ar501));
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert!(matches!(left, Ok(Left::Done { .. })), "{left:?}");
    assert!(folder.devices().is_empty());
    assert_eq!(fixture.state_of_key(&identity).await.0, "revoked");
    back.shutdown().await;
}

/// "Already closed" and "unknown" count as a confirmation.
#[tokio::test(flavor = "multi_thread")]
async fn already_closed_and_unknown_count_as_a_confirmation() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, _registrations) = support::settings();
    // Revoked by the admin: already closed.
    let (name, folder) = approved(&fixture, &s).await;
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert!(matches!(left, Ok(Left::Done { .. })), "{left:?}");
    assert!(folder.devices().is_empty());

    // An identity the hub does not know.
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    let device = folder.only_device();
    let stranger = StaticKeypair::generate().unwrap();
    std::fs::write(
        folder.device_path(&device).join("identity.key"),
        stranger.private().expose(),
    )
    .unwrap();
    assert!(!fixture.knows(stranger.public()).await);
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert!(matches!(left, Ok(Left::Done { .. })), "{left:?}");
    assert!(folder.devices().is_empty());
}

/// The hub does not answer, or is not the hub of the stored key: the identity is not deleted,
/// AR-505. Running it again tries again.
#[tokio::test(flavor = "multi_thread")]
async fn a_hub_that_does_not_answer_keeps_the_identity() {
    let mut fixture = Fixture::new(settings()).await;
    let (s, _watch, registrations) = support::settings();
    let (_, folder) = approved(&fixture, &s).await;
    let device = folder.only_device();

    // Not the hub of the stored key.
    let original = std::fs::read(folder.device_path(&device).join("hub.json")).unwrap();
    let mut changed = folder.hub_json(&device);
    changed["hub_key"] =
        serde_json::Value::from(StaticKeypair::generate().unwrap().public().to_text());
    std::fs::write(
        folder.device_path(&device).join("hub.json"),
        serde_json::to_vec(&changed).unwrap(),
    )
    .unwrap();
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert_eq!(left, Err(Said::code(ErrorCode::Ar505)));
    assert_eq!(folder.devices(), vec![device.clone()]);
    std::fs::write(folder.device_path(&device).join("hub.json"), original).unwrap();

    // No hub at all, with no leader running.
    fixture.stop().await;
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert_eq!(left, Err(Said::code(ErrorCode::Ar505)));
    assert_eq!(folder.devices(), vec![device.clone()]);
    assert!(folder.device_path(&device).join("identity.key").exists());

    // No hub, with a leader running: the request goes through it, and nothing is deleted.
    let leader = Connector::start(folder.home.clone(), s.clone())
        .await
        .unwrap();
    let left = leave(&folder.home, false, &Hermes::default(), &s).await;
    assert_eq!(left, Err(Said::code(ErrorCode::Ar505)));
    assert_eq!(folder.devices(), vec![device.clone()]);
    assert!(folder.device_path(&device).join("identity.key").exists());
    assert!(!removed(&registrations));
    leader.shutdown().await;
}

/// `leave --local`: the files and the registration go without the hub; the device still counts
/// on the hub.
#[tokio::test(flavor = "multi_thread")]
async fn the_local_leave_deletes_without_the_hub() {
    let mut fixture = Fixture::new(settings()).await;
    let (s, mut watch, registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    connector.shutdown().await;
    let identity = folder.public(&folder.current().unwrap());
    fixture.stop().await;
    let left = leave(&folder.home, true, &Hermes::default(), &s).await;
    assert_eq!(
        left,
        Ok(Left::Done {
            identities: 0,
            unregistered: true
        })
    );
    assert!(folder.devices().is_empty());
    assert_eq!(folder.current(), None);
    assert!(removed(&registrations));
    fixture.start().await;
    assert_eq!(fixture.state_of_key(&identity).await.0, "active");
}
