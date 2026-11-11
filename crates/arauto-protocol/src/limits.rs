//! Sizes, counts and deadlines of the channel.
//!
//! The numbers come from `docs/14-protocolo-do-canal.md` (sections 1 to 7) and from
//! `docs/13-conversa-entre-hermes.md` (sections 3 to 6 and 8). They exist because a connection
//! is open to anyone until the session starts: size never brings the receiver down.
//!
//! Sizes and counts are constants. Deadlines also live in [`Deadlines`], whose default holds the
//! documented values and whose fields a test can shrink.

// ---------------------------------------------------------------------------------------------
// Websocket and handshake
// ---------------------------------------------------------------------------------------------

/// Largest websocket message, in bytes. Only binary messages are accepted.
pub const WS_MESSAGE_MAX: usize = 65_535;

/// Exact size of the first handshake message (connector to hub: `e, es`).
pub const HANDSHAKE_FIRST_LEN: usize = 48;

/// Exact size of the second handshake message (hub to connector: `e, ee`).
pub const HANDSHAKE_SECOND_LEN: usize = 48;

/// Exact size of the third handshake message (connector to hub: `s, se`).
pub const HANDSHAKE_THIRD_LEN: usize = 64;

/// Size of the handshake hash (the Noise value `h`).
pub const HANDSHAKE_HASH_LEN: usize = 32;

/// Size of an X25519 key, private or public.
pub const KEY_LEN: usize = 32;

/// Size of the authentication tag that Noise appends to every encrypted message.
pub const NOISE_TAG_LEN: usize = 16;

// ---------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------

/// Size of the frame header: one byte of flags and four bytes of stream number.
pub const FRAME_HEADER_LEN: usize = 5;

/// Largest chunk of a message carried by one frame.
pub const FRAME_CHUNK_MAX: usize = 60_000;

/// Largest frame before encryption: header plus the largest chunk.
pub const FRAME_PLAINTEXT_MAX: usize = FRAME_HEADER_LEN + FRAME_CHUNK_MAX;

/// Streams one side may keep open at the same time, counted by the receiver.
pub const MAX_OPEN_STREAMS: usize = 8;

// An encrypted frame always fits one websocket message.
const _: () = assert!(FRAME_PLAINTEXT_MAX + NOISE_TAG_LEN <= WS_MESSAGE_MAX);

// ---------------------------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------------------------

/// Largest message before the session starts. Only stream 0 is accepted in that phase.
pub const PRE_SESSION_MESSAGE_MAX: usize = 4_096;

/// Largest message from the connector to the hub, in session (6 MiB).
pub const CONNECTOR_MESSAGE_MAX: usize = 6 * 1024 * 1024;

/// Largest message from the hub to the connector, in session (16 MiB).
pub const HUB_MESSAGE_MAX: usize = 16 * 1024 * 1024;

/// In session, the message of unknown type that closes the connection with AR-204. The ones
/// before it are ignored.
pub const UNKNOWN_MESSAGES_MAX: u32 = 100;

/// Most JSON values one message may have, counting every object, array, text, number, boolean
/// and null. The documents give no ceiling. Without one, a message of a few megabytes made of
/// tiny values would take dozens of times its size in memory to be read; the largest answer
/// the tools give stays far below this.
pub const MESSAGE_VALUES_MAX: usize = 100_000;

/// Deepest nesting of objects and arrays in one message, with the message itself as the first
/// level. The documents give no ceiling; this one keeps the reader far from the end of the
/// stack and does not depend on the limit a JSON library happens to have.
pub const MESSAGE_DEPTH_MAX: usize = 64;

/// Calls in progress per connection. The next one is refused with AR-710, without closing.
pub const CALLS_IN_FLIGHT_MAX: usize = 32;

/// Starting value of the hub-wide budget for messages under reassembly (256 MiB). Without room,
/// the connection that asks for more is closed with AR-205.
pub const REASSEMBLY_BUDGET_BYTES: usize = 256 * 1024 * 1024;

/// Connections without a session the hub keeps, apart from the ones waiting for approval.
pub const CONNECTIONS_WITHOUT_SESSION_MAX: usize = 1_024;

/// Connections without a session per origin, apart from the ones waiting for approval.
pub const CONNECTIONS_WITHOUT_SESSION_PER_ORIGIN_MAX: usize = 16;

// ---------------------------------------------------------------------------------------------
// Message fields
// ---------------------------------------------------------------------------------------------

/// Largest sealed credential, in bytes.
pub const CREDENTIAL_MAX_LEN: usize = 1_024;

/// Size of the nonce the hub sends in `enrolled`.
pub const ENROLL_NONCE_LEN: usize = 32;

/// Size of the nonce of `check` and `check_ok`.
pub const CHECK_NONCE_LEN: usize = 16;

/// Size of a SHA-256 digest.
pub const DIGEST_LEN: usize = 32;

/// Largest declared text (machine name, system, versions), in characters.
pub const DECLARED_TEXT_MAX_CHARS: usize = 128;

/// Enrollments the hub accepts for one connection request.
pub const ENROLLMENTS_PER_REQUEST_MAX: u8 = 5;

/// Largest `retry_after_s`, in seconds (one day).
pub const RETRY_AFTER_MAX_S: u32 = 86_400;

/// Largest `confirm_within_s`, in seconds. The documents give no ceiling; one day keeps the
/// number sane.
pub const CONFIRM_WITHIN_MAX_S: u32 = 86_400;

/// Largest `count` of a notice.
pub const NOTICE_COUNT_MAX: u16 = 9_999;

/// Largest Hermes name or number alias, in characters.
pub const NAME_MAX_LEN: usize = 32;

/// Largest chat identifier of a notice, in characters.
pub const CHAT_ID_MAX_LEN: usize = 64;

/// Size of an invite, in bytes.
pub const INVITE_LEN: usize = 32;

/// Size of an invite written as text: `arc1` plus 52 base32 characters.
pub const INVITE_TEXT_LEN: usize = 56;

/// Largest tool name, in characters. The documents give no ceiling; the longest name of the
/// catalog has 23. With this one, the largest list of the largest names still fits `session`,
/// which travels with the limit of the phase before the session.
pub const TOOL_NAME_MAX_LEN: usize = 48;

/// Largest list of released tools. The documents give no ceiling; the whole catalog has about 20.
pub const TOOL_LIST_MAX: usize = 64;

// ---------------------------------------------------------------------------------------------
// Deadlines, in milliseconds
// ---------------------------------------------------------------------------------------------

/// One second, in milliseconds.
pub const SECOND_MS: i64 = 1_000;

/// One minute, in milliseconds.
pub const MINUTE_MS: i64 = 60 * SECOND_MS;

/// One hour, in milliseconds.
pub const HOUR_MS: i64 = 60 * MINUTE_MS;

/// One day, in milliseconds.
pub const DAY_MS: i64 = 24 * HOUR_MS;

/// Hub: time for the request headers to arrive.
pub const HTTP_HEADERS_TIMEOUT_MS: i64 = 5 * SECOND_MS;

/// Hub: time for the handshake to end, counted from the opening of the websocket.
pub const HANDSHAKE_TIMEOUT_MS: i64 = 10 * SECOND_MS;

/// Hub: time for `hello` to arrive after the handshake.
pub const HELLO_TIMEOUT_MS: i64 = 5 * SECOND_MS;

/// Hub: time for each answer of the connector before the session.
pub const CONNECTOR_REPLY_TIMEOUT_MS: i64 = 10 * SECOND_MS;

/// Hub: time for `check_ok` after `check`.
pub const CHECK_REPLY_TIMEOUT_MS: i64 = 5 * SECOND_MS;

/// Hub: time the older connection of the same device has to answer the probe.
pub const OLD_CONNECTION_PROBE_TIMEOUT_MS: i64 = 3 * SECOND_MS;

/// Hub: time for `saved` after `credential`, in any exchange.
pub const SAVED_TIMEOUT_MS: i64 = 60 * SECOND_MS;

/// Hub: time a connection has to become a session, apart from the wait for approval.
pub const BECOME_SESSION_TIMEOUT_MS: i64 = 90 * SECOND_MS;

/// Receiver: an open stream without a new chunk for this long closes the connection.
pub const STREAM_IDLE_TIMEOUT_MS: i64 = 60 * SECOND_MS;

/// Hub: time it waits for calls in progress during an orderly stop.
pub const SHUTDOWN_GRACE_MS: i64 = 8 * SECOND_MS;

/// Hub: interval of the credential exchange inside an open session.
pub const ROTATION_INTERVAL_MS: i64 = DAY_MS;

/// Connector: interval at which a connector that is too old presents itself again, after the
/// presentation at the start of the leader.
pub const OUTDATED_PRESENTATION_INTERVAL_MS: i64 = DAY_MS;

/// Both sides: interval between `ping`s, from `hello_ok` on.
pub const PING_INTERVAL_MS: i64 = 30 * SECOND_MS;

/// Both sides: time without receiving any valid frame that ends the connection.
pub const SILENCE_TIMEOUT_MS: i64 = 90 * SECOND_MS;

/// Connector: wait for `hello_ok`.
pub const WAIT_HELLO_OK_MS: i64 = 10 * SECOND_MS;

/// Connector: wait for `state`.
pub const WAIT_STATE_MS: i64 = 10 * SECOND_MS;

/// Connector: wait for `check` after `present`.
pub const WAIT_CHECK_MS: i64 = 15 * SECOND_MS;

/// Connector: wait for `credential` after `check_ok`.
pub const WAIT_CREDENTIAL_MS: i64 = 20 * SECOND_MS;

/// Connector: wait for `session` after `saved`.
pub const WAIT_SESSION_MS: i64 = 20 * SECOND_MS;

/// Connector: wait for `enrolled`.
pub const WAIT_ENROLLED_MS: i64 = 15 * SECOND_MS;

/// Connector: wait for `left`.
pub const WAIT_LEFT_MS: i64 = 10 * SECOND_MS;

/// Connector: pause before a new attempt with the same identity, after closing one in which
/// `present` or `enroll` was already sent.
pub const SAME_IDENTITY_RETRY_GAP_MS: i64 = 6 * SECOND_MS;

/// Connector: first ceiling of the random reconnection wait. It doubles at every failure.
pub const RECONNECT_BACKOFF_MIN_MS: i64 = SECOND_MS;

/// Connector: last ceiling of the random reconnection wait.
pub const RECONNECT_BACKOFF_MAX_MS: i64 = 60 * SECOND_MS;

/// Connector: shortest random wait after AR-205 (the hub is stopping or out of resources).
pub const HUB_STOPPING_RETRY_MIN_MS: i64 = SECOND_MS;

/// Connector: longest random wait after AR-205.
pub const HUB_STOPPING_RETRY_MAX_MS: i64 = 10 * SECOND_MS;

/// Connector: shortest random wait after AR-206 (attempt replaced by a newer one).
pub const REPLACED_RETRY_MIN_MS: i64 = 5 * SECOND_MS;

/// Connector: longest random wait after AR-206.
pub const REPLACED_RETRY_MAX_MS: i64 = 15 * SECOND_MS;

/// Every deadline of the channel, in milliseconds.
///
/// [`Deadlines::default`] holds the documented values. Tests build their own with smaller
/// numbers; nothing in the protocol reads the constants directly once a `Deadlines` is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deadlines {
    /// Hub: time for the request headers to arrive.
    pub http_headers_ms: i64,
    /// Hub: time for the handshake to end, counted from the opening of the websocket.
    pub handshake_ms: i64,
    /// Hub: time for `hello` to arrive after the handshake.
    pub hello_ms: i64,
    /// Hub: time for each answer of the connector before the session.
    pub connector_reply_ms: i64,
    /// Hub: time for `check_ok` after `check`.
    pub check_reply_ms: i64,
    /// Hub: time the older connection of the same device has to answer the probe.
    pub old_connection_probe_ms: i64,
    /// Hub: time for `saved` after `credential`.
    pub saved_ms: i64,
    /// Hub: time a connection has to become a session, apart from the wait for approval.
    pub become_session_ms: i64,
    /// Receiver: idle time of an open stream that closes the connection.
    pub stream_idle_ms: i64,
    /// Hub: time it waits for calls in progress during an orderly stop.
    pub shutdown_grace_ms: i64,
    /// Hub: interval of the credential exchange inside an open session.
    pub rotation_interval_ms: i64,
    /// Connector: interval at which a connector that is too old presents itself again.
    pub outdated_presentation_interval_ms: i64,
    /// Both sides: interval between `ping`s.
    pub ping_interval_ms: i64,
    /// Both sides: time without receiving anything that ends the connection.
    pub silence_ms: i64,
    /// Connector: wait for `hello_ok`.
    pub wait_hello_ok_ms: i64,
    /// Connector: wait for `state`.
    pub wait_state_ms: i64,
    /// Connector: wait for `check` after `present`.
    pub wait_check_ms: i64,
    /// Connector: wait for `credential` after `check_ok`.
    pub wait_credential_ms: i64,
    /// Connector: wait for `session` after `saved`.
    pub wait_session_ms: i64,
    /// Connector: wait for `enrolled`.
    pub wait_enrolled_ms: i64,
    /// Connector: wait for `left`.
    pub wait_left_ms: i64,
    /// Connector: pause before a new attempt with the same identity.
    pub same_identity_retry_gap_ms: i64,
    /// Connector: first ceiling of the random reconnection wait.
    pub reconnect_backoff_min_ms: i64,
    /// Connector: last ceiling of the random reconnection wait.
    pub reconnect_backoff_max_ms: i64,
    /// Connector: shortest random wait after AR-205.
    pub hub_stopping_retry_min_ms: i64,
    /// Connector: longest random wait after AR-205.
    pub hub_stopping_retry_max_ms: i64,
    /// Connector: shortest random wait after AR-206.
    pub replaced_retry_min_ms: i64,
    /// Connector: longest random wait after AR-206.
    pub replaced_retry_max_ms: i64,
}

impl Deadlines {
    /// The documented values.
    pub const DOCUMENTED: Deadlines = Deadlines {
        http_headers_ms: HTTP_HEADERS_TIMEOUT_MS,
        handshake_ms: HANDSHAKE_TIMEOUT_MS,
        hello_ms: HELLO_TIMEOUT_MS,
        connector_reply_ms: CONNECTOR_REPLY_TIMEOUT_MS,
        check_reply_ms: CHECK_REPLY_TIMEOUT_MS,
        old_connection_probe_ms: OLD_CONNECTION_PROBE_TIMEOUT_MS,
        saved_ms: SAVED_TIMEOUT_MS,
        become_session_ms: BECOME_SESSION_TIMEOUT_MS,
        stream_idle_ms: STREAM_IDLE_TIMEOUT_MS,
        shutdown_grace_ms: SHUTDOWN_GRACE_MS,
        rotation_interval_ms: ROTATION_INTERVAL_MS,
        outdated_presentation_interval_ms: OUTDATED_PRESENTATION_INTERVAL_MS,
        ping_interval_ms: PING_INTERVAL_MS,
        silence_ms: SILENCE_TIMEOUT_MS,
        wait_hello_ok_ms: WAIT_HELLO_OK_MS,
        wait_state_ms: WAIT_STATE_MS,
        wait_check_ms: WAIT_CHECK_MS,
        wait_credential_ms: WAIT_CREDENTIAL_MS,
        wait_session_ms: WAIT_SESSION_MS,
        wait_enrolled_ms: WAIT_ENROLLED_MS,
        wait_left_ms: WAIT_LEFT_MS,
        same_identity_retry_gap_ms: SAME_IDENTITY_RETRY_GAP_MS,
        reconnect_backoff_min_ms: RECONNECT_BACKOFF_MIN_MS,
        reconnect_backoff_max_ms: RECONNECT_BACKOFF_MAX_MS,
        hub_stopping_retry_min_ms: HUB_STOPPING_RETRY_MIN_MS,
        hub_stopping_retry_max_ms: HUB_STOPPING_RETRY_MAX_MS,
        replaced_retry_min_ms: REPLACED_RETRY_MIN_MS,
        replaced_retry_max_ms: REPLACED_RETRY_MAX_MS,
    };
}

impl Default for Deadlines {
    fn default() -> Self {
        Self::DOCUMENTED
    }
}

// ---------------------------------------------------------------------------------------------
// Messages between Hermes agents
// ---------------------------------------------------------------------------------------------

/// Largest text of a message between Hermes agents, in UTF-8 bytes.
pub const PEER_TEXT_MAX_BYTES: usize = 16_384;

/// Messages one sender may send per minute.
pub const PEER_MESSAGES_PER_SENDER_PER_MINUTE: u32 = 30;

/// Messages one sender may send per day.
pub const PEER_MESSAGES_PER_SENDER_PER_DAY: u32 = 500;

/// Bytes of text one sender may send per day (4 MiB).
pub const PEER_BYTES_PER_SENDER_PER_DAY: u64 = 4 * 1024 * 1024;

/// Messages per link, per hour.
pub const PEER_MESSAGES_PER_LINK_PER_HOUR: u32 = 30;

/// New threads per link, per hour.
pub const PEER_NEW_THREADS_PER_LINK_PER_HOUR: u32 = 5;

/// Unread messages per link. Above it the box of the recipient is full.
pub const PEER_UNREAD_PER_LINK_MAX: u32 = 200;

/// Notices per link, per hour, adding every thread. Above it the message is kept without notice.
pub const PEER_NOTICES_PER_LINK_PER_HOUR: u32 = 10;

/// Message notices per recipient, per hour, adding every sender.
pub const PEER_NOTICES_PER_RECIPIENT_PER_HOUR: u32 = 30;

/// Wake-ups the connector accepts per minute, of any kind.
pub const WAKES_PER_MINUTE: u32 = 30;

/// Wake-ups the connector accepts per hour, of any kind.
pub const WAKES_PER_HOUR: u32 = 300;

/// Hub: wait before sending again a notice the connector answered with `throttled`.
pub const THROTTLED_RETRY_MS: i64 = 5 * MINUTE_MS;

/// Hub: shortest interval between two notices of the same sender to the same recipient.
pub const PEER_NOTICE_GAP_MS: i64 = 10 * SECOND_MS;

/// Budget of one thread read, in bytes (64 KiB). The answer is cut in whole messages.
pub const PEER_READ_BUDGET_BYTES: usize = 64 * 1024;

/// `hermes_list_threads`: default `limit`.
pub const PEER_LIST_THREADS_DEFAULT: u32 = 20;

/// `hermes_list_threads`: largest `limit`.
pub const PEER_LIST_THREADS_MAX: u32 = 50;

/// `hermes_read_thread`: default `limit`.
pub const PEER_READ_THREAD_DEFAULT: u32 = 30;

/// `hermes_read_thread`: largest `limit`.
pub const PEER_READ_THREAD_MAX: u32 = 100;

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers of the documents, written a second time: a constant edited by accident
    /// fails here.
    #[test]
    fn sizes_and_counts_match_the_documents() {
        assert_eq!(WS_MESSAGE_MAX, 65_535);
        assert_eq!(
            (
                HANDSHAKE_FIRST_LEN,
                HANDSHAKE_SECOND_LEN,
                HANDSHAKE_THIRD_LEN
            ),
            (48, 48, 64)
        );
        assert_eq!(HANDSHAKE_HASH_LEN, 32);
        assert_eq!(FRAME_HEADER_LEN, 5);
        assert_eq!(FRAME_CHUNK_MAX, 60_000);
        assert_eq!(FRAME_PLAINTEXT_MAX, 60_005);
        assert_eq!(MAX_OPEN_STREAMS, 8);
        assert_eq!(PRE_SESSION_MESSAGE_MAX, 4_096);
        assert_eq!(CONNECTOR_MESSAGE_MAX, 6_291_456);
        assert_eq!(HUB_MESSAGE_MAX, 16_777_216);
        assert_eq!(UNKNOWN_MESSAGES_MAX, 100);
        // Ceilings the documents do not have; see the comment of each constant.
        assert_eq!((MESSAGE_VALUES_MAX, MESSAGE_DEPTH_MAX), (100_000, 64));
        assert_eq!(CALLS_IN_FLIGHT_MAX, 32);
        assert_eq!(REASSEMBLY_BUDGET_BYTES, 268_435_456);
        assert_eq!(CONNECTIONS_WITHOUT_SESSION_MAX, 1_024);
        assert_eq!(CONNECTIONS_WITHOUT_SESSION_PER_ORIGIN_MAX, 16);
        assert_eq!(CREDENTIAL_MAX_LEN, 1_024);
        assert_eq!((ENROLL_NONCE_LEN, CHECK_NONCE_LEN), (32, 16));
        assert_eq!(DECLARED_TEXT_MAX_CHARS, 128);
        assert_eq!(ENROLLMENTS_PER_REQUEST_MAX, 5);
        assert_eq!(RETRY_AFTER_MAX_S, 86_400);
        assert_eq!(NOTICE_COUNT_MAX, 9_999);
        assert_eq!((NAME_MAX_LEN, CHAT_ID_MAX_LEN), (32, 64));
        assert_eq!((INVITE_LEN, INVITE_TEXT_LEN), (32, 56));
    }

    #[test]
    fn default_deadlines_match_the_documents() {
        let d = Deadlines::default();
        assert_eq!(d, Deadlines::DOCUMENTED);
        assert_eq!(d.http_headers_ms, 5_000);
        assert_eq!(d.handshake_ms, 10_000);
        assert_eq!(d.hello_ms, 5_000);
        assert_eq!(d.connector_reply_ms, 10_000);
        assert_eq!(d.check_reply_ms, 5_000);
        assert_eq!(d.old_connection_probe_ms, 3_000);
        assert_eq!(d.saved_ms, 60_000);
        assert_eq!(d.become_session_ms, 90_000);
        assert_eq!(d.stream_idle_ms, 60_000);
        assert_eq!(d.shutdown_grace_ms, 8_000);
        assert_eq!(d.rotation_interval_ms, 86_400_000);
        assert_eq!(d.outdated_presentation_interval_ms, 86_400_000);
        assert_eq!(d.ping_interval_ms, 30_000);
        assert_eq!(d.silence_ms, 90_000);
        assert_eq!(d.wait_hello_ok_ms, 10_000);
        assert_eq!(d.wait_state_ms, 10_000);
        assert_eq!(d.wait_check_ms, 15_000);
        assert_eq!(d.wait_credential_ms, 20_000);
        assert_eq!(d.wait_session_ms, 20_000);
        assert_eq!(d.wait_enrolled_ms, 15_000);
        assert_eq!(d.wait_left_ms, 10_000);
        assert_eq!(d.same_identity_retry_gap_ms, 6_000);
        assert_eq!(
            (d.reconnect_backoff_min_ms, d.reconnect_backoff_max_ms),
            (1_000, 60_000)
        );
        assert_eq!(
            (d.hub_stopping_retry_min_ms, d.hub_stopping_retry_max_ms),
            (1_000, 10_000)
        );
        assert_eq!(
            (d.replaced_retry_min_ms, d.replaced_retry_max_ms),
            (5_000, 15_000)
        );
    }

    /// The connector always waits longer than the hub takes: otherwise it would give up on an
    /// attempt the hub is still about to answer.
    #[test]
    fn connector_waits_outlast_the_hub() {
        let d = Deadlines::default();
        // `check` comes right after `present`; the hub then waits `check_ok` for 5 s.
        assert!(d.wait_check_ms > d.check_reply_ms);
        // Between `check_ok` and `credential` the hub may probe an older connection.
        assert!(d.wait_credential_ms > d.old_connection_probe_ms + d.check_reply_ms);
        // The silence limit covers at least two missed pings.
        assert!(d.silence_ms >= 2 * d.ping_interval_ms);
        // The pause between attempts outlasts the hub's own wait for `check_ok`, so the answer
        // of an abandoned attempt never meets a newer one half way.
        assert!(d.same_identity_retry_gap_ms > d.check_reply_ms);
    }

    #[test]
    fn peer_ceilings_match_the_documents() {
        assert_eq!(PEER_TEXT_MAX_BYTES, 16_384);
        assert_eq!(PEER_MESSAGES_PER_SENDER_PER_MINUTE, 30);
        assert_eq!(PEER_MESSAGES_PER_SENDER_PER_DAY, 500);
        assert_eq!(PEER_BYTES_PER_SENDER_PER_DAY, 4_194_304);
        assert_eq!(PEER_MESSAGES_PER_LINK_PER_HOUR, 30);
        assert_eq!(PEER_NEW_THREADS_PER_LINK_PER_HOUR, 5);
        assert_eq!(PEER_UNREAD_PER_LINK_MAX, 200);
        assert_eq!(PEER_NOTICES_PER_LINK_PER_HOUR, 10);
        assert_eq!(PEER_NOTICES_PER_RECIPIENT_PER_HOUR, 30);
        assert_eq!((WAKES_PER_MINUTE, WAKES_PER_HOUR), (30, 300));
        assert_eq!(THROTTLED_RETRY_MS, 300_000);
        assert_eq!(PEER_NOTICE_GAP_MS, 10_000);
        assert_eq!(PEER_READ_BUDGET_BYTES, 65_536);
        assert_eq!((PEER_LIST_THREADS_DEFAULT, PEER_LIST_THREADS_MAX), (20, 50));
        assert_eq!((PEER_READ_THREAD_DEFAULT, PEER_READ_THREAD_MAX), (30, 100));
    }
}
