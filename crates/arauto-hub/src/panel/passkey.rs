//! The passkeys of the panel: the two WebAuthn ceremonies, on the side of the hub.
//!
//! The checks are those of the `webauthn_rp` library (pure Rust); this module only chooses the
//! policy and keeps the open ceremonies. No database, no HTTP and no log here: the caller loads
//! and stores the rows and shows the person one fixed sentence when something fails.
//!
//! Policy:
//! - the relying party is the host of the panel's own origin, and only that exact origin
//!   (scheme, host and port) is accepted; `http://` only for `localhost`;
//! - user verification is required in both ceremonies; a response from a frame of another
//!   origin (`crossOrigin`) is refused;
//! - attestation "none" is asked for and accepted (the library also accepts a self-signed
//!   "packed" one, and refuses every other format);
//! - a challenge is drawn by the library from the generator of the system, lives five minutes,
//!   and is taken out of memory by the first attempt that names it, whether it succeeds or not;
//! - a signature counter that does not go up (when it is not zero) fails the ceremony.
//!
//! What is kept for a passkey: the credential id, the user handle (the UUID of the admin), the
//! static state (public key and the extensions that matter later) and the dynamic state
//! (counter, backup flags), the last two as opaque bytes from the library's `Encode`.

use std::collections::HashSet;
use std::hash::Hash;
use std::num::NonZeroU32;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use webauthn_rp::bin::{Decode, Encode};
use webauthn_rp::request::auth::AuthenticationVerificationOptions;
use webauthn_rp::request::register::{
    Nickname, PublicKeyCredentialUserEntity, RegistrationVerificationOptions, UserHandle, Username,
};
use webauthn_rp::request::{
    AsciiDomain, DomainOrigin, Port, PublicKeyCredentialDescriptor, RpId, Scheme, TimedCeremony,
};
use webauthn_rp::response::auth::error::AuthCeremonyErr;
use webauthn_rp::response::register::error::RegCeremonyErr;
use webauthn_rp::response::register::{CompressedPubKey, DynamicState, StaticState};
use webauthn_rp::response::{AuthTransports, CredentialId, SentChallenge};
use webauthn_rp::{
    AuthenticatedCredential, DiscoverableAuthentication16, DiscoverableAuthenticationServerState,
    DiscoverableCredentialRequestOptions, PublicKeyCredentialCreationOptions, Registration,
    RegistrationServerState,
};

/// The bytes of a user handle: the UUID of the admin.
pub const USER_HANDLE_LEN: usize = 16;

/// How many ceremonies of each kind may be open at once, by default.
pub const DEFAULT_CAPACITY: usize = 256;

/// How long a ceremony stays open, by default (the library's own five minutes).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// The bytes of the dynamic state, as the library encodes it.
const DYNAMIC_STATE_LEN: usize = 7;

/// The public key of a stored passkey, as the library decodes it.
type StoredKey = CompressedPubKey<[u8; 32], [u8; 32], [u8; 48], Vec<u8>>;

/// Why a ceremony, or the relying party, could not be made.
///
/// The text never carries a byte that came from the browser, a challenge or a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasskeyError {
    /// The origin of the panel is not `https://host[:port]`, or `http://localhost[:port]`.
    #[error("the origin of the panel cannot be a relying party")]
    InvalidOrigin,
    /// The user name is empty, too long, or not acceptable to WebAuthn.
    #[error("the user name cannot be used for a passkey")]
    InvalidUserName,
    /// Too many ceremonies are open; none has expired yet.
    #[error("too many passkey ceremonies are open")]
    Busy,
    /// The response of the browser could not be read.
    #[error("the response of the browser could not be read")]
    Malformed,
    /// No open ceremony has this challenge: never issued, already used, or dropped.
    #[error("no open ceremony has this challenge")]
    UnknownChallenge,
    /// The ceremony was open, but its time had run out.
    #[error("the ceremony expired")]
    Expired,
    /// The response was made for another origin or relying party, or inside a frame of
    /// another origin.
    #[error("the response is for another origin")]
    WrongOrigin,
    /// The authenticator did not verify the user.
    #[error("the user was not verified")]
    UserNotVerified,
    /// The signature does not match the key.
    #[error("the signature does not match")]
    BadSignature,
    /// The signature counter did not go up: the passkey may have been copied.
    #[error("the signature counter went back")]
    CounterWentBack,
    /// The registration was started for another admin than the one finishing it.
    #[error("the registration belongs to another user")]
    WrongUser,
    /// The response names no passkey the hub keeps.
    #[error("the passkey is not known")]
    UnknownCredential,
    /// The stored row does not match the response (another credential id or user handle).
    #[error("the passkey does not match the stored one")]
    CredentialMismatch,
    /// The stored bytes of a passkey could not be decoded.
    #[error("the stored passkey could not be read")]
    InvalidStoredState,
    /// The response broke another rule of WebAuthn.
    #[error("the response was refused")]
    Rejected,
}

/// A passkey just registered: what must be stored, in BYTEA columns.
///
/// `Debug` shows only the lengths.
pub struct NewPasskey {
    /// The credential id. Must be unique: the caller refuses one it already has.
    pub credential_id: Vec<u8>,
    /// The user handle the ceremony was started for.
    pub user_handle: [u8; USER_HANDLE_LEN],
    /// The static state (public key and extensions), opaque.
    pub static_state: Vec<u8>,
    /// The dynamic state (counter and flags), opaque; replaced after each sign-in that
    /// changes it.
    pub dynamic_state: Vec<u8>,
}

impl std::fmt::Debug for NewPasskey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewPasskey")
            .field("credential_id", &self.credential_id.len())
            .field("static_state", &self.static_state.len())
            .field("dynamic_state", &self.dynamic_state.len())
            .finish_non_exhaustive()
    }
}

/// A stored passkey, as the caller loaded it from its row.
#[derive(Clone, Copy)]
pub struct StoredPasskey<'a> {
    /// The credential id.
    pub credential_id: &'a [u8],
    /// The user handle (16 bytes).
    pub user_handle: &'a [u8],
    /// The static state, as [`NewPasskey::static_state`] gave it.
    pub static_state: &'a [u8],
    /// The current dynamic state.
    pub dynamic_state: &'a [u8],
}

impl std::fmt::Debug for StoredPasskey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoredPasskey(..)")
    }
}

/// A sign-in response that was read and whose challenge was taken out of memory.
///
/// It names the passkey it claims, so that the caller can load the row; then it goes to
/// [`RelyingParty::finish_authentication`]. Dropping it ends the ceremony: the challenge is
/// already spent.
pub struct PendingAuthentication {
    response: DiscoverableAuthentication16,
    state: DiscoverableAuthenticationServerState,
}

impl PendingAuthentication {
    /// The credential id the response claims (not checked yet).
    #[must_use]
    pub fn credential_id(&self) -> &[u8] {
        self.response.raw_id().into_inner()
    }

    /// The user handle the response claims (not checked yet).
    #[must_use]
    pub fn user_handle(&self) -> [u8; USER_HANDLE_LEN] {
        let Ok(bytes) = self.response.response().user_handle().encode();
        bytes
    }
}

impl std::fmt::Debug for PendingAuthentication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingAuthentication(..)")
    }
}

/// A sign-in that passed every check.
#[derive(Debug)]
pub struct Authenticated {
    /// The new dynamic state, when it changed (the counter, the backup flags): the caller
    /// writes it over the stored one. `None` when nothing changed.
    pub updated_dynamic_state: Option<Vec<u8>>,
}

/// The ceremonies of one kind that are open, with a hard limit on how many.
struct CeremonyStore<T> {
    open: HashSet<T>,
    capacity: usize,
}

impl<T> CeremonyStore<T>
where
    T: TimedCeremony + Hash + Eq + std::borrow::Borrow<SentChallenge>,
{
    fn new(capacity: usize) -> Self {
        Self {
            open: HashSet::new(),
            capacity,
        }
    }

    /// Keeps `ceremony`, dropping the expired ones first when full. Refuses when still full.
    fn insert(&mut self, ceremony: T) -> Result<(), PasskeyError> {
        if self.open.len() >= self.capacity {
            let now = Instant::now();
            self.open.retain(|c| c.expiration() >= now);
        }
        if self.open.len() >= self.capacity || !self.open.insert(ceremony) {
            return Err(PasskeyError::Busy);
        }
        Ok(())
    }

    /// Takes out the ceremony with this challenge: it can be used once.
    fn take(&mut self, challenge: &SentChallenge) -> Option<T> {
        self.open.take(challenge)
    }

    fn len(&self) -> usize {
        self.open.len()
    }
}

/// The relying party of the panel, with its open ceremonies.
///
/// Safe to share between tasks: each store sits behind its own lock, held only for an insert
/// or a take.
pub struct RelyingParty {
    rp_id: RpId,
    /// The host, lower case, as the browser writes it in the origin.
    host: String,
    /// `false` only for `http://localhost`.
    https: bool,
    /// The port, when the origin names one that is not the default of its scheme.
    port: Option<u16>,
    timeout: NonZeroU32,
    registrations: Mutex<CeremonyStore<RegistrationServerState<USER_HANDLE_LEN>>>,
    authentications: Mutex<CeremonyStore<DiscoverableAuthenticationServerState>>,
}

impl std::fmt::Debug for RelyingParty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelyingParty")
            .field("rp_id", &self.rp_id.as_ref())
            .field("https", &self.https)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl RelyingParty {
    /// The relying party for the panel at `origin`, with the default limits.
    ///
    /// # Errors
    /// [`PasskeyError::InvalidOrigin`] when `origin` is not `https://host[:port]` or
    /// `http://localhost[:port]` (an optional `/` at the end is allowed).
    pub fn new(origin: &str) -> Result<Self, PasskeyError> {
        Self::with_limits(origin, DEFAULT_CAPACITY, DEFAULT_TIMEOUT)
    }

    /// The relying party with `capacity` open ceremonies of each kind and ceremonies that last
    /// `timeout` (at least one millisecond, at most about 49 days).
    ///
    /// # Errors
    /// [`PasskeyError::InvalidOrigin`] as in [`Self::new`]; [`PasskeyError::Busy`] when
    /// `capacity` is zero; [`PasskeyError::Rejected`] when `timeout` is out of range.
    pub fn with_limits(
        origin: &str,
        capacity: usize,
        timeout: Duration,
    ) -> Result<Self, PasskeyError> {
        let parsed = parse_origin(origin)?;
        if capacity == 0 {
            return Err(PasskeyError::Busy);
        }
        let timeout = u32::try_from(timeout.as_millis())
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(PasskeyError::Rejected)?;
        let domain =
            AsciiDomain::try_from(parsed.host.clone()).map_err(|_| PasskeyError::InvalidOrigin)?;
        Ok(Self {
            rp_id: RpId::Domain(domain),
            host: parsed.host,
            https: parsed.https,
            port: parsed.port,
            timeout,
            registrations: Mutex::new(CeremonyStore::new(capacity)),
            authentications: Mutex::new(CeremonyStore::new(capacity)),
        })
    }

    /// The RP ID: the host of the panel, without the port.
    #[must_use]
    pub fn rp_id(&self) -> &str {
        self.rp_id.as_ref()
    }

    /// How many registrations and how many sign-ins are open now (expired ones included until
    /// they are dropped).
    #[must_use]
    pub fn open_ceremonies(&self) -> (usize, usize) {
        let registrations = lock(&self.registrations).len();
        let authentications = lock(&self.authentications).len();
        (registrations, authentications)
    }

    /// Starts the registration of a passkey for the admin with `user_handle`, shown to the
    /// person as `user_name`. `existing` are the credential ids the admin already has, so that
    /// the authenticator does not make a second passkey on top of one of them.
    ///
    /// Returns the JSON for `PublicKeyCredential.parseCreationOptionsFromJSON()`.
    ///
    /// # Errors
    /// [`PasskeyError::InvalidUserName`], [`PasskeyError::InvalidStoredState`] for an id in
    /// `existing` of an impossible length, [`PasskeyError::Busy`].
    pub fn start_registration<C: AsRef<[u8]>>(
        &self,
        user_handle: &[u8; USER_HANDLE_LEN],
        user_name: &str,
        existing: &[C],
    ) -> Result<String, PasskeyError> {
        let Ok(id) = UserHandle::<USER_HANDLE_LEN>::decode(*user_handle);
        let name = Username::try_from(user_name).map_err(|_| PasskeyError::InvalidUserName)?;
        let display_name = Nickname::try_from(user_name).ok();
        let transports = AuthTransports::decode(0).map_err(|_| PasskeyError::InvalidStoredState)?;
        let exclude = existing
            .iter()
            .map(|id| {
                CredentialId::<Vec<u8>>::decode(id.as_ref().to_vec())
                    .map(|id| PublicKeyCredentialDescriptor { id, transports })
                    .map_err(|_| PasskeyError::InvalidStoredState)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut options = PublicKeyCredentialCreationOptions::passkey(
            &self.rp_id,
            PublicKeyCredentialUserEntity {
                name,
                id: &id,
                display_name,
            },
            exclude,
        );
        options.timeout = self.timeout;
        let (server, client) = options
            .start_ceremony()
            .map_err(|_| PasskeyError::Rejected)?;
        let json = serde_json::to_string(&client).map_err(|_| PasskeyError::Rejected)?;
        lock(&self.registrations).insert(server)?;
        Ok(json)
    }

    /// Finishes a registration with the JSON of `credential.toJSON()`. `user_handle` is the
    /// admin of the panel session finishing it, who must be the one it was started for.
    ///
    /// The challenge is spent by this call, whatever its result.
    ///
    /// # Errors
    /// [`PasskeyError::Malformed`], [`PasskeyError::UnknownChallenge`],
    /// [`PasskeyError::WrongUser`], or the check that failed.
    pub fn finish_registration(
        &self,
        user_handle: &[u8; USER_HANDLE_LEN],
        response_json: &[u8],
    ) -> Result<NewPasskey, PasskeyError> {
        let registration =
            Registration::from_json_relaxed(response_json).map_err(|_| PasskeyError::Malformed)?;
        let challenge = registration
            .challenge_relaxed()
            .map_err(|_| PasskeyError::Malformed)?;
        let server = lock(&self.registrations)
            .take(&challenge)
            .ok_or(PasskeyError::UnknownChallenge)?;
        let origins = [self.domain_origin()];
        let options = RegistrationVerificationOptions::<DomainOrigin<'_, '_>, DomainOrigin<'_, '_>> {
            allowed_origins: &origins,
            allowed_top_origins: None,
            // Some password managers and browsers answer with extensions nobody asked for.
            error_on_unsolicited_extensions: false,
            ..RegistrationVerificationOptions::default()
        };
        let credential = server
            .verify(&self.rp_id, &registration, &options)
            .map_err(registration_error)?;
        let Ok(owner) = credential.user_id().encode();
        if owner != *user_handle {
            return Err(PasskeyError::WrongUser);
        }
        let Ok(static_state) = credential.static_state().encode();
        let Ok(dynamic_state) = credential.dynamic_state().encode();
        Ok(NewPasskey {
            credential_id: credential.id().into_inner().to_vec(),
            user_handle: owner,
            static_state,
            dynamic_state: dynamic_state.to_vec(),
        })
    }

    /// Starts a sign-in with a discoverable passkey: no user name is typed.
    ///
    /// Returns the JSON for `PublicKeyCredential.parseRequestOptionsFromJSON()`.
    ///
    /// # Errors
    /// [`PasskeyError::Busy`].
    pub fn start_authentication(&self) -> Result<String, PasskeyError> {
        let mut options = DiscoverableCredentialRequestOptions::passkey(&self.rp_id);
        options.0.timeout = self.timeout;
        let (server, client) = options
            .start_ceremony()
            .map_err(|_| PasskeyError::Rejected)?;
        let json = serde_json::to_string(&client).map_err(|_| PasskeyError::Rejected)?;
        lock(&self.authentications).insert(server)?;
        Ok(json)
    }

    /// First half of finishing a sign-in: reads the JSON of `credential.toJSON()` and takes
    /// its challenge out of memory. The result names the passkey it claims, for the caller to
    /// load its row.
    ///
    /// # Errors
    /// [`PasskeyError::Malformed`], [`PasskeyError::UnknownChallenge`].
    pub fn claim_authentication(
        &self,
        response_json: &[u8],
    ) -> Result<PendingAuthentication, PasskeyError> {
        let response = DiscoverableAuthentication16::from_json_relaxed(response_json)
            .map_err(|_| PasskeyError::Malformed)?;
        let challenge = response
            .challenge_relaxed()
            .map_err(|_| PasskeyError::Malformed)?;
        let state = lock(&self.authentications)
            .take(&challenge)
            .ok_or(PasskeyError::UnknownChallenge)?;
        Ok(PendingAuthentication { response, state })
    }

    /// Second half: checks the response against the stored passkey the caller found for it
    /// (`None` when it found none).
    ///
    /// # Errors
    /// [`PasskeyError::UnknownCredential`] when `stored` is `None`;
    /// [`PasskeyError::InvalidStoredState`]; or the check that failed.
    pub fn finish_authentication(
        &self,
        pending: PendingAuthentication,
        stored: Option<StoredPasskey<'_>>,
    ) -> Result<Authenticated, PasskeyError> {
        let stored = stored.ok_or(PasskeyError::UnknownCredential)?;
        let user_handle: [u8; USER_HANDLE_LEN] = stored
            .user_handle
            .try_into()
            .map_err(|_| PasskeyError::InvalidStoredState)?;
        let Ok(user) = UserHandle::<USER_HANDLE_LEN>::decode(user_handle);
        let id = CredentialId::<&[u8]>::decode(stored.credential_id)
            .map_err(|_| PasskeyError::InvalidStoredState)?;
        let static_state = StaticState::<StoredKey>::decode(stored.static_state)
            .map_err(|_| PasskeyError::InvalidStoredState)?;
        let dynamic: [u8; DYNAMIC_STATE_LEN] = stored
            .dynamic_state
            .try_into()
            .map_err(|_| PasskeyError::InvalidStoredState)?;
        let dynamic_state =
            DynamicState::decode(dynamic).map_err(|_| PasskeyError::InvalidStoredState)?;
        let mut credential = AuthenticatedCredential::new(id, &user, static_state, dynamic_state)
            .map_err(|_| PasskeyError::InvalidStoredState)?;

        let origins = [self.domain_origin()];
        let options =
            AuthenticationVerificationOptions::<DomainOrigin<'_, '_>, DomainOrigin<'_, '_>> {
                allowed_origins: &origins,
                allowed_top_origins: None,
                // As in the registration: an unasked extension is not a reason to refuse.
                error_on_unsolicited_extensions: false,
                ..AuthenticationVerificationOptions::default()
            };
        let changed = pending
            .state
            .verify(&self.rp_id, &pending.response, &mut credential, &options)
            .map_err(authentication_error)?;
        let updated_dynamic_state = if changed {
            let Ok(bytes) = credential.dynamic_state().encode();
            Some(bytes.to_vec())
        } else {
            None
        };
        Ok(Authenticated {
            updated_dynamic_state,
        })
    }

    /// The one origin the browser may write in its client data.
    fn domain_origin(&self) -> DomainOrigin<'_, '_> {
        DomainOrigin {
            scheme: if self.https {
                Scheme::Https
            } else {
                Scheme::Other("http")
            },
            host: &self.host,
            port: self.port.map_or(Port::None, Port::Val),
        }
    }
}

/// Takes a lock; a panic elsewhere while it was held leaves the set whole, so it is used as is.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The parts of the origin of the panel.
struct ParsedOrigin {
    https: bool,
    host: String,
    port: Option<u16>,
}

/// Reads `https://host[:port]` or `http://localhost[:port]`, with an optional `/` at the end.
/// The host must be a domain name (no IP address): browsers do not allow one as an RP ID.
fn parse_origin(origin: &str) -> Result<ParsedOrigin, PasskeyError> {
    let origin = origin.strip_suffix('/').unwrap_or(origin);
    let (https, rest) = if let Some(rest) = origin.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = origin.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err(PasskeyError::InvalidOrigin);
    };
    let (host, port) = match rest.split_once(':') {
        Some((host, port)) => (host, Some(parse_port(port)?)),
        None => (rest, None),
    };
    let host = host.to_ascii_lowercase();
    if !is_domain(&host) {
        return Err(PasskeyError::InvalidOrigin);
    }
    if !https && host != "localhost" {
        return Err(PasskeyError::InvalidOrigin);
    }
    let default_port = if https { 443 } else { 80 };
    let port = port.filter(|p| *p != default_port);
    Ok(ParsedOrigin { https, host, port })
}

/// A port written as a browser writes it: digits, no leading zero, not zero.
fn parse_port(text: &str) -> Result<u16, PasskeyError> {
    if text.is_empty() || text.starts_with('0') || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PasskeyError::InvalidOrigin);
    }
    text.parse().map_err(|_| PasskeyError::InvalidOrigin)
}

/// A domain name in lower case: labels of letters, digits and hyphens, not starting or ending
/// with a hyphen, and a last label that is not all digits (which would be an IPv4 address).
fn is_domain(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    let labels_ok = host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    });
    let last_not_numeric = host
        .rsplit('.')
        .next()
        .is_some_and(|last| !last.bytes().all(|b| b.is_ascii_digit()));
    labels_ok && last_not_numeric
}

/// The error of a failed registration, without its details.
fn registration_error(error: RegCeremonyErr) -> PasskeyError {
    match error {
        RegCeremonyErr::Timeout => PasskeyError::Expired,
        RegCeremonyErr::OriginMismatch
        | RegCeremonyErr::CrossOrigin
        | RegCeremonyErr::TopOriginMismatch
        | RegCeremonyErr::RpIdHashMismatch => PasskeyError::WrongOrigin,
        RegCeremonyErr::UserNotVerified | RegCeremonyErr::UserNotPresent => {
            PasskeyError::UserNotVerified
        }
        RegCeremonyErr::AttestationSignature => PasskeyError::BadSignature,
        RegCeremonyErr::CollectedClientDataRelaxed(_) | RegCeremonyErr::CollectedClientData(_) => {
            PasskeyError::Malformed
        }
        _ => PasskeyError::Rejected,
    }
}

/// The error of a failed sign-in, without its details.
fn authentication_error(error: AuthCeremonyErr) -> PasskeyError {
    match error {
        AuthCeremonyErr::Timeout => PasskeyError::Expired,
        AuthCeremonyErr::OriginMismatch
        | AuthCeremonyErr::CrossOrigin
        | AuthCeremonyErr::TopOriginMismatch
        | AuthCeremonyErr::RpIdHashMismatch => PasskeyError::WrongOrigin,
        AuthCeremonyErr::UserNotVerified => PasskeyError::UserNotVerified,
        AuthCeremonyErr::AssertionSignature => PasskeyError::BadSignature,
        AuthCeremonyErr::SignatureCounter => PasskeyError::CounterWentBack,
        AuthCeremonyErr::UserHandleMismatch | AuthCeremonyErr::CredentialIdMismatch => {
            PasskeyError::CredentialMismatch
        }
        AuthCeremonyErr::CollectedClientDataRelaxed(_)
        | AuthCeremonyErr::CollectedClientData(_) => PasskeyError::Malformed,
        _ => PasskeyError::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arauto_testkit::passkey::SoftAuthenticator;

    const ORIGIN: &str = "https://panel.example.com";
    const ADMIN: [u8; USER_HANDLE_LEN] = [7; USER_HANDLE_LEN];

    /// Registers one passkey for [`ADMIN`] and returns what the hub would store.
    fn register(rp: &RelyingParty, authenticator: &mut SoftAuthenticator) -> NewPasskey {
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        rp.finish_registration(&ADMIN, response.as_bytes()).unwrap()
    }

    fn stored(passkey: &NewPasskey) -> StoredPasskey<'_> {
        StoredPasskey {
            credential_id: &passkey.credential_id,
            user_handle: &passkey.user_handle,
            static_state: &passkey.static_state,
            dynamic_state: &passkey.dynamic_state,
        }
    }

    /// Signs in once with `authenticator`, the hub finding `passkey` for it.
    fn sign_in(
        rp: &RelyingParty,
        authenticator: &mut SoftAuthenticator,
        passkey: &NewPasskey,
    ) -> Result<Authenticated, PasskeyError> {
        let options = rp.start_authentication().unwrap();
        let response = authenticator.get(&options).unwrap();
        let pending = rp.claim_authentication(response.as_bytes())?;
        rp.finish_authentication(pending, Some(stored(passkey)))
    }

    fn counter_of(dynamic_state: &[u8]) -> u32 {
        let bytes: [u8; DYNAMIC_STATE_LEN] = dynamic_state.try_into().unwrap();
        DynamicState::decode(bytes).unwrap().sign_count
    }

    #[test]
    fn a_passkey_registers_and_then_signs_in() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        assert_eq!(rp.rp_id(), "panel.example.com");
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let mut passkey = register(&rp, &mut authenticator);
        assert_eq!(passkey.user_handle, ADMIN);
        assert_eq!(
            authenticator.credential_ids(),
            vec![passkey.credential_id.clone()]
        );

        // The stored bytes decode again.
        StaticState::<StoredKey>::decode(passkey.static_state.as_slice()).unwrap();
        assert_eq!(counter_of(&passkey.dynamic_state), 0);

        // The claimed passkey is the registered one.
        let options = rp.start_authentication().unwrap();
        let response = authenticator.get(&options).unwrap();
        let pending = rp.claim_authentication(response.as_bytes()).unwrap();
        assert_eq!(pending.credential_id(), passkey.credential_id.as_slice());
        assert_eq!(pending.user_handle(), ADMIN);
        let first = rp
            .finish_authentication(pending, Some(stored(&passkey)))
            .unwrap();
        let updated = first.updated_dynamic_state.unwrap();
        assert_eq!(counter_of(&updated), 1);
        passkey.dynamic_state = updated;

        // The counter moves on the next sign-in.
        let second = sign_in(&rp, &mut authenticator, &passkey).unwrap();
        assert_eq!(counter_of(&second.updated_dynamic_state.unwrap()), 2);
        assert_eq!(rp.open_ceremonies(), (0, 0));
    }

    #[test]
    fn an_existing_passkey_is_excluded() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut authenticator);
        let options = rp
            .start_registration(&ADMIN, "admin", &[passkey.credential_id])
            .unwrap();
        assert!(authenticator.create(&options).is_err());
    }

    #[test]
    fn a_registration_for_another_origin_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new("https://evil.example.net");
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        assert_eq!(
            rp.finish_registration(&ADMIN, response.as_bytes())
                .unwrap_err(),
            PasskeyError::WrongOrigin
        );
    }

    #[test]
    fn a_sign_in_for_another_origin_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut authenticator);
        // Same host, another port: still another origin.
        authenticator.answer_as("https://panel.example.com:8443");
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::WrongOrigin
        );
        authenticator.answer_as("http://panel.example.com");
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::WrongOrigin
        );
    }

    #[test]
    fn a_response_from_a_frame_of_another_origin_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        authenticator.inside_frame_of(Some("https://evil.example.net"));
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        assert_eq!(
            rp.finish_registration(&ADMIN, response.as_bytes())
                .unwrap_err(),
            PasskeyError::WrongOrigin
        );

        authenticator.inside_frame_of(None);
        let passkey = register(&rp, &mut authenticator);
        authenticator.inside_frame_of(Some("https://evil.example.net"));
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::WrongOrigin
        );
    }

    #[test]
    fn a_response_without_user_verification_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        authenticator.skip_user_verification(true);
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        assert_eq!(
            rp.finish_registration(&ADMIN, response.as_bytes())
                .unwrap_err(),
            PasskeyError::UserNotVerified
        );

        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut authenticator);
        authenticator.skip_user_verification(true);
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::UserNotVerified
        );
    }

    #[test]
    fn a_replayed_response_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut authenticator);
        let registration = authenticator.replay().unwrap();
        assert_eq!(
            rp.finish_registration(&ADMIN, registration.as_bytes())
                .unwrap_err(),
            PasskeyError::UnknownChallenge
        );

        sign_in(&rp, &mut authenticator, &passkey).unwrap();
        let assertion = authenticator.replay().unwrap();
        assert_eq!(
            rp.claim_authentication(assertion.as_bytes()).unwrap_err(),
            PasskeyError::UnknownChallenge
        );
    }

    #[test]
    fn a_failed_attempt_spends_the_challenge() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut authenticator);
        authenticator.tamper_signature(true);
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::BadSignature
        );
        let spoiled = authenticator.replay().unwrap();
        assert_eq!(
            rp.claim_authentication(spoiled.as_bytes()).unwrap_err(),
            PasskeyError::UnknownChallenge
        );
    }

    #[test]
    fn a_tampered_signature_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut authenticator);
        authenticator.tamper_signature(true);
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::BadSignature
        );
        authenticator.tamper_signature(false);
        sign_in(&rp, &mut authenticator, &passkey).unwrap();
    }

    #[test]
    fn an_unknown_passkey_is_told_apart_from_a_bad_signature() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut known = SoftAuthenticator::new(ORIGIN);
        let passkey = register(&rp, &mut known);

        // A passkey made for this origin but never stored by the hub.
        let mut stranger = SoftAuthenticator::new(ORIGIN);
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        stranger.create(&options).unwrap();

        let options = rp.start_authentication().unwrap();
        let response = stranger.get(&options).unwrap();
        let pending = rp.claim_authentication(response.as_bytes()).unwrap();
        assert_ne!(pending.credential_id(), passkey.credential_id.as_slice());
        assert_eq!(
            rp.finish_authentication(pending, None).unwrap_err(),
            PasskeyError::UnknownCredential
        );

        // Handed the wrong row anyway, it does not pass either.
        let options = rp.start_authentication().unwrap();
        let response = stranger.get(&options).unwrap();
        let pending = rp.claim_authentication(response.as_bytes()).unwrap();
        assert_eq!(
            rp.finish_authentication(pending, Some(stored(&passkey)))
                .unwrap_err(),
            PasskeyError::CredentialMismatch
        );

        known.tamper_signature(true);
        assert_eq!(
            sign_in(&rp, &mut known, &passkey).unwrap_err(),
            PasskeyError::BadSignature
        );
    }

    #[test]
    fn a_counter_that_goes_back_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let mut passkey = register(&rp, &mut authenticator);
        authenticator.set_counter(9);
        let signed = sign_in(&rp, &mut authenticator, &passkey).unwrap();
        passkey.dynamic_state = signed.updated_dynamic_state.unwrap();
        assert_eq!(counter_of(&passkey.dynamic_state), 10);
        authenticator.set_counter(3);
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::CounterWentBack
        );
    }

    #[test]
    fn a_registration_finished_by_another_admin_is_refused() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        assert_eq!(
            rp.finish_registration(&[8; USER_HANDLE_LEN], response.as_bytes())
                .unwrap_err(),
            PasskeyError::WrongUser
        );
    }

    #[test]
    fn an_expired_ceremony_is_refused() {
        let rp = RelyingParty::with_limits(ORIGIN, 4, Duration::from_millis(1)).unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            rp.finish_registration(&ADMIN, response.as_bytes())
                .unwrap_err(),
            PasskeyError::Expired
        );
    }

    #[test]
    fn the_ceremony_store_does_not_grow_past_its_capacity() {
        let rp = RelyingParty::with_limits(ORIGIN, 3, DEFAULT_TIMEOUT).unwrap();
        for _ in 0..3 {
            rp.start_authentication().unwrap();
            rp.start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
                .unwrap();
        }
        for _ in 0..50 {
            assert_eq!(rp.start_authentication().unwrap_err(), PasskeyError::Busy);
            assert_eq!(
                rp.start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
                    .unwrap_err(),
                PasskeyError::Busy
            );
        }
        assert_eq!(rp.open_ceremonies(), (3, 3));

        // Expired ceremonies make room again.
        let rp = RelyingParty::with_limits(ORIGIN, 2, Duration::from_millis(1)).unwrap();
        rp.start_authentication().unwrap();
        rp.start_authentication().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        rp.start_authentication().unwrap();
        assert_eq!(rp.open_ceremonies(), (0, 1));
    }

    #[test]
    fn only_https_or_localhost_builds_a_relying_party() {
        for refused in [
            "http://panel.example.com",
            "http://192.0.2.10:8445",
            "https://192.0.2.10",
            "https://[2001:db8::1]",
            "ftp://panel.example.com",
            "panel.example.com",
            "https://panel.example.com/path",
            "https://panel.example.com:0",
            "https://panel.example.com:08445",
            "https://panel.example.com:99999",
            "https://user@panel.example.com",
            "https://",
            "https://-panel.example.com",
            "https://panel..example.com",
            "http://localhost.example.com",
        ] {
            assert_eq!(
                RelyingParty::new(refused).unwrap_err(),
                PasskeyError::InvalidOrigin,
                "{refused}"
            );
        }
        let local = RelyingParty::new("http://localhost:8445").unwrap();
        assert_eq!(local.rp_id(), "localhost");
        let with_port = RelyingParty::new("https://Panel.Example.com:8443/").unwrap();
        assert_eq!(with_port.rp_id(), "panel.example.com");
    }

    #[test]
    fn localhost_with_a_port_accepts_only_that_port() {
        let origin = "http://localhost:8445";
        let rp = RelyingParty::new(origin).unwrap();
        let mut authenticator = SoftAuthenticator::new(origin);
        let options = rp
            .start_registration(&ADMIN, "admin", &[] as &[Vec<u8>])
            .unwrap();
        let response = authenticator.create(&options).unwrap();
        let passkey = rp.finish_registration(&ADMIN, response.as_bytes()).unwrap();
        sign_in(&rp, &mut authenticator, &passkey).unwrap();
        authenticator.answer_as("http://localhost:8446");
        assert_eq!(
            sign_in(&rp, &mut authenticator, &passkey).unwrap_err(),
            PasskeyError::WrongOrigin
        );
    }

    #[test]
    fn the_default_port_is_the_origin_without_a_port() {
        let rp = RelyingParty::new("https://panel.example.com:443").unwrap();
        let mut authenticator = SoftAuthenticator::new(ORIGIN);
        register(&rp, &mut authenticator);
    }

    #[test]
    fn errors_do_not_echo_the_response() {
        let rp = RelyingParty::new(ORIGIN).unwrap();
        let hostile =
            br#"{"id":"c2VjcmV0LXNlY3JldC1zZWNyZXQ","response":{"clientDataJSON":"<script>"}}"#;
        let error = rp.claim_authentication(hostile).unwrap_err();
        assert_eq!(error, PasskeyError::Malformed);
        let text = error.to_string();
        assert!(!text.contains("script") && !text.contains("c2Vj"));
    }
}
