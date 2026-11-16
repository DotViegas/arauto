//! The program `ARAUTO_HERMES_BIN` points to in the tests: what the command of Hermes does with
//! the registration of a server of tools (`referencia/hermes-fatos.md`), kept in the folder of
//! the program.
//!
//! ```text
//! hermes mcp add NAME --command PROGRAM [--env KEY=VALUE]... --args ARGUMENT...
//!                                                            asks on the input, then keeps NAME
//! hermes mcp list                                            prints the table of the servers
//! hermes mcp remove NAME                                     drops NAME; a NAME it does not have is refused
//! hermes config get NAME                                     exit 0 when NAME is set, 1 when it is not, no output
//! hermes config set NAME VALUE                               keeps VALUE in the secrets, exit 0, no output
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
//! - `--env KEY=VALUE` may come several times, before `--args` (which stays last, as in Hermes
//!   v0.22.5, `referencia/hermes-fatos.md`). The pair is kept **literally** in the `env` map of
//!   the entry: the reference `${NAME}` is resolved only when a copy is started
//!   ([`crate::hermes::FakeHermes::start`]). A `KEY` out of `[A-Za-z_][A-Za-z0-9_]*` is a usage
//!   error. With a file [`NO_ENV_FILE`] in the folder an `add` that has `--env` exits with the
//!   usage code before any question and saves nothing, as an older Hermes whose argument parser
//!   does not know the option.
//! - Every call is first appended to [`CALLS_FILE`], one JSON array of the arguments per line.
//! - Every call also appends to [`ENVIRONMENT_FILE`] the sorted **names** (never the values) of
//!   its own environment, one JSON array per line.
//! - With a file [`REFUSE_FILE`] in the folder, every call is refused, as a Hermes that turns
//!   the entry down.
//! - The servers registered are in [`SERVERS_FILE`]:
//!   `{"NAME": {"command": ..., "args": [...], "enabled": true}}`. Adding a name that is there
//!   replaces it.
//!
//! **`config get NAME`** exits 0 when the setting holds a value and 1 when it does not, and
//! prints nothing (as Hermes v0.22.5 does for the exit code; the connector reads nothing else).
//! `NAME` is set when it is in [`SECRETS_FILE`], or in the list of [`CONFIG_SET_FILE`], or, when
//! [`NO_KEY_FILE`] is absent, when it is `API_SERVER_KEY`: a fake Hermes has a key unless a test
//! says otherwise, so that the tests that are not about the key never make one.
//! **`config set NAME VALUE`** writes VALUE in [`SECRETS_FILE`], which is also where `${NAME}`
//! of an entry resolves from. Three files change the `config` calls only: [`GET_FAILS_FILE`]
//! (`get` exits with the usage code), [`SET_FAILS_FILE`] (`set` exits 1 and saves nothing) and
//! [`HANG_FILE`] (the call named in the file, `get` or `set`, does not end until it is killed,
//! the file is removed or two minutes pass).
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

/// While this file exists, `mcp add` does not know the option `--env`.
pub const NO_ENV_FILE: &str = "no-env";

/// The names of the environment of each call, one JSON array per line.
pub const ENVIRONMENT_FILE: &str = "environment.jsonl";

/// The secret scope of the fake Hermes: what `${NAME}` in an entry resolves from.
pub const SECRETS_FILE: &str = "secrets.json";

/// What the secret source of the fake Hermes adds to every server it starts.
pub const INJECTED_FILE: &str = "injected.json";

/// What `mcp add` read on its input, appended.
pub const INPUT_FILE: &str = "input.log";

/// While this file exists, `API_SERVER_KEY` is not set unless a test or `config set` set it.
pub const NO_KEY_FILE: &str = "no-key";

/// The names, a JSON list, that `config get` says are set.
pub const CONFIG_SET_FILE: &str = "config-set.json";

/// While this file exists, `config get` exits with the usage code.
pub const GET_FAILS_FILE: &str = "config-get-fails";

/// While this file exists, `config set` exits 1 and saves nothing.
pub const SET_FAILS_FILE: &str = "config-set-fails";

/// While this file names a `config` call (`get` or `set`), that call does not end.
pub const HANG_FILE: &str = "hang";

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
        ["mcp", "add", name, "--command", command, rest @ ..] => {
            let Some((env, args)) = split_options(rest) else {
                return USAGE;
            };
            if !env.is_empty() && folder.join(NO_ENV_FILE).exists() {
                return USAGE;
            }
            add(folder, name, command, &env, &args, input)
        }
        ["config", "get", name] => config_get(folder, name),
        ["config", "set", name, value] => config_set(folder, name, value),
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

/// The `--env KEY=VALUE` pairs, and the arguments after `--args`.
type Options<'a> = (Vec<(&'a str, &'a str)>, Vec<&'a str>);

/// The `--env KEY=VALUE` pairs and the arguments after `--args`, or `None` when the options
/// are not in the form `(--env KEY=VALUE)... --args ARG...`.
fn split_options<'a>(rest: &[&'a str]) -> Option<Options<'a>> {
    let mut env = Vec::new();
    let mut at = 0;
    while rest.get(at) == Some(&"--env") {
        let (key, value) = rest.get(at + 1)?.split_once('=')?;
        let mut chars = key.chars();
        let first = chars.next()?;
        if !(first.is_ascii_alphabetic() || first == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }
        env.push((key, value));
        at += 2;
    }
    if rest.get(at) != Some(&"--args") {
        return None;
    }
    Some((env, rest[at + 1..].to_vec()))
}

/// The sorted names of the environment of this process: never the values.
#[must_use]
pub fn environment_names() -> Vec<String> {
    let mut names: Vec<String> = std::env::vars_os()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// `mcp add`: the questions of Hermes, answered from `input`.
fn add(
    folder: &Path,
    name: &str,
    command: &str,
    env: &[(&str, &str)],
    args: &[&str],
    input: &mut dyn Read,
) -> u8 {
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
        let mut entry = json!({"command": command, "args": args, "enabled": enabled});
        if !env.is_empty() {
            let env: Map<String, Value> = env
                .iter()
                .map(|(key, value)| ((*key).to_owned(), Value::from(*value)))
                .collect();
            entry["env"] = Value::Object(env);
        }
        servers.insert(name.to_owned(), entry);
        DONE
    })
}

/// Blocks, as a call that does not end, while [`HANG_FILE`] names `call`. It ends by itself
/// when the file is removed or after two minutes, so that a call whose caller was killed does not
/// stay on the machine.
fn hang_when_asked(folder: &Path, call: &str) {
    let named = |folder: &Path| {
        std::fs::read_to_string(folder.join(HANG_FILE)).is_ok_and(|text| text.trim() == call)
    };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while named(folder) && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// `config get NAME`: 0 when it is set, 1 when it is not.
fn config_get(folder: &Path, name: &str) -> u8 {
    hang_when_asked(folder, "get");
    if folder.join(GET_FAILS_FILE).exists() {
        return USAGE;
    }
    let in_secrets = read_object(&folder.join(SECRETS_FILE))
        .get(name)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty());
    let listed = std::fs::read(folder.join(CONFIG_SET_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .is_some_and(|names| names.iter().any(|listed| listed == name));
    let by_default = name == "API_SERVER_KEY" && !folder.join(NO_KEY_FILE).exists();
    if in_secrets || listed || by_default {
        DONE
    } else {
        REFUSED
    }
}

/// `config set NAME VALUE`: the value goes to the secrets of this Hermes.
fn config_set(folder: &Path, name: &str, value: &str) -> u8 {
    hang_when_asked(folder, "set");
    if folder.join(SET_FAILS_FILE).exists() {
        return REFUSED;
    }
    let mut secrets = read_object(&folder.join(SECRETS_FILE));
    secrets.insert(name.to_owned(), Value::from(value));
    let Ok(bytes) = serde_json::to_vec(&Value::Object(secrets)) else {
        return BROKEN;
    };
    if std::fs::write(folder.join(SECRETS_FILE), bytes).is_err() {
        return BROKEN;
    }
    DONE
}

fn read_object(path: &Path) -> Map<String, Value> {
    match std::fs::read(path).map(|bytes| serde_json::from_slice(&bytes)) {
        Ok(Ok(Value::Object(map))) => map,
        _ => Map::new(),
    }
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
    let mut line = serde_json::to_vec(&environment_names()).map_err(std::io::Error::other)?;
    line.push(b'\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join(ENVIRONMENT_FILE))?
        .write_all(&line)?;
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

    /// The environment is kept literally, several pairs are allowed, and `--args` stays last.
    #[test]
    fn the_env_of_an_entry_is_kept_literally() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        let add = "mcp add arauto --command /p/c --env ARAUTO_HERMES_API_KEY=${API_SERVER_KEY} --env B_2=x --args mcp --home /h";
        assert_eq!(call(folder, add, "y\ny\n").0, DONE);
        assert_eq!(
            servers(folder).unwrap()["arauto"]["env"],
            json!({"ARAUTO_HERMES_API_KEY": "${API_SERVER_KEY}", "B_2": "x"})
        );
        // Without `--env` there is no `env` at all.
        assert_eq!(call(folder, "mcp remove arauto", "").0, DONE);
        assert_eq!(
            call(folder, "mcp add arauto --command /p/c --args mcp", "y\ny\n").0,
            DONE
        );
        assert!(servers(folder).unwrap()["arauto"].get("env").is_none());
        // The names of the environment of each call are kept, never a value.
        let names = std::fs::read_to_string(folder.join(ENVIRONMENT_FILE)).unwrap();
        assert_eq!(names.lines().count(), 3);
    }

    /// An older Hermes does not know the option: usage error before any question, nothing saved.
    #[test]
    fn an_older_hermes_refuses_env_before_it_asks() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        std::fs::write(folder.join(NO_ENV_FILE), b"").unwrap();
        let with = "mcp add arauto --command /p/c --env A=1 --args mcp";
        assert_eq!(call(folder, with, "y\ny\n").0, USAGE);
        assert!(servers(folder).unwrap().is_empty());
        assert!(!folder.join(INPUT_FILE).exists());
        assert_eq!(
            call(folder, "mcp add arauto --command /p/c --args mcp", "y\ny\n").0,
            DONE
        );
        assert!(servers(folder).unwrap().contains_key("arauto"));
    }

    #[test]
    fn a_malformed_env_is_a_usage_error() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        for bad in [
            "mcp add arauto --command /p/c --env NOEQUALS --args mcp",
            "mcp add arauto --command /p/c --env 1A=x --args mcp",
            "mcp add arauto --command /p/c --env A-B=x --args mcp",
            "mcp add arauto --command /p/c --env =x --args mcp",
            "mcp add arauto --command /p/c --env A=x",
            "mcp add arauto --command /p/c --args mcp --env A=x --args y",
            "mcp add arauto --command /p/c",
        ] {
            let (code, _) = call(folder, bad, "y\ny\n");
            // Anything after `--args` is an argument, so that one is accepted.
            if bad.ends_with("--args y") {
                assert_eq!(code, DONE, "{bad}");
                let _ = call(folder, "mcp remove arauto", "");
            } else {
                assert_eq!(code, USAGE, "{bad}");
            }
        }
        assert!(servers(folder).unwrap().is_empty());
    }

    /// `config get` and `config set`: a key by default, the scripted ways to not have one, and
    /// a value kept where `${NAME}` resolves from.
    #[test]
    fn config_get_and_set_behave_as_scripted() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        let get = |name: &str| call(folder, &format!("config get {name}"), "").0;
        assert_eq!(get("API_SERVER_KEY"), DONE);
        assert_eq!(get("API_SERVER_HOST"), REFUSED);
        std::fs::write(folder.join(NO_KEY_FILE), b"").unwrap();
        assert_eq!(get("API_SERVER_KEY"), REFUSED);
        std::fs::write(folder.join(CONFIG_SET_FILE), br#"["API_SERVER_HOST"]"#).unwrap();
        assert_eq!(get("API_SERVER_HOST"), DONE);
        assert_eq!(get("gateway.api_server.host"), REFUSED);
        std::fs::write(folder.join(GET_FAILS_FILE), b"").unwrap();
        assert_eq!(get("API_SERVER_KEY"), USAGE);
        std::fs::remove_file(folder.join(GET_FAILS_FILE)).unwrap();
        std::fs::write(folder.join(SET_FAILS_FILE), b"").unwrap();
        assert_eq!(call(folder, "config set API_SERVER_KEY abc", "").0, REFUSED);
        assert_eq!(get("API_SERVER_KEY"), REFUSED);
        std::fs::remove_file(folder.join(SET_FAILS_FILE)).unwrap();
        assert_eq!(call(folder, "config set API_SERVER_KEY abc", "").0, DONE);
        assert_eq!(get("API_SERVER_KEY"), DONE);
        assert_eq!(
            read_object(&folder.join(SECRETS_FILE))["API_SERVER_KEY"],
            "abc"
        );
        // Every call is recorded, the value of a `set` included: that file is the fake's.
        let calls = std::fs::read_to_string(folder.join(CALLS_FILE)).unwrap();
        assert_eq!(calls.lines().count(), 10);
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
