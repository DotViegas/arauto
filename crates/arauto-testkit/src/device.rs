//! A connector of high level: one device of a test, with its identity and a "disk" in memory
//! for its credential, that enrolls, goes through the channel to the session, answers the
//! 24-hour swap and makes calls (`docs/14-protocolo-do-canal.md`, sections 5 and 6).
//!
//! It behaves as the connector does, unless the test asks for a fault:
//!
//! | Fault | What the device does |
//! |---|---|
//! | [`Faults::drop_at`] | Drops the connection at that step of the swap, without a word. |
//! | [`Faults::hold_check_ok`] | Never answers the `check` of the hub. |
//! | [`Presentation::Chosen`] | Presents a credential the test chose, instead of the one on its disk. |
//! | [`SessionClient::answer_checks`] | In session, stops answering the tests of a newer connection. |
//! | [`SessionClient::skip_saved`] | In the 24-hour swap, never says "saved". |
//!
//! Two attempts at the same time are two [`Device::channel`] calls on clones of one device,
//! which share the disk. A copy of the files used by another process is [`Device::copy_files`]:
//! the same identity, and a disk of its own with what the original had at that moment.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use arauto_protocol::code::VerificationCode;
use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::message::{
    Call, CallId, CallOutcome, CallResult, ChannelState, CheckOk, Declared, DeclaredText, Enroll,
    Generation, HelloOk, Left, LeftStatus, Message, Payload, Present, Purpose, Saved, Session,
};
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use arauto_protocol::secret::{SealedCredential, Secret};
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value};
use tokio::time::Instant;

use crate::connector::{RawSocket, TestConnector, hello};
use crate::{Ended, KitError};

/// What the connector waits for each answer of the hub (`docs/14-protocolo-do-canal.md`,
/// section 4: 10 to 20 seconds); a test that measures shorter deadlines asks for less.
const ANSWER: Duration = Duration::from_secs(20);

/// The disk of a device: the credential it keeps, and nothing else.
#[derive(Default)]
pub struct Disk {
    credential: Mutex<Option<SealedCredential>>,
}

impl std::fmt::Debug for Disk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Disk").finish_non_exhaustive()
    }
}

impl Disk {
    fn held(&self) -> MutexGuard<'_, Option<SealedCredential>> {
        self.credential
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// A copy of the credential on the disk.
    #[must_use]
    pub fn credential(&self) -> Option<SealedCredential> {
        self.held()
            .as_ref()
            .and_then(|sealed| SealedCredential::from_slice(sealed.expose()).ok())
    }

    /// Writes a credential, as the connector does before it says "saved".
    pub fn store(&self, credential: SealedCredential) {
        *self.held() = Some(credential);
    }

    /// The disk was lost: no credential.
    pub fn erase(&self) {
        *self.held() = None;
    }
}

/// Where the swap of a device may be dropped on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// After `state`, before `present`.
    BeforePresent,
    /// After `check`, before `check_ok`.
    BeforeCheckOk,
    /// After `credential`, before it is written to the disk.
    BeforeSaving,
    /// After the credential is on the disk, before `saved` is sent.
    BeforeSaved,
    /// After `saved` is sent, before `session` is read.
    BeforeSession,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Step; 5] = [
        Step::BeforePresent,
        Step::BeforeCheckOk,
        Step::BeforeSaving,
        Step::BeforeSaved,
        Step::BeforeSession,
    ];
}

/// What a device presents.
#[derive(Debug, Default)]
pub enum Presentation {
    /// What is on its disk, as the connector does.
    #[default]
    Disk,
    /// This, whatever is on the disk: `None` is `null`.
    Chosen(Option<SealedCredential>),
}

/// The faults of one attempt.
#[derive(Debug, Default)]
pub struct Faults {
    /// Drop the connection at this step.
    pub drop_at: Option<Step>,
    /// Never answer the `check` of the hub; the attempt then waits for the hub to close it.
    pub hold_check_ok: bool,
    /// What to present.
    pub present: Presentation,
}

/// How an attempt of [`Device::channel`] ended.
#[derive(Debug)]
pub enum Reached {
    /// The session began.
    Session(Box<SessionClient>),
    /// The hub closed the connection: the `state` it said, if it said one, and the `closing`.
    Closed {
        /// The last `state` the hub sent.
        state: Option<ChannelState>,
        /// How the connection ended.
        ended: Ended,
    },
    /// The device dropped the connection at this step, as it was told.
    Dropped(Step),
}

impl Reached {
    /// The session, or what happened instead.
    ///
    /// # Errors
    /// [`KitError::Unexpected`] when the attempt did not reach the session.
    pub fn session(self) -> Result<SessionClient, KitError> {
        match self {
            Reached::Session(session) => Ok(*session),
            other => Err(KitError::Unexpected(format!("no session: {other:?}"))),
        }
    }

    /// The code of the `closing`, when the hub closed.
    #[must_use]
    pub fn closing(&self) -> Option<ErrorCode> {
        match self {
            Reached::Closed { ended, .. } => ended.closing,
            Reached::Session(_) | Reached::Dropped(_) => None,
        }
    }
}

/// How an enrollment ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enrolled {
    /// The hub accepted it: this is the code the Hermes says and the admin types.
    Code(VerificationCode),
    /// The hub refused it with this code.
    Refused(ErrorCode),
}

/// One device: its identity, its disk, and the hub it speaks to.
#[derive(Clone)]
pub struct Device {
    hub: SocketAddr,
    hub_key: PublicKey,
    identity: Arc<StaticKeypair>,
    disk: Arc<Disk>,
    proto: (u32, u32),
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("identity", self.identity.public())
            .finish_non_exhaustive()
    }
}

impl Device {
    /// A new device, with an identity from the generator of the system and an empty disk.
    ///
    /// # Errors
    /// [`KitError::Random`].
    pub fn new(hub: SocketAddr, hub_key: PublicKey) -> Result<Self, KitError> {
        Ok(Self::with_identity(
            hub,
            hub_key,
            StaticKeypair::generate()?,
        ))
    }

    /// A device with this identity and an empty disk.
    #[must_use]
    pub fn with_identity(hub: SocketAddr, hub_key: PublicKey, identity: StaticKeypair) -> Self {
        let range = arauto_protocol::message::ProtoRange::CURRENT;
        Self {
            hub,
            hub_key,
            identity: Arc::new(identity),
            disk: Arc::new(Disk::default()),
            proto: (range.min(), range.max()),
        }
    }

    /// The same device, speaking to another address of the hub.
    #[must_use]
    pub fn at(mut self, hub: SocketAddr) -> Self {
        self.hub = hub;
        self
    }

    /// The same device, offering other versions of the protocol in `hello`.
    #[must_use]
    pub fn speaking(mut self, proto: (u32, u32)) -> Self {
        self.proto = proto;
        self
    }

    /// The public identity.
    #[must_use]
    pub fn public(&self) -> &PublicKey {
        self.identity.public()
    }

    /// The identity, with its private part: for a test that plants it.
    #[must_use]
    pub fn identity(&self) -> &StaticKeypair {
        &self.identity
    }

    /// The disk.
    #[must_use]
    pub fn disk(&self) -> &Disk {
        &self.disk
    }

    /// A copy of the files, as another process would use them: the same identity, and a disk
    /// of its own with what this one has now.
    #[must_use]
    pub fn copy_files(&self) -> Self {
        let identity = StaticKeypair::from_private(Secret::new(*self.identity.private().expose()));
        let disk = Disk::default();
        if let Some(credential) = self.disk.credential() {
            disk.store(credential);
        }
        Self {
            hub: self.hub,
            hub_key: self.hub_key,
            identity: Arc::new(identity),
            disk: Arc::new(disk),
            proto: self.proto,
        }
    }

    async fn connect(&self) -> Result<TestConnector, KitError> {
        RawSocket::open(self.hub)
            .await?
            .handshake(&self.identity, &self.hub_key, ANSWER)
            .await
    }

    async fn hello(
        &self,
        connector: &mut TestConnector,
        purpose: Purpose,
    ) -> Result<HelloOk, KitError> {
        connector.send(&hello(self.proto, purpose)?).await?;
        match connector.recv(ANSWER).await? {
            Message::HelloOk(answer) => Ok(answer),
            other => Err(unexpected(&other)),
        }
    }

    /// Enrolls with `invite`, declaring `declared`: a short connection.
    ///
    /// # Errors
    /// [`KitError`] for what the hub should not have done.
    pub async fn enroll(&self, invite: &Invite, declared: Declared) -> Result<Enrolled, KitError> {
        let mut connector = self.connect().await?;
        if let Err(KitError::Ended(ended)) = self.hello(&mut connector, Purpose::Enroll).await {
            return Ok(Enrolled::Refused(ended.closing.unwrap_or(ErrorCode::Ar204)));
        }
        let invite = Invite::parse(&invite.reveal())
            .map_err(|_| KitError::Unexpected("the invite of the test".into()))?;
        connector
            .send(&Message::Enroll(Enroll { invite, declared }))
            .await?;
        match connector.recv(ANSWER).await {
            Ok(Message::Enrolled(enrolled)) => {
                let code = VerificationCode::derive(&enrolled.nonce, connector.handshake_hash())
                    .map_err(|_| KitError::Unexpected("no code from the nonce".into()))?;
                Ok(Enrolled::Code(code))
            }
            Ok(other) => Err(unexpected(&other)),
            Err(KitError::Ended(ended)) => {
                Ok(Enrolled::Refused(ended.closing.unwrap_or(ErrorCode::Ar204)))
            }
            Err(other) => Err(other),
        }
    }

    /// The request to leave, by a short connection.
    ///
    /// # Errors
    /// [`KitError::Ended`] when the hub closes without `left`.
    pub async fn leave(&self) -> Result<LeftStatus, KitError> {
        let mut connector = self.connect().await?;
        self.hello(&mut connector, Purpose::Leave).await?;
        connector.send(&Message::Leave).await?;
        match connector.recv(ANSWER).await? {
            Message::Left(Left { status }) => Ok(status),
            other => Err(unexpected(&other)),
        }
    }

    /// The channel, as the connector opens it: `state`, the wait for approval if pending, the
    /// presentation, the check, the swap, and the session.
    ///
    /// # Errors
    /// [`KitError`] for what the hub should not have done, or the network.
    pub async fn channel(&self, faults: Faults) -> Result<Reached, KitError> {
        self.channel_waiting(faults, ANSWER).await
    }

    /// [`Device::channel`], waiting at most `approval` for the admin while pending.
    ///
    /// # Errors
    /// As [`Device::channel`].
    pub async fn channel_waiting(
        &self,
        faults: Faults,
        approval: Duration,
    ) -> Result<Reached, KitError> {
        let mut connector = self.connect().await?;
        let mut state = None;
        let closed = |state: Option<ChannelState>, ended: Ended| Reached::Closed { state, ended };
        let answered = match self.hello(&mut connector, Purpose::Channel).await {
            Ok(answered) => answered,
            Err(KitError::Ended(ended)) => return Ok(closed(state, ended)),
            Err(other) => return Err(other),
        };
        let _ = answered;
        // The state, and the wait while pending.
        let mut within = ANSWER;
        loop {
            match connector.recv(within).await {
                Ok(Message::State(said)) => {
                    state = Some(said);
                    if said != ChannelState::Waiting {
                        break;
                    }
                    match connector.recv(ANSWER).await {
                        Ok(Message::Check(check)) => {
                            if faults.hold_check_ok {
                                return Ok(closed(state, connector.ended(ANSWER).await?));
                            }
                            connector
                                .send(&Message::CheckOk(CheckOk { nonce: check.nonce }))
                                .await?;
                        }
                        Ok(other) => return Err(unexpected(&other)),
                        Err(KitError::Ended(ended)) => return Ok(closed(state, ended)),
                        Err(other) => return Err(other),
                    }
                    within = approval;
                }
                Ok(Message::Check(check)) if state == Some(ChannelState::Waiting) => {
                    // A newer connection of the same identity tests this one.
                    connector
                        .send(&Message::CheckOk(CheckOk { nonce: check.nonce }))
                        .await?;
                }
                Ok(other) => return Err(unexpected(&other)),
                Err(KitError::Ended(ended)) => return Ok(closed(state, ended)),
                Err(other) => return Err(other),
            }
        }
        if faults.drop_at == Some(Step::BeforePresent) {
            return Ok(Reached::Dropped(Step::BeforePresent));
        }
        let credential = match faults.present {
            Presentation::Disk => self.disk.credential(),
            Presentation::Chosen(chosen) => chosen,
        };
        connector
            .send(&Message::Present(Present { credential }))
            .await?;
        let check = match connector.recv(ANSWER).await {
            Ok(Message::Check(check)) => check,
            Ok(other) => return Err(unexpected(&other)),
            Err(KitError::Ended(ended)) => return Ok(closed(state, ended)),
            Err(other) => return Err(other),
        };
        if faults.drop_at == Some(Step::BeforeCheckOk) {
            return Ok(Reached::Dropped(Step::BeforeCheckOk));
        }
        if faults.hold_check_ok {
            return Ok(closed(state, connector.ended(ANSWER).await?));
        }
        connector
            .send(&Message::CheckOk(CheckOk { nonce: check.nonce }))
            .await?;
        // An ended state hears only the `closing` from here on.
        let credential = match connector.recv(ANSWER).await {
            Ok(Message::Credential(credential)) => credential,
            Ok(other) => return Err(unexpected(&other)),
            Err(KitError::Ended(ended)) => return Ok(closed(state, ended)),
            Err(other) => return Err(other),
        };
        if faults.drop_at == Some(Step::BeforeSaving) {
            return Ok(Reached::Dropped(Step::BeforeSaving));
        }
        let saved = Saved {
            generation: credential.generation,
            digest: CredentialDigest::of(&credential.sealed),
        };
        self.disk.store(credential.sealed);
        if faults.drop_at == Some(Step::BeforeSaved) {
            return Ok(Reached::Dropped(Step::BeforeSaved));
        }
        connector.send(&Message::Saved(saved)).await?;
        if faults.drop_at == Some(Step::BeforeSession) {
            return Ok(Reached::Dropped(Step::BeforeSession));
        }
        match connector.recv(ANSWER).await {
            Ok(Message::Session(session)) => Ok(Reached::Session(Box::new(SessionClient {
                connector,
                device: self.clone(),
                session,
                next_id: 0,
                rotated: Vec::new(),
                answer_checks: true,
                skip_saved: false,
            }))),
            Ok(other) => Err(unexpected(&other)),
            Err(KitError::Ended(ended)) => Ok(closed(state, ended)),
            Err(other) => Err(other),
        }
    }
}

fn unexpected(message: &Message) -> KitError {
    KitError::Unexpected(format!("the hub sent {:?}", message.kind()))
}

/// A session of the channel, from the side of the connector.
pub struct SessionClient {
    connector: TestConnector,
    device: Device,
    /// What the hub said in `session`.
    pub session: Session,
    next_id: u64,
    /// The generations the 24-hour swaps confirmed, in order.
    pub rotated: Vec<Generation>,
    /// Whether the tests of a newer connection are answered (they are by default).
    pub answer_checks: bool,
    /// Whether the "saved" of a 24-hour swap is left unsaid.
    pub skip_saved: bool,
}

impl std::fmt::Debug for SessionClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionClient")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl SessionClient {
    /// The device of this session.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// The connector under it, for a test that sends by hand.
    pub fn connector(&mut self) -> &mut TestConnector {
        &mut self.connector
    }

    /// Sends a call with this identifier, without waiting for its result.
    ///
    /// # Errors
    /// The network, or a name that is not one.
    pub async fn send_call(&mut self, id: u64, tool: &str, args: Value) -> Result<(), KitError> {
        let id = CallId::new(id).ok_or_else(|| KitError::Unexpected("call id zero".into()))?;
        let tool = ToolName::parse(tool).map_err(|_| KitError::Unexpected("a tool name".into()))?;
        let args = match args {
            Value::Object(args) => args,
            _ => Map::new(),
        };
        self.connector
            .send(&Message::Call(Call {
                id,
                tool,
                args: Payload::new(args),
            }))
            .await
    }

    /// A call, and its result.
    ///
    /// # Errors
    /// [`KitError::Ended`] when the hub closes first.
    pub async fn call(&mut self, tool: &str, args: Value) -> Result<CallOutcome, KitError> {
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        self.send_call(id, tool, args).await?;
        let deadline = Instant::now() + ANSWER;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let result = self.next_result(left).await?;
            if result.id.get() == id {
                return Ok(result.outcome);
            }
        }
    }

    /// The next `result`, handling everything else that comes before it.
    ///
    /// # Errors
    /// [`KitError::TimedOut`]; [`KitError::Ended`].
    pub async fn next_result(&mut self, within: Duration) -> Result<CallResult, KitError> {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.pump(left).await? {
                Some(Message::Result(result)) => return Ok(result),
                Some(other) => return Err(unexpected(&other)),
                None => {}
            }
        }
    }

    /// Reads one message and handles what the connector handles by itself: the test of a
    /// newer connection and the 24-hour swap. Returns any other message.
    ///
    /// # Errors
    /// [`KitError::TimedOut`]; [`KitError::Ended`].
    pub async fn pump(&mut self, within: Duration) -> Result<Option<Message>, KitError> {
        match self.connector.recv(within).await? {
            Message::Check(check) => {
                if self.answer_checks {
                    self.connector
                        .send(&Message::CheckOk(CheckOk { nonce: check.nonce }))
                        .await?;
                }
                Ok(None)
            }
            Message::Rotate => Ok(None),
            Message::Credential(credential) => {
                if !self.skip_saved {
                    let saved = Saved {
                        generation: credential.generation,
                        digest: CredentialDigest::of(&credential.sealed),
                    };
                    self.device.disk.store(credential.sealed);
                    self.connector.send(&Message::Saved(saved)).await?;
                }
                Ok(None)
            }
            Message::Rotated(rotated) => {
                self.rotated.push(rotated.generation);
                Ok(None)
            }
            other => Ok(Some(other)),
        }
    }

    /// Keeps the session for `time`, answering the heartbeat, the tests and the swaps.
    ///
    /// # Errors
    /// [`KitError::Ended`] when the hub closes meanwhile; another message.
    pub async fn idle(&mut self, time: Duration) -> Result<(), KitError> {
        let until = Instant::now() + time;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(());
            }
            match self.pump(left).await {
                Ok(None) | Err(KitError::TimedOut) => {}
                Ok(Some(other)) => return Err(unexpected(&other)),
                Err(other) => return Err(other),
            }
        }
    }

    /// Waits for the hub to end the connection.
    ///
    /// # Errors
    /// [`KitError::TimedOut`] when the connection is still open after `within`.
    pub async fn ended(&mut self, within: Duration) -> Result<Ended, KitError> {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.pump(left).await {
                Ok(_) => {}
                Err(KitError::Ended(ended)) => return Ok(ended),
                Err(other) => return Err(other),
            }
        }
    }

    /// The request to leave over this channel.
    ///
    /// # Errors
    /// [`KitError::Ended`] when the hub closes without `left`.
    pub async fn leave(&mut self) -> Result<LeftStatus, KitError> {
        self.connector.send(&Message::Leave).await?;
        loop {
            match self.pump(ANSWER).await? {
                Some(Message::Left(Left { status })) => return Ok(status),
                Some(other) => return Err(unexpected(&other)),
                None => {}
            }
        }
    }
}

/// What a device declares in a test.
#[must_use]
pub fn declared() -> Declared {
    Declared {
        hostname: DeclaredText::lossy("test-host"),
        os: DeclaredText::lossy("test-os"),
        hermes_version: DeclaredText::lossy("0.0.0-test"),
    }
}
