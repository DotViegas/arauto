//! The first access: the block the hub writes in its own log, with the start code in it, until
//! the first passkey is registered.
//!
//! A hub that starts with a panel and has never had a first passkey issues a start code (a
//! code with no deadline, [`crate::repo::panel::issue_start_code`]) and writes it where
//! whoever installed the hub reads: the output of the start. The code is the one secret the hub
//! ever writes there, and it does so on purpose (`docs/07-telas.md`, T0): it works once, and it
//! only opens the registration of a passkey. It goes straight to the output, never through the
//! lines of the log (`tracing`), so that no log setting copies, reformats or ships it.
//!
//! **One owner.** The clear code exists only in the memory of the task of this module, in one
//! holder ([`Holder`]). No request handler can reach it: a request can only *signal* the task
//! ([`crate::channel::Hub::first_passkey_kept`]), and only the registration of the first
//! passkey does. The holder is cleared when the code is spent or cancelled, when the first
//! access is done, and when the hub stops.
//!
//! **What the task writes.** The block at its start and every 10 minutes while its code is
//! still unspent and the first access is not done; the "replaced" block once, when it finds
//! its code cancelled by a command code (then it stops; cancelled by another start it stops in
//! silence); the "done" line once, when it is told the first passkey was kept. Before every
//! write it reads the database, and when the read fails it writes nothing.
//!
//! **Not guaranteed.** The code in the log is as secret as the log: whoever reads it can open
//! the registration of a passkey until the code is spent or replaced.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use arauto_protocol::limits::MINUTE_MS;
use arauto_protocol::message::DeclaredText;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

use crate::admin::{AdminError, Session};
use crate::channel::Hub;
use crate::db::ResolvedOrg;
use crate::repo;
use crate::repo::panel::{EntryKind, EntryStanding};

/// How often the block is written again while its code is still unspent.
pub const REPEAT_MS: i64 = 10 * MINUTE_MS;

/// How long the task waits to read the database again after a read that failed.
const RETRY_MS: i64 = MINUTE_MS;

/// The longest the task sleeps at once: the time it is due is judged again at each waking.
const LONGEST_SLEEP: Duration = Duration::from_secs(60);

/// What the hub writes after the heading of the block it writes at its start.
const HEADING: &str = "ARAUTO FIRST ACCESS";

/// The line that says the hub has no panel (besides the line of the log).
pub const NO_PANEL_LINE: &str = "=== ARAUTO FIRST ACCESS: this hub has no panel. Set ARAUTO_LISTEN_PANEL and ARAUTO_PANEL_URL, then start it again. ===";

/// The line that says the start code could not be issued (besides the line of the log, and
/// with no error text).
pub const NOT_ISSUED_LINE: &str = "=== ARAUTO FIRST ACCESS: the installation code could not be issued. Restart the hub, or run arauto-hub admin panel code on its server. ===";

/// Where the blocks go. In the hub it is the standard error; a test brings one that keeps what
/// it was given, and counts it.
pub trait Output: Send + Sync + 'static {
    /// Writes `text` as one piece, as it is.
    fn write(&self, text: &str);
}

/// The standard error of the process.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stderr;

impl Output for Stderr {
    fn write(&self, text: &str) {
        use std::io::Write;
        let mut out = std::io::stderr().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }
}

/// What tells the task that the first passkey was kept: the only thing a request does about the
/// first access. It keeps one permit, so a signal that comes while the task is not waiting (it
/// is reading the database, or writing) is not lost.
#[derive(Debug, Default)]
pub(crate) struct Signal {
    notify: Notify,
    signals: AtomicU64,
}

impl Signal {
    /// The first passkey was kept.
    pub(crate) fn kept(&self) {
        self.signals.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_one();
    }

    /// Resolves when [`Signal::kept`] was called, before or after the wait began.
    pub(crate) async fn wait(&self) {
        self.notify.notified().await;
    }

    /// How many times it was kept.
    pub(crate) fn count(&self) -> u64 {
        self.signals.load(Ordering::Relaxed)
    }
}

/// The clear start code, in the one place it lives. It is cleared (and its memory zeroed) when
/// the code is spent or cancelled, when the first access is done and when the task ends.
///
/// It is neither `Clone`, nor `Display`, nor `Serialize`, and its `Debug` does not show the
/// code.
struct Holder(Option<Zeroizing<String>>);

impl Holder {
    fn clear(&mut self) {
        self.0 = None;
    }

    fn code(&self) -> Option<&str> {
        self.0.as_deref().map(String::as_str)
    }
}

impl fmt::Debug for Holder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "Holder(held)"
        } else {
            "Holder(empty)"
        })
    }
}

/// `YYYY-MM-DD HH:MM:SS`, in UTC, of a time in milliseconds. The date is there so that "the
/// block with the latest time" stays right across midnight.
pub(crate) fn clock_time(at_ms: i64) -> String {
    let at = at_ms.div_euclid(1_000);
    let days = at.div_euclid(86_400);
    let seconds = at.rem_euclid(86_400);
    // The civil date of a day count (the algorithm of days-from-civil, run backwards).
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

/// The block with the code: a blank line before and after. It holds the code, so its memory
/// is zeroed when it is dropped, as the memory of the holder is. It is built in a buffer of
/// its exact size, so that no growth leaves a copy of the code in freed memory.
#[must_use]
pub fn block(at_ms: i64, code: &str, origin: &str) -> Zeroizing<String> {
    use std::fmt::Write;
    let time = clock_time(at_ms);
    let head = format!(
        "\n=== {HEADING} ({time} UTC) ===\n\
         Welcome! Your Arauto hub is installed and waiting for its first admin.\n\
         INSTALLATION CODE: "
    );
    let tail = format!(
        "\nOpen: {origin}\n\
         Type the code there and create your passkey.\n\
         The code has no time limit and works once.\n\
         Each restart writes a new one: use the block with the latest time.\n\
         === end of {HEADING} ===\n\n"
    );
    let mut text = Zeroizing::new(String::with_capacity(head.len() + code.len() + tail.len()));
    let _ = text.write_str(&head);
    let _ = text.write_str(code);
    let _ = text.write_str(&tail);
    text
}

/// The block written once, when a command code replaced the start code.
#[must_use]
pub fn replaced_block(at_ms: i64) -> String {
    format!(
        "\n=== {HEADING} ({time} UTC) ===\n\
         The code above was replaced by one issued with arauto-hub admin panel code.\n\
         Use that code, or run the command again.\n\
         === end of {HEADING} ===\n\n",
        time = clock_time(at_ms)
    )
}

/// The line written once, when the first passkey is registered.
#[must_use]
pub fn done_line(at_ms: i64) -> String {
    format!(
        "=== {HEADING}: done ({time} UTC). From now on the panel is entered with your passkey. ===\n",
        time = clock_time(at_ms)
    )
}

/// What the task does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Wait this long (the clock of the hub, in milliseconds), then call [`Task::tick`] again.
    Wait(i64),
    /// The task is over: nothing more will be written.
    Finished,
}

/// The task of the first access, for a hub whose start code was issued. [`start`] runs it;
/// a test drives it by hand with [`prepare`], [`Task::tick`] and [`Task::signalled`], with the
/// fake clock of the test.
#[derive(Debug)]
pub struct Task {
    hub: Arc<Hub>,
    output: Arc<dyn Output>,
    organization: ResolvedOrg,
    origin: String,
    entry: uuid::Uuid,
    holder: Holder,
    /// When the block was last written.
    written_at: i64,
    /// A read that should have been made failed: the next tick reads again, whatever the time.
    read_again: bool,
}

impl fmt::Debug for dyn Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Output(..)")
    }
}

/// What [`prepare`] found.
#[derive(Debug)]
pub enum Prepared {
    /// The hub has no panel: the line that says so was written.
    NoPanel,
    /// The first access was done already: nothing is written and nothing is issued.
    AlreadyDone,
    /// The start code could not be issued: the line that says so was written.
    NotIssued,
    /// The database does not have one organization with its local admin, so the panel serves
    /// none and no code would open it: nothing is issued, and nothing is written where the
    /// block goes (the line of the log says why).
    NotOneOrganization,
    /// The start code was issued and its block was written once; the task goes on from here.
    Ready(Task),
}

/// What a hub does at its start about the first access: writes the line of a hub without a
/// panel; or, with a panel and no first passkey ever, issues a start code and writes its block.
pub async fn prepare(hub: &Arc<Hub>, output: Arc<dyn Output>, os_user: DeclaredText) -> Prepared {
    let Some(panel) = hub.panel() else {
        output.write(&format!("{NO_PANEL_LINE}\n"));
        return Prepared::NoPanel;
    };
    // One resolution of the organization for the entry and for the start code: when the entry
    // does not know which organization it serves (the read at the start failed, or there are
    // zero or several), no code is issued, or the log would write a block for an entry that
    // shows the page of who comes back.
    let Some(served) = panel.organization() else {
        tracing::warn!(
            "no installation code was issued: the panel does not know which organization it \
             serves"
        );
        return Prepared::NotOneOrganization;
    };
    let origin = panel.origin().to_owned();
    let now = hub.now_ms();
    let opened = Session::open(hub.pool().clone(), hub.mark().clone(), os_user).await;
    let issued = match opened {
        Ok(session) => {
            let organization = session.organization();
            if organization != served {
                tracing::warn!(
                    "no installation code was issued: the admin command and the panel resolve \
                     different organizations"
                );
                return Prepared::NotOneOrganization;
            }
            session
                .installation_code(now)
                .await
                .map(|issued| (organization, issued))
        }
        // Neither a restart nor the command would help here: the line of the log says so, and
        // the place of the block stays quiet instead of naming a remedy that cannot work.
        Err(error @ (AdminError::NotInstalled | AdminError::Organizations { .. })) => {
            tracing::warn!(
                %error,
                "no installation code was issued: the panel serves one organization with its \
                 local admin"
            );
            return Prepared::NotOneOrganization;
        }
        Err(error) => Err(error),
    };
    match issued {
        Ok((_, None)) => Prepared::AlreadyDone,
        Ok((organization, Some(issued))) => {
            let code = issued.code.reveal();
            output.write(&block(now, code.as_str(), &origin));
            Prepared::Ready(Task {
                hub: Arc::clone(hub),
                output,
                organization,
                origin,
                entry: issued.id,
                holder: Holder(Some(code)),
                written_at: now,
                read_again: false,
            })
        }
        Err(error) => {
            tracing::warn!(
                %error,
                "the installation code was not issued: use `arauto-hub admin panel code`"
            );
            output.write(&format!("{NOT_ISSUED_LINE}\n"));
            Prepared::NotIssued
        }
    }
}

impl Task {
    /// The time asks. The database is read at every ask, however little time has passed
    /// (one row, and nothing is written), so that a code that was cancelled or spent is let go
    /// of at the next waking and not at the next block. The block is written again only when
    /// it is due, and only after that read.
    pub async fn tick(&mut self, now: i64) -> Step {
        let since = now.saturating_sub(self.written_at);
        // A clock that went back is judged as due: better a block more than a lost one.
        let due = !(0..REPEAT_MS).contains(&since);
        self.look(now, due || self.read_again).await
    }

    /// The first passkey was kept (the hub signalled): reads the database and, when the mark of
    /// the first access is there, writes the closing line. It never writes the block.
    pub async fn signalled(&mut self, now: i64) -> Step {
        match repo::panel::first_access_standing(self.hub.pool(), self.organization, self.entry)
            .await
        {
            Ok((true, _)) => self.finish_done(now),
            Ok((false, _)) => {
                Step::Wait(REPEAT_MS.saturating_sub(now.saturating_sub(self.written_at)))
            }
            Err(_) => {
                self.read_again = true;
                Step::Wait(RETRY_MS)
            }
        }
    }

    /// Reads the database, and acts on what it says about the first access and about the code.
    /// The block is written only when `due`.
    async fn look(&mut self, now: i64, due: bool) -> Step {
        let standing =
            repo::panel::first_access_standing(self.hub.pool(), self.organization, self.entry)
                .await;
        let (done, entry) = match standing {
            Ok(read) => read,
            // A read that fails writes nothing, and is made again.
            Err(_) => {
                if due {
                    self.read_again = true;
                    return Step::Wait(RETRY_MS);
                }
                return Step::Wait(self.until_due(now));
            }
        };
        if due {
            self.read_again = false;
        }
        if done {
            return self.finish_done(now);
        }
        match entry {
            EntryStanding::Issued => {
                if !due {
                    return Step::Wait(self.until_due(now));
                }
                if let Some(code) = self.holder.code() {
                    self.output.write(&block(now, code, &self.origin));
                }
                self.written_at = now;
                Step::Wait(REPEAT_MS)
            }
            EntryStanding::Cancelled(EntryKind::Command) => {
                self.holder.clear();
                self.output.write(&replaced_block(now));
                Step::Finished
            }
            // Another start took its place, or the row is gone: this hub has nothing to say.
            EntryStanding::Cancelled(EntryKind::Start)
            | EntryStanding::Used
            | EntryStanding::Missing => {
                self.holder.clear();
                Step::Finished
            }
        }
    }

    /// How long until the block is due again.
    fn until_due(&self, now: i64) -> i64 {
        REPEAT_MS
            .saturating_sub(now.saturating_sub(self.written_at))
            .max(1)
    }

    fn finish_done(&mut self, now: i64) -> Step {
        self.holder.clear();
        self.output.write(&done_line(now));
        Step::Finished
    }

    /// Whether the holder still has the code. For tests of the task: the code itself is never
    /// handed out.
    #[must_use]
    pub fn holds_the_code(&self) -> bool {
        self.holder.code().is_some()
    }

    async fn run(mut self) {
        loop {
            let now = self.hub.now_ms();
            let Step::Wait(wait) = self.tick(now).await else {
                return;
            };
            let sleep = Duration::from_millis(u64::try_from(wait.max(1_000)).unwrap_or(1_000))
                .min(LONGEST_SLEEP);
            let hub = Arc::clone(&self.hub);
            tokio::select! {
                () = hub.first_passkey_signal() => {
                    if self.signalled(self.hub.now_ms()).await == Step::Finished {
                        return;
                    }
                }
                () = tokio::time::sleep(sleep) => {}
            }
        }
    }
}

/// The task of the first access of a running hub, until it stops.
#[derive(Debug)]
pub struct FirstAccess {
    task: Option<JoinHandle<()>>,
}

impl FirstAccess {
    /// Stops the task, if it still runs. Its holder is dropped, and the code with it.
    pub async fn stop(self) {
        if let Some(task) = self.task {
            task.abort();
            let _ = task.await;
        }
    }

    /// Whether a task is still running.
    #[must_use]
    pub fn running(&self) -> bool {
        self.task.as_ref().is_some_and(|task| !task.is_finished())
    }
}

/// What `arauto-hub serve` calls once the panel listens: [`prepare`], and the task that goes
/// on writing the block every 10 minutes while the code is unspent.
pub async fn start(hub: &Arc<Hub>, output: Arc<dyn Output>, os_user: DeclaredText) -> FirstAccess {
    match prepare(hub, output, os_user).await {
        Prepared::Ready(task) => FirstAccess {
            task: Some(tokio::spawn(task.run())),
        },
        Prepared::NoPanel
        | Prepared::AlreadyDone
        | Prepared::NotIssued
        | Prepared::NotOneOrganization => FirstAccess { task: None },
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use super::*;

    /// Whether `$type` implements `$trait`, decided at compile time for a concrete type.
    macro_rules! implements {
        ($type:ty, $trait:path) => {{
            #[allow(dead_code)]
            trait Fallback {
                const YES: bool = false;
            }
            impl<T> Fallback for T {}
            struct Probe<T>(PhantomData<T>);
            impl<T: $trait> Probe<T> {
                #[allow(dead_code)]
                const YES: bool = true;
            }
            <Probe<$type>>::YES
        }};
    }

    #[test]
    fn the_detector_of_traits_sees_one_that_is_there() {
        assert!(implements!(String, Clone));
        assert!(implements!(u8, std::fmt::Display));
        assert!(!implements!(Holder, Clone));
    }

    /// R9, R10: a signal that comes while nobody waits (the task is reading the database) is
    /// kept, and two in a row are one: the done line is not lost, and not written twice.
    #[tokio::test]
    async fn a_signal_with_no_one_waiting_is_kept_once() {
        let signal = Signal::default();
        signal.kept();
        signal.kept();
        assert_eq!(signal.count(), 2);
        tokio::time::timeout(Duration::from_millis(500), signal.wait())
            .await
            .expect("the first signal was kept for the waiter");
        assert!(
            tokio::time::timeout(Duration::from_millis(100), signal.wait())
                .await
                .is_err(),
            "two signals in a row leave one permit"
        );
    }

    #[test]
    fn the_holder_is_not_clone_display_or_serialize_and_its_debug_hides_the_code() {
        assert!(!implements!(Holder, Clone));
        assert!(!implements!(Holder, std::fmt::Display));
        assert!(!implements!(Holder, serde::Serialize));
        let code = "abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef";
        let mut holder = Holder(Some(Zeroizing::new(code.to_owned())));
        assert!(!format!("{holder:?}").contains(code));
        assert!(!format!("{holder:#?}").contains("abcd"));
        assert_eq!(holder.code(), Some(code));
        holder.clear();
        assert_eq!(holder.code(), None);
        assert!(!format!("{holder:?}").contains(code));
    }

    /// D2: the heading carries the date, so "the latest time" is right across midnight.
    #[test]
    fn the_time_of_a_block_is_utc_with_its_date() {
        assert_eq!(clock_time(0), "1970-01-01 00:00:00");
        assert_eq!(clock_time(1_794_661_509_000 + 999), "2026-11-14 13:05:09");
        assert_eq!(clock_time(86_400_000 + 1_000), "1970-01-02 00:00:01");
        assert_eq!(clock_time(-1_000), "1969-12-31 23:59:59");
        // A leap day, and the first second after it.
        assert_eq!(clock_time(1_709_251_199_000), "2024-02-29 23:59:59");
        assert_eq!(clock_time(1_709_251_200_000), "2024-03-01 00:00:00");
        // Across midnight the text of the later block sorts after the earlier one.
        assert!(clock_time(1_794_661_509_000 + 86_400_000) > clock_time(1_794_661_509_000));
    }

    /// R16: every text the hub writes where the block goes is word for word the approved one,
    /// with the time, the code and the address filled in. The expected texts are written out
    /// here, apart from the code that builds them.
    #[test]
    fn the_texts_of_the_first_access_are_the_approved_ones_word_for_word() {
        let code = "abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef";
        let origin = "https://panel.example.org";
        // 2026-11-14 13:05:09 UTC.
        let at = 1_794_661_509_000;
        let expected_block = [
            "",
            "=== ARAUTO FIRST ACCESS (2026-11-14 13:05:09 UTC) ===",
            "Welcome! Your Arauto hub is installed and waiting for its first admin.",
            "INSTALLATION CODE: abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef",
            "Open: https://panel.example.org",
            "Type the code there and create your passkey.",
            "The code has no time limit and works once.",
            "Each restart writes a new one: use the block with the latest time.",
            "=== end of ARAUTO FIRST ACCESS ===",
            "",
            "",
        ]
        .join("\n");
        assert_eq!(block(at, code, origin).as_str(), expected_block);
        let expected_replaced = [
            "",
            "=== ARAUTO FIRST ACCESS (2026-11-14 13:05:09 UTC) ===",
            "The code above was replaced by one issued with arauto-hub admin panel code.",
            "Use that code, or run the command again.",
            "=== end of ARAUTO FIRST ACCESS ===",
            "",
            "",
        ]
        .join("\n");
        assert_eq!(replaced_block(at), expected_replaced);
        assert_eq!(
            done_line(at),
            "=== ARAUTO FIRST ACCESS: done (2026-11-14 13:05:09 UTC). From now on the panel is entered \
             with your passkey. ===\n"
        );
        assert_eq!(
            NO_PANEL_LINE,
            "=== ARAUTO FIRST ACCESS: this hub has no panel. Set ARAUTO_LISTEN_PANEL and \
             ARAUTO_PANEL_URL, then start it again. ==="
        );
        assert_eq!(
            NOT_ISSUED_LINE,
            "=== ARAUTO FIRST ACCESS: the installation code could not be issued. Restart the \
             hub, or run arauto-hub admin panel code on its server. ==="
        );
    }

    #[test]
    fn the_block_has_the_fixed_text_the_origin_and_no_minutes() {
        let code = "abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef";
        let origin = "https://panel.example.org";
        let text = block(1_800_000_000_000, code, origin);
        assert!(text.starts_with("\n=== ARAUTO FIRST ACCESS ("));
        assert!(text.ends_with("=== end of ARAUTO FIRST ACCESS ===\n\n"));
        assert!(text.contains(&format!("INSTALLATION CODE: {code}\n")));
        assert!(text.contains(&format!("\nOpen: {origin}\n")));
        assert!(text.contains("no time limit"));
        assert!(text.contains("latest"));
        assert!(!text.contains("minutes"));
        assert!(text.is_ascii());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 10, "a blank line, eight lines, a blank line");
        assert!(lines[0].is_empty() && lines[9].is_empty());
        for line in &lines {
            // The code and the address stand on lines of their own, and the fixed text is short.
            if !line.starts_with("INSTALLATION CODE:") && !line.starts_with("Open:") {
                assert!(line.len() <= 80, "{line}");
            }
        }
        let replaced = replaced_block(0);
        assert!(replaced.starts_with("\n=== ARAUTO FIRST ACCESS (1970-01-01 00:00:00 UTC) ===\n"));
        assert!(replaced.ends_with("=== end of ARAUTO FIRST ACCESS ===\n\n"));
        for line in replaced.lines() {
            assert!(line.len() <= 80, "{line}");
        }
        for text in [
            replaced,
            done_line(0),
            NO_PANEL_LINE.to_owned(),
            NOT_ISSUED_LINE.to_owned(),
        ] {
            assert!(text.is_ascii());
            assert!(!text.contains("INSTALLATION CODE"));
            assert!(!text.contains("minutes"));
        }
    }
}
