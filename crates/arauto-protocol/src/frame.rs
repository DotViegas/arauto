//! Frames: how a message is cut in pieces and put together again.
//!
//! After the handshake every binary websocket message is one Noise transport message, and its
//! decrypted content is a frame (`docs/14-protocolo-do-canal.md`, section 3):
//!
//! ```text
//! byte 0      flags: bit 0 = FIN (last chunk of the message); bit 1 = CONTROL
//! bytes 1-4   stream number, unsigned, most significant byte first
//! bytes 5...  chunk of the message
//! ```
//!
//! A message that fits one frame goes on stream 0. A larger one is cut in chunks of up to
//! 60,000 bytes on a stream of its own: odd numbers for the connector, even numbers from 2 for
//! the hub, always growing.
//!
//! [`Splitter`] is the sending half and [`Reassembler`] the receiving half. Both are pure: the
//! reassembler receives the time as an argument and never allocates more than it has received.

use core::fmt;
use std::borrow::Cow;
use std::sync::Arc;

use crate::errors::ErrorCode;
use crate::limits::{
    CONNECTOR_MESSAGE_MAX, FRAME_CHUNK_MAX, FRAME_HEADER_LEN, HUB_MESSAGE_MAX, MAX_OPEN_STREAMS,
    PRE_SESSION_MESSAGE_MAX,
};

#[cfg(test)]
mod tests;

/// Flag of the last chunk of a message.
pub const FLAG_FIN: u8 = 0b0000_0001;

/// Flag of a control message (heartbeat and check): it goes ahead of the data queue.
pub const FLAG_CONTROL: u8 = 0b0000_0010;

/// Bits 2 to 7 are always zero.
const RESERVED_BITS: u8 = !(FLAG_FIN | FLAG_CONTROL);

/// One of the two programs, as the sender of frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The connector: it opens the odd streams.
    Connector,
    /// The hub: it opens the even streams, from 2.
    Hub,
}

impl Side {
    /// The other side.
    #[must_use]
    pub const fn peer(self) -> Side {
        match self {
            Side::Connector => Side::Hub,
            Side::Hub => Side::Connector,
        }
    }

    /// Largest message this side may send once the session has started.
    #[must_use]
    pub const fn session_message_max(self) -> usize {
        match self {
            Side::Connector => CONNECTOR_MESSAGE_MAX,
            Side::Hub => HUB_MESSAGE_MAX,
        }
    }

    const fn first_stream(self) -> u32 {
        match self {
            Side::Connector => 1,
            Side::Hub => 2,
        }
    }

    /// Whether `stream` is a number this side may open.
    const fn owns(self, stream: u32) -> bool {
        let odd = stream % 2 == 1;
        stream != 0 && matches!((self, odd), (Side::Connector, true) | (Side::Hub, false))
    }
}

/// How much is accepted on a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Until `session`: only stream 0, and messages of up to 4,096 bytes.
    PreSession,
    /// From `session` on: streams, and the size limit of each direction.
    Session,
}

impl Phase {
    const fn message_max(self, sender: Side) -> usize {
        match self {
            Phase::PreSession => PRE_SESSION_MESSAGE_MAX,
            Phase::Session => sender.session_message_max(),
        }
    }
}

/// The five bytes in front of every chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameHeader {
    /// Last chunk of the message.
    pub fin: bool,
    /// Control message.
    pub control: bool,
    /// Stream number. Zero is the stream of single-frame messages.
    pub stream: u32,
}

impl FrameHeader {
    /// Writes the header.
    #[must_use]
    pub const fn encode(self) -> [u8; FRAME_HEADER_LEN] {
        let mut flags = 0;
        if self.fin {
            flags |= FLAG_FIN;
        }
        if self.control {
            flags |= FLAG_CONTROL;
        }
        let [a, b, c, d] = self.stream.to_be_bytes();
        [flags, a, b, c, d]
    }

    /// Separates the header from the chunk of a decrypted frame.
    ///
    /// Only the layout is checked here: the size of the header and the reserved bits. The rules
    /// about streams belong to the [`Reassembler`].
    ///
    /// # Errors
    /// [`FrameError::Truncated`] and [`FrameError::ReservedBits`].
    pub fn decode(frame: &[u8]) -> Result<(FrameHeader, &[u8]), FrameError> {
        let (head, chunk) = frame
            .split_first_chunk::<FRAME_HEADER_LEN>()
            .ok_or(FrameError::Truncated)?;
        let [flags, a, b, c, d] = *head;
        if flags & RESERVED_BITS != 0 {
            return Err(FrameError::ReservedBits);
        }
        let header = FrameHeader {
            fin: flags & FLAG_FIN != 0,
            control: flags & FLAG_CONTROL != 0,
            stream: u32::from_be_bytes([a, b, c, d]),
        };
        Ok((header, chunk))
    }
}

/// A frame the other side should never have sent, or a limit it went over. Every one of them
/// closes the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum FrameError {
    /// Fewer bytes than a header.
    #[error("frame shorter than its header")]
    Truncated,
    /// One of the bits 2 to 7 of the flags is set.
    #[error("reserved flag bits are set")]
    ReservedBits,
    /// A header with nothing after it.
    #[error("frame without a chunk")]
    EmptyChunk,
    /// A chunk above 60,000 bytes.
    #[error("chunk larger than the limit")]
    ChunkTooLarge,
    /// CONTROL on a stream other than 0, or without FIN.
    #[error("CONTROL outside a single frame on stream 0")]
    ControlNotSingleFrame,
    /// Stream 0 only carries whole messages.
    #[error("frame on stream 0 without FIN")]
    StreamZeroWithoutFin,
    /// An even number from the connector, or an odd one from the hub.
    #[error("stream number of the wrong parity")]
    WrongParity,
    /// A number already used, or below the last one that side opened.
    #[error("stream number repeated or lower than the last one opened")]
    StreamNotIncreasing,
    /// A message of one frame belongs on stream 0.
    #[error("single-frame message outside stream 0")]
    SingleFrameOnStream,
    /// A ninth stream open at the same time.
    #[error("too many streams open")]
    TooManyStreams,
    /// Before the session only stream 0 exists.
    #[error("stream opened before the session")]
    StreamBeforeSession,
    /// More bytes than a message may have in this phase and direction.
    #[error("message larger than the limit")]
    MessageTooLarge,
    /// An open stream stayed too long without a new chunk.
    #[error("an open stream stayed idle for too long")]
    StreamIdle,
    /// The budget for messages under reassembly has no room for this connection.
    #[error("no room left in the reassembly budget")]
    BudgetExhausted,
    /// An earlier frame already failed: this connection is over.
    #[error("the connection already failed")]
    Closed,
}

impl FrameError {
    /// The code that goes in `closing` before the connection is closed: AR-205 when the hub is
    /// out of room, AR-204 for everything else.
    #[must_use]
    pub const fn closing_code(self) -> ErrorCode {
        match self {
            FrameError::BudgetExhausted => ErrorCode::Ar205,
            _ => ErrorCode::Ar204,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sending
// ---------------------------------------------------------------------------------------------

/// Why a message cannot be cut in frames. None of these closes the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum SplitError {
    /// A message has at least one byte.
    #[error("empty message")]
    Empty,
    /// The message does not fit the limit of this phase and direction. The call that produced
    /// it is answered with AR-709.
    #[error("message larger than the limit")]
    TooLarge,
    /// A control message always fits one frame.
    #[error("control message larger than one frame")]
    ControlTooLarge,
    /// This side has used every stream number. It closes the connection with AR-205 and
    /// reconnects.
    #[error("no stream numbers left")]
    StreamsExhausted,
}

/// Cuts the messages one side sends on a connection.
///
/// It checks the size before anything is sent, so that size becomes an error of the call and
/// never a closed connection, and hands out the stream numbers.
///
/// The receiver wants the streams opened in growing order. When more than one task produces
/// messages, call [`Splitter::split`] and queue the first frame of the result under the same
/// lock.
#[derive(Debug)]
pub struct Splitter {
    side: Side,
    phase: Phase,
    next_stream: Option<u32>,
}

impl Splitter {
    /// A splitter for the frames `side` sends, before the session.
    #[must_use]
    pub const fn new(side: Side) -> Self {
        Self {
            side,
            phase: Phase::PreSession,
            next_stream: Some(side.first_stream()),
        }
    }

    /// Switches to the limits of the session.
    ///
    /// The `session` message itself still belongs to the phase before: the hub cuts it first
    /// and switches afterwards, because the connector reads it with the limits of that phase.
    /// The connector switches as soon as it has read `session`.
    pub fn enter_session(&mut self) {
        self.phase = Phase::Session;
    }

    /// The current phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Plans the frames of one message. Nothing is copied: the frames borrow `message`.
    ///
    /// A message of up to 60,000 bytes becomes one frame on stream 0. A larger one takes the
    /// next stream number of this side.
    ///
    /// # Errors
    /// [`SplitError`]. No stream number is used when the message is refused.
    pub fn split<'m>(
        &mut self,
        message: &'m [u8],
        control: bool,
    ) -> Result<Frames<'m>, SplitError> {
        if message.is_empty() {
            return Err(SplitError::Empty);
        }
        if message.len() > self.phase.message_max(self.side) {
            return Err(SplitError::TooLarge);
        }
        if message.len() <= FRAME_CHUNK_MAX {
            return Ok(Frames {
                stream: 0,
                control,
                rest: message,
            });
        }
        if control {
            return Err(SplitError::ControlTooLarge);
        }
        let stream = self.next_stream.ok_or(SplitError::StreamsExhausted)?;
        self.next_stream = stream.checked_add(2);
        Ok(Frames {
            stream,
            control: false,
            rest: message,
        })
    }

    #[cfg(test)]
    fn starting_at(side: Side, next_stream: u32) -> Self {
        Self {
            side,
            phase: Phase::Session,
            next_stream: Some(next_stream),
        }
    }
}

/// The frames of one message, in the order they must be sent.
#[derive(Clone)]
pub struct Frames<'m> {
    stream: u32,
    control: bool,
    rest: &'m [u8],
}

impl Frames<'_> {
    /// The stream of these frames. Zero for a single-frame message.
    #[must_use]
    pub const fn stream(&self) -> u32 {
        self.stream
    }

    /// Whether the message opens a stream, which counts toward the 8 the receiver allows.
    #[must_use]
    pub const fn opens_stream(&self) -> bool {
        self.stream != 0
    }
}

impl<'m> Iterator for Frames<'m> {
    type Item = Frame<'m>;

    fn next(&mut self) -> Option<Frame<'m>> {
        if self.rest.is_empty() {
            return None;
        }
        let (chunk, rest) = self.rest.split_at(self.rest.len().min(FRAME_CHUNK_MAX));
        self.rest = rest;
        let header = FrameHeader {
            fin: rest.is_empty(),
            control: self.control,
            stream: self.stream,
        };
        Some(Frame { header, chunk })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.rest.len().div_ceil(FRAME_CHUNK_MAX);
        (left, Some(left))
    }
}

impl ExactSizeIterator for Frames<'_> {}

impl fmt::Debug for Frames<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The bytes are message content: only their amount is shown.
        f.debug_struct("Frames")
            .field("stream", &self.stream)
            .field("control", &self.control)
            .field("bytes_left", &self.rest.len())
            .finish()
    }
}

/// One frame ready to be encrypted.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Frame<'m> {
    /// The header.
    pub header: FrameHeader,
    /// The chunk: from 1 to 60,000 bytes.
    pub chunk: &'m [u8],
}

impl Frame<'_> {
    /// The frame as bytes: header, then chunk.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FRAME_HEADER_LEN.saturating_add(self.chunk.len()));
        bytes.extend_from_slice(&self.header.encode());
        bytes.extend_from_slice(self.chunk);
        bytes
    }
}

impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("header", &self.header)
            .field("chunk_len", &self.chunk.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------------------------
// Receiving
// ---------------------------------------------------------------------------------------------

/// The byte budget for messages under reassembly, shared by every connection of the hub.
///
/// Whoever owns the connections implements it. The reassembler asks before it keeps a byte and
/// gives everything back when the message is delivered, when the connection fails and when it
/// is dropped.
pub trait ReassemblyBudget {
    /// Takes `bytes` from the budget. Returns `false`, taking nothing, when they do not fit.
    fn try_reserve(&self, bytes: usize) -> bool;

    /// Gives back `bytes` taken before.
    fn release(&self, bytes: usize);
}

impl<B: ReassemblyBudget + ?Sized> ReassemblyBudget for &B {
    fn try_reserve(&self, bytes: usize) -> bool {
        (**self).try_reserve(bytes)
    }

    fn release(&self, bytes: usize) {
        (**self).release(bytes);
    }
}

impl<B: ReassemblyBudget + ?Sized> ReassemblyBudget for Arc<B> {
    fn try_reserve(&self, bytes: usize) -> bool {
        (**self).try_reserve(bytes)
    }

    fn release(&self, bytes: usize) {
        (**self).release(bytes);
    }
}

/// A budget that never refuses. For the connector, which has one connection and whose limit is
/// the size of a message times the open streams.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unlimited;

impl ReassemblyBudget for Unlimited {
    fn try_reserve(&self, _bytes: usize) -> bool {
        true
    }

    fn release(&self, _bytes: usize) {}
}

/// A whole message, as the other side sent it.
#[derive(Clone, PartialEq, Eq)]
pub struct Delivered<'f> {
    /// The message came with the CONTROL flag.
    pub control: bool,
    /// The bytes of the message. Borrowed from the frame when the message was a single frame.
    pub payload: Cow<'f, [u8]>,
}

impl fmt::Debug for Delivered<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The bytes are message content: only their amount is shown.
        f.debug_struct("Delivered")
            .field("control", &self.control)
            .field("len", &self.payload.len())
            .finish()
    }
}

struct OpenStream {
    id: u32,
    buffer: Vec<u8>,
    /// Bytes taken from the budget for `buffer`. Never below `buffer.len()`.
    reserved: usize,
    last_chunk_ms: i64,
}

/// Puts together the messages one side receives on a connection, and applies every rule of the
/// frame table and the limits of each phase.
///
/// Any error is final: the reassembler gives its bytes back to the budget and refuses every
/// later frame with [`FrameError::Closed`]. The caller sends `closing` with
/// [`FrameError::closing_code`] and closes the connection.
pub struct Reassembler<B: ReassemblyBudget> {
    sender: Side,
    phase: Phase,
    stream_idle_ms: i64,
    budget: B,
    last_opened: u32,
    open: Vec<OpenStream>,
    failed: bool,
}

impl<B: ReassemblyBudget> Reassembler<B> {
    /// A reassembler for the frames that `sender` sends, before the session.
    ///
    /// `stream_idle_ms` is how long an open stream may stay without a new chunk
    /// ([`crate::limits::Deadlines::stream_idle_ms`]).
    #[must_use]
    pub const fn new(sender: Side, stream_idle_ms: i64, budget: B) -> Self {
        Self {
            sender,
            phase: Phase::PreSession,
            stream_idle_ms,
            budget,
            last_opened: 0,
            open: Vec::new(),
            failed: false,
        }
    }

    /// Switches to the limits of the session.
    ///
    /// Call it between two frames: the hub when it sends `session`, the connector as soon as
    /// [`Reassembler::push`] delivers `session`, before pushing the next frame.
    pub fn enter_session(&mut self) {
        self.phase = Phase::Session;
    }

    /// The current phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Streams open right now. Stream 0 never counts.
    #[must_use]
    pub fn open_streams(&self) -> usize {
        self.open.len()
    }

    /// When the first open stream becomes idle, if nothing arrives for it. The caller arms a
    /// timer for this instant and then calls [`Reassembler::check_idle`].
    #[must_use]
    pub fn next_deadline_ms(&self) -> Option<i64> {
        self.open
            .iter()
            .map(|stream| stream.last_chunk_ms.saturating_add(self.stream_idle_ms))
            .min()
    }

    /// Fails when an open stream has stayed `stream_idle_ms` without a new chunk.
    ///
    /// # Errors
    /// [`FrameError::StreamIdle`], or [`FrameError::Closed`] after an earlier error.
    pub fn check_idle(&mut self, now_ms: i64) -> Result<(), FrameError> {
        if self.failed {
            return Err(FrameError::Closed);
        }
        self.find_idle(now_ms).inspect_err(|_| self.fail())
    }

    /// Receives one decrypted frame. Returns a message when the frame completes one.
    ///
    /// # Errors
    /// [`FrameError`]. After the first error every call fails with [`FrameError::Closed`].
    pub fn push<'f>(
        &mut self,
        frame: &'f [u8],
        now_ms: i64,
    ) -> Result<Option<Delivered<'f>>, FrameError> {
        if self.failed {
            return Err(FrameError::Closed);
        }
        self.accept(frame, now_ms).inspect_err(|_| self.fail())
    }

    fn accept<'f>(
        &mut self,
        frame: &'f [u8],
        now_ms: i64,
    ) -> Result<Option<Delivered<'f>>, FrameError> {
        // A chunk that arrives late does not rescue a stream that already ran out of time.
        self.find_idle(now_ms)?;
        let (header, chunk) = FrameHeader::decode(frame)?;
        if chunk.is_empty() {
            return Err(FrameError::EmptyChunk);
        }
        if chunk.len() > FRAME_CHUNK_MAX {
            return Err(FrameError::ChunkTooLarge);
        }
        if header.control && (header.stream != 0 || !header.fin) {
            return Err(FrameError::ControlNotSingleFrame);
        }
        if header.stream == 0 {
            if !header.fin {
                return Err(FrameError::StreamZeroWithoutFin);
            }
            if chunk.len() > self.phase.message_max(self.sender) {
                return Err(FrameError::MessageTooLarge);
            }
            return Ok(Some(Delivered {
                control: header.control,
                payload: Cow::Borrowed(chunk),
            }));
        }
        if self.phase == Phase::PreSession {
            return Err(FrameError::StreamBeforeSession);
        }
        if !self.sender.owns(header.stream) {
            return Err(FrameError::WrongParity);
        }
        match self
            .open
            .iter()
            .position(|stream| stream.id == header.stream)
        {
            Some(index) => self.continue_stream(index, header.fin, chunk, now_ms),
            None => self.open_stream(header, chunk, now_ms).map(|()| None),
        }
    }

    fn open_stream(
        &mut self,
        header: FrameHeader,
        chunk: &[u8],
        now_ms: i64,
    ) -> Result<(), FrameError> {
        if header.stream <= self.last_opened {
            return Err(FrameError::StreamNotIncreasing);
        }
        if header.fin {
            return Err(FrameError::SingleFrameOnStream);
        }
        if self.open.len() >= MAX_OPEN_STREAMS {
            return Err(FrameError::TooManyStreams);
        }
        let max = self.phase.message_max(self.sender);
        if chunk.len() > max {
            return Err(FrameError::MessageTooLarge);
        }
        let mut stream = OpenStream {
            id: header.stream,
            buffer: Vec::new(),
            reserved: 0,
            last_chunk_ms: now_ms,
        };
        grow(&self.budget, &mut stream, chunk.len(), max)?;
        stream.buffer.extend_from_slice(chunk);
        self.last_opened = header.stream;
        self.open.push(stream);
        Ok(())
    }

    fn continue_stream<'f>(
        &mut self,
        index: usize,
        fin: bool,
        chunk: &[u8],
        now_ms: i64,
    ) -> Result<Option<Delivered<'f>>, FrameError> {
        let max = self.phase.message_max(self.sender);
        let stream = self.open.get_mut(index).ok_or(FrameError::Closed)?;
        let needed = stream.buffer.len().saturating_add(chunk.len());
        if needed > max {
            return Err(FrameError::MessageTooLarge);
        }
        grow(&self.budget, stream, needed, max)?;
        stream.buffer.extend_from_slice(chunk);
        stream.last_chunk_ms = now_ms;
        if !fin {
            return Ok(None);
        }
        let stream = self.open.swap_remove(index);
        self.budget.release(stream.reserved);
        Ok(Some(Delivered {
            control: false,
            payload: Cow::Owned(stream.buffer),
        }))
    }

    fn find_idle(&self, now_ms: i64) -> Result<(), FrameError> {
        let idle = |stream: &OpenStream| {
            now_ms.saturating_sub(stream.last_chunk_ms) >= self.stream_idle_ms
        };
        if self.open.iter().any(idle) {
            Err(FrameError::StreamIdle)
        } else {
            Ok(())
        }
    }

    fn fail(&mut self) {
        self.failed = true;
        self.release_all();
    }

    fn release_all(&mut self) {
        for stream in self.open.drain(..) {
            self.budget.release(stream.reserved);
        }
    }
}

impl<B: ReassemblyBudget> Drop for Reassembler<B> {
    fn drop(&mut self) {
        self.release_all();
    }
}

impl<B: ReassemblyBudget> fmt::Debug for Reassembler<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reassembler")
            .field("sender", &self.sender)
            .field("phase", &self.phase)
            .field("open_streams", &self.open.len())
            .field("last_opened", &self.last_opened)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

/// Makes room in `stream` for `needed` bytes in total, charging the budget for what is really
/// kept. `needed` never passes `max`, and only counts bytes that have already arrived: nothing
/// here is sized by a number the other side announced.
fn grow<B: ReassemblyBudget>(
    budget: &B,
    stream: &mut OpenStream,
    needed: usize,
    max: usize,
) -> Result<(), FrameError> {
    if needed <= stream.reserved {
        return Ok(());
    }
    // Doubling keeps the copying linear in the size of the message. When the budget has no
    // room for that, the exact size still lets the message through.
    let doubled = stream.reserved.saturating_mul(2).min(max);
    let target = if doubled >= needed && budget.try_reserve(doubled.saturating_sub(stream.reserved))
    {
        doubled
    } else if budget.try_reserve(needed.saturating_sub(stream.reserved)) {
        needed
    } else {
        return Err(FrameError::BudgetExhausted);
    };
    let taken = target.saturating_sub(stream.reserved);
    let additional = target.saturating_sub(stream.buffer.len());
    if stream.buffer.try_reserve_exact(additional).is_err() {
        budget.release(taken);
        return Err(FrameError::BudgetExhausted);
    }
    stream.reserved = target;
    Ok(())
}
