//! The messages of the channel.
//!
//! Every message is a JSON object with the field `"t"`, its type
//! (`docs/14-protocolo-do-canal.md`, section 5). [`decode`] and [`encode`] are the only ways in
//! and out:
//!
//! - an unknown field is ignored, so that a newer version can add one;
//! - a repeated key is an error, at any depth, so that a message is never read in two ways;
//! - a message of unknown type becomes [`Message::Unknown`], and the caller decides: before the
//!   session it closes the connection, afterwards it ignores up to a limit;
//! - binary data travels as strict base64, and every field has its limit checked while the
//!   message is read;
//! - a message has a ceiling of values and of nesting, so that reading it never takes much more
//!   memory than the bytes that arrived;
//! - no error repeats what the other side sent.
//!
//! [`Message`] deliberately has no serde implementation of its own. The three messages that
//! carry a secret (`enroll`, `present` and `credential`) can only be written by [`encode`],
//! so no generic serializer ever puts an invite or a credential in a log or an audit record.

use core::cell::Cell;
use core::fmt;

use serde::de::{DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::code::EnrollNonce;
use crate::digest::CredentialDigest;
use crate::errors::{ErrorCode, RetryAfter};
use crate::frame::Side;
use crate::invite::Invite;
use crate::limits::{
    CHECK_NONCE_LEN, CONFIRM_WITHIN_MAX_S, CREDENTIAL_MAX_LEN, DECLARED_TEXT_MAX_CHARS,
    ENROLLMENTS_PER_REQUEST_MAX, MESSAGE_DEPTH_MAX, MESSAGE_VALUES_MAX, TOOL_LIST_MAX,
};
use crate::names::HermesName;
use crate::notice::{AckOutcome, NoticeId, NoticeMsg};
use crate::secret::{RandomError, SealedCredential, fill_random};
use crate::tools::{ToolArgError, ToolCall, ToolName};

#[cfg(test)]
mod tests;

/// The protocol version this build speaks.
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------------------------
// Field types
// ---------------------------------------------------------------------------------------------

/// The text is not a declared text: at most 128 characters, none of them a control character,
/// a line separator, or a character that reorders or hides text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "a declared text has at most 128 characters, in one line, without control or direction characters"
)]
pub struct DeclaredTextError;

/// A text a device says about itself: the machine name, the system, a version.
///
/// It is not trusted and decides nothing. It is shown as "informed by the device, not
/// verified", and whoever shows it still escapes it. Three kinds of character are refused on
/// the way in, because the text is shown in one line to the person who approves the device:
/// control characters, so that an escape sequence never reaches a terminal through this field;
/// the line and paragraph separators; and the characters that reorder or hide text
/// ([`crate::names::is_deceptive`]), so that the name shown is the name that was sent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct DeclaredText(String);

impl DeclaredText {
    /// Whether a declared text may have this character.
    fn allows(c: char) -> bool {
        !c.is_control() && !crate::names::is_deceptive(c) && !matches!(c, '\u{2028}' | '\u{2029}')
    }

    /// Checks the size and the characters.
    ///
    /// # Errors
    /// [`DeclaredTextError`] for more than 128 characters or a character that is not allowed.
    pub fn parse(text: &str) -> Result<Self, DeclaredTextError> {
        // 128 characters never take more than 512 bytes: the long texts stop here.
        if text.len() > DECLARED_TEXT_MAX_CHARS * 4
            || text.chars().count() > DECLARED_TEXT_MAX_CHARS
            || !text.chars().all(Self::allows)
        {
            return Err(DeclaredTextError);
        }
        Ok(Self(text.to_owned()))
    }

    /// Makes any text fit: the characters that are not allowed are dropped and the rest is cut
    /// at 128 characters. For what the connector reads from its own machine.
    #[must_use]
    pub fn lossy(text: &str) -> Self {
        Self(
            text.chars()
                .filter(|c| Self::allows(*c))
                .take(DECLARED_TEXT_MAX_CHARS)
                .collect(),
        )
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for DeclaredText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DeclaredText {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::serde_util::text(deserializer, "declared text", |text| Self::parse(text).ok())
    }
}

bounded_integer!(
    /// Which enrollment of a connection request this is: from 1 to 5.
    EnrollmentNumber(u8),
    1,
    ENROLLMENTS_PER_REQUEST_MAX,
    "enrollment number"
);

bounded_integer!(
    /// In how many seconds the code has to be confirmed: from 1 to 86,400.
    ConfirmWithin(u32),
    1,
    CONFIRM_WITHIN_MAX_S,
    "confirmation deadline"
);

bounded_integer!(
    /// The identifier of a call: a positive integer the connector picks, unique among the calls
    /// in progress on that connection.
    CallId(u64),
    1,
    u64::MAX,
    "call identifier"
);

bounded_integer!(
    /// The generation of a credential: a positive integer. Generation zero is the device that
    /// has not confirmed a credential yet, and never travels. The ceiling is the largest value
    /// a signed 64-bit column holds.
    Generation(u64),
    1,
    u64::MAX >> 1,
    "credential generation"
);

impl Generation {
    /// The generation as a signed integer, the way a database stores it.
    #[must_use]
    pub fn as_i64(self) -> i64 {
        i64::try_from(self.get()).unwrap_or(i64::MAX)
    }
}

/// The versions of the protocol a program speaks: the lowest and the highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "(u32, u32)", into = "(u32, u32)")]
pub struct ProtoRange {
    min: u32,
    max: u32,
}

/// The version range is not `1 <= min <= max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a protocol range is two versions, from 1, lowest first")]
pub struct ProtoRangeError;

/// What the hub concludes from the versions of a `hello`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Negotiation {
    /// There is a version in common: this is the highest one.
    Agreed(u32),
    /// The connector is too old. In `channel` the hub answers `hello_ok` with `outdated` and
    /// follows the presentation without exchange; in `enroll` it closes with AR-401; in
    /// `leave` it serves the request.
    ConnectorTooOld,
    /// The connector is too new: `closing` with AR-402.
    ConnectorTooNew,
}

impl ProtoRange {
    /// The versions this build speaks.
    pub const CURRENT: ProtoRange = ProtoRange {
        min: PROTOCOL_VERSION,
        max: PROTOCOL_VERSION,
    };

    /// A range. Returns `None` unless `1 <= min <= max`.
    #[must_use]
    pub const fn new(min: u32, max: u32) -> Option<Self> {
        if min == 0 || min > max {
            None
        } else {
            Some(Self { min, max })
        }
    }

    /// The lowest version.
    #[must_use]
    pub const fn min(self) -> u32 {
        self.min
    }

    /// The highest version.
    #[must_use]
    pub const fn max(self) -> u32 {
        self.max
    }

    /// Whether `version` is one of the versions of the range.
    #[must_use]
    pub const fn contains(self, version: u32) -> bool {
        self.min <= version && version <= self.max
    }

    /// Compares the versions of the hub (`self`) with the ones a connector sent. What decides
    /// AR-401 is the version of the protocol, never the version of the program.
    #[must_use]
    pub const fn negotiate(self, connector: ProtoRange) -> Negotiation {
        if connector.max < self.min {
            Negotiation::ConnectorTooOld
        } else if connector.min > self.max {
            Negotiation::ConnectorTooNew
        } else if connector.max < self.max {
            Negotiation::Agreed(connector.max)
        } else {
            Negotiation::Agreed(self.max)
        }
    }
}

impl TryFrom<(u32, u32)> for ProtoRange {
    type Error = ProtoRangeError;

    fn try_from((min, max): (u32, u32)) -> Result<Self, ProtoRangeError> {
        Self::new(min, max).ok_or(ProtoRangeError)
    }
}

impl From<ProtoRange> for (u32, u32) {
    fn from(range: ProtoRange) -> Self {
        (range.min, range.max)
    }
}

text_enum!(
    /// What a connection is for.
    pub enum Purpose, "purpose" {
        /// The enrollment: a short connection that presents an invite.
        Enroll => "enroll",
        /// The channel: the connection the leader keeps.
        Channel => "channel",
        /// A short connection that asks for the closing of the device.
        Leave => "leave",
    }
);

/// The nonce of `check` and `check_ok`: the proof that whoever presented a credential is
/// still on the line.
#[derive(Clone, Copy)]
pub struct CheckNonce([u8; CHECK_NONCE_LEN]);

impl CheckNonce {
    /// Draws a nonce from the generator of the operating system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn random() -> Result<Self, RandomError> {
        let mut bytes = [0u8; CHECK_NONCE_LEN];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// A nonce from its 16 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; CHECK_NONCE_LEN]) -> Self {
        Self(bytes)
    }

    /// The 16 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; CHECK_NONCE_LEN] {
        &self.0
    }
}

impl PartialEq for CheckNonce {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl Eq for CheckNonce {}

impl fmt::Debug for CheckNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CheckNonce({})", data_encoding::HEXLOWER.encode(&self.0))
    }
}

impl_base64_serde!(CheckNonce, CHECK_NONCE_LEN, "check nonce");

/// The list of released tools has more than 64 names, or the same name twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a list of tools has at most 64 names, each of them once")]
pub struct ToolListError;

/// The names of the tools released to a Hermes, as the hub sends them in `session` and `tools`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolList(Vec<ToolName>);

impl ToolList {
    /// A list of at most 64 names, each of them once.
    ///
    /// # Errors
    /// [`ToolListError`] for a longer list or a repeated name.
    pub fn new(names: Vec<ToolName>) -> Result<Self, ToolListError> {
        // The size comes first: it bounds the comparisons below.
        if names.len() > TOOL_LIST_MAX {
            return Err(ToolListError);
        }
        let repeated = names
            .iter()
            .enumerate()
            .any(|(index, name)| names.iter().take(index).any(|earlier| earlier == name));
        if repeated {
            return Err(ToolListError);
        }
        Ok(Self(names))
    }

    /// The names.
    #[must_use]
    pub fn as_slice(&self) -> &[ToolName] {
        &self.0
    }

    /// Whether `name` is in the list.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|tool| tool.as_str() == name)
    }
}

impl Serialize for ToolList {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<ToolName>::deserialize(deserializer)?;
        Self::new(names).map_err(serde::de::Error::custom)
    }
}

/// A JSON object carried by a message: the arguments of a call or the content of a result.
///
/// It is content, often written by third parties. Its `Debug` shows only how many keys it has,
/// so that it does not end up in a log.
#[derive(Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Payload(Map<String, Value>);

impl Payload {
    /// Wraps an object.
    #[must_use]
    pub const fn new(object: Map<String, Value>) -> Self {
        Self(object)
    }

    /// The object.
    #[must_use]
    pub const fn as_map(&self) -> &Map<String, Value> {
        &self.0
    }

    /// The object, taken out.
    #[must_use]
    pub fn into_map(self) -> Map<String, Value> {
        self.0
    }
}

impl From<Map<String, Value>> for Payload {
    fn from(object: Map<String, Value>) -> Self {
        Self(object)
    }
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Payload({} keys)", self.0.len())
    }
}

// ---------------------------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------------------------

/// `hello`: the first message, from the connector. Its format never changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// The lowest and the highest protocol version the connector speaks.
    pub proto: ProtoRange,
    /// The version of the program. Declared text: it is shown and decides nothing.
    pub connector: DeclaredText,
    /// What the connection is for.
    pub purpose: Purpose,
}

/// `hello_ok`: the answer of the hub to `hello`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "HelloOkWire", into = "HelloOkWire")]
pub enum HelloOk {
    /// The highest version in common, which is at least 1.
    Agreed(u32),
    /// The connector is too old. Only in `channel`: the presentation without exchange follows.
    Outdated,
}

impl HelloOk {
    /// Whether this answer fits the versions the connector `offered` in its `hello`.
    ///
    /// The hub answers with the highest version in common, so an agreed version outside the
    /// range that was offered comes from a hub that is broken or hostile: the connector closes
    /// instead of speaking a version it never offered. `outdated` fits any offer.
    #[must_use]
    pub const fn fits(self, offered: ProtoRange) -> bool {
        match self {
            HelloOk::Agreed(version) => offered.contains(version),
            HelloOk::Outdated => true,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct HelloOkWire {
    proto: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    outdated: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl TryFrom<HelloOkWire> for HelloOk {
    type Error = &'static str;

    fn try_from(wire: HelloOkWire) -> Result<Self, Self::Error> {
        match (wire.proto, wire.outdated) {
            (0, true) => Ok(HelloOk::Outdated),
            (version, false) if version >= 1 => Ok(HelloOk::Agreed(version)),
            _ => Err("protocol version and outdated flag disagree"),
        }
    }
}

impl From<HelloOk> for HelloOkWire {
    fn from(message: HelloOk) -> Self {
        match message {
            HelloOk::Agreed(proto) => Self {
                proto,
                outdated: false,
            },
            HelloOk::Outdated => Self {
                proto: 0,
                outdated: true,
            },
        }
    }
}

/// What the device says about itself in `enroll`. None of it is verified.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Declared {
    /// The name of the machine.
    pub hostname: DeclaredText,
    /// The operating system.
    pub os: DeclaredText,
    /// The version of the Hermes.
    pub hermes_version: DeclaredText,
}

/// `enroll`: the connector presents an invite. It carries a secret, so it has no serde.
#[derive(Debug, PartialEq, Eq)]
pub struct Enroll {
    /// The invite.
    pub invite: Invite,
    /// What the device declares.
    pub declared: Declared,
}

/// `enrolled`: the hub accepted the enrollment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enrolled {
    /// The nonce that, with the handshake hash, gives the verification code.
    pub nonce: EnrollNonce,
    /// In how many seconds the admin has to type the code.
    pub confirm_within_s: ConfirmWithin,
    /// Which enrollment of this request this is.
    pub enrollment: EnrollmentNumber,
}

/// `state`: what the hub knows about the device, always the first message after `hello_ok`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StateWire", into = "StateWire")]
pub enum ChannelState {
    /// Pending device: the hub sends `check` and the connection stays waiting.
    Waiting,
    /// Approved device, at generation zero: the exchange follows.
    Approved,
    /// Active device: the exchange follows.
    Active,
    /// Any other state, unknown identity or outdated connector: the presentation without
    /// exchange follows, and then `closing` with this code.
    Ended(ErrorCode),
}

text_enum!(
    enum StateName, "state" {
        Waiting => "waiting",
        Approved => "approved",
        Active => "active",
        Ended => "ended",
    }
);

#[derive(Serialize, Deserialize)]
struct StateWire {
    state: StateName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<ErrorCode>,
}

impl TryFrom<StateWire> for ChannelState {
    type Error = &'static str;

    fn try_from(wire: StateWire) -> Result<Self, Self::Error> {
        match (wire.state, wire.code) {
            (StateName::Waiting, _) => Ok(ChannelState::Waiting),
            (StateName::Approved, _) => Ok(ChannelState::Approved),
            (StateName::Active, _) => Ok(ChannelState::Active),
            (StateName::Ended, Some(code)) => Ok(ChannelState::Ended(code)),
            (StateName::Ended, None) => Err("an ended state has a code"),
        }
    }
}

impl From<ChannelState> for StateWire {
    fn from(state: ChannelState) -> Self {
        match state {
            ChannelState::Waiting => Self {
                state: StateName::Waiting,
                code: None,
            },
            ChannelState::Approved => Self {
                state: StateName::Approved,
                code: None,
            },
            ChannelState::Active => Self {
                state: StateName::Active,
                code: None,
            },
            ChannelState::Ended(code) => Self {
                state: StateName::Ended,
                code: Some(code),
            },
        }
    }
}

/// `present`: the connector shows the credential it has. It carries a secret, so it has no
/// serde.
#[derive(Debug, PartialEq, Eq)]
pub struct Present {
    /// The sealed credential, or `None` when the connector has none.
    pub credential: Option<SealedCredential>,
}

/// `check`: the hub asks whoever presented to answer on this same connection. CONTROL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// The nonce to send back.
    pub nonce: CheckNonce,
}

/// `check_ok`: the answer to `check`, with the same nonce. CONTROL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckOk {
    /// The nonce of the `check`.
    pub nonce: CheckNonce,
}

/// `credential`: the new credential, already recorded as pending. It carries a secret, so it
/// has no serde.
#[derive(Debug, PartialEq, Eq)]
pub struct Credential {
    /// The generation of the new credential.
    pub generation: Generation,
    /// The sealed block.
    pub sealed: SealedCredential,
}

/// `saved`: the connector wrote the new credential to disk, durably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// The generation that was written.
    pub generation: Generation,
    /// SHA-256 of the block received.
    pub digest: CredentialDigest,
}

/// `session`: the promotion was recorded. The session of the channel starts here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The name of this Hermes.
    pub hermes: HermesName,
    /// The released tools.
    pub tools: ToolList,
    /// Whether this Hermes may be woken up.
    pub wake: bool,
}

/// `tools`: the list of released tools changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tools {
    /// The released tools.
    pub tools: ToolList,
}

/// `call`: a tool call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Call {
    /// The identifier, which comes back in `result`.
    pub id: CallId,
    /// The name of the tool.
    pub tool: ToolName,
    /// The arguments.
    pub args: Payload,
}

impl Call {
    /// Checks the name of the tool and every argument, the way the hub does for each call.
    ///
    /// # Errors
    /// [`ToolArgError`], answered with AR-711.
    pub fn checked(&self) -> Result<ToolCall, ToolArgError> {
        ToolCall::parse(self.tool.as_str(), self.args.as_map())
    }
}

/// How a call ended.
#[derive(Debug, Clone, PartialEq)]
pub enum CallOutcome {
    /// The call ran: this is its content.
    Ok(Payload),
    /// The call was refused. Only the code travels; the phrase is in the connector.
    Refused {
        /// The code of the refusal.
        code: ErrorCode,
        /// In how many seconds trying again makes sense, when the hub knows.
        retry_after: Option<RetryAfter>,
    },
}

/// `result`: the answer to a call.
#[derive(Debug, Clone, PartialEq)]
pub struct CallResult {
    /// The identifier of the call.
    pub id: CallId,
    /// What happened.
    pub outcome: CallOutcome,
}

impl Serialize for CallResult {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            id: CallId,
            ok: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            content: Option<&'a Payload>,
            #[serde(skip_serializing_if = "Option::is_none")]
            code: Option<ErrorCode>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retry_after_s: Option<RetryAfter>,
        }
        let wire = match &self.outcome {
            CallOutcome::Ok(content) => Wire {
                id: self.id,
                ok: true,
                content: Some(content),
                code: None,
                retry_after_s: None,
            },
            CallOutcome::Refused { code, retry_after } => Wire {
                id: self.id,
                ok: false,
                content: None,
                code: Some(*code),
                retry_after_s: *retry_after,
            },
        };
        wire.serialize(serializer)
    }
}

/// `notice_ack`: what the connector did with a notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeAck {
    /// The identifier of the notice.
    pub id: NoticeId,
    /// What happened.
    pub outcome: AckOutcome,
}

/// `rotated`: the exchange of the 24 hours was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rotated {
    /// The generation that is now current.
    pub generation: Generation,
}

/// `ping`: heartbeat. CONTROL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    /// A number the `pong` sends back.
    pub n: u64,
}

/// `pong`: the answer to a `ping`. CONTROL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// The number of the `ping`.
    pub n: u64,
}

text_enum!(
    /// What the hub did with a request to leave. Any of the three lets the connector delete
    /// the folder of that identity.
    pub enum LeftStatus, "status of a request to leave" {
        /// The device was closed now.
        Closed => "closed",
        /// The device was already closed.
        AlreadyClosed => "already_closed",
        /// The hub does not know this device.
        Unknown => "unknown",
    }
);

/// `left`: the answer to `leave`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Left {
    /// What the hub did.
    pub status: LeftStatus,
}

/// `closing`: why the hub is about to close the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Closing {
    /// The reason.
    pub code: ErrorCode,
}

/// A message of the channel.
#[derive(Debug, PartialEq)]
pub enum Message {
    /// `hello`
    Hello(Hello),
    /// `hello_ok`
    HelloOk(HelloOk),
    /// `enroll`
    Enroll(Enroll),
    /// `enrolled`
    Enrolled(Enrolled),
    /// `state`
    State(ChannelState),
    /// `present`
    Present(Present),
    /// `check`
    Check(Check),
    /// `check_ok`
    CheckOk(CheckOk),
    /// `credential`
    Credential(Credential),
    /// `saved`
    Saved(Saved),
    /// `session`
    Session(Session),
    /// `tools`
    Tools(Tools),
    /// `call`
    Call(Call),
    /// `result`
    Result(CallResult),
    /// `notice`
    Notice(NoticeMsg),
    /// `notice_ack`
    NoticeAck(NoticeAck),
    /// `rotate`: the hub starts the exchange of the 24 hours.
    Rotate,
    /// `rotated`
    Rotated(Rotated),
    /// `ping`
    Ping(Ping),
    /// `pong`
    Pong(Pong),
    /// `leave`: the connector asks for the closing of its own device.
    Leave,
    /// `left`
    Left(Left),
    /// `closing`
    Closing(Closing),
    /// A well-formed message of a type this version does not know.
    Unknown,
}

impl Message {
    /// The type of the message. `None` for [`Message::Unknown`].
    #[must_use]
    pub const fn kind(&self) -> Option<MessageType> {
        Some(match self {
            Message::Hello(_) => MessageType::Hello,
            Message::HelloOk(_) => MessageType::HelloOk,
            Message::Enroll(_) => MessageType::Enroll,
            Message::Enrolled(_) => MessageType::Enrolled,
            Message::State(_) => MessageType::State,
            Message::Present(_) => MessageType::Present,
            Message::Check(_) => MessageType::Check,
            Message::CheckOk(_) => MessageType::CheckOk,
            Message::Credential(_) => MessageType::Credential,
            Message::Saved(_) => MessageType::Saved,
            Message::Session(_) => MessageType::Session,
            Message::Tools(_) => MessageType::Tools,
            Message::Call(_) => MessageType::Call,
            Message::Result(_) => MessageType::Result,
            Message::Notice(_) => MessageType::Notice,
            Message::NoticeAck(_) => MessageType::NoticeAck,
            Message::Rotate => MessageType::Rotate,
            Message::Rotated(_) => MessageType::Rotated,
            Message::Ping(_) => MessageType::Ping,
            Message::Pong(_) => MessageType::Pong,
            Message::Leave => MessageType::Leave,
            Message::Left(_) => MessageType::Left,
            Message::Closing(_) => MessageType::Closing,
            Message::Unknown => return None,
        })
    }

    /// Whether the message travels with the CONTROL flag, ahead of the data queue.
    #[must_use]
    pub const fn is_control(&self) -> bool {
        match self.kind() {
            Some(kind) => kind.is_control(),
            None => false,
        }
    }
}

/// The type of a message: the value of its field `"t"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    /// `hello`
    Hello,
    /// `hello_ok`
    HelloOk,
    /// `enroll`
    Enroll,
    /// `enrolled`
    Enrolled,
    /// `state`
    State,
    /// `present`
    Present,
    /// `check`
    Check,
    /// `check_ok`
    CheckOk,
    /// `credential`
    Credential,
    /// `saved`
    Saved,
    /// `session`
    Session,
    /// `tools`
    Tools,
    /// `call`
    Call,
    /// `result`
    Result,
    /// `notice`
    Notice,
    /// `notice_ack`
    NoticeAck,
    /// `rotate`
    Rotate,
    /// `rotated`
    Rotated,
    /// `ping`
    Ping,
    /// `pong`
    Pong,
    /// `leave`
    Leave,
    /// `left`
    Left,
    /// `closing`
    Closing,
}

impl MessageType {
    /// Every type.
    pub const ALL: &'static [MessageType] = &[
        MessageType::Hello,
        MessageType::HelloOk,
        MessageType::Enroll,
        MessageType::Enrolled,
        MessageType::State,
        MessageType::Present,
        MessageType::Check,
        MessageType::CheckOk,
        MessageType::Credential,
        MessageType::Saved,
        MessageType::Session,
        MessageType::Tools,
        MessageType::Call,
        MessageType::Result,
        MessageType::Notice,
        MessageType::NoticeAck,
        MessageType::Rotate,
        MessageType::Rotated,
        MessageType::Ping,
        MessageType::Pong,
        MessageType::Leave,
        MessageType::Left,
        MessageType::Closing,
    ];

    /// The value of `"t"`.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            MessageType::Hello => "hello",
            MessageType::HelloOk => "hello_ok",
            MessageType::Enroll => "enroll",
            MessageType::Enrolled => "enrolled",
            MessageType::State => "state",
            MessageType::Present => "present",
            MessageType::Check => "check",
            MessageType::CheckOk => "check_ok",
            MessageType::Credential => "credential",
            MessageType::Saved => "saved",
            MessageType::Session => "session",
            MessageType::Tools => "tools",
            MessageType::Call => "call",
            MessageType::Result => "result",
            MessageType::Notice => "notice",
            MessageType::NoticeAck => "notice_ack",
            MessageType::Rotate => "rotate",
            MessageType::Rotated => "rotated",
            MessageType::Ping => "ping",
            MessageType::Pong => "pong",
            MessageType::Leave => "leave",
            MessageType::Left => "left",
            MessageType::Closing => "closing",
        }
    }

    /// The type of this tag, when this version knows it.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.tag() == tag)
    }

    /// Whether messages of this type carry the CONTROL flag: the heartbeat and the check.
    #[must_use]
    pub const fn is_control(self) -> bool {
        matches!(
            self,
            MessageType::Check | MessageType::CheckOk | MessageType::Ping | MessageType::Pong
        )
    }

    /// Whether a connection that is waiting for approval may send messages of this type. A
    /// pending device only answers the check, keeps the heartbeat and asks to leave.
    #[must_use]
    pub const fn allowed_while_waiting(self) -> bool {
        matches!(
            self,
            MessageType::CheckOk | MessageType::Ping | MessageType::Pong | MessageType::Leave
        )
    }

    /// Whether `side` ever sends messages of this type. A message that comes from the wrong
    /// side is a protocol error.
    #[must_use]
    pub const fn sent_by(self, side: Side) -> bool {
        match self {
            MessageType::Ping | MessageType::Pong => true,
            MessageType::Hello
            | MessageType::Enroll
            | MessageType::Present
            | MessageType::CheckOk
            | MessageType::Saved
            | MessageType::Call
            | MessageType::NoticeAck
            | MessageType::Leave => matches!(side, Side::Connector),
            MessageType::HelloOk
            | MessageType::Enrolled
            | MessageType::State
            | MessageType::Check
            | MessageType::Credential
            | MessageType::Session
            | MessageType::Tools
            | MessageType::Result
            | MessageType::Notice
            | MessageType::Rotate
            | MessageType::Rotated
            | MessageType::Left
            | MessageType::Closing => matches!(side, Side::Hub),
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

// ---------------------------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------------------------

/// Why a message could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EncodeError {
    /// [`Message::Unknown`] only exists on the way in.
    #[error("a message of unknown type cannot be sent")]
    UnknownType,
    /// The arguments or the content have more values than a message may have. The call that
    /// produced them is answered with AR-709.
    #[error("the message has too many values")]
    TooManyValues,
    /// The arguments or the content are nested deeper than a message may be. The call that
    /// produced them is answered with AR-709.
    #[error("the message is nested too deep")]
    TooDeep,
    /// The JSON library refused the content.
    #[error("the message could not be written as JSON")]
    Json,
    /// A `hello_ok` that agrees on version zero: the other side would refuse to read it.
    #[error("a hello_ok agrees on a version from 1")]
    VersionZero,
}

/// Values a `call` or a `result` has besides its payload: the message itself, `t`, `id`, and
/// `tool` or `ok`.
const ENVELOPE_VALUES: usize = 4;

/// The values still to visit inside an array or an object.
enum Children<'a> {
    Array(core::slice::Iter<'a, Value>),
    Object(serde_json::map::Values<'a>),
}

impl<'a> Iterator for Children<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<&'a Value> {
        match self {
            Children::Array(items) => items.next(),
            Children::Object(values) => values.next(),
        }
    }
}

/// Counts the values and the levels of an object, itself included, the way the reader of the
/// other side will. It stops as soon as one of the two passes the ceiling, and it does not
/// recurse: the object may come from anywhere.
fn measure(object: &Map<String, Value>) -> (usize, usize) {
    let mut values = 1usize;
    let mut deepest = 1usize;
    let mut open = vec![Children::Object(object.values())];
    while let Some(children) = open.last_mut() {
        let Some(child) = children.next() else {
            open.pop();
            continue;
        };
        values = values.saturating_add(1);
        if values > MESSAGE_VALUES_MAX {
            break;
        }
        let inner = match child {
            Value::Array(items) => Children::Array(items.iter()),
            Value::Object(object) => Children::Object(object.values()),
            _ => continue,
        };
        open.push(inner);
        deepest = deepest.max(open.len());
        if deepest > MESSAGE_DEPTH_MAX {
            break;
        }
    }
    (values, deepest)
}

/// Refuses, before anything is written, a payload the other side would refuse to read: what
/// does not fit becomes an error of the call, never a closed connection.
fn check_payload(payload: &Payload) -> Result<(), EncodeError> {
    let (values, depth) = measure(payload.as_map());
    if values.saturating_add(ENVELOPE_VALUES) > MESSAGE_VALUES_MAX {
        return Err(EncodeError::TooManyValues);
    }
    // The payload sits one level inside the message.
    if depth.saturating_add(1) > MESSAGE_DEPTH_MAX {
        return Err(EncodeError::TooDeep);
    }
    Ok(())
}

#[derive(Serialize)]
struct Tagged<'a, T> {
    t: &'static str,
    #[serde(flatten)]
    body: &'a T,
}

#[derive(Serialize)]
struct NoFields {}

#[derive(Serialize)]
struct EnrollWire<'a> {
    invite: &'a str,
    declared: &'a Declared,
}

#[derive(Serialize)]
struct PresentWire<'a> {
    credential: Option<&'a str>,
}

#[derive(Serialize)]
struct CredentialWire<'a> {
    generation: Generation,
    sealed: &'a str,
}

fn tagged<T: Serialize>(kind: MessageType, body: &T) -> Result<Vec<u8>, EncodeError> {
    serde_json::to_vec(&Tagged {
        t: kind.tag(),
        body,
    })
    .map_err(|_| EncodeError::Json)
}

/// The base64 of a credential, in a buffer that is wiped when dropped.
fn sealed_text(sealed: &SealedCredential) -> Zeroizing<String> {
    Zeroizing::new(crate::base64::encode(sealed.expose()))
}

/// Writes a message as the bytes that go into frames. `"t"` comes first.
///
/// The size in bytes is not checked here: [`crate::frame::Splitter::split`] does it, for the
/// phase and the direction of the connection. The number of values and the nesting of the
/// arguments of a call and of the content of a result are.
///
/// # Errors
/// [`EncodeError`].
pub fn encode(message: &Message) -> Result<Vec<u8>, EncodeError> {
    match message {
        Message::Hello(body) => tagged(MessageType::Hello, body),
        Message::HelloOk(HelloOk::Agreed(0)) => Err(EncodeError::VersionZero),
        Message::HelloOk(body) => tagged(MessageType::HelloOk, body),
        Message::Enroll(body) => {
            let invite = body.invite.reveal();
            let wire = EnrollWire {
                invite: invite.as_str(),
                declared: &body.declared,
            };
            tagged(MessageType::Enroll, &wire)
        }
        Message::Enrolled(body) => tagged(MessageType::Enrolled, body),
        Message::State(body) => tagged(MessageType::State, body),
        Message::Present(body) => {
            let credential = body.credential.as_ref().map(sealed_text);
            let wire = PresentWire {
                credential: credential.as_ref().map(|text| text.as_str()),
            };
            tagged(MessageType::Present, &wire)
        }
        Message::Check(body) => tagged(MessageType::Check, body),
        Message::CheckOk(body) => tagged(MessageType::CheckOk, body),
        Message::Credential(body) => {
            let sealed = sealed_text(&body.sealed);
            let wire = CredentialWire {
                generation: body.generation,
                sealed: sealed.as_str(),
            };
            tagged(MessageType::Credential, &wire)
        }
        Message::Saved(body) => tagged(MessageType::Saved, body),
        Message::Session(body) => tagged(MessageType::Session, body),
        Message::Tools(body) => tagged(MessageType::Tools, body),
        Message::Call(body) => {
            check_payload(&body.args)?;
            tagged(MessageType::Call, body)
        }
        Message::Result(body) => {
            if let CallOutcome::Ok(content) = &body.outcome {
                check_payload(content)?;
            }
            tagged(MessageType::Result, body)
        }
        Message::Notice(body) => tagged(MessageType::Notice, body),
        Message::NoticeAck(body) => tagged(MessageType::NoticeAck, body),
        Message::Rotate => tagged(MessageType::Rotate, &NoFields {}),
        Message::Rotated(body) => tagged(MessageType::Rotated, body),
        Message::Ping(body) => tagged(MessageType::Ping, body),
        Message::Pong(body) => tagged(MessageType::Pong, body),
        Message::Leave => tagged(MessageType::Leave, &NoFields {}),
        Message::Left(body) => tagged(MessageType::Left, body),
        Message::Closing(body) => tagged(MessageType::Closing, body),
        Message::Unknown => Err(EncodeError::UnknownType),
    }
}

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

/// Why bytes are not a message. Every one of these is a protocol error: the hub closes with
/// AR-204. None repeats what was received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// Not JSON, more than one value, or not UTF-8.
    #[error("the message is not valid JSON")]
    Json,
    /// The same key twice in one object, at any depth.
    #[error("the message repeats a key")]
    RepeatedKey,
    /// More values than a message may have.
    #[error("the message has too many values")]
    TooManyValues,
    /// Objects and arrays nested deeper than a message may be.
    #[error("the message is nested too deep")]
    TooDeep,
    /// Valid JSON that is not an object.
    #[error("the message is not a JSON object")]
    NotAnObject,
    /// No `"t"`, or a `"t"` that is not a string.
    #[error("the message has no type")]
    NoType,
    /// A known type with a field missing, of the wrong type or out of its limit.
    #[error("message of type {0} with a missing or invalid field")]
    InvalidField(MessageType),
    /// The CONTROL flag of the frame does not belong to this type of message.
    #[error("message of type {0} with the wrong CONTROL flag")]
    ControlFlag(MessageType),
}

impl DecodeError {
    /// The code that goes in `closing` before the connection is closed.
    #[must_use]
    pub const fn closing_code(self) -> ErrorCode {
        ErrorCode::Ar204
    }
}

/// What the strict reader found besides the value: the reason of a refusal, which tells it
/// apart from every other error of the JSON library.
#[derive(Default)]
struct Findings {
    values: Cell<usize>,
    repeated_key: Cell<bool>,
    too_many_values: Cell<bool>,
    too_deep: Cell<bool>,
}

impl Findings {
    /// The error for a reading that failed.
    fn error(&self) -> DecodeError {
        if self.repeated_key.get() {
            DecodeError::RepeatedKey
        } else if self.too_many_values.get() {
            DecodeError::TooManyValues
        } else if self.too_deep.get() {
            DecodeError::TooDeep
        } else {
            DecodeError::Json
        }
    }
}

/// Reads one JSON value and refuses, at any depth: an object with the same key twice, more
/// values than a message may have, and more nesting than a message may have.
#[derive(Clone, Copy)]
struct Strict<'a> {
    findings: &'a Findings,
    /// How many objects and arrays are open around the value being read.
    depth: usize,
}

impl Strict<'_> {
    /// Counts one more value.
    fn count<E: serde::de::Error>(self) -> Result<(), E> {
        let values = self.findings.values.get().saturating_add(1);
        self.findings.values.set(values);
        if values > MESSAGE_VALUES_MAX {
            self.findings.too_many_values.set(true);
            return Err(E::custom("too many values"));
        }
        Ok(())
    }

    /// Counts one more array or object, and gives the reader of what is inside it.
    fn enter<E: serde::de::Error>(self) -> Result<Self, E> {
        self.count()?;
        if self.depth >= MESSAGE_DEPTH_MAX {
            self.findings.too_deep.set(true);
            return Err(E::custom("nested too deep"));
        }
        Ok(Self {
            findings: self.findings,
            depth: self.depth.saturating_add(1),
        })
    }
}

impl<'de> DeserializeSeed<'de> for Strict<'_> {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Strict<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        self.count()?;
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        self.count()?;
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        self.count()?;
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        self.count()?;
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number out of range"))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        self.count()?;
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Value, E> {
        self.count()?;
        Ok(Value::String(value))
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        self.count()?;
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let inside = self.enter()?;
        let mut items = Vec::new();
        while let Some(item) = access.next_element_seed(inside)? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let inside = self.enter()?;
        let mut object = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if object.contains_key(&key) {
                self.findings.repeated_key.set(true);
                return Err(serde::de::Error::custom("repeated key"));
            }
            let value = access.next_value_seed(inside)?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

/// Reads the bytes of one message, as the reassembler delivered them.
///
/// `control` is the CONTROL flag of the frame: it must match the type of the message. A
/// message of unknown type is accepted with either flag.
///
/// # Errors
/// [`DecodeError`]. The connection is to be closed.
pub fn decode(bytes: &[u8], control: bool) -> Result<Message, DecodeError> {
    let findings = Findings::default();
    let mut reader = serde_json::Deserializer::from_slice(bytes);
    let parsed = Strict {
        findings: &findings,
        depth: 0,
    }
    .deserialize(&mut reader)
    .and_then(|value| reader.end().map(|()| value));
    let Ok(value) = parsed else {
        return Err(findings.error());
    };
    let Value::Object(object) = value else {
        return Err(DecodeError::NotAnObject);
    };
    let Some(Value::String(tag)) = object.get("t") else {
        return Err(DecodeError::NoType);
    };
    let Some(kind) = MessageType::from_tag(tag) else {
        return Ok(Message::Unknown);
    };
    if kind.is_control() != control {
        return Err(DecodeError::ControlFlag(kind));
    }
    read_body(kind, object).ok_or(DecodeError::InvalidField(kind))
}

fn read_body(kind: MessageType, mut object: Map<String, Value>) -> Option<Message> {
    Some(match kind {
        MessageType::Hello => Message::Hello(typed(object)?),
        MessageType::HelloOk => Message::HelloOk(typed(object)?),
        MessageType::Enroll => Message::Enroll(read_enroll(&mut object)?),
        MessageType::Enrolled => Message::Enrolled(typed(object)?),
        MessageType::State => Message::State(typed(object)?),
        MessageType::Present => Message::Present(read_present(&mut object)?),
        MessageType::Check => Message::Check(typed(object)?),
        MessageType::CheckOk => Message::CheckOk(typed(object)?),
        MessageType::Credential => Message::Credential(read_credential(&mut object)?),
        MessageType::Saved => Message::Saved(typed(object)?),
        MessageType::Session => Message::Session(typed(object)?),
        MessageType::Tools => Message::Tools(typed(object)?),
        MessageType::Call => Message::Call(read_call(&mut object)?),
        MessageType::Result => Message::Result(read_result(&mut object)?),
        MessageType::Notice => Message::Notice(typed(object)?),
        MessageType::NoticeAck => Message::NoticeAck(typed(object)?),
        MessageType::Rotate => Message::Rotate,
        MessageType::Rotated => Message::Rotated(typed(object)?),
        MessageType::Ping => Message::Ping(typed(object)?),
        MessageType::Pong => Message::Pong(typed(object)?),
        MessageType::Leave => Message::Leave,
        MessageType::Left => Message::Left(typed(object)?),
        MessageType::Closing => Message::Closing(typed(object)?),
    })
}

/// Reads the fields of a message from its object. Fields the type does not have are ignored.
fn typed<T: DeserializeOwned>(object: Map<String, Value>) -> Option<T> {
    serde_json::from_value(Value::Object(object)).ok()
}

/// Takes one field and reads it. `None` when the field is absent or invalid.
fn field<T: DeserializeOwned>(object: &mut Map<String, Value>, key: &str) -> Option<T> {
    serde_json::from_value(object.remove(key)?).ok()
}

/// Takes one field that may be absent or `null`. The outer `None` is an invalid field.
fn optional_field<T: DeserializeOwned>(
    object: &mut Map<String, Value>,
    key: &str,
) -> Option<Option<T>> {
    match object.remove(key) {
        None | Some(Value::Null) => Some(None),
        Some(value) => serde_json::from_value(value).ok().map(Some),
    }
}

/// Takes a field that must be a string, into a buffer that is wiped when dropped.
fn take_text(object: &mut Map<String, Value>, key: &str) -> Option<Zeroizing<String>> {
    match object.remove(key)? {
        Value::String(text) => Some(Zeroizing::new(text)),
        _ => None,
    }
}

fn read_sealed(text: &str) -> Option<SealedCredential> {
    let bytes = crate::base64::decode(text, CREDENTIAL_MAX_LEN).ok()?;
    SealedCredential::new(bytes).ok()
}

fn read_enroll(object: &mut Map<String, Value>) -> Option<Enroll> {
    let invite = Invite::parse(&take_text(object, "invite")?).ok()?;
    // An object, never the positional form a derived reader would also take.
    let declared = match object.remove("declared")? {
        Value::Object(declared) => typed(declared)?,
        _ => return None,
    };
    Some(Enroll { invite, declared })
}

fn read_present(object: &mut Map<String, Value>) -> Option<Present> {
    // The field must be there: `null` is how a connector says it has no credential.
    match object.remove("credential")? {
        Value::Null => Some(Present { credential: None }),
        Value::String(text) => {
            let text = Zeroizing::new(text);
            Some(Present {
                credential: Some(read_sealed(&text)?),
            })
        }
        _ => None,
    }
}

fn read_credential(object: &mut Map<String, Value>) -> Option<Credential> {
    let sealed = read_sealed(&take_text(object, "sealed")?)?;
    let generation = field(object, "generation")?;
    Some(Credential { generation, sealed })
}

/// The arguments and the content may be large: they are moved out of the object that was read,
/// never copied.
fn read_call(object: &mut Map<String, Value>) -> Option<Call> {
    let Value::Object(args) = object.remove("args")? else {
        return None;
    };
    Some(Call {
        id: field(object, "id")?,
        tool: field(object, "tool")?,
        args: Payload::new(args),
    })
}

fn read_result(object: &mut Map<String, Value>) -> Option<CallResult> {
    let id = field(object, "id")?;
    let Value::Bool(ok) = object.remove("ok")? else {
        return None;
    };
    let content = match object.remove("content") {
        None | Some(Value::Null) => None,
        Some(Value::Object(content)) => Some(Payload::new(content)),
        Some(_) => return None,
    };
    let code = optional_field::<ErrorCode>(object, "code")?;
    let retry_after = optional_field::<RetryAfter>(object, "retry_after_s")?;
    // Content when the call ran, a code when it was refused, and never the two.
    let outcome = match (ok, content, code, retry_after) {
        (true, Some(content), None, None) => CallOutcome::Ok(content),
        (false, None, Some(code), retry_after) => CallOutcome::Refused { code, retry_after },
        _ => return None,
    };
    Some(CallResult { id, outcome })
}
