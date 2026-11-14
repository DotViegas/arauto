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
//!   terminal of a Hermes may not have it on its `PATH`). The first that starts wins. Nothing
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

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::home::Home;
use crate::settings::{Settings, duration};

/// The name of the server of tools in Hermes.
pub const SERVER_NAME: &str = "arauto";

/// The program run when `ARAUTO_HERMES_BIN` is not set.
pub const DEFAULT_PROGRAM: &str = "hermes";

/// What `add` types for the questions of Hermes: the first `y` is "overwrite" when the entry
/// exists and "enable all the tools" otherwise, the second is "enable all" after an overwrite.
const ANSWERS: &[u8] = b"y\ny\n";

/// How much of the list `add` keeps to look for its row.
const LIST_CEILING: usize = 64 * 1024;

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

/// How a run of the program ended.
struct Ran {
    success: bool,
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

    /// `hermes mcp add arauto --command PROGRAM --args mcp --home HOME`, answered, and then
    /// checked in `hermes mcp list`.
    pub(crate) async fn add(&self, home: &Home, settings: &Settings) -> Result<(), HermesError> {
        let arguments = add_arguments(&home.program(), Some(home.root()))?;
        #[cfg(feature = "testing")]
        if let Some(answer) = &settings.testing.hermes {
            return if answer(&arguments) {
                Ok(())
            } else {
                Err(HermesError::Refused)
            };
        }
        let added = self.run(&arguments, settings, Some(ANSWERS), false).await?;
        if !added.success {
            return Err(HermesError::Refused);
        }
        let listed = self.run(&list_arguments(), settings, None, true).await?;
        if !listed.success {
            return Err(HermesError::Refused);
        }
        match registration_in(&listed.captured) {
            Listed::Enabled => Ok(()),
            Listed::Disabled => {
                // Best effort: a disabled entry is no registration, and it would be asked
                // about again ("overwrite?") by the next try.
                let _ = self.remove(settings).await;
                Err(HermesError::Disabled)
            }
            Listed::Missing => Err(HermesError::NotSaved),
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
        let removed = self.run(&remove_arguments(), settings, None, false).await?;
        if removed.success {
            Ok(())
        } else {
            Err(HermesError::Refused)
        }
    }

    /// The fixed places tried after `hermes` is not found: none when the program was named, and
    /// none off Unix.
    fn fallbacks(&self) -> Vec<PathBuf> {
        if self.explicit || !cfg!(unix) {
            return Vec::new();
        }
        let user = arauto_platform::default_home()
            .ok()
            .and_then(|home| home.parent().map(Path::to_path_buf));
        fallback_candidates(user.as_deref())
    }

    async fn run(
        &self,
        arguments: &[OsString],
        settings: &Settings,
        answers: Option<&[u8]>,
        capture: bool,
    ) -> Result<Ran, HermesError> {
        if is_batch_file(&self.program) {
            return Err(HermesError::Interpreter);
        }
        let mut programs = vec![self.program.clone()];
        programs.extend(self.fallbacks().into_iter().map(PathBuf::into_os_string));
        run_first(
            &programs,
            self.explicit,
            arguments,
            settings,
            answers,
            capture,
        )
        .await
    }
}

/// The fixed absolute places of the program of Hermes, in the order they are tried: the shim of
/// the Docker install, the launcher inside its virtual environment, and the launcher of the
/// install script of the user. The last one is left out when the folder of the user is unknown
/// or is not absolute.
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

/// Starts the first of `programs` that exists and runs it to its end, within the time limit:
/// `answers` on its input (closed after them; null without them), its output kept up to the
/// ceiling when `capture` is set (null otherwise), its error output null.
async fn run_first(
    programs: &[OsString],
    explicit: bool,
    arguments: &[OsString],
    settings: &Settings,
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
            captured,
        })
    };
    match tokio::time::timeout(duration(settings.hermes_ms), work).await {
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

/// The arguments of the registration.
pub(crate) fn add_arguments(
    program: &Path,
    home: Option<&Path>,
) -> Result<Vec<OsString>, HermesError> {
    let mut arguments: Vec<OsString> = ["mcp", "add", SERVER_NAME, "--command"]
        .into_iter()
        .map(OsString::from)
        .collect();
    arguments.push(path_argument(program)?);
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
    fn the_registration_is_the_official_command() {
        let program = absolute("arauto-connector");
        let arguments = add_arguments(&program, None).unwrap();
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
        let arguments = add_arguments(&program, Some(&home)).unwrap();
        assert_eq!(
            &arguments[7..],
            &[OsString::from("--home"), home.into_os_string()]
        );
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
            add_arguments(Path::new("relative"), None),
            Err(HermesError::Path)
        ));
        assert!(matches!(
            add_arguments(&absolute("x\nmcp add evil"), None),
            Err(HermesError::Path)
        ));
        assert!(matches!(
            add_arguments(&absolute("ok"), Some(&absolute("a\u{7}b"))),
            Err(HermesError::Path)
        ));
        assert!(matches!(
            add_arguments(&absolute("ok"), Some(Path::new("-x"))),
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
        #[cfg(not(unix))]
        assert!(Hermes::default().fallbacks().is_empty());
    }

    /// The first program that exists wins; the ones the system says are missing are skipped.
    #[tokio::test]
    async fn a_missing_program_is_skipped_and_the_first_that_starts_wins() {
        let fake = fake();
        let settings = Settings::default();
        let missing = |name: &str| absolute(name).into_os_string();
        let programs = [
            missing("no-such-hermes-one"),
            missing("no-such-hermes-two"),
            fake.program().into_os_string(),
        ];
        let ran = run_first(&programs, false, &list_arguments(), &settings, None, true)
            .await
            .unwrap();
        assert!(ran.success);
        assert!(!fake.calls().is_empty());
        // None of them exists: not found when nothing was named, not started when it was.
        let none = [missing("no-such-hermes-one"), missing("no-such-hermes-two")];
        assert!(matches!(
            run_first(&none, false, &list_arguments(), &settings, None, true)
                .await
                .map(|_| ()),
            Err(HermesError::NotFound)
        ));
        assert!(matches!(
            run_first(&none[..1], true, &list_arguments(), &settings, None, true)
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
        hermes.add(&home, &settings).await.unwrap();
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
        let refusing = self::fake();
        refusing.refuse(true);
        assert!(matches!(
            hermes_of(&refusing).add(&home, &settings).await,
            Err(HermesError::Refused)
        ));
        assert_eq!(refusing.calls().len(), 1);
    }
}
