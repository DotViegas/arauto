//! Which variables of the environment the connector reads, and which programs it starts.
//!
//! `docs/11-conector.md`, section 6: Hermes also gives every server of tools variables that are
//! none of the connector's business (the key of a model provider, seen by name in the
//! environment of the connector on Hermes v0.22.5). The connector must never read, log or
//! forward any variable other than its own.
//!
//! Without `unsafe` there is no way to prove from inside a process that a variable was not read,
//! so this is a **source check**, and it says so: every line of the source of the connector (and
//! the one file of the platform crate that finds the folder of the user) that reads a variable,
//! declares one for the command line, or starts a program is in the list below, exactly. A new
//! such line fails this test until a person reads it and adds it here with the reason. The
//! behavioural proof that nothing else leaks is the canary of `tests/hermes.rs`: canaries in the
//! terminal and in the environment Hermes gives the server never appear anywhere.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

/// The text that marks a line to be checked.
const MARKS: &[&str] = &[
    "env::var",
    "env::vars",
    "var_os(",
    "env = \"",
    "Command::new",
];

/// (file, the line trimmed, why it is allowed).
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "arauto-connector/src/cli.rs",
        "#[arg(long, global = true, env = \"ARAUTO_HOME\", value_name = \"FOLDER\")]",
        "the folder of the connector",
    ),
    (
        "arauto-connector/src/hermes.rs",
        "let local = std::env::var_os(\"LOCALAPPDATA\").map(PathBuf::from);",
        "Windows only: the place of the launcher of Hermes",
    ),
    (
        "arauto-connector/src/hermes.rs",
        "let here = std::env::var_os(\"LOCALAPPDATA\").map(PathBuf::from);",
        "the test of the line above",
    ),
    (
        "arauto-connector/src/hermes.rs",
        "let mut command = tokio::process::Command::new(program);",
        "the only program the connector starts: the command of Hermes",
    ),
    (
        "arauto-connector/src/main.rs",
        "let hermes = std::env::var_os(\"ARAUTO_HERMES_BIN\");",
        "the command of Hermes",
    ),
    (
        "arauto-connector/src/main.rs",
        "std::env::var_os(arauto_connector::wake::URL_VARIABLE),",
        "the address of the local API, read by `mcp` only",
    ),
    (
        "arauto-connector/src/main.rs",
        "std::env::var_os(arauto_connector::wake::KEY_VARIABLE),",
        "the key of the local API, read by `mcp` only",
    ),
    (
        "arauto-platform/src/home.rs",
        "home_under(std::env::var_os(HOME_VARIABLE))",
        "the folder of the user, for the default folder",
    ),
    (
        "arauto-platform/src/home.rs",
        "match std::env::var_os(variable) {",
        "the test of the line above",
    ),
];

fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn sources(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(folder).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn only_the_listed_lines_read_the_environment_or_start_a_program() {
    let root = crates();
    let mut files = Vec::new();
    sources(&root.join("arauto-connector/src"), &mut files);
    files.push(root.join("arauto-platform/src/home.rs"));
    assert!(files.len() > 20, "the sources were not found: {files:?}");

    let mut unlisted = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for file in &files {
        let name = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(file).unwrap();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            // A comment explains; it does not read anything.
            if line.starts_with("//") || !MARKS.iter().any(|mark| line.contains(mark)) {
                continue;
            }
            match ALLOWED
                .iter()
                .position(|(file, allowed, _)| *file == name && *allowed == line)
            {
                Some(at) => {
                    seen.insert(at);
                }
                None => unlisted.push(format!("{name}:{}: {line}", number + 1)),
            }
        }
    }
    assert!(
        unlisted.is_empty(),
        "lines that read the environment or start a program and are not in the list:\n{}",
        unlisted.join("\n")
    );
    // The list does not keep lines that are gone.
    let stale: Vec<&str> = ALLOWED
        .iter()
        .enumerate()
        .filter(|(at, _)| !seen.contains(at))
        .map(|(_, (_, line, _))| *line)
        .collect();
    assert!(
        stale.is_empty(),
        "lines in the list that no longer exist: {stale:?}"
    );
}

/// The key is read only by `mcp`: `main` hands the reading to the command line as a function,
/// and the other commands never call it.
#[test]
fn the_key_is_read_only_by_the_command_that_uses_it() {
    let root = crates().join("arauto-connector/src");
    let cli = std::fs::read_to_string(root.join("cli.rs")).unwrap();
    assert_eq!(
        cli.matches("waking()").count(),
        1,
        "the reading must be called once, in the arm of `mcp`"
    );
    let at = cli.find("waking()").unwrap();
    let arm = cli[..at].rfind("Command::").unwrap();
    assert!(
        cli[arm..at].starts_with("Command::Mcp"),
        "{}",
        &cli[arm..at]
    );
}
