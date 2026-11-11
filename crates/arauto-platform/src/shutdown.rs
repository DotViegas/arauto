//! The request to stop the process.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Why the wait for the stop request could not be set up.
#[derive(Debug, thiserror::Error)]
pub enum ShutdownSignalError {
    /// The call was made outside a Tokio runtime.
    #[error("the wait for the stop signal needs a running Tokio runtime")]
    NoRuntime,

    /// The system refused the signal handler.
    #[error("could not listen for the stop signal: {cause}")]
    Io {
        /// The error reported by the system.
        cause: io::Error,
    },
}

/// A future that completes when the process is asked to stop: `SIGTERM` or `SIGINT` on
/// Unix, Ctrl+C on Windows.
///
/// It can be awaited again (`(&mut signal).await`) to wait for a second request. Requests
/// that arrive between two waits are not lost, but several of them count as one.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ShutdownSignal {
    signals: imp::Signals,
}

/// Starts listening for the request to stop the process.
///
/// The listening starts here, not at the first poll: call it early and keep the value, so a
/// request that arrives during the start-up is still seen.
///
/// From this call on the request no longer ends the process by itself. On Unix that lasts
/// for the rest of the process, even after the value is dropped; on Windows it lasts while
/// the value lives.
///
/// # Errors
///
/// [`ShutdownSignalError::NoRuntime`] outside a Tokio runtime, and
/// [`ShutdownSignalError::Io`] when the system refuses the handler.
///
/// # Panics
///
/// On Unix, Tokio panics when the runtime was built without its I/O driver (`enable_io` or
/// `enable_all`).
pub fn shutdown_signal() -> Result<ShutdownSignal, ShutdownSignalError> {
    if tokio::runtime::Handle::try_current().is_err() {
        return Err(ShutdownSignalError::NoRuntime);
    }
    let signals = imp::Signals::listen().map_err(|cause| ShutdownSignalError::Io { cause })?;
    Ok(ShutdownSignal { signals })
}

impl Future for ShutdownSignal {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.signals.poll(cx)
    }
}

#[cfg(unix)]
mod imp {
    use std::io;
    use std::task::{Context, Poll};

    use tokio::signal::unix::{Signal, SignalKind, signal};

    #[derive(Debug)]
    pub(super) struct Signals {
        terminate: Signal,
        interrupt: Signal,
    }

    impl Signals {
        pub(super) fn listen() -> io::Result<Self> {
            Ok(Self {
                terminate: signal(SignalKind::terminate())?,
                interrupt: signal(SignalKind::interrupt())?,
            })
        }

        /// Ready when either signal arrived. Both are polled, so both wake the task.
        pub(super) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
            let terminate = self.terminate.poll_recv(cx);
            let interrupt = self.interrupt.poll_recv(cx);
            if terminate.is_ready() || interrupt.is_ready() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::task::{Context, Poll};

    use tokio::signal::windows::{CtrlC, ctrl_c};

    #[derive(Debug)]
    pub(super) struct Signals {
        ctrl_c: CtrlC,
    }

    impl Signals {
        pub(super) fn listen() -> io::Result<Self> {
            Ok(Self { ctrl_c: ctrl_c()? })
        }

        pub(super) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
            self.ctrl_c.poll_recv(cx).map(|_| ())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn outside_a_runtime_it_is_an_error_and_not_a_panic() {
        let outcome = shutdown_signal();

        assert!(
            matches!(outcome, Err(ShutdownSignalError::NoRuntime)),
            "{outcome:?}"
        );
    }

    // On Windows the handler steps aside again once nobody listens, so this test does not
    // change how Ctrl+C ends the test run. On Unix the handler would stay for the life of
    // the process: there the same check runs in a second process (see below).
    #[cfg(windows)]
    #[tokio::test]
    async fn without_a_request_it_stays_pending() {
        let mut signal = shutdown_signal().unwrap();

        let waited = tokio::time::timeout(Duration::from_millis(100), &mut signal).await;

        assert!(waited.is_err(), "nobody asked the process to stop");
    }

    #[cfg(unix)]
    mod unix {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};

        use super::*;
        use crate::test_support::{CHILD_READY, ChildGuard, child_role};

        /// Role of the second process that listens and waits to be signalled.
        const ROLE_WAIT_FOR_SIGNAL: &str = "wait-for-signal";

        /// What the second process does. It ends well only if the signal completes the
        /// future; without the handler the signal would kill it, and that fails too.
        fn wait_for_the_signal() {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                let mut signal = shutdown_signal().unwrap();
                let early = tokio::time::timeout(Duration::from_millis(100), &mut signal).await;
                assert!(early.is_err(), "nobody asked the process to stop yet");

                println!("{CHILD_READY}");
                tokio::time::timeout(Duration::from_secs(20), &mut signal)
                    .await
                    .expect("the signal completes the future");
            });
        }

        fn second_process_completes_on(test: &str, signal: &str) {
            let mut listener = ChildGuard::spawn(
                Command::new(std::env::current_exe().unwrap())
                    .args([test, "--exact", "--nocapture"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null()),
                ROLE_WAIT_FOR_SIGNAL,
            );
            let mut output = BufReader::new(listener.stdout());
            let mut line = String::new();
            while line.trim_end() != CHILD_READY {
                line.clear();
                assert_ne!(output.read_line(&mut line).unwrap(), 0, "it never listened");
            }

            let sent = Command::new("sh")
                .arg("-c")
                .arg(format!("kill -{signal} {}", listener.id()))
                .status()
                .unwrap();
            assert!(sent.success());

            assert!(
                listener.wait().success(),
                "SIG{signal} did not complete the future"
            );
        }

        #[test]
        fn sigterm_completes_it() {
            if child_role().as_deref() == Some(ROLE_WAIT_FOR_SIGNAL) {
                return wait_for_the_signal();
            }
            second_process_completes_on("shutdown::tests::unix::sigterm_completes_it", "TERM");
        }

        #[test]
        fn sigint_completes_it() {
            if child_role().as_deref() == Some(ROLE_WAIT_FOR_SIGNAL) {
                return wait_for_the_signal();
            }
            second_process_completes_on("shutdown::tests::unix::sigint_completes_it", "INT");
        }
    }
}
