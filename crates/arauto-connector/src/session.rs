//! The session of the channel, on the side of the connector (`docs/14-protocolo-do-canal.md`,
//! sections 5.3 and 6; `docs/04-seguranca-conexao.md`, C7 and C8).
//!
//! | From the hub | What the connector does |
//! |---|---|
//! | `result` | Answers the call of that identifier. |
//! | `check` | Already answered by the reader of the connection: the test of a newer connection. |
//! | `rotate`, then `credential` | The 24-hour swap: the credential is written durably and only then `saved` goes; the calls go on meanwhile. A `credential` without `rotate` before it ends the connection. |
//! | `rotated` | The swap was recorded. |
//! | `tools` | The list of released tools changed: it is kept in `state.json`. |
//! | `notice` | Checked, and the Hermes is woken through its local API ([`crate::wake`]): `rejected` when out of format, `duplicate`, `throttled`, `accepted` or `failed`. |
//! | `left` | The answer to the request to leave sent over this channel. |
//! | anything else | Ends the connection. |
//!
//! **Calls.** At most 32 in progress: the 33rd is refused here with AR-710, without reaching the
//! hub. A call that does not fit is refused here with AR-709. A call that never left when the
//! channel goes down gets AR-203; one that left and got no answer, AR-207.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::CALLS_IN_FLIGHT_MAX;
use arauto_protocol::message::{
    Call, CallId, CallOutcome, Message, NoticeAck, Payload, Saved, Session,
};
use arauto_protocol::tools::ToolName;
use tokio::sync::{mpsc, oneshot, watch};

use crate::link::{End, Link, LinkHandle, SendError};
use crate::writes::{self, Saving};

/// A refusal of the connector itself.
pub(crate) const fn refused(code: ErrorCode) -> CallOutcome {
    CallOutcome::Refused {
        code,
        retry_after: None,
    }
}

/// The calls in progress of one session.
#[derive(Debug, Default)]
struct Flight {
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, oneshot::Sender<CallOutcome>>>,
    closed: AtomicBool,
}

impl Flight {
    fn waiting(&self) -> MutexGuard<'_, HashMap<u64, oneshot::Sender<CallOutcome>>> {
        // A map of senders: a panic elsewhere leaves it usable.
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes an identifier for a new call, or refuses it.
    fn register(&self) -> Result<(CallId, oneshot::Receiver<CallOutcome>), ErrorCode> {
        let mut waiting = self.waiting();
        if self.closed.load(Ordering::SeqCst) {
            return Err(ErrorCode::Ar203);
        }
        if waiting.len() >= CALLS_IN_FLIGHT_MAX {
            return Err(ErrorCode::Ar710);
        }
        // Positive, and unique among the calls in progress of this connection.
        let id = loop {
            let id = self.next.fetch_add(1, Ordering::SeqCst).saturating_add(1);
            if !waiting.contains_key(&id) {
                break id;
            }
        };
        let call_id = CallId::new(id).ok_or(ErrorCode::Ar203)?;
        let (answer, answered) = oneshot::channel();
        waiting.insert(id, answer);
        Ok((call_id, answered))
    }

    fn forget(&self, id: CallId) {
        self.waiting().remove(&id.get());
    }

    fn answer(&self, id: CallId, outcome: CallOutcome) {
        if let Some(waiting) = self.waiting().remove(&id.get()) {
            let _ = waiting.send(outcome);
        }
    }

    /// The session is over: every call still waiting gets AR-207, and no new one is taken.
    fn close(&self) {
        let mut waiting = self.waiting();
        self.closed.store(true, Ordering::SeqCst);
        waiting.clear();
    }
}

/// How the calls go through a live session.
#[derive(Debug, Clone)]
pub(crate) struct Calls {
    link: LinkHandle,
    flight: Arc<Flight>,
}

impl Calls {
    /// Makes one call through the session and waits for its answer.
    pub(crate) async fn call(&self, tool: ToolName, args: Payload) -> CallOutcome {
        let (id, answered) = match self.flight.register() {
            Ok(registered) => registered,
            Err(code) => return refused(code),
        };
        let call = Message::Call(Call { id, tool, args });
        match self.link.send(&call).await {
            Ok(()) => {}
            Err(SendError::TooLarge) => {
                self.flight.forget(id);
                return refused(ErrorCode::Ar709);
            }
            Err(SendError::NotSent) => {
                self.flight.forget(id);
                return refused(ErrorCode::Ar203);
            }
            Err(SendError::Interrupted) => {
                self.flight.forget(id);
                return refused(ErrorCode::Ar207);
            }
        }
        drop(call);
        // Sent: without an answer, the result is unknown.
        answered.await.unwrap_or(refused(ErrorCode::Ar207))
    }
}

/// What the leader learns of a session.
#[derive(Debug, Clone)]
pub(crate) struct Live {
    /// How calls go through it.
    pub(crate) calls: Calls,
    /// What the hub said in `session`.
    pub(crate) session: Session,
}

/// What a session tells the code around it while it runs.
#[derive(Debug)]
pub(crate) enum News {
    /// The connection is waiting for the admin.
    Waiting,
    /// The session began.
    Session(Live),
    /// The list of released tools changed.
    Tools(Vec<ToolName>),
    /// The established connection ended.
    Ended,
}

/// A request to leave over the open channel: the answer is the status of `left`, or `None`
/// when the channel ended without one.
pub(crate) type LeaveAsk = oneshot::Sender<Option<arauto_protocol::message::LeftStatus>>;

/// How the session ended.
#[derive(Debug)]
pub(crate) struct SessionEnd {
    pub(crate) end: End,
    /// The status of `left`, when this side asked to leave and the hub answered.
    pub(crate) left: Option<arauto_protocol::message::LeftStatus>,
    /// Asked to stop by the code around it.
    pub(crate) stopped: bool,
}

/// What the session needs from around it.
pub(crate) struct Around<'a> {
    pub(crate) saving: Saving,
    pub(crate) news: &'a mpsc::UnboundedSender<News>,
    pub(crate) leave: &'a mut mpsc::Receiver<LeaveAsk>,
    pub(crate) stop: &'a mut watch::Receiver<bool>,
}

/// Serves the session until the connection ends.
pub(crate) async fn serve(mut link: Link, session: Session, around: Around<'_>) -> SessionEnd {
    let flight = Arc::new(Flight::default());
    let calls = Calls {
        link: link.handle(),
        flight: Arc::clone(&flight),
    };
    let _ = around.news.send(News::Session(Live {
        calls,
        session: session.clone(),
    }));
    let device = around.saving.dir.id().to_string();
    let mut rotating = false;
    let mut saving: Option<tokio::task::JoinHandle<()>> = None;
    let mut leaving: Option<LeaveAsk> = None;
    let mut left = None;
    let mut stopped = false;
    let end = loop {
        tokio::select! {
            message = link.recv() => match message {
                Ok(Message::Result(result)) => flight.answer(result.id, result.outcome),
                // Answered by the reader of the connection.
                Ok(Message::Check(_)) => {}
                Ok(Message::Rotate) => rotating = true,
                Ok(Message::Credential(credential)) if rotating => {
                    rotating = false;
                    if saving.as_ref().is_some_and(|task| !task.is_finished()) {
                        // A second swap before the first was saved: the hub never does it.
                        link.close();
                        continue;
                    }
                    let saving_now = around.saving.clone();
                    let handle = link.handle();
                    let device = device.clone();
                    saving = Some(tokio::spawn(async move {
                        rotate(saving_now, handle, credential, device).await;
                    }));
                }
                Ok(Message::Rotated(_)) => {
                    #[cfg(feature = "testing")]
                    around.saving.settings().emit(|| crate::testing::Event::Rotated {
                        device: device.clone(),
                    });
                    tracing::info!("the 24-hour swap of the credential was recorded");
                }
                Ok(Message::Tools(tools)) => {
                    let _ = around.news.send(News::Tools(tools.tools.as_slice().to_vec()));
                }
                Ok(Message::Notice(notice)) => {
                    // Its own task: the local API of Hermes may take its time, and the loop
                    // of the session goes on meanwhile.
                    let handle = link.handle();
                    let home = around.saving.home.clone();
                    let waker = Arc::clone(&around.saving.settings.waker);
                    tokio::spawn(async move {
                        let outcome = waker.handle(&home, &notice).await;
                        let ack = Message::NoticeAck(NoticeAck {
                            id: notice.id,
                            outcome,
                        });
                        let _ = handle.send(&ack).await;
                    });
                }
                Ok(Message::Left(answer)) if leaving.is_some() => {
                    left = Some(answer.status);
                    if let Some(asker) = leaving.take() {
                        let _ = asker.send(Some(answer.status));
                    }
                }
                // Anything else belongs before the session, or never comes from the hub.
                Ok(_) => link.close(),
                Err(end) => break end,
            },
            asked = around.leave.recv(), if leaving.is_none() => {
                if let Some(asker) = asked {
                    if link.send(&Message::Leave).await.is_ok() {
                        leaving = Some(asker);
                    } else {
                        let _ = asker.send(None);
                    }
                }
            }
            changed = around.stop.changed() => {
                if changed.is_err() || *around.stop.borrow() {
                    stopped = true;
                    link.close();
                }
            }
        }
    };
    flight.close();
    if let Some(asker) = leaving.take() {
        let _ = asker.send(None);
    }
    if let Some(task) = saving {
        // A write under way finishes, so that what is on disk is a whole credential.
        let _ = task.await;
    }
    if stopped {
        // The program may be about to end: the close leaves first.
        link.handle().closed().await;
    }
    let _ = around.news.send(News::Ended);
    SessionEnd { end, left, stopped }
}

/// The 24-hour swap: the credential is written durably, and only then `saved` goes.
async fn rotate(
    saving: Saving,
    link: LinkHandle,
    credential: arauto_protocol::message::Credential,
    device: String,
) {
    #[cfg(not(feature = "testing"))]
    let _ = &device;
    #[cfg(feature = "testing")]
    if saving
        .settings()
        .fails(&device, crate::testing::Point::RotationBeforeSaving)
    {
        link.close();
        return;
    }
    let saved = Saved {
        generation: credential.generation,
        digest: CredentialDigest::of(&credential.sealed),
    };
    // The write fails as on a full disk: nothing is written, and nothing is "saved".
    #[cfg(feature = "testing")]
    if saving
        .settings()
        .fails(&device, crate::testing::Point::RotationWrite)
    {
        crate::testing::slow_disk().await;
        link.close();
        return;
    }
    match writes::save_credential(&saving, credential.sealed).await {
        Ok(()) => {}
        Err(error) => {
            // Nothing is "saved": the connection ends, and on the way back what is on disk is
            // presented.
            tracing::warn!(%error, "the new credential could not be written");
            link.close();
            return;
        }
    }
    #[cfg(feature = "testing")]
    if saving
        .settings()
        .fails(&device, crate::testing::Point::RotationBeforeSaved)
    {
        link.close();
        return;
    }
    let sent = link.send(&Message::Saved(saved)).await;
    #[cfg(feature = "testing")]
    if sent.is_ok()
        && saving
            .settings()
            .fails(&device, crate::testing::Point::RotationBeforeRotated)
    {
        link.close();
    }
    #[cfg(not(feature = "testing"))]
    let _ = sent;
}
