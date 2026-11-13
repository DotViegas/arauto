//! A passkey authenticator in software, for the tests of the panel.
//!
//! [`SoftAuthenticator`] plays the browser and a platform authenticator for one origin. It reads
//! the JSON the hub gives to `PublicKeyCredential.parseCreationOptionsFromJSON()` and
//! `parseRequestOptionsFromJSON()`, and answers with the JSON a browser gives from
//! `credential.toJSON()`. Its passkeys are ES256 (P-256) keys, discoverable, kept in memory,
//! each with a signature counter. The attestation is always "none".
//!
//! The CBOR it writes is made by hand for the few fixed shapes WebAuthn needs (the attestation
//! object and the COSE key); it is test material, not a general encoder.
//!
//! Knobs for the tests that break the rules: answer as another origin or from inside a frame of
//! another origin, leave out user verification, spoil the signature, set the counter, and hand
//! back the last response to replay it.

use anyhow::{Context, Result, anyhow, bail};
use data_encoding::BASE64URL_NOPAD;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Flag of the authenticator data: the user was present.
const FLAG_UP: u8 = 0x01;
/// Flag of the authenticator data: the user was verified.
const FLAG_UV: u8 = 0x04;
/// Flag of the authenticator data: the credential may be backed up.
const FLAG_BE: u8 = 0x08;
/// Flag of the authenticator data: the credential is backed up.
const FLAG_BS: u8 = 0x10;
/// Flag of the authenticator data: attested credential data follows.
const FLAG_AT: u8 = 0x40;

/// COSE identifier of ES256.
const ES256: i64 = -7;

/// The bytes of a credential id this authenticator draws.
const CREDENTIAL_ID_LEN: usize = 16;

/// The DER prefix of a P-256 public key in SubjectPublicKeyInfo form, before the 65 bytes of
/// the uncompressed point.
const P256_SPKI_PREFIX: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// One passkey kept by the authenticator.
struct Credential {
    id: Vec<u8>,
    rp_id: String,
    user_handle: Vec<u8>,
    key: SigningKey,
    counter: u32,
}

/// A browser and a platform authenticator in software, for one origin.
///
/// `Debug` shows the origin and how many passkeys it keeps, never a key.
pub struct SoftAuthenticator {
    origin: String,
    credentials: Vec<Credential>,
    skip_user_verification: bool,
    tamper_signature: bool,
    top_origin: Option<String>,
    last_response: Option<String>,
}

impl std::fmt::Debug for SoftAuthenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftAuthenticator")
            .field("origin", &self.origin)
            .field("credentials", &self.credentials.len())
            .finish_non_exhaustive()
    }
}

impl SoftAuthenticator {
    /// An authenticator with no passkey, answering for `origin` (for example
    /// `https://panel.example.com`).
    #[must_use]
    pub fn new(origin: &str) -> Self {
        Self {
            origin: origin.to_owned(),
            credentials: Vec::new(),
            skip_user_verification: false,
            tamper_signature: false,
            top_origin: None,
            last_response: None,
        }
    }

    /// From now on, answer as if the page were at `origin`.
    pub fn answer_as(&mut self, origin: &str) {
        origin.clone_into(&mut self.origin);
    }

    /// From now on, leave the "user verified" flag out (`true`) or put it back (`false`).
    pub fn skip_user_verification(&mut self, skip: bool) {
        self.skip_user_verification = skip;
    }

    /// From now on, sign something other than what is sent (`true`): the signature stays well
    /// formed but does not match.
    pub fn tamper_signature(&mut self, tamper: bool) {
        self.tamper_signature = tamper;
    }

    /// From now on, answer as if the page were inside a frame of the page at `top_origin`
    /// (`Some`), or at the top again (`None`).
    pub fn inside_frame_of(&mut self, top_origin: Option<&str>) {
        self.top_origin = top_origin.map(str::to_owned);
    }

    /// Sets the signature counter of every passkey, as a cloned authenticator would show it.
    pub fn set_counter(&mut self, value: u32) {
        for credential in &mut self.credentials {
            credential.counter = value;
        }
    }

    /// The signature counter of the newest passkey, if there is one.
    #[must_use]
    pub fn counter(&self) -> Option<u32> {
        self.credentials.last().map(|credential| credential.counter)
    }

    /// The ids of the passkeys kept, oldest first.
    #[must_use]
    pub fn credential_ids(&self) -> Vec<Vec<u8>> {
        self.credentials.iter().map(|c| c.id.clone()).collect()
    }

    /// The last response given, to send it again.
    #[must_use]
    pub fn replay(&self) -> Option<String> {
        self.last_response.clone()
    }

    /// Answers `navigator.credentials.create()`: takes the JSON of the creation options and
    /// returns the JSON of the new credential.
    ///
    /// # Errors
    /// When the options cannot be read, do not offer ES256, or exclude a passkey this
    /// authenticator already holds for the same relying party.
    pub fn create(&mut self, options_json: &str) -> Result<String> {
        let options: Value = serde_json::from_str(options_json).context("creation options")?;
        let challenge = text(&options, &["challenge"])?;
        let rp_id = text(&options, &["rp", "id"])?.to_owned();
        let user_handle = BASE64URL_NOPAD
            .decode(text(&options, &["user", "id"])?.as_bytes())
            .context("user id")?;
        let offers_es256 = options
            .get("pubKeyCredParams")
            .and_then(Value::as_array)
            .is_some_and(|params| {
                params
                    .iter()
                    .any(|p| p.get("alg").and_then(Value::as_i64) == Some(ES256))
            });
        if !offers_es256 {
            bail!("the options do not offer ES256");
        }
        let excluded = options
            .get("excludeCredentials")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for descriptor in excluded {
            let id = descriptor
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("excluded credential without id"))?;
            let id = BASE64URL_NOPAD
                .decode(id.as_bytes())
                .context("excluded id")?;
            if self
                .credentials
                .iter()
                .any(|c| c.id == id && c.rp_id == rp_id)
            {
                bail!("a passkey of this authenticator is excluded");
            }
        }

        let key = random_key()?;
        let mut id = vec![0_u8; CREDENTIAL_ID_LEN];
        getrandom::fill(&mut id).map_err(|e| anyhow!("generator of the system: {e}"))?;
        let point = key.verifying_key().to_encoded_point(false);
        let (Some(x), Some(y)) = (point.x(), point.y()) else {
            bail!("the public key has no coordinates");
        };

        let client_data = self.client_data("webauthn.create", challenge)?;
        let counter = 0_u32;
        let mut auth_data = self.auth_data_head(&rp_id, FLAG_AT, counter);
        auth_data.extend_from_slice(&[0_u8; 16]); // AAGUID: none.
        let id_len = u16::try_from(id.len()).context("credential id length")?;
        auth_data.extend_from_slice(&id_len.to_be_bytes());
        auth_data.extend_from_slice(&id);
        auth_data.extend_from_slice(&cose_p256(x, y));

        let attestation_object = attestation_none(&auth_data)?;
        let mut spki = P256_SPKI_PREFIX.to_vec();
        spki.extend_from_slice(point.as_bytes());

        let id_text = BASE64URL_NOPAD.encode(&id);
        let response = serde_json::json!({
            "id": id_text,
            "rawId": id_text,
            "type": "public-key",
            "authenticatorAttachment": "platform",
            "clientExtensionResults": {},
            "response": {
                "clientDataJSON": BASE64URL_NOPAD.encode(client_data.as_bytes()),
                "attestationObject": BASE64URL_NOPAD.encode(&attestation_object),
                "authenticatorData": BASE64URL_NOPAD.encode(&auth_data),
                "transports": ["hybrid", "internal"],
                "publicKeyAlgorithm": ES256,
                "publicKey": BASE64URL_NOPAD.encode(&spki),
            },
        })
        .to_string();

        self.credentials.push(Credential {
            id,
            rp_id,
            user_handle,
            key,
            counter,
        });
        self.last_response = Some(response.clone());
        Ok(response)
    }

    /// Answers `navigator.credentials.get()` with the newest passkey for the relying party of
    /// the options: takes the JSON of the request options and returns the JSON of the
    /// assertion. The signature counter goes up by one first.
    ///
    /// # Errors
    /// When the options cannot be read or no passkey is kept for that relying party.
    pub fn get(&mut self, options_json: &str) -> Result<String> {
        let options: Value = serde_json::from_str(options_json).context("request options")?;
        let challenge = text(&options, &["challenge"])?;
        let rp_id = text(&options, &["rpId"])?.to_owned();
        let client_data = self.client_data("webauthn.get", challenge)?;
        let flags = self.flags(0);
        let tamper = self.tamper_signature;

        let credential = self
            .credentials
            .iter_mut()
            .rev()
            .find(|c| c.rp_id == rp_id)
            .ok_or_else(|| anyhow!("no passkey for this relying party"))?;
        credential.counter = credential.counter.saturating_add(1);

        let mut auth_data = Sha256::digest(rp_id.as_bytes()).to_vec();
        auth_data.push(flags);
        auth_data.extend_from_slice(&credential.counter.to_be_bytes());

        let mut signed = auth_data.clone();
        signed.extend_from_slice(&Sha256::digest(client_data.as_bytes()));
        if tamper {
            // Still a valid DER signature, over other bytes.
            signed.push(0);
        }
        let signature: Signature = credential.key.sign(&signed);
        let signature = signature.to_der();

        let id_text = BASE64URL_NOPAD.encode(&credential.id);
        let response = serde_json::json!({
            "id": id_text,
            "rawId": id_text,
            "type": "public-key",
            "authenticatorAttachment": "platform",
            "clientExtensionResults": {},
            "response": {
                "clientDataJSON": BASE64URL_NOPAD.encode(client_data.as_bytes()),
                "authenticatorData": BASE64URL_NOPAD.encode(&auth_data),
                "signature": BASE64URL_NOPAD.encode(signature.as_bytes()),
                "userHandle": BASE64URL_NOPAD.encode(&credential.user_handle),
            },
        })
        .to_string();
        self.last_response = Some(response.clone());
        Ok(response)
    }

    /// The client data, in the order a browser writes it.
    fn client_data(&self, kind: &str, challenge: &str) -> Result<String> {
        let mut data = format!(
            "{{\"type\":{},\"challenge\":{},\"origin\":{},\"crossOrigin\":{}",
            serde_json::to_string(kind)?,
            serde_json::to_string(challenge)?,
            serde_json::to_string(&self.origin)?,
            self.top_origin.is_some(),
        );
        if let Some(top) = &self.top_origin {
            data.push_str(&format!(",\"topOrigin\":{}", serde_json::to_string(top)?));
        }
        data.push('}');
        Ok(data)
    }

    /// The flags of a response: present, verified unless told otherwise, and backed up like a
    /// synced passkey, plus `extra`.
    fn flags(&self, extra: u8) -> u8 {
        let mut flags = FLAG_UP | FLAG_BE | FLAG_BS | extra;
        if !self.skip_user_verification {
            flags |= FLAG_UV;
        }
        flags
    }

    /// The hash of the relying party, the flags and the counter.
    fn auth_data_head(&self, rp_id: &str, extra_flags: u8, counter: u32) -> Vec<u8> {
        let mut data = Sha256::digest(rp_id.as_bytes()).to_vec();
        data.push(self.flags(extra_flags));
        data.extend_from_slice(&counter.to_be_bytes());
        data
    }
}

/// The string at `path` in `value`.
fn text<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str> {
    let mut current = value;
    for key in path {
        current = current
            .get(key)
            .ok_or_else(|| anyhow!("missing field {}", path.join(".")))?;
    }
    current
        .as_str()
        .ok_or_else(|| anyhow!("field {} is not text", path.join(".")))
}

/// A P-256 key drawn from the generator of the system.
fn random_key() -> Result<SigningKey> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| anyhow!("generator of the system: {e}"))?;
        if let Ok(key) = SigningKey::from_slice(&bytes) {
            return Ok(key);
        }
    }
    bail!("no valid P-256 key in eight draws")
}

/// The COSE key of an ES256 public key, in canonical CBOR:
/// `{1: 2, 3: -7, -1: 1, -2: x, -3: y}`.
fn cose_p256(x: &[u8], y: &[u8]) -> Vec<u8> {
    let mut cose = vec![
        0xa5, // map of 5
        0x01, 0x02, // kty: EC2
        0x03, 0x26, // alg: ES256
        0x20, 0x01, // crv: P-256
        0x21, 0x58, 0x20, // x: bytes(32)
    ];
    cose.extend_from_slice(x);
    cose.extend_from_slice(&[0x22, 0x58, 0x20]); // y: bytes(32)
    cose.extend_from_slice(y);
    cose
}

/// The attestation object with format "none": `{"fmt": "none", "attStmt": {}, "authData": ..}`.
fn attestation_none(auth_data: &[u8]) -> Result<Vec<u8>> {
    let mut object = vec![0xa3]; // map of 3
    object.extend_from_slice(&[0x63, b'f', b'm', b't']);
    object.extend_from_slice(&[0x64, b'n', b'o', b'n', b'e']);
    object.extend_from_slice(&[0x67, b'a', b't', b't', b'S', b't', b'm', b't']);
    object.push(0xa0); // empty map
    object.extend_from_slice(&[0x68, b'a', b'u', b't', b'h', b'D', b'a', b't', b'a']);
    match u8::try_from(auth_data.len()) {
        Ok(len) if len > 23 => object.extend_from_slice(&[0x58, len]),
        _ => {
            let len = u16::try_from(auth_data.len()).context("authenticator data length")?;
            object.push(0x59);
            object.extend_from_slice(&len.to_be_bytes());
        }
    }
    object.extend_from_slice(auth_data);
    Ok(object)
}
