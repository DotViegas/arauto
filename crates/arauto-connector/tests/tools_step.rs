//! Source checks of the tools step of `join` and of the read of `status`
//! (`docs/11-conector.md`, section 6), the way `tests/key_step.rs` checks the key step: what
//! cannot be seen from inside a process is read from the text of the program, and the test says so.
//!
//! - **Only `add_step` writes the list:** `write_tool_list` is called from one place, between the
//!   two marks of `toolsets.rs`, and `add_step` is called from one place, `tools_step` of `cli.rs`,
//!   which only `join` calls.
//! - **No Hermes command in the leader:** the read of the list is made by `toolsets.rs` for
//!   `status` alone, and nothing of the leader, the `mcp` process, the runtime, the waking, the
//!   registration, `leave` or the relay runs it.
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
                    .replace(std::path::MAIN_SEPARATOR, "/");
                found.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found
}

/// The lines of code that call `name(`: no comment lines and no definition.
fn calls(name: &str) -> Vec<String> {
    let needle = format!("{name}(");
    let definition = format!("fn {name}(");
    sources()
        .iter()
        .flat_map(|(file, text)| {
            let needle = needle.clone();
            let definition = definition.clone();
            text.lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .filter(move |line| line.contains(&needle) && !line.contains(&definition))
                .map(|line| format!("{file}: {}", line.trim()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn the_list_is_written_from_one_place_and_the_step_runs_from_one_place() {
    assert!(sources().len() > 20);
    // `write_tool_list` is called once in all the sources, inside the marks of `toolsets.rs`.
    let writes = calls("write_tool_list");
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert!(writes[0].starts_with("toolsets.rs: "), "{writes:?}");
    let toolsets = read("toolsets.rs");
    let begin = toolsets.find("// BEGIN tools step").unwrap();
    let end = toolsets.find("// END tools step").unwrap();
    assert!(begin < end);
    assert!(toolsets[begin..end].contains(".write_tool_list("));
    // `add_step` is called once outside `toolsets.rs`, in `tools_step` of `cli.rs`.
    let callers: Vec<String> = calls("add_step")
        .into_iter()
        .filter(|line| !line.starts_with("toolsets.rs: "))
        .collect();
    assert_eq!(callers.len(), 1, "{callers:?}");
    assert!(callers[0].starts_with("cli.rs: "), "{callers:?}");
    let cli = read("cli.rs");
    let call = cli.find("add_step(").unwrap();
    let function = cli[..call].rfind("async fn ").unwrap();
    assert!(
        cli[function..].starts_with("async fn tools_step("),
        "{}",
        &cli[function..call]
    );
    // `tools_step` is called twice in `join` and defined once.
    let users: Vec<usize> = cli.match_indices("tools_step(").map(|(at, _)| at).collect();
    assert_eq!(users.len(), 3, "{users:?}");
    let join_start = cli.find("async fn join(").unwrap();
    let join_end = cli.find("async fn tools_step(").unwrap();
    assert!(join_start < join_end);
    let inside = users
        .iter()
        .filter(|at| (join_start..join_end).contains(*at))
        .count();
    assert_eq!(inside, 2, "{users:?}");
}

#[test]
fn the_list_is_read_by_the_tools_module_for_join_and_status_and_by_no_one_else() {
    let reads = calls("read_tool_list");
    assert!(
        reads.iter().all(|line| line.starts_with("toolsets.rs: ")),
        "{reads:?}"
    );
    let now = calls("read_now");
    assert!(
        now.iter()
            .all(|line| line.starts_with("report.rs: ") || line.starts_with("toolsets.rs: ")),
        "{now:?}"
    );
    assert!(now.iter().any(|line| line.starts_with("report.rs: ")));
    for name in [
        "leader.rs",
        "mcp.rs",
        "runtime.rs",
        "wake.rs",
        "register.rs",
        "leave.rs",
        "relay.rs",
    ] {
        let text = read(name);
        for forbidden in ["read_tool_list", "write_tool_list", "read_now", "add_step"] {
            assert!(!text.contains(forbidden), "{forbidden} in {name}");
        }
    }
}
