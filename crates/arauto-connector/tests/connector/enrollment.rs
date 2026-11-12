//! `join` and each of its cases (`docs/11-conector.md`, section 1; `docs/04-seguranca-conexao.md`,
//! C3 to C6 and C10), with the hub of a fixture and the admin of the same database.

use std::sync::Arc;

use arauto_connector::catalog::{Local, Said};
use arauto_connector::testing::Event;
use arauto_connector::{ChannelStatus, Connector, DeviceStatus, Joined, Role};
use arauto_protocol::clock::{ManualClock, SystemClock};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::limits::MINUTE_MS;
use arauto_protocol::noise::StaticKeypair;
use arauto_testkit::device::{self, Enrolled};
use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::support::{
    self, Folder, PATIENCE, approve, code_of, connected, eventually, join, join_with, refusal,
    status_works,
};

/// The whole way in: the invite, the code shown, the wait for the admin, the code typed, the
/// first swap and the session.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_enters_with_the_invite_and_the_code_and_reaches_the_session() {
    let fixture = Fixture::new(settings()).await;
    let (connector_settings, mut watch, registrations) = support::settings();
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;

    let joined = join(&fixture, &folder, &invite, &connector_settings).await;
    let Ok(Joined::Enrolled {
        code,
        minutes,
        replaces,
        registered,
    }) = joined
    else {
        panic!("{joined:?}");
    };
    assert!(!replaces && registered);
    assert!((1..=10).contains(&minutes), "{minutes}");
    // The official command of Hermes, with the folder, which is not the default one.
    let program = folder.home.program();
    assert_eq!(
        registrations.all(),
        vec![vec![
            "mcp".to_owned(),
            "add".to_owned(),
            "arauto".to_owned(),
            "--command".to_owned(),
            program.to_string_lossy().into_owned(),
            "--args".to_owned(),
            "mcp".to_owned(),
            "--home".to_owned(),
            folder.root().to_string_lossy().into_owned(),
        ]]
    );
    // The program copied itself to its fixed place.
    assert_eq!(std::fs::read(&program).unwrap(), b"a program for the tests");
    // One request, and no device in use yet.
    let request = folder.only_device();
    assert_eq!(folder.current(), None);
    assert_eq!(
        fixture.state_of_key(&folder.public(&request)).await.0,
        "pending"
    );

    let connector = Connector::start(folder.home.clone(), connector_settings)
        .await
        .unwrap();
    assert_eq!(connector.role(), Role::Leader);
    eventually("the request waits for the admin", PATIENCE, || async {
        matches!(
            connector.status().await.request,
            Some(DeviceStatus {
                channel: ChannelStatus::Waiting,
                ..
            })
        )
    })
    .await;
    // Before the session the calls do not go anywhere.
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar203));

    approve(&fixture, &name, &code.to_string()).await;
    watch
        .until(
            PATIENCE,
            |event| matches!(event, Event::Takeover { device } if *device == request),
        )
        .await;
    assert_eq!(folder.current().as_deref(), Some(request.as_str()));
    assert!(status_works(&connector).await);
    assert_eq!(
        fixture.state_of_key(&folder.public(&request)).await.0,
        "active"
    );
    assert_eq!(folder.hub_json(&request)["situation"], "active");
    assert!(folder.credential(&request).is_some());
    let tools: Vec<String> = connector
        .tools()
        .iter()
        .map(|tool| tool.as_str().to_owned())
        .collect();
    assert_eq!(tools, ["arauto_status"]);
    let status = connector.status().await;
    assert!(
        matches!(
            &status.device,
            Some(DeviceStatus {
                channel: ChannelStatus::Session { hermes, .. },
                ..
            }) if hermes.as_str() == name
        ),
        "{status:?}"
    );
    assert_eq!(status.request, None);
    connector.shutdown().await;
}

/// The same invite again: pending, the same request with a new code; approved, AR-104 and
/// nothing changes; denied or expired, the folder goes only while it has no credential.
#[tokio::test(flavor = "multi_thread")]
async fn the_same_invite_again_finds_the_request_where_it_is() {
    let clock = ManualClock::new(arauto_protocol::clock::Clock::now_ms(&SystemClock));
    let fixture = Fixture::with_clock(settings(), Arc::new(clock.clone())).await;
    let (s, _watch, registrations) = support::settings();

    // Pending: the same folder, the same identity, a code that replaces the first.
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    let device = folder.only_device();
    let identity = folder.public(&device);
    let again = join(&fixture, &folder, &invite, &s).await;
    let Ok(Joined::Enrolled {
        code,
        replaces: true,
        ..
    }) = again
    else {
        panic!("{again:?}");
    };
    assert_eq!(folder.only_device(), device);
    assert_eq!(folder.public(&device), identity);

    // Approved: AR-104, and nothing changes.
    approve(&fixture, &name, &code.to_string()).await;
    let calls = registrations.all().len();
    let approved = join(&fixture, &folder, &invite, &s).await;
    assert_eq!(approved, Ok(Joined::AlreadyApproved { registered: true }));
    assert_eq!(folder.only_device(), device);
    // Registered already: the registration is not made again.
    assert_eq!(registrations.all().len(), calls);

    // Denied, without a credential: the folder of that request goes.
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    fixture.admin.deny(&name, fixture.now()).await.unwrap();
    let denied = join(&fixture, &folder, &invite, &s).await;
    assert_eq!(denied, Err(Said::code(ErrorCode::Ar301)));
    assert!(folder.devices().is_empty());

    // Denied, with a credential: nothing is discarded, and the error is shown.
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    let device = folder.only_device();
    std::fs::write(
        folder.device_path(&device).join("credential.seal"),
        [7u8; 64],
    )
    .unwrap();
    fixture.admin.deny(&name, fixture.now()).await.unwrap();
    let denied = join(&fixture, &folder, &invite, &s).await;
    assert_eq!(denied, Err(Said::code(ErrorCode::Ar301)));
    assert_eq!(folder.devices(), vec![device]);

    // Expired without the code: AR-302, and the folder without a credential goes.
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    clock.advance(11 * MINUTE_MS);
    let expired = join(&fixture, &folder, &invite, &s).await;
    assert_eq!(expired, Err(Said::code(ErrorCode::Ar302)));
    assert!(folder.devices().is_empty());

    // Approved and expired before the first credential was confirmed: AR-303. Without a
    // credential the folder goes; with one, it stays.
    let mut approved_requests = Vec::new();
    for planted in [false, true] {
        let name = fixture.hermes().await;
        let folder = Folder::new();
        let invite = fixture.invite(&name).await;
        let code = code_of(join(&fixture, &folder, &invite, &s).await);
        approve(&fixture, &name, &code).await;
        let device = folder.only_device();
        if planted {
            std::fs::write(
                folder.device_path(&device).join("credential.seal"),
                [9u8; 64],
            )
            .unwrap();
        }
        approved_requests.push((planted, folder, invite, device));
    }
    clock.advance(arauto_protocol::limits::DAY_MS + MINUTE_MS);
    for (planted, folder, invite, device) in approved_requests {
        // The sweep of the hub applies the deadline of an approved request.
        let identity = folder.public(&device);
        eventually("the approved request expires", PATIENCE, || async {
            fixture.state_of_key(&identity).await.0 == "expired"
        })
        .await;
        let expired = join(&fixture, &folder, &invite, &s).await;
        assert_eq!(expired, Err(Said::code(ErrorCode::Ar303)), "{planted}");
        if planted {
            assert_eq!(folder.devices(), vec![device]);
        } else {
            assert!(folder.devices().is_empty());
        }
    }
}

/// AR-101 and AR-102: the folder made for the request goes, and nothing else changes.
#[tokio::test(flavor = "multi_thread")]
async fn an_invite_the_hub_refuses_leaves_nothing_behind() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, registrations) = support::settings();
    let folder = Folder::new();

    // Well formed, issued by nobody.
    let nobody = Invite::generate().unwrap();
    assert_eq!(
        join(&fixture, &folder, &nobody, &s).await,
        Err(Said::code(ErrorCode::Ar101))
    );
    assert!(folder.devices().is_empty());
    // Not an invite at all: refused here, before the hub.
    let malformed = join_with(
        &folder,
        &fixture.channel_url(),
        &fixture.hub_key_text(),
        "arc1-this-is-not-an-invite",
        &s,
    )
    .await;
    assert_eq!(malformed, Err(Said::code(ErrorCode::Ar101)));

    // Used by another device first.
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let other = fixture.device();
    let enrolled = other.enroll(&invite, device::declared()).await.unwrap();
    assert!(matches!(enrolled, Enrolled::Code(_)), "{enrolled:?}");
    assert_eq!(
        join(&fixture, &folder, &invite, &s).await,
        Err(Said::code(ErrorCode::Ar102))
    );
    assert!(folder.devices().is_empty());
    // The first request is untouched, and no registration was made.
    assert_eq!(fixture.state_of(&other).await.0, "pending");
    assert!(registrations.all().is_empty());
}

/// A new invite with an old request without a credential: the old one goes, a new identity.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_invite_replaces_an_old_request_without_a_credential() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, _registrations) = support::settings();
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let first = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &first, &s).await);
    let old = folder.only_device();
    let old_identity = folder.public(&old);
    // A new invite for the same Hermes closes the request in progress on the hub.
    let second = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &second, &s).await);
    let new = folder.only_device();
    assert_ne!(new, old);
    assert_ne!(folder.public(&new), old_identity);
    assert_eq!(
        fixture.state_of_key(&folder.public(&new)).await.0,
        "pending"
    );
    assert_ne!(fixture.state_of_key(&old_identity).await.0, "pending");
}

/// AR-601: the request stays open on the hub, and the same join registers again.
#[tokio::test(flavor = "multi_thread")]
async fn a_registration_that_fails_is_ar601_and_the_same_join_registers_again() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, registrations) = support::settings();
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    registrations.refusing(true);
    assert_eq!(
        join(&fixture, &folder, &invite, &s).await,
        Err(Said::code(ErrorCode::Ar601))
    );
    let device = folder.only_device();
    assert_eq!(
        fixture.state_of_key(&folder.public(&device)).await.0,
        "pending"
    );
    registrations.refusing(false);
    let again = join(&fixture, &folder, &invite, &s).await;
    assert!(
        matches!(
            again,
            Ok(Joined::Enrolled {
                registered: true,
                replaces: true,
                ..
            })
        ),
        "{again:?}"
    );
    assert_eq!(folder.only_device(), device);
    assert_eq!(registrations.all().len(), 2);
}

/// AR-105: a device of another hub, by another key or another address. Nothing changes.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_of_another_hub_is_refused_and_nothing_changes() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    connector.shutdown().await;
    let devices = folder.devices();
    let current = folder.current();
    let other_key = StaticKeypair::generate().unwrap().public().to_text();
    let invite = Invite::generate().unwrap();
    let other_address = format!("ws://localhost:{}/v1/channel", fixture.addr().port());
    for (hub, key) in [
        (fixture.channel_url(), other_key.clone()),
        (other_address, fixture.hub_key_text()),
    ] {
        let refused = join_with(&folder, &hub, &key, &invite.reveal(), &s).await;
        assert_eq!(refused, Err(Said::code(ErrorCode::Ar105)));
        assert_eq!(folder.devices(), devices);
        assert_eq!(folder.current(), current);
    }
}

/// AR-202: a server without the key of the block does not complete the handshake.
#[tokio::test(flavor = "multi_thread")]
async fn a_hub_of_another_key_is_ar202_and_nothing_is_left() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, _registrations) = support::settings();
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let folder = Folder::new();
    let other_key = StaticKeypair::generate().unwrap().public().to_text();
    let refused = join_with(
        &folder,
        &fixture.channel_url(),
        &other_key,
        &invite.reveal(),
        &s,
    )
    .await;
    assert_eq!(refused, Err(Said::code(ErrorCode::Ar202)));
    assert!(folder.devices().is_empty());
    // The invite was never presented: the right block still works.
    code_of(join(&fixture, &folder, &invite, &s).await);
}

/// `ws://` only for this machine: refused before anything is created.
#[tokio::test(flavor = "multi_thread")]
async fn a_plain_address_to_another_machine_is_refused_before_anything() {
    let (s, _watch, _registrations) = support::settings();
    let folder = Folder::new();
    let key = StaticKeypair::generate().unwrap().public().to_text();
    let invite = Invite::generate().unwrap();
    for hub in [
        "ws://192.0.2.1:9/v1/channel",
        "ws://hub.example.com/v1/channel",
        "ws://[2001:db8::1]:9/v1/channel",
    ] {
        assert_eq!(
            join_with(&folder, hub, &key, &invite.reveal(), &s).await,
            Err(Said::Local(Local::PlainNotLocal)),
            "{hub}"
        );
    }
    assert_eq!(
        join_with(
            &folder,
            "https://hub.example.com",
            &key,
            &invite.reveal(),
            &s
        )
        .await,
        Err(Said::Local(Local::HubAddress))
    );
    assert_eq!(
        join_with(
            &folder,
            "ws://127.0.0.1:9/v1/channel",
            "arh1notakey",
            &invite.reveal(),
            &s
        )
        .await,
        Err(Said::Local(Local::HubKey))
    );
    assert!(!folder.root().exists());
}

/// Reconnecting the same Hermes: the device in use goes on until the new one has its first
/// credential confirmed; then the hub replaces it and its folder is deleted.
#[tokio::test(flavor = "multi_thread")]
async fn reconnecting_keeps_the_device_until_the_new_one_takes_over() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (name, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let old = folder.current().unwrap();
    let old_identity = folder.public(&old);

    let invite = fixture.invite(&name).await;
    let code = code_of(join(&fixture, &folder, &invite, &s).await);
    let new = folder
        .devices()
        .into_iter()
        .find(|device| *device != old)
        .unwrap();
    eventually("the new request is followed apart", PATIENCE, || async {
        matches!(
            connector.status().await.request,
            Some(DeviceStatus {
                channel: ChannelStatus::Waiting,
                ..
            })
        )
    })
    .await;
    // The device in use still serves.
    assert!(status_works(&connector).await);
    assert_eq!(folder.current().as_deref(), Some(old.as_str()));

    approve(&fixture, &name, &code).await;
    watch
        .until(
            PATIENCE,
            |event| matches!(event, Event::Takeover { device } if *device == new),
        )
        .await;
    eventually("the folder of the old device goes", PATIENCE, || async {
        !folder.devices().contains(&old)
    })
    .await;
    assert_eq!(folder.current().as_deref(), Some(new.as_str()));
    assert!(status_works(&connector).await);
    assert_eq!(fixture.state_of_key(&old_identity).await.0, "replaced");
    assert_eq!(fixture.state_of_key(&folder.public(&new)).await.0, "active");
    connector.shutdown().await;
}

/// An invite of another line of the panel on a connected Hermes: one machine, one device. Once
/// the new device takes over, the connector asks the hub to close the old one, which the hub
/// did not replace, and deletes its folder only after the answer.
#[tokio::test(flavor = "multi_thread")]
async fn an_invite_of_another_line_closes_the_old_device_after_the_new_one_takes_over() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let old = folder.current().unwrap();
    let old_identity = folder.public(&old);

    let other = fixture.hermes().await;
    let invite = fixture.invite(&other).await;
    let code = code_of(join(&fixture, &folder, &invite, &s).await);
    let new = folder
        .devices()
        .into_iter()
        .find(|device| *device != old)
        .unwrap();
    // The old device serves until then.
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
    assert!(status_works(&connector).await);
    approve(&fixture, &other, &code).await;
    watch
        .until(
            PATIENCE,
            |event| matches!(event, Event::Takeover { device } if *device == new),
        )
        .await;
    watch
        .until(
            PATIENCE,
            |event| matches!(event, Event::Deleted { device } if *device == old),
        )
        .await;
    assert_eq!(folder.devices(), vec![new.clone()]);
    // Closed at its own request, and the admin is told.
    assert_eq!(fixture.state_of_key(&old_identity).await.0, "revoked");
    assert!(status_works(&connector).await);
    connector.shutdown().await;
}

/// Without the `tls` feature a hub over TLS cannot be reached: refused before anything is
/// created.
#[cfg(not(feature = "tls"))]
#[tokio::test(flavor = "multi_thread")]
async fn without_tls_a_hub_over_tls_is_refused_before_anything() {
    let (s, _watch, _registrations) = support::settings();
    let folder = Folder::new();
    let key = StaticKeypair::generate().unwrap().public().to_text();
    let invite = Invite::generate().unwrap();
    assert_eq!(
        join_with(
            &folder,
            "wss://hub.example.com/v1/channel",
            &key,
            &invite.reveal(),
            &s
        )
        .await,
        Err(Said::Local(Local::NoTls))
    );
    assert!(!folder.root().exists());
}

/// Over TLS the certificate is always checked: a server of this machine that does not speak
/// TLS with a certificate of a public authority is not reached, and nothing is left behind.
#[cfg(feature = "tls")]
#[tokio::test(flavor = "multi_thread")]
async fn over_tls_a_server_without_a_valid_certificate_is_not_reached() {
    use tokio::io::AsyncWriteExt;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n\r\nnot a hub").await;
        }
    });
    let (s, _watch, _registrations) = support::settings();
    let folder = Folder::new();
    let key = StaticKeypair::generate().unwrap().public().to_text();
    let invite = Invite::generate().unwrap();
    let hub = format!("wss://localhost:{port}/v1/channel");
    let joined = join_with(&folder, &hub, &key, &invite.reveal(), &s).await;
    assert_eq!(
        joined,
        Err(Said::Unreachable(
            arauto_protocol::errors::ConnectFailure::Refused
        ))
    );
    assert!(folder.devices().is_empty());
}
