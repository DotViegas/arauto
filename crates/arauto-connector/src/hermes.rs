//! The registration of the connector in Hermes, by its official command
//! (`docs/11-conector.md`, section 4; `referencia/hermes-fatos.md`):
//!
//! ```text
//! hermes mcp add arauto --command <folder>/arauto-connector --args mcp [--home <folder>]
//! hermes mcp remove arauto
//! ```
//!
//! - The program comes from `ARAUTO_HERMES_BIN`, `hermes` by default, and is run directly: no
//!   interpreter of commands, on any system. A batch file of Windows would be one, so it is
//!   refused.
//! - Every argument is a fixed word or an absolute path of this machine; a path with a control
//!   character, or that is not text, is refused. Nothing that came from the hub or from the
//!   block becomes an argument.
//! - `--home` goes in the arguments when the folder is not the default one: Hermes starts the
//!   connector with a filtered environment, where `ARAUTO_HOME` would not arrive.
//! - What the command prints is not read and not repeated.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use crate::home::Home;
use crate::settings::{Settings, duration};

/// The name of the server of tools in Hermes.
pub const SERVER_NAME: &str = "arauto";

/// The program run when `ARAUTO_HERMES_BIN` is not set.
pub const DEFAULT_PROGRAM: &str = "hermes";

/// Why the registration failed: AR-601.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HermesError {
    /// A path that cannot be an argument.
    #[error("a path of the connector cannot be given to Hermes")]
    Path,
    /// The program of Hermes is a batch file, which runs through an interpreter.
    #[error("the program of Hermes is a batch file")]
    Interpreter,
    /// The program could not be started.
    #[error("the program of Hermes could not be started")]
    Start,
    /// It did not end in time.
    #[error("the program of Hermes did not end in time")]
    TimedOut,
    /// It refused.
    #[error("the program of Hermes refused")]
    Refused,
}

/// The command of Hermes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hermes {
    program: OsString,
}

impl Default for Hermes {
    fn default() -> Self {
        Self {
            program: OsString::from(DEFAULT_PROGRAM),
        }
    }
}

impl Hermes {
    /// The command named by `ARAUTO_HERMES_BIN`, or `hermes`.
    #[must_use]
    pub fn new(program: Option<OsString>) -> Self {
        match program {
            Some(program) if !program.is_empty() => Self { program },
            _ => Self::default(),
        }
    }

    /// `hermes mcp add arauto --command PROGRAM --args mcp [--home HOME]`.
    pub(crate) async fn add(&self, home: &Home, settings: &Settings) -> Result<(), HermesError> {
        let arguments = add_arguments(&home.program(), (!home.is_default()).then(|| home.root()))?;
        self.run(&arguments, settings).await
    }

    /// `hermes mcp remove arauto`.
    pub(crate) async fn remove(&self, settings: &Settings) -> Result<(), HermesError> {
        self.run(&remove_arguments(), settings).await
    }

    async fn run(&self, arguments: &[OsString], settings: &Settings) -> Result<(), HermesError> {
        #[cfg(feature = "testing")]
        if let Some(answer) = &settings.testing.hermes {
            return if answer(arguments) {
                Ok(())
            } else {
                Err(HermesError::Refused)
            };
        }
        if is_batch_file(&self.program) {
            return Err(HermesError::Interpreter);
        }
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|_| HermesError::Start)?;
        match tokio::time::timeout(duration(settings.hermes_ms), child.wait()).await {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(_) => Err(HermesError::Refused),
            Err(_) => {
                let _ = child.kill().await;
                Err(HermesError::TimedOut)
            }
        }
    }
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

/// The arguments of the removal.
pub(crate) fn remove_arguments() -> Vec<OsString> {
    ["mcp", "remove", SERVER_NAME]
        .into_iter()
        .map(OsString::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
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
        #[cfg(not(feature = "testing"))]
        assert!(matches!(
            batch.remove(&settings).await,
            Err(HermesError::Interpreter)
        ));
        #[cfg(feature = "testing")]
        assert!(matches!(
            batch.remove(&settings).await,
            Err(HermesError::Interpreter)
        ));
    }
}
