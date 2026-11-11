use std::sync::atomic::{AtomicUsize, Ordering};

use proptest::prelude::*;

use super::*;
use crate::clock::{Clock, ManualClock, SeededRng};
use crate::limits::{Deadlines, STREAM_IDLE_TIMEOUT_MS};

const IDLE: i64 = STREAM_IDLE_TIMEOUT_MS;

/// A budget with a ceiling, which remembers how much was taken at the worst moment.
#[derive(Debug)]
struct TestBudget {
    limit: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
}

impl TestBudget {
    fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        })
    }

    fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }

    fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}

impl ReassemblyBudget for TestBudget {
    fn try_reserve(&self, bytes: usize) -> bool {
        let used = self.used.load(Ordering::SeqCst);
        match used.checked_add(bytes) {
            Some(total) if total <= self.limit => {
                self.used.store(total, Ordering::SeqCst);
                self.peak.fetch_max(total, Ordering::SeqCst);
                true
            }
            _ => false,
        }
    }

    fn release(&self, bytes: usize) {
        let used = self.used.load(Ordering::SeqCst);
        assert!(
            bytes <= used,
            "released {bytes} bytes with only {used} taken"
        );
        self.used.store(used - bytes, Ordering::SeqCst);
    }
}

/// A frame built by hand, so that a test can send what a correct sender never would.
fn raw(flags: u8, stream: u32, chunk: &[u8]) -> Vec<u8> {
    let mut bytes = vec![flags];
    bytes.extend_from_slice(&stream.to_be_bytes());
    bytes.extend_from_slice(chunk);
    bytes
}

fn in_session(sender: Side) -> Reassembler<Unlimited> {
    let mut reassembler = Reassembler::new(sender, IDLE, Unlimited);
    reassembler.enter_session();
    reassembler
}

fn payload_of(delivered: Option<Delivered<'_>>) -> Vec<u8> {
    delivered
        .expect("a message should have been delivered")
        .payload
        .into_owned()
}

/// Pushes every frame of `message`, which must be delivered by the last one only.
fn push_message(
    reassembler: &mut Reassembler<impl ReassemblyBudget>,
    frames: Frames<'_>,
    now_ms: i64,
) -> Vec<u8> {
    let total = frames.len();
    let mut delivered = None;
    for (index, frame) in frames.enumerate() {
        let bytes = frame.to_bytes();
        let result = reassembler.push(&bytes, now_ms).unwrap();
        if index + 1 == total {
            delivered = Some(payload_of(result));
        } else {
            assert!(result.is_none(), "delivered before FIN");
        }
    }
    delivered.expect("a message has at least one frame")
}

fn pattern(len: usize, seed: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    SeededRng::new(seed).fill(&mut bytes);
    bytes
}

// ---------------------------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------------------------

#[test]
fn header_golden_bytes() {
    let cases: [(FrameHeader, [u8; 5]); 6] = [
        (
            FrameHeader {
                fin: true,
                control: false,
                stream: 0,
            },
            [0x01, 0, 0, 0, 0],
        ),
        (
            FrameHeader {
                fin: true,
                control: true,
                stream: 0,
            },
            [0x03, 0, 0, 0, 0],
        ),
        (
            FrameHeader {
                fin: false,
                control: false,
                stream: 1,
            },
            [0x00, 0, 0, 0, 1],
        ),
        (
            FrameHeader {
                fin: true,
                control: false,
                stream: 2,
            },
            [0x01, 0, 0, 0, 2],
        ),
        (
            FrameHeader {
                fin: false,
                control: false,
                stream: 0x0102_0304,
            },
            [0x00, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            FrameHeader {
                fin: true,
                control: false,
                stream: u32::MAX,
            },
            [0x01, 0xff, 0xff, 0xff, 0xff],
        ),
    ];
    for (header, bytes) in cases {
        assert_eq!(header.encode(), bytes, "{header:?}");
        let mut frame = bytes.to_vec();
        frame.extend_from_slice(b"chunk");
        assert_eq!(FrameHeader::decode(&frame), Ok((header, &b"chunk"[..])));
    }
    assert_eq!((FLAG_FIN, FLAG_CONTROL), (0x01, 0x02));
}

#[test]
fn header_needs_five_bytes() {
    for len in 0..5 {
        assert_eq!(
            FrameHeader::decode(&vec![1u8; len]),
            Err(FrameError::Truncated),
            "{len}"
        );
    }
    let (header, chunk) = FrameHeader::decode(&[1, 0, 0, 0, 0]).unwrap();
    assert_eq!(
        header,
        FrameHeader {
            fin: true,
            control: false,
            stream: 0
        }
    );
    assert!(chunk.is_empty());
}

#[test]
fn every_reserved_bit_is_refused() {
    for bit in 2..8 {
        let flags = FLAG_FIN | (1u8 << bit);
        assert_eq!(
            FrameHeader::decode(&raw(flags, 0, b"x")),
            Err(FrameError::ReservedBits),
            "bit {bit}"
        );
        let mut reassembler = in_session(Side::Connector);
        assert_eq!(
            reassembler.push(&raw(flags, 0, b"x"), 0),
            Err(FrameError::ReservedBits)
        );
        // The connection is over: even a good frame is refused now.
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN, 0, b"x"), 0),
            Err(FrameError::Closed)
        );
    }
    assert_eq!(
        FrameHeader::decode(&raw(0xff, 0, b"x")),
        Err(FrameError::ReservedBits)
    );
}

// ---------------------------------------------------------------------------------------------
// Splitter
// ---------------------------------------------------------------------------------------------

#[test]
fn a_message_of_one_frame_goes_on_stream_zero() {
    for side in [Side::Connector, Side::Hub] {
        let mut splitter = Splitter::new(side);
        splitter.enter_session();
        for len in [1usize, 2, 4_096, 59_999, 60_000] {
            let message = pattern(len, 1);
            let frames = splitter.split(&message, false).unwrap();
            assert_eq!(frames.stream(), 0);
            assert!(!frames.opens_stream());
            let frames: Vec<Frame<'_>> = frames.collect();
            assert_eq!(frames.len(), 1, "{len}");
            assert_eq!(
                frames[0].header,
                FrameHeader {
                    fin: true,
                    control: false,
                    stream: 0
                }
            );
            assert_eq!(frames[0].chunk, message.as_slice());
        }
    }
}

#[test]
fn a_larger_message_is_cut_on_a_stream_of_its_side() {
    for (side, first) in [(Side::Connector, 1u32), (Side::Hub, 2u32)] {
        let mut splitter = Splitter::new(side);
        splitter.enter_session();
        let message = pattern(60_001, 2);
        let frames = splitter.split(&message, false).unwrap();
        assert_eq!(frames.stream(), first);
        assert!(frames.opens_stream());
        assert_eq!(frames.len(), 2);
        let frames: Vec<Frame<'_>> = frames.collect();
        assert_eq!(
            frames[0].header,
            FrameHeader {
                fin: false,
                control: false,
                stream: first
            }
        );
        assert_eq!(frames[0].chunk.len(), 60_000);
        assert_eq!(
            frames[1].header,
            FrameHeader {
                fin: true,
                control: false,
                stream: first
            }
        );
        assert_eq!(frames[1].chunk.len(), 1);
    }
}

#[test]
fn stream_numbers_keep_their_parity_and_always_grow() {
    let big = pattern(120_001, 3);
    let small = pattern(10, 3);

    let mut connector = Splitter::new(Side::Connector);
    connector.enter_session();
    let mut hub = Splitter::new(Side::Hub);
    hub.enter_session();
    let mut from_connector = Vec::new();
    let mut from_hub = Vec::new();
    for round in 0..20 {
        // Single-frame messages in between use no stream number.
        assert_eq!(connector.split(&small, round % 2 == 0).unwrap().stream(), 0);
        from_connector.push(connector.split(&big, false).unwrap().stream());
        from_hub.push(hub.split(&big, false).unwrap().stream());
    }
    let expected_connector: Vec<u32> = (0..20).map(|i| 1 + 2 * i).collect();
    let expected_hub: Vec<u32> = (0..20).map(|i| 2 + 2 * i).collect();
    assert_eq!(from_connector, expected_connector);
    assert_eq!(from_hub, expected_hub);
}

#[test]
fn no_chunk_ever_passes_sixty_thousand_bytes() {
    let sizes = [
        1usize,
        60_000,
        60_001,
        119_999,
        120_000,
        120_001,
        1_000_000,
        CONNECTOR_MESSAGE_MAX - 1,
        CONNECTOR_MESSAGE_MAX,
    ];
    let mut splitter = Splitter::new(Side::Connector);
    splitter.enter_session();
    for size in sizes {
        let message = pattern(size, 4);
        let frames = splitter.split(&message, false).unwrap();
        let stream = frames.stream();
        assert_eq!(frames.len(), size.div_ceil(60_000));
        let mut joined = Vec::with_capacity(size);
        let mut count = 0;
        let total = frames.len();
        for frame in frames {
            count += 1;
            assert!(!frame.chunk.is_empty());
            assert!(
                frame.chunk.len() <= FRAME_CHUNK_MAX,
                "{size}: {}",
                frame.chunk.len()
            );
            assert_eq!(frame.header.stream, stream);
            assert_eq!(
                frame.header.fin,
                count == total,
                "only the last frame carries FIN"
            );
            assert!(!frame.header.control);
            assert_eq!(frame.to_bytes().len(), 5 + frame.chunk.len());
            assert!(frame.to_bytes().len() <= crate::limits::FRAME_PLAINTEXT_MAX);
            joined.extend_from_slice(frame.chunk);
        }
        assert_eq!(count, total);
        assert_eq!(joined, message, "{size}");
    }
}

#[test]
fn the_sender_checks_the_size_before_sending() {
    // Before the session: 4,096 bytes, in both directions.
    for side in [Side::Connector, Side::Hub] {
        let mut splitter = Splitter::new(side);
        assert_eq!(splitter.phase(), Phase::PreSession);
        assert!(
            splitter
                .split(&vec![0u8; PRE_SESSION_MESSAGE_MAX], false)
                .is_ok()
        );
        assert_eq!(
            splitter
                .split(&vec![0u8; PRE_SESSION_MESSAGE_MAX + 1], false)
                .err(),
            Some(SplitError::TooLarge)
        );
        assert_eq!(
            splitter
                .split(&vec![0u8; PRE_SESSION_MESSAGE_MAX + 1], true)
                .err(),
            Some(SplitError::TooLarge)
        );
    }
    // In session: 6 MiB from the connector, 16 MiB from the hub.
    for (side, max) in [
        (Side::Connector, CONNECTOR_MESSAGE_MAX),
        (Side::Hub, HUB_MESSAGE_MAX),
    ] {
        let mut splitter = Splitter::new(side);
        splitter.enter_session();
        assert_eq!(splitter.phase(), Phase::Session);
        let mut message = vec![0u8; max];
        assert!(splitter.split(&message, false).is_ok());
        message.push(0);
        assert_eq!(
            splitter.split(&message, false).err(),
            Some(SplitError::TooLarge)
        );
        // A refused message uses no stream number: the next one takes the following number.
        let next = splitter.split(&message[..60_001], false).unwrap().stream();
        assert_eq!(next, if side == Side::Connector { 3 } else { 4 });
    }
    assert_eq!(Side::Connector.session_message_max(), 6 * 1024 * 1024);
    assert_eq!(Side::Hub.session_message_max(), 16 * 1024 * 1024);
}

#[test]
fn an_empty_message_is_not_sent() {
    let mut splitter = Splitter::new(Side::Hub);
    assert_eq!(splitter.split(&[], false).err(), Some(SplitError::Empty));
    assert_eq!(splitter.split(&[], true).err(), Some(SplitError::Empty));
}

#[test]
fn a_control_message_is_one_frame_on_stream_zero() {
    let mut splitter = Splitter::new(Side::Connector);
    let frames: Vec<Frame<'_>> = splitter
        .split(b"{\"t\":\"ping\",\"n\":1}", true)
        .unwrap()
        .collect();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0].header,
        FrameHeader {
            fin: true,
            control: true,
            stream: 0
        }
    );
    assert_eq!(frames[0].to_bytes()[0], 0x03);

    splitter.enter_session();
    assert!(splitter.split(&vec![0u8; FRAME_CHUNK_MAX], true).is_ok());
    assert_eq!(
        splitter.split(&vec![0u8; FRAME_CHUNK_MAX + 1], true).err(),
        Some(SplitError::ControlTooLarge)
    );
    // The refusal used no stream number.
    assert_eq!(
        splitter
            .split(&vec![0u8; FRAME_CHUNK_MAX + 1], false)
            .unwrap()
            .stream(),
        1
    );
}

#[test]
fn stream_numbers_run_out_instead_of_wrapping() {
    let big = vec![0u8; 60_001];

    let mut connector = Splitter::starting_at(Side::Connector, u32::MAX - 2);
    assert_eq!(connector.split(&big, false).unwrap().stream(), u32::MAX - 2);
    assert_eq!(connector.split(&big, false).unwrap().stream(), u32::MAX);
    assert_eq!(
        connector.split(&big, false).err(),
        Some(SplitError::StreamsExhausted)
    );
    assert_eq!(
        connector.split(&big, false).err(),
        Some(SplitError::StreamsExhausted)
    );
    // Single-frame messages still go: they need no number.
    assert!(connector.split(b"x", false).is_ok());

    let mut hub = Splitter::starting_at(Side::Hub, u32::MAX - 3);
    assert_eq!(hub.split(&big, false).unwrap().stream(), u32::MAX - 3);
    assert_eq!(hub.split(&big, false).unwrap().stream(), u32::MAX - 1);
    assert_eq!(
        hub.split(&big, false).err(),
        Some(SplitError::StreamsExhausted)
    );
}

// ---------------------------------------------------------------------------------------------
// Reassembler: the table of rules
// ---------------------------------------------------------------------------------------------

#[test]
fn single_frame_messages_are_delivered_at_once_and_open_nothing() {
    for sender in [Side::Connector, Side::Hub] {
        let mut reassembler = in_session(sender);
        for round in 0..50u8 {
            let frame = raw(FLAG_FIN, 0, &[round, 1, 2]);
            let delivered = reassembler.push(&frame, 0).unwrap().unwrap();
            assert!(!delivered.control);
            assert!(
                matches!(delivered.payload, Cow::Borrowed(_)),
                "no copy for a single frame"
            );
            assert_eq!(delivered.payload.as_ref(), [round, 1, 2]);
            assert_eq!(reassembler.open_streams(), 0);
        }
        assert_eq!(reassembler.next_deadline_ms(), None);
    }
}

#[test]
fn an_empty_chunk_closes_the_connection() {
    for (flags, stream) in [(FLAG_FIN, 0u32), (0, 1), (FLAG_FIN | FLAG_CONTROL, 0)] {
        let mut reassembler = in_session(Side::Connector);
        assert_eq!(
            reassembler.push(&raw(flags, stream, b""), 0),
            Err(FrameError::EmptyChunk)
        );
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN, 0, b"x"), 0),
            Err(FrameError::Closed)
        );
    }
    // An empty chunk in the middle of a message is refused as well.
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"abc"), 0), Ok(None));
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 1, b""), 0),
        Err(FrameError::EmptyChunk)
    );
}

#[test]
fn a_truncated_frame_closes_the_connection() {
    for len in 0..5 {
        let mut reassembler = in_session(Side::Hub);
        assert_eq!(
            reassembler.push(&vec![1u8; len], 0),
            Err(FrameError::Truncated)
        );
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN, 0, b"x"), 0),
            Err(FrameError::Closed)
        );
    }
}

#[test]
fn a_chunk_above_the_limit_closes_the_connection() {
    let mut reassembler = in_session(Side::Hub);
    assert!(
        reassembler
            .push(&raw(FLAG_FIN, 0, &vec![7u8; FRAME_CHUNK_MAX]), 0)
            .is_ok()
    );
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 0, &vec![7u8; FRAME_CHUNK_MAX + 1]), 0),
        Err(FrameError::ChunkTooLarge)
    );

    let mut reassembler = in_session(Side::Hub);
    assert_eq!(
        reassembler.push(&raw(0, 2, &vec![7u8; FRAME_CHUNK_MAX + 1]), 0),
        Err(FrameError::ChunkTooLarge)
    );
    let mut reassembler = in_session(Side::Hub);
    assert_eq!(
        reassembler.push(&raw(0, 2, &vec![7u8; FRAME_CHUNK_MAX]), 0),
        Ok(None)
    );
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 2, &vec![7u8; FRAME_CHUNK_MAX + 1]), 0),
        Err(FrameError::ChunkTooLarge)
    );
}

#[test]
fn the_wrong_parity_closes_the_connection() {
    // The hub receives from the connector: only odd numbers.
    let mut at_hub = in_session(Side::Connector);
    assert_eq!(
        at_hub.push(&raw(0, 2, b"x"), 0),
        Err(FrameError::WrongParity)
    );
    assert_eq!(at_hub.push(&raw(0, 1, b"x"), 0), Err(FrameError::Closed));

    // The connector receives from the hub: only even numbers.
    let mut at_connector = in_session(Side::Hub);
    assert_eq!(
        at_connector.push(&raw(0, 1, b"x"), 0),
        Err(FrameError::WrongParity)
    );

    for stream in [4u32, 1_000, u32::MAX - 1] {
        let mut reassembler = in_session(Side::Connector);
        assert_eq!(
            reassembler.push(&raw(0, stream, b"x"), 0),
            Err(FrameError::WrongParity)
        );
    }
    for stream in [3u32, 1_001, u32::MAX] {
        let mut reassembler = in_session(Side::Hub);
        assert_eq!(
            reassembler.push(&raw(0, stream, b"x"), 0),
            Err(FrameError::WrongParity)
        );
    }
    // The right parity is accepted at both ends of the range.
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, u32::MAX, b"x"), 0), Ok(None));
    let mut reassembler = in_session(Side::Hub);
    assert_eq!(reassembler.push(&raw(0, u32::MAX - 1, b"x"), 0), Ok(None));
}

#[test]
fn a_repeated_stream_number_closes_the_connection() {
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"ab"), 0), Ok(None));
    assert_eq!(
        payload_of(reassembler.push(&raw(FLAG_FIN, 1, b"cd"), 0).unwrap()),
        b"abcd"
    );
    // Stream 1 was delivered and closed. Its number never comes back.
    assert_eq!(
        reassembler.push(&raw(0, 1, b"ab"), 0),
        Err(FrameError::StreamNotIncreasing)
    );
    assert_eq!(
        reassembler.push(&raw(0, 3, b"ab"), 0),
        Err(FrameError::Closed)
    );
}

#[test]
fn a_lower_stream_number_closes_the_connection() {
    let mut reassembler = in_session(Side::Connector);
    // Gaps are fine: the numbers only have to grow.
    assert_eq!(reassembler.push(&raw(0, 5, b"ab"), 0), Ok(None));
    assert_eq!(reassembler.push(&raw(0, 9, b"ab"), 0), Ok(None));
    // 7 was never used, but it is below the last one opened.
    assert_eq!(
        reassembler.push(&raw(0, 7, b"ab"), 0),
        Err(FrameError::StreamNotIncreasing)
    );

    let mut reassembler = in_session(Side::Hub);
    assert_eq!(reassembler.push(&raw(0, 8, b"ab"), 0), Ok(None));
    assert_eq!(
        reassembler.push(&raw(0, 2, b"ab"), 0),
        Err(FrameError::StreamNotIncreasing)
    );
}

#[test]
fn streams_interleave_and_each_message_is_delivered_at_its_fin() {
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"one-"), 0), Ok(None));
    assert_eq!(reassembler.push(&raw(0, 3, b"three-"), 0), Ok(None));
    assert_eq!(reassembler.open_streams(), 2);
    // A single-frame message passes between the chunks.
    assert_eq!(
        payload_of(reassembler.push(&raw(FLAG_FIN, 0, b"zero"), 0).unwrap()),
        b"zero"
    );
    assert_eq!(reassembler.push(&raw(0, 1, b"more-"), 0), Ok(None));
    // Stream 3 ends first, although it was opened later.
    let last_of_three = raw(FLAG_FIN, 3, b"end");
    let third = reassembler.push(&last_of_three, 0).unwrap().unwrap();
    assert!(!third.control);
    assert!(matches!(third.payload, Cow::Owned(_)));
    assert_eq!(third.payload.as_ref(), b"three-end");
    assert_eq!(reassembler.open_streams(), 1);
    assert_eq!(
        payload_of(reassembler.push(&raw(FLAG_FIN, 1, b"end"), 0).unwrap()),
        b"one-more-end"
    );
    assert_eq!(reassembler.open_streams(), 0);
}

#[test]
fn control_is_a_single_frame_on_stream_zero() {
    let mut reassembler = in_session(Side::Connector);
    let ping = raw(FLAG_FIN | FLAG_CONTROL, 0, b"ping");
    let delivered = reassembler.push(&ping, 0).unwrap().unwrap();
    assert!(delivered.control);
    assert_eq!(delivered.payload.as_ref(), b"ping");
    // Control passes while streams are open.
    assert_eq!(reassembler.push(&raw(0, 1, b"data"), 0), Ok(None));
    assert!(
        reassembler
            .push(&raw(FLAG_FIN | FLAG_CONTROL, 0, b"pong"), 0)
            .unwrap()
            .unwrap()
            .control
    );

    // Without FIN.
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(
        reassembler.push(&raw(FLAG_CONTROL, 0, b"ping"), 0),
        Err(FrameError::ControlNotSingleFrame)
    );
    // On another stream, opening it.
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(
        reassembler.push(&raw(FLAG_CONTROL, 1, b"ping"), 0),
        Err(FrameError::ControlNotSingleFrame)
    );
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN | FLAG_CONTROL, 1, b"ping"), 0),
        Err(FrameError::ControlNotSingleFrame)
    );
    // On a stream that is already open.
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"data"), 0), Ok(None));
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN | FLAG_CONTROL, 1, b"ping"), 0),
        Err(FrameError::ControlNotSingleFrame)
    );
}

#[test]
fn stream_zero_only_carries_whole_messages() {
    let mut reassembler = in_session(Side::Hub);
    assert_eq!(
        reassembler.push(&raw(0, 0, b"half"), 0),
        Err(FrameError::StreamZeroWithoutFin)
    );
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 0, b"rest"), 0),
        Err(FrameError::Closed)
    );
}

#[test]
fn a_single_frame_message_on_another_stream_closes_the_connection() {
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 1, b"whole"), 0),
        Err(FrameError::SingleFrameOnStream)
    );
    let mut reassembler = in_session(Side::Hub);
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 2, b"whole"), 0),
        Err(FrameError::SingleFrameOnStream)
    );
}

// ---------------------------------------------------------------------------------------------
// Reassembler: the limits of each phase
// ---------------------------------------------------------------------------------------------

#[test]
fn before_the_session_only_stream_zero_exists() {
    for sender in [Side::Connector, Side::Hub] {
        let own = if sender == Side::Connector { 1 } else { 2 };
        let mut reassembler = Reassembler::new(sender, IDLE, Unlimited);
        assert_eq!(reassembler.phase(), Phase::PreSession);
        assert!(
            reassembler
                .push(&raw(FLAG_FIN, 0, b"hello"), 0)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            reassembler.push(&raw(0, own, b"x"), 0),
            Err(FrameError::StreamBeforeSession)
        );
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN, 0, b"hello"), 0),
            Err(FrameError::Closed)
        );

        // The same frame is fine once the session has started.
        let mut reassembler = Reassembler::new(sender, IDLE, Unlimited);
        reassembler.enter_session();
        assert_eq!(reassembler.phase(), Phase::Session);
        assert_eq!(reassembler.push(&raw(0, own, b"x"), 0), Ok(None));
    }
}

#[test]
fn before_the_session_a_message_has_at_most_4096_bytes() {
    for sender in [Side::Connector, Side::Hub] {
        let mut reassembler = Reassembler::new(sender, IDLE, Unlimited);
        let largest = vec![1u8; PRE_SESSION_MESSAGE_MAX];
        assert_eq!(
            payload_of(reassembler.push(&raw(FLAG_FIN, 0, &largest), 0).unwrap()),
            largest
        );
        // Control messages have the same limit.
        assert!(
            reassembler
                .push(&raw(FLAG_FIN | FLAG_CONTROL, 0, &largest), 0)
                .is_ok()
        );
        let over = vec![1u8; PRE_SESSION_MESSAGE_MAX + 1];
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN, 0, &over), 0),
            Err(FrameError::MessageTooLarge)
        );

        let mut reassembler = Reassembler::new(sender, IDLE, Unlimited);
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN | FLAG_CONTROL, 0, &over), 0),
            Err(FrameError::MessageTooLarge)
        );

        // In session the same message is small.
        let mut reassembler = Reassembler::new(sender, IDLE, Unlimited);
        reassembler.enter_session();
        assert!(reassembler.push(&raw(FLAG_FIN, 0, &over), 0).is_ok());
    }
}

/// Sends `total` bytes on one stream, in full chunks, and returns what the last frame gave.
fn push_total(
    reassembler: &mut Reassembler<impl ReassemblyBudget>,
    stream: u32,
    total: usize,
) -> Result<Option<usize>, FrameError> {
    let chunk = vec![0x5au8; FRAME_CHUNK_MAX];
    let mut sent = 0;
    loop {
        let size = (total - sent).min(FRAME_CHUNK_MAX);
        sent += size;
        let flags = if sent == total { FLAG_FIN } else { 0 };
        let frame = raw(flags, stream, &chunk[..size]);
        let delivered = reassembler
            .push(&frame, 0)?
            .map(|message| message.payload.len());
        if sent == total {
            return Ok(delivered);
        }
        assert!(delivered.is_none());
    }
}

#[test]
fn in_session_the_connector_sends_at_most_six_mebibytes() {
    let mut at_hub = in_session(Side::Connector);
    assert_eq!(
        push_total(&mut at_hub, 1, CONNECTOR_MESSAGE_MAX),
        Ok(Some(CONNECTOR_MESSAGE_MAX))
    );
    assert_eq!(
        push_total(&mut at_hub, 3, CONNECTOR_MESSAGE_MAX + 1),
        Err(FrameError::MessageTooLarge)
    );
    assert_eq!(
        at_hub.push(&raw(FLAG_FIN, 0, b"x"), 0),
        Err(FrameError::Closed)
    );
}

#[test]
fn in_session_the_hub_sends_at_most_sixteen_mebibytes() {
    let mut at_connector = in_session(Side::Hub);
    assert_eq!(
        push_total(&mut at_connector, 2, HUB_MESSAGE_MAX),
        Ok(Some(HUB_MESSAGE_MAX))
    );
    assert_eq!(
        push_total(&mut at_connector, 4, HUB_MESSAGE_MAX + 1),
        Err(FrameError::MessageTooLarge)
    );
    // What the connector may not send, the hub may: the limits are per direction.
    let mut at_connector = in_session(Side::Hub);
    assert_eq!(
        push_total(&mut at_connector, 2, CONNECTOR_MESSAGE_MAX + 1),
        Ok(Some(CONNECTOR_MESSAGE_MAX + 1))
    );
}

#[test]
fn the_size_limit_is_enforced_as_the_bytes_arrive() {
    // The message is refused at the chunk that crosses the limit, without waiting for FIN.
    let mut at_hub = in_session(Side::Connector);
    let chunk = vec![1u8; FRAME_CHUNK_MAX];
    let full_chunks = CONNECTOR_MESSAGE_MAX / FRAME_CHUNK_MAX;
    for _ in 0..full_chunks {
        assert_eq!(at_hub.push(&raw(0, 1, &chunk), 0), Ok(None));
    }
    let left = CONNECTOR_MESSAGE_MAX - full_chunks * FRAME_CHUNK_MAX;
    assert_eq!(at_hub.push(&raw(0, 1, &chunk[..left]), 0), Ok(None));
    assert_eq!(
        at_hub.push(&raw(0, 1, &chunk[..1]), 0),
        Err(FrameError::MessageTooLarge)
    );
}

#[test]
fn at_most_eight_streams_are_open_at_once() {
    for (sender, first) in [(Side::Connector, 1u32), (Side::Hub, 2u32)] {
        let mut reassembler = in_session(sender);
        for index in 0..8 {
            assert_eq!(
                reassembler.push(&raw(0, first + 2 * index, b"x"), 0),
                Ok(None)
            );
        }
        assert_eq!(reassembler.open_streams(), MAX_OPEN_STREAMS);
        // Stream 0 does not count.
        assert!(
            reassembler
                .push(&raw(FLAG_FIN, 0, b"x"), 0)
                .unwrap()
                .is_some()
        );
        assert!(
            reassembler
                .push(&raw(FLAG_FIN | FLAG_CONTROL, 0, b"x"), 0)
                .unwrap()
                .is_some()
        );
        // The ninth does.
        assert_eq!(
            reassembler.push(&raw(0, first + 16, b"x"), 0),
            Err(FrameError::TooManyStreams)
        );
        assert_eq!(
            reassembler.push(&raw(FLAG_FIN, 0, b"x"), 0),
            Err(FrameError::Closed)
        );
    }
}

#[test]
fn a_finished_stream_makes_room_for_another() {
    let mut reassembler = in_session(Side::Connector);
    for index in 0..8u32 {
        assert_eq!(reassembler.push(&raw(0, 1 + 2 * index, b"x"), 0), Ok(None));
    }
    assert!(
        reassembler
            .push(&raw(FLAG_FIN, 5, b"y"), 0)
            .unwrap()
            .is_some()
    );
    assert_eq!(reassembler.open_streams(), 7);
    assert_eq!(reassembler.push(&raw(0, 17, b"x"), 0), Ok(None));
    assert_eq!(reassembler.open_streams(), 8);
    assert_eq!(
        reassembler.push(&raw(0, 19, b"x"), 0),
        Err(FrameError::TooManyStreams)
    );
}

#[test]
fn an_idle_stream_closes_the_connection() {
    let start = 1_000_000;
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.next_deadline_ms(), None);
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), start), Ok(None));
    assert_eq!(reassembler.next_deadline_ms(), Some(start + IDLE));
    // One millisecond before the limit nothing happens.
    assert_eq!(reassembler.check_idle(start + IDLE - 1), Ok(()));
    assert!(
        reassembler
            .push(&raw(FLAG_FIN, 0, b"x"), start + IDLE - 1)
            .is_ok()
    );
    // At the limit the connection is over, whatever arrives.
    assert_eq!(
        reassembler.check_idle(start + IDLE),
        Err(FrameError::StreamIdle)
    );
    assert_eq!(
        reassembler.check_idle(start + IDLE),
        Err(FrameError::Closed)
    );
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 0, b"x"), start + IDLE),
        Err(FrameError::Closed)
    );
}

#[test]
fn a_late_chunk_does_not_rescue_its_stream() {
    let mut reassembler = in_session(Side::Hub);
    assert_eq!(reassembler.push(&raw(0, 2, b"x"), 0), Ok(None));
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 2, b"y"), IDLE),
        Err(FrameError::StreamIdle)
    );
}

#[test]
fn a_chunk_renews_only_its_own_stream() {
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), 0), Ok(None));
    assert_eq!(reassembler.push(&raw(0, 3, b"x"), 10_000), Ok(None));
    assert_eq!(reassembler.next_deadline_ms(), Some(IDLE));
    // Stream 1 gets a chunk and a new deadline; stream 3 keeps the old one.
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), 50_000), Ok(None));
    assert_eq!(reassembler.next_deadline_ms(), Some(10_000 + IDLE));
    // Frames on stream 0 renew nothing.
    assert!(reassembler.push(&raw(FLAG_FIN, 0, b"x"), 69_999).is_ok());
    assert_eq!(reassembler.next_deadline_ms(), Some(10_000 + IDLE));
    assert_eq!(
        reassembler.push(&raw(0, 1, b"x"), 70_000),
        Err(FrameError::StreamIdle)
    );
}

#[test]
fn a_delivered_stream_has_no_deadline() {
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), 0), Ok(None));
    assert!(
        reassembler
            .push(&raw(FLAG_FIN, 1, b"y"), 5)
            .unwrap()
            .is_some()
    );
    assert_eq!(reassembler.next_deadline_ms(), None);
    assert_eq!(reassembler.check_idle(i64::MAX), Ok(()));
}

#[test]
fn the_idle_time_comes_from_the_configuration() {
    let mut reassembler = Reassembler::new(Side::Connector, 250, Unlimited);
    reassembler.enter_session();
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), 1_000), Ok(None));
    assert_eq!(reassembler.next_deadline_ms(), Some(1_250));
    assert_eq!(reassembler.check_idle(1_249), Ok(()));
    assert_eq!(reassembler.check_idle(1_250), Err(FrameError::StreamIdle));
}

/// The way a program uses it: the deadline comes from the configuration, the time from a
/// clock, and a test moves that clock by hand.
#[test]
fn the_idle_rule_with_a_clock_the_test_controls() {
    let deadlines = Deadlines {
        stream_idle_ms: 40,
        ..Deadlines::default()
    };
    let clock = ManualClock::new(10_000);
    let mut reassembler = Reassembler::new(Side::Hub, deadlines.stream_idle_ms, Unlimited);
    reassembler.enter_session();

    assert_eq!(
        reassembler.push(&raw(0, 2, b"first"), clock.now_ms()),
        Ok(None)
    );
    clock.advance(39);
    assert_eq!(reassembler.check_idle(clock.now_ms()), Ok(()));
    // A chunk in time renews the stream.
    assert_eq!(
        reassembler.push(&raw(0, 2, b"second"), clock.now_ms()),
        Ok(None)
    );
    clock.advance(39);
    assert_eq!(reassembler.check_idle(clock.now_ms()), Ok(()));
    assert_eq!(reassembler.next_deadline_ms(), Some(clock.now_ms() + 1));
    // One millisecond more and the stream has stayed idle for the whole deadline.
    clock.advance(1);
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 2, b"late"), clock.now_ms()),
        Err(FrameError::StreamIdle)
    );
}

#[test]
fn a_clock_that_goes_back_or_jumps_does_not_panic() {
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), 1_000), Ok(None));
    // Back in time: the stream is simply not idle yet.
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), -5_000), Ok(None));
    assert_eq!(reassembler.check_idle(i64::MIN), Ok(()));
    assert_eq!(reassembler.next_deadline_ms(), Some(-5_000 + IDLE));
    // The far future saturates instead of overflowing.
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), i64::MAX - 5), Ok(None));
    assert_eq!(reassembler.next_deadline_ms(), Some(i64::MAX));
    let mut reassembler = in_session(Side::Connector);
    assert_eq!(reassembler.push(&raw(0, 1, b"x"), i64::MIN), Ok(None));
    assert_eq!(
        reassembler.check_idle(i64::MAX),
        Err(FrameError::StreamIdle)
    );
}

// ---------------------------------------------------------------------------------------------
// Reassembler: the budget
// ---------------------------------------------------------------------------------------------

#[test]
fn the_budget_is_charged_only_for_bytes_that_arrived() {
    let budget = TestBudget::new(usize::MAX);
    let mut reassembler = Reassembler::new(Side::Hub, IDLE, budget.clone());
    reassembler.enter_session();

    // Single-frame messages take nothing.
    assert!(
        reassembler
            .push(&raw(FLAG_FIN, 0, &[1u8; 5_000]), 0)
            .is_ok()
    );
    assert_eq!(budget.used(), 0);

    // Ten bytes arrived: ten bytes are taken, not a chunk and not the size of a message.
    assert_eq!(reassembler.push(&raw(0, 2, &[1u8; 10]), 0), Ok(None));
    assert_eq!(budget.used(), 10);

    let mut received = 10;
    for _ in 0..200 {
        assert_eq!(reassembler.push(&raw(0, 2, &[1u8; 1_000]), 0), Ok(None));
        received += 1_000;
        assert!(budget.used() >= received);
        assert!(
            budget.used() <= 2 * received,
            "{} taken for {received} bytes",
            budget.used()
        );
    }
    assert!(
        reassembler
            .push(&raw(FLAG_FIN, 2, &[1u8; 1]), 0)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        budget.used(),
        0,
        "a delivered message gives everything back"
    );
}

#[test]
fn the_budget_never_passes_the_largest_message() {
    let budget = TestBudget::new(usize::MAX);
    let mut reassembler = Reassembler::new(Side::Connector, IDLE, budget.clone());
    reassembler.enter_session();
    assert_eq!(
        push_total(&mut reassembler, 1, CONNECTOR_MESSAGE_MAX),
        Ok(Some(CONNECTOR_MESSAGE_MAX))
    );
    assert_eq!(budget.used(), 0);
    assert!(budget.peak() <= CONNECTOR_MESSAGE_MAX, "{}", budget.peak());
    assert!(budget.peak() >= CONNECTOR_MESSAGE_MAX);
}

#[test]
fn without_room_in_the_budget_the_connection_is_closed_with_ar205() {
    let budget = TestBudget::new(100_000);
    let mut reassembler = Reassembler::new(Side::Connector, IDLE, budget.clone());
    reassembler.enter_session();
    assert_eq!(reassembler.push(&raw(0, 1, &[1u8; 60_000]), 0), Ok(None));
    assert_eq!(budget.used(), 60_000);
    // Doubling does not fit, the exact size does: the message still goes through.
    assert_eq!(reassembler.push(&raw(0, 1, &[1u8; 40_000]), 0), Ok(None));
    assert_eq!(budget.used(), 100_000);
    // One more byte does not fit at all.
    let error = reassembler.push(&raw(0, 1, &[1u8; 1]), 0).unwrap_err();
    assert_eq!(error, FrameError::BudgetExhausted);
    assert_eq!(error.closing_code(), ErrorCode::Ar205);
    assert_eq!(
        budget.used(),
        0,
        "a failed connection gives everything back"
    );
    assert_eq!(
        reassembler.push(&raw(FLAG_FIN, 0, b"x"), 0),
        Err(FrameError::Closed)
    );
}

#[test]
fn a_new_stream_without_room_is_refused() {
    let budget = TestBudget::new(50);
    let mut reassembler = Reassembler::new(Side::Hub, IDLE, budget.clone());
    reassembler.enter_session();
    assert_eq!(
        reassembler.push(&raw(0, 2, &[1u8; 51]), 0),
        Err(FrameError::BudgetExhausted)
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn connections_share_one_budget() {
    let budget = TestBudget::new(100);
    let mut first = Reassembler::new(Side::Connector, IDLE, budget.clone());
    first.enter_session();
    let mut second = Reassembler::new(Side::Connector, IDLE, budget.clone());
    second.enter_session();
    assert_eq!(first.push(&raw(0, 1, &[1u8; 80]), 0), Ok(None));
    // The second connection finds the budget almost full: it is the one that is closed.
    assert_eq!(
        second.push(&raw(0, 1, &[1u8; 30]), 0),
        Err(FrameError::BudgetExhausted)
    );
    assert_eq!(budget.used(), 80);
    assert_eq!(
        payload_of(first.push(&raw(FLAG_FIN, 1, &[2u8; 20]), 0).unwrap()).len(),
        100
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn every_way_out_gives_the_budget_back() {
    // Dropped in the middle of a message.
    let budget = TestBudget::new(usize::MAX);
    let mut reassembler = Reassembler::new(Side::Connector, IDLE, budget.clone());
    reassembler.enter_session();
    assert_eq!(reassembler.push(&raw(0, 1, &[1u8; 1_000]), 0), Ok(None));
    assert_eq!(reassembler.push(&raw(0, 3, &[1u8; 2_000]), 0), Ok(None));
    assert_eq!(budget.used(), 3_000);
    drop(reassembler);
    assert_eq!(budget.used(), 0);

    // A protocol error.
    let mut reassembler = Reassembler::new(Side::Connector, IDLE, budget.clone());
    reassembler.enter_session();
    assert_eq!(reassembler.push(&raw(0, 1, &[1u8; 1_000]), 0), Ok(None));
    assert_eq!(
        reassembler.push(&raw(0x80, 0, b"x"), 0),
        Err(FrameError::ReservedBits)
    );
    assert_eq!(budget.used(), 0);
    drop(reassembler);
    assert_eq!(budget.used(), 0, "nothing is given back twice");

    // An idle stream, found by the timer.
    let mut reassembler = Reassembler::new(Side::Connector, IDLE, budget.clone());
    reassembler.enter_session();
    assert_eq!(reassembler.push(&raw(0, 1, &[1u8; 1_000]), 0), Ok(None));
    assert_eq!(reassembler.check_idle(IDLE), Err(FrameError::StreamIdle));
    assert_eq!(budget.used(), 0);
}

#[test]
fn closing_codes() {
    let protocol_errors = [
        FrameError::Truncated,
        FrameError::ReservedBits,
        FrameError::EmptyChunk,
        FrameError::ChunkTooLarge,
        FrameError::ControlNotSingleFrame,
        FrameError::StreamZeroWithoutFin,
        FrameError::WrongParity,
        FrameError::StreamNotIncreasing,
        FrameError::SingleFrameOnStream,
        FrameError::TooManyStreams,
        FrameError::StreamBeforeSession,
        FrameError::MessageTooLarge,
        FrameError::StreamIdle,
        FrameError::Closed,
    ];
    for error in protocol_errors {
        assert_eq!(error.closing_code(), ErrorCode::Ar204, "{error:?}");
        assert!(!error.to_string().is_empty());
    }
    assert_eq!(FrameError::BudgetExhausted.closing_code(), ErrorCode::Ar205);
}

#[test]
fn debug_output_shows_sizes_and_never_content() {
    let secret = b"CANARY-frame-content";
    let mut splitter = Splitter::new(Side::Connector);
    let frames = splitter.split(secret, false).unwrap();
    let frame = frames.clone().next().unwrap();
    let mut reassembler = in_session(Side::Connector);
    let bytes = frame.to_bytes();
    let delivered = reassembler.push(&bytes, 0).unwrap().unwrap();
    let mut pending = in_session(Side::Connector);
    assert_eq!(pending.push(&raw(0, 1, secret), 0), Ok(None));
    for output in [
        format!("{frames:?}"),
        format!("{frame:?}"),
        format!("{delivered:?}"),
        format!("{pending:?}"),
        format!("{splitter:?}"),
    ] {
        assert!(!output.contains("CANARY"), "{output}");
        assert!(!output.contains("67, 65, 78"), "{output}");
    }
    assert_eq!(
        format!("{delivered:?}"),
        "Delivered { control: false, len: 20 }"
    );
}

#[test]
fn the_side_knows_its_peer() {
    assert_eq!(Side::Connector.peer(), Side::Hub);
    assert_eq!(Side::Hub.peer(), Side::Connector);
}

// ---------------------------------------------------------------------------------------------
// Sender and receiver together
// ---------------------------------------------------------------------------------------------

#[test]
fn what_the_splitter_cuts_the_reassembler_puts_together() {
    for (side, max) in [
        (Side::Connector, CONNECTOR_MESSAGE_MAX),
        (Side::Hub, HUB_MESSAGE_MAX),
    ] {
        let mut splitter = Splitter::new(side);
        splitter.enter_session();
        let mut reassembler = in_session(side);
        for (index, size) in [1usize, 5, 59_999, 60_000, 60_001, 120_000, 123_457, max]
            .into_iter()
            .enumerate()
        {
            let message = pattern(size, u64::try_from(index).unwrap());
            let frames = splitter.split(&message, false).unwrap();
            assert_eq!(push_message(&mut reassembler, frames, 0), message, "{size}");
            assert_eq!(reassembler.open_streams(), 0);
        }
    }
}

#[test]
fn the_pre_session_limits_agree_on_both_halves() {
    for side in [Side::Connector, Side::Hub] {
        let mut splitter = Splitter::new(side);
        let mut reassembler = Reassembler::new(side, IDLE, Unlimited);
        let message = pattern(PRE_SESSION_MESSAGE_MAX, 9);
        let frames = splitter.split(&message, false).unwrap();
        assert_eq!(push_message(&mut reassembler, frames, 0), message);
    }
}

fn size_strategy() -> impl Strategy<Value = usize> {
    prop_oneof![
        4 => 1usize..=300,
        2 => 59_990usize..=60_010,
        1 => 119_990usize..=120_010,
        3 => 60_001usize..=250_000,
    ]
}

/// One message on its way: the frames still to send, and the whole message to compare.
struct InFlight {
    frames: Vec<Vec<u8>>,
    next: usize,
    expected: Vec<u8>,
}

proptest! {
    #![proptest_config(crate::test_util::random_rounds(48))]

    /// Random messages, cut and interleaved between streams the way a writer with several
    /// producers would send them, come out identical on the other end.
    #[test]
    fn interleaved_messages_come_out_identical(
        seed in any::<u64>(),
        sizes in prop::collection::vec(size_strategy(), 1..14),
        from_hub in any::<bool>(),
    ) {
        let side = if from_hub { Side::Hub } else { Side::Connector };
        let mut rng = SeededRng::new(seed);
        let budget = TestBudget::new(usize::MAX);
        let mut splitter = Splitter::new(side);
        splitter.enter_session();
        let mut reassembler = Reassembler::new(side, IDLE, budget.clone());
        reassembler.enter_session();

        let messages: Vec<Vec<u8>> = sizes
            .iter()
            .map(|size| {
                let mut bytes = vec![0u8; *size];
                rng.fill(&mut bytes);
                bytes
            })
            .collect();

        let mut waiting = messages.iter();
        let mut in_flight: Vec<InFlight> = Vec::new();
        let mut delivered: Vec<Vec<u8>> = Vec::new();
        let mut expected: Vec<Vec<u8>> = Vec::new();
        let mut now_ms = 0i64;
        let mut started = 0usize;

        loop {
            let can_start = started < messages.len() && in_flight.len() < MAX_OPEN_STREAMS;
            if in_flight.is_empty() && !can_start {
                break;
            }
            now_ms += rng.between(0, 500);
            let start_one = can_start && (in_flight.is_empty() || rng.chance(1, 3));
            if start_one {
                // The first frame of a message goes out as soon as its stream number is taken:
                // streams reach the other side in growing order.
                let message = waiting.next().expect("counted above");
                started += 1;
                let frames: Vec<Vec<u8>> = splitter
                    .split(message, false)
                    .expect("within the limits")
                    .map(|frame| frame.to_bytes())
                    .collect();
                in_flight.push(InFlight { frames, next: 0, expected: message.clone() });
            }
            let index = if start_one {
                in_flight.len() - 1
            } else {
                usize::try_from(rng.below(u64::try_from(in_flight.len()).unwrap())).unwrap()
            };
            let message = &mut in_flight[index];
            let frame = &message.frames[message.next];
            prop_assert!(frame.len() <= crate::limits::FRAME_PLAINTEXT_MAX);
            message.next += 1;
            let last = message.next == message.frames.len();
            let result = reassembler.push(frame, now_ms);
            prop_assert!(result.is_ok(), "seed {}: {:?}", seed, result.err());
            match result.unwrap() {
                Some(whole) => {
                    prop_assert!(last, "seed {}: delivered before FIN", seed);
                    delivered.push(whole.payload.into_owned());
                }
                None => prop_assert!(!last, "seed {}: FIN delivered nothing", seed),
            }
            if last {
                expected.push(in_flight.swap_remove(index).expected);
            }
            prop_assert!(reassembler.open_streams() <= MAX_OPEN_STREAMS);
        }

        prop_assert_eq!(delivered.len(), messages.len());
        prop_assert!(delivered == expected, "seed {}: a message came out different", seed);
        prop_assert_eq!(reassembler.open_streams(), 0);
        prop_assert_eq!(budget.used(), 0);
        prop_assert_eq!(reassembler.next_deadline_ms(), None);
    }
}
