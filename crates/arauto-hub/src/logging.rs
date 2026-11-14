//! How the lines of the log are written.
//!
//! The log goes to the standard error. ANSI colors are written only when that is a terminal: a
//! container or a service that collects the output would otherwise store the escape characters
//! as text, and a person who reads the log of the first access would find them around the
//! words.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::util::SubscriberInitExt;

/// Which lines the log keeps: `RUST_LOG` when it is set and valid. Without it the log says
/// what an operator needs: the start, the warnings of the engine, the restored mode.
#[must_use]
pub fn filter_from_env() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// The log of the hub, keeping the lines `filter` lets through and writing them to `writer`,
/// with colors only when `terminal` says the output is a terminal.
pub fn subscriber<W>(
    filter: EnvFilter,
    terminal: bool,
    writer: W,
) -> impl tracing::Subscriber + Send + Sync + 'static
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(terminal)
        .with_writer(writer)
        .finish()
}

/// Starts the log of the hub on the standard error ([`subscriber`]). A log that was started
/// already is left as it is.
pub fn init_stderr() {
    let _ = subscriber(
        filter_from_env(),
        std::io::stderr().is_terminal(),
        std::io::stderr,
    )
    .try_init();
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Ok(mut inside) = self.0.lock() {
                inside.extend_from_slice(bytes);
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// One line of the log, written by the subscriber the hub starts, told whether the output
    /// is a terminal. The filter is fixed, so that a `RUST_LOG` of the machine that runs the
    /// test cannot hide the line.
    fn line_with(terminal: bool) -> String {
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber = subscriber(EnvFilter::new("info"), terminal, move || writer.clone());
        tracing::subscriber::with_default(subscriber, || tracing::warn!("a line"));
        let written = captured
            .0
            .lock()
            .map(|inside| inside.clone())
            .unwrap_or_default();
        String::from_utf8_lossy(&written).into_owned()
    }

    #[test]
    fn colors_are_written_only_when_the_output_is_a_terminal() {
        let plain = line_with(false);
        assert!(plain.contains("a line"), "{plain}");
        assert!(!plain.contains('\u{1b}'), "{plain:?}");
        assert!(line_with(true).contains('\u{1b}'));
    }
}
