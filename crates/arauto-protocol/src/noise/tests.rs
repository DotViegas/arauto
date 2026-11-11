use super::*;
use crate::secret::wipe_probe;
use crate::test_util::{assert_hides, hex, work_probe};

/// A finished handshake between a new connector and a new hub.
struct Pair {
    identity: StaticKeypair,
    hub: StaticKeypair,
    connector: Channel,
    at_hub: Channel,
}

fn handshake() -> Pair {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let (connector, at_hub) = handshake_between(&identity, &hub);
    Pair {
        identity,
        hub,
        connector,
        at_hub,
    }
}

fn handshake_between(identity: &StaticKeypair, hub: &StaticKeypair) -> (Channel, Channel) {
    let (pending_connector, first) = ConnectorHandshake::start(identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(hub, &first).unwrap();
    let (third, connector) = pending_connector.finish(&second).unwrap();
    let at_hub = pending_hub.finish(&third).unwrap();
    (connector, at_hub)
}

fn key(hexadecimal: &str) -> [u8; 32] {
    let bytes = data_encoding::HEXLOWER
        .decode(hexadecimal.as_bytes())
        .unwrap();
    <[u8; 32]>::try_from(bytes.as_slice()).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------------------------

#[test]
fn the_public_key_follows_rfc_7748() {
    // RFC 7748, section 6.1: the key pairs of Alice and of Bob.
    let alice = StaticKeypair::from_private(Secret::new(key(
        "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
    )));
    assert_eq!(
        hex(alice.public().as_bytes()),
        "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
    );
    let bob = StaticKeypair::from_private(Secret::new(key(
        "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
    )));
    assert_eq!(
        hex(bob.public().as_bytes()),
        "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
    );
}

#[test]
fn generated_key_pairs_differ_and_can_be_rebuilt() {
    let a = StaticKeypair::generate().unwrap();
    let b = StaticKeypair::generate().unwrap();
    assert_ne!(a.public(), b.public());
    assert_ne!(a.private(), b.private());
    assert_ne!(a.private().expose(), &[0u8; 32]);
    let rebuilt = StaticKeypair::from_private(Secret::new(*a.private().expose()));
    assert_eq!(rebuilt, a);
    assert_eq!(rebuilt.public(), a.public());
}

#[test]
fn public_keys_are_read_from_exactly_32_bytes() {
    assert_eq!(
        PublicKey::from_slice(&[7u8; 32]),
        Some(PublicKey::from_bytes([7u8; 32]))
    );
    assert_eq!(PublicKey::from_slice(&[7u8; 31]), None);
    assert_eq!(PublicKey::from_slice(&[7u8; 33]), None);
    assert_eq!(PublicKey::from_slice(&[]), None);
    assert_eq!(
        format!("{:?}", PublicKey::from_bytes([0xa5; 32])),
        format!("PublicKey({})", "a5".repeat(32))
    );
}

#[test]
fn a_key_pair_does_not_print_its_private_part() {
    let private = key("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let pair = StaticKeypair::from_private(Secret::new(private));
    let debug = format!("{pair:?} {pair:#?}");
    assert_hides(&debug, &private, "Debug of StaticKeypair");
    assert!(debug.contains("[redacted]"));
    assert!(
        debug.contains("8520f0098930a754"),
        "the public part is not a secret: {debug}"
    );
    assert_not_impl!(StaticKeypair: core::fmt::Display, serde::Serialize, Clone);
}

// ---------------------------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_messages_have_48_48_and_64_bytes() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    let (third, _) = pending_connector.finish(&second).unwrap();
    assert!(pending_hub.finish(&third).is_ok());
    assert_eq!((first.len(), second.len(), third.len()), (48, 48, 64));
    assert_eq!(
        (
            HANDSHAKE_FIRST_LEN,
            HANDSHAKE_SECOND_LEN,
            HANDSHAKE_THIRD_LEN
        ),
        (first.len(), second.len(), third.len())
    );
}

#[test]
fn both_sides_reach_the_same_handshake_hash() {
    let pair = handshake();
    assert_eq!(pair.connector.handshake_hash, pair.at_hub.handshake_hash);
    assert_ne!(pair.connector.handshake_hash.as_bytes(), &[0u8; 32]);
    // Another handshake between the same two keys has another hash: the ephemeral keys change.
    let (again, _) = handshake_between(&pair.identity, &pair.hub);
    assert_ne!(again.handshake_hash, pair.connector.handshake_hash);
}

#[test]
fn the_hub_learns_the_identity_and_the_connector_confirms_the_hub() {
    let pair = handshake();
    assert_eq!(&pair.at_hub.remote_static, pair.identity.public());
    assert_eq!(&pair.connector.remote_static, pair.hub.public());
}

#[test]
fn the_wrong_hub_key_does_not_complete() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let other = StaticKeypair::generate().unwrap();

    // The connector has the key of another hub in its block: this hub cannot even read the
    // first message.
    let (_, first) = ConnectorHandshake::start(&identity, other.public()).unwrap();
    assert_eq!(
        HubHandshake::respond(&hub, &first).err(),
        Some(NoiseError::Handshake)
    );
}

#[test]
fn a_false_hub_cannot_answer() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let false_hub = StaticKeypair::generate().unwrap();

    // A server without the private key of the block cannot read what the connector sent.
    let (pending, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    assert_eq!(
        HubHandshake::respond(&false_hub, &first).err(),
        Some(NoiseError::Handshake)
    );

    // Whatever it sends back instead does not complete the handshake: noise, zeros, or the
    // second message of a handshake of its own.
    let (_, own_first) = ConnectorHandshake::start(&identity, false_hub.public()).unwrap();
    let (_, own_second) = HubHandshake::respond(&false_hub, &own_first).unwrap();
    assert_eq!(
        pending.finish(&own_second).err(),
        Some(NoiseError::Handshake)
    );

    let (pending, _) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    assert_eq!(
        pending.finish(&[0u8; 48]).err(),
        Some(NoiseError::Handshake)
    );
    let (pending, _) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    assert_eq!(
        pending.finish(&[0xa7u8; 48]).err(),
        Some(NoiseError::Handshake)
    );
}

#[test]
fn only_the_holder_of_the_identity_completes_the_third_message() {
    let identity = StaticKeypair::generate().unwrap();
    let thief = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();

    // The third message of another handshake, made with another identity, proves nothing here.
    let (pending_thief, thief_first) = ConnectorHandshake::start(&thief, hub.public()).unwrap();
    let (_, thief_second) = HubHandshake::respond(&hub, &thief_first).unwrap();
    let (thief_third, _) = pending_thief.finish(&thief_second).unwrap();

    let (_, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, _) = HubHandshake::respond(&hub, &first).unwrap();
    assert_eq!(
        pending_hub.finish(&thief_third).err(),
        Some(NoiseError::Handshake)
    );
}

#[test]
fn handshake_messages_of_another_size_are_refused() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let (_, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();

    for len in [0usize, 1, 32, 47, 49, 64, 96, 65_535] {
        let mut message = first.to_vec();
        message.resize(len, 0);
        assert_eq!(
            HubHandshake::respond(&hub, &message).err(),
            Some(NoiseError::HandshakeLength),
            "first message with {len} bytes"
        );
    }
    let (_, second) = HubHandshake::respond(&hub, &first).unwrap();
    for len in [0usize, 47, 49, 64] {
        let (pending, own_first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
        let (_, own_second) = HubHandshake::respond(&hub, &own_first).unwrap();
        let mut message = own_second.to_vec();
        message.resize(len, 0);
        assert_eq!(
            pending.finish(&message).err(),
            Some(NoiseError::HandshakeLength),
            "second message with {len} bytes"
        );
    }
    assert_eq!(second.len(), 48);
    for len in [0usize, 48, 63, 65, 80] {
        let (pending_connector, own_first) =
            ConnectorHandshake::start(&identity, hub.public()).unwrap();
        let (pending_hub, own_second) = HubHandshake::respond(&hub, &own_first).unwrap();
        let (third, _) = pending_connector.finish(&own_second).unwrap();
        let mut message = third.to_vec();
        message.resize(len, 0);
        assert_eq!(
            pending_hub.finish(&message).err(),
            Some(NoiseError::HandshakeLength),
            "third message with {len} bytes"
        );
    }
}

#[test]
fn a_changed_handshake_message_does_not_complete() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();

    // First message: every byte is covered, the ephemeral key through the key it derives.
    for position in [0usize, 15, 31, 32, 40, 47] {
        let (_, mut first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
        first[position] ^= 0x01;
        assert_eq!(
            HubHandshake::respond(&hub, &first).err(),
            Some(NoiseError::Handshake),
            "first message, byte {position}"
        );
    }
    for position in [0usize, 15, 31, 32, 40, 47] {
        let (pending, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
        let (_, mut second) = HubHandshake::respond(&hub, &first).unwrap();
        second[position] ^= 0x01;
        assert_eq!(
            pending.finish(&second).err(),
            Some(NoiseError::Handshake),
            "second message, byte {position}"
        );
    }
    for position in [0usize, 20, 31, 32, 47, 48, 55, 63] {
        let (pending_connector, first) =
            ConnectorHandshake::start(&identity, hub.public()).unwrap();
        let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
        let (mut third, _) = pending_connector.finish(&second).unwrap();
        third[position] ^= 0x01;
        assert_eq!(
            pending_hub.finish(&third).err(),
            Some(NoiseError::Handshake),
            "third message, byte {position}"
        );
    }
}

#[test]
fn handshake_messages_cannot_be_replayed_into_another_handshake() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();

    // A recorded handshake.
    let (recorded_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (recorded_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    let (third, _) = recorded_connector.finish(&second).unwrap();
    assert!(recorded_hub.finish(&third).is_ok());

    // The hub answers a replayed first message with a new ephemeral key, so the recorded third
    // message does not fit the new handshake.
    let (pending_hub, new_second) = HubHandshake::respond(&hub, &first).unwrap();
    assert_ne!(new_second, second);
    assert_eq!(
        pending_hub.finish(&third).err(),
        Some(NoiseError::Handshake)
    );

    // And a recorded second message does not fit a new handshake of the connector.
    let (pending_connector, _) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    assert_eq!(
        pending_connector.finish(&second).err(),
        Some(NoiseError::Handshake)
    );
}

/// The handshake and the prologue are frozen forever. Here the other side is written straight
/// on the library, with the protocol name and the prologue spelled out again: if either
/// changed in the crate, a connector of any age would stop getting through.
#[test]
fn the_handshake_is_plain_noise_xk_with_the_arauto_prologue() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let params = || {
        "Noise_XK_25519_ChaChaPoly_BLAKE2s"
            .parse::<snow::params::NoiseParams>()
            .unwrap()
    };

    // A plain initiator against our hub.
    let mut plain = snow::Builder::new(params())
        .local_private_key(identity.private().expose())
        .unwrap()
        .remote_public_key(hub.public().as_bytes())
        .unwrap()
        .prologue(b"arauto/1")
        .unwrap()
        .build_initiator()
        .unwrap();
    let mut buffer = [0u8; 128];
    let mut payload = [0u8; 16];
    let len = plain.write_message(&[], &mut buffer).unwrap();
    assert_eq!(len, 48);
    let (pending_hub, second) = HubHandshake::respond(&hub, &buffer[..len]).unwrap();
    plain.read_message(&second, &mut payload).unwrap();
    let len = plain.write_message(&[], &mut buffer).unwrap();
    assert_eq!(len, 64);
    let mut at_hub = pending_hub.finish(&buffer[..len]).unwrap();
    assert_eq!(
        at_hub.handshake_hash.as_bytes().as_slice(),
        plain.get_handshake_hash()
    );
    assert_eq!(&at_hub.remote_static, identity.public());

    // The transport with the counter kept by the library reads what our halves write, and the
    // other way round: our counter is the standard one, from zero, one per message.
    let mut plain = plain.into_transport_mode().unwrap();
    let mut out = [0u8; 256];
    for round in 0..3u8 {
        let sealed = at_hub.sealer.seal(&[round; 9]).unwrap();
        let len = plain.read_message(&sealed, &mut out).unwrap();
        assert_eq!(&out[..len], [round; 9]);
        let len = plain.write_message(&[round; 5], &mut out).unwrap();
        assert_eq!(at_hub.opener.open(&out[..len]).unwrap(), [round; 5]);
    }

    // A plain responder against our connector.
    let mut plain = snow::Builder::new(params())
        .local_private_key(hub.private().expose())
        .unwrap()
        .prologue(b"arauto/1")
        .unwrap()
        .build_responder()
        .unwrap();
    let (pending, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    plain.read_message(&first, &mut payload).unwrap();
    let len = plain.write_message(&[], &mut buffer).unwrap();
    let (third, connector) = pending.finish(&buffer[..len]).unwrap();
    plain.read_message(&third, &mut payload).unwrap();
    assert_eq!(
        connector.handshake_hash.as_bytes().as_slice(),
        plain.get_handshake_hash()
    );
    assert_eq!(
        plain.get_remote_static(),
        Some(identity.public().as_bytes().as_slice())
    );
}

#[test]
fn another_prologue_does_not_complete() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    for prologue in [&b"arauto/2"[..], b"", b"arauto/1 ", b"ARAUTO/1"] {
        let mut stranger = snow::Builder::new(NOISE_PATTERN.parse().unwrap())
            .local_private_key(identity.private().expose())
            .unwrap()
            .remote_public_key(hub.public().as_bytes())
            .unwrap()
            .prologue(prologue)
            .unwrap()
            .build_initiator()
            .unwrap();
        let mut buffer = [0u8; 128];
        let len = stranger.write_message(&[], &mut buffer).unwrap();
        assert_eq!(
            HubHandshake::respond(&hub, &buffer[..len]).err(),
            Some(NoiseError::Handshake),
            "{prologue:?}"
        );
    }
    assert_eq!(PROLOGUE, b"arauto/1");
    assert_eq!(NOISE_PATTERN, "Noise_XK_25519_ChaChaPoly_BLAKE2s");
}

#[test]
fn a_handshake_message_with_a_payload_is_refused() {
    // The third message of a plain initiator that smuggles a payload has another size; a hub
    // never sees a payload because the sizes are exact.
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let mut plain = snow::Builder::new(NOISE_PATTERN.parse().unwrap())
        .local_private_key(identity.private().expose())
        .unwrap()
        .remote_public_key(hub.public().as_bytes())
        .unwrap()
        .prologue(PROLOGUE)
        .unwrap()
        .build_initiator()
        .unwrap();
    let mut buffer = [0u8; 128];
    let len = plain.write_message(b"extra", &mut buffer).unwrap();
    assert_eq!(len, 53);
    assert_eq!(
        HubHandshake::respond(&hub, &buffer[..len]).err(),
        Some(NoiseError::HandshakeLength)
    );
}

// ---------------------------------------------------------------------------------------------
// Keys nobody holds
// ---------------------------------------------------------------------------------------------

/// The low-order points of Curve25519, in every form that is read as one of them: the seven
/// values every implementation lists, and each with the top bit set, which X25519 ignores.
fn low_order_points() -> Vec<[u8; 32]> {
    let listed = [
        // 0, of order 4.
        "0000000000000000000000000000000000000000000000000000000000000000",
        // 1, of order 1.
        "0100000000000000000000000000000000000000000000000000000000000000",
        // The two of order 8.
        "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800",
        "5f9c95bca3508c24b1d0b1559c83ef5b04445cc4581c8e86d8224eddd09f1157",
        // p - 1, of order 2; then p and p + 1, which are 0 and 1 written another way.
        "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
    ];
    let mut points = Vec::new();
    for text in listed {
        let point = key(text);
        let mut top_bit = point;
        top_bit[31] |= 0x80;
        points.push(point);
        points.push(top_bit);
    }
    points
}

#[test]
fn low_order_points_are_told_apart_from_keys() {
    let points = low_order_points();
    assert_eq!(points.len(), 14);
    for point in &points {
        assert!(
            PublicKey::from_bytes(*point).is_low_order(),
            "{}",
            hex(point)
        );
        // With such a point every private key gives the same result: that is what makes it
        // nobody's key.
        for private in [
            [0x42u8; 32],
            [0x07u8; 32],
            *b"any private key at all, 32 bytes",
        ] {
            let shared = StaticSecret::from(private).diffie_hellman(&DalekPublicKey::from(*point));
            assert_eq!(shared.as_bytes(), &[0u8; 32], "{}", hex(point));
        }
    }
    // Keys people hold are not of low order: the base point, the keys of RFC 7748, a neighbour
    // of each listed point, and whatever is generated.
    let mut ordinary = vec![
        key("0900000000000000000000000000000000000000000000000000000000000000"),
        key("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"),
        key("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"),
        key("0200000000000000000000000000000000000000000000000000000000000000"),
        key("e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b801"),
        key("ebffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f"),
    ];
    for _ in 0..32 {
        ordinary.push(*StaticKeypair::generate().unwrap().public().as_bytes());
    }
    for point in ordinary {
        assert!(
            !PublicKey::from_bytes(point).is_low_order(),
            "{}",
            hex(&point)
        );
    }
}

/// What anyone can do without a private key: show `claimed` as the static key and take every
/// exchange made with it to be zero. With `claimed` empty it is an honest initiator, the
/// control of the tests that use it.
struct Impostor {
    inner: Box<dyn Dh>,
    is_static: bool,
    claimed: Option<[u8; 32]>,
}

impl Dh for Impostor {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn pub_len(&self) -> usize {
        32
    }

    fn priv_len(&self) -> usize {
        32
    }

    fn set(&mut self, privkey: &[u8]) {
        // Only the static key is set from outside; the ephemeral one is generated.
        self.is_static = true;
        self.inner.set(privkey);
    }

    fn generate(&mut self, rng: &mut dyn Random) -> Result<(), snow::Error> {
        self.inner.generate(rng)
    }

    fn pubkey(&self) -> &[u8] {
        match &self.claimed {
            Some(claimed) if self.is_static => claimed,
            _ => self.inner.pubkey(),
        }
    }

    fn privkey(&self) -> &[u8] {
        self.inner.privkey()
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) -> Result<(), snow::Error> {
        if self.is_static && self.claimed.is_some() {
            out[..32].fill(0);
            Ok(())
        } else {
            self.inner.dh(pubkey, out)
        }
    }
}

struct ImpostorResolver(Option<[u8; 32]>);

impl CryptoResolver for ImpostorResolver {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        DefaultResolver.resolve_rng()
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        Some(Box::new(Impostor {
            inner: DefaultResolver.resolve_dh(choice)?,
            is_static: false,
            claimed: self.0,
        }))
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        DefaultResolver.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        DefaultResolver.resolve_cipher(choice)
    }
}

/// Runs the three messages against our hub with an initiator that claims `claimed` as its
/// identity, and gives what the hub concluded.
fn hub_facing_an_impostor(
    hub: &StaticKeypair,
    private: &[u8; 32],
    claimed: Option<[u8; 32]>,
) -> Result<Channel, NoiseError> {
    let mut impostor = snow::Builder::with_resolver(
        NOISE_PATTERN.parse().unwrap(),
        Box::new(ImpostorResolver(claimed)),
    )
    .local_private_key(private)
    .unwrap()
    .remote_public_key(hub.public().as_bytes())
    .unwrap()
    .prologue(PROLOGUE)
    .unwrap()
    .build_initiator()
    .unwrap();
    let mut buffer = [0u8; 128];
    let mut payload = [0u8; 16];
    let len = impostor.write_message(&[], &mut buffer).unwrap();
    let (pending, second) = HubHandshake::respond(hub, &buffer[..len])?;
    impostor.read_message(&second, &mut payload).unwrap();
    let len = impostor.write_message(&[], &mut buffer).unwrap();
    assert_eq!(len, 64);
    pending.finish(&buffer[..len])
}

/// The handshake proves to the hub that the other side holds the private key of the identity
/// it shows. A low-order point has no private key: without the refusal anyone completes the
/// handshake as that identity, and a device approved with it would be anyone's.
#[test]
fn the_hub_refuses_an_identity_nobody_holds() {
    let hub = StaticKeypair::generate().unwrap();

    // The control: the same initiator, when it does hold its key, gets through.
    let honest = StaticKeypair::generate().unwrap();
    let channel = hub_facing_an_impostor(&hub, honest.private().expose(), None).unwrap();
    assert_eq!(&channel.remote_static, honest.public());

    for point in low_order_points() {
        // Any 32 bytes: this "private key" never enters a computation.
        let outcome = hub_facing_an_impostor(&hub, &[0x42u8; 32], Some(point));
        assert_eq!(
            outcome.err(),
            Some(NoiseError::Handshake),
            "the hub took {} as an identity",
            hex(&point)
        );
    }
}

/// A block with a low-order point as the key of the hub describes a hub anyone can be: the
/// connector refuses it before it sends anything.
#[test]
fn the_connector_refuses_a_hub_key_nobody_holds() {
    let identity = StaticKeypair::generate().unwrap();
    for point in low_order_points() {
        assert_eq!(
            ConnectorHandshake::start(&identity, &PublicKey::from_bytes(point)).err(),
            Some(NoiseError::Handshake),
            "{}",
            hex(&point)
        );
    }
    // The control.
    let hub = StaticKeypair::generate().unwrap();
    assert!(ConnectorHandshake::start(&identity, hub.public()).is_ok());
}

/// The Noise library keeps the private keys it is given in plain arrays, which are freed as
/// they are. Ours are wiped: the two of each side, static and ephemeral, when its handshake
/// state goes away, whether the handshake completed or not.
#[test]
fn the_private_keys_of_a_handshake_are_wiped_with_its_state() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let _ = wipe_probe::take();

    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    assert_eq!(wipe_probe::take(), (0, true), "the states are still alive");
    let (third, connector) = pending_connector.finish(&second).unwrap();
    assert_eq!(wipe_probe::take(), (2, true), "the state of the connector");
    let at_hub = pending_hub.finish(&third).unwrap();
    assert_eq!(wipe_probe::take(), (2, true), "the state of the hub");
    // The transport holds no private key.
    drop((connector, at_hub));
    assert_eq!(wipe_probe::take(), (0, true));

    // A handshake that is abandoned or fails.
    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, _) = HubHandshake::respond(&hub, &first).unwrap();
    drop(pending_connector);
    assert_eq!(wipe_probe::take(), (2, true));
    assert!(pending_hub.finish(&[0u8; 64]).is_err());
    assert_eq!(wipe_probe::take(), (2, true));
    // Refused at the first message: the hub had not drawn its ephemeral key yet.
    assert!(HubHandshake::respond(&hub, &[0u8; 48]).is_err());
    assert_eq!(wipe_probe::take(), (1, true));

    // The key pairs themselves were not touched.
    assert_ne!(identity.private().expose(), &[0u8; 32]);
    assert_ne!(hub.private().expose(), &[0u8; 32]);
}

/// Anyone reaches the hub with a first message. One of the wrong size is refused by its size
/// alone, before the hub loads its private key into a handshake for a stranger.
#[test]
fn the_size_of_the_first_message_is_checked_before_any_key_work() {
    let identity = StaticKeypair::generate().unwrap();
    let hub = StaticKeypair::generate().unwrap();
    let (_, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let _ = work_probe::take(KEY_WORK);

    for len in [0usize, 1, 47, 49, 64, 65_535] {
        let mut message = first.to_vec();
        message.resize(len, 0);
        assert_eq!(
            HubHandshake::respond(&hub, &message).err(),
            Some(NoiseError::HandshakeLength)
        );
        assert_eq!(
            work_probe::take(KEY_WORK),
            0,
            "the key of the hub was loaded for a first message of {len} bytes"
        );
    }
    // The control: a message of the right size does get the key of the hub loaded, even when
    // it is noise, and a good one gets the ephemeral key too.
    assert!(HubHandshake::respond(&hub, &[0xa7u8; 48]).is_err());
    assert_eq!(work_probe::take(KEY_WORK), 1);
    assert!(HubHandshake::respond(&hub, &first).is_ok());
    assert_eq!(work_probe::take(KEY_WORK), 2);
}

// ---------------------------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------------------------

#[test]
fn frames_go_both_ways() {
    let mut pair = handshake();
    let sealed = pair.connector.sealer.seal(b"from the connector").unwrap();
    assert_eq!(sealed.len(), 18 + 16);
    assert_ne!(&sealed[..18], b"from the connector");
    assert_eq!(
        pair.at_hub.opener.open(&sealed).unwrap(),
        b"from the connector"
    );

    let sealed = pair.at_hub.sealer.seal(b"from the hub").unwrap();
    assert_eq!(
        pair.connector.opener.open(&sealed).unwrap(),
        b"from the hub"
    );

    // An empty frame is still authenticated.
    let sealed = pair.connector.sealer.seal(b"").unwrap();
    assert_eq!(sealed.len(), 16);
    assert_eq!(pair.at_hub.opener.open(&sealed).unwrap(), b"");
}

#[test]
fn counters_start_at_zero_and_go_up_by_one() {
    let mut pair = handshake();
    assert_eq!(pair.connector.sealer.counter(), 0);
    assert_eq!(pair.at_hub.opener.counter(), 0);
    for expected in 0..20u64 {
        assert_eq!(pair.connector.sealer.counter(), expected);
        let sealed = pair.connector.sealer.seal(&expected.to_be_bytes()).unwrap();
        assert_eq!(pair.at_hub.opener.counter(), expected);
        assert_eq!(
            pair.at_hub.opener.open(&sealed).unwrap(),
            expected.to_be_bytes()
        );
    }
    assert_eq!(pair.connector.sealer.counter(), 20);
    assert_eq!(pair.at_hub.opener.counter(), 20);
}

#[test]
fn the_counter_does_not_travel() {
    let mut pair = handshake();
    // Same content, different counter: different bytes, and not one byte longer than the
    // content plus the tag.
    let first = pair.connector.sealer.seal(b"same content").unwrap();
    let second = pair.connector.sealer.seal(b"same content").unwrap();
    assert_ne!(first, second);
    assert_eq!(first.len(), b"same content".len() + NOISE_TAG_LEN);
    assert_eq!(second.len(), first.len());
}

#[test]
fn the_two_directions_have_independent_counters() {
    let mut pair = handshake();
    // Five frames one way.
    for round in 0..5u8 {
        let sealed = pair.connector.sealer.seal(&[round]).unwrap();
        assert_eq!(pair.at_hub.opener.open(&sealed).unwrap(), [round]);
    }
    // The other way is still at zero.
    assert_eq!(pair.at_hub.sealer.counter(), 0);
    assert_eq!(pair.connector.opener.counter(), 0);
    let sealed = pair.at_hub.sealer.seal(b"first from the hub").unwrap();
    assert_eq!(
        pair.connector.opener.open(&sealed).unwrap(),
        b"first from the hub"
    );
    assert_eq!(
        (
            pair.connector.sealer.counter(),
            pair.at_hub.opener.counter()
        ),
        (5, 5)
    );
    assert_eq!(
        (
            pair.at_hub.sealer.counter(),
            pair.connector.opener.counter()
        ),
        (1, 1)
    );

    // The directions also have different keys: a frame does not open on the side that made it.
    let own = pair.connector.sealer.seal(b"mine").unwrap();
    assert_eq!(pair.connector.opener.open(&own), Err(NoiseError::Decrypt));
}

#[test]
fn a_repeated_frame_does_not_open() {
    let mut pair = handshake();
    let sealed = pair.connector.sealer.seal(b"once").unwrap();
    assert_eq!(pair.at_hub.opener.open(&sealed).unwrap(), b"once");
    assert_eq!(pair.at_hub.opener.open(&sealed), Err(NoiseError::Decrypt));
    // The channel is over: not even the right next frame opens.
    let next = pair.connector.sealer.seal(b"next").unwrap();
    assert_eq!(pair.at_hub.opener.open(&next), Err(NoiseError::Closed));
}

#[test]
fn a_frame_out_of_order_does_not_open() {
    let mut pair = handshake();
    let first = pair.connector.sealer.seal(b"first").unwrap();
    let second = pair.connector.sealer.seal(b"second").unwrap();
    assert_eq!(pair.at_hub.opener.open(&second), Err(NoiseError::Decrypt));
    assert_eq!(pair.at_hub.opener.open(&first), Err(NoiseError::Closed));
}

#[test]
fn a_skipped_frame_does_not_open() {
    let mut pair = handshake();
    let first = pair.at_hub.sealer.seal(b"first").unwrap();
    let _lost = pair.at_hub.sealer.seal(b"lost on the way").unwrap();
    let third = pair.at_hub.sealer.seal(b"third").unwrap();
    assert_eq!(pair.connector.opener.open(&first).unwrap(), b"first");
    assert_eq!(pair.connector.opener.open(&third), Err(NoiseError::Decrypt));
    assert_eq!(
        pair.connector.opener.counter(),
        1,
        "a failure does not move the counter"
    );
}

#[test]
fn a_changed_frame_does_not_open() {
    let content = b"twenty bytes of data";
    for position in [0usize, 1, 10, 19, 20, 27, 35] {
        for mask in [0x01u8, 0x80] {
            let mut pair = handshake();
            let mut sealed = pair.connector.sealer.seal(content).unwrap();
            assert_eq!(sealed.len(), 36);
            sealed[position] ^= mask;
            assert_eq!(
                pair.at_hub.opener.open(&sealed),
                Err(NoiseError::Decrypt),
                "byte {position}, mask {mask:#x}"
            );
        }
    }
}

#[test]
fn a_cut_or_extended_frame_does_not_open() {
    for change in [-36isize, -20, -17, -16, -1, 1, 16] {
        let mut pair = handshake();
        let mut sealed = pair.connector.sealer.seal(b"twenty bytes of data").unwrap();
        let new_len = sealed.len().checked_add_signed(change).unwrap();
        sealed.resize(new_len, 0);
        assert_eq!(
            pair.at_hub.opener.open(&sealed),
            Err(NoiseError::Decrypt),
            "{change}"
        );
        assert_eq!(pair.at_hub.opener.open(&sealed), Err(NoiseError::Closed));
    }
}

#[test]
fn a_frame_of_another_session_does_not_open() {
    let mut one = handshake();
    let mut other = handshake();
    let sealed = one.connector.sealer.seal(b"for the first hub").unwrap();
    assert_eq!(other.at_hub.opener.open(&sealed), Err(NoiseError::Decrypt));
    // Not even between the same two keys: each handshake has its own transport keys.
    let (mut again, mut again_at_hub) = handshake_between(&one.identity, &one.hub);
    let sealed = again.sealer.seal(b"new session").unwrap();
    assert_eq!(one.at_hub.opener.open(&sealed), Err(NoiseError::Decrypt));
    let sealed = again.sealer.seal(b"second of the new session").unwrap();
    assert_eq!(
        again_at_hub.opener.open(&sealed),
        Err(NoiseError::Decrypt),
        "frame 0 was skipped"
    );
}

#[test]
fn size_limits_of_one_noise_message() {
    let mut pair = handshake();
    assert_eq!(NOISE_PLAINTEXT_MAX, 65_519);
    let largest = vec![0x5au8; NOISE_PLAINTEXT_MAX];
    let sealed = pair.connector.sealer.seal(&largest).unwrap();
    assert_eq!(sealed.len(), WS_MESSAGE_MAX);
    assert_eq!(pair.at_hub.opener.open(&sealed).unwrap(), largest);

    // Too large to seal: nothing is consumed and the channel goes on.
    let too_large = vec![0x5au8; NOISE_PLAINTEXT_MAX + 1];
    assert_eq!(
        pair.connector.sealer.seal(&too_large),
        Err(NoiseError::TooLarge)
    );
    assert_eq!(pair.connector.sealer.counter(), 1);
    let sealed = pair.connector.sealer.seal(b"still here").unwrap();
    assert_eq!(pair.at_hub.opener.open(&sealed).unwrap(), b"still here");

    // Too large to be a websocket message of this protocol: refused before any work.
    assert_eq!(
        pair.at_hub.opener.open(&vec![0u8; WS_MESSAGE_MAX + 1]),
        Err(NoiseError::TooLarge)
    );
    assert_eq!(pair.at_hub.opener.open(&sealed), Err(NoiseError::Closed));
}

#[test]
fn every_frame_of_the_protocol_fits_one_websocket_message() {
    let mut pair = handshake();
    let frame = vec![1u8; crate::limits::FRAME_PLAINTEXT_MAX];
    let sealed = pair.at_hub.sealer.seal(&frame).unwrap();
    assert_eq!(sealed.len(), 60_021);
    assert!(sealed.len() <= WS_MESSAGE_MAX);
    assert_eq!(pair.connector.opener.open(&sealed).unwrap(), frame);
}

#[test]
fn the_counter_ends_instead_of_wrapping() {
    let mut pair = handshake();
    pair.connector.sealer.counter = u64::MAX - 1;
    pair.at_hub.opener.counter = u64::MAX - 1;
    // The last usable value still works.
    let sealed = pair.connector.sealer.seal(b"last").unwrap();
    assert_eq!(pair.at_hub.opener.open(&sealed).unwrap(), b"last");
    assert_eq!(pair.connector.sealer.counter(), u64::MAX);
    // After it there is no counter left: the value zero never comes back.
    assert_eq!(
        pair.connector.sealer.seal(b"one more"),
        Err(NoiseError::CounterExhausted)
    );
    assert_eq!(
        pair.at_hub.opener.open(&sealed),
        Err(NoiseError::CounterExhausted)
    );
    assert_eq!(pair.at_hub.opener.open(&sealed), Err(NoiseError::Closed));
}

#[test]
fn the_halves_can_move_to_different_tasks() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Sealer>();
    assert_send::<Opener>();
    assert_send::<Channel>();
    assert_send::<ConnectorHandshake>();
    assert_send::<HubHandshake>();
    assert_sync::<Sealer>();
    assert_sync::<Opener>();
    assert_send::<StaticKeypair>();
    assert_sync::<StaticKeypair>();

    let pair = handshake();
    let Channel { mut sealer, .. } = pair.connector;
    let Channel { mut opener, .. } = pair.at_hub;
    let writer = std::thread::spawn(move || {
        (0..50u8)
            .map(|round| sealer.seal(&[round; 3]).unwrap())
            .collect::<Vec<_>>()
    });
    let frames = writer.join().unwrap();
    let reader = std::thread::spawn(move || {
        frames
            .iter()
            .map(|frame| opener.open(frame).unwrap())
            .collect::<Vec<_>>()
    });
    let opened = reader.join().unwrap();
    assert_eq!(opened.len(), 50);
    assert_eq!(opened[49], [49u8; 3]);
}

#[test]
fn nothing_of_the_channel_prints_a_key() {
    let private = key("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let identity = StaticKeypair::from_private(Secret::new(private));
    let hub_private = key("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    let hub = StaticKeypair::from_private(Secret::new(hub_private));

    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    let mut shown = format!("{pending_connector:?} {pending_hub:?}");
    let (third, connector) = pending_connector.finish(&second).unwrap();
    let at_hub = pending_hub.finish(&third).unwrap();
    shown.push_str(&format!("{connector:?} {at_hub:?} {connector:#?}"));
    for error in [
        NoiseError::Library,
        NoiseError::Random,
        NoiseError::HandshakeLength,
        NoiseError::Handshake,
        NoiseError::TooLarge,
        NoiseError::Decrypt,
        NoiseError::CounterExhausted,
        NoiseError::Closed,
    ] {
        shown.push_str(&format!("{error} {error:?}"));
    }
    assert_hides(&shown, &private, "Debug of the channel");
    assert_hides(&shown, &hub_private, "Debug of the channel");
    assert_not_impl!(Sealer: core::fmt::Display, serde::Serialize, Clone);
    assert_not_impl!(Opener: core::fmt::Display, serde::Serialize, Clone);
}

#[test]
fn the_key_of_a_hub_is_written_and_read_as_its_only_text() {
    let hub = StaticKeypair::generate().unwrap();
    let text = hub.public().to_text();
    assert_eq!(text.len(), HUB_KEY_TEXT_LEN);
    assert!(text.starts_with(HUB_KEY_PREFIX));
    assert!(
        text[4..]
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7'))
    );
    assert_eq!(&PublicKey::parse_text(&text).unwrap(), hub.public());

    // Golden text: the base point of the curve (9 followed by zeros).
    let mut nine = [0u8; 32];
    nine[0] = 9;
    assert_eq!(
        PublicKey::from_bytes(nine).to_text(),
        "arh1beaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn the_text_of_the_key_of_a_hub_is_strict() {
    let text = StaticKeypair::generate().unwrap().public().to_text();

    assert_eq!(
        PublicKey::parse_text(&text[..55]),
        Err(PublicKeyTextError::Length)
    );
    assert_eq!(
        PublicKey::parse_text(&format!("{text}a")),
        Err(PublicKeyTextError::Length)
    );
    assert_eq!(
        PublicKey::parse_text(&format!("arc1{}", &text[4..])),
        Err(PublicKeyTextError::Prefix)
    );
    assert_eq!(
        PublicKey::parse_text(&text.to_ascii_uppercase().replace("ARH1", "arh1")),
        Err(PublicKeyTextError::Character)
    );
    assert_eq!(
        PublicKey::parse_text(&format!("{}=", &text[..55])),
        Err(PublicKeyTextError::Character)
    );
    // 52 characters of base32 carry 260 bits: the last character has four spare bits, and
    // only the form with those bits at zero is a key.
    assert_eq!(
        PublicKey::parse_text(&format!("{}b", &text[..55])),
        Err(PublicKeyTextError::Encoding)
    );
}

#[test]
fn a_value_nobody_holds_is_not_read_as_the_key_of_a_hub() {
    let zero = PublicKey::from_bytes([0u8; 32]);
    assert!(zero.is_low_order());
    assert_eq!(
        PublicKey::parse_text(&zero.to_text()),
        Err(PublicKeyTextError::LowOrder)
    );
}
