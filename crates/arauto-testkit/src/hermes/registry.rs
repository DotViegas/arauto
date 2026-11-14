//! The program `ARAUTO_HERMES_BIN` points to in the tests: what the command of Hermes does with
//! the registration of a server of tools (`referencia/hermes-fatos.md`), kept in the folder of
//! the program.
//!
//! ```text
//! hermes mcp add NAME --command PROGRAM --args ARGUMENT...   asks on the input, then keeps NAME
//! hermes mcp list                                            prints the table of the servers
//! hermes mcp remove NAME                                     drops NAME; a NAME it does not have is refused
//! anything else                                              refused
//! ```
//!
//! `add` behaves as Hermes v0.22.5 does (`referencia/hermes-fatos.md`, F1 to F4): it reads its
//! answers from the standard input, one per line, and saves nothing when an answer is not `y`
//! (or the input ends). With an entry of that name already there it first asks to overwrite it,
//! then asks to enable all the tools. With a file [`PROBE_FAILS_FILE`] in the folder the probe
//! of the server fails, the question is whether to save anyway, and the entry is kept
//! **disabled**. With a file [`CANCEL_FILE`] it answers "no" to every question, as a Hermes
//! whose questions nobody answered. Every byte it reads is appended to [`INPUT_FILE`]. The exit
//! code is 0 in all of these cases, as the one of Hermes is.
//!
//! - Every call is first appended to [`CALLS_FILE`], one JSON array of the arguments per line.
//! - With a file [`REFUSE_FILE`] in the folder, every call is refused, as a Hermes that turns
//!   the entry down.
//! - The servers registered are in [`SERVERS_FILE`]:
//!   `{"NAME": {"command": ..., "args": [...], "enabled": true}}`. Adding a name that is there
//!   replaces it.
//!
//! Exit codes: 0 done, 1 refused, 2 not a command it knows, 3 its folder could not be used.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Map, Value, json};

/// Every call, one JSON array of the arguments per line.
pub const CALLS_FILE: &str = "calls.jsonl";

/// The servers of tools registered.
pub const SERVERS_FILE: &str = "servers.json";

/// While this file exists, every call is refused.
pub const REFUSE_FILE: &str = "refuse";

/// While this file exists, the probe of the server fails on `mcp add`.
pub const PROBE_FAILS_FILE: &str = "probe-fails";

/// While this file exists, `mcp add` answers "no" to every question.
pub const CANCEL_FILE: &str = "cancel";

/// What `mcp add` read on its input, appended.
pub const INPUT_FILE: &str = "input.log";

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
        Some(folder) => run(
            &folder,
            &arguments,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
        ),
        None => BROKEN,
    };
    ExitCode::from(code)
}

/// One call with `arguments`, with the records in `folder`, the answers on `input` and what
/// Hermes prints on `output`. Returns the exit code.
#[must_use]
pub fn run(
    folder: &Path,
    arguments: &[String],
    input: &mut dyn Read,
    output: &mut dyn Write,
) -> u8 {
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
        ] => add(folder, name, command, args, input),
        ["mcp", "list"] => list(folder, output),
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

/// `mcp add`: the questions of Hermes, answered from `input`.
fn add(folder: &Path, name: &str, command: &str, args: &[&str], input: &mut dyn Read) -> u8 {
    let mut typed = Vec::new();
    let _ = input.read_to_end(&mut typed);
    if record_input(folder, &typed).is_err() {
        return BROKEN;
    }
    let mut answers = String::from_utf8_lossy(&typed)
        .lines()
        .map(|line| line.trim().eq_ignore_ascii_case("y"))
        .collect::<Vec<_>>()
        .into_iter();
    let mut yes = move || answers.next().unwrap_or(false);
    let cancel = folder.join(CANCEL_FILE).exists();
    let enabled = !folder.join(PROBE_FAILS_FILE).exists();
    change(folder, |servers| {
        if cancel {
            return DONE;
        }
        if servers.contains_key(name) && !yes() {
            return DONE;
        }
        if !yes() {
            return DONE;
        }
        servers.insert(
            name.to_owned(),
            json!({"command": command, "args": args, "enabled": enabled}),
        );
        DONE
    })
}

/// `mcp list`: a table, one row per server, the name first.
fn list(folder: &Path, output: &mut dyn Write) -> u8 {
    let Ok(servers) = servers(folder) else {
        return BROKEN;
    };
    let mut table =
        String::from("  Name             Transport                      Tools        Status\n");
    for (name, entry) in &servers {
        let command = entry["command"].as_str().unwrap_or_default();
        let status = if entry["enabled"].as_bool() == Some(false) {
            "disabled"
        } else {
            "\u{2713} enabled"
        };
        table.push_str(&format!(
            "  {name:<16} {command:<30} all          {status}\n"
        ));
    }
    if output.write_all(table.as_bytes()).is_err() {
        return BROKEN;
    }
    DONE
}

fn record_input(folder: &Path, typed: &[u8]) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join(INPUT_FILE))?
        .write_all(typed)
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
    let before = servers.clone();
    let code = work(&mut servers);
    if code != DONE || servers == before {
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

    fn call(folder: &Path, text: &str, input: &str) -> (u8, String) {
        let mut output = Vec::new();
        let code = run(folder, &words(text), &mut input.as_bytes(), &mut output);
        (code, String::from_utf8(output).unwrap())
    }

    #[test]
    fn the_registration_is_kept_and_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        let add = "mcp add arauto --command /p/c --args mcp --home /h";
        assert_eq!(call(folder, add, "y\ny\n").0, DONE);
        let kept = servers(folder).unwrap();
        assert_eq!(
            kept["arauto"],
            json!({"command": "/p/c", "args": ["mcp", "--home", "/h"], "enabled": true})
        );
        assert!(call(folder, "mcp list", "").1.contains("enabled"));
        assert_eq!(call(folder, "mcp remove arauto", "").0, DONE);
        assert!(servers(folder).unwrap().is_empty());
        assert_eq!(call(folder, "mcp remove arauto", "").0, REFUSED);
        assert_eq!(call(folder, "mcp nothing", "").0, USAGE);
        std::fs::write(folder.join(REFUSE_FILE), b"").unwrap();
        assert_eq!(
            call(folder, "mcp add arauto --command /p/c --args mcp", "y\ny\n").0,
            REFUSED
        );
        assert!(servers(folder).unwrap().is_empty());
        let calls = std::fs::read_to_string(folder.join(CALLS_FILE)).unwrap();
        assert_eq!(calls.lines().count(), 6);
        let first: Vec<String> = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
        assert_eq!(first, words(add));
    }

    /// F1 to F4 of `referencia/hermes-fatos.md`: it saves only when it reads `y`.
    #[test]
    fn it_asks_as_hermes_does() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        let add = "mcp add arauto --command /p/c --args mcp";
        let table = |folder: &Path| call(folder, "mcp list", "").1;
        // The input ends before an answer: cancelled, nothing saved, exit 0 (F1).
        assert_eq!(call(folder, add, "").0, DONE);
        assert!(!table(folder).contains("arauto"));
        assert_eq!(call(folder, add, "n\n").0, DONE);
        assert!(!table(folder).contains("arauto"));
        // A `y` saves it enabled (F2).
        assert_eq!(call(folder, add, "y\n").0, DONE);
        assert!(table(folder).contains("enabled"));
        // An entry that is there: the first question is the overwrite (F3).
        std::fs::write(folder.join(PROBE_FAILS_FILE), b"").unwrap();
        assert_eq!(call(folder, add, "").0, DONE);
        assert!(!table(folder).contains("disabled"));
        assert_eq!(call(folder, add, "n\ny\n").0, DONE);
        assert!(!table(folder).contains("disabled"));
        // The probe fails: it is saved disabled (F4).
        assert_eq!(call(folder, add, "y\ny\n").0, DONE);
        let listed = table(folder);
        assert!(listed.contains("disabled"), "{listed}");
        assert!(!listed.contains("\u{2713}"), "{listed}");
        // Every byte it read was kept.
        let read = std::fs::read_to_string(folder.join(INPUT_FILE)).unwrap();
        assert_eq!(read, "n\ny\nn\ny\ny\ny\n");
    }
}
