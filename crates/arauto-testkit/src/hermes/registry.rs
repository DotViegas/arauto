//! The program `ARAUTO_HERMES_BIN` points to in the tests: what the command of Hermes does with
//! the registration of a server of tools (`referencia/hermes-fatos.md`), kept in the folder of
//! the program.
//!
//! ```text
//! hermes mcp add NAME --command PROGRAM --args ARGUMENT...   keeps NAME with PROGRAM and the ARGUMENTs
//! hermes mcp remove NAME                                     drops NAME; a NAME it does not have is refused
//! anything else                                              refused
//! ```
//!
//! - Every call is first appended to [`CALLS_FILE`], one JSON array of the arguments per line.
//! - With a file [`REFUSE_FILE`] in the folder, every call is refused, as a Hermes that turns
//!   the entry down.
//! - The servers registered are in [`SERVERS_FILE`]: `{"NAME": {"command": ..., "args": [...]}}`.
//!   Adding a name that is there replaces it.
//!
//! Exit codes: 0 done, 1 refused, 2 not a command it knows, 3 its folder could not be used.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Map, Value, json};

/// Every call, one JSON array of the arguments per line.
pub const CALLS_FILE: &str = "calls.jsonl";

/// The servers of tools registered.
pub const SERVERS_FILE: &str = "servers.json";

/// While this file exists, every call is refused.
pub const REFUSE_FILE: &str = "refuse";

/// Done.
pub const DONE: u8 = 0;
/// Refused.
pub const REFUSED: u8 = 1;
/// Not a command it knows.
pub const USAGE: u8 = 2;
/// The folder could not be used.
pub const BROKEN: u8 = 3;

/// The program: its records live in the folder of its executable.
#[must_use]
pub fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let folder = std::env::current_exe()
        .ok()
        .and_then(|program| program.parent().map(Path::to_path_buf));
    let code = match folder {
        Some(folder) => run(&folder, &arguments),
        None => BROKEN,
    };
    ExitCode::from(code)
}

/// One call with `arguments`, with the records in `folder`. Returns the exit code.
#[must_use]
pub fn run(folder: &Path, arguments: &[String]) -> u8 {
    if record(folder, arguments).is_err() {
        return BROKEN;
    }
    if folder.join(REFUSE_FILE).exists() {
        return REFUSED;
    }
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        [
            "mcp",
            "add",
            name,
            "--command",
            command,
            "--args",
            args @ ..,
        ] => {
            let entry = json!({"command": command, "args": args});
            change(folder, |servers| {
                servers.insert((*name).to_owned(), entry);
                DONE
            })
        }
        ["mcp", "remove", name] => change(folder, |servers| {
            if servers.remove(*name).is_some() {
                DONE
            } else {
                REFUSED
            }
        }),
        _ => USAGE,
    }
}

fn record(folder: &Path, arguments: &[String]) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(arguments).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut calls = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join(CALLS_FILE))?;
    calls.write_all(&line)?;
    calls.sync_all()
}

/// Reads the servers, lets `work` change them and writes them back whole, by a temporary file
/// and a rename.
fn change(folder: &Path, work: impl FnOnce(&mut Map<String, Value>) -> u8) -> u8 {
    let Ok(mut servers) = servers(folder) else {
        return BROKEN;
    };
    let code = work(&mut servers);
    if code != DONE {
        return code;
    }
    let path = folder.join(SERVERS_FILE);
    let temporary: PathBuf = folder.join(format!("{SERVERS_FILE}.tmp"));
    let Ok(bytes) = serde_json::to_vec(&Value::Object(servers)) else {
        return BROKEN;
    };
    if std::fs::write(&temporary, bytes).is_err() || std::fs::rename(&temporary, &path).is_err() {
        return BROKEN;
    }
    DONE
}

/// The servers registered in `folder`.
///
/// # Errors
/// The file exists and is not a JSON object.
pub fn servers(folder: &Path) -> std::io::Result<Map<String, Value>> {
    match std::fs::read(folder.join(SERVERS_FILE)) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(Value::Object(servers)) => Ok(servers),
            _ => Err(std::io::Error::other("servers.json is not an object")),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn the_registration_is_kept_and_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        assert_eq!(
            run(
                folder,
                &words("mcp add arauto --command /p/c --args mcp --home /h")
            ),
            DONE
        );
        let kept = servers(folder).unwrap();
        assert_eq!(
            kept["arauto"],
            json!({"command": "/p/c", "args": ["mcp", "--home", "/h"]})
        );
        assert_eq!(run(folder, &words("mcp remove arauto")), DONE);
        assert!(servers(folder).unwrap().is_empty());
        assert_eq!(run(folder, &words("mcp remove arauto")), REFUSED);
        assert_eq!(run(folder, &words("mcp list")), USAGE);
        std::fs::write(folder.join(REFUSE_FILE), b"").unwrap();
        assert_eq!(
            run(folder, &words("mcp add arauto --command /p/c --args mcp")),
            REFUSED
        );
        assert!(servers(folder).unwrap().is_empty());
        let calls = std::fs::read_to_string(folder.join(CALLS_FILE)).unwrap();
        assert_eq!(calls.lines().count(), 5);
        let first: Vec<String> = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
        assert_eq!(
            first,
            words("mcp add arauto --command /p/c --args mcp --home /h")
        );
    }
}
