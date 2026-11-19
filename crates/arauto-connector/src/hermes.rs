//! The registration of the connector in Hermes, by its official command
//! (`docs/11-conector.md`, section 4; `referencia/hermes-fatos.md`):
//!
//! ```text
//! hermes mcp add arauto --command <folder>/arauto-connector --args mcp --home <folder>
//! hermes mcp list
//! hermes mcp remove arauto
//! ```
//!
//! - **The program.** `ARAUTO_HERMES_BIN`, when set, is used exactly. When it is not, `hermes` is
//!   tried through the `PATH` and, only if the system says it does not exist, these fixed
//!   absolute places, in this order, on Unix: `/opt/hermes/bin/hermes`,
//!   `/opt/hermes/.venv/bin/hermes` and `.local/bin/hermes` in the folder of the user (the
//!   terminal of a Hermes may not have it on its `PATH`; this also covers macOS). On Windows the
//!   one place is `hermes\bin\hermes.exe` inside `LOCALAPPDATA`, read from the environment of
//!   this machine and left out when it is unset or not absolute. The first that starts wins. Nothing
//!   from the hub or from the command line of `join` enters that list. The program is run
//!   directly: no interpreter of commands, on any system. A batch file of Windows would be one,
//!   so it is refused.
//! - Every argument is a fixed word or an absolute path of this machine; a path with a control
//!   character, or that is not text, is refused. Nothing that came from the hub or from the
//!   block becomes an argument.
//! - `--home` always goes in the arguments, also for the default folder: Hermes starts the
//!   connector with a filtered environment, where `ARAUTO_HOME` would not arrive, and in a
//!   container (checked on v0.22.5) with another `HOME` than the one of the terminal where
//!   `join` ran. Without the folder in the entry, the copy Hermes starts would look for its
//!   devices in a folder that is not the one `join` wrote.
//! - **`add` asks questions.** Hermes v0.22.5 asks on the terminal whether to overwrite an
//!   entry that exists and whether to enable all the tools, and saves nothing when the input
//!   ends without an answer, with the exit code 0 all the same. So `add` gives it the bytes
//!   `y`, newline, `y`, newline on a pipe and closes the pipe; its output stays null.
//! - **The exit code is not the proof.** `add` is only a success when a following `mcp list`
//!   has a row whose first word is exactly the name of the server and that does not say
//!   `disabled`. A row that is there but disabled (the probe of the server failed) is removed,
//!   as far as it can be, and is a failure. The text of the list is read for this check alone,
//!   through a ceiling of 64 KiB: it is never logged, printed, or put in an error. What Hermes
//!   prints on `add` and `remove` is not read.
//! - Not guaranteed: another version of Hermes may ask other questions or print the list in
//!   another format; then `add` fails (AR-601) instead of reporting a registration it cannot see.
//! - **The key of the local API, by reference.** The entry carries
//!   `--env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}` ([`KEY_REFERENCE`]). The reference is not a
//!   secret: it is a fixed text of this program, which no input of any kind can change. Hermes
//!   resolves it from its own secret scope when it starts the connector (observed on v0.22.5,
//!   Docker, Linux, 15/11/2026, `referencia/hermes-fatos.md`). The connector never reads the
//!   secrets of Hermes and never restarts Hermes; it creates the key of the local API only in
//!   `join`, only when Hermes has none, through `config set` (`docs/11-conector.md`,
//!   section 6). An `add`
//!   that rejects `--env` with a non-zero exit is tried again without it
//!   ([`Registration::WithoutKeyReference`]): the connection never fails for lack of the
//!   option. An `add` that silently ignores `--env` cannot be seen from here; `status` then
//!   says the key was not received.
//! - **The environment of the command of Hermes** is the one of the terminal that runs the
//!   connector, minus `API_SERVER_KEY` and `ARAUTO_HERMES_API_KEY`: no layer between the
//!   connector and Hermes (a shim that re-parses its arguments through a shell, say) can turn
//!   the reference into the key. The rest stays, because Hermes needs to find its own folder.
//! - **The tools of a woken run** (proposal, 18/11/2026). From `join` and `status` the connector
//!   reads the one setting [`TOOLS_SETTING`] with its output (`config get`, up to 64 KiB, parsed
//!   in `toolsets` and dropped: never printed, logged or put in an error), and only `join`
//!   writes it, by appending `arauto` to an explicit list that lacks it (`config set`). Both
//!   take fixed arguments and go through the same `run`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

use crate::home::Home;
use crate::settings::{Settings, duration};

/// The name of the server of tools in Hermes.
pub const SERVER_NAME: &str = "arauto";

/// The setting of Hermes that holds the key of its local API (observed on v0.22.5).
pub const HERMES_KEY_VARIABLE: &str = "API_SERVER_KEY";

/// The setting of Hermes that lists the tools of the runs of its local API.
pub(crate) const TOOLS_SETTING: &str = "platform_toolsets.api_server";

/// The reference the entry carries so that Hermes hands its key to the connector. Fixed text,
/// not a secret: `${` and `}` are literal characters of the constant.
pub(crate) const KEY_REFERENCE: &str = "ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}";

/// The variables the command of Hermes never receives from this program.
const WITHHELD: &[&str] = &[crate::wake::KEY_VARIABLE, HERMES_KEY_VARIABLE];

/// The program run when `ARAUTO_HERMES_BIN` is not set.
pub const DEFAULT_PROGRAM: &str = "hermes";

/// What `add` types for the questions of Hermes: the first `y` is "overwrite" when the entry
/// exists and "enable all the tools" otherwise, the second is "enable all" after an overwrite.
const ANSWERS: &[u8] = b"y\ny\n";

/// How much of the list `add` keeps to look for its row.
pub(crate) const LIST_CEILING: usize = 64 * 1024;

/// Why the registration failed: AR-601.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HermesError {
    /// A path that cannot be an argument.
    #[error("a path of the connector cannot be given to Hermes")]
    Path,
    /// The program of Hermes is a batch file, which runs through an interpreter.
    #[error("the program of Hermes is a batch file")]
    Interpreter,
    /// There is no program of Hermes in the `PATH` nor in the fixed places.
    #[error("the program of Hermes was not found")]
    NotFound,
    /// The program could not be started.
    #[error("the program of Hermes could not be started")]
    Start,
    /// It did not end in time.
    #[error("the program of Hermes did not end in time")]
    TimedOut,
    /// It refused.
    #[error("the program of Hermes refused")]
    Refused,
    /// The command ran and the entry is not in the list.
    #[error("the entry was not saved by Hermes")]
    NotSaved,
    /// The entry is in the list, disabled.
    #[error("the entry was saved disabled by Hermes")]
    Disabled,
}

impl HermesError {
    /// What the command says: AR-601, and for a missing program a sentence of its own (also
    /// AR-601, same exit code) that says where to look.
    pub(crate) fn said(&self) -> crate::catalog::Said {
        use crate::catalog::{Local, Said};
        match self {
            HermesError::NotFound => Said::Local(Local::HermesNotFound),
            _ => Said::code(arauto_protocol::errors::ErrorCode::Ar601),
        }
    }
}

/// The command of Hermes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hermes {
    program: OsString,
    /// Whether `ARAUTO_HERMES_BIN` named it: then it is the only program tried.
    explicit: bool,
}

impl Default for Hermes {
    fn default() -> Self {
        Self {
            program: OsString::from(DEFAULT_PROGRAM),
            explicit: false,
        }
    }
}

/// What the list says of the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Listed {
    /// No row of the server.
    Missing,
    /// A row, enabled.
    Enabled,
    /// A row that says `disabled`.
    Disabled,
}

/// What was saved by `add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registration {
    /// The entry carries the reference to the key of the local API of Hermes.
    WithKeyReference,
    /// This Hermes refused the option, so the entry was saved without the reference.
    WithoutKeyReference,
}

/// How one try of `add` ended.
enum Once {
    Saved,
    /// The `add` run itself exited with a code that is not 0.
    AddExitedNonZero,
    Failed(HermesError),
}

/// A setting of Hermes the key step of `join` asks about, with `hermes config get NAME`. Only
/// the exit code is used: 0 is set, 1 is not set (`docs/11-conector.md`, section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Setting {
    /// The key of the local API.
    ApiServerKey,
    /// The host of the local API, in the environment file.
    ApiServerHost,
    /// The block of the local API under `gateway.platforms` (one call covers the block under
    /// `platforms` too).
    PlatformsBlock,
    /// The host, in the shorthand block `gateway.api_server`.
    ShorthandHost,
    /// The extra fields, where a host may also be, in the shorthand block.
    ShorthandExtra,
}

impl Setting {
    /// The four whose answer decides the address guard, in the order they are asked.
    pub(crate) const ADDRESS: [Setting; 4] = [
        Setting::ApiServerHost,
        Setting::PlatformsBlock,
        Setting::ShorthandHost,
        Setting::ShorthandExtra,
    ];

    /// The name Hermes knows it by.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Setting::ApiServerKey => "API_SERVER_KEY",
            Setting::ApiServerHost => "API_SERVER_HOST",
            Setting::PlatformsBlock => "gateway.platforms.api_server",
            Setting::ShorthandHost => "gateway.api_server.host",
            Setting::ShorthandExtra => "gateway.api_server.extra",
        }
    }
}

/// The time the key step of `join` has in all: every call of Hermes and the probe draw from it.
pub(crate) struct Budget {
    end: tokio::time::Instant,
}

impl Budget {
    pub(crate) fn new(ms: i64) -> Self {
        Self {
            end: tokio::time::Instant::now() + duration(ms),
        }
    }

    /// What is left of the step.
    pub(crate) fn left(&self) -> std::time::Duration {
        self.end
            .saturating_duration_since(tokio::time::Instant::now())
    }

    /// The limit of one call of Hermes: its own, or what is left of the step.
    pub(crate) fn call_limit(&self, settings: &Settings) -> std::time::Duration {
        self.left().min(duration(settings.hermes_ms))
    }
}

/// How a run of the program ended.
struct Ran {
    success: bool,
    /// The exit code; none when the program was ended by a signal.
    code: Option<i32>,
    /// The start of the standard output, when it was asked for.
    captured: Vec<u8>,
}

impl Hermes {
    /// The command named by `ARAUTO_HERMES_BIN`, or `hermes`.
    #[must_use]
    pub fn new(program: Option<OsString>) -> Self {
        match program {
            Some(program) if !program.is_empty() => Self {
                program,
                explicit: true,
            },
            _ => Self::default(),
        }
    }

    /// `hermes mcp add arauto --command PROGRAM --env REFERENCE --args mcp --home HOME`,
    /// answered, and then checked in `hermes mcp list`. When this Hermes rejects the option
    /// (the `add` exits with a code that is not 0), the same without the reference.
    pub(crate) async fn add(
        &self,
        home: &Home,
        settings: &Settings,
    ) -> Result<Registration, HermesError> {
        match self.add_once(home, settings, true).await {
            Once::Saved => return Ok(Registration::WithKeyReference),
            Once::Failed(error) => return Err(error),
            Once::AddExitedNonZero => {}
        }
        match self.add_once(home, settings, false).await {
            Once::Saved => Ok(Registration::WithoutKeyReference),
            Once::AddExitedNonZero => Err(HermesError::Refused),
            Once::Failed(error) => Err(error),
        }
    }

    async fn add_once(&self, home: &Home, settings: &Settings, reference: bool) -> Once {
        let arguments = match add_arguments(&home.program(), Some(home.root()), reference) {
            Ok(arguments) => arguments,
            Err(error) => return Once::Failed(error),
        };
        #[cfg(feature = "testing")]
        if let Some(answer) = &settings.testing.hermes {
            return if answer(&arguments) {
                Once::Saved
            } else {
                Once::AddExitedNonZero
            };
        }
        let limit = duration(settings.hermes_ms);
        let added = match self.run(&arguments, limit, Some(ANSWERS), false).await {
            Ok(added) => added,
            Err(error) => return Once::Failed(error),
        };
        if !added.success {
            return Once::AddExitedNonZero;
        }
        let listed = match self.run(&list_arguments(), limit, None, true).await {
            Ok(listed) => listed,
            Err(error) => return Once::Failed(error),
        };
        if !listed.success {
            return Once::Failed(HermesError::Refused);
        }
        match registration_in(&listed.captured) {
            Listed::Enabled => Once::Saved,
            Listed::Disabled => {
                // Best effort: a disabled entry is no registration, and it would be asked
                // about again ("overwrite?") by the next try.
                let _ = self.remove(settings).await;
                Once::Failed(HermesError::Disabled)
            }
            Listed::Missing => Once::Failed(HermesError::NotSaved),
        }
    }

    /// `hermes mcp remove arauto`.
    pub(crate) async fn remove(&self, settings: &Settings) -> Result<(), HermesError> {
        #[cfg(feature = "testing")]
        if let Some(answer) = &settings.testing.hermes {
            return if answer(&remove_arguments()) {
                Ok(())
            } else {
                Err(HermesError::Refused)
            };
        }
        let limit = duration(settings.hermes_ms);
        let removed = self.run(&remove_arguments(), limit, None, false).await?;
        if removed.success {
            Ok(())
        } else {
            Err(HermesError::Refused)
        }
    }

    /// `hermes config get NAME`, by its exit code alone: 0 is set, 1 is not set, anything else
    /// (another code, not started, ended by force) is an error, which the caller reads as "do
    /// not know". Nothing Hermes prints is read.
    pub(crate) async fn is_set(
        &self,
        setting: Setting,
        limit: std::time::Duration,
    ) -> Result<bool, HermesError> {
        let arguments: Vec<OsString> = ["config", "get", setting.name()]
            .into_iter()
            .map(OsString::from)
            .collect();
        match self.run(&arguments, limit, None, false).await?.code {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(HermesError::Refused),
        }
    }

    /// `hermes config get platform_toolsets.api_server`, with its output: `Ok(None)` for exit 1
    /// (no row), `Ok(Some(bytes))` for exit 0 with the start of the standard output (64 KiB),
    /// `Err` for anything else, an output that filled the ceiling (it may be cut) included. The bytes
    /// are only parsed by the caller: never logged or shown.
    pub(crate) async fn read_tool_list(
        &self,
        limit: std::time::Duration,
    ) -> Result<Option<Vec<u8>>, HermesError> {
        let arguments: Vec<OsString> = ["config", "get", TOOLS_SETTING]
            .into_iter()
            .map(OsString::from)
            .collect();
        let ran = self.run(&arguments, limit, None, true).await?;
        match ran.code {
            // An output that filled the ceiling may have been cut: it is not read.
            Some(0) if ran.captured.len() >= LIST_CEILING => Err(HermesError::Refused),
            Some(0) => Ok(Some(ran.captured)),
            Some(1) => Ok(None),
            _ => Err(HermesError::Refused),
        }
    }

    /// `hermes config set platform_toolsets.api_server VALUE`: `Ok(true)` exit 0, `Ok(false)` an
    /// exit code that is not 0, `Err` not started, timed out or ended by a signal (it may have
    /// written: the caller cannot tell).
    pub(crate) async fn write_tool_list(
        &self,
        value: &str,
        limit: std::time::Duration,
    ) -> Result<bool, HermesError> {
        let arguments: Vec<OsString> = vec![
            OsString::from("config"),
            OsString::from("set"),
            OsString::from(TOOLS_SETTING),
            OsString::from(value),
        ];
        match self.run(&arguments, limit, None, false).await?.code {
            Some(0) => Ok(true),
            Some(_) => Ok(false),
            None => Err(HermesError::Refused),
        }
    }

    /// `hermes config set API_SERVER_KEY KEY`: the one place the key leaves the memory of the
    /// connector, and only as the last argument of this command (`docs/11-conector.md`, section
    /// 6). Success is the exit code 0 and nothing else.
    pub(crate) async fn save_key(
        &self,
        key: &Zeroizing<String>,
        limit: std::time::Duration,
    ) -> Result<(), HermesError> {
        let arguments: Vec<OsString> = vec![
            OsString::from("config"),
            OsString::from("set"),
            OsString::from(Setting::ApiServerKey.name()),
            OsString::from(key.as_str()),
        ];
        let saved = self.run(&arguments, limit, None, false).await?;
        if saved.success {
            Ok(())
        } else {
            Err(HermesError::Refused)
        }
    }

    /// The fixed places tried after `hermes` is not found: none when the program was named. On
    /// Unix the three of [`fallback_candidates`]; on Windows the launcher in `LOCALAPPDATA`.
    fn fallbacks(&self) -> Vec<PathBuf> {
        if self.explicit {
            return Vec::new();
        }
        #[cfg(unix)]
        {
            let user = arauto_platform::default_home()
                .ok()
                .and_then(|home| home.parent().map(Path::to_path_buf));
            fallback_candidates(user.as_deref())
        }
        #[cfg(windows)]
        {
            let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
            windows_candidate(local.as_deref()).into_iter().collect()
        }
        #[cfg(not(any(unix, windows)))]
        {
            Vec::new()
        }
    }

    async fn run(
        &self,
        arguments: &[OsString],
        limit: std::time::Duration,
        answers: Option<&[u8]>,
        capture: bool,
    ) -> Result<Ran, HermesError> {
        if is_batch_file(&self.program) {
            return Err(HermesError::Interpreter);
        }
        if limit.is_zero() {
            return Err(HermesError::TimedOut);
        }
        let mut programs = vec![self.program.clone()];
        programs.extend(self.fallbacks().into_iter().map(PathBuf::into_os_string));
        run_first(&programs, self.explicit, arguments, limit, answers, capture).await
    }
}

/// The fixed absolute places of the program of Hermes, in the order they are tried: the shim of
/// the Docker install, the launcher inside its virtual environment, and the launcher of the
/// install script of the user. The last one is left out when the folder of the user is unknown
/// or is not absolute.
#[cfg_attr(not(unix), allow(dead_code))]
fn fallback_candidates(user_home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/hermes/bin/hermes"),
        PathBuf::from("/opt/hermes/.venv/bin/hermes"),
    ];
    if let Some(home) = user_home.filter(|home| home.is_absolute()) {
        candidates.push(home.join(".local").join("bin").join("hermes"));
    }
    candidates
}

/// The launcher of Hermes on native Windows: `hermes\bin\hermes.exe` inside `LOCALAPPDATA`
/// (the default of `HERMES_HOME`; read in its documentation, never run). `None` when the
/// variable is unset or its value is not an absolute path.
#[cfg(windows)]
fn windows_candidate(local_app_data: Option<&Path>) -> Option<PathBuf> {
    local_app_data
        .filter(|folder| folder.is_absolute())
        .map(|folder| folder.join("hermes").join("bin").join("hermes.exe"))
}

/// Starts the first of `programs` that exists and runs it to its end, within `limit`, and kills
/// it when the time is up: `answers` on its input (closed after them; null without them), its
/// output kept up to the ceiling when `capture` is set (null otherwise), its error output null.
async fn run_first(
    programs: &[OsString],
    explicit: bool,
    arguments: &[OsString],
    limit: std::time::Duration,
    answers: Option<&[u8]>,
    capture: bool,
) -> Result<Ran, HermesError> {
    let mut child = None;
    for program in programs {
        let mut command = tokio::process::Command::new(program);
        command
            .args(arguments)
            .stdin(if answers.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(if capture {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::null())
            .kill_on_drop(true);
        for name in WITHHELD {
            command.env_remove(name);
        }
        match command.spawn() {
            Ok(started) => {
                child = Some(started);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(HermesError::Start),
        }
    }
    let Some(mut child) = child else {
        return Err(if explicit {
            HermesError::Start
        } else {
            HermesError::NotFound
        });
    };
    let work = async {
        if let (Some(mut input), Some(answers)) = (child.stdin.take(), answers) {
            // Hermes may end before it reads everything: that is not an error here.
            let _ = input.write_all(answers).await;
            let _ = input.shutdown().await;
        }
        let mut captured = Vec::new();
        if let Some(mut output) = child.stdout.take() {
            // Everything is read, so that Hermes never blocks on a full pipe, and only the
            // start is kept.
            let mut chunk = [0_u8; 4096];
            while let Ok(count) = output.read(&mut chunk).await {
                if count == 0 {
                    break;
                }
                let room = LIST_CEILING.saturating_sub(captured.len());
                captured.extend_from_slice(&chunk[..count.min(room)]);
            }
        }
        child.wait().await.map(|status| Ran {
            success: status.success(),
            code: status.code(),
            captured,
        })
    };
    match tokio::time::timeout(limit, work).await {
        Ok(Ok(ran)) => Ok(ran),
        Ok(Err(_)) => Err(HermesError::Refused),
        Err(_) => {
            let _ = child.kill().await;
            Err(HermesError::TimedOut)
        }
    }
}

/// What the list of `hermes mcp list` says of the server: the row is the line whose first word
/// is exactly its name. Nothing else in a line counts: not another server whose name starts
/// the same, not a column that holds the name.
pub(crate) fn registration_in(list: &[u8]) -> Listed {
    let text = String::from_utf8_lossy(list);
    for line in text.lines() {
        if line.split_whitespace().next() == Some(SERVER_NAME) {
            return if line.contains("disabled") {
                Listed::Disabled
            } else {
                Listed::Enabled
            };
        }
    }
    Listed::Missing
}

/// Whether `program` would run through the interpreter of commands of Windows. Windows drops
/// the dots and spaces at the end of a name: `hermes.bat.` is `hermes.bat`.
fn is_batch_file(program: &OsString) -> bool {
    let lower = program
        .to_string_lossy()
        .trim_end_matches(['.', ' '])
        .to_ascii_lowercase();
    lower.ends_with(".bat") || lower.ends_with(".cmd")
}

/// A path as an argument: absolute, text, and without a control character.
fn path_argument(path: &Path) -> Result<OsString, HermesError> {
    let text = path.to_str().ok_or(HermesError::Path)?;
    if !path.is_absolute() || text.chars().any(char::is_control) || text.starts_with('-') {
        return Err(HermesError::Path);
    }
    Ok(OsString::from(text))
}

/// The arguments of the registration, with the reference to the key or without it.
pub(crate) fn add_arguments(
    program: &Path,
    home: Option<&Path>,
    reference: bool,
) -> Result<Vec<OsString>, HermesError> {
    let mut arguments: Vec<OsString> = ["mcp", "add", SERVER_NAME, "--command"]
        .into_iter()
        .map(OsString::from)
        .collect();
    arguments.push(path_argument(program)?);
    if reference {
        // `--args` must stay the last option.
        arguments.push(OsString::from("--env"));
        arguments.push(OsString::from(KEY_REFERENCE));
    }
    arguments.push(OsString::from("--args"));
    arguments.push(OsString::from("mcp"));
    if let Some(home) = home {
        arguments.push(OsString::from("--home"));
        arguments.push(path_argument(home)?);
    }
    Ok(arguments)
}

/// The arguments of the list.
pub(crate) fn list_arguments() -> Vec<OsString> {
    ["mcp", "list"].into_iter().map(OsString::from).collect()
}

/// The arguments of the removal.
pub(crate) fn remove_arguments() -> Vec<OsString> {
    ["mcp", "remove", SERVER_NAME]
        .into_iter()
        .map(OsString::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use arauto_testkit::hermes::{FakeHermes, built_example, registry};

    use super::*;

    fn absolute(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    fn fake() -> FakeHermes {
        FakeHermes::new(&built_example("fake_hermes"))
    }

    fn hermes_of(fake: &FakeHermes) -> Hermes {
        Hermes::new(Some(fake.program().into_os_string()))
    }

    fn home_in(dir: &tempfile::TempDir) -> Home {
        Home::at(dir.path()).unwrap()
    }

    #[test]
    fn the_key_reference_is_fixed() {
        assert_eq!(
            KEY_REFERENCE,
            format!("{}=${{{}}}", crate::wake::KEY_VARIABLE, HERMES_KEY_VARIABLE)
        );
        assert_eq!(KEY_REFERENCE, "ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}");
        assert_eq!(WITHHELD, ["ARAUTO_HERMES_API_KEY", "API_SERVER_KEY"]);
    }

    #[test]
    fn the_registration_is_the_official_command() {
        let program = absolute("arauto-connector");
        // Without the reference: exactly the list from before the reference existed.
        let arguments = add_arguments(&program, None, false).unwrap();
        let expected: Vec<OsString> = vec![
            "mcp".into(),
            "add".into(),
            "arauto".into(),
            "--command".into(),
            program.clone().into_os_string(),
            "--args".into(),
            "mcp".into(),
        ];
        assert_eq!(arguments, expected);
        let home = absolute("another-home");
        let arguments = add_arguments(&program, Some(&home), false).unwrap();
        assert_eq!(
            &arguments[7..],
            &[OsString::from("--home"), home.clone().into_os_string()]
        );
        // With it: `--env` and its value are two items, and `--args` stays last.
        let arguments = add_arguments(&program, Some(&home), true).unwrap();
        let expected: Vec<OsString> = vec![
            "mcp".into(),
            "add".into(),
            "arauto".into(),
            "--command".into(),
            program.clone().into_os_string(),
            "--env".into(),
            "ARAUTO_HERMES_API_KEY=${API_SERVER_KEY}".into(),
            "--args".into(),
            "mcp".into(),
            "--home".into(),
            home.into_os_string(),
        ];
        assert_eq!(arguments, expected);
        assert_eq!(
            remove_arguments(),
            vec![
                OsString::from("mcp"),
                OsString::from("remove"),
                OsString::from("arauto")
            ]
        );
        assert_eq!(
            list_arguments(),
            vec![OsString::from("mcp"), OsString::from("list")]
        );
    }

    /// No argument that could be read as something else: relative paths, control characters.
    #[test]
    fn a_path_that_could_inject_is_refused() {
        assert!(matches!(
            add_arguments(Path::new("relative"), None, true),
            Err(HermesError::Path)
        ));
        assert!(matches!(
            add_arguments(&absolute("x\nmcp add evil"), None, true),
            Err(HermesError::Path)
        ));
        assert!(matches!(
            add_arguments(&absolute("ok"), Some(&absolute("a\u{7}b")), true),
            Err(HermesError::Path)
        ));
        assert!(matches!(
            add_arguments(&absolute("ok"), Some(Path::new("-x")), false),
            Err(HermesError::Path)
        ));
    }

    #[test]
    fn a_batch_file_is_never_run() {
        for name in [
            "hermes.bat",
            "HERMES.CMD",
            "x.Bat",
            "hermes.bat.",
            "hermes.cmd . ",
        ] {
            assert!(is_batch_file(&OsString::from(name)), "{name}");
        }
        assert!(!is_batch_file(&OsString::from("hermes")));
        assert!(!is_batch_file(&OsString::from("hermes.exe")));
        assert_eq!(Hermes::new(None), Hermes::default());
        assert_eq!(Hermes::new(Some(OsString::new())), Hermes::default());
    }

    #[tokio::test]
    async fn a_program_that_does_not_exist_is_a_failed_registration() {
        let hermes = Hermes::new(Some(absolute("no-such-hermes-program").into_os_string()));
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        let settings = Settings::default();
        assert!(matches!(
            hermes.add(&home, &settings).await,
            Err(HermesError::Start)
        ));
        let batch = Hermes::new(Some(absolute("hermes.cmd").into_os_string()));
        assert!(matches!(
            batch.remove(&settings).await,
            Err(HermesError::Interpreter)
        ));
    }

    /// The row is the line whose first word is the name, and it must not say `disabled`.
    #[test]
    fn the_list_is_read_for_its_own_row() {
        let enabled = "  Name  Transport  Tools  Status\n  arauto           /opt/data/.arauto/arauto-connector   all          \u{2713} enabled\n";
        assert_eq!(registration_in(enabled.as_bytes()), Listed::Enabled);
        let disabled = "  Name  Transport  Tools  Status\n  arauto           /opt/x/arauto-connector   all          disabled\n";
        assert_eq!(registration_in(disabled.as_bytes()), Listed::Disabled);
        assert_eq!(registration_in(b""), Listed::Missing);
        assert_eq!(
            registration_in(
                b"  Name  Transport  Tools  Status\n  other   /x/y   all   \xe2\x9c\x93 enabled\n"
            ),
            Listed::Missing
        );
        // Another server whose name only starts the same.
        assert_eq!(
            registration_in(b"  arauto2     /x/y   all   enabled\n"),
            Listed::Missing
        );
        // The name in another column, or in the middle of a line.
        assert_eq!(
            registration_in(
                b"  other     /opt/arauto/arauto-connector   all   enabled\n  x arauto enabled\n"
            ),
            Listed::Missing
        );
        // Not text: it does not panic, and says nothing.
        assert_eq!(registration_in(&[0xff, 0xfe, b'\n', 0x00]), Listed::Missing);
    }

    #[test]
    fn the_fixed_places_come_in_order_and_are_absolute() {
        let user = std::env::temp_dir();
        let candidates = fallback_candidates(Some(&user));
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/opt/hermes/bin/hermes"),
                PathBuf::from("/opt/hermes/.venv/bin/hermes"),
                user.join(".local").join("bin").join("hermes"),
            ]
        );
        // Without the folder of the user, or with one that is not absolute, it is left out.
        assert_eq!(fallback_candidates(None).len(), 2);
        assert_eq!(fallback_candidates(Some(Path::new("relative"))).len(), 2);
        #[cfg(unix)]
        {
            for candidate in &candidates {
                assert!(candidate.is_absolute(), "{candidate:?}");
            }
            let found = Hermes::default().fallbacks();
            assert!(found.len() >= 2 && found[0] == candidates[0], "{found:?}");
            assert!(found.iter().all(|path| path.is_absolute()));
        }
    }

    #[test]
    fn a_named_program_has_no_other_place() {
        assert!(
            Hermes::new(Some(OsString::from("/x/hermes")))
                .fallbacks()
                .is_empty()
        );
        #[cfg(not(any(unix, windows)))]
        assert!(Hermes::default().fallbacks().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_launcher_is_one_absolute_place_in_the_local_app_data_folder() {
        let local = std::env::temp_dir();
        assert_eq!(
            windows_candidate(Some(&local)),
            Some(local.join("hermes").join("bin").join("hermes.exe"))
        );
        // Unset, or not absolute: left out.
        assert_eq!(windows_candidate(None), None);
        assert_eq!(windows_candidate(Some(Path::new("relative"))), None);
        // The program of the machine is read from the environment of the machine, and only this
        // one place is tried; the Unix places never appear.
        let found = Hermes::default().fallbacks();
        let here = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let expected: Vec<PathBuf> = windows_candidate(here.as_deref()).into_iter().collect();
        assert_eq!(found, expected);
        // This machine has the folder: the place is really in the list.
        assert_eq!(
            here.is_some_and(|folder| folder.is_absolute()),
            found.len() == 1
        );
        assert!(
            Hermes::new(Some(OsString::from("C:\\x\\hermes.exe")))
                .fallbacks()
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_windows_launcher_never_appears_on_unix() {
        let found = Hermes::default().fallbacks();
        assert!(
            found.iter().all(|path| path.extension().is_none()),
            "{found:?}"
        );
        assert!(found.iter().all(|path| !path.ends_with("hermes.exe")));
    }

    /// The first program that exists wins; the ones the system says are missing are skipped.
    #[tokio::test]
    async fn a_missing_program_is_skipped_and_the_first_that_starts_wins() {
        let fake = fake();
        let limit = duration(Settings::default().hermes_ms);
        let missing = |name: &str| absolute(name).into_os_string();
        let programs = [
            missing("no-such-hermes-one"),
            missing("no-such-hermes-two"),
            fake.program().into_os_string(),
        ];
        let ran = run_first(&programs, false, &list_arguments(), limit, None, true)
            .await
            .unwrap();
        assert!(ran.success);
        assert!(!fake.calls().is_empty());
        // None of them exists: not found when nothing was named, not started when it was.
        let none = [missing("no-such-hermes-one"), missing("no-such-hermes-two")];
        assert!(matches!(
            run_first(&none, false, &list_arguments(), limit, None, true)
                .await
                .map(|_| ()),
            Err(HermesError::NotFound)
        ));
        assert!(matches!(
            run_first(&none[..1], true, &list_arguments(), limit, None, true)
                .await
                .map(|_| ()),
            Err(HermesError::Start)
        ));
    }

    #[test]
    fn a_missing_program_has_its_own_sentence_and_the_rest_are_ar601() {
        use crate::catalog::Said;
        use arauto_protocol::errors::ErrorCode;
        let said = HermesError::NotFound.said();
        assert!(matches!(said, Said::Local(_)));
        assert!(said.to_string().starts_with("AR-601: "));
        assert!(said.to_string().contains("ARAUTO_HERMES_BIN"));
        for error in [
            HermesError::Path,
            HermesError::Interpreter,
            HermesError::Start,
            HermesError::TimedOut,
            HermesError::Refused,
            HermesError::NotSaved,
            HermesError::Disabled,
        ] {
            assert_eq!(error.said(), Said::code(ErrorCode::Ar601), "{error:?}");
        }
    }

    /// `add` against a real fake program that behaves as Hermes does (F1 to F4): it saves only
    /// when it reads `y`, and exits 0 whatever happens.
    #[tokio::test]
    async fn add_is_a_success_only_when_the_list_shows_the_entry_enabled() {
        let settings = Settings::default();
        let dir = tempfile::tempdir().unwrap();
        let home = home_in(&dir);

        // It saves, and what it read is exactly the two answers, then the end of the input.
        let fake = fake();
        let hermes = hermes_of(&fake);
        assert_eq!(
            hermes.add(&home, &settings).await.unwrap(),
            Registration::WithKeyReference
        );
        assert!(fake.registered(SERVER_NAME).is_some());
        let read = std::fs::read(fake.folder().join(registry::INPUT_FILE)).unwrap();
        assert_eq!(read, b"y\ny\n");
        let calls = fake.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1], ["mcp", "list"]);

        // The entry exists: the same two answers overwrite it.
        hermes.add(&home, &settings).await.unwrap();
        assert!(fake.registered(SERVER_NAME).is_some());

        // The command exits 0 and saves nothing: not saved.
        let silent = self::fake();
        std::fs::write(silent.folder().join(registry::CANCEL_FILE), b"").unwrap();
        assert!(matches!(
            hermes_of(&silent).add(&home, &settings).await,
            Err(HermesError::NotSaved)
        ));
        assert!(silent.registered(SERVER_NAME).is_none());

        // The probe fails: Hermes saves it disabled, and it is taken out again.
        let probing = self::fake();
        std::fs::write(probing.folder().join(registry::PROBE_FAILS_FILE), b"").unwrap();
        assert!(matches!(
            hermes_of(&probing).add(&home, &settings).await,
            Err(HermesError::Disabled)
        ));
        assert!(probing.registered(SERVER_NAME).is_none());
        assert_eq!(
            probing.calls().last().unwrap(),
            &["mcp", "remove", "arauto"]
        );

        // The command refuses (exit code not 0): it is a failure, and the list is not asked.
        // It is tried twice, with the reference and without it, because a refusal is how an
        // older Hermes answers an option it does not know.
        let refusing = self::fake();
        refusing.refuse(true);
        assert!(matches!(
            hermes_of(&refusing).add(&home, &settings).await,
            Err(HermesError::Refused)
        ));
        assert_eq!(refusing.calls().len(), 2);
    }

    /// The reference reaches the entry literally: nothing between the connector and the command
    /// of Hermes expands it. On Windows this proves the quoting of the command line.
    #[tokio::test]
    async fn add_records_the_reference_literally() {
        let settings = Settings::default();
        let dir = tempfile::tempdir().unwrap();
        let home = home_in(&dir);
        let fake = fake();
        hermes_of(&fake).add(&home, &settings).await.unwrap();
        let entry = fake.registered(SERVER_NAME).unwrap();
        assert_eq!(
            entry.env,
            std::collections::BTreeMap::from([(
                "ARAUTO_HERMES_API_KEY".to_owned(),
                "${API_SERVER_KEY}".to_owned()
            )])
        );
        assert_eq!(entry.args[0], "mcp");
        let calls = fake.calls();
        assert_eq!(calls[0][5], "--env");
        assert_eq!(calls[0][6], KEY_REFERENCE);
        assert_eq!(calls[0][7], "--args");
    }

    /// A Hermes that does not know `--env` (exit 2 before any question) is connected without it.
    #[tokio::test]
    async fn add_falls_back_when_hermes_does_not_know_env() {
        let settings = Settings::default();
        let dir = tempfile::tempdir().unwrap();
        let home = home_in(&dir);
        let fake = fake();
        fake.without_env_option(true);
        let registration = hermes_of(&fake).add(&home, &settings).await.unwrap();
        assert_eq!(registration, Registration::WithoutKeyReference);
        let calls = fake.calls();
        assert_eq!(calls.len(), 3, "{calls:?}");
        assert!(calls[0].contains(&"--env".to_owned()));
        assert!(!calls[1].contains(&"--env".to_owned()));
        assert_eq!(calls[1][1], "add");
        assert_eq!(calls[2], ["mcp", "list"]);
        let entry = fake.registered(SERVER_NAME).unwrap();
        assert!(entry.env.is_empty());
    }

    /// Only a refusal of the `add` itself is tried again without the reference.
    #[tokio::test]
    async fn other_failures_are_not_retried_without_the_reference() {
        let settings = Settings::default();
        let dir = tempfile::tempdir().unwrap();
        let home = home_in(&dir);
        let adds = |fake: &FakeHermes| {
            fake.calls()
                .iter()
                .filter(|call| call.get(1).is_some_and(|word| word == "add"))
                .count()
        };

        let silent = fake();
        std::fs::write(silent.folder().join(registry::CANCEL_FILE), b"").unwrap();
        assert!(matches!(
            hermes_of(&silent).add(&home, &settings).await,
            Err(HermesError::NotSaved)
        ));
        assert_eq!(adds(&silent), 1);

        let probing = fake();
        std::fs::write(probing.folder().join(registry::PROBE_FAILS_FILE), b"").unwrap();
        assert!(matches!(
            hermes_of(&probing).add(&home, &settings).await,
            Err(HermesError::Disabled)
        ));
        assert_eq!(adds(&probing), 1);

        let missing = Hermes::new(Some(absolute("no-such-hermes-program").into_os_string()));
        assert!(matches!(
            missing.add(&home, &settings).await,
            Err(HermesError::Start)
        ));
        let batch = Hermes::new(Some(absolute("hermes.cmd").into_os_string()));
        assert!(matches!(
            batch.add(&home, &settings).await,
            Err(HermesError::Interpreter)
        ));
    }

    /// A shell between the connector and the command of Hermes (the shim of the Docker install
    /// is a script) hands the argument on unchanged when it quotes it as `"$@"` does.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_shell_shim_passes_the_reference_unchanged() {
        use std::os::unix::fs::PermissionsExt;

        let settings = Settings::default();
        let dir = tempfile::tempdir().unwrap();
        let home = home_in(&dir);
        let fake = fake();
        let shim = fake.folder().join("shim");
        std::fs::write(
            &shim,
            format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", fake.program().display()),
        )
        .unwrap();
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
        let hermes = Hermes::new(Some(shim.into_os_string()));
        hermes.add(&home, &settings).await.unwrap();
        assert_eq!(fake.calls()[0][6], KEY_REFERENCE);
    }
}
