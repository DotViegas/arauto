//! The channel kept by the leader (`docs/14-protocolo-do-canal.md`, sections 4 and 7;
//! `docs/04-seguranca-conexao.md`, C9 and C10): the waits before a new attempt, quarantine and
//! revocation without the connector dying, the copies of the folder found by the hub, and the
//! leader with the copies that forward to it.

use arauto_connector::testing::Event;
use arauto_connector::{ChannelStatus, Connector, DeviceStatus, Hermes, JoinArgs, Joined, Role};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::noise::PublicKey;
use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::support::{
    self, Folder, PATIENCE, approve, code_of, connected, eventually, join, program, refusal,
    status_works,
};

/// Ends the connections of `identity` from the side of the hub: with `closing` and the code,
/// or without a word.
fn close_links(fixture: &Fixture, identity: &PublicKey, code: Option<ErrorCode>) {
    for link in fixture.hub().links() {
        if link.info().identity == *identity {
            match code {
                Some(code) => link.close(code),
                None => link.close_silently(),
            }
        }
    }
}

fn wait_of(event: &Event) -> Option<u64> {
    match event {
        Event::Waiting { ms, .. } => Some(*ms),
        _ => None,
    }
}

async fn next_wait(watch: &mut support::Watch) -> u64 {
    let event = watch
        .until(PATIENCE, |event| matches!(event, Event::Waiting { .. }))
        .await;
    watch.forget();
    wait_of(&event).unwrap()
}

async fn back_in_session(watch: &mut support::Watch) {
    watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    watch.forget();
}

/// After a drop the first attempt is at once (C9), and AR-208 too; AR-205 and AR-206 wait in
/// their ranges; failures in a row wait a time drawn under a ceiling that doubles (the
/// settings of the tests: from 50 to 400 ms).
#[tokio::test(flavor = "multi_thread")]
async fn the_waits_before_a_new_attempt_follow_the_table() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let seed = 0x5eed_0003_u64;
    println!("seed of the waits: {seed}");
    let (_, folder, connector) = connected(&fixture, s.with_seed(seed), &mut watch).await;
    let device = folder.current().unwrap();
    let identity = folder.public(&device);
    watch.forget();

    close_links(&fixture, &identity, Some(ErrorCode::Ar208));
    assert_eq!(next_wait(&mut watch).await, 0, "AR-208 is at once");
    back_in_session(&mut watch).await;

    close_links(&fixture, &identity, None);
    assert_eq!(
        next_wait(&mut watch).await,
        0,
        "a dropped session comes back at once"
    );
    back_in_session(&mut watch).await;

    close_links(&fixture, &identity, Some(ErrorCode::Ar205));
    let wait = next_wait(&mut watch).await;
    assert!((50..=200).contains(&wait), "AR-205 waited {wait}");
    back_in_session(&mut watch).await;

    close_links(&fixture, &identity, Some(ErrorCode::Ar206));
    let wait = next_wait(&mut watch).await;
    assert!((50..=200).contains(&wait), "AR-206 waited {wait}");
    back_in_session(&mut watch).await;

    // Nothing listens where the hub was: failures in a row.
    let path = folder.device_path(&device).join("hub.json");
    let original = std::fs::read(&path).unwrap();
    let mut elsewhere = folder.hub_json(&device);
    elsewhere["hub"] = serde_json::Value::from("ws://127.0.0.1:1/v1/channel");
    std::fs::write(&path, serde_json::to_vec(&elsewhere).unwrap()).unwrap();
    close_links(&fixture, &identity, None);
    assert_eq!(next_wait(&mut watch).await, 0);
    let mut waits = Vec::new();
    for ceiling in [50u64, 100, 200, 400, 400, 400] {
        let wait = next_wait(&mut watch).await;
        assert!(wait <= ceiling, "{wait} above {ceiling} in {waits:?}");
        waits.push(wait);
    }
    // Drawn under the ceiling, not the ceiling itself.
    let below = waits
        .iter()
        .zip([50u64, 100, 200, 400, 400, 400])
        .filter(|(wait, ceiling)| *wait < ceiling)
        .count();
    assert!(below >= 3, "the waits are drawn: {waits:?}");
    // While the channel is down the calls get AR-203.
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar203));
    std::fs::write(&path, original).unwrap();
    back_in_session(&mut watch).await;
    assert!(status_works(&connector).await);
    connector.shutdown().await;
}

/// Revoked, and in quarantine: the connector stops using the channel and stays alive,
/// answering every call with the code, and serving the local socket. Nothing is deleted.
#[tokio::test(flavor = "multi_thread")]
async fn quarantine_and_revocation_keep_the_connector_alive_with_the_error() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (name, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let device = folder.current().unwrap();
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    watch
        .until(PATIENCE, |event| {
            matches!(
                event,
                Event::Stopped {
                    code: ErrorCode::Ar502,
                    ..
                }
            )
        })
        .await;
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar502));
    eventually("the status says revoked", PATIENCE, || async {
        matches!(
            connector.status().await.device,
            Some(DeviceStatus {
                channel: ChannelStatus::Stopped(Some(ErrorCode::Ar502)),
                ..
            })
        )
    })
    .await;
    // The local socket still answers.
    let report = arauto_connector::report(&folder.home, &s).await;
    assert!(report.leader.is_some(), "{report:?}");
    assert_eq!(folder.devices(), vec![device.clone()]);
    assert!(folder.credential(&device).is_some());
    assert_eq!(folder.hub_json(&device)["code"], "AR-502");
    connector.shutdown().await;

    // A credential that does not open: quarantine for an unreadable credential.
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s, &mut watch).await;
    let device = folder.current().unwrap();
    let identity = folder.public(&device);
    let garbage = vec![0x5au8; folder.credential(&device).unwrap().len()];
    std::fs::write(
        folder.device_path(&device).join("credential.seal"),
        &garbage,
    )
    .unwrap();
    close_links(&fixture, &identity, None);
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
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar501));
    assert_eq!(fixture.state_of_key(&identity).await.0, "quarantined");
    assert_eq!(folder.credential(&device), Some(garbage));
    assert_eq!(connector.role(), Role::Leader);
    connector.shutdown().await;
}

/// A copy of the folder used by another process is found: with the true one on, as a double
/// connection; with the true one off, when it comes back.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_of_the_folder_is_found() {
    let fixture = Fixture::new(settings()).await;

    // The true one on.
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s, &mut watch).await;
    let identity = folder.public(&folder.current().unwrap());
    let copy = folder.copy();
    watch.forget();
    let (copy_settings, mut copy_watch, _) = support::settings();
    let copied = Connector::start(copy.home.clone(), copy_settings)
        .await
        .unwrap();
    copy_watch
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
    assert_eq!(fixture.state_of_key(&identity).await.0, "quarantined");
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar501));
    // A double connection: the true one answered the test of the hub in its session, so it
    // was never given up for dead (AR-208, a connection taken), and the copy never had a
    // session.
    let ends: Vec<_> = watch
        .drain()
        .iter()
        .filter_map(|event| match event {
            Event::AttemptEnded {
                closing,
                established,
                ..
            } => Some((*closing, *established)),
            _ => None,
        })
        .collect();
    assert_eq!(
        ends.first(),
        Some(&(Some(ErrorCode::Ar501), true)),
        "{:?}",
        watch.seen
    );
    assert_eq!(
        fixture.alerts_of_key("connection_taken", &identity).await,
        0
    );
    assert!(
        !copy_watch
            .drain()
            .iter()
            .any(|event| matches!(event, Event::Session { .. })),
        "{:?}",
        copy_watch.seen
    );
    copied.shutdown().await;
    connector.shutdown().await;

    // The true one off: the copy works, and is found when the true one comes back.
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let identity = folder.public(&folder.current().unwrap());
    connector.shutdown().await;
    let copy = folder.copy();
    let (copy_settings, mut copy_watch, _) = support::settings();
    let copied = Connector::start(copy.home.clone(), copy_settings)
        .await
        .unwrap();
    copy_watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    assert!(status_works(&copied).await);
    copied.shutdown().await;
    watch.forget();
    let back = Connector::start(folder.home.clone(), s).await.unwrap();
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
    assert_eq!(fixture.state_of_key(&identity).await.0, "quarantined");
    back.shutdown().await;
}

/// One leader per folder: a second copy forwards to it, and takes its place when it ends. The
/// hub never sees two channel connections of the device.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_copy_forwards_to_the_leader_and_takes_its_place() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, leader) = connected(&fixture, s, &mut watch).await;
    let identity = folder.public(&folder.current().unwrap());

    let (relay_settings, mut relay_watch, _) = support::settings();
    let relay = Connector::start(folder.home.clone(), relay_settings)
        .await
        .unwrap();
    assert_eq!(relay.role(), Role::Relay);
    assert!(status_works(&relay).await);
    let status = relay.status().await;
    assert_eq!(status.role, Role::Relay);
    assert_eq!(status.leader_process, Some(std::process::id()));
    eventually("the relay knows the tools", PATIENCE, || async {
        relay
            .tools()
            .iter()
            .any(|tool| tool.as_str() == "arauto_status")
    })
    .await;
    assert_eq!(fixture.links_of(&identity), 1);

    leader.shutdown().await;
    relay_watch
        .until(PATIENCE, |event| {
            matches!(event, Event::Role { leader: true })
        })
        .await;
    relay_watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    assert_eq!(relay.role(), Role::Leader);
    assert!(status_works(&relay).await);
    eventually("one channel connection", PATIENCE, || async {
        fixture.links_of(&identity) == 1
    })
    .await;
    assert_eq!(fixture.state_of_key(&identity).await.0, "active");
    relay.shutdown().await;
}

/// Through a copy that forwards, a call without a session gets AR-203 from the leader.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_through_a_copy_without_a_session_is_ar203() {
    let fixture = Fixture::new(settings()).await;
    let (s, _watch, _registrations) = support::settings();
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    code_of(join(&fixture, &folder, &invite, &s).await);
    let leader = Connector::start(folder.home.clone(), s.clone())
        .await
        .unwrap();
    let relay = Connector::start(folder.home.clone(), s).await.unwrap();
    assert_eq!(relay.role(), Role::Relay);
    assert_eq!(refusal(&relay).await, Some(ErrorCode::Ar203));
    relay.shutdown().await;
    leader.shutdown().await;
}

/// A `join` that changed the program asks the leader to end, for Hermes to start the new
/// version.
#[tokio::test(flavor = "multi_thread")]
async fn a_join_with_a_new_program_asks_the_leader_to_end() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (name, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    let invite = fixture.invite(&name).await;
    let newer = program();
    std::fs::write(&newer, b"a newer program for the tests").unwrap();
    let args = arauto_connector::JoinArgs {
        hub: fixture.channel_url(),
        key: fixture.hub_key_text(),
        invite: zeroize::Zeroizing::new(invite.reveal().to_string()),
        register: false,
        program: newer,
    };
    let joined = arauto_connector::join(
        &folder.home,
        args,
        &arauto_connector::Hermes::default(),
        &s,
        support::now(),
    )
    .await;
    assert!(joined.is_ok(), "{joined:?}");
    assert_eq!(
        std::fs::read(folder.home.program()).unwrap(),
        b"a newer program for the tests"
    );
    tokio::time::timeout(PATIENCE, connector.finished())
        .await
        .expect("the leader was asked to end");
    connector.shutdown().await;
}

/// The same when the registration is made late, on AR-104: the program it names is copied
/// then, and a new one asks the leader to end.
#[tokio::test(flavor = "multi_thread")]
async fn a_late_registration_with_a_new_program_asks_the_leader_to_end() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, registrations) = support::settings();
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    let args = |register: bool, program: std::path::PathBuf| JoinArgs {
        hub: fixture.channel_url(),
        key: fixture.hub_key_text(),
        invite: zeroize::Zeroizing::new(invite.reveal().to_string()),
        register,
        program,
    };
    // Joined without the registration, approved, and the leader in session.
    let joined = arauto_connector::join(
        &folder.home,
        args(false, program()),
        &Hermes::default(),
        &s,
        support::now(),
    )
    .await;
    approve(&fixture, &name, &code_of(joined)).await;
    let connector = Connector::start(folder.home.clone(), s.clone())
        .await
        .unwrap();
    watch
        .until(PATIENCE, |event| matches!(event, Event::Takeover { .. }))
        .await;
    assert!(registrations.all().is_empty());

    // The same invite, with the registration and a newer program.
    let newer = program();
    std::fs::write(&newer, b"a newer program for the tests").unwrap();
    let joined = arauto_connector::join(
        &folder.home,
        args(true, newer),
        &Hermes::default(),
        &s,
        support::now(),
    )
    .await;
    assert_eq!(joined, Ok(Joined::AlreadyApproved { registered: true }));
    assert_eq!(registrations.all().len(), 1);
    assert_eq!(
        std::fs::read(folder.home.program()).unwrap(),
        b"a newer program for the tests"
    );
    tokio::time::timeout(PATIENCE, connector.finished())
        .await
        .expect("the leader was asked to end");
    connector.shutdown().await;
}

/// A request the hub denies while the leader follows it: the only folder discarded because of
/// what the hub said is one without a credential; with one, nothing is deleted and the error is
/// kept.
#[tokio::test(flavor = "multi_thread")]
async fn a_request_denied_while_followed_is_discarded_only_without_a_credential() {
    let fixture = Fixture::new(settings()).await;
    for planted in [false, true] {
        let (s, mut watch, _registrations) = support::settings();
        let name = fixture.hermes().await;
        let folder = Folder::new();
        let invite = fixture.invite(&name).await;
        code_of(join(&fixture, &folder, &invite, &s).await);
        let request = folder.only_device();
        if planted {
            std::fs::write(
                folder.device_path(&request).join("credential.seal"),
                [3u8; 48],
            )
            .unwrap();
        }
        let connector = Connector::start(folder.home.clone(), s).await.unwrap();
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
        fixture.admin.deny(&name, fixture.now()).await.unwrap();
        if planted {
            watch
                .until(PATIENCE, |event| {
                    matches!(
                        event,
                        Event::Stopped {
                            code: ErrorCode::Ar301,
                            ..
                        }
                    )
                })
                .await;
            assert_eq!(folder.devices(), vec![request.clone()]);
            assert_eq!(folder.hub_json(&request)["code"], "AR-301");
        } else {
            watch
                .until(
                    PATIENCE,
                    |event| matches!(event, Event::Deleted { device } if *device == request),
                )
                .await;
            assert!(folder.devices().is_empty());
        }
        connector.shutdown().await;
    }
}

/// A call that left when the channel dropped gets AR-207: the result is unknown. The next one,
/// once the channel is back, works.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_in_flight_when_the_channel_drops_is_ar207() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s, &mut watch).await;
    let identity = folder.public(&folder.current().unwrap());
    let connector = std::sync::Arc::new(connector);
    // The hub holds the answer: `arauto_status` waits for the table of the links.
    let mut held = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let call = {
        let connector = std::sync::Arc::clone(&connector);
        tokio::spawn(async move { refusal(&connector).await })
    };
    fixture.until_a_query_waits_for_a_lock().await;
    watch.forget();
    close_links(&fixture, &identity, None);
    let refused = tokio::time::timeout(PATIENCE, call).await.unwrap().unwrap();
    assert_eq!(refused, Some(ErrorCode::Ar207));
    held.rollback().await.unwrap();
    watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    assert!(status_works(&connector).await);
    if let Ok(connector) = std::sync::Arc::try_unwrap(connector) {
        connector.shutdown().await;
    }
}

/// A connector too old for the hub (decision 0030): it presents what it has, hears AR-401,
/// stops using the channel with the error in every call, and presents itself again later.
/// Its credential was looked at and not taken for a copy.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_too_old_presents_itself_and_shows_ar401() {
    let mut fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    connector.shutdown().await;
    let device = folder.current().unwrap();
    let identity = folder.public(&device);
    // The hub comes back speaking only a version above the one of this connector.
    fixture.stop().await;
    fixture.settings.versions = arauto_protocol::message::ProtoRange::new(2, 2).unwrap();
    fixture.start().await;
    let path = folder.device_path(&device).join("hub.json");
    let mut moved = folder.hub_json(&device);
    moved["hub"] = serde_json::Value::from(fixture.channel_url());
    std::fs::write(&path, serde_json::to_vec(&moved).unwrap()).unwrap();

    watch.forget();
    let connector = Connector::start(folder.home.clone(), s).await.unwrap();
    watch
        .until(PATIENCE, |event| {
            matches!(
                event,
                Event::Stopped {
                    code: ErrorCode::Ar401,
                    ..
                }
            )
        })
        .await;
    assert_eq!(refusal(&connector).await, Some(ErrorCode::Ar401));
    // Presented again, after the interval of the tests.
    watch.forget();
    watch
        .until(PATIENCE, |event| {
            matches!(event, Event::AttemptStarted { .. })
        })
        .await;
    watch
        .until(PATIENCE, |event| {
            matches!(
                event,
                Event::Stopped {
                    code: ErrorCode::Ar401,
                    ..
                }
            )
        })
        .await;
    assert_eq!(fixture.state_of_key(&identity).await.0, "active");
    assert_eq!(fixture.alerts_of_key("quarantined", &identity).await, 0);
    connector.shutdown().await;
}
