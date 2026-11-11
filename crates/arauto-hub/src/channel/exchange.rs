//! The channel, from `hello_ok` to the session: `purpose: channel`
//! (`docs/14-protocolo-do-canal.md`, sections 5.3 and 6; `docs/04-seguranca-conexao.md`, C6,
//! C7, C9 and C11).
//!
//! ```text
//!  1 hub       state                       always the first message
//!  2 connector present                     (not while waiting)
//!  3 hub       check        5 s            no answer: closes, without consequence
//!  4 connector check_ok
//!  5 hub       the newest connection of the identity?     no -> AR-206, without consequence
//!  6 hub       the older ones not established yet         -> AR-206
//!  7 hub       pre-judgment, only reading
//!  8 hub       would be accepted and another connection is established:
//!              check to it, 3 s     answered -> double connection (C9; while waiting, C6)
//!                                   silent   -> it closes with AR-208, this one goes on
//!  9 hub       T1                                      (repo::swap::issue)
//! 10 hub       credential
//! 11 connector saved        60 s
//! 12 hub       T2                                      (repo::swap::confirm)
//! 13 hub       session                                 (session)
//! ```
//!
//! | `state` | What follows |
//! |---|---|
//! | `waiting` | The check, then the wait for the admin. Approved: a new `state` and the swap. Denied or expired: `closing` with the code. |
//! | `approved`, `active` | The swap. |
//! | `ended` | The presentation without swap: the hub only looks (`look`, `look_unknown`) and always ends with `closing` and the code. It never issues a credential here. |
//!
//! No wait for the network holds a transaction or a connection of the database: T1 and T2
//! are short transactions of the repository, and every wait (the check, the probe, the
//! "saved", the admin) happens between them.

use std::sync::Arc;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{
    ChannelState, Check, CheckNonce, Credential, DeclaredText, Hello, HelloOk, Message,
};
use arauto_protocol::secret::SealedCredential;
use tokio::time::Instant;
use uuid::Uuid;

use super::Hub;
use super::link::{Link, LinkClosed, RecvError};
use super::registry::{Entry, Place, Registration, Status};
use super::session;
use super::settings::duration;
use crate::core::model::DeviceState;
use crate::db::{self, ResolvedOrg, SystemTask};
use crate::repo::approval::{self, DoubleWaitingOutcome};
use crate::repo::connection::{self, ChannelConnection, CloseReason};
use crate::repo::swap::{
    self, Activated, ConfirmOutcome, DoubleConnectionOutcome, IssueOutcome, Issued, LookOutcome,
    Stranger, UnknownOutcome,
};
use crate::repo::{self, RepoError};
use crate::restore::Evidence;

/// `purpose: channel`, from `hello_ok` to the end of the connection.
pub(crate) async fn run(hub: Arc<Hub>, link: Link, hello: Hello, answered: HelloOk) {
    let registration = hub.registry().register(&link);
    let mut conversation = Conversation {
        hub,
        link,
        registration,
        connector_version: hello.connector,
        proto: match answered {
            HelloOk::Agreed(version) => version,
            HelloOk::Outdated => 0,
        },
        outdated: answered == HelloOk::Outdated,
        record: None,
    };
    let reason = conversation.converse().await;
    conversation.end(reason).await;
}

/// The other connections of the same identity, after their test (step 8).
enum Probed {
    /// None is established and open.
    Nobody,
    /// One of them answered: two holders of the same files. Every established one.
    Answered(Vec<Arc<Entry>>),
    /// None answered in time. These were tested and stayed open without an answer: they
    /// are given up for dead. One that ended by itself meanwhile is not among them.
    Silent(Vec<Silent>),
}

/// An established connection that did not answer its test.
struct Silent {
    entry: Arc<Entry>,
    /// It was serving a session when it was tested.
    served: bool,
}

struct Conversation {
    hub: Arc<Hub>,
    link: Link,
    registration: Registration,
    connector_version: DeclaredText,
    /// The version agreed in `hello_ok`; zero for a connector that is too old.
    proto: u32,
    outdated: bool,
    /// The record of this connection, from the moment it answered its check as the newest
    /// connection of a known device.
    record: Option<(ResolvedOrg, ChannelConnection)>,
}

impl Conversation {
    fn close(&self, code: ErrorCode) {
        self.link.close(code);
    }

    /// The connection ended under this task: why, for the record.
    fn dropped(&self) -> CloseReason {
        if self.hub.is_stopping() {
            CloseReason::HubStopping
        } else {
            CloseReason::Dropped
        }
    }

    /// Records the end of the connection, when it was recorded at all. An ending with three
    /// credentials and no "saved" in 24 hours raises the alert there.
    async fn end(self, reason: CloseReason) {
        let Some((organization, record)) = &self.record else {
            return;
        };
        let reason = if self.hub.is_stopping() {
            CloseReason::HubStopping
        } else {
            reason
        };
        let closed = connection::close(
            self.hub.pool(),
            *organization,
            record,
            reason,
            self.hub.now_ms(),
        )
        .await;
        if let Err(error) = closed {
            tracing::error!(%error, "the end of a channel connection could not be recorded");
        }
    }

    async fn converse(&mut self) -> CloseReason {
        let identity = self.link.info().identity;
        let found = match find(&self.hub, &identity).await {
            Ok(found) => found,
            Err(error) => {
                tracing::error!(%error, "the device of a connection could not be read");
                self.close(ErrorCode::Ar205);
                return CloseReason::Dropped;
            }
        };
        if let Some((organization, device)) = &found {
            self.registration.entry().set_place(Place {
                organization: *organization,
                device: device.id,
                hermes: device.hermes_id,
            });
        }
        // Step 1: the hub always speaks first. A connector too old only presents, and hears
        // AR-401 unless its device ended for another reason.
        let state = match &found {
            None => ChannelState::Ended(ErrorCode::Ar504),
            Some((_, device)) if self.outdated => {
                ChannelState::Ended(device.ended_code().unwrap_or(ErrorCode::Ar401))
            }
            Some((_, device)) => device.channel_state(),
        };
        if self.link.send(&Message::State(state)).await.is_err() {
            return self.dropped();
        }
        let place = found.map(|(organization, device)| (organization, device.id));
        match (state, place) {
            (ChannelState::Waiting, Some((organization, device))) => {
                self.wait(organization, device).await
            }
            (ChannelState::Approved | ChannelState::Active, Some((organization, device))) => {
                self.swap(organization, device).await
            }
            (ChannelState::Ended(code), place) => self.present_only(place, code).await,
            // A state of a device is never sent without the device.
            (_, None) => {
                self.close(ErrorCode::Ar205);
                CloseReason::Protocol
            }
        }
    }

    // ------------------------------------------------------------------ steps 2 to 6

    /// Step 2: `present`, within the deadline of each answer.
    async fn present(&mut self) -> Result<Option<SealedCredential>, CloseReason> {
        let within = self.hub.settings().deadlines.connector_reply_ms;
        match self.link.recv_within(within).await {
            Ok(Message::Present(present)) => Ok(present.credential),
            Ok(_) | Err(RecvError::TimedOut) => {
                self.close(ErrorCode::Ar204);
                Err(CloseReason::Protocol)
            }
            Err(RecvError::Closed) => Err(self.dropped()),
        }
    }

    /// Steps 3 and 4: whoever presented answers on this same connection (C7, rule 3). A
    /// connection that does not answer in 5 seconds closes, and nothing is decided.
    async fn check(&mut self) -> Result<(), CloseReason> {
        let Ok(nonce) = CheckNonce::random() else {
            tracing::error!("the generator of the system failed");
            self.close(ErrorCode::Ar205);
            return Err(CloseReason::Dropped);
        };
        if self
            .link
            .send_control(&Message::Check(Check { nonce }))
            .is_err()
        {
            return Err(self.dropped());
        }
        let within = self.hub.settings().deadlines.check_reply_ms;
        match self.link.recv_within(within).await {
            Ok(Message::CheckOk(answer)) if answer.nonce == nonce => Ok(()),
            Ok(_) | Err(RecvError::TimedOut) => {
                self.close(ErrorCode::Ar204);
                Err(CloseReason::Protocol)
            }
            Err(RecvError::Closed) => Err(self.dropped()),
        }
    }

    /// Steps 5 and 6: only the newest connection of a device is judged. The answer of an
    /// attempt abandoned on the way, which arrives late, decides nothing.
    fn take_turn(&self) -> Result<(), CloseReason> {
        if !self.registration.is_newest() {
            self.close(ErrorCode::Ar206);
            return Err(CloseReason::ReplacedAttempt);
        }
        self.registration.close_older_attempts();
        Ok(())
    }

    /// Records the connection: it answered its check and is the newest of a known device.
    async fn open_record(
        &mut self,
        organization: ResolvedOrg,
        device: Uuid,
    ) -> Result<ChannelConnection, CloseReason> {
        if let Some((_, record)) = &self.record {
            return Ok(record.clone());
        }
        let opened = connection::open(
            self.hub.pool(),
            organization,
            device,
            Some(self.link.info().address),
            &self.connector_version,
            self.proto,
            self.hub.now_ms(),
        )
        .await;
        match opened {
            Ok(record) => {
                self.record = Some((organization, record.clone()));
                Ok(record)
            }
            Err(error) => {
                tracing::error!(%error, "a channel connection could not be recorded");
                self.close(ErrorCode::Ar205);
                Err(CloseReason::Dropped)
            }
        }
    }

    /// Step 8: a `check` to every other established connection of the identity, with 3
    /// seconds for all of them. A connection that is already ending is not tested: a session
    /// whose end is still being recorded is in the record, and it is nobody's to take. One
    /// that ends during its test answers nothing and is not given up either.
    async fn probe_established(&self) -> Result<Probed, CloseReason> {
        let established: Vec<Arc<Entry>> = self
            .registration
            .others()
            .into_iter()
            .filter(|other| other.status().is_established() && other.handle().is_open())
            .collect();
        if established.is_empty() {
            return Ok(Probed::Nobody);
        }
        let deadline =
            Instant::now() + duration(self.hub.settings().deadlines.old_connection_probe_ms);
        let mut answered = false;
        let mut silent = Vec::new();
        for other in &established {
            let Ok(nonce) = CheckNonce::random() else {
                tracing::error!("the generator of the system failed");
                self.close(ErrorCode::Ar205);
                return Err(CloseReason::Dropped);
            };
            let served = other.status() == Status::Session;
            let Some(waiting) = other.probe(nonce) else {
                continue;
            };
            let handle = other.handle().clone();
            let replied = tokio::time::timeout_at(deadline, async move {
                tokio::select! {
                    replied = waiting => replied.is_ok(),
                    () = handle.closed() => false,
                }
            })
            .await;
            match replied {
                Ok(true) => answered = true,
                // It ended by itself while it was tested.
                Ok(false) => {}
                Err(_) => silent.push(Silent {
                    entry: Arc::clone(other),
                    served,
                }),
            }
        }
        Ok(if answered {
            Probed::Answered(established)
        } else {
            Probed::Silent(silent)
        })
    }

    // ------------------------------------------------------------------ waiting

    /// A pending device: the check, then the wait for the admin, out of the ceiling of
    /// connections without a session and with the 90 seconds stopped.
    async fn wait(&mut self, organization: ResolvedOrg, device: Uuid) -> CloseReason {
        // The check passes ahead of the data queue: it waits until `state` left it.
        if self.link.handle().data_taken().await.is_err() {
            return self.dropped();
        }
        if let Err(reason) = self.check().await {
            return reason;
        }
        let turn = self.registration.turn().await;
        if let Err(reason) = self.take_turn() {
            return reason;
        }
        match self.probe_established().await {
            Ok(Probed::Answered(others)) => {
                // C6: a second waiting connection of the same identity, and the first one
                // answers. The identity was copied before the approval.
                return self.double_waiting(organization, device, &others).await;
            }
            Ok(Probed::Silent(others)) => {
                for other in &others {
                    other.entry.handle().close(ErrorCode::Ar208);
                }
            }
            Ok(Probed::Nobody) => {}
            Err(reason) => return reason,
        }
        if let Err(reason) = self.open_record(organization, device).await {
            return reason;
        }
        self.link.enter_waiting();
        let entry = Arc::clone(self.registration.entry());
        entry.set_status(Status::Waiting);
        drop(turn);
        loop {
            tokio::select! {
                message = self.link.recv() => match message {
                    // A late answer to a test that gave up: the reader answers the test
                    // under way itself.
                    Ok(Message::CheckOk(_)) => {}
                    Ok(Message::Leave) => {
                        return session::leave(&self.hub, &self.link, organization, device).await;
                    }
                    // The transport lets nothing else through while waiting.
                    Ok(_) => {
                        self.close(ErrorCode::Ar204);
                        return CloseReason::Protocol;
                    }
                    Err(LinkClosed) => return self.dropped(),
                },
                () = entry.poked() => {
                    let standing =
                        repo::device::standing(self.hub.pool(), organization, device).await;
                    match standing {
                        Ok(Some(standing)) => match standing.state {
                            DeviceState::Pending => {}
                            DeviceState::Approved | DeviceState::Active => {
                                // The request was approved: a new `state`, and the swap.
                                self.link.leave_waiting();
                                entry.set_status(Status::Attempt);
                                let state = Message::State(standing.channel_state());
                                if self.link.send(&state).await.is_err() {
                                    return self.dropped();
                                }
                                return self.swap(organization, device).await;
                            }
                            _ => {
                                self.close(standing.ended_code().unwrap_or(ErrorCode::Ar205));
                                return CloseReason::DeviceEnded;
                            }
                        },
                        Ok(None) => {
                            self.close(ErrorCode::Ar504);
                            return CloseReason::DeviceEnded;
                        }
                        // Read again at the next poke, or at the next sweep.
                        Err(error) => {
                            tracing::error!(%error, "the state of a waiting device could not be read");
                        }
                    }
                }
            }
        }
    }

    /// Two waiting connections of one identity, and the older answered: the request is
    /// denied, the admin is alerted, and both connections close.
    async fn double_waiting(
        &mut self,
        organization: ResolvedOrg,
        device: Uuid,
        others: &[Arc<Entry>],
    ) -> CloseReason {
        let outcome = approval::deny_double_waiting(
            self.hub.pool(),
            self.hub.mark(),
            organization,
            device,
            Some(self.link.info().address),
            self.hub.now_ms(),
        )
        .await;
        match outcome {
            Ok(DoubleWaitingOutcome::Denied) => {
                for other in others {
                    other.handle().close(ErrorCode::Ar301);
                }
                self.close(ErrorCode::Ar301);
            }
            // No longer a pending request: this attempt comes back and is told what it is.
            Ok(DoubleWaitingOutcome::NotPending) => self.close(ErrorCode::Ar205),
            Err(error) => {
                self.close(ErrorCode::Ar205);
                self.hub
                    .security_write_failed(organization, Some(device), error)
                    .await;
            }
        }
        CloseReason::DeviceEnded
    }

    // ------------------------------------------------------------------ the swap

    /// Steps 2 to 13, for an approved or active device.
    async fn swap(&mut self, organization: ResolvedOrg, device: Uuid) -> CloseReason {
        let presented = match self.present().await {
            Ok(presented) => presented,
            Err(reason) => return reason,
        };
        if let Err(reason) = self.check().await {
            return reason;
        }
        // From step 5 to the end of T1, one connection of the identity at a time.
        let turn = self.registration.turn().await;
        if let Err(reason) = self.take_turn() {
            return reason;
        }
        let record = match self.open_record(organization, device).await {
            Ok(record) => record,
            Err(reason) => return reason,
        };
        let key = match self.hub.organization_key(organization).await {
            Ok(key) => key,
            Err(error) => {
                tracing::error!(%error, "the key of an organization could not be opened");
                self.close(ErrorCode::Ar205);
                return CloseReason::Dropped;
            }
        };
        let sealed = presented.as_ref().map(SealedCredential::expose);

        // Steps 7 and 8: only a presentation that would be accepted tests the established
        // connection of the device. Without the answer of the pre-judgment, step 8 is not
        // skipped: the attempt closes, and the connector comes back.
        let would_accept =
            match swap::would_accept(self.hub.pool(), organization, &key, device, sealed).await {
                Ok(would_accept) => would_accept,
                Err(error) => {
                    tracing::error!(%error, "the pre-judgment of a presentation failed");
                    self.close(ErrorCode::Ar205);
                    return CloseReason::Dropped;
                }
            };
        if would_accept {
            match self.probe_established().await {
                Ok(Probed::Answered(others)) => {
                    return self.double_connection(organization, device, &others).await;
                }
                Ok(Probed::Silent(others)) => {
                    self.given_up_for_dead(organization, device, &others).await;
                }
                Ok(Probed::Nobody) => {}
                Err(reason) => return reason,
            }
        }

        // Step 9: T1, only while this is still the newest connection and it is still open: an
        // attempt abandoned on the way judges nothing.
        if !self.registration.is_newest() || !self.link.handle().is_open() {
            self.close(ErrorCode::Ar206);
            return CloseReason::ReplacedAttempt;
        }
        let issued = swap::issue(
            self.hub.pool(),
            self.hub.mark(),
            organization,
            &key,
            &record,
            sealed,
            self.hub.now_ms(),
        )
        .await;
        drop(turn);
        drop(presented);
        let issued = match issued {
            Ok(IssueOutcome::Issued(issued)) => issued,
            Ok(other) => return self.not_issued(other).await,
            Err(error) => return self.failed(organization, device, error).await,
        };
        self.replaced(issued.activated);
        let Issued {
            sealed, receipt, ..
        } = *issued;

        // Steps 10 and 11: the credential, then the "saved", which the connector only sends
        // once the credential is on its disk.
        let credential = Message::Credential(Credential {
            generation: receipt.credential().generation,
            sealed,
        });
        let sent = self.link.send(&credential).await;
        drop(credential);
        if sent.is_err() {
            return self.dropped();
        }
        let saved = match self
            .link
            .recv_within(self.hub.settings().deadlines.saved_ms)
            .await
        {
            Ok(Message::Saved(saved)) => saved,
            Ok(_) => {
                self.close(ErrorCode::Ar204);
                return CloseReason::Protocol;
            }
            Err(RecvError::TimedOut) => {
                self.close(ErrorCode::Ar204);
                return CloseReason::SavedTimeout;
            }
            Err(RecvError::Closed) => return self.dropped(),
        };

        // Step 12: T2. Only the pending credential issued on this very connection is
        // promoted, and only while the device is still approved or active.
        let confirmed = swap::confirm(
            self.hub.pool(),
            self.hub.mark(),
            organization,
            &record,
            &receipt,
            &saved,
            self.hub.now_ms(),
        )
        .await;
        let start = match confirmed {
            Ok(ConfirmOutcome::Confirmed { activated, session }) => {
                self.replaced(activated);
                session
            }
            Ok(ConfirmOutcome::Refused(refused)) => {
                return session::saved_refused(&self.link, refused);
            }
            Err(error) => return self.failed(organization, device, error).await,
        };

        // Step 13: the session.
        session::serve(
            Arc::clone(&self.hub),
            &mut self.link,
            Arc::clone(self.registration.entry()),
            organization,
            device,
            record,
            start,
        )
        .await
    }

    /// A first confirmation that replaced the device in use: its connections get AR-503.
    fn replaced(&self, activated: Option<Activated>) {
        if let Some(Activated {
            replaced: Some(old),
        }) = activated
        {
            self.hub.registry().close_device(old, ErrorCode::Ar503);
        }
    }

    /// C9: the established connection answered its test. Two holders of the same files: the
    /// device goes to quarantine and both connections close.
    async fn double_connection(
        &mut self,
        organization: ResolvedOrg,
        device: Uuid,
        others: &[Arc<Entry>],
    ) -> CloseReason {
        let outcome = swap::quarantine_double_connection(
            self.hub.pool(),
            self.hub.mark(),
            organization,
            device,
            Some(self.link.info().address),
            self.hub.now_ms(),
        )
        .await;
        match outcome {
            Ok(DoubleConnectionOutcome::Quarantined) => {
                for other in others {
                    other.handle().close(ErrorCode::Ar501);
                }
                self.close(ErrorCode::Ar501);
            }
            // No longer approved or active: this attempt comes back and is told its state.
            Ok(DoubleConnectionOutcome::NotServing(_)) => self.close(ErrorCode::Ar205),
            Err(error) => {
                self.close(ErrorCode::Ar205);
                self.hub
                    .security_write_failed(organization, Some(device), error)
                    .await;
            }
        }
        CloseReason::DeviceEnded
    }

    /// C9: the established connection did not answer in 3 seconds. It is given up for dead
    /// and closes with AR-208; if it was a session that served when it was tested, and it is
    /// still open now, the admin is told that the connection was taken by another, coming
    /// from this address.
    async fn given_up_for_dead(&self, organization: ResolvedOrg, device: Uuid, others: &[Silent]) {
        for other in others {
            let open = other.entry.handle().is_open();
            other.entry.handle().close(ErrorCode::Ar208);
            if other.served && open {
                let taken = connection::taken(
                    self.hub.pool(),
                    organization,
                    device,
                    Some(self.link.info().address),
                    self.hub.now_ms(),
                )
                .await;
                if let Err(error) = taken {
                    tracing::error!(%error, "the alert of a taken connection could not be written");
                }
            }
        }
    }

    /// T1 issued nothing: the outcome is the answer.
    async fn not_issued(&self, outcome: IssueOutcome) -> CloseReason {
        match outcome {
            IssueOutcome::Quarantined(_) => {
                self.close(ErrorCode::Ar501);
                CloseReason::DeviceEnded
            }
            IssueOutcome::Restoration(evidence) => {
                self.close(ErrorCode::Ar205);
                self.hub.enter_restored_mode(evidence).await;
                CloseReason::DeviceEnded
            }
            IssueOutcome::NotSwapped(not_swapped) => {
                self.close(not_swapped.code.unwrap_or(ErrorCode::Ar205));
                CloseReason::DeviceEnded
            }
            IssueOutcome::ConnectionClosed => {
                self.close(ErrorCode::Ar205);
                CloseReason::Dropped
            }
            IssueOutcome::Issued(_) | IssueOutcome::NotDue | IssueOutcome::NotASession => {
                self.close(ErrorCode::Ar205);
                CloseReason::Protocol
            }
        }
    }

    async fn failed(
        &self,
        organization: ResolvedOrg,
        device: Uuid,
        error: RepoError,
    ) -> CloseReason {
        self.close(ErrorCode::Ar205);
        self.hub
            .security_write_failed(organization, Some(device), error)
            .await;
        CloseReason::Dropped
    }

    // ------------------------------------------------------------------ without swap

    /// The presentation without swap (an ended device, an unknown identity, a connector too
    /// old): the hub looks at what is presented, never issues, and ends with `closing` and
    /// the code.
    async fn present_only(
        &mut self,
        place: Option<(ResolvedOrg, Uuid)>,
        code: ErrorCode,
    ) -> CloseReason {
        let presented = match self.present().await {
            Ok(presented) => presented,
            Err(reason) => return reason,
        };
        if let Err(reason) = self.check().await {
            return reason;
        }
        let _turn = self.registration.turn().await;
        if let Err(reason) = self.take_turn() {
            return reason;
        }
        let sealed = presented.as_ref().map(SealedCredential::expose);
        let Some((organization, device)) = place else {
            self.stranger(sealed).await;
            return CloseReason::Presented;
        };
        let record = match self.open_record(organization, device).await {
            Ok(record) => record,
            Err(reason) => return reason,
        };
        let key = match self.hub.organization_key(organization).await {
            Ok(key) => key,
            Err(error) => {
                tracing::error!(%error, "the key of an organization could not be opened");
                self.close(ErrorCode::Ar205);
                return CloseReason::Dropped;
            }
        };
        let looked = swap::look(
            self.hub.pool(),
            self.hub.mark(),
            organization,
            &key,
            &record,
            sealed,
            self.hub.now_ms(),
        )
        .await;
        match looked {
            // Reuse or an unreadable seal of an approved or active device: quarantine. A
            // connector too old still hears that it is too old.
            Ok(LookOutcome::Quarantined(_)) => self.close(if self.outdated {
                ErrorCode::Ar401
            } else {
                ErrorCode::Ar501
            }),
            Ok(LookOutcome::Restoration(evidence)) => {
                self.close(code);
                self.hub.enter_restored_mode(evidence).await;
            }
            Ok(LookOutcome::NotSwapped(not_swapped)) => {
                self.close(not_swapped.code.unwrap_or(code));
            }
            Ok(LookOutcome::ConnectionClosed) => self.close(code),
            Err(error) => {
                return self.failed(organization, device, error).await;
            }
        }
        CloseReason::Presented
    }

    /// An identity the database does not know: AR-504 whatever it presents. A valid seal of
    /// an organization lists it (C11) and, while no restored mode handled the listing, turns
    /// the mode on. Nothing is recorded for `null` or a seal that does not open.
    async fn stranger(&self, sealed: Option<&[u8]>) {
        let evidence = self.list_stranger(sealed).await;
        // The answer does not depend on what was found.
        self.close(ErrorCode::Ar504);
        if let Some(evidence) = evidence {
            self.hub.enter_restored_mode(evidence).await;
        }
    }

    /// Tries the key of each organization on the seal of an unknown identity, and lists it in
    /// the first one it opens for. Returns the proof of a restoration, when it is one.
    async fn list_stranger(&self, sealed: Option<&[u8]>) -> Option<Evidence> {
        sealed?;
        let organizations =
            match db::organizations_for_system_task(self.hub.pool(), SystemTask::UnknownSeal).await
            {
                Ok(organizations) => organizations,
                Err(error) => {
                    tracing::error!(%error, "the organizations could not be listed");
                    return None;
                }
            };
        let info = self.link.info();
        let stranger = Stranger {
            identity: &info.identity,
            presented: sealed,
            connector_version: Some(&self.connector_version),
            address: Some(info.address),
        };
        for organization in organizations {
            let key = match self.hub.organization_key(organization).await {
                Ok(key) => key,
                Err(error) => {
                    tracing::error!(%error, "the key of an organization could not be opened");
                    continue;
                }
            };
            let looked = swap::look_unknown(
                self.hub.pool(),
                organization,
                &key,
                &stranger,
                self.hub.now_ms(),
            )
            .await;
            match looked {
                Ok(UnknownOutcome::NotOurs) => {}
                Ok(UnknownOutcome::Listed { evidence, .. }) => return evidence,
                Err(error) => {
                    tracing::error!(%error, "an unknown device could not be listed");
                    return None;
                }
            }
        }
        None
    }
}

/// The device of `identity`, with its organization, through the entry function of the
/// identity.
async fn find(
    hub: &Hub,
    identity: &arauto_protocol::noise::PublicKey,
) -> Result<Option<(ResolvedOrg, crate::core::model::Device)>, RepoError> {
    let Some(organization) = db::organization_of_device(hub.pool(), identity).await? else {
        return Ok(None);
    };
    Ok(
        repo::device::by_identity(hub.pool(), organization, identity)
            .await?
            .map(|device| (organization, device)),
    )
}
