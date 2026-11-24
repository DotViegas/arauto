//! The short-lived address of the file of a message (decision 0055, section 1):
//! `whatsapp_media_link` gives it, `GET` and `HEAD /v1/media/{token}` on the port of the
//! connectors serve it.
//!
//! - **The token.** 32 bytes from the generator of the system (rule 4), written in base64url
//!   without padding: 43 characters. The hub keeps only its SHA-256, with a domain of its own
//!   ([`MediaDigest`]), so that the digest of a token is never the digest of another secret.
//!   The token appears once, in the answer of the tool: it is never logged and never audited
//!   (rule 2). [`MediaToken`] has no `Display`, `Clone` or serde, and its `Debug` says
//!   `[redacted]`.
//! - **Ten minutes.** The address serves until its end, as many times as it is opened.
//! - **A read changes nothing** (rule 5). A `GET` or a `HEAD` reads the row and writes none: no
//!   count of the openings, no new end. Its log line says only the status code.
//! - **One answer for what does not serve.** A token out of its format, unknown or past its end
//!   gets one 404, byte for byte the same.
//! - **The access is read again** (rule 9). The address serves only while the Hermes it was
//!   given to still may read that chat on that number, and while the number serves calls.
//! - **The file** comes from the engine of the organization, up to 10 MB, with its media type
//!   and `Content-Disposition: attachment`.

use arauto_protocol::limits::{
    WHATSAPP_FILE_BY_ADDRESS_MAX_BYTES, WHATSAPP_MEDIA_LINK_VALIDITY_MS,
};
use arauto_protocol::secret::{RandomError, Secret};
use axum::body::Body;
use axum::http::{HeaderValue, Response, StatusCode, header};
use data_encoding::BASE64URL_NOPAD;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::channel::Hub;
use crate::core::access::Verb;
use crate::db::ResolvedOrg;
use crate::engine::EngineMessageId;
use crate::repo::{self, RepoError};
use crate::vault::OrganizationKey;

/// The prefix of a short-lived address of a file, on the port of the connectors.
pub const MEDIA_PREFIX: &str = "/v1/media/";

/// The bytes of a token.
const LEN: usize = 32;

/// The characters of a token, as they go in the address.
pub const TOKEN_TEXT_LEN: usize = 43;

/// How many addresses are served at one time. Above it: 503. Each holds up to 10 MB.
pub const MEDIA_SERVED_MAX: usize = 4;

/// What is hashed before the token.
const DOMAIN: &[u8] = b"arauto media link\0";

/// The one answer for an address that does not serve.
const GONE: &str = "Not found.\n";

/// The answer when too many addresses are served at one time.
const BUSY: &str = "The hub is busy. Try again in a moment.\n";

/// The SHA-256 of a token: what the database keeps and what an address is found by.
#[derive(Clone, Copy)]
pub struct MediaDigest([u8; 32]);

impl MediaDigest {
    /// The bytes, to bind in a query.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for MediaDigest {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl Eq for MediaDigest {}

impl std::fmt::Debug for MediaDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MediaDigest(..)")
    }
}

/// The token in a short-lived address of a file.
pub struct MediaToken(Secret<LEN>);

impl MediaToken {
    /// Draws a token from the generator of the system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn generate() -> Result<Self, RandomError> {
        Secret::random().map(Self)
    }

    /// Reads the text of an address: exactly 43 characters of `A-Z a-z 0-9 - _`, the last one
    /// canonical. Anything else is not a token.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != TOKEN_TEXT_LEN
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return None;
        }
        // The decoder refuses a last character with bits set past the 256: one text per token.
        let bytes = Zeroizing::new(BASE64URL_NOPAD.decode(text.as_bytes()).ok()?);
        Secret::from_slice(&bytes).map(Self)
    }

    /// The token as it goes in the address.
    #[must_use]
    pub fn reveal(&self) -> Zeroizing<String> {
        Zeroizing::new(BASE64URL_NOPAD.encode(self.0.expose()))
    }

    /// What the hub keeps of it.
    #[must_use]
    pub fn digest(&self) -> MediaDigest {
        let mut hasher = Sha256::new();
        hasher.update(DOMAIN);
        hasher.update(self.0.expose());
        MediaDigest(hasher.finalize().into())
    }
}

impl std::fmt::Debug for MediaToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MediaToken([redacted])")
    }
}

/// An address just given: the text of the address and its end, in seconds since 1970.
pub(crate) struct Issued {
    /// `<public address of the hub>/v1/media/<token>`. Short-lived secret: it goes only into
    /// the answer of the tool.
    pub(crate) url: Zeroizing<String>,
    /// The end of the address, in seconds since 1970.
    pub(crate) expires_at_s: i64,
}

/// Gives a new address of the file of `message`, in `chat` of `number`, to `hermes`. The caller
/// checked that the Hermes may read that chat, at this call.
pub(crate) async fn issue(
    hub: &Hub,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    link: &repo::media_link::NewLink<'_>,
) -> Result<Issued, RepoError> {
    let token = MediaToken::generate().map_err(|_| RepoError::Random)?;
    let now = hub.now_ms();
    let expires_at = now.saturating_add(WHATSAPP_MEDIA_LINK_VALIDITY_MS);
    repo::media_link::issue(
        hub.pool(),
        organization,
        key,
        link,
        &token.digest(),
        now,
        expires_at,
    )
    .await?;
    let origin = hub.public_url().origin();
    let text = token.reveal();
    // The token is a short-lived secret: the address is built once, at its exact size, in a
    // buffer that is wiped when dropped.
    let mut url = Zeroizing::new(String::with_capacity(
        origin.len() + MEDIA_PREFIX.len() + text.len(),
    ));
    url.push_str(&origin);
    url.push_str(MEDIA_PREFIX);
    url.push_str(&text);
    Ok(Issued {
        url,
        expires_at_s: expires_at.div_euclid(1_000),
    })
}

fn respond(status: StatusCode, body: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    secure_headers(headers);
    response
}

fn secure_headers(headers: &mut axum::http::HeaderMap) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
}

/// The one answer for an address that does not serve: out of its format, unknown, past its
/// end, of an access taken away, of a message without a file. Byte for byte the same.
fn gone() -> Response<Body> {
    respond(StatusCode::NOT_FOUND, GONE)
}

fn busy() -> Response<Body> {
    respond(StatusCode::SERVICE_UNAVAILABLE, BUSY)
}

/// `GET` (or `HEAD`, with `head`) of `/v1/media/{token}`: reads, never writes. The log line
/// says the status code only.
pub async fn answer(hub: &Hub, token: &str, head: bool) -> Response<Body> {
    let response = serve(hub, token, head).await;
    tracing::info!(
        status = response.status().as_u16(),
        "a media address was asked for"
    );
    response
}

/// Why an address does not serve: one answer for all of them.
enum NotServed {
    Gone,
    Busy,
}

async fn serve(hub: &Hub, token: &str, head: bool) -> Response<Body> {
    let Some(token) = MediaToken::parse(token) else {
        return gone();
    };
    let Ok(_permit) = hub.media_served().try_acquire() else {
        return busy();
    };
    match file(hub, &token.digest()).await {
        Ok(file) => {
            let size = file.data.len();
            let mut response = if head {
                Response::new(Body::empty())
            } else {
                Response::new(Body::from(file.data))
            };
            let headers = response.headers_mut();
            if let Ok(value) = HeaderValue::from_str(file.mimetype.as_str()) {
                headers.insert(header::CONTENT_TYPE, value);
            }
            headers.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
            // Only the word, never the name the other side gave the file.
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment"),
            );
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'none'; sandbox"),
            );
            secure_headers(headers);
            response
        }
        Err(NotServed::Gone) => gone(),
        Err(NotServed::Busy) => busy(),
    }
}

/// The file an address names, read against the state of now.
async fn file(hub: &Hub, digest: &MediaDigest) -> Result<crate::engine::FileContent, NotServed> {
    let now = hub.now_ms();
    let failed = |error: &dyn std::fmt::Display| {
        tracing::error!(%error, "a media address could not be read");
        NotServed::Busy
    };
    let organization = crate::db::organization_of_media_link(hub.pool(), digest, now)
        .await
        .map_err(|error| failed(&error))?
        .ok_or(NotServed::Gone)?;
    let key = hub
        .organization_key(organization)
        .await
        .map_err(|error| failed(&error))?;
    let link = repo::media_link::find(hub.pool(), organization, &key, digest, now)
        .await
        .map_err(|error| failed(&error))?
        .ok_or(NotServed::Gone)?;
    // The access of now: the Hermes still reads that chat of that number.
    let accesses = repo::access::of_hermes(hub.pool(), organization, link.hermes)
        .await
        .map_err(|error| failed(&error))?;
    let access = accesses
        .iter()
        .find(|access| access.number.id == link.number)
        .ok_or(NotServed::Gone)?;
    if !access.number.state.serves_calls() {
        return Err(NotServed::Gone);
    }
    let line = repo::access::chat_line(hub.pool(), organization, access.id, link.chat)
        .await
        .map_err(|error| failed(&error))?;
    access
        .grant
        .in_chat(line, Verb::Read)
        .map_err(|_| NotServed::Gone)?;
    let ids = repo::chat::engine_ids(hub.pool(), organization, &key, link.number, link.chat)
        .await
        .map_err(|error| failed(&error))?;
    let engine_chat = ids.first().ok_or(NotServed::Gone)?;
    let engine = hub
        .engines()
        .of(hub.pool(), hub.vault(), organization)
        .await
        .map_err(|_| NotServed::Busy)?;
    download(&engine, &access.number.session, engine_chat, &link.message).await
}

async fn download(
    engine: &crate::engine::registry::LoadedEngine,
    session: &crate::engine::SessionName,
    chat: &crate::engine::EngineChatId,
    message: &EngineMessageId,
) -> Result<crate::engine::FileContent, NotServed> {
    use crate::engine::EngineError;
    engine
        .client
        .download_media(session, chat, message, WHATSAPP_FILE_BY_ADDRESS_MAX_BYTES)
        .await
        .map_err(|error| match error {
            // No file, or the engine says the message is not there: it does not serve.
            EngineError::NotFound | EngineError::Refused | EngineError::Invalid(_) => {
                NotServed::Gone
            }
            EngineError::Unavailable(crate::engine::Cause::TooLarge) => NotServed::Gone,
            _ => NotServed::Busy,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_has_43_characters_one_text_and_does_not_print_itself() {
        let token = MediaToken::generate().unwrap();
        let text = token.reveal();
        assert_eq!(text.len(), TOKEN_TEXT_LEN);
        assert!(
            text.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
        let again = MediaToken::parse(&text).expect("the text of a token is read back");
        assert!(again.digest() == token.digest());
        assert_eq!(again.reveal().as_str(), text.as_str());
        // Two draws differ.
        assert!(MediaToken::generate().unwrap().digest() != token.digest());
        // The strict form.
        assert!(MediaToken::parse(&text[..42]).is_none(), "42 characters");
        assert!(
            MediaToken::parse(&format!("{}A", text.as_str())).is_none(),
            "44 characters"
        );
        assert!(MediaToken::parse(&format!("{}=", &text[..42])).is_none());
        assert!(MediaToken::parse(&format!("{}+", &text[..42])).is_none());
        assert!(MediaToken::parse(&format!("{}/", &text[..42])).is_none());
        assert!(MediaToken::parse(&format!("{}.", &text[..42])).is_none());
        assert!(MediaToken::parse("").is_none());
        // The last character carries 4 bits of the token and 2 of padding: one with the padding
        // set is another text for the same bytes, and is refused.
        let zero = "A".repeat(43);
        assert!(MediaToken::parse(&zero).is_some());
        assert!(MediaToken::parse(&format!("{}B", "A".repeat(42))).is_none());
        assert!(MediaToken::parse(&format!("{}E", "A".repeat(42))).is_some());
        // Nothing prints it.
        assert_eq!(format!("{token:?}"), "MediaToken([redacted])");
        assert_eq!(format!("{:?}", token.digest()), "MediaDigest(..)");
        // The digest has its own domain, pinned: SHA-256 of "arauto media link\0" and the bytes.
        let zero = MediaToken::parse(&zero).unwrap();
        let mut expected = Sha256::new();
        expected.update(b"arauto media link\0");
        expected.update([0_u8; 32]);
        assert_eq!(
            zero.digest().as_bytes().as_slice(),
            expected.finalize().as_slice()
        );
        assert_ne!(
            zero.digest().as_bytes().as_slice(),
            Sha256::digest([0_u8; 32]).as_slice(),
            "a digest of its own domain"
        );
    }

    #[test]
    fn what_does_not_serve_is_one_answer() {
        let one = gone();
        let two = gone();
        assert_eq!(one.status(), StatusCode::NOT_FOUND);
        assert_eq!(one.headers(), two.headers());
        assert_eq!(GONE, "Not found.\n");
    }
}
