//! The vault: the black box of the hub.
//!
//! This is the **only** module that touches the master key. Everything else hands it bytes
//! and gets bytes back.
//!
//! ```text
//! master key           a file of 32 random bytes, outside the database
//!   ├─ hub key         the X25519 private key of the hub, encrypted in `hub_key`
//!   └─ organization    32 random bytes per organization, encrypted in `organization`
//!        ├─ HKDF "arauto/seal/1"   seals the credentials (docs/14, section 9)
//!        └─ HKDF "arauto/peer/1"   encrypts the text of the messages (docs/13, section 7)
//! ```
//!
//! Known pieces only: XChaCha20-Poly1305 with a 24-byte nonce from the generator of the
//! system, and HKDF-SHA-256 to give each use of the organization key its own key. Every
//! encryption binds, as authenticated data, a label of its own and the identifiers of what it
//! protects, so a block moved to another row, organization or device does not open.
//!
//! The master key is never logged, printed or put in an error. Its file is created once, by
//! [`Vault::create`], and never written again.
//!
//! The functions that read or write the key file block the calling thread.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use arauto_platform::{FsError, LockError, create_private_dir, write_durable};
use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::limits::{KEY_LEN, PEER_TEXT_MAX_BYTES};
use arauto_protocol::message::Generation;
use arauto_protocol::names::MessageText;
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use arauto_protocol::secret::{SealedCredential, Secret};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use data_encoding::BASE64;
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::lock_wait::acquire_within;

/// The file of the master key, inside the data folder.
pub const MASTER_KEY_FILE: &str = "master.key";

/// The lock taken while the master key is created, so that two commands never create two.
const MASTER_KEY_LOCK_FILE: &str = "master.key.lock";

/// How long the creation of the master key waits for another command that is creating it.
const MASTER_KEY_LOCK_PATIENCE: Duration = Duration::from_secs(5);

/// The version byte of every sealed block.
const BLOCK_VERSION: u8 = 1;

/// The nonce of XChaCha20-Poly1305.
pub const NONCE_LEN: usize = 24;

/// The authentication tag of Poly1305.
const TAG_LEN: usize = 16;

/// The random identifier inside a seal.
pub const SEAL_ID_LEN: usize = 16;

/// The label of the seal: the HKDF `info` of its key and the start of its authenticated data.
const SEAL_LABEL: &[u8] = b"arauto/seal/1";

/// The label of the message texts, used the same two ways.
const PEER_LABEL: &[u8] = b"arauto/peer/1";

/// The label of the hub key encrypted with the master key.
const HUB_KEY_LABEL: &[u8] = b"arauto/hub-key/1";

/// The label of an organization key encrypted with the master key.
const ORGANIZATION_KEY_LABEL: &[u8] = b"arauto/org-key/1";

/// What went wrong in the vault. No variant carries key material.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// The master key is already there: it is never created over an existing file.
    #[error("{}: the master key already exists and is never created again", .path.display())]
    MasterKeyExists {
        /// The key file.
        path: PathBuf,
    },

    /// There is no master key where it should be.
    #[error(
        "{}: no master key; run `arauto-hub init` on a new installation, or put back the key \
         file that belongs to this database",
        .path.display()
    )]
    MasterKeyMissing {
        /// The key file.
        path: PathBuf,
    },

    /// The key file does not hold exactly 32 bytes.
    #[error("{}: the master key file is damaged (it must hold exactly 32 bytes)", .path.display())]
    MasterKeyDamaged {
        /// The key file.
        path: PathBuf,
    },

    /// Another command is creating the master key right now.
    #[error("the master key is being created by another command: {0}")]
    Busy(LockError),

    /// The folder or the file could not be prepared or written.
    #[error(transparent)]
    Fs(#[from] FsError),

    /// The key file could not be read.
    #[error("could not read {}: {cause}", .path.display())]
    Read {
        /// The key file.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },

    /// The random generator of the operating system failed.
    #[error("the random generator of the operating system failed")]
    Random,

    /// A key stored in the database does not open with this master key: the key file is not
    /// the one of this database, or the row was changed.
    #[error("the stored {what} does not open with this master key")]
    StoredKeyUnreadable {
        /// Which key: "hub key" or "organization key".
        what: &'static str,
    },

    /// The cipher refused the input. It does not happen with the sizes this module uses.
    #[error("the cipher refused the input")]
    Cipher,
}

/// A sealed credential that does not open: of another device or organization, changed, of
/// another version, or too large. The reasons are not told apart on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the sealed credential does not open")]
pub struct Unreadable;

/// The text of a message does not open: it was changed, or moved from another message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the stored message text does not open")]
pub struct PeerTextUnreadable;

/// The master key of an installation.
pub struct Vault {
    master: Secret<KEY_LEN>,
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Vault([redacted])")
    }
}

impl Vault {
    /// Creates the master key of a new installation: 32 bytes from the generator of the
    /// system, written durably to `master.key` in `data_dir`, a folder only its owner uses.
    /// The folder is created when missing; its parent must exist.
    ///
    /// An existing key file is never replaced, whatever it holds.
    ///
    /// # Errors
    /// [`VaultError::MasterKeyExists`] when the file is already there; [`VaultError::Busy`]
    /// when another command is creating it; [`VaultError::Fs`] when the folder is not private
    /// to this user or the file could not be made durable (nothing usable is left behind).
    pub fn create(data_dir: &Path) -> Result<Self, VaultError> {
        create_private_dir(data_dir)?;
        let _creating = acquire_within(
            &data_dir.join(MASTER_KEY_LOCK_FILE),
            MASTER_KEY_LOCK_PATIENCE,
        )
        .map_err(VaultError::Busy)?;
        let path = data_dir.join(MASTER_KEY_FILE);
        if exists(&path)? {
            return Err(VaultError::MasterKeyExists { path });
        }
        let master = Secret::<KEY_LEN>::random().map_err(|_| VaultError::Random)?;
        match write_durable(&path, master.expose()) {
            Ok(()) => Ok(Self { master }),
            Err(error) => {
                // A key that may not survive a power loss must not be the key of anything:
                // nothing was encrypted with it yet, so it goes away and `init` starts again.
                if matches!(error, FsError::NotDurable { .. }) {
                    let _ = fs::remove_file(&path);
                }
                Err(VaultError::Fs(error))
            }
        }
    }

    /// Reads the master key of an existing installation. Never creates anything.
    ///
    /// # Errors
    /// [`VaultError::MasterKeyMissing`] when the folder or the file is not there;
    /// [`VaultError::MasterKeyDamaged`] when the file does not hold exactly 32 bytes;
    /// [`VaultError::Fs`] when the folder is not private to this user.
    pub fn open(data_dir: &Path) -> Result<Self, VaultError> {
        let path = data_dir.join(MASTER_KEY_FILE);
        if !data_dir.is_dir() {
            return Err(VaultError::MasterKeyMissing { path });
        }
        // Refuses, on Unix, a folder of another user, and closes one that is open to others.
        create_private_dir(data_dir)?;
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                return Err(VaultError::MasterKeyMissing { path });
            }
            Err(cause) => return Err(VaultError::Read { path, cause }),
        };
        // One byte more than a key, to notice a file that is too long.
        let mut buffer = Zeroizing::new([0u8; KEY_LEN + 1]);
        let mut filled = 0usize;
        while let Some(free) = buffer.get_mut(filled..).filter(|free| !free.is_empty()) {
            match file.read(free) {
                Ok(0) => break,
                Ok(count) => filled = filled.saturating_add(count),
                Err(cause) if cause.kind() == io::ErrorKind::Interrupted => {}
                Err(cause) => return Err(VaultError::Read { path, cause }),
            }
        }
        let master = (filled == KEY_LEN)
            .then(|| buffer.get(..KEY_LEN).and_then(Secret::from_slice))
            .flatten();
        match master {
            Some(master) => Ok(Self { master }),
            None => Err(VaultError::MasterKeyDamaged { path }),
        }
    }

    /// Draws the key pair of the hub and returns the public part and the private part
    /// encrypted for the row `key_id` of `hub_key`.
    ///
    /// # Errors
    /// [`VaultError::Random`] when the generator of the system fails.
    pub fn generate_hub_key(&self, key_id: Uuid) -> Result<NewHubKey, VaultError> {
        let pair = StaticKeypair::generate().map_err(|_| VaultError::Random)?;
        let public = *pair.public();
        let private_key_enc = seal_block(
            &self.master,
            &hub_key_aad(key_id, &public),
            pair.private().expose(),
        )?;
        Ok(NewHubKey {
            public,
            private_key_enc,
        })
    }

    /// Opens the private key of the hub stored in the row `key_id`, whose public part is
    /// `public`.
    ///
    /// # Errors
    /// [`VaultError::StoredKeyUnreadable`] when the block does not open with this master key
    /// for that row, or the private key inside is not the one of `public`.
    pub fn open_hub_key(
        &self,
        key_id: Uuid,
        public: &PublicKey,
        private_key_enc: &[u8],
    ) -> Result<StaticKeypair, VaultError> {
        let unreadable = VaultError::StoredKeyUnreadable { what: "hub key" };
        let Some(private) = open_block(&self.master, &hub_key_aad(key_id, public), private_key_enc)
        else {
            return Err(unreadable);
        };
        let Some(private) = Secret::<KEY_LEN>::from_slice(&private) else {
            return Err(unreadable);
        };
        let pair = StaticKeypair::from_private(private);
        if pair.public() != public {
            return Err(unreadable);
        }
        Ok(pair)
    }

    /// Draws the key of the organization `organization_id` and returns it encrypted, as it
    /// goes into `organization.key_enc`.
    ///
    /// # Errors
    /// [`VaultError::Random`] when the generator of the system fails.
    pub fn generate_organization_key(&self, organization_id: Uuid) -> Result<Vec<u8>, VaultError> {
        let key = Secret::<KEY_LEN>::random().map_err(|_| VaultError::Random)?;
        seal_block(
            &self.master,
            &organization_key_aad(organization_id),
            key.expose(),
        )
    }

    /// Opens the key of the organization `organization_id` and derives the key of each use.
    ///
    /// # Errors
    /// [`VaultError::StoredKeyUnreadable`] when the block does not open with this master key
    /// for that organization.
    pub fn open_organization_key(
        &self,
        organization_id: Uuid,
        key_enc: &[u8],
    ) -> Result<OrganizationKey, VaultError> {
        let unreadable = VaultError::StoredKeyUnreadable {
            what: "organization key",
        };
        let Some(key) = open_block(
            &self.master,
            &organization_key_aad(organization_id),
            key_enc,
        ) else {
            return Err(unreadable);
        };
        let Some(key) = Secret::<KEY_LEN>::from_slice(&key) else {
            return Err(unreadable);
        };
        Ok(OrganizationKey {
            organization_id,
            seal: derive(&key, SEAL_LABEL)?,
            peer: derive(&key, PEER_LABEL)?,
        })
    }
}

/// A new key pair of the hub, as it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewHubKey {
    /// The public part, which goes in every block as `arh1...` ([`PublicKey::to_text`]).
    pub public: PublicKey,
    /// The private part, encrypted with the master key for that row.
    pub private_key_enc: Vec<u8>,
}

/// The keys of one organization, one per use, ready to seal and to open.
pub struct OrganizationKey {
    organization_id: Uuid,
    seal: Secret<KEY_LEN>,
    peer: Secret<KEY_LEN>,
}

impl fmt::Debug for OrganizationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OrganizationKey({}, [redacted])",
            self.organization_id.as_hyphenated()
        )
    }
}

/// A credential the vault has just sealed.
#[derive(Debug)]
pub struct IssuedSeal {
    /// The block that goes to the connector, in `credential`.
    pub sealed: SealedCredential,
    /// The generation written inside.
    pub generation: Generation,
    /// SHA-256 of the whole block: what the hub keeps.
    pub digest: CredentialDigest,
}

/// What a seal says, once it opened for the device that presented it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenedSeal {
    /// The generation of the credential.
    pub generation: Generation,
    /// The random identifier of this seal.
    pub id: [u8; SEAL_ID_LEN],
    /// When the hub issued it, in milliseconds. No rule of the MVP reads it.
    pub issued_at: i64,
    /// SHA-256 of the whole block that was presented.
    pub digest: CredentialDigest,
}

/// The content of a seal, in the order `docs/14-protocolo-do-canal.md` section 9 gives.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SealContent {
    v: u8,
    org: Uuid,
    device: String,
    generation: Generation,
    id: String,
    issued_at: i64,
}

/// The identifiers a message text is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerMessageIds {
    /// The thread.
    pub thread: Uuid,
    /// The message.
    pub message: Uuid,
    /// The Hermes that sent it.
    pub from: Uuid,
    /// The Hermes it was sent to.
    pub to: Uuid,
}

/// An encrypted message text, as the two columns of `peer_message` hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerCiphertext {
    /// The nonce.
    pub nonce: [u8; NONCE_LEN],
    /// The encrypted text, with the authentication tag.
    pub text_enc: Vec<u8>,
}

impl OrganizationKey {
    /// The organization these keys belong to.
    #[must_use]
    pub const fn organization_id(&self) -> Uuid {
        self.organization_id
    }

    /// Seals a new credential for `device`, at `generation`, issued at `issued_at`.
    ///
    /// The seal is exactly the one of `docs/14-protocolo-do-canal.md`, section 9:
    /// `version (1) || nonce (24) || XChaCha20-Poly1305(content)`, with the authenticated data
    /// `"arauto/seal/1" || version || organization (16) || device (32)`.
    ///
    /// # Errors
    /// [`VaultError::Random`] when the generator of the system fails.
    pub fn seal_credential(
        &self,
        device: &PublicKey,
        generation: Generation,
        issued_at: i64,
    ) -> Result<IssuedSeal, VaultError> {
        let mut id = [0u8; SEAL_ID_LEN];
        fill_random(&mut id)?;
        let content = SealContent {
            v: BLOCK_VERSION,
            org: self.organization_id,
            device: BASE64.encode(device.as_bytes()),
            generation,
            id: BASE64.encode(&id),
            issued_at,
        };
        let content = serde_json::to_vec(&content).map_err(|_| VaultError::Cipher)?;
        let block = seal_block(&self.seal, &self.seal_aad(device), &content)?;
        let sealed = SealedCredential::new(block).map_err(|_| VaultError::Cipher)?;
        let digest = CredentialDigest::of(&sealed);
        Ok(IssuedSeal {
            sealed,
            generation,
            digest,
        })
    }

    /// Opens what `presenter` presented.
    ///
    /// # Errors
    /// [`Unreadable`] when the block is too large or too small, is of another version, was
    /// changed anywhere, was sealed for another device or by another organization, or says
    /// inside something else than the data it is bound to.
    pub fn open_credential(
        &self,
        sealed: &[u8],
        presenter: &PublicKey,
    ) -> Result<OpenedSeal, Unreadable> {
        // The size comes first, by the type of a sealed credential: nothing is computed over
        // a block that is empty or too large.
        let block = SealedCredential::from_slice(sealed).map_err(|_| Unreadable)?;
        let content =
            open_block(&self.seal, &self.seal_aad(presenter), block.expose()).ok_or(Unreadable)?;
        let content: SealContent = serde_json::from_slice(&content).map_err(|_| Unreadable)?;
        let device = decode_exact::<KEY_LEN>(&content.device).ok_or(Unreadable)?;
        let id = decode_exact::<SEAL_ID_LEN>(&content.id).ok_or(Unreadable)?;
        if content.v != BLOCK_VERSION
            || content.org != self.organization_id
            || &device != presenter.as_bytes()
        {
            return Err(Unreadable);
        }
        Ok(OpenedSeal {
            generation: content.generation,
            id,
            issued_at: content.issued_at,
            digest: CredentialDigest::of(&block),
        })
    }

    /// Encrypts the text of a message, bound to the organization and to `ids`.
    ///
    /// # Errors
    /// [`VaultError::Random`] when the generator of the system fails.
    pub fn seal_peer_text(
        &self,
        ids: &PeerMessageIds,
        text: &MessageText,
    ) -> Result<PeerCiphertext, VaultError> {
        let mut nonce = [0u8; NONCE_LEN];
        fill_random(&mut nonce)?;
        let text_enc = encrypt(
            &self.peer,
            &nonce,
            &self.peer_aad(ids),
            text.as_str().as_bytes(),
        )?;
        Ok(PeerCiphertext { nonce, text_enc })
    }

    /// Opens the text of the message `ids`.
    ///
    /// # Errors
    /// [`PeerTextUnreadable`] when the nonce or the text was changed, the text was stored for
    /// another message, thread, sender, recipient or organization, or what opens is not a
    /// valid message text.
    pub fn open_peer_text(
        &self,
        ids: &PeerMessageIds,
        nonce: &[u8],
        text_enc: &[u8],
    ) -> Result<MessageText, PeerTextUnreadable> {
        if text_enc.len() > PEER_TEXT_MAX_BYTES.saturating_add(TAG_LEN) {
            return Err(PeerTextUnreadable);
        }
        let nonce = <[u8; NONCE_LEN]>::try_from(nonce).map_err(|_| PeerTextUnreadable)?;
        let text =
            decrypt(&self.peer, &nonce, &self.peer_aad(ids), text_enc).ok_or(PeerTextUnreadable)?;
        let text = String::from_utf8(text.to_vec()).map_err(|_| PeerTextUnreadable)?;
        MessageText::new(text).map_err(|_| PeerTextUnreadable)
    }

    fn seal_aad(&self, device: &PublicKey) -> Vec<u8> {
        let mut aad = Vec::with_capacity(SEAL_LABEL.len() + 1 + 16 + KEY_LEN);
        aad.extend_from_slice(SEAL_LABEL);
        aad.push(BLOCK_VERSION);
        aad.extend_from_slice(self.organization_id.as_bytes());
        aad.extend_from_slice(device.as_bytes());
        aad
    }

    fn peer_aad(&self, ids: &PeerMessageIds) -> Vec<u8> {
        let mut aad = Vec::with_capacity(PEER_LABEL.len() + 5 * 16);
        aad.extend_from_slice(PEER_LABEL);
        aad.extend_from_slice(self.organization_id.as_bytes());
        aad.extend_from_slice(ids.thread.as_bytes());
        aad.extend_from_slice(ids.message.as_bytes());
        aad.extend_from_slice(ids.from.as_bytes());
        aad.extend_from_slice(ids.to.as_bytes());
        aad
    }
}

fn hub_key_aad(key_id: Uuid, public: &PublicKey) -> Vec<u8> {
    let mut aad = Vec::with_capacity(HUB_KEY_LABEL.len() + 1 + 16 + KEY_LEN);
    aad.extend_from_slice(HUB_KEY_LABEL);
    aad.push(BLOCK_VERSION);
    aad.extend_from_slice(key_id.as_bytes());
    aad.extend_from_slice(public.as_bytes());
    aad
}

fn organization_key_aad(organization_id: Uuid) -> Vec<u8> {
    let mut aad = Vec::with_capacity(ORGANIZATION_KEY_LABEL.len() + 1 + 16);
    aad.extend_from_slice(ORGANIZATION_KEY_LABEL);
    aad.push(BLOCK_VERSION);
    aad.extend_from_slice(organization_id.as_bytes());
    aad
}

/// Whether anything, of any kind, is under `path`. A link is not followed.
fn exists(path: &Path) -> Result<bool, VaultError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(VaultError::Read {
            path: path.to_path_buf(),
            cause,
        }),
    }
}

fn fill_random(dest: &mut [u8]) -> Result<(), VaultError> {
    getrandom::fill(dest).map_err(|_| VaultError::Random)
}

/// Base64 text of exactly `N` bytes.
fn decode_exact<const N: usize>(text: &str) -> Option<[u8; N]> {
    // The text of N bytes is never longer than this; a longer one is not even decoded.
    if text.len() > N.saturating_mul(2) {
        return None;
    }
    let bytes = BASE64.decode(text.as_bytes()).ok()?;
    <[u8; N]>::try_from(bytes.as_slice()).ok()
}

/// The key of one use of the organization key: HKDF-SHA-256 without salt, with `label` as
/// `info`.
fn derive(key: &Secret<KEY_LEN>, label: &[u8]) -> Result<Secret<KEY_LEN>, VaultError> {
    let mut derived = Zeroizing::new([0u8; KEY_LEN]);
    Hkdf::<Sha256>::new(None, key.expose())
        .expand(label, derived.as_mut_slice())
        .map_err(|_| VaultError::Cipher)?;
    Ok(Secret::new(*derived))
}

fn encrypt(
    key: &Secret<KEY_LEN>,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose()).map_err(|_| VaultError::Cipher)?;
    cipher
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| VaultError::Cipher)
}

fn decrypt(
    key: &Secret<KEY_LEN>,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> Option<Zeroizing<Vec<u8>>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose()).ok()?;
    cipher
        .decrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()
        .map(Zeroizing::new)
}

/// `version (1) || nonce (24) || XChaCha20-Poly1305(plaintext)`, with a fresh nonce.
fn seal_block(key: &Secret<KEY_LEN>, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;
    let ciphertext = encrypt(key, &nonce, aad, plaintext)?;
    let mut block = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    block.push(BLOCK_VERSION);
    block.extend_from_slice(&nonce);
    block.extend_from_slice(&ciphertext);
    Ok(block)
}

/// Opens a block made by [`seal_block`]. `None` for any block that does not open.
fn open_block(key: &Secret<KEY_LEN>, aad: &[u8], block: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let (version, rest) = block.split_first()?;
    if *version != BLOCK_VERSION {
        return None;
    }
    let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN)?;
    if ciphertext.len() < TAG_LEN {
        return None;
    }
    let nonce = <[u8; NONCE_LEN]>::try_from(nonce).ok()?;
    decrypt(key, &nonce, aad, ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arauto_protocol::limits::CREDENTIAL_MAX_LEN;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::create(&dir.path().join("data")).unwrap();
        (dir, vault)
    }

    fn organization(vault: &Vault) -> (Uuid, Vec<u8>, OrganizationKey) {
        let id = Uuid::new_v4();
        let key_enc = vault.generate_organization_key(id).unwrap();
        let key = vault.open_organization_key(id, &key_enc).unwrap();
        (id, key_enc, key)
    }

    fn device() -> PublicKey {
        *StaticKeypair::generate().unwrap().public()
    }

    fn generation(value: u64) -> Generation {
        Generation::new(value).unwrap()
    }

    /// Every way the bytes of a secret could show up in a text.
    fn shows(output: &str, secret: &[u8]) -> bool {
        use data_encoding::{
            BASE32, BASE32_NOPAD, BASE64_NOPAD, BASE64URL, BASE64URL_NOPAD, HEXLOWER, HEXUPPER,
        };
        [
            HEXLOWER.encode(secret),
            HEXUPPER.encode(secret),
            BASE64.encode(secret),
            BASE64_NOPAD.encode(secret),
            BASE64URL.encode(secret),
            BASE64URL_NOPAD.encode(secret),
            BASE32.encode(secret),
            BASE32_NOPAD.encode(secret),
            BASE32_NOPAD.encode(secret).to_ascii_lowercase(),
            format!("{secret:?}"),
        ]
        .iter()
        .any(|form| output.contains(form.as_str()))
    }

    // ------------------------------------------------------------------ the master key

    #[test]
    fn the_master_key_is_32_bytes_in_its_own_file() {
        let (dir, _vault) = vault();
        let stored = fs::read(dir.path().join("data").join(MASTER_KEY_FILE)).unwrap();
        assert_eq!(stored.len(), KEY_LEN);
        assert_ne!(stored, vec![0u8; KEY_LEN]);
    }

    #[test]
    fn the_master_key_is_created_once_and_never_over_an_existing_file() {
        let (dir, _vault) = vault();
        let data = dir.path().join("data");
        let before = fs::read(data.join(MASTER_KEY_FILE)).unwrap();
        for _ in 0..3 {
            assert!(matches!(
                Vault::create(&data),
                Err(VaultError::MasterKeyExists { .. })
            ));
        }
        assert_eq!(fs::read(data.join(MASTER_KEY_FILE)).unwrap(), before);
    }

    #[test]
    fn a_damaged_or_foreign_file_is_not_replaced_either() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MASTER_KEY_FILE);
        for content in [&b""[..], &b"short"[..], &[7u8; 64][..]] {
            fs::write(&path, content).unwrap();
            assert!(matches!(
                Vault::create(dir.path()),
                Err(VaultError::MasterKeyExists { .. })
            ));
            assert_eq!(fs::read(&path).unwrap(), content);
        }
    }

    #[test]
    fn two_installations_have_different_master_keys() {
        let (first_dir, _first) = vault();
        let (second_dir, _second) = vault();
        let read = |dir: &tempfile::TempDir| {
            fs::read(dir.path().join("data").join(MASTER_KEY_FILE)).unwrap()
        };
        assert_ne!(read(&first_dir), read(&second_dir));
    }

    #[test]
    fn opening_reads_the_same_key_and_creates_nothing() {
        let (dir, vault) = vault();
        let data = dir.path().join("data");
        let (id, key_enc, _) = organization(&vault);
        let reopened = Vault::open(&data).unwrap();
        assert!(reopened.open_organization_key(id, &key_enc).is_ok());

        let nowhere = dir.path().join("nowhere");
        assert!(matches!(
            Vault::open(&nowhere),
            Err(VaultError::MasterKeyMissing { .. })
        ));
        assert!(!nowhere.exists(), "opening must not create the folder");

        let empty = dir.path().join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(matches!(
            Vault::open(&empty),
            Err(VaultError::MasterKeyMissing { .. })
        ));
        assert!(!empty.join(MASTER_KEY_FILE).exists());
    }

    #[test]
    fn a_key_file_of_the_wrong_size_is_refused() {
        for size in [0usize, 1, 31, 33, 64, 4096] {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(MASTER_KEY_FILE), vec![0x42u8; size]).unwrap();
            assert!(
                matches!(
                    Vault::open(dir.path()),
                    Err(VaultError::MasterKeyDamaged { .. })
                ),
                "{size} bytes"
            );
        }
    }

    #[test]
    fn the_creation_waits_for_its_lock() {
        let dir = tempfile::tempdir().unwrap();
        let held =
            arauto_platform::FileLock::try_acquire(&dir.path().join(MASTER_KEY_LOCK_FILE)).unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            drop(held);
        });
        assert!(Vault::create(dir.path()).is_ok());
        releaser.join().unwrap();
    }

    #[test]
    fn the_master_key_shows_in_no_debug_and_in_no_error() {
        let (dir, vault) = vault();
        let data = dir.path().join("data");
        let master = fs::read(data.join(MASTER_KEY_FILE)).unwrap();
        let (org_id, key_enc, org_key) = organization(&vault);
        let hub_id = Uuid::new_v4();
        let hub = vault.generate_hub_key(hub_id).unwrap();

        let mut outputs = vec![
            format!("{vault:?}"),
            format!("{vault:#?}"),
            format!("{org_key:?}"),
            format!("{hub:?}"),
        ];
        let errors = [
            Vault::create(&data).unwrap_err(),
            vault
                .open_organization_key(Uuid::new_v4(), &key_enc)
                .unwrap_err(),
            vault.open_organization_key(org_id, &master).unwrap_err(),
            vault
                .open_hub_key(hub_id, &device(), &hub.private_key_enc)
                .unwrap_err(),
            vault
                .open_hub_key(hub_id, &hub.public, &master)
                .unwrap_err(),
            Vault::open(&dir.path().join("nowhere")).unwrap_err(),
        ];
        for error in &errors {
            outputs.push(format!("{error}"));
            outputs.push(format!("{error:?}"));
        }
        outputs.push(format!("{:?}", org_key.open_credential(&master, &device())));
        for output in &outputs {
            assert!(!shows(output, &master), "the master key shows in: {output}");
            assert!(!shows(output, org_key.seal.expose()), "{output}");
            assert!(!shows(output, org_key.peer.expose()), "{output}");
        }
        assert_eq!(format!("{vault:?}"), "Vault([redacted])");
    }

    // ------------------------------------------------------------------ keys in the database

    #[test]
    fn the_hub_key_comes_back_only_for_its_own_row() {
        let (_dir, vault) = vault();
        let id = Uuid::new_v4();
        let new = vault.generate_hub_key(id).unwrap();
        let pair = vault
            .open_hub_key(id, &new.public, &new.private_key_enc)
            .unwrap();
        assert_eq!(pair.public(), &new.public);
        assert!(!new.public.is_low_order());
        assert!(new.public.to_text().starts_with("arh1"));

        let unreadable = |outcome: Result<StaticKeypair, VaultError>| {
            matches!(
                outcome,
                Err(VaultError::StoredKeyUnreadable { what: "hub key" })
            )
        };
        // Another row, another public key, a changed block, another master key.
        assert!(unreadable(vault.open_hub_key(
            Uuid::new_v4(),
            &new.public,
            &new.private_key_enc
        )));
        assert!(unreadable(vault.open_hub_key(
            id,
            &device(),
            &new.private_key_enc
        )));
        for position in 0..new.private_key_enc.len() {
            let mut changed = new.private_key_enc.clone();
            changed[position] ^= 0x01;
            assert!(
                unreadable(vault.open_hub_key(id, &new.public, &changed)),
                "byte {position}"
            );
        }
        let (_other_dir, other) = self::vault();
        assert!(unreadable(other.open_hub_key(
            id,
            &new.public,
            &new.private_key_enc
        )));
    }

    #[test]
    fn the_private_key_of_the_hub_is_not_stored_in_the_clear() {
        let (_dir, vault) = vault();
        let id = Uuid::new_v4();
        let new = vault.generate_hub_key(id).unwrap();
        let pair = vault
            .open_hub_key(id, &new.public, &new.private_key_enc)
            .unwrap();
        let private = pair.private().expose();
        assert!(
            !new.private_key_enc
                .windows(KEY_LEN)
                .any(|window| window == private)
        );
        assert_eq!(new.private_key_enc.len(), 1 + NONCE_LEN + KEY_LEN + TAG_LEN);
    }

    #[test]
    fn the_organization_key_opens_only_for_its_organization() {
        let (_dir, vault) = vault();
        let (id, key_enc, _) = organization(&vault);
        let unreadable = |outcome: Result<OrganizationKey, VaultError>| {
            matches!(
                outcome,
                Err(VaultError::StoredKeyUnreadable {
                    what: "organization key"
                })
            )
        };
        assert!(unreadable(
            vault.open_organization_key(Uuid::new_v4(), &key_enc)
        ));
        for position in 0..key_enc.len() {
            let mut changed = key_enc.clone();
            changed[position] ^= 0x80;
            assert!(
                unreadable(vault.open_organization_key(id, &changed)),
                "byte {position}"
            );
        }
        assert!(unreadable(vault.open_organization_key(id, &[])));
        let (_other_dir, other) = self::vault();
        assert!(unreadable(other.open_organization_key(id, &key_enc)));
        // The label of the use is part of what is authenticated: a hub key block is not an
        // organization key, even under the same identifier.
        let hub = vault.generate_hub_key(id).unwrap();
        assert!(unreadable(
            vault.open_organization_key(id, &hub.private_key_enc)
        ));
    }

    #[test]
    fn each_use_of_the_organization_key_has_its_own_key() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        assert_ne!(key.seal, key.peer);
    }

    #[test]
    fn the_derived_keys_are_the_documented_hkdf() {
        // RFC 5869 by hand, with HMAC built from the hash, for a second opinion on the
        // library call: no salt means a salt of 32 zero bytes, and one block of output is
        // HMAC(prk, info || 0x01).
        fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
            use sha2::Digest;
            let mut block = [0u8; 64];
            block[..key.len()].copy_from_slice(key);
            let inner: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
            let outer: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();
            let inner_hash = Sha256::new()
                .chain_update(&inner)
                .chain_update(message)
                .finalize();
            Sha256::new()
                .chain_update(&outer)
                .chain_update(inner_hash)
                .finalize()
                .into()
        }
        let key = Secret::<KEY_LEN>::new([0x5a; KEY_LEN]);
        for label in [SEAL_LABEL, PEER_LABEL] {
            let prk = hmac(&[0u8; 32], key.expose());
            let mut info = label.to_vec();
            info.push(1);
            let expected = hmac(&prk, &info);
            assert_eq!(derive(&key, label).unwrap().expose(), &expected);
        }
    }

    // ------------------------------------------------------------------ the seal

    #[test]
    fn a_seal_goes_and_comes_back() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let issued = key
            .seal_credential(&device, generation(7), 1_700_000_000_123)
            .unwrap();
        assert_eq!(issued.generation, generation(7));
        assert_eq!(issued.digest, CredentialDigest::of(&issued.sealed));

        let opened = key
            .open_credential(issued.sealed.expose(), &device)
            .unwrap();
        assert_eq!(opened.generation, generation(7));
        assert_eq!(opened.issued_at, 1_700_000_000_123);
        assert_eq!(opened.digest, issued.digest);
    }

    #[test]
    fn the_seal_has_the_documented_layout() {
        let (_dir, vault) = vault();
        let (org_id, _, key) = organization(&vault);
        let device = device();
        let issued = key.seal_credential(&device, generation(3), 42).unwrap();
        let block = issued.sealed.expose();

        // version || nonce || ciphertext, opened here without the functions of the module.
        assert_eq!(block[0], 1);
        let nonce = <[u8; NONCE_LEN]>::try_from(&block[1..1 + NONCE_LEN]).unwrap();
        let mut aad = b"arauto/seal/1".to_vec();
        aad.push(1);
        aad.extend_from_slice(org_id.as_bytes());
        aad.extend_from_slice(device.as_bytes());
        let cipher = XChaCha20Poly1305::new_from_slice(key.seal.expose()).unwrap();
        let content = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &block[1 + NONCE_LEN..],
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(block.len(), 1 + NONCE_LEN + content.len() + TAG_LEN);
        assert!(block.len() <= CREDENTIAL_MAX_LEN);

        let text = String::from_utf8(content).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let object = value.as_object().unwrap();
        let fields: Vec<&str> = object.keys().map(String::as_str).collect();
        assert_eq!(fields.len(), 6, "{text}");
        for field in ["v", "org", "device", "generation", "id", "issued_at"] {
            assert!(fields.contains(&field), "{field} in {text}");
        }
        assert_eq!(object["v"], 1);
        assert_eq!(object["org"], org_id.as_hyphenated().to_string());
        assert_eq!(object["device"], BASE64.encode(device.as_bytes()));
        assert_eq!(object["generation"], 3);
        assert_eq!(object["issued_at"], 42);
        let id = BASE64
            .decode(object["id"].as_str().unwrap().as_bytes())
            .unwrap();
        assert_eq!(id.len(), SEAL_ID_LEN);
        // In the order of the document.
        assert!(text.starts_with(r#"{"v":1,"org":""#), "{text}");
        let position = |field: &str| text.find(&format!("\"{field}\":")).unwrap();
        assert!(position("org") < position("device"));
        assert!(position("device") < position("generation"));
        assert!(position("generation") < position("id"));
        assert!(position("id") < position("issued_at"));
    }

    #[test]
    fn two_seals_of_the_same_credential_are_different_blocks() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let first = key.seal_credential(&device, generation(1), 5).unwrap();
        let second = key.seal_credential(&device, generation(1), 5).unwrap();
        assert_ne!(first.sealed.expose(), second.sealed.expose());
        assert_ne!(first.digest, second.digest);
        let first = key.open_credential(first.sealed.expose(), &device).unwrap();
        let second = key
            .open_credential(second.sealed.expose(), &device)
            .unwrap();
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn a_seal_of_another_device_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let owner = device();
        let issued = key.seal_credential(&owner, generation(2), 0).unwrap();
        assert_eq!(
            key.open_credential(issued.sealed.expose(), &device()),
            Err(Unreadable)
        );
        assert!(key.open_credential(issued.sealed.expose(), &owner).is_ok());
    }

    #[test]
    fn a_seal_of_another_organization_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let (_, _, other) = organization(&vault);
        let device = device();
        let issued = key.seal_credential(&device, generation(2), 0).unwrap();
        assert_eq!(
            other.open_credential(issued.sealed.expose(), &device),
            Err(Unreadable)
        );
    }

    #[test]
    fn the_organization_in_the_authenticated_data_matters_even_with_the_same_key() {
        // Two organizations that (wrongly) shared key bytes would still not open each
        // other's seals: the identifier is bound, not only the key.
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let twin = OrganizationKey {
            organization_id: Uuid::new_v4(),
            seal: Secret::new(*key.seal.expose()),
            peer: Secret::new(*key.peer.expose()),
        };
        let device = device();
        let issued = key.seal_credential(&device, generation(2), 0).unwrap();
        assert_eq!(
            twin.open_credential(issued.sealed.expose(), &device),
            Err(Unreadable)
        );
    }

    #[test]
    fn a_seal_changed_anywhere_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let issued = key.seal_credential(&device, generation(9), 77).unwrap();
        let block = issued.sealed.expose();
        // The version, every byte of the nonce, of the content and of the tag.
        for position in 0..block.len() {
            for flip in [0x01u8, 0x80] {
                let mut changed = block.to_vec();
                changed[position] ^= flip;
                assert_eq!(
                    key.open_credential(&changed, &device),
                    Err(Unreadable),
                    "byte {position} with {flip:#x}"
                );
            }
        }
    }

    #[test]
    fn a_seal_of_another_version_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let issued = key.seal_credential(&device, generation(9), 77).unwrap();
        for version in [0u8, 2, 3, 0x7f, 0xff] {
            let mut changed = issued.sealed.expose().to_vec();
            changed[0] = version;
            assert_eq!(
                key.open_credential(&changed, &device),
                Err(Unreadable),
                "version {version}"
            );
        }
        // A block correctly encrypted, but under the version 2 in the authenticated data
        // and in the first byte: the version is checked, not only authenticated.
        let mut aad = SEAL_LABEL.to_vec();
        aad.push(2);
        aad.extend_from_slice(key.organization_id.as_bytes());
        aad.extend_from_slice(device.as_bytes());
        let nonce = [3u8; NONCE_LEN];
        let content = br#"{"v":2,"org":"x","device":"x","generation":1,"id":"x","issued_at":0}"#;
        let mut forged = vec![2u8];
        forged.extend_from_slice(&nonce);
        forged.extend_from_slice(&encrypt(&key.seal, &nonce, &aad, content).unwrap());
        assert_eq!(key.open_credential(&forged, &device), Err(Unreadable));
    }

    #[test]
    fn a_seal_of_the_wrong_size_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let issued = key.seal_credential(&device, generation(9), 77).unwrap();
        let block = issued.sealed.expose();

        assert_eq!(key.open_credential(&[], &device), Err(Unreadable));
        assert_eq!(key.open_credential(&[1], &device), Err(Unreadable));
        assert_eq!(
            key.open_credential(&block[..1 + NONCE_LEN], &device),
            Err(Unreadable)
        );
        assert_eq!(
            key.open_credential(&block[..1 + NONCE_LEN + TAG_LEN - 1], &device),
            Err(Unreadable)
        );
        assert_eq!(
            key.open_credential(&block[..block.len() - 1], &device),
            Err(Unreadable)
        );
        let mut longer = block.to_vec();
        longer.push(0);
        assert_eq!(key.open_credential(&longer, &device), Err(Unreadable));
        for size in [CREDENTIAL_MAX_LEN + 1, 65_536, 1 << 20] {
            assert_eq!(
                key.open_credential(&vec![1u8; size], &device),
                Err(Unreadable),
                "{size} bytes"
            );
        }
    }

    #[test]
    fn a_valid_seal_larger_than_a_credential_does_not_open() {
        // A seal that is right in everything but its size. Only the hub could make one, so
        // the test seals it by hand.
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let content = SealContent {
            v: BLOCK_VERSION,
            org: key.organization_id,
            device: BASE64.encode(device.as_bytes()),
            generation: generation(1),
            id: BASE64.encode(&[0u8; SEAL_ID_LEN]),
            issued_at: 0,
        };
        let mut json = serde_json::to_vec(&content).unwrap();
        // JSON allows white space after the value: the content stays valid.
        json.resize(CREDENTIAL_MAX_LEN, b' ');
        let block = seal_block(&key.seal, &key.seal_aad(&device), &json).unwrap();
        assert!(block.len() > CREDENTIAL_MAX_LEN);
        assert_eq!(key.open_credential(&block, &device), Err(Unreadable));

        // The same content within the limit opens: only the size made the difference.
        json.truncate(CREDENTIAL_MAX_LEN - 1 - NONCE_LEN - TAG_LEN);
        let block = seal_block(&key.seal, &key.seal_aad(&device), &json).unwrap();
        assert_eq!(block.len(), CREDENTIAL_MAX_LEN);
        assert!(key.open_credential(&block, &device).is_ok());
    }

    #[test]
    fn a_seal_whose_content_disagrees_with_its_binding_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let aad = key.seal_aad(&device);
        let good = SealContent {
            v: BLOCK_VERSION,
            org: key.organization_id,
            device: BASE64.encode(device.as_bytes()),
            generation: generation(4),
            id: BASE64.encode(&[1u8; SEAL_ID_LEN]),
            issued_at: 9,
        };
        let seal = |content: &serde_json::Value| {
            seal_block(&key.seal, &aad, &serde_json::to_vec(content).unwrap()).unwrap()
        };
        let base = serde_json::to_value(&good).unwrap();
        assert!(key.open_credential(&seal(&base), &device).is_ok());

        let mut changes: Vec<serde_json::Value> = Vec::new();
        let mut with = |field: &str, value: serde_json::Value| {
            let mut changed = base.clone();
            changed[field] = value;
            changes.push(changed);
        };
        with("v", 2.into());
        with("org", Uuid::new_v4().to_string().into());
        with("device", BASE64.encode(self::device().as_bytes()).into());
        with("device", "not base64".into());
        with("device", BASE64.encode(&[0u8; 31]).into());
        with("generation", 0.into());
        with("generation", (-1).into());
        with("generation", "4".into());
        with("id", BASE64.encode(&[0u8; 15]).into());
        with("id", "A".repeat(4096).into());
        with("issued_at", "soon".into());
        with("extra", true.into());
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("id");
        changes.push(missing);
        for changed in &changes {
            assert_eq!(
                key.open_credential(&seal(changed), &device),
                Err(Unreadable),
                "{changed}"
            );
        }
        for not_an_object in [&b"null"[..], b"[]", b"", b"{", b"\xff\xfe"] {
            let block = seal_block(&key.seal, &aad, not_an_object).unwrap();
            assert_eq!(key.open_credential(&block, &device), Err(Unreadable));
        }
    }

    #[test]
    fn the_seal_does_not_show_what_it_says() {
        let (_dir, vault) = vault();
        let (org_id, _, key) = organization(&vault);
        let device = device();
        let issued = key.seal_credential(&device, generation(5), 1).unwrap();
        let block = issued.sealed.expose();
        let contains = |needle: &[u8]| block.windows(needle.len()).any(|window| window == needle);
        assert!(!contains(org_id.as_bytes()));
        assert!(!contains(device.as_bytes()));
        assert!(!contains(b"generation"));
        assert_eq!(format!("{:?}", issued.sealed), "[redacted]");
    }

    #[test]
    fn the_seal_key_is_not_the_peer_key() {
        // A text encrypted for a message is not a credential, and the other way around.
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let device = device();
        let crossed = OrganizationKey {
            organization_id: key.organization_id,
            seal: Secret::new(*key.peer.expose()),
            peer: Secret::new(*key.seal.expose()),
        };
        let issued = key.seal_credential(&device, generation(1), 0).unwrap();
        assert_eq!(
            crossed.open_credential(issued.sealed.expose(), &device),
            Err(Unreadable)
        );
    }

    // ------------------------------------------------------------------ message texts

    fn ids() -> PeerMessageIds {
        PeerMessageIds {
            thread: Uuid::new_v4(),
            message: Uuid::new_v4(),
            from: Uuid::new_v4(),
            to: Uuid::new_v4(),
        }
    }

    #[test]
    fn a_message_text_goes_and_comes_back() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let ids = ids();
        let text = MessageText::parse("status?\nsecond line \u{1F600}").unwrap();
        let stored = key.seal_peer_text(&ids, &text).unwrap();
        assert_eq!(stored.text_enc.len(), text.as_str().len() + TAG_LEN);
        assert!(
            !stored
                .text_enc
                .windows(7)
                .any(|window| window == b"status?")
        );
        let opened = key
            .open_peer_text(&ids, &stored.nonce, &stored.text_enc)
            .unwrap();
        assert_eq!(opened, text);

        let again = key.seal_peer_text(&ids, &text).unwrap();
        assert_ne!(again.nonce, stored.nonce);
        assert_ne!(again.text_enc, stored.text_enc);
    }

    #[test]
    fn the_longest_text_fits() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let ids = ids();
        let text = MessageText::new("a".repeat(PEER_TEXT_MAX_BYTES)).unwrap();
        let stored = key.seal_peer_text(&ids, &text).unwrap();
        assert!(
            key.open_peer_text(&ids, &stored.nonce, &stored.text_enc)
                .is_ok()
        );
        let mut longer = stored.text_enc.clone();
        longer.push(0);
        assert_eq!(
            key.open_peer_text(&ids, &stored.nonce, &longer),
            Err(PeerTextUnreadable)
        );
    }

    #[test]
    fn a_message_text_does_not_open_as_another_message() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let (_, _, other_organization) = organization(&vault);
        let ids = ids();
        let text = MessageText::parse("do it").unwrap();
        let stored = key.seal_peer_text(&ids, &text).unwrap();

        let moved = [
            PeerMessageIds {
                thread: Uuid::new_v4(),
                ..ids
            },
            PeerMessageIds {
                message: Uuid::new_v4(),
                ..ids
            },
            PeerMessageIds {
                from: Uuid::new_v4(),
                ..ids
            },
            PeerMessageIds {
                to: Uuid::new_v4(),
                ..ids
            },
            // The sender and the recipient swapped: a text is not turned around.
            PeerMessageIds {
                from: ids.to,
                to: ids.from,
                ..ids
            },
        ];
        for other in &moved {
            assert_eq!(
                key.open_peer_text(other, &stored.nonce, &stored.text_enc),
                Err(PeerTextUnreadable),
                "{other:?}"
            );
        }
        assert_eq!(
            other_organization.open_peer_text(&ids, &stored.nonce, &stored.text_enc),
            Err(PeerTextUnreadable)
        );
    }

    #[test]
    fn a_message_text_changed_anywhere_does_not_open() {
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let ids = ids();
        let text = MessageText::parse("a text of some length").unwrap();
        let stored = key.seal_peer_text(&ids, &text).unwrap();
        for position in 0..stored.text_enc.len() {
            let mut changed = stored.text_enc.clone();
            changed[position] ^= 0x01;
            assert_eq!(
                key.open_peer_text(&ids, &stored.nonce, &changed),
                Err(PeerTextUnreadable)
            );
        }
        for position in 0..NONCE_LEN {
            let mut nonce = stored.nonce;
            nonce[position] ^= 0x01;
            assert_eq!(
                key.open_peer_text(&ids, &nonce, &stored.text_enc),
                Err(PeerTextUnreadable)
            );
        }
        for nonce in [&[][..], &stored.nonce[..23], &[0u8; 25][..]] {
            assert_eq!(
                key.open_peer_text(&ids, nonce, &stored.text_enc),
                Err(PeerTextUnreadable)
            );
        }
        assert_eq!(
            key.open_peer_text(&ids, &stored.nonce, &[]),
            Err(PeerTextUnreadable)
        );
    }

    #[test]
    fn what_opens_must_still_be_a_valid_text() {
        // Whoever has the key could store anything; the reader still gets a checked text.
        let (_dir, vault) = vault();
        let (_, _, key) = organization(&vault);
        let ids = ids();
        let nonce = [9u8; NONCE_LEN];
        for raw in [&b""[..], b"\x1b[2J", b"\xff\xfe", b"   "] {
            let text_enc = encrypt(&key.peer, &nonce, &key.peer_aad(&ids), raw).unwrap();
            assert_eq!(
                key.open_peer_text(&ids, &nonce, &text_enc),
                Err(PeerTextUnreadable),
                "{raw:?}"
            );
        }
    }
}
