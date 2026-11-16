//! Source checks of the key step of `join` (`docs/11-conector.md`, section 6), the way
//! `tests/environment.rs` checks the environment: what cannot be seen from inside a process is
//! read from the text of the program, and the test says so.
//!
//! - **T6, the generator:** between the two marks of `join.rs` the key comes from
//!   `getrandom::fill` and from nothing else: no clock, no counter, no other generator.
//! - **T7, only `join`:** the key is saved from one place, `give_key` in `join.rs`, and `give_key`
//!   is called from one place, the function `join` of `cli.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(src().join(name)).unwrap()
}

fn sources() -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut pending = vec![src()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path
                    .strip_prefix(src())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found
}

/// The code of the key step, between its two marks (the first of each: the tests are below).
fn key_step() -> String {
    let join = read("join.rs");
    let begin = join.find("// BEGIN key step").unwrap();
    let end = join.find("// END key step").unwrap();
    assert!(begin < end);
    join[begin..end].to_owned()
}

/// Lines of code: no comment lines.
fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_key_bytes_come_from_the_generator_of_the_system_and_nothing_else() {
    let step = code(&key_step());
    assert_eq!(step.matches("getrandom::fill(").count(), 1, "{step}");
    for forbidden in [
        "SystemTime",
        "Instant",
        "Duration",
        "now(",
        "rand::",
        "fastrand",
        "Uuid",
        "hash",
        "Sha256",
        "counter",
        "seed",
        "thread_rng",
        "process::id",
    ] {
        assert!(!step.contains(forbidden), "{forbidden} in the key step");
    }
    // Nothing of the key step reads the environment or starts a program by itself.
    for forbidden in ["env::", "Command::new", "var_os("] {
        assert!(!step.contains(forbidden), "{forbidden} in the key step");
    }
}

#[test]
fn the_key_is_saved_from_one_place_and_the_step_runs_from_one_place() {
    let sources = sources();
    assert!(sources.len() > 20);
    // `save_key` is called once in all the sources, inside the key step.
    let calls: Vec<String> = sources
        .iter()
        .flat_map(|(name, text)| {
            text.lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .filter(|line| line.contains("save_key(") && !line.contains("fn save_key("))
                .map(|line| format!("{name}: {}", line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].starts_with("join.rs: "), "{calls:?}");
    assert!(code(&key_step()).contains(".save_key("));
    // `give_key` is called once outside `join.rs`, in the function `join` of `cli.rs`.
    let callers: Vec<String> = sources
        .iter()
        .filter(|(name, _)| name != "join.rs")
        .flat_map(|(name, text)| {
            text.lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .filter(|line| line.contains("give_key"))
                .map(|line| format!("{name}: {}", line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(callers.len(), 1, "{callers:?}");
    assert!(callers[0].starts_with("cli.rs: "), "{callers:?}");
    // That call is in `key_step`, which only `join` calls, from the two arms that print a code
    // or the AR-104 line.
    let cli = read("cli.rs");
    let call = cli.find("give_key(").unwrap();
    let function = cli[..call].rfind("async fn ").unwrap();
    assert!(
        cli[function..].starts_with("async fn key_step("),
        "{}",
        &cli[function..call]
    );
    let users: Vec<usize> = cli.match_indices("key_step(").map(|(at, _)| at).collect();
    // The two calls of `join`, and the definition.
    assert_eq!(users.len(), 3, "{users:?}");
    let join_start = cli.find("async fn join(").unwrap();
    // `key_step` is defined right after `join`.
    let join_end = cli.find("async fn key_step(").unwrap();
    assert!(join_start < join_end);
    let inside = users
        .iter()
        .filter(|at| (join_start..join_end).contains(*at))
        .count();
    assert_eq!(inside, 2, "{users:?}");
}
