//! What the tests of the transport share: the hub started in the test and its installation come
//! from the kit of test ([`arauto_testkit::hub::support`]); here are the purposes that take a
//! connection where a test needs it (session, waiting, flood), apart from the ones of the hub.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arauto_hub::channel::{Hub, Link, Purposes};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{
    CallId, CallOutcome, CallResult, Hello, HelloOk, Message, Payload, Session, ToolList, Tools,
};
use arauto_protocol::names::HermesName;
use serde_json::{Map, Value};

pub use arauto_testkit::hub::support::{
    TestHub, config, eventually, install, settings, shared, start, start_with, timed,
};

// ---------------------------------------------------------------------------- purposes

/// What the purpose `channel` does in a test.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// Stays before the session and reads whatever comes.
    Park,
    /// Enters the session, and answers every message with `tools`.
    Session,
    /// Enters the session and sends `count` results of `size` bytes, then `tools`. Says in
    /// `blocked` whether a send had to wait for room.
    Flood {
        count: usize,
        size: usize,
        blocked: Arc<AtomicBool>,
    },
    /// Waits for approval for `ms` milliseconds, then goes back to before the session.
    Wait { ms: u64 },
}

/// The purposes of a test: `channel` does the [`Behavior`]; `enroll` closes with AR-205.
#[derive(Debug, Clone)]
pub struct TestPurposes(pub Behavior);

pub fn session_message() -> Message {
    Message::Session(Session {
        hermes: HermesName::parse("test").unwrap(),
        tools: ToolList::new(vec![]).unwrap(),
        wake: false,
    })
}

pub fn tools_message() -> Message {
    Message::Tools(Tools {
        tools: ToolList::new(vec![]).unwrap(),
    })
}

/// A `result` with `size` bytes of padding.
pub fn big_result(size: usize) -> Message {
    let mut content = Map::new();
    content.insert("pad".to_owned(), Value::String("b".repeat(size)));
    Message::Result(CallResult {
        id: CallId::MIN,
        outcome: CallOutcome::Ok(Payload::new(content)),
    })
}

async fn park(link: &mut Link) {
    while link.recv().await.is_ok() {}
}

impl Purposes for TestPurposes {
    async fn enroll(&self, _hub: Arc<Hub>, link: Link, _hello: Hello) {
        link.close(ErrorCode::Ar205);
    }

    async fn channel(&self, _hub: Arc<Hub>, mut link: Link, _hello: Hello, _answered: HelloOk) {
        match &self.0 {
            Behavior::Park => park(&mut link).await,
            Behavior::Session => {
                if link.enter_session(&session_message()).await.is_err() {
                    return;
                }
                while link.recv().await.is_ok() {
                    if link.send(&tools_message()).await.is_err() {
                        return;
                    }
                }
            }
            Behavior::Flood {
                count,
                size,
                blocked,
            } => {
                if link.enter_session(&session_message()).await.is_err() {
                    return;
                }
                let message = big_result(*size);
                for _ in 0..*count {
                    // A send is never abandoned half way: it would leave a stream unfinished.
                    let send = link.send(&message);
                    tokio::pin!(send);
                    let sent = tokio::select! {
                        sent = &mut send => sent,
                        () = tokio::time::sleep(Duration::from_millis(200)) => {
                            blocked.store(true, Ordering::SeqCst);
                            send.await
                        }
                    };
                    if sent.is_err() {
                        return;
                    }
                }
                if link.send(&tools_message()).await.is_err() {
                    return;
                }
                park(&mut link).await;
            }
            Behavior::Wait { ms } => {
                link.enter_waiting();
                let _ = tokio::time::timeout(Duration::from_millis(*ms), park(&mut link)).await;
                link.leave_waiting();
                park(&mut link).await;
            }
        }
    }
}
