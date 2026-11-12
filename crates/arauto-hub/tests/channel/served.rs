//! The purposes of the hub, end to end: a connector enrolls with the invite and the code,
//! waits for the admin, swaps its credential and serves; `arauto_status`; the rules of the
//! calls; leaving; the connector that is too old; the identity nobody knows; the 24-hour swap
//! (`docs/14-protocolo-do-canal.md`, sections 5 and 6; `docs/04-seguranca-conexao.md`, C3 to
//! C11; `docs/09-contrato-mcp.md`).

use std::sync::Arc;
use std::time::Duration;

use arauto_hub::channel::ChannelSettings;
use arauto_hub::repo::approval::{ApproveOutcome, Recognition, Recognized};
use arauto_hub::vault::Vault;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{
    Call, CallId, CallOutcome, ChannelState, CheckOk, Generation, HelloOk, LeftStatus, Message,
    MessageType, Payload, Ping, Pong, ProtoRange, Purpose, Saved,
};
use arauto_protocol::secret::SealedCredential;
use arauto_protocol::tools::{ARAUTO_STATUS, ToolName};
use arauto_testkit::KitError;
use arauto_testkit::connector::TestConnector;
use arauto_testkit::device::{Faults, Presentation, Reached};
use serde_json::json;

use crate::fixture::{Fixture, PATIENCE, settings, until_the_end};
use crate::support::eventually;

fn refused(outcome: &CallOutcome) -> Option<ErrorCode> {
    match outcome {
        CallOutcome::Refused { code, .. } => Some(*code),
        CallOutcome::Ok(_) => None,
    }
}

/// C3 to C7: the invite, the code typed by the admin, the wait, the first swap, the session.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_enters_with_the_invite_and_the_code_and_reaches_the_session() {
    let fixture = Fixture::new(settings()).await;
    let name = fixture.hermes().await;
    let device = fixture.device();
    let code = fixture.enroll(&name, &device).await;
    assert_eq!(fixture.state_of(&device).await.0, "pending");

    // The leader opens the channel while the request waits for the admin.
    let waiting = {
        let device = device.clone();
        tokio::spawn(async move {
            device
                .channel_waiting(Faults::default(), Duration::from_secs(30))
                .await
        })
    };
    let hub = Arc::clone(fixture.hub());
    assert!(
        eventually(PATIENCE, || hub.sessions_by_device().is_empty()
            && hub.links().len() == 1)
        .await
    );
    // The waiting connection does not count toward the ceiling of connections without a
    // session, so it stays as long as the admin takes.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let ApproveOutcome::Approved(_) = fixture.approve(&name, &code).await else {
        panic!("the right code approves");
    };
    let session = waiting.await.unwrap().unwrap().session().unwrap();
    assert_eq!(session.session.hermes.as_str(), name);
    let tools: Vec<&str> = session
        .session
        .tools
        .as_slice()
        .iter()
        .map(ToolName::as_str)
        .collect();
    assert_eq!(tools, [ARAUTO_STATUS]);
    assert!(!session.session.wake);
    assert!(device.disk().credential().is_some());

    let row: (String, i64, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT state, current_generation, pending_generation, first_confirmed_at \
         FROM device WHERE public_key = $1",
    )
    .bind(device.public().as_bytes().as_slice())
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "active");
    assert_eq!(row.1, 1);
    assert_eq!(row.2, None);
    assert!(row.3.is_some());
    // The connection is on record with its credential and its confirmation.
    let recorded: (Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT c.credential_issued_at, c.swap_confirmed_at, c.closed_at \
         FROM channel_connection c JOIN device d ON d.id = c.device_id \
         WHERE d.public_key = $1 AND c.swap_confirmed_at IS NOT NULL",
    )
    .bind(device.public().as_bytes().as_slice())
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert!(recorded.0.is_some() && recorded.1.is_some() && recorded.2.is_none());
    let hub = Arc::clone(fixture.hub());
    let id = fixture.id_of(&device).await;
    assert!(eventually(PATIENCE, || hub.sessions_by_device().get(&id) == Some(&1)).await);
}

/// `arauto_status` is answered by the hub, without the engine: connected, the name, whether
/// it can be woken, the numbers (none yet) and the links with other Hermes. No secret.
#[tokio::test(flavor = "multi_thread")]
async fn arauto_status_answers_from_the_hub_and_says_no_secret() {
    let fixture = Fixture::new(settings()).await;
    let (name, mut session) = fixture.connected().await;
    let outcome = session.call(ARAUTO_STATUS, json!({})).await.unwrap();
    let CallOutcome::Ok(content) = outcome else {
        panic!("{outcome:?}");
    };
    let content = serde_json::Value::Object(content.into_map());
    assert_eq!(
        content,
        json!({
            "connected": true,
            "hermes": name,
            "can_be_woken": false,
            "numbers": [],
            "peers": {"enabled": false, "can_send": false, "can_receive": false},
        })
    );
    // A link from another Hermes to this one: it can receive, not send.
    let other = fixture.hermes().await;
    sqlx::query(
        "INSERT INTO peer_link (organization_id, from_hermes_id, to_hermes_id, created_at, \
                                changed_at) \
         SELECT o.organization_id, o.id, t.id, 1, 1 FROM hermes o, hermes t \
         WHERE o.name = $1 AND t.name = $2",
    )
    .bind(&other)
    .bind(&name)
    .execute(&fixture.pool)
    .await
    .unwrap();
    fixture
        .admin
        .wake(&name, true, fixture.now())
        .await
        .unwrap();
    // The link gives it the tools that read: the hub tells the session.
    loop {
        match session
            .pump(std::time::Duration::from_secs(20))
            .await
            .unwrap()
        {
            Some(arauto_protocol::message::Message::Tools(_)) => break,
            Some(other) => panic!("the hub sent {:?}", other.kind()),
            None => {}
        }
    }
    let CallOutcome::Ok(content) = session.call(ARAUTO_STATUS, json!({})).await.unwrap() else {
        panic!("not answered");
    };
    let text = serde_json::Value::Object(content.into_map()).to_string();
    assert!(
        text.contains(r#""peers":{"can_receive":true,"can_send":false,"enabled":true}"#)
            || text.contains(r#""enabled":true"#),
        "{text}"
    );
    assert!(text.contains(r#""can_be_woken":true"#), "{text}");
    // Nothing of the credential, of the identity or of the key of the hub.
    let credential = session.device().disk().credential().unwrap();
    for secret in [
        data_encoding::BASE64.encode(credential.expose()),
        data_encoding::HEXLOWER.encode(credential.expose()),
        data_encoding::HEXLOWER.encode(session.device().public().as_bytes()),
        data_encoding::BASE64.encode(session.device().public().as_bytes()),
        fixture.installation.hub_key.to_text(),
    ] {
        assert!(!text.contains(&secret));
    }
}

/// The name and every argument are checked again by the hub (AR-711), a tool that is not
/// released is refused (AR-701), and a call before the session closes the connection.
#[tokio::test(flavor = "multi_thread")]
async fn every_call_is_checked_again_and_only_released_tools_run() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let cases = [
        ("arauto_status", json!({"extra": 1}), ErrorCode::Ar711),
        ("whatsapp_send_text", json!({}), ErrorCode::Ar711),
        ("no_such_tool", json!({}), ErrorCode::Ar711),
        (
            "hermes_send_message",
            json!({"to": "Not A Name", "text": "x"}),
            ErrorCode::Ar711,
        ),
        ("hermes_list_peers", json!({}), ErrorCode::Ar701),
        ("hermes_list_threads", json!({}), ErrorCode::Ar701),
        (
            "hermes_send_message",
            json!({"to": "someone", "text": "hello"}),
            ErrorCode::Ar701,
        ),
    ];
    for (tool, args, code) in cases {
        let outcome = session.call(tool, args).await.unwrap();
        assert_eq!(refused(&outcome), Some(code), "{tool}");
    }
    // The connection goes on after every refusal.
    assert!(matches!(
        session.call(ARAUTO_STATUS, json!({})).await.unwrap(),
        CallOutcome::Ok(_)
    ));

    // C7, rule 4: a call before `session` is a protocol error.
    let (_, device) = fixture.approved().await;
    let mut early = TestConnector::connect(
        fixture.addr(),
        device.identity(),
        &fixture.installation.hub_key,
    )
    .await
    .unwrap();
    early.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        early.recv(PATIENCE).await.unwrap(),
        Message::State(ChannelState::Approved)
    ));
    early
        .send(&Message::Call(Call {
            id: CallId::MIN,
            tool: ToolName::parse(ARAUTO_STATUS).unwrap(),
            args: Payload::default(),
        }))
        .await
        .unwrap();
    assert_eq!(
        early.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar204)
    );
    assert_eq!(fixture.state_of(&device).await.0, "approved");
}

/// C8: the state of the device is read at every call. Revoked, with the notice and the sweep
/// out of the way, the next call is refused with the code of the state, and the connection
/// closes after it.
#[tokio::test(flavor = "multi_thread")]
async fn every_call_reads_the_state_of_the_device_again() {
    let mut chosen = settings();
    chosen.listen_admin = false;
    chosen.sweep_interval_ms = 600_000;
    let fixture = Fixture::new(chosen).await;
    let (name, mut session) = fixture.connected().await;
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    let outcome = session.call(ARAUTO_STATUS, json!({})).await.unwrap();
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar502));
    assert_eq!(
        session.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar502)
    );
}

/// Up to 32 calls in progress; the 33rd gets AR-710 and the connection goes on. An
/// identifier repeated among the calls in progress closes the connection.
#[tokio::test(flavor = "multi_thread")]
async fn thirty_two_calls_in_progress_and_a_repeated_identifier() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;

    // `arauto_status` reads the links of the Hermes: while the test holds that table, every
    // call stays in progress.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *holder)
        .await
        .unwrap();
    for id in 1..=33 {
        session
            .send_call(id, ARAUTO_STATUS, json!({}))
            .await
            .unwrap();
    }
    let first = session.next_result(PATIENCE).await.unwrap();
    assert_eq!(first.id.get(), 33);
    assert_eq!(refused(&first.outcome), Some(ErrorCode::Ar710));
    holder.commit().await.unwrap();
    let mut answered = Vec::new();
    for _ in 0..32 {
        let result = session.next_result(PATIENCE).await.unwrap();
        assert!(matches!(result.outcome, CallOutcome::Ok(_)), "{result:?}");
        answered.push(result.id.get());
    }
    answered.sort_unstable();
    assert_eq!(answered, (1..=32).collect::<Vec<u64>>());
    // The connection went on.
    assert!(matches!(
        session.call(ARAUTO_STATUS, json!({})).await.unwrap(),
        CallOutcome::Ok(_)
    ));

    // The same identifier twice while the first is in progress.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *holder)
        .await
        .unwrap();
    session
        .send_call(500, ARAUTO_STATUS, json!({}))
        .await
        .unwrap();
    session
        .send_call(500, ARAUTO_STATUS, json!({}))
        .await
        .unwrap();
    let ended = session.ended(PATIENCE).await.unwrap();
    holder.commit().await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));
    assert_eq!(fixture.state_of(session.device()).await.0, "active");
}

/// C10: leaving over the open channel, and over a short connection while a session is open.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_leaves_over_the_open_channel_or_by_a_short_connection() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let device = session.device().clone();
    assert_eq!(
        session.leave().await.unwrap(),
        arauto_protocol::message::LeftStatus::Closed
    );
    // The connection ends after `left`, without `closing`.
    assert_eq!(session.ended(PATIENCE).await.unwrap().closing, None);
    assert_eq!(
        fixture.state_of(&device).await,
        ("revoked".to_owned(), Some("left".to_owned()))
    );
    assert_eq!(fixture.alerts("device_left", &device).await, 1);

    // A short connection of the same identity while another device's session is open: the
    // session of that device ends with the code of its new state.
    let (_, mut other) = fixture.connected().await;
    let leaving = other.device().clone();
    assert_eq!(
        leaving.leave().await.unwrap(),
        arauto_protocol::message::LeftStatus::Closed
    );
    assert_eq!(
        other.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar502)
    );

    // A pending request that asks to leave over its waiting channel is denied.
    let name = fixture.hermes().await;
    let pending = fixture.device();
    fixture.enroll(&name, &pending).await;
    let mut waiting = TestConnector::connect(
        fixture.addr(),
        pending.identity(),
        &fixture.installation.hub_key,
    )
    .await
    .unwrap();
    waiting.hello(Purpose::Channel).await.unwrap();
    let said = waiting.recv(PATIENCE).await;
    assert!(
        matches!(said, Ok(Message::State(ChannelState::Waiting))),
        "{said:?}"
    );
    let Message::Check(check) = waiting.recv(PATIENCE).await.unwrap() else {
        panic!("no check");
    };
    waiting
        .send(&Message::CheckOk(arauto_protocol::message::CheckOk {
            nonce: check.nonce,
        }))
        .await
        .unwrap();
    waiting.send(&Message::Leave).await.unwrap();
    assert!(matches!(
        waiting.recv(PATIENCE).await.unwrap(),
        Message::Left(_)
    ));
    assert_eq!(fixture.state_of(&pending).await.0, "denied");
}

/// A connector too old (decision 0030): its credential is looked at before the refusal.
/// What it presents is judged as in the swap, nothing is ever issued, and it hears AR-401.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_too_old_is_looked_at_and_told_ar401() {
    let mut fixture = Fixture::new(settings()).await;
    let (_, session) = fixture.connected().await;
    let device = session.device().clone();
    drop(session);
    // An older credential of the same device, then a newer swap kills it.
    let older = device.disk().credential().unwrap();
    let session = device
        .clone()
        .channel(Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    drop(session);
    let (_, reused) = fixture.connected().await;
    let reused = reused.device().clone();

    // The hub now speaks only a version this connector does not.
    fixture.stop().await;
    fixture.settings.versions = ProtoRange::new(2, 2).unwrap();
    fixture.start().await;
    let device = device.at(fixture.addr());
    let reused = reused.at(fixture.addr());

    // Its current credential: AR-401, and the device stays as it was.
    let reached = device.channel(Faults::default()).await.unwrap();
    let Reached::Closed { state, ended } = reached else {
        panic!("{reached:?}");
    };
    assert_eq!(state, Some(ChannelState::Ended(ErrorCode::Ar401)));
    assert_eq!(ended.closing, Some(ErrorCode::Ar401));
    assert_eq!(fixture.state_of(&device).await.0, "active");
    let generation: i64 =
        sqlx::query_scalar("SELECT max_generation FROM device WHERE public_key = $1")
            .bind(device.public().as_bytes().as_slice())
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(generation, 2, "nothing was issued");

    // A credential that was already swapped: reuse, quarantine, and still AR-401.
    let reached = reused
        .channel(Faults {
            present: Presentation::Chosen(None),
            ..Faults::default()
        })
        .await
        .unwrap();
    assert_eq!(reached.closing(), Some(ErrorCode::Ar401));
    assert_eq!(fixture.state_of(&reused).await.0, "quarantined");
    let reached = device
        .channel(Faults {
            present: Presentation::Chosen(Some(older)),
            ..Faults::default()
        })
        .await
        .unwrap();
    assert_eq!(reached.closing(), Some(ErrorCode::Ar401));
    assert_eq!(
        fixture.state_of(&device).await,
        ("quarantined".to_owned(), Some("reuse".to_owned()))
    );
    // Enrolling with a connector too old is refused before anything.
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let newcomer = fixture.device();
    assert_eq!(
        newcomer
            .enroll(&invite, arauto_testkit::device::declared())
            .await
            .unwrap(),
        arauto_testkit::device::Enrolled::Refused(ErrorCode::Ar401)
    );
    // Leaving is served whatever the version.
    let mut leaving = TestConnector::connect(
        fixture.addr(),
        device.identity(),
        &fixture.installation.hub_key,
    )
    .await
    .unwrap();
    assert_eq!(
        leaving.hello(Purpose::Leave).await.unwrap(),
        HelloOk::Outdated
    );
    leaving.send(&Message::Leave).await.unwrap();
    assert!(matches!(
        leaving.recv(PATIENCE).await.unwrap(),
        Message::Left(_)
    ));
}

/// C11: an identity nobody knows hears AR-504 whatever it presents. Only a valid seal of an
/// organization lists it, and that is proof that the hub came back from a copy.
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_identity_hears_ar504_and_is_listed_only_with_a_valid_seal() {
    let fixture = Fixture::new(settings()).await;
    let stranger = fixture.device();
    let listed = || async {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM unknown_device")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        count
    };
    for presented in [
        None,
        Some(SealedCredential::from_slice(&[7u8; 100]).unwrap()),
    ] {
        let reached = stranger
            .channel(Faults {
                present: Presentation::Chosen(presented),
                ..Faults::default()
            })
            .await
            .unwrap();
        let Reached::Closed { state, ended } = reached else {
            panic!("{reached:?}");
        };
        assert_eq!(state, Some(ChannelState::Ended(ErrorCode::Ar504)));
        assert_eq!(ended.closing, Some(ErrorCode::Ar504));
        assert_eq!(listed().await, 0, "nothing is recorded for an attempt");
    }
    assert!(!fixture.restored_mode().await);

    // A valid seal of this organization, for this identity, that the database does not know.
    let vault = Vault::open(&fixture.installation.data_dir).unwrap();
    let key = arauto_hub::repo::device::organization_key(
        &fixture.pool,
        fixture.installation.organization,
        &vault,
    )
    .await
    .unwrap();
    let seal = key
        .seal_credential(
            stranger.public(),
            Generation::new(4).unwrap(),
            fixture.now(),
        )
        .unwrap();
    let reached = stranger
        .channel(Faults {
            present: Presentation::Chosen(Some(seal.sealed)),
            ..Faults::default()
        })
        .await
        .unwrap();
    assert_eq!(reached.closing(), Some(ErrorCode::Ar504));
    assert_eq!(listed().await, 1);
    // The connection hears AR-504 first; the mode is turned on right after. With no
    // suspended device, the sweep ends it at once: what stays is the event.
    let mut entered = 0i64;
    for _ in 0..600 {
        entered =
            sqlx::query_scalar("SELECT count(*) FROM audit WHERE action = 'restored_mode.entered'")
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        if entered > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(entered, 1, "a valid seal nobody knows is proof");
}

/// The 24-hour swap, with the clock of the hub moved forward: `rotate`, `credential`,
/// `saved`, `rotated`, while the calls go on. Without the `saved` in time the connection
/// closes, and on the way back the device is not locked out.
#[tokio::test(flavor = "multi_thread")]
async fn the_24_hour_swap_with_the_clock_moved_forward() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let mut chosen: ChannelSettings = settings();
    chosen.deadlines.saved_ms = 1_500;
    chosen.sweep_interval_ms = 100;
    let fixture = Fixture::with_clock(chosen, Arc::new(clock.clone())).await;
    let (_, mut session) = fixture.connected().await;
    let device = session.device().clone();
    assert!(session.rotated.is_empty());

    clock.advance(24 * 3_600_000);
    // The calls go on while the swap happens.
    for _ in 0..5 {
        assert!(matches!(
            session.call(ARAUTO_STATUS, json!({})).await.unwrap(),
            CallOutcome::Ok(_)
        ));
        session.idle(Duration::from_millis(100)).await.unwrap();
    }
    session.idle(Duration::from_millis(500)).await.unwrap();
    assert_eq!(session.rotated, [Generation::new(2).unwrap()]);
    let current: i64 = sqlx::query_scalar(
        "SELECT current_generation FROM device WHERE public_key = $1 AND pending_generation IS NULL",
    )
    .bind(device.public().as_bytes().as_slice())
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(current, 2);
    // Not due again until another 24 hours.
    session.idle(Duration::from_millis(500)).await.unwrap();
    assert_eq!(session.rotated.len(), 1);

    // The "saved" never comes: the connection closes after 60 seconds (1.5 here).
    session.skip_saved = true;
    clock.advance(24 * 3_600_000);
    let ended = session.ended(Duration::from_secs(10)).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));
    // On the way back the device presents what it has, which is still the current one.
    let back = device.channel(Faults::default()).await.unwrap();
    assert!(matches!(back, Reached::Session(_)), "{back:?}");
    assert_eq!(fixture.state_of(&device).await.0, "active");
}

/// C10: a new invite for a Hermes that has a device. The admin is shown the last use of the
/// device in use and says whether it is recognized; the new device replaces the old one when
/// it confirms its first credential, and the old connection hears AR-503.
#[tokio::test(flavor = "multi_thread")]
async fn a_reconnect_shows_the_last_use_and_replaces_the_old_device() {
    let fixture = Fixture::new(settings()).await;
    let (name, mut old) = fixture.connected().await;
    let old_device = old.device().clone();
    let keep = tokio::spawn(async move {
        let ended = old.ended(Duration::from_secs(60)).await;
        ended.map(|ended| ended.closing)
    });

    let newer = fixture.device();
    let code = fixture.enroll(&name, &newer).await;
    let ApproveOutcome::NeedsRecognition(shown) = fixture
        .admin
        .approve(&name, &code, None, fixture.now())
        .await
        .unwrap()
    else {
        panic!("a reconnect asks about the last use");
    };
    assert!(shown.at.is_some());
    assert_eq!(shown.address.as_deref(), Some("127.0.0.1"));
    let ApproveOutcome::Approved(approved) = fixture
        .admin
        .approve(
            &name,
            &code,
            Some(Recognized {
                answer: Recognition::Recognized,
                shown,
            }),
            fixture.now(),
        )
        .await
        .unwrap()
    else {
        panic!("approved");
    };
    assert!(approved.replaces.is_some());
    // Until the new one confirms, the old one goes on.
    assert_eq!(fixture.state_of(&old_device).await.0, "active");
    let session = newer
        .channel(Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    assert_eq!(session.session.hermes.as_str(), name);
    assert_eq!(keep.await.unwrap().unwrap(), Some(ErrorCode::Ar503));
    assert_eq!(fixture.state_of(&old_device).await.0, "replaced");
    assert_eq!(fixture.state_of(&newer).await.0, "active");

    // The last use not recognized: the device in use goes to quarantine at once.
    let (name, mut old) = fixture.connected().await;
    let newer = fixture.device();
    let code = fixture.enroll(&name, &newer).await;
    let ApproveOutcome::NeedsRecognition(shown) = fixture
        .admin
        .approve(&name, &code, None, fixture.now())
        .await
        .unwrap()
    else {
        panic!("a reconnect asks about the last use");
    };
    fixture
        .admin
        .approve(
            &name,
            &code,
            Some(Recognized {
                answer: Recognition::NotRecognized,
                shown,
            }),
            fixture.now(),
        )
        .await
        .unwrap();
    assert_eq!(
        old.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar501)
    );
    assert_eq!(fixture.alerts("copy_suspected", old.device()).await, 1);
}

/// C7, rule 7, in the session: a `saved` that refers to no 24-hour swap under way closes the
/// connection with AR-204, and the device stays active.
#[tokio::test(flavor = "multi_thread")]
async fn a_saved_outside_a_24_hour_swap_closes_the_session() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let credential = session.device().disk().credential().unwrap();
    session
        .connector()
        .send(&Message::Saved(Saved {
            generation: Generation::MIN,
            digest: CredentialDigest::of(&credential),
        }))
        .await
        .unwrap();
    assert_eq!(
        session.ended(Duration::from_secs(5)).await.unwrap().closing,
        Some(ErrorCode::Ar204)
    );
    assert_eq!(fixture.state_of(session.device()).await.0, "active");
}

/// C7, rule 4: a `call` the connector sends right after `saved`, before it read `session`, is
/// never served. The hub knows it came before `session` left: the connection closes with
/// AR-204 and no `result` is sent. The promotion is recorded, and the next attempt goes on.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_sent_before_session_was_read_is_never_served() {
    let fixture = Fixture::new(settings()).await;
    let (name, device) = fixture.approved().await;
    let (mut connector, nonce) = fixture.up_to_the_check(&device).await;
    connector
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    let Message::Credential(credential) = connector.recv(PATIENCE).await.unwrap() else {
        panic!("no credential");
    };
    let saved = Saved {
        generation: credential.generation,
        digest: CredentialDigest::of(&credential.sealed),
    };
    device.disk().store(credential.sealed);
    // T2 waits for the Hermes, which the test holds: the call surely arrives before
    // `session` leaves.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE name = $1 FOR UPDATE")
        .bind(&name)
        .execute(&mut *holder)
        .await
        .unwrap();
    connector.send(&Message::Saved(saved)).await.unwrap();
    connector
        .send(&Message::Call(Call {
            id: CallId::new(7).unwrap(),
            tool: ToolName::parse(ARAUTO_STATUS).unwrap(),
            args: Payload::default(),
        }))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    holder.commit().await.unwrap();
    let (seen, ended) = until_the_end(&mut connector).await;
    assert_eq!(seen, [MessageType::Session], "the call was served");
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));
    assert_eq!(fixture.state_of(&device).await.0, "active");
    let back = device.channel(Faults::default()).await.unwrap();
    assert!(matches!(back, Reached::Session(_)), "{back:?}");
}

/// C8: the state of the device comes first, whatever the call asks. A revoked device, with
/// the notice and the sweep out of the way, that sends a call with a malformed argument hears
/// the code of its state, and the connection closes.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_that_no_longer_serves_hears_its_code_whatever_the_call_asks() {
    let mut chosen = settings();
    chosen.listen_admin = false;
    chosen.sweep_interval_ms = 600_000;
    let fixture = Fixture::new(chosen).await;
    let (name, mut session) = fixture.connected().await;
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    let outcome = session
        .call(ARAUTO_STATUS, json!({"extra": 1}))
        .await
        .unwrap();
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar502));
    assert_eq!(
        session.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar502)
    );
}

/// `docs/15-mvp.md`, section 3, and the rule of hostile input: with 32 calls in progress, a
/// connector that keeps calling without reading anything gets its AR-710 from the loop of the
/// session. The refusals wait for room to leave, the wait reaches the TCP connection of the
/// connector, and the tasks of the hub do not grow with the calls.
#[tokio::test(flavor = "multi_thread")]
async fn calls_refused_with_ar710_do_not_pile_up_in_the_hub() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    // While the test holds the table of links, every `arauto_status` stays in progress.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *holder)
        .await
        .unwrap();
    for id in 1..=32 {
        session
            .send_call(id, ARAUTO_STATUS, json!({}))
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let metrics = tokio::runtime::Handle::current().metrics();
    let before = metrics.num_alive_tasks();
    // Thousands of calls, nothing read: sending stops when the hub stops reading.
    let mut sent = 0u64;
    let _ = tokio::time::timeout(Duration::from_secs(3), async {
        for id in 33..=20_032 {
            if session
                .send_call(id, ARAUTO_STATUS, json!({}))
                .await
                .is_err()
            {
                break;
            }
            sent += 1;
        }
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = metrics.num_alive_tasks();
    holder.rollback().await.unwrap();
    assert!(sent > 100, "only {sent} calls left the test");
    assert!(
        after <= before + 16,
        "alive tasks: {before} before, {after} after {sent} calls"
    );
}

/// The 14, section 7: the orderly stop lets a call in progress send its `result`, refuses with
/// AR-205 a call that comes during the stop, and only then sends `closing` with AR-205.
#[tokio::test(flavor = "multi_thread")]
async fn the_orderly_stop_lets_the_calls_in_progress_answer() {
    let mut fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *holder)
        .await
        .unwrap();
    session
        .send_call(1, ARAUTO_STATUS, json!({}))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let running = fixture.running.take().unwrap();
    let stopping = tokio::spawn(async move {
        let started = tokio::time::Instant::now();
        running.stop().await;
        started.elapsed()
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    session
        .send_call(2, ARAUTO_STATUS, json!({}))
        .await
        .unwrap();
    let refused_during_the_stop = session.next_result(PATIENCE).await.unwrap();
    assert_eq!(refused_during_the_stop.id.get(), 2);
    assert_eq!(
        refused(&refused_during_the_stop.outcome),
        Some(ErrorCode::Ar205)
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    holder.commit().await.unwrap();
    let answered = session.next_result(PATIENCE).await.unwrap();
    assert_eq!(answered.id.get(), 1);
    assert!(
        matches!(answered.outcome, CallOutcome::Ok(_)),
        "{answered:?}"
    );
    assert_eq!(
        session.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar205)
    );
    let took = stopping.await.unwrap();
    assert!(took < Duration::from_secs(8), "{took:?}");
}

/// The 14, section 7: a call still running when the grace ends does not hold the stop. The hub
/// closes what is still open with AR-205, without the `result` of that call, and the stop ends
/// within the grace and two lingers.
#[tokio::test(flavor = "multi_thread")]
async fn the_orderly_stop_closes_by_force_what_is_still_open_when_the_grace_ends() {
    let mut chosen = settings();
    chosen.deadlines.shutdown_grace_ms = 1_000;
    let mut fixture = Fixture::new(chosen).await;
    let (_, mut session) = fixture.connected().await;
    // The call stays stuck past the grace, and well before the 5 seconds of the
    // `lock_timeout` of the hub would end it.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *holder)
        .await
        .unwrap();
    session
        .send_call(1, ARAUTO_STATUS, json!({}))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let running = fixture.running.take().unwrap();
    let started = tokio::time::Instant::now();
    let stopping = tokio::spawn(async move { running.stop().await });
    let ended = loop {
        match session.pump(Duration::from_secs(4)).await {
            Ok(None) => {}
            Ok(Some(other)) => panic!("the hub sent {other:?} instead of closing by force"),
            Err(KitError::Ended(ended)) => break ended,
            Err(other) => panic!("the session was not closed by force: {other}"),
        }
    };
    let closed_after = started.elapsed();
    assert_eq!(ended.closing, Some(ErrorCode::Ar205));
    assert!(
        closed_after < Duration::from_millis(3_500),
        "{closed_after:?}"
    );
    stopping.await.unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(4_500),
        "{:?}",
        started.elapsed()
    );
    holder.rollback().await.unwrap();
}

/// The request to leave by a short connection whose closing the audit mark could not follow
/// (another process holds the lock of the mark): the device left, `left` says so, and the
/// admin is told, as on the open channel.
#[tokio::test(flavor = "multi_thread")]
async fn a_short_leave_the_audit_mark_could_not_follow_tells_the_admin() {
    let fixture = Fixture::new(settings()).await;
    let (_, session) = fixture.connected().await;
    let device = session.device().clone();
    drop(session);
    let hub = Arc::clone(fixture.hub());
    assert!(eventually(PATIENCE, || hub.sessions_by_device().is_empty()).await);
    let other_process = arauto_platform::FileLock::try_acquire(
        &fixture
            .installation
            .data_dir
            .join(arauto_hub::mark::MARK_LOCK_FILE),
    )
    .unwrap();
    let left = device.leave().await;
    drop(other_process);
    assert_eq!(left.unwrap(), LeftStatus::Closed);
    assert_eq!(
        fixture.state_of(&device).await,
        ("revoked".to_owned(), Some("left".to_owned()))
    );
    assert_eq!(fixture.alerts("mark_not_advanced", &device).await, 1);
}

/// The 14, section 4: a stream that gets no new chunk for 60 seconds closes the connection.
/// The 60 seconds are counted on the clock of the network, not on the clock of the hub: a
/// jump of the time of the hub does not cut a message under way.
#[tokio::test(flavor = "multi_thread")]
async fn a_jump_of_the_clock_of_the_hub_does_not_cut_a_message_under_way() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let fixture = Fixture::with_clock(settings(), Arc::new(clock.clone())).await;
    let (_, mut session) = fixture.connected().await;
    let frames = session
        .connector()
        .frames_of(&arauto_testkit::hostile::big_call(100_000).unwrap())
        .unwrap();
    assert!(frames.len() > 1);
    session.connector().send_frame(&frames[0]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    clock.advance(10 * 60_000);
    // Any frame wakes the reader, which works out the deadline of the stream again.
    session
        .connector()
        .send(&Message::Ping(Ping { n: 9 }))
        .await
        .unwrap();
    loop {
        match session.connector().recv_any(PATIENCE).await.unwrap() {
            Message::Pong(Pong { n: 9 }) => break,
            Message::Ping(_) => {}
            other => panic!("{:?}", other.kind()),
        }
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    for frame in &frames[1..] {
        session.connector().send_frame(frame).await.unwrap();
    }
    // The call arrives whole: it is answered (its argument is not one of the tool).
    let result = session.next_result(PATIENCE).await.unwrap();
    assert_eq!(refused(&result.outcome), Some(ErrorCode::Ar711));
}
