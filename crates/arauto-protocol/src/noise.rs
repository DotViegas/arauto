//! The Noise channel between the connector and the hub.
//!
//! One fixed suite, `Noise_XK_25519_ChaChaPoly_BLAKE2s`, with the prologue `arauto/1`, through
//! the `snow` library. The connector initiates; its static key is the identity of the device,
//! and the static key of the hub came in the block (`docs/14-protocolo-do-canal.md`, sections 2
//! and 3):
//!
//! ```text
//! connector -> hub   e, es            48 bytes
//! hub -> connector   e, ee            48 bytes
//! connector -> hub   s, se            64 bytes
//! ```
//!
//! The three messages carry no payload and have an exact size. A hub without the key of the
//! block does not get past the first one, and nothing it answers completes the handshake on
//! the connector.
//!
//! After the handshake each direction has a counter that starts at zero, goes up by one at
//! every frame and never travels: [`Sealer`] and [`Opener`] each keep their own. A frame that
//! is repeated, missing, out of order or changed does not open, and from then on the opener
//! refuses everything.
//!
//! The XK pattern does not bar an unknown identity: anyone who has the public key of the hub
//! completes the handshake. What it proves to the hub is that the other side holds the private
//! key of the identity it shows. For that to be true of every identity, a key exchange whose
//! result is all zeros is refused ([`PublicKey::is_low_order`]): with a low-order point as its
//! key, anyone would complete the handshake with no private key at all.
//!
//! The Diffie-Hellman function of the handshake is `x25519-dalek`, plugged into the library in
//! place of the one it brings, for that refusal and so that the private keys, static and
//! ephemeral, are wiped when the handshake state is dropped. What the library keeps of the
//! symmetric state (the chaining key and the two transport keys) is not wiped by it.

use core::fmt;
use std::sync::Arc;

use snow::params::{CipherChoice, DHChoice, HashChoice};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::types::{Cipher, Dh, Hash, Random};
use snow::{Builder, HandshakeState, StatelessTransportState};
use x25519_dalek::{PublicKey as DalekPublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::limits::{
    HANDSHAKE_FIRST_LEN, HANDSHAKE_HASH_LEN, HANDSHAKE_SECOND_LEN, HANDSHAKE_THIRD_LEN, KEY_LEN,
    NOISE_TAG_LEN, WS_MESSAGE_MAX,
};
use crate::secret::{RandomError, Secret};

#[cfg(test)]
mod tests;

/// The Noise protocol name. One suite, with no fallback.
pub const NOISE_PATTERN: &str = "Noise_XK_25519_ChaChaPoly_BLAKE2s";

/// The prologue. It does not change with the protocol version.
pub const PROLOGUE: &[u8] = b"arauto/1";

/// Largest frame that fits one Noise message, before encryption.
pub const NOISE_PLAINTEXT_MAX: usize = WS_MESSAGE_MAX - NOISE_TAG_LEN;

/// What can go wrong in the channel. Every variant but [`NoiseError::TooLarge`] on the sending
/// half ends the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum NoiseError {
    /// The Noise library refused an operation that should always work.
    #[error("the Noise library refused the operation")]
    Library,
    /// The generator of the operating system failed while drawing the ephemeral key.
    #[error("the random generator of the operating system failed")]
    Random,
    /// A handshake message that is not exactly 48, 48 or 64 bytes.
    #[error("handshake message of the wrong size")]
    HandshakeLength,
    /// The handshake did not complete: the other side does not hold the key it should, the
    /// message was changed on the way, or a key is a low-order point, which nobody holds. For
    /// the connector this is AR-202.
    #[error("the handshake did not complete")]
    Handshake,
    /// More bytes than fit one Noise message.
    #[error("frame larger than a Noise message")]
    TooLarge,
    /// The frame did not decrypt: it was changed, repeated, skipped or is out of order.
    #[error("the frame did not decrypt")]
    Decrypt,
    /// The counter of this direction reached its end.
    #[error("the frame counter reached its end")]
    CounterExhausted,
    /// An earlier frame already failed to open: this channel is over.
    #[error("the channel already failed")]
    Closed,
}

/// An X25519 public key: the identity of a device as the hub knows it, or the key of the hub
/// that goes in every block. It is not a secret.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey([u8; KEY_LEN]);

impl PublicKey {
    /// A public key from its 32 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// A public key from a slice. Returns `None` when it does not have 32 bytes.
    #[must_use]
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        <[u8; KEY_LEN]>::try_from(bytes).ok().map(Self)
    }

    /// The 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Whether this is a low-order point: one of the few values with which every private key
    /// gives the same result, all zeros.
    ///
    /// Such a value is nobody's key. As the identity of a device it would be an identity
    /// anyone can show, and as the key of a hub it would be a hub anyone can be. The handshake
    /// refuses it on both sides; whoever reads a key from a block or from a file can refuse it
    /// earlier with this.
    #[must_use]
    pub fn is_low_order(&self) -> bool {
        // The scalar of all-zero bytes is clamped to 2^254, and 2^254 times a point is the
        // neutral element exactly when the order of the point is a power of two.
        !StaticSecret::from([0u8; KEY_LEN])
            .diffie_hellman(&DalekPublicKey::from(self.0))
            .was_contributory()
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", data_encoding::HEXLOWER.encode(&self.0))
    }
}

/// What starts the text of the public key of a hub.
pub const HUB_KEY_PREFIX: &str = "arh1";

/// The size of the text of the public key of a hub: the prefix and 52 characters of base32.
pub const HUB_KEY_TEXT_LEN: usize = 56;

/// Why a text is not the public key of a hub. No variant repeats the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PublicKeyTextError {
    /// Not exactly 56 characters.
    #[error("the key of a hub has 56 characters")]
    Length,
    /// Does not start with `arh1`.
    #[error("the key of a hub starts with arh1")]
    Prefix,
    /// A character outside `a-z` and `2-7` after the prefix.
    #[error("the key of a hub has only the characters a-z and 2-7 after the prefix")]
    Character,
    /// The last character carries bits that no key has.
    #[error("the key of a hub is not written in its only valid form")]
    Encoding,
    /// A value that is nobody's key.
    #[error("this value is not the key of any hub")]
    LowOrder,
}

impl PublicKey {
    /// Writes the key the way it goes into the block: `arh1` and the 32 bytes in lowercase
    /// base32 without padding. It is public: it is how the connector knows the right hub.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = String::with_capacity(HUB_KEY_TEXT_LEN);
        text.push_str(HUB_KEY_PREFIX);
        text.push_str(&data_encoding::BASE32_NOPAD.encode(&self.0));
        text.make_ascii_lowercase();
        text
    }

    /// Reads the text of the key of a hub, strictly: one text for each key, and no key that
    /// nobody holds.
    ///
    /// # Errors
    /// [`PublicKeyTextError`] says which rule the text breaks.
    pub fn parse_text(text: &str) -> Result<Self, PublicKeyTextError> {
        if text.len() != HUB_KEY_TEXT_LEN {
            return Err(PublicKeyTextError::Length);
        }
        let body = text
            .strip_prefix(HUB_KEY_PREFIX)
            .ok_or(PublicKeyTextError::Prefix)?;
        if !body.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7')) {
            return Err(PublicKeyTextError::Character);
        }
        // The library knows the uppercase alphabet. Uppercase in the input was refused above.
        let bytes = data_encoding::BASE32_NOPAD
            .decode(body.to_ascii_uppercase().as_bytes())
            .map_err(|_| PublicKeyTextError::Encoding)?;
        let key = Self::from_slice(&bytes).ok_or(PublicKeyTextError::Encoding)?;
        if key.is_low_order() {
            return Err(PublicKeyTextError::LowOrder);
        }
        Ok(key)
    }
}

/// A static X25519 key pair: the identity of a device, or the key pair of the hub.
///
/// The private part is born where it is used and never leaves: the identity in the connector,
/// the key of the hub in the hub.
#[derive(Debug, PartialEq, Eq)]
pub struct StaticKeypair {
    public: PublicKey,
    private: Secret<KEY_LEN>,
}

impl StaticKeypair {
    /// Draws a new key pair from the generator of the operating system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn generate() -> Result<Self, RandomError> {
        Secret::random().map(Self::from_private)
    }

    /// Rebuilds the pair from a stored private key.
    #[must_use]
    pub fn from_private(private: Secret<KEY_LEN>) -> Self {
        let public = PublicKey(public_of(&private));
        Self { public, private }
    }

    /// The public part.
    #[must_use]
    pub const fn public(&self) -> &PublicKey {
        &self.public
    }

    /// The private part, to be written where only its owner reads.
    #[must_use]
    pub const fn private(&self) -> &Secret<KEY_LEN> {
        &self.private
    }
}

/// The handshake hash: the Noise value `h` at the end of the third message. Both sides read it
/// before moving to the frames, and it goes into the verification code.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HandshakeHash([u8; HANDSHAKE_HASH_LEN]);

impl HandshakeHash {
    /// A handshake hash from its 32 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HANDSHAKE_HASH_LEN]) -> Self {
        Self(bytes)
    }

    /// The 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HANDSHAKE_HASH_LEN] {
        &self.0
    }
}

impl fmt::Debug for HandshakeHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HandshakeHash({})",
            data_encoding::HEXLOWER.encode(&self.0)
        )
    }
}

/// The public key of a private key.
fn public_of(private: &Secret<KEY_LEN>) -> [u8; KEY_LEN] {
    DalekPublicKey::from(&StaticSecret::from(*private.expose())).to_bytes()
}

/// The name under which the tests count the private keys loaded into handshake states.
#[cfg(test)]
pub(crate) const KEY_WORK: &str = "handshake key loaded";

/// One X25519 key of a handshake, static or ephemeral: what the Noise library calls a `Dh`.
///
/// It takes the place of the one the library brings, with the same function and two
/// differences. The private key is a [`Secret`], wiped when the handshake state is dropped,
/// where the library keeps a plain array. And a result of all zeros is an error, where the
/// library carries on: that is the result of any private key with a low-order point.
#[derive(Default)]
struct HandshakeKey {
    private: Option<Secret<KEY_LEN>>,
    public: [u8; KEY_LEN],
}

impl HandshakeKey {
    fn load(&mut self, private: Option<Secret<KEY_LEN>>) {
        #[cfg(test)]
        crate::test_util::work_probe::record(KEY_WORK);
        self.public = private.as_ref().map_or([0u8; KEY_LEN], public_of);
        self.private = private;
    }
}

impl Dh for HandshakeKey {
    fn name(&self) -> &'static str {
        "25519"
    }

    fn pub_len(&self) -> usize {
        KEY_LEN
    }

    fn priv_len(&self) -> usize {
        KEY_LEN
    }

    fn set(&mut self, privkey: &[u8]) {
        // The trait cannot refuse. A key of another size leaves this one empty, and an empty
        // key fails every exchange.
        self.load(Secret::from_slice(privkey));
    }

    fn generate(&mut self, rng: &mut dyn Random) -> Result<(), snow::Error> {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        rng.try_fill_bytes(bytes.as_mut_slice())?;
        self.load(Some(Secret::new(*bytes)));
        Ok(())
    }

    fn pubkey(&self) -> &[u8] {
        &self.public
    }

    fn privkey(&self) -> &[u8] {
        self.private
            .as_ref()
            .map_or(&[], |private| private.expose().as_slice())
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) -> Result<(), snow::Error> {
        let private = self.private.as_ref().ok_or(snow::Error::Dh)?;
        let theirs = pubkey
            .get(..KEY_LEN)
            .and_then(|bytes| <[u8; KEY_LEN]>::try_from(bytes).ok())
            .ok_or(snow::Error::Dh)?;
        let shared =
            StaticSecret::from(*private.expose()).diffie_hellman(&DalekPublicKey::from(theirs));
        // All zeros: the other key is a low-order point, and this exchange proves nothing.
        if !shared.was_contributory() {
            return Err(snow::Error::Dh);
        }
        out.get_mut(..KEY_LEN)
            .ok_or(snow::Error::Dh)?
            .copy_from_slice(shared.as_bytes());
        Ok(())
    }
}

/// Gives the Noise library [`HandshakeKey`] as its X25519, and the library's own cipher, hash
/// and random generator (the generator of the operating system).
struct Resolver;

impl CryptoResolver for Resolver {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        DefaultResolver.resolve_rng()
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        match choice {
            DHChoice::Curve25519 => Some(Box::<HandshakeKey>::default()),
            _ => None,
        }
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        DefaultResolver.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        DefaultResolver.resolve_cipher(choice)
    }
}

fn library(_: snow::Error) -> NoiseError {
    NoiseError::Library
}

/// A builder of the one suite of the protocol, with its prologue.
fn builder<'a>() -> Result<Builder<'a>, NoiseError> {
    let params = NOISE_PATTERN.parse().map_err(library)?;
    Builder::with_resolver(params, Box::new(Resolver))
        .prologue(PROLOGUE)
        .map_err(library)
}

/// Writes the next handshake message, which has no payload and exactly `N` bytes.
fn write_exact<const N: usize>(state: &mut HandshakeState) -> Result<[u8; N], NoiseError> {
    let mut buffer = [0u8; 128];
    let written = state
        .write_message(&[], &mut buffer)
        .map_err(|error| match error {
            snow::Error::Rng => NoiseError::Random,
            // The key of the other side is a low-order point.
            snow::Error::Dh => NoiseError::Handshake,
            _ => NoiseError::Library,
        })?;
    buffer
        .get(..written)
        .and_then(|message| <[u8; N]>::try_from(message).ok())
        .ok_or(NoiseError::Library)
}

/// Reads a handshake message, which must have exactly `expected` bytes and no payload.
fn read_exact(
    state: &mut HandshakeState,
    message: &[u8],
    expected: usize,
) -> Result<(), NoiseError> {
    if message.len() != expected {
        return Err(NoiseError::HandshakeLength);
    }
    let mut payload = [0u8; 16];
    match state.read_message(message, &mut payload) {
        Ok(0) => Ok(()),
        _ => Err(NoiseError::Handshake),
    }
}

/// The handshake on the side of the connector, after the first message was written.
pub struct ConnectorHandshake {
    state: HandshakeState,
}

impl ConnectorHandshake {
    /// Starts a handshake with the hub of `hub_key` and writes the first message (48 bytes).
    ///
    /// # Errors
    /// [`NoiseError::Handshake`] when `hub_key` is a low-order point: no hub holds its private
    /// key, and nothing is sent. [`NoiseError::Random`] or [`NoiseError::Library`].
    pub fn start(
        identity: &StaticKeypair,
        hub_key: &PublicKey,
    ) -> Result<(Self, [u8; HANDSHAKE_FIRST_LEN]), NoiseError> {
        let mut state = builder()?
            .local_private_key(identity.private.expose())
            .map_err(library)?
            .remote_public_key(hub_key.as_bytes())
            .map_err(library)?
            .build_initiator()
            .map_err(library)?;
        let first = write_exact(&mut state)?;
        Ok((Self { state }, first))
    }

    /// Reads the second message (48 bytes) and writes the third (64 bytes). The channel is
    /// ready as soon as the third message is sent.
    ///
    /// # Errors
    /// [`NoiseError::HandshakeLength`] for another size. [`NoiseError::Handshake`] when the
    /// other side is not the hub of the key given to [`ConnectorHandshake::start`]: the
    /// connector closes with AR-202.
    pub fn finish(
        mut self,
        second: &[u8],
    ) -> Result<([u8; HANDSHAKE_THIRD_LEN], Channel), NoiseError> {
        read_exact(&mut self.state, second, HANDSHAKE_SECOND_LEN)?;
        let third = write_exact(&mut self.state)?;
        let channel = Channel::from_handshake(self.state)?;
        Ok((third, channel))
    }
}

impl fmt::Debug for ConnectorHandshake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorHandshake").finish_non_exhaustive()
    }
}

/// The handshake on the side of the hub, after the second message was written.
pub struct HubHandshake {
    state: HandshakeState,
}

impl HubHandshake {
    /// Reads the first message (48 bytes) and writes the second (48 bytes).
    ///
    /// # Errors
    /// [`NoiseError::HandshakeLength`] for another size. [`NoiseError::Handshake`] when the
    /// message was not written for the key of this hub.
    pub fn respond(
        hub: &StaticKeypair,
        first: &[u8],
    ) -> Result<(Self, [u8; HANDSHAKE_SECOND_LEN]), NoiseError> {
        // The size is checked before any key work is done for a stranger.
        if first.len() != HANDSHAKE_FIRST_LEN {
            return Err(NoiseError::HandshakeLength);
        }
        let mut state = builder()?
            .local_private_key(hub.private.expose())
            .map_err(library)?
            .build_responder()
            .map_err(library)?;
        read_exact(&mut state, first, HANDSHAKE_FIRST_LEN)?;
        let second = write_exact(&mut state)?;
        Ok((Self { state }, second))
    }

    /// Reads the third message (64 bytes). The channel that comes out carries the identity the
    /// other side proved to hold.
    ///
    /// # Errors
    /// [`NoiseError::HandshakeLength`] for another size. [`NoiseError::Handshake`] when the
    /// message does not prove the identity it shows, or shows a low-order point, which is
    /// nobody's identity.
    pub fn finish(mut self, third: &[u8]) -> Result<Channel, NoiseError> {
        read_exact(&mut self.state, third, HANDSHAKE_THIRD_LEN)?;
        Channel::from_handshake(self.state)
    }
}

impl fmt::Debug for HubHandshake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HubHandshake").finish_non_exhaustive()
    }
}

/// What a finished handshake leaves: the hash, the static key of the other side and the two
/// halves of the transport.
#[derive(Debug)]
pub struct Channel {
    /// The handshake hash, read before the transport took over.
    pub handshake_hash: HandshakeHash,
    /// The static key of the other side. For the hub, the identity of the device. For the
    /// connector, the key of the hub it asked for.
    pub remote_static: PublicKey,
    /// Encrypts what this side sends. It belongs to the one task that writes on the websocket.
    pub sealer: Sealer,
    /// Decrypts what this side receives.
    pub opener: Opener,
}

impl Channel {
    fn from_handshake(state: HandshakeState) -> Result<Self, NoiseError> {
        if !state.is_handshake_finished() {
            return Err(NoiseError::Library);
        }
        let handshake_hash = <[u8; HANDSHAKE_HASH_LEN]>::try_from(state.get_handshake_hash())
            .map(HandshakeHash)
            .map_err(|_| NoiseError::Library)?;
        let remote_static = state
            .get_remote_static()
            .and_then(PublicKey::from_slice)
            .ok_or(NoiseError::Library)?;
        let transport = Arc::new(state.into_stateless_transport_mode().map_err(library)?);
        Ok(Self {
            handshake_hash,
            remote_static,
            sealer: Sealer {
                transport: Arc::clone(&transport),
                counter: 0,
            },
            opener: Opener {
                transport,
                counter: 0,
                failed: false,
            },
        })
    }
}

/// The sending half of the transport.
///
/// Only the task that writes on the websocket seals, at the moment of writing and after it
/// picked between the control queue and the data queue. Sealing when a frame is queued would
/// break the counter at the first frame that jumps the queue.
pub struct Sealer {
    transport: Arc<StatelessTransportState>,
    counter: u64,
}

impl Sealer {
    /// Encrypts one frame. The result has 16 bytes more than `plaintext` and is one binary
    /// websocket message.
    ///
    /// # Errors
    /// [`NoiseError::TooLarge`] when `plaintext` has more than 65,519 bytes; nothing was
    /// consumed and the channel goes on. [`NoiseError::CounterExhausted`] at the end of the
    /// counter.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if plaintext.len() > NOISE_PLAINTEXT_MAX {
            return Err(NoiseError::TooLarge);
        }
        // The last value is reserved by Noise.
        let next = self
            .counter
            .checked_add(1)
            .ok_or(NoiseError::CounterExhausted)?;
        let mut message = vec![0u8; plaintext.len().saturating_add(NOISE_TAG_LEN)];
        let written = self
            .transport
            .write_message(self.counter, plaintext, &mut message)
            .map_err(library)?;
        message.truncate(written);
        self.counter = next;
        Ok(message)
    }

    /// Frames sealed so far, which is also the counter of the next one.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.counter
    }
}

impl fmt::Debug for Sealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sealer")
            .field("counter", &self.counter)
            .finish_non_exhaustive()
    }
}

/// The receiving half of the transport.
pub struct Opener {
    transport: Arc<StatelessTransportState>,
    counter: u64,
    failed: bool,
}

impl Opener {
    /// Decrypts one websocket message into a frame.
    ///
    /// The counter is never taken from the wire: this half uses its own. So a frame that is
    /// repeated, skipped, out of order or changed fails here, and after the first failure
    /// every call fails: the connection has to be closed.
    ///
    /// # Errors
    /// [`NoiseError::Decrypt`], [`NoiseError::TooLarge`], [`NoiseError::CounterExhausted`], and
    /// [`NoiseError::Closed`] after any of them.
    pub fn open(&mut self, message: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if self.failed {
            return Err(NoiseError::Closed);
        }
        self.open_next(message).inspect_err(|_| self.failed = true)
    }

    fn open_next(&mut self, message: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if message.len() > WS_MESSAGE_MAX {
            return Err(NoiseError::TooLarge);
        }
        let plaintext_len = message
            .len()
            .checked_sub(NOISE_TAG_LEN)
            .ok_or(NoiseError::Decrypt)?;
        let next = self
            .counter
            .checked_add(1)
            .ok_or(NoiseError::CounterExhausted)?;
        // Sized by the bytes that arrived, never by a number the other side announced.
        let mut frame = vec![0u8; plaintext_len];
        let read = self
            .transport
            .read_message(self.counter, message, &mut frame)
            .map_err(|_| NoiseError::Decrypt)?;
        frame.truncate(read);
        self.counter = next;
        Ok(frame)
    }

    /// Frames opened so far, which is also the counter of the next one.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.counter
    }
}

impl fmt::Debug for Opener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opener")
            .field("counter", &self.counter)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}
