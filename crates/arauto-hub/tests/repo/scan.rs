//! The rule "the organization comes resolved" (`docs/15-mvp.md`, section 2, rule 2), kept by
//! reading the code:
//!
//! - every function of the repositories (`src/repo`) that can reach the database takes a
//!   `ResolvedOrg`, except the ones of a closed list;
//! - everywhere else in `src`, a function that is handed a handle of the database and no
//!   `ResolvedOrg` is in another closed list, with its name, its file and why it may: the
//!   entry functions that find the organization from the outside, the tasks of the whole
//!   installation, and the plumbing that runs no query of business data.
//!
//! What the scan reads is the signature. It does not read the SQL: that a statement filters
//! by the organization it was given is kept by the tests that call each function with the
//! organization next door (`hermes::what_is_of_one_organization_is_not_reached_from_another`).
//! A method that reaches the database through `self` (the session of the admin command, the
//! security write) is not seen either: those values are built from a resolved organization
//! or carry none.

use std::path::{Path, PathBuf};

/// The functions of `src/repo` that reach the database without a resolved organization, and
/// why each one may.
const WITHOUT_ORGANIZATION: &[(&str, &str)] = &[
    (
        "flush_refused",
        "writes the refusals of invites nobody issued: they belong to no organization",
    ),
    (
        "retention",
        "reads the days each table is kept: they are of the installation, the same for every \
         organization, and the purge then deletes organization by organization",
    ),
];

/// The types through which a function reaches the database.
const DATABASE_HANDLES: &[&str] = &[
    "PgPool",
    "PgConnection",
    "PgExecutor",
    "Transaction",
    "SecurityWrite",
];

/// A function found in a source file.
#[derive(Debug, PartialEq, Eq)]
struct Function {
    name: String,
    is_async: bool,
    parameters: String,
}

/// The source without its comments, so that a signature in a comment is not read as code.
fn without_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split("//").next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `fn` of a source file, with the text between its parentheses.
fn functions(source: &str) -> Vec<Function> {
    let code = without_comments(source);
    let mut found = Vec::new();
    let mut rest = code.as_str();
    let mut before = String::new();
    while let Some(at) = rest.find("fn ") {
        before.push_str(&rest[..at]);
        let after = &rest[at + 3..];
        let starts_a_word = before
            .chars()
            .last()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let signature = &after[name.len()..];
        // A generic list, then the parameters. `fn(` with no name is a function type.
        let opens = signature.find('(');
        if starts_a_word
            && !name.is_empty()
            && let Some(opens) = opens
            && signature[..opens].chars().all(|c| {
                c == '<'
                    || c == '>'
                    || c == '\''
                    || c.is_alphanumeric()
                    || matches!(c, ' ' | ',' | ':' | '_' | '+' | '?' | '\n')
            })
        {
            let mut depth = 0usize;
            let mut parameters = String::new();
            for c in signature[opens..].chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    break;
                }
                parameters.push(c);
            }
            let is_async = before.trim_end().ends_with("async");
            found.push(Function {
                name,
                is_async,
                parameters,
            });
        }
        before.push_str("fn ");
        rest = after;
    }
    found
}

/// Why a function breaks the rule, if it does.
fn breach(function: &Function) -> Option<&'static str> {
    if function.parameters.contains("ResolvedOrg")
        || WITHOUT_ORGANIZATION
            .iter()
            .any(|(name, _)| *name == function.name)
    {
        return None;
    }
    if function.is_async {
        return Some("is async and does not take the resolved organization");
    }
    if DATABASE_HANDLES
        .iter()
        .any(|handle| function.parameters.contains(handle))
    {
        return Some("takes a handle of the database and not the resolved organization");
    }
    None
}

fn repo_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("repo");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    assert!(
        files
            .iter()
            .all(|file| file.extension().is_some_and(|e| e == "rs")),
        "src/repo holds only source files, with no folder inside: {files:?}"
    );
    files
}

/// The functions of `src`, outside the repositories, that are handed a handle of the database
/// and no resolved organization: the file, the name, and why each one may.
const OUTSIDE_THE_REPOSITORIES: &[(&str, &str, &str)] = &[
    // The entry functions: the only ways an organization is found from the outside.
    (
        "db.rs",
        "organization_of_device",
        "entry: finds the organization by the identity of a device",
    ),
    (
        "db.rs",
        "organization_of_invite",
        "entry: finds the organization by the digest of an invite",
    ),
    (
        "db.rs",
        "organization_of_engine_events",
        "entry: finds the organization by the engine named in the address of its inbox; \
         nothing is handed back until the signature made with the secret of its events is \
         checked (docs/15-mvp.md, section 2, rule 2: the entry by the secret of the engine)",
    ),
    (
        "db.rs",
        "organization_of_panel_entry",
        "entry: finds the organization by the digest of an entry code of the panel",
    ),
    (
        "db.rs",
        "organization_of_panel_session",
        "entry: finds the organization by the digest of the cookie of a session of the panel",
    ),
    (
        "db.rs",
        "organization_of_panel_passkey",
        "entry: finds the organization by the credential of an active passkey of the panel; \
         nothing is opened until the signature made with that passkey is checked",
    ),
    (
        "db.rs",
        "organizations_for_system_task",
        "entry: every organization, for a named task of the hub itself",
    ),
    // The database itself: no business data.
    (
        "db.rs",
        "read_durability",
        "reads two settings of the server",
    ),
    (
        "db.rs",
        "check_durability",
        "reads two settings of the server",
    ),
    (
        "db.rs",
        "schema_state",
        "reads the record of the migrations",
    ),
    ("db.rs", "apply_all", "applies the migrations"),
    ("db.rs", "migrate_new_database", "applies the migrations"),
    (
        "db.rs",
        "migrate_existing_database",
        "applies the migrations",
    ),
    (
        "db.rs",
        "require_current_schema",
        "reads the record of the migrations",
    ),
    (
        "db.rs",
        "begin_durable",
        "opens a transaction; runs no query of business data",
    ),
    (
        "db.rs",
        "rotate_password",
        "task of the installation: the password of the database role is one for every \
         organization; the pools are only given its new password",
    ),
    // The audit: the organization of an event is in the event.
    (
        "audit.rs",
        "record",
        "writes the event it is given, with the organization the event names",
    ),
    (
        "audit.rs",
        "insert",
        "writes the event it is given, with the organization the event names",
    ),
    (
        "audit.rs",
        "begin_security",
        "task of the installation: the mark is one for every organization",
    ),
    // The restored mode: it is of the whole installation, by definition (C11).
    (
        "restore.rs",
        "standing",
        "task of the installation: compares the whole audit with the mark",
    ),
    (
        "restore.rs",
        "check",
        "task of the installation: compares the whole audit with the mark",
    ),
    (
        "restore.rs",
        "installation",
        "task of the installation: reads its one row",
    ),
    (
        "restore.rs",
        "enter",
        "task of the installation: the restored mode suspends the devices of every organization",
    ),
    (
        "restore.rs",
        "entered_last",
        "task of the installation: reads the last security event of the whole audit",
    ),
    (
        "restore.rs",
        "end_when_done",
        "task of the installation: the mode ends when no organization has a suspended device",
    ),
    // The admin command, before it has a session: the installation and the key of the hub.
    (
        "admin/mod.rs",
        "has_data",
        "task of the installation: whether anything was ever installed",
    ),
    (
        "admin/mod.rs",
        "install_if_new",
        "task of the installation: `init`, when the hub starts on an empty database",
    ),
    (
        "admin/mod.rs",
        "active_hub_key",
        "the key of the hub is one for every organization",
    ),
    (
        "admin/mod.rs",
        "hub_public_key",
        "the key of the hub is one for every organization",
    ),
    (
        "admin/mod.rs",
        "hub_keypair",
        "the key of the hub is one for every organization",
    ),
    (
        "admin/mod.rs",
        "the_organization",
        "resolves the one organization of the MVP through the entry function",
    ),
    (
        "admin/mod.rs",
        "init",
        "creates the installation and its first organization",
    ),
    ("admin/mod.rs", "migrate", "applies the migrations"),
    (
        "admin/mod.rs",
        "status",
        "resolves the organization itself, through the entry function",
    ),
    (
        "admin/mod.rs",
        "dev_reset",
        "development only: erases the whole database",
    ),
    (
        "admin/mod.rs",
        "open",
        "resolves the organization itself, through the entry function",
    ),
    // The channel: the start of `serve`, and the request to leave of a short connection.
    (
        "channel/mod.rs",
        "recover",
        "task of the installation: the restored mode, before the hub listens",
    ),
    (
        "channel/mod.rs",
        "close_left_open",
        "goes through every organization by the entry function, for the start of `serve`",
    ),
    (
        "channel/hello.rs",
        "leave_by_identity",
        "resolves the organization by the identity of the device, through the entry function",
    ),
];

/// Every source file of `src` outside the repositories, with its path from `src`.
fn other_files() -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, from: &Path, found: &mut Vec<(String, PathBuf)>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, from, found);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let relative = path
                    .strip_prefix(from)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((relative, path));
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    walk(&src, &src, &mut found);
    found.retain(|(relative, _)| !relative.starts_with("repo/"));
    found.sort();
    found
}

fn takes_a_handle(function: &Function) -> bool {
    DATABASE_HANDLES
        .iter()
        .any(|handle| function.parameters.contains(handle))
}

#[test]
fn outside_the_repositories_a_function_with_a_database_handle_is_on_the_closed_list() {
    let mut files = 0usize;
    let mut excused = Vec::new();
    let mut breaches = Vec::new();
    for (relative, file) in other_files() {
        files += 1;
        let source = std::fs::read_to_string(&file).unwrap();
        let code = source.split("#[cfg(test)]").next().unwrap();
        for function in functions(code) {
            if !takes_a_handle(&function) || function.parameters.contains("ResolvedOrg") {
                continue;
            }
            let listed = OUTSIDE_THE_REPOSITORIES
                .iter()
                .any(|(of, name, _)| *of == relative && *name == function.name);
            if listed {
                excused.push((relative.clone(), function.name.clone()));
            } else {
                breaches.push(format!("{relative}: `{}`", function.name));
            }
        }
    }
    assert!(
        breaches.is_empty(),
        "functions that reach the database without the resolved organization:\n  {}\nTake a \
         `ResolvedOrg`, or add the function to the closed list of this test and say why.",
        breaches.join("\n  ")
    );
    // The scan read the whole of `src`, and the list has no entry that no longer exists.
    assert!(files >= 10, "only {files} source files were found");
    let mut listed: Vec<(String, String)> = OUTSIDE_THE_REPOSITORIES
        .iter()
        .map(|(of, name, _)| ((*of).to_owned(), (*name).to_owned()))
        .collect();
    listed.sort();
    excused.sort();
    assert_eq!(excused, listed, "the closed list names what exists, once");
    assert!(
        OUTSIDE_THE_REPOSITORIES
            .iter()
            .all(|(_, _, why)| !why.is_empty()),
        "every entry says why"
    );
}

#[test]
fn every_repository_function_takes_the_resolved_organization() {
    let mut scanned = 0usize;
    let mut asynchronous = 0usize;
    let mut excused = Vec::new();
    let mut breaches = Vec::new();
    for file in repo_files() {
        let source = std::fs::read_to_string(&file).unwrap();
        // The tests inside a file are not repository functions.
        let code = source.split("#[cfg(test)]").next().unwrap();
        for function in functions(code) {
            scanned += 1;
            asynchronous += usize::from(function.is_async);
            if WITHOUT_ORGANIZATION
                .iter()
                .any(|(name, _)| *name == function.name)
            {
                excused.push(function.name.clone());
            }
            if let Some(why) = breach(&function) {
                breaches.push(format!(
                    "{}: `{}` {why}",
                    file.file_name().unwrap().to_string_lossy(),
                    function.name
                ));
            }
        }
    }
    assert!(
        breaches.is_empty(),
        "repository functions without the resolved organization:\n  {}\nTake a `ResolvedOrg`, \
         or add the function to the closed list of this test and say why.",
        breaches.join("\n  ")
    );
    // The scan found the repositories, and the list has no entry that no longer exists.
    assert!(scanned > 80, "only {scanned} functions were found");
    assert!(
        asynchronous > 40,
        "only {asynchronous} async functions were found"
    );
    let listed: Vec<&str> = WITHOUT_ORGANIZATION.iter().map(|(name, _)| *name).collect();
    assert_eq!(excused, listed, "the closed list names what exists, once");
}

#[test]
fn the_scan_reads_signatures_the_way_they_are_written() {
    let source = r"
        // pub async fn in_a_comment(pool: &PgPool) {}
        pub async fn with(pool: &PgPool, organization: ResolvedOrg, now: i64) -> u8 { 0 }
        pub async fn without(pool: &PgPool, now: i64) -> u8 { 0 }
        pub(crate) async fn helper<'e>(
            executor: impl PgExecutor<'e>,
            id: Uuid,
        ) -> Result<(), sqlx::Error> { Ok(()) }
        async fn generic<T: Into<i64> + ?Sized>(
            connection: &mut PgConnection,
            organization: ResolvedOrg,
            callback: impl Fn(i64) -> i64,
        ) {}
        fn sync_with_handle(write: &mut SecurityWrite) {}
        pub const fn pure(now: i64, deadline: Option<i64>) -> bool { true }
        pub fn method(&self) -> Actor { todo() }
        fn takes_a_function(apply: fn(i64) -> i64) {}
    ";
    let found = functions(source);
    let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "with",
            "without",
            "helper",
            "generic",
            "sync_with_handle",
            "pure",
            "method",
            "takes_a_function"
        ]
    );
    let verdicts: Vec<(&str, bool)> = found
        .iter()
        .map(|f| (f.name.as_str(), breach(f).is_some()))
        .collect();
    assert_eq!(
        verdicts,
        [
            ("with", false),
            ("without", true),
            ("helper", true),
            ("generic", false),
            ("sync_with_handle", true),
            ("pure", false),
            ("method", false),
            ("takes_a_function", false),
        ]
    );
    let generic = found.iter().find(|f| f.name == "generic").unwrap();
    assert!(generic.is_async);
    assert!(generic.parameters.contains("impl Fn(i64) -> i64"));
    assert!(!found.iter().find(|f| f.name == "pure").unwrap().is_async);
}
