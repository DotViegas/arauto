//! One attempt of the channel of a device, from the connection to the end of the session
//! (`docs/14-protocolo-do-canal.md`, sections 5.3 and 6; `docs/04-seguranca-conexao.md`, C6,
//! C7, C9 and C11).
//!
//! ```text
//!  connector  hello                      purpose channel
//!  hub        hello_ok                   10 s; another version: AR-401 / AR-402
//!  hub        state                      10 s
//!   waiting:  check -> check_ok; the connection waits for the admin; a new state, or closing
//!   approved, active:
//!  connector  present                    what is on disk, or null
//!  hub        check                      15 s; answered on this same connection
//!  hub        credential                 20 s
//!  connector  writes it durably, and only then: saved
//!  hub        session                    20 s
//!   ended:    present -> check -> check_ok -> closing with the code; never a credential
//! ```
//!
//! Every attempt reads the identity and the credential from disk, never from memory: what the
//! file holds is the credential, also after a write that could not be made durable. The
//! connector closes an attempt before it opens another one.

use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{
    ChannelState, DeclaredText, Hello, HelloOk, LeftStatus, Message, Present, ProtoRange, Purpose,
    Saved,
};
use tokio::sync::{mpsc, watch};

use crate::backoff::Ended;
use crate::catalog::{Local, Said};
use crate::link::{self, End, Link, OpenError, RecvError};
use crate::session::{self, Around, LeaveAsk, News};
use crate::writes::{self, Saving};

/// The version of the program, as `hello` declares it.
pub(crate) fn version() -> DeclaredText {
    DeclaredText::lossy(env!("CARGO_PKG_VERSION"))
}

/// How an attempt ended.
#[derive(Debug)]
pub(crate) struct Outcome {
    /// What the reconnection looks at.
    pub(crate) ended: Ended,
    /// Why it did not reach the hub, when it did not: what calls and `status` say meanwhile.
    pub(crate) problem: Option<Said>,
    /// The session began on this attempt.
    pub(crate) reached_session: bool,
    /// The hub answered a request to leave sent over this channel.
    pub(crate) left: Option<LeftStatus>,
    /// The code around it asked the attempt to stop.
    pub(crate) stopped: bool,
    /// A fault of a test made it end.
    #[cfg(feature = "testing")]
    pub(crate) faulted: Option<crate::testing::Point>,
}

impl Outcome {
    fn new() -> Self {
        Self {
            ended: Ended {
                closing: None,
                established: false,
                presented: false,
                busy: false,
            },
            problem: None,
            reached_session: false,
            left: None,
            stopped: false,
            #[cfg(feature = "testing")]
            faulted: None,
        }
    }

    fn problem(mut self, said: Said) -> Self {
        self.problem = Some(said);
        self
    }

    fn end(mut self, end: End) -> Self {
        self.ended.closing = end.closing;
        self
    }
}

/// What an attempt needs from around it.
pub(crate) struct Ctx<'a> {
    pub(crate) saving: Saving,
    pub(crate) news: &'a mpsc::UnboundedSender<News>,
    pub(crate) leave: &'a mut mpsc::Receiver<LeaveAsk>,
    pub(crate) stop: &'a mut watch::Receiver<bool>,
}

/// How a step before the session went.
enum Step<T> {
    Go(T),
    Over(Outcome),
}

/// Runs one attempt of the channel of the device of `ctx.saving.dir`.
pub(crate) async fn run(ctx: Ctx<'_>) -> Outcome {
    let mut attempt = Attempt {
        ctx,
        outcome: Outcome::new(),
    };
    let outcome = attempt.converse().await;
    #[cfg(feature = "testing")]
    {
        let device = attempt.device();
        let (closing, established) = (outcome.ended.closing, outcome.ended.established);
        attempt
            .ctx
            .saving
            .settings()
            .emit(|| crate::testing::Event::AttemptEnded {
                device,
                closing,
                established,
            });
    }
    outcome
}

struct Attempt<'a> {
    ctx: Ctx<'a>,
    outcome: Outcome,
}

impl Attempt<'_> {
    #[cfg(feature = "testing")]
    fn device(&self) -> String {
        self.ctx.saving.dir.id().to_string()
    }

    fn over(&mut self) -> Outcome {
        std::mem::replace(&mut self.outcome, Outcome::new())
    }

    /// Ends the attempt: the connection closes and the outcome says how it ended. Asked to
    /// stop, it also waits for the close to leave: the program may be about to end.
    async fn finish(&mut self, link: &Link) -> Outcome {
        link.close();
        let end = link.handle().ended().await;
        if self.outcome.stopped {
            link.handle().closed().await;
        }
        let outcome = self.over();
        outcome.end(end)
    }

    #[cfg(feature = "testing")]
    async fn faulted(&mut self, link: &Link, point: crate::testing::Point) -> Outcome {
        self.outcome.faulted = Some(point);
        self.finish(link).await
    }

    #[cfg(feature = "testing")]
    fn fails(&self, point: crate::testing::Point) -> bool {
        self.ctx.saving.settings().fails(&self.device(), point)
    }

    /// The next message within `ms`, or the end of the attempt: the hub closed, the time ran
    /// out, or the code around it asked to stop.
    async fn recv(&mut self, link: &mut Link, ms: i64) -> Step<Message> {
        let received = tokio::select! {
            received = link.recv_within(ms) => received,
            () = stopped(self.ctx.stop) => {
                self.outcome.stopped = true;
                return Step::Over(self.finish(link).await);
            }
        };
        match received {
            Ok(message) => Step::Go(message),
            Err(RecvError::Ended(end)) => Step::Over(self.over().end(end)),
            Err(RecvError::TimedOut) => Step::Over(self.finish(link).await),
        }
    }

    async fn converse(&mut self) -> Outcome {
        let dir = self.ctx.saving.dir.clone();
        let settings = std::sync::Arc::clone(&self.ctx.saving.settings);
        let (record, identity) = match (dir.record(), dir.identity()) {
            (Ok(record), Ok(identity)) => (record, identity),
            _ => return self.over().problem(Said::Local(Local::Damaged)),
        };
        // Read now, presented below: what the file holds is the credential.
        let credential = match dir.credential() {
            Ok(credential) => credential,
            Err(_) => return self.over().problem(Said::Local(Local::Damaged)),
        };
        #[cfg(feature = "testing")]
        settings.emit(|| crate::testing::Event::AttemptStarted {
            device: self.device(),
        });
        let opened = tokio::select! {
            opened = link::open(&record.address, &identity, &record.key, &settings) => opened,
            () = stopped(self.ctx.stop) => {
                self.outcome.stopped = true;
                return self.over();
            }
        };
        drop(identity);
        let mut link = match opened {
            Ok(link) => link,
            Err(error) => return self.not_opened(error),
        };
        let offered = ProtoRange::CURRENT;
        let hello = Message::Hello(Hello {
            proto: offered,
            connector: version(),
            purpose: Purpose::Channel,
        });
        if link.send(&hello).await.is_err() {
            return self.finish(&link).await;
        }
        let deadlines = settings.deadlines.clone();
        match self.recv(&mut link, deadlines.wait_hello_ok_ms).await {
            Step::Go(Message::HelloOk(answer)) if answer.fits(offered) => {
                if answer == HelloOk::Outdated {
                    tracing::warn!("the hub says this connector is outdated");
                }
            }
            // A version that was never offered, or another message: a broken hub.
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        }
        link.start_heartbeat();
        let state = match self.recv(&mut link, deadlines.wait_state_ms).await {
            Step::Go(Message::State(state)) => state,
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        };
        match state {
            ChannelState::Waiting => self.wait(link, credential).await,
            ChannelState::Approved | ChannelState::Active => self.swap(link, credential).await,
            ChannelState::Ended(_) => self.present_only(link, credential).await,
        }
    }

    fn not_opened(&mut self, error: OpenError) -> Outcome {
        let outcome = self.over();
        match error {
            OpenError::Unreachable(cause) => outcome.problem(Said::Unreachable(cause)),
            OpenError::Busy => {
                let mut outcome = outcome.problem(Said::code(ErrorCode::Ar205));
                outcome.ended.busy = true;
                outcome
            }
            OpenError::NotTheHub => outcome.problem(Said::code(ErrorCode::Ar202)),
            OpenError::NoTls => outcome.problem(Said::Local(Local::NoTls)),
            OpenError::Random => outcome.problem(Said::Local(Local::Random)),
        }
    }

    /// The device is pending: the check, then the wait for the admin (C6).
    async fn wait(
        &mut self,
        mut link: Link,
        credential: Option<arauto_protocol::secret::SealedCredential>,
    ) -> Outcome {
        let wait_check = self.ctx.saving.settings.deadlines.wait_check_ms;
        match self.recv(&mut link, wait_check).await {
            // The reader of the connection answered it.
            Step::Go(Message::Check(_)) => {}
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        }
        self.outcome.ended.established = true;
        let _ = self.ctx.news.send(News::Waiting);
        let mut leaving: Option<LeaveAsk> = None;
        let approved = loop {
            tokio::select! {
                message = link.recv() => match message {
                    Ok(Message::State(state @ (ChannelState::Approved | ChannelState::Active))) => {
                        break state;
                    }
                    // The test of a newer connection: the reader answered it.
                    Ok(Message::Check(_)) => {}
                    Ok(Message::Left(left)) if leaving.is_some() => {
                        self.outcome.left = Some(left.status);
                        if let Some(asker) = leaving.take() {
                            let _ = asker.send(Some(left.status));
                        }
                    }
                    Ok(_) => {
                        let _ = self.ctx.news.send(News::Ended);
                        return self.finish(&link).await;
                    }
                    Err(end) => {
                        let _ = self.ctx.news.send(News::Ended);
                        if let Some(asker) = leaving.take() {
                            let _ = asker.send(None);
                        }
                        return self.over().end(end);
                    }
                },
                asked = self.ctx.leave.recv(), if leaving.is_none() => {
                    if let Some(asker) = asked {
                        if link.send(&Message::Leave).await.is_ok() {
                            leaving = Some(asker);
                        } else {
                            let _ = asker.send(None);
                        }
                    }
                }
                () = stopped(self.ctx.stop) => {
                    self.outcome.stopped = true;
                    let _ = self.ctx.news.send(News::Ended);
                    return self.finish(&link).await;
                }
            }
        };
        let _ = approved;
        // Approved: no longer waiting, an attempt again.
        let _ = self.ctx.news.send(News::Ended);
        self.outcome.ended.established = false;
        self.swap(link, credential).await
    }

    /// Steps 2 to 13 for an approved or active device.
    async fn swap(
        &mut self,
        mut link: Link,
        credential: Option<arauto_protocol::secret::SealedCredential>,
    ) -> Outcome {
        let deadlines = self.ctx.saving.settings.deadlines.clone();
        #[cfg(feature = "testing")]
        if self.fails(crate::testing::Point::BeforePresent) {
            return self
                .faulted(&link, crate::testing::Point::BeforePresent)
                .await;
        }
        #[cfg(feature = "testing")]
        let hold_check = self.fails(crate::testing::Point::BeforeCheckOk);
        #[cfg(feature = "testing")]
        if hold_check {
            link.handle().hold_checks();
        }
        let present = Message::Present(Present { credential });
        let sent = link.send(&present).await;
        drop(present);
        if sent.is_err() {
            return self.finish(&link).await;
        }
        self.outcome.ended.presented = true;
        match self.recv(&mut link, deadlines.wait_check_ms).await {
            Step::Go(Message::Check(_)) => {}
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        }
        #[cfg(feature = "testing")]
        if hold_check {
            return self
                .faulted(&link, crate::testing::Point::BeforeCheckOk)
                .await;
        }
        let credential = match self.recv(&mut link, deadlines.wait_credential_ms).await {
            Step::Go(Message::Credential(credential)) => credential,
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        };
        #[cfg(feature = "testing")]
        if self.fails(crate::testing::Point::BeforeSaving) {
            return self
                .faulted(&link, crate::testing::Point::BeforeSaving)
                .await;
        }
        let saved = Saved {
            generation: credential.generation,
            digest: CredentialDigest::of(&credential.sealed),
        };
        // The write fails as on a full disk: nothing is written, nothing is "saved".
        #[cfg(feature = "testing")]
        if self.fails(crate::testing::Point::WriteCredential) {
            crate::testing::slow_disk().await;
            return self
                .faulted(&link, crate::testing::Point::WriteCredential)
                .await;
        }
        if let Err(error) = writes::save_credential(&self.ctx.saving, credential.sealed).await {
            // Nothing is "saved": the attempt ends, and the next one presents what is on disk.
            tracing::warn!(%error, "the new credential could not be written");
            return self.finish(&link).await;
        }
        #[cfg(feature = "testing")]
        if self.fails(crate::testing::Point::BeforeSaved) {
            return self
                .faulted(&link, crate::testing::Point::BeforeSaved)
                .await;
        }
        if link.send(&Message::Saved(saved)).await.is_err() {
            return self.finish(&link).await;
        }
        #[cfg(feature = "testing")]
        if self.fails(crate::testing::Point::BeforeSession) {
            return self
                .faulted(&link, crate::testing::Point::BeforeSession)
                .await;
        }
        let session = match self.recv(&mut link, deadlines.wait_session_ms).await {
            Step::Go(Message::Session(session)) => session,
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        };
        link.enter_session().await;
        self.outcome.ended.established = true;
        self.outcome.reached_session = true;
        #[cfg(feature = "testing")]
        self.ctx
            .saving
            .settings()
            .emit(|| crate::testing::Event::Session {
                device: self.device(),
            });
        let served = session::serve(
            link,
            session,
            Around {
                saving: self.ctx.saving.clone(),
                news: self.ctx.news,
                leave: self.ctx.leave,
                stop: self.ctx.stop,
            },
        )
        .await;
        let mut outcome = self.over();
        outcome.left = served.left;
        outcome.stopped = served.stopped;
        outcome.end(served.end)
    }

    /// The presentation without swap: an ended device, an unknown identity, or a connector
    /// too old. What is on disk is presented and the check answered; the hub ends with the
    /// code and never issues a credential here.
    async fn present_only(
        &mut self,
        mut link: Link,
        credential: Option<arauto_protocol::secret::SealedCredential>,
    ) -> Outcome {
        let deadlines = self.ctx.saving.settings.deadlines.clone();
        let present = Message::Present(Present { credential });
        let sent = link.send(&present).await;
        drop(present);
        if sent.is_err() {
            return self.finish(&link).await;
        }
        self.outcome.ended.presented = true;
        match self.recv(&mut link, deadlines.wait_check_ms).await {
            Step::Go(Message::Check(_)) => {}
            Step::Go(_) => return self.finish(&link).await,
            Step::Over(outcome) => return outcome,
        }
        // Only the `closing` is expected now. A credential here is never written.
        match self.recv(&mut link, deadlines.wait_credential_ms).await {
            Step::Go(_) => self.finish(&link).await,
            Step::Over(outcome) => outcome,
        }
    }
}

/// Resolves once the code around asks the attempt to stop.
pub(crate) async fn stopped(stop: &mut watch::Receiver<bool>) {
    if stop.wait_for(|stop| *stop).await.is_err() {
        // Nobody can ask any more.
        std::future::pending::<()>().await;
    }
}

/// One attempt for the tests, with the code of the leader and nothing around it.
#[cfg(feature = "testing")]
pub(crate) async fn one_for_tests(
    home: &crate::home::Home,
    device: &str,
    settings: &crate::settings::Settings,
    linger: std::time::Duration,
) -> Result<crate::testing::Attempted, crate::store::StoreError> {
    use crate::testing::Attempted;

    let id = crate::store::DeviceId::parse(device)
        .ok_or_else(|| crate::store::StoreError::NoSuchDevice(device.to_owned()))?;
    let dir = crate::store::DeviceDir::of(home, &id);
    if !dir.exists() {
        return Err(crate::store::StoreError::NoSuchDevice(device.to_owned()));
    }
    let (news, mut heard) = mpsc::unbounded_channel();
    let (_asks, mut leave) = mpsc::channel(1);
    let (stop_now, mut stop) = watch::channel(false);
    let ctx = Ctx {
        saving: Saving {
            home: home.clone(),
            dir,
            settings: std::sync::Arc::new(settings.clone()),
        },
        news: &news,
        leave: &mut leave,
        stop: &mut stop,
    };
    let running = run(ctx);
    tokio::pin!(running);
    // The session is kept for `linger`, then closed.
    let outcome = loop {
        tokio::select! {
            outcome = &mut running => break outcome,
            Some(heard) = heard.recv() => {
                if matches!(heard, News::Session(_)) {
                    let stop_now = stop_now.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(linger).await;
                        stop_now.send_replace(true);
                    });
                }
            }
        }
    };
    if let Some(point) = outcome.faulted {
        return Ok(Attempted::Faulted(point));
    }
    if outcome.reached_session {
        let closing = if outcome.stopped {
            None
        } else {
            outcome.ended.closing
        };
        return Ok(Attempted::Session(closing));
    }
    if let Some(problem) = outcome.problem {
        return Ok(Attempted::Failed(problem.to_string()));
    }
    Ok(Attempted::Closed(outcome.ended.closing))
}
