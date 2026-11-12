//! What the tests that run the program itself share: the program, its commands, the copies of
//! `mcp`, the local socket asked from outside, and the detector of secrets of the canaries.
//!
//! Each test file declares this module with `#[path = "common/mod.rs"] mod common;`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use arauto_platform::LocalStream;
use arauto_protocol::invite::Invite;
use data_encoding::{BASE32_NOPAD, BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// The program, as built for the tests.
pub const PROGRAM: &str = env!("CARGO_BIN_EXE_arauto-connector");

/// What a command printed, and how it ended.
#[derive(Debug, Clone)]
pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// A folder of a connector.
pub struct Folder {
    _dir: tempfile::TempDir,
    pub home: PathBuf,
}

impl Folder {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("arauto");
        Self { _dir: dir, home }
    }

    pub fn devices(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.home.join("devices"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| !name.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    pub fn only_device(&self) -> String {
        let devices = self.devices();
        assert_eq!(devices.len(), 1, "{devices:?}");
        devices[0].clone()
    }

    pub fn device_file(&self, device: &str, file: &str) -> PathBuf {
        self.home.join("devices").join(device).join(file)
    }

    pub fn public(&self, device: &str) -> arauto_protocol::noise::PublicKey {
        let bytes = std::fs::read(self.device_file(device, "identity.key")).unwrap();
        let identity = arauto_protocol::noise::StaticKeypair::from_private(
            arauto_protocol::secret::Secret::from_slice(&bytes).unwrap(),
        );
        *identity.public()
    }
}

/// The program with `arguments`, its folder, and no `ARAUTO_HOME` from the environment of the
/// test.
pub fn command(home: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(PROGRAM);
    command
        .args(arguments)
        .arg("--home")
        .arg(home)
        .env_remove("ARAUTO_HOME")
        .env_remove("ARAUTO_HERMES_BIN")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

/// Runs a command to its end.
pub async fn run(home: &Path, arguments: &[&str]) -> Ran {
    ran(command(home, arguments)).await
}

/// Runs `command` to its end.
pub async fn ran(mut command: Command) -> Ran {
    let output = tokio::time::timeout(Duration::from_secs(120), command.output())
        .await
        .expect("the command ended in time")
        .unwrap();
    Ran {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The code a `join` printed.
pub fn code_in(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("VERIFICATION CODE: "))
        .unwrap_or_else(|| panic!("no code in: {stdout}"))
        .trim()
        .to_owned()
}

/// A copy of `mcp`, as Hermes starts it: its input open, its output read.
pub struct Copy {
    pub child: Child,
    pub stdout: tokio::task::JoinHandle<Vec<u8>>,
    pub stderr: tokio::task::JoinHandle<Vec<u8>>,
}

impl Copy {
    pub fn start(home: &Path) -> Self {
        let mut command = command(home, &["mcp"]);
        command.stdin(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let stdout = tokio::spawn(async move {
            let mut all = Vec::new();
            let _ = stdout.read_to_end(&mut all).await;
            all
        });
        let stderr = tokio::spawn(async move {
            let mut all = Vec::new();
            let _ = stderr.read_to_end(&mut all).await;
            all
        });
        Self {
            child,
            stdout,
            stderr,
        }
    }

    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Kills it, as a process that dies.
    pub async fn kill(mut self) -> (Vec<u8>, Vec<u8>) {
        let _ = self.child.kill().await;
        let stdout = self.stdout.await.unwrap_or_default();
        let stderr = self.stderr.await.unwrap_or_default();
        (stdout, stderr)
    }

    /// Closes its input, as Hermes does to stop it, and waits for it to end.
    pub async fn stop(mut self) -> (Option<i32>, Vec<u8>, Vec<u8>) {
        drop(self.child.stdin.take());
        let status = tokio::time::timeout(Duration::from_secs(20), self.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
        if status.is_none() {
            let _ = self.child.kill().await;
        }
        let stdout = self.stdout.await.unwrap_or_default();
        let stderr = self.stderr.await.unwrap_or_default();
        (status.and_then(|status| status.code()), stdout, stderr)
    }
}

/// Asks the leader of `home` for its situation, through the local socket.
pub async fn ask(home: &Path, line: &str) -> Option<Value> {
    let attempt = async {
        let stream = LocalStream::connect(home).await.ok()?;
        let (reader, mut writer) = tokio::io::split(stream);
        writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .ok()?;
        writer.flush().await.ok()?;
        let mut lines = BufReader::new(reader).lines();
        let answer = lines.next_line().await.ok()??;
        serde_json::from_str(&answer).ok()
    };
    tokio::time::timeout(Duration::from_secs(10), attempt)
        .await
        .ok()
        .flatten()
}

/// The process of the leader of `home`, when one answers.
pub async fn leader_process(home: &Path) -> Option<u32> {
    let answer = ask(home, r#"{"v":1,"op":"status"}"#).await?;
    answer["status"]["leader_process"]
        .as_u64()
        .and_then(|process| u32::try_from(process).ok())
}

/// Polls `condition` until it holds, or fails the test.
pub async fn eventually<F, Fut>(what: &str, within: Duration, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let until = tokio::time::Instant::now() + within;
    loop {
        if condition().await {
            return;
        }
        assert!(tokio::time::Instant::now() < until, "never: {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Every line of the standard output of `mcp` is a message of JSON-RPC 2.0: it carries only
/// MCP.
pub fn only_messages(stdout: &[u8]) {
    let text = String::from_utf8(stdout.to_vec()).unwrap();
    assert!(text.is_empty() || text.ends_with('\n'), "{text:?}");
    for line in text.lines() {
        let message: Value = serde_json::from_str(line)
            .unwrap_or_else(|_| panic!("not a message on the standard output: {line:?}"));
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
    }
}

/// A copy closed as Hermes closes it ended by itself, well, in less than the 2 seconds Hermes
/// gives, and wrote only MCP.
pub fn ended_in_order(closed: &arauto_testkit::hermes::Closed) {
    assert!(!closed.forced, "it had to be ended by force: {closed:?}");
    assert!(
        closed.after < arauto_testkit::hermes::CLOSE_GRACE,
        "it took {:?} to end",
        closed.after
    );
    assert_eq!(closed.status.and_then(|status| status.code()), Some(0));
    only_messages(&closed.stdout);
}

/// The ways a secret of bytes can be written down.
pub fn spellings(bytes: &[u8]) -> Vec<String> {
    vec![
        HEXLOWER.encode(bytes),
        HEXUPPER.encode(bytes),
        BASE64.encode(bytes),
        BASE64_NOPAD.encode(bytes),
        BASE64URL_NOPAD.encode(bytes),
        BASE32_NOPAD.encode(bytes),
        BASE32_NOPAD.encode(bytes).to_ascii_lowercase(),
        format!("{bytes:?}"),
    ]
}

/// The 32 bytes of an invite, from its text.
pub fn invite_bytes(invite: &Invite) -> Vec<u8> {
    let text = invite.reveal();
    BASE32_NOPAD
        .decode(text[4..].to_ascii_uppercase().as_bytes())
        .unwrap()
}

/// A verification code, as `123-456` or as six digits standing alone.
pub fn has_code(text: &str, code: &str) -> bool {
    let digits: String = code.chars().filter(char::is_ascii_digit).collect();
    if text.contains(code) {
        return true;
    }
    text.match_indices(digits.as_str()).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + digits.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// The secrets a canary looks for, in every way they can be written down.
#[derive(Default)]
pub struct Secrets {
    pub written: Vec<(String, String)>,
    pub codes: Vec<String>,
}

impl Secrets {
    pub fn add(&mut self, what: &str, bytes: &[u8]) {
        for written in spellings(bytes) {
            self.written.push((what.to_owned(), written));
        }
    }

    pub fn add_text(&mut self, what: &str, text: &str) {
        self.written.push((what.to_owned(), text.to_owned()));
    }

    /// What of the secrets shows up in `text`.
    pub fn leaks(&self, text: &str, with_codes: bool) -> Vec<String> {
        let mut found: Vec<String> = self
            .written
            .iter()
            .filter(|(_, written)| text.contains(written.as_str()))
            .map(|(what, _)| what.clone())
            .collect();
        if with_codes {
            found.extend(
                self.codes
                    .iter()
                    .filter(|code| has_code(text, code))
                    .map(|code| format!("code {code}")),
            );
        }
        found.sort();
        found.dedup();
        found
    }
}

/// Everything of the folder but the two files of secrets and the program.
pub fn files_of(home: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut pending = vec![home.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if name == "identity.key"
                || name == "credential.seal"
                || name.starts_with("arauto-connector")
            {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap_or_default();
            found.push((
                path.display().to_string(),
                String::from_utf8_lossy(&bytes).into_owned(),
            ));
        }
    }
    found
}
