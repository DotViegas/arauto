//! The session of the channel (`docs/14-protocolo-do-canal.md`, sections 5.3 and 6;
//! `docs/04-seguranca-conexao.md`, C7 and C8; `docs/09-contrato-mcp.md`).
//!
//! It begins with `session`, sent only after T2 recorded the promotion. From then on:
//!
//! | Message | What the hub does |
//! |---|---|
//! | `call` | An identifier that is a positive integer, unique among the calls in progress (repeated: the connection closes). Up to 32 in progress; the next one gets AR-710 and the connection goes on. The device is read again at every call that runs (C8: authorization at every call), before its arguments: a device that no longer serves gets the code of its state, and the connection closes. Then the name and every argument are checked again (AR-711); a tool that is not released gets AR-701; an answer that does not fit gets AR-709. |
//! | `check_ok` | The reader of the link answers the test of a newer connection itself (step 8); one that reaches here is late, and ignored. |
//! | `saved` | Only during the 24-hour swap: T2, then `rotated`. Anything else closes (C7, rule 7). |
//! | `leave` | The device asks for its own closing: `left`, and the connection ends. |
//! | `notice_ack` | No notice is sent before the second milestone: ignored. |
//!
//! The refusal AR-710 is sent by the loop of the session itself, never by a task of its own:
//! a connector that keeps calling while 32 calls are in progress waits for its refusals to
//! leave, and the wait reaches its TCP connection. The AR-710 does not read the device: it is
//! the answer of a connection that is already doing all it may, and it does no work; the
//! sweep closes a device that no longer serves within 2 seconds.
//!
//! **The orderly stop** (`docs/14-protocolo-do-canal.md`, section 7). The session takes no
//! new call (AR-205), lets the calls in progress send their `result`, and then closes with
//! AR-205. The hub forces the close after its grace.
//!
//! **The 24-hour swap.** When 24 hours passed since the last confirmed swap of the
//! connection, the hub runs T1 as if the current credential had been presented, sends
//! `rotate` and `credential`, waits 60 seconds for `saved`, runs T2 and answers `rotated`.
//! The calls go on meanwhile. Without the `saved` in time the connection closes, and on the
//! way back the usual table holds.
//!
//! In the first milestone only `arauto_status` is released: the tools of the conversation
//! between Hermes come with the second, and the ones of WhatsApp with the third. It is
//! answered by the hub itself, without the engine, and says no secret.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::CALLS_IN_FLIGHT_MAX;
use arauto_protocol::message::{
    Call, CallId, CallOutcome, CallResult, Credential, Left, LeftStatus, Message, Payload, Rotated,
    Saved, Session, ToolList,
};
use arauto_protocol::tools::{self, Grants, ToolCall};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::{Instant, MissedTickBehavior};
use uuid::Uuid;

use super::Hub;
use super::link::{Link, LinkHandle, SendError};
use super::registry::{Entry, Status};
use super::settings::duration;
use crate::audit::AuditError;
use crate::db::ResolvedOrg;
use crate::repo::connection::{ChannelConnection, CloseReason};
use crate::repo::device::Standing;
use crate::repo::swap::{
    self, ConfirmOutcome, IssueOutcome, Issued, IssuedHere, SavedRefused, SessionStart,
};
use crate::repo::{self, RepoError};

/// What the Hermes has in the first milestone: no link and no number. The lists of pairs and
/// the matrix of access come with the second and third milestones.
const GRANTS: Grants = Grants {
    peer_any: false,
    peer_outbound: false,
    whatsapp_read: false,
    whatsapp_send: false,
    whatsapp_modify: false,
};

/// The names of the released tools, as `session` carries them.
fn released_tools() -> ToolList {
    let names = tools::released(&GRANTS)
        .map(tools::ToolSpec::tool_name)
        .collect();
    // The catalog has fewer than 64 tools, each once.
    ToolList::new(names).unwrap_or_default()
}

/// The `saved` was refused (T2): the connection closes with the reason.
pub(crate) fn saved_refused(link: &Link, refused: SavedRefused) -> CloseReason {
    match refused {
        // C7, rule 7: a "saved" that is not about the pending credential of this connection.
        SavedRefused::NotThePending => {
            link.close(ErrorCode::Ar204);
            CloseReason::SavedRefused
        }
        SavedRefused::ConnectionClosed => {
            link.close(ErrorCode::Ar205);
            CloseReason::Dropped
        }
        // Revoked, quarantined or replaced between the credential and the "saved".
        SavedRefused::DeviceEnded { code, .. } => {
            link.close(code.unwrap_or(ErrorCode::Ar205));
            CloseReason::DeviceEnded
        }
    }
}

/// The request to leave over an open channel, waiting or in session (C10): `left` with what
/// was done, and the connection ends.
pub(crate) async fn leave(
    hub: &Hub,
    link: &Link,
    organization: ResolvedOrg,
    device: Uuid,
) -> CloseReason {
    let left = repo::closing::leave(
        hub.pool(),
        hub.mark(),
        organization,
        device,
        Some(link.info().address),
        hub.now_ms(),
    )
    .await;
    match left {
        Ok(left) => {
            let _ = link
                .send(&Message::Left(Left {
                    status: left.status,
                }))
                .await;
        }
        // The closing is committed; only the mark did not follow. The alert is written
        // before the connector hears `left` and deletes its folder.
        Err(RepoError::Audit(AuditError::MarkNotAdvanced { seq, cause })) => {
            hub.security_write_failed(
                organization,
                Some(device),
                RepoError::Audit(AuditError::MarkNotAdvanced { seq, cause }),
            )
            .await;
            let _ = link
                .send(&Message::Left(Left {
                    status: LeftStatus::Closed,
                }))
                .await;
        }
        Err(error) => {
            link.close(ErrorCode::Ar205);
            hub.security_write_failed(organization, Some(device), error)
                .await;
        }
    }
    CloseReason::DeviceEnded
}

/// The identifiers of the calls in progress of one session.
type InFlight = Arc<Mutex<HashSet<u64>>>;

fn lock(in_flight: &InFlight) -> MutexGuard<'_, HashSet<u64>> {
    // A set of numbers: a panic elsewhere leaves it usable.
    in_flight.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What becomes of a call that arrives.
enum Admission {
    /// Its identifier is among the calls in progress: a protocol error.
    Repeated,
    /// 32 calls are in progress: AR-710.
    Full,
    /// It runs.
    Admitted,
}

/// The 24-hour swap under way: what T1 issued, and until when the `saved` may come.
struct Rotation {
    receipt: IssuedHere,
    deadline: Instant,
}

/// Starts the session and serves it until the connection ends.
pub(crate) async fn serve(
    hub: Arc<Hub>,
    link: &mut Link,
    entry: Arc<Entry>,
    organization: ResolvedOrg,
    device: Uuid,
    record: ChannelConnection,
    start: SessionStart,
) -> CloseReason {
    let session = Message::Session(Session {
        hermes: start.name,
        tools: released_tools(),
        wake: start.can_be_woken,
    });
    if link.enter_session(&session).await.is_err() {
        return if hub.is_stopping() {
            CloseReason::HubStopping
        } else {
            CloseReason::Dropped
        };
    }
    entry.set_status(Status::Session);
    let swapped_at = hub.now_ms();
    let mut serving = Serving {
        hub,
        link,
        entry,
        organization,
        device,
        record,
        calls: JoinSet::new(),
        in_flight: InFlight::default(),
        rotation: None,
        swapped_at,
        draining: false,
    };
    let reason = serving.run().await;
    // The calls still running end with the connection.
    serving.calls.abort_all();
    reason
}

struct Serving<'a> {
    hub: Arc<Hub>,
    link: &'a mut Link,
    entry: Arc<Entry>,
    organization: ResolvedOrg,
    device: Uuid,
    record: ChannelConnection,
    /// Each call runs in its own task.
    calls: JoinSet<()>,
    /// The identifiers of the calls in progress. A task takes its own out before it sends its
    /// `result`: once the connector has the answer, the identifier is free again, whether or
    /// not this loop has looked at the task.
    in_flight: InFlight,
    rotation: Option<Rotation>,
    /// The time of the hub of the last confirmed swap of this connection.
    swapped_at: i64,
    /// The hub is stopping: no new call, and the session closes once the calls in progress
    /// answered.
    draining: bool,
}

impl Serving<'_> {
    fn close(&self, code: ErrorCode) {
        self.link.close(code);
    }

    fn dropped(&self) -> CloseReason {
        if self.hub.is_stopping() {
            CloseReason::HubStopping
        } else {
            CloseReason::Dropped
        }
    }

    async fn run(&mut self) -> CloseReason {
        // The 24-hour swap is looked at as often as the hub sweeps: the clock of the hub may
        // move faster than this process sleeps.
        let every = duration(self.hub.settings().sweep_interval_ms.max(1));
        let mut tick = tokio::time::interval(every);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut stopping = self.hub.stopping();
        loop {
            // The calls that ended are let go of before anything else.
            while self.calls.try_join_next().is_some() {}
            if self.draining && self.calls.is_empty() {
                // Every call in progress when the stop began has answered.
                self.close(ErrorCode::Ar205);
                return CloseReason::HubStopping;
            }
            let saved_by = self.rotation.as_ref().map(|rotation| rotation.deadline);
            tokio::select! {
                biased;
                message = self.link.recv() => match message {
                    Ok(message) => {
                        if let Some(reason) = self.on_message(message).await {
                            return reason;
                        }
                    }
                    Err(_) => return self.dropped(),
                },
                Some(_) = self.calls.join_next(), if !self.calls.is_empty() => {}
                () = stopped(&mut stopping), if !self.draining => {
                    self.draining = true;
                }
                () = self.entry.poked() => {
                    if let Some(reason) = self.read_again().await {
                        return reason;
                    }
                }
                () = sleep_until(saved_by), if saved_by.is_some() => {
                    // The "saved" of the 24-hour swap did not come in time.
                    self.close(ErrorCode::Ar204);
                    return CloseReason::SavedTimeout;
                }
                _ = tick.tick(), if !self.draining => {
                    if let Some(reason) = self.rotate_when_due().await {
                        return reason;
                    }
                }
            }
        }
    }

    async fn on_message(&mut self, message: Message) -> Option<CloseReason> {
        match message {
            Message::Call(call) => self.on_call(call).await,
            // The reader answers the test under way; this one came too late for it.
            Message::CheckOk(_) => None,
            Message::Saved(saved) => self.on_saved(saved).await,
            Message::Leave => {
                // Out of the sessions the sweep reads: the device is about to be closed by its
                // own request, and `left` must reach the connector before any `closing`.
                self.entry.set_status(Status::Attempt);
                Some(leave(&self.hub, self.link, self.organization, self.device).await)
            }
            Message::NoticeAck(_) => None,
            // Anything else the connector sends belongs before the session.
            _ => {
                self.close(ErrorCode::Ar204);
                Some(CloseReason::Protocol)
            }
        }
    }

    async fn on_call(&mut self, call: Call) -> Option<CloseReason> {
        let id = call.id.get();
        let admission = {
            let mut in_flight = lock(&self.in_flight);
            if in_flight.contains(&id) {
                Admission::Repeated
            } else if self.draining || in_flight.len() >= CALLS_IN_FLIGHT_MAX {
                Admission::Full
            } else {
                in_flight.insert(id);
                Admission::Admitted
            }
        };
        match admission {
            // An identifier repeated among the calls in progress is a protocol error.
            Admission::Repeated => {
                self.close(ErrorCode::Ar204);
                Some(CloseReason::Protocol)
            }
            // Refused without closing, by this loop: the next message is read only once the
            // refusal has room to leave.
            Admission::Full => {
                let code = if self.draining {
                    ErrorCode::Ar205
                } else {
                    ErrorCode::Ar710
                };
                match self.link.send(&refused(call.id, code)).await {
                    Err(SendError::Closed) => Some(self.dropped()),
                    Ok(())
                    | Err(SendError::TooLarge | SendError::Encode(_) | SendError::Misuse) => None,
                }
            }
            Admission::Admitted => {
                let hub = Arc::clone(&self.hub);
                let link = self.link.handle();
                let in_flight = Arc::clone(&self.in_flight);
                let (organization, device) = (self.organization, self.device);
                self.calls.spawn(async move {
                    answer(&hub, &link, &in_flight, organization, device, call).await;
                });
                None
            }
        }
    }

    /// The device of this session, read again: what is no longer active closes with its code.
    async fn read_again(&self) -> Option<CloseReason> {
        match repo::device::standing(self.hub.pool(), self.organization, self.device).await {
            Ok(Some(standing)) if standing.may_serve() => None,
            Ok(standing) => {
                self.close(ended_code(standing.as_ref()));
                Some(CloseReason::DeviceEnded)
            }
            // The sweep reads it again in 2 seconds.
            Err(error) => {
                tracing::error!(%error, "the state of a device in session could not be read");
                None
            }
        }
    }

    /// T1 of the 24-hour swap, when it is due.
    async fn rotate_when_due(&mut self) -> Option<CloseReason> {
        if self.rotation.is_some() {
            return None;
        }
        let now = self.hub.now_ms();
        let interval = self.hub.settings().deadlines.rotation_interval_ms;
        if now.saturating_sub(self.swapped_at) < interval {
            return None;
        }
        let key = match self.hub.organization_key(self.organization).await {
            Ok(key) => key,
            Err(error) => {
                tracing::error!(%error, "the key of an organization could not be opened");
                return None;
            }
        };
        let rotated = swap::rotate(
            self.hub.pool(),
            self.hub.mark(),
            self.organization,
            &key,
            &self.record,
            now,
        )
        .await;
        match rotated {
            Ok(IssueOutcome::Issued(issued)) => {
                let Issued {
                    sealed, receipt, ..
                } = *issued;
                if self.link.send(&Message::Rotate).await.is_err() {
                    return Some(self.dropped());
                }
                let credential = Message::Credential(Credential {
                    generation: receipt.credential().generation,
                    sealed,
                });
                let sent = self.link.send(&credential).await;
                drop(credential);
                if sent.is_err() {
                    return Some(self.dropped());
                }
                let saved_ms = self.hub.settings().deadlines.saved_ms;
                self.rotation = Some(Rotation {
                    receipt,
                    deadline: Instant::now() + duration(saved_ms),
                });
                None
            }
            // A pending credential still waits for its "saved": nothing new is issued.
            Ok(IssueOutcome::NotDue) => {
                self.swapped_at = now;
                None
            }
            Ok(IssueOutcome::Quarantined(_)) => {
                self.close(ErrorCode::Ar501);
                Some(CloseReason::DeviceEnded)
            }
            Ok(IssueOutcome::Restoration(evidence)) => {
                self.close(ErrorCode::Ar205);
                self.hub.enter_restored_mode(evidence).await;
                Some(CloseReason::DeviceEnded)
            }
            Ok(IssueOutcome::NotSwapped(not_swapped)) => {
                self.close(not_swapped.code.unwrap_or(ErrorCode::Ar205));
                Some(CloseReason::DeviceEnded)
            }
            Ok(IssueOutcome::ConnectionClosed | IssueOutcome::NotASession) => {
                self.close(ErrorCode::Ar205);
                Some(CloseReason::Dropped)
            }
            Err(error) => {
                self.close(ErrorCode::Ar205);
                self.hub
                    .security_write_failed(self.organization, Some(self.device), error)
                    .await;
                Some(CloseReason::Dropped)
            }
        }
    }

    /// T2 of the 24-hour swap. A `saved` without a swap under way closes.
    async fn on_saved(&mut self, saved: Saved) -> Option<CloseReason> {
        let Some(rotation) = self.rotation.take() else {
            self.close(ErrorCode::Ar204);
            return Some(CloseReason::Protocol);
        };
        let now = self.hub.now_ms();
        let confirmed = swap::confirm(
            self.hub.pool(),
            self.hub.mark(),
            self.organization,
            &self.record,
            &rotation.receipt,
            &saved,
            now,
        )
        .await;
        match confirmed {
            Ok(ConfirmOutcome::Confirmed { .. }) => {
                self.swapped_at = now;
                let rotated = Message::Rotated(Rotated {
                    generation: rotation.receipt.credential().generation,
                });
                if self.link.send(&rotated).await.is_err() {
                    return Some(self.dropped());
                }
                None
            }
            Ok(ConfirmOutcome::Refused(refused)) => Some(saved_refused(self.link, refused)),
            Err(error) => {
                self.close(ErrorCode::Ar205);
                self.hub
                    .security_write_failed(self.organization, Some(self.device), error)
                    .await;
                Some(CloseReason::Dropped)
            }
        }
    }
}

/// Resolves once the hub is stopping.
async fn stopped(stopping: &mut watch::Receiver<bool>) {
    if stopping.wait_for(|stopping| *stopping).await.is_err() {
        // The hub is gone: nothing will ever stop it.
        std::future::pending::<()>().await;
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// The code of a device that no longer serves: the one of its state, or AR-504 when the hub
/// no longer finds it.
fn ended_code(standing: Option<&Standing>) -> ErrorCode {
    standing
        .and_then(Standing::ended_code)
        .unwrap_or(ErrorCode::Ar504)
}

fn refused(id: CallId, code: ErrorCode) -> Message {
    Message::Result(CallResult {
        id,
        outcome: CallOutcome::Refused {
            code,
            retry_after: None,
        },
    })
}

/// How a call ended, and whether the connection closes after the answer.
struct Answer {
    outcome: CallOutcome,
    close: Option<ErrorCode>,
}

impl From<CallOutcome> for Answer {
    fn from(outcome: CallOutcome) -> Self {
        Self {
            outcome,
            close: None,
        }
    }
}

fn refusal(code: ErrorCode) -> CallOutcome {
    CallOutcome::Refused {
        code,
        retry_after: None,
    }
}

/// Runs one call and sends its `result`.
async fn answer(
    hub: &Hub,
    link: &LinkHandle,
    in_flight: &InFlight,
    organization: ResolvedOrg,
    device: Uuid,
    call: Call,
) {
    let id = call.id;
    let answered = run_call(hub, organization, device, &call).await;
    drop(call);
    // Free before the connector can have the answer.
    lock(in_flight).remove(&id.get());
    let result = Message::Result(CallResult {
        id,
        outcome: answered.outcome,
    });
    match link.send(&result).await {
        // An answer that does not fit is the error of the call, never a closed connection.
        Err(SendError::TooLarge | SendError::Encode(_)) => {
            let _ = link.send(&refused(id, ErrorCode::Ar709)).await;
        }
        Ok(()) | Err(SendError::Misuse | SendError::Closed) => {}
    }
    if let Some(code) = answered.close {
        link.close(code);
    }
}

async fn run_call(hub: &Hub, organization: ResolvedOrg, device: Uuid, call: &Call) -> Answer {
    // C8: the state of the device is read at every call, before anything else. A device that
    // no longer serves gets its code, whatever it asked, and the connection closes after it.
    let standing = match repo::device::standing(hub.pool(), organization, device).await {
        Ok(Some(standing)) if standing.may_serve() => standing,
        Ok(standing) => {
            let code = ended_code(standing.as_ref());
            return Answer {
                outcome: refusal(code),
                close: Some(code),
            };
        }
        Err(error) => {
            tracing::error!(%error, "the state of a device could not be read for a call");
            return refusal(ErrorCode::Ar205).into();
        }
    };
    // The connector may be hostile: the name and every argument are checked again.
    let checked = match call.checked() {
        Ok(checked) => checked,
        Err(error) => return refusal(error.code()).into(),
    };
    if !checked.family().released_by(&GRANTS) {
        return refusal(ErrorCode::Ar701).into();
    }
    match checked {
        ToolCall::ArautoStatus => status(hub, organization, &standing).await.into(),
        // No grant of this milestone releases them, so the check above answered them with
        // AR-701. Reaching here would be a tool the hub does not serve yet.
        ToolCall::HermesListPeers
        | ToolCall::HermesSendMessage(_)
        | ToolCall::HermesListThreads(_)
        | ToolCall::HermesReadThread(_) => refusal(ErrorCode::Ar205).into(),
    }
}

/// `arauto_status`, answered by the hub without the engine. No secret: the name of the
/// Hermes, whether it can be woken, its numbers and verbs (none before the third milestone)
/// and whether it has links with other Hermes.
async fn status(hub: &Hub, organization: ResolvedOrg, standing: &Standing) -> CallOutcome {
    let links = match repo::hermes::peer_links(hub.pool(), organization, standing.hermes).await {
        Ok(links) => links,
        Err(error) => {
            tracing::error!(%error, "the links of a Hermes could not be read");
            return refusal(ErrorCode::Ar205);
        }
    };
    let content = json!({
        "connected": true,
        "hermes": standing.name.as_str(),
        "can_be_woken": standing.can_be_woken,
        "numbers": [],
        "peers": {
            "enabled": links.outbound || links.inbound,
            "can_send": links.outbound,
            "can_receive": links.inbound,
        },
    });
    match content {
        Value::Object(content) => CallOutcome::Ok(Payload::new(content)),
        _ => refusal(ErrorCode::Ar205),
    }
}
