//! Each abuse of the hostile client ends the connection the way `docs/14-protocolo-do-canal.md`
//! says, and the hub goes on serving everyone else.

use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::frame::FLAG_FIN;
use arauto_protocol::message::{HelloOk, Message, Ping, Purpose, Saved};
use arauto_testkit::connector::{TestConnector, hello, identity};
use arauto_testkit::hostile::{self, HandshakeStep, Numbering, padded_hello, unknown_message};
use arauto_testkit::{Ended, RawSocket};

use crate::support::{self, Behavior, TestHub, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(10);

fn closing(code: ErrorCode) -> Option<ErrorCode> {
    Some(code)
}

/// A connector that behaves still gets its `hello_ok`.
async fn still_serves(hub: &TestHub) {
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    assert_eq!(
        connector.hello(Purpose::Leave).await.unwrap(),
        HelloOk::Agreed(1)
    );
}

/// A hub that waits a minute for `hello`: an abuse is never closed by that deadline instead
/// of by its own rule.
async fn hub() -> TestHub {
    let mut settings = support::settings();
    settings.deadlines.hello_ms = 60_000;
    support::start(settings, TestPurposes(Behavior::Park)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_text_message_is_answered_with_ar204_after_the_handshake_and_silence_before() {
    let hub = hub().await;
    let ended = hub.hostile().text_message().await.unwrap();
    assert_eq!(ended.closing, closing(ErrorCode::Ar204));
    let ended = hub.hostile().text_before_the_handshake().await.unwrap();
    assert_eq!(ended.closing, None);
    still_serves(&hub).await;
}

/// The limit is set in the websocket library: had the hub taken the message whole, it would
/// have tried to decrypt it and answered AR-204.
#[tokio::test(flavor = "multi_thread")]
async fn a_websocket_message_above_65535_bytes_is_never_put_together() {
    let hub = hub().await;
    let ended = hub.hostile().oversized_websocket_message().await.unwrap();
    assert_eq!(ended.closing, None);
    still_serves(&hub).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_handshake_message_of_another_size_ends_the_connection_without_an_answer() {
    let hub = hub().await;
    for (step, len) in [
        (HandshakeStep::First, 47),
        (HandshakeStep::First, 49),
        (HandshakeStep::First, 0),
        (HandshakeStep::Third, 63),
        (HandshakeStep::Third, 65),
    ] {
        let ended = hub
            .hostile()
            .handshake_of_the_wrong_size(step, len)
            .await
            .unwrap();
        assert_eq!(ended.closing, None, "{step:?} with {len} bytes");
    }
    still_serves(&hub).await;
}

/// The rule itself (a point of low order is nobody's key) is tested in the protocol crate,
/// where a forged handshake can be built; here the hub only has to refuse and go on.
#[tokio::test(flavor = "multi_thread")]
async fn a_key_of_low_order_ends_the_connection() {
    let hub = hub().await;
    let ended = hub.hostile().low_order_key().await.unwrap();
    assert_eq!(ended.closing, None);
    still_serves(&hub).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn headers_that_never_end_are_cut_at_the_deadline() {
    let mut settings = support::settings();
    settings.deadlines.http_headers_ms = 400;
    let hub = support::start(settings, TestPurposes(Behavior::Park)).await;
    let ended = hub
        .hostile()
        .slow_headers(Duration::from_millis(100))
        .await
        .unwrap();
    assert!(
        ended.after >= Duration::from_millis(350) && ended.after < Duration::from_secs(4),
        "{ended:?}"
    );
    still_serves(&hub).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_handshake_that_never_ends_is_cut_counting_from_the_websocket() {
    let mut settings = support::settings();
    settings.deadlines.handshake_ms = 500;
    let hub = support::start(settings, TestPurposes(Behavior::Park)).await;
    for send_first in [false, true] {
        let ended = hub
            .hostile()
            .handshake_that_never_ends(send_first)
            .await
            .unwrap();
        assert_eq!(ended.closing, None);
        assert!(
            ended.after >= Duration::from_millis(400) && ended.after < Duration::from_secs(4),
            "{send_first}: {ended:?}"
        );
    }
    still_serves(&hub).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_frame_with_reserved_bits_or_an_empty_chunk_is_answered_with_ar204() {
    let hub = hub().await;
    assert_eq!(
        hub.hostile().reserved_bits().await.unwrap().closing,
        closing(ErrorCode::Ar204)
    );
    assert_eq!(
        hub.hostile().empty_chunk().await.unwrap().closing,
        closing(ErrorCode::Ar204)
    );
    still_serves(&hub).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn before_the_session_only_stream_zero_and_4096_bytes() {
    let hub = hub().await;
    assert_eq!(
        hub.hostile()
            .stream_before_the_session()
            .await
            .unwrap()
            .closing,
        closing(ErrorCode::Ar204)
    );
    assert_eq!(
        hub.hostile()
            .oversized_before_the_session()
            .await
            .unwrap()
            .closing,
        closing(ErrorCode::Ar204)
    );
    // The same `hello`, one byte shorter, is a `hello`.
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    connector
        .send_frame(&TestConnector::raw_frame(
            FLAG_FIN,
            0,
            &padded_hello(4_096).unwrap(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::HelloOk(HelloOk::Agreed(1))
    ));
    still_serves(&hub).await;
}

/// The counter does not travel: a frame sent twice, or two frames in the other order, do not
/// decrypt with the counter of the hub.
#[tokio::test(flavor = "multi_thread")]
async fn a_repeated_or_reordered_frame_is_answered_with_ar204() {
    let hub = hub().await;
    assert_eq!(
        hub.hostile().repeated_frame().await.unwrap().closing,
        closing(ErrorCode::Ar204)
    );
    assert_eq!(
        hub.hostile().reordered_frames().await.unwrap().closing,
        closing(ErrorCode::Ar204)
    );
    still_serves(&hub).await;
}

async fn in_session(hub: &TestHub) -> TestConnector {
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    connector.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::Session(_)
    ));
    connector
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_number_of_the_wrong_parity_repeated_or_lower_is_answered_with_ar204() {
    let hub = support::start(support::settings(), TestPurposes(Behavior::Session)).await;
    for abuse in [
        Numbering::WrongParity,
        Numbering::Repeated,
        Numbering::Lower,
    ] {
        let mut connector = in_session(&hub).await;
        let ended = hostile::stream_numbering(&mut connector, abuse)
            .await
            .unwrap();
        assert_eq!(ended.closing, closing(ErrorCode::Ar204), "{abuse:?}");
    }
    // A message of two frames on a stream that grows is fine.
    let mut connector = in_session(&hub).await;
    connector
        .send(&hostile::big_call(70_000).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::Tools(_)
    ));
}

async fn after_hello(hub: &TestHub, purpose: Purpose) -> TestConnector {
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    connector.hello(purpose).await.unwrap();
    connector
}

async fn ends_with(connector: &mut TestConnector) -> Ended {
    connector.ended(PATIENCE).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn messages_out_of_place_before_the_session_are_answered_with_ar204() {
    let hub = hub().await;
    // The first message is not `hello`.
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    connector.send(&Message::Leave).await.unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
    // A `ping` before `hello_ok`: the heartbeat has not started.
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    connector.send(&Message::Ping(Ping { n: 1 })).await.unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
    // A type this version does not know.
    let mut connector = after_hello(&hub, Purpose::Channel).await;
    connector
        .send_frame(&TestConnector::raw_frame(FLAG_FIN, 0, &unknown_message()))
        .await
        .unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
    // A message only the hub sends.
    let mut connector = after_hello(&hub, Purpose::Channel).await;
    connector
        .send_frame(&TestConnector::raw_frame(FLAG_FIN, 0, br#"{"t":"rotate"}"#))
        .await
        .unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
    // `hello` again, with the CONTROL flag it does not have.
    let mut connector = after_hello(&hub, Purpose::Channel).await;
    let body = arauto_protocol::message::encode(&hello((1, 1), Purpose::Leave).unwrap()).unwrap();
    connector
        .send_frame(&TestConnector::raw_frame(FLAG_FIN | 0b10, 0, &body))
        .await
        .unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
    still_serves(&hub).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_waiting_connection_only_sends_check_ok_ping_pong_and_leave() {
    let hub = support::start(
        support::settings(),
        TestPurposes(Behavior::Wait { ms: 60_000 }),
    )
    .await;
    let mut connector = after_hello(&hub, Purpose::Channel).await;
    // Allowed while waiting: the heartbeat.
    connector.send(&Message::Ping(Ping { n: 7 })).await.unwrap();
    assert!(matches!(
        connector.recv_any(PATIENCE).await.unwrap(),
        Message::Pong(arauto_protocol::message::Pong { n: 7 })
    ));
    // Not allowed while waiting.
    let saved = Message::Saved(Saved {
        generation: arauto_protocol::message::Generation::MIN,
        digest: arauto_protocol::digest::CredentialDigest::from_bytes([7; 32]),
    });
    connector.send(&saved).await.unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
}

/// In session a message of unknown type is ignored, so that a newer connector can speak to
/// an older hub; the hundredth closes.
#[tokio::test(flavor = "multi_thread")]
async fn in_session_the_hundredth_message_of_unknown_type_closes() {
    let hub = support::start(support::settings(), TestPurposes(Behavior::Session)).await;
    let mut connector = in_session(&hub).await;
    let frame = TestConnector::raw_frame(FLAG_FIN, 0, &unknown_message());
    for _ in 0..99 {
        connector.send_frame(&frame).await.unwrap();
    }
    connector
        .send(&hostile::big_call(10).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::Tools(_)
    ));
    connector.send_frame(&frame).await.unwrap();
    assert_eq!(
        ends_with(&mut connector).await.closing,
        closing(ErrorCode::Ar204)
    );
}

/// The connector raw socket opens and the hub never hears from it again: nothing is held.
#[tokio::test(flavor = "multi_thread")]
async fn a_websocket_dropped_after_opening_leaves_nothing_behind() {
    let hub = hub().await;
    let socket = RawSocket::open(hub.addr()).await.unwrap();
    drop(socket);
    assert!(hub.links_become(0, PATIENCE).await);
    still_serves(&hub).await;
}
