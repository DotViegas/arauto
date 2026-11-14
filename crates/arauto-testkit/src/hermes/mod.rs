//! A fake Hermes, for the tests of the connector (`docs/15-mvp.md`, section 5). Never shipped.
//!
//! | Piece | What it does |
//! |---|---|
//! | [`registry`] | The program `ARAUTO_HERMES_BIN` points to: it records each call, keeps the servers of tools registered with `mcp add` (it asks its questions on the input, as Hermes v0.22.5 does), prints them with `mcp list` and drops them with `mcp remove`, as the command of Hermes does. The connector runs it as it would run Hermes: no code of the tests is in the connector. The example `fake_hermes` of the connector is this program. |
//! | [`FakeHermes`] | Its folder, its records, and the side of Hermes that starts servers of tools: each copy of the registered command, with its input and output through pipes and a **filtered environment**, as Hermes does (`referencia/hermes-fatos.md`): the folder of the user, the path of the programs, the temporary folders, and nothing else. |
//! | [`McpCopy`] | One copy, as a client of MCP talks to it: `initialize`, `tools/list`, `tools/call`, or any line at all. It is killed, or closed as Hermes closes it: the input first, the force 2 seconds later. |
//!
//! The folder of a fake Hermes:
//!
//! ```text
//! hermes(.exe)     the registry program, copied here: what ARAUTO_HERMES_BIN gets
//! calls.jsonl      every call it received
//! servers.json     the servers of tools registered
//! input.log        what `mcp add` read on its input
//! refuse           while it exists, every call is refused
//! user/            the folder of the user of this Hermes: HOME, or USERPROFILE on Windows
//! ```
// Test support: a failure here is the failure of the test that called it, with the reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod registry;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// How long a request waits for its answer by default.
pub const PATIENCE: Duration = Duration::from_secs(30);

/// How long Hermes waits, after it closes the input of a server, before it ends it by force.
pub const CLOSE_GRACE: Duration = Duration::from_secs(2);

/// The version of MCP the fake Hermes asks for.
pub const ASKED_VERSION: &str = "2025-11-25";

/// The variable of the folder of the user.
#[cfg(unix)]
const USER_FOLDER_VARIABLES: &[&str] = &["HOME"];
#[cfg(windows)]
const USER_FOLDER_VARIABLES: &[&str] = &["USERPROFILE", "APPDATA", "LOCALAPPDATA"];

/// What a server of tools keeps of the environment of Hermes: the path of the programs, the
/// temporary folders, and on Windows what any program needs to start.
#[cfg(unix)]
const KEPT_VARIABLES: &[&str] = &["PATH", "TMPDIR", "LANG", "USER", "LOGNAME"];
#[cfg(windows)]
const KEPT_VARIABLES: &[&str] = &[
    "PATH",
    "PATHEXT",
    "TEMP",
    "TMP",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "USERNAME",
    "PROCESSOR_ARCHITECTURE",
    "NUMBER_OF_PROCESSORS",
];

/// The program of the example `name` of the package whose tests run: `cargo test` builds the
/// examples next to the test programs.
#[must_use]
pub fn built_example(name: &str) -> PathBuf {
    let test = std::env::current_exe().unwrap();
    let profile = test
        .parent()
        .and_then(Path::parent)
        .expect("the test program runs from the folder of the compilation");
    let program = profile
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.is_file(),
        "the example {name} was not built: run cargo test without choosing the test targets"
    );
    program
}

/// A server of tools as Hermes registered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    /// The program.
    pub command: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
}

/// A fake Hermes: its folder, with the registry program and its records.
pub struct FakeHermes {
    dir: tempfile::TempDir,
}

impl FakeHermes {
    /// A fake Hermes whose command is a copy of the registry program `program`.
    pub fn new(program: &Path) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(
            program,
            dir.path()
                .join(format!("hermes{}", std::env::consts::EXE_SUFFIX)),
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("user")).unwrap();
        Self { dir }
    }

    /// The folder.
    #[must_use]
    pub fn folder(&self) -> &Path {
        self.dir.path()
    }

    /// The command of Hermes: what `ARAUTO_HERMES_BIN` gets.
    #[must_use]
    pub fn program(&self) -> PathBuf {
        self.folder()
            .join(format!("hermes{}", std::env::consts::EXE_SUFFIX))
    }

    /// The folder of the user of this Hermes.
    #[must_use]
    pub fn user_folder(&self) -> PathBuf {
        self.folder().join("user")
    }

    /// The default folder of the connector of this user.
    #[must_use]
    pub fn default_home(&self) -> PathBuf {
        self.user_folder().join(".arauto")
    }

    /// Every call the command received, its arguments.
    #[must_use]
    pub fn calls(&self) -> Vec<Vec<String>> {
        std::fs::read_to_string(self.folder().join(registry::CALLS_FILE))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// The server of tools registered as `name`.
    #[must_use]
    pub fn registered(&self, name: &str) -> Option<Registered> {
        let servers = registry::servers(self.folder()).unwrap();
        let entry = servers.get(name)?;
        Some(Registered {
            command: PathBuf::from(entry["command"].as_str().unwrap()),
            args: entry["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap().to_owned())
                .collect(),
        })
    }

    /// Makes every call of the command refused, or accepted again.
    pub fn refuse(&self, refuse: bool) {
        let path = self.folder().join(registry::REFUSE_FILE);
        if refuse {
            std::fs::write(path, b"").unwrap();
        } else {
            let _ = std::fs::remove_file(path);
        }
    }

    /// The environment a server of tools started by this Hermes receives.
    #[must_use]
    pub fn environment(&self) -> Vec<(OsString, OsString)> {
        let mut environment: Vec<(OsString, OsString)> = KEPT_VARIABLES
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| (OsString::from(name), value)))
            .collect();
        for name in USER_FOLDER_VARIABLES {
            environment.push((OsString::from(name), self.user_folder().into_os_string()));
        }
        environment
    }

    /// A command the operator runs in the terminal of this Hermes: the folder of the user is
    /// the one of this Hermes, `ARAUTO_HERMES_BIN` is its command, and no `ARAUTO_HOME`.
    #[must_use]
    pub fn terminal(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        for name in USER_FOLDER_VARIABLES {
            command.env(name, self.user_folder());
        }
        command
            .env("ARAUTO_HERMES_BIN", self.program())
            .env_remove("ARAUTO_HOME")
            .current_dir(self.user_folder())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }

    /// Starts a copy of the server of tools registered as `name`, as Hermes does.
    pub fn start(&self, name: &str) -> McpCopy {
        let registered = self
            .registered(name)
            .unwrap_or_else(|| panic!("no server {name} is registered"));
        let args: Vec<OsString> = registered.args.iter().map(OsString::from).collect();
        self.start_command(&registered.command, &args)
    }

    /// Starts `program` with `args` as a server of tools of this Hermes.
    pub fn start_command(&self, program: &Path, args: &[OsString]) -> McpCopy {
        let mut command = Command::new(program);
        command
            .args(args)
            .env_clear()
            .envs(self.environment())
            .current_dir(self.user_folder());
        McpCopy::spawn(command)
    }
}

/// How a copy ended when it was closed.
#[derive(Debug)]
pub struct Closed {
    /// How it exited.
    pub status: Option<ExitStatus>,
    /// From the close of its input to its end.
    pub after: Duration,
    /// It had to be ended by force.
    pub forced: bool,
    /// Everything it wrote to its standard output.
    pub stdout: Vec<u8>,
    /// Everything it wrote to its standard error.
    pub stderr: Vec<u8>,
}

/// One copy of a server of tools, with its input and output through pipes.
pub struct McpCopy {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::UnboundedReceiver<Vec<u8>>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
    next: i64,
    /// Messages read while waiting for another one.
    aside: Vec<Value>,
}

impl McpCopy {
    /// Starts `command` with its standard streams through pipes.
    pub fn spawn(mut command: Command) -> Self {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().unwrap();
        let stdin = child.stdin.take();
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let (sender, lines) = mpsc::unbounded_channel();
        let kept = Arc::clone(&stdout);
        let reading = tokio::spawn(async move {
            let mut line = Vec::new();
            let mut buffer = vec![0u8; 64 * 1024];
            loop {
                let read = out.read(&mut buffer).await.unwrap_or(0);
                if read == 0 {
                    return;
                }
                let chunk = &buffer[..read];
                kept.lock().unwrap().extend_from_slice(chunk);
                for byte in chunk {
                    if *byte == b'\n' {
                        let _ = sender.send(std::mem::take(&mut line));
                    } else {
                        line.push(*byte);
                    }
                }
            }
        });
        let kept = Arc::clone(&stderr);
        let erring = tokio::spawn(async move {
            let mut buffer = vec![0u8; 16 * 1024];
            loop {
                let read = err.read(&mut buffer).await.unwrap_or(0);
                if read == 0 {
                    return;
                }
                kept.lock().unwrap().extend_from_slice(&buffer[..read]);
            }
        });
        Self {
            child,
            stdin,
            lines,
            stdout,
            stderr,
            readers: vec![reading, erring],
            next: 0,
            aside: Vec::new(),
        }
    }

    /// The process.
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Whether it ended.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Writes `bytes` to its input as they are.
    pub async fn write(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("the input is open");
        stdin.write_all(bytes).await.unwrap();
        stdin.flush().await.unwrap();
    }

    /// Writes one line.
    pub async fn send_line(&mut self, line: &str) {
        self.write(format!("{line}\n").as_bytes()).await;
    }

    /// Writes one message.
    pub async fn send(&mut self, message: &Value) {
        self.send_line(&message.to_string()).await;
    }

    /// The next line of its output, in the order it came, or `None` when nothing came in time
    /// or the output ended.
    pub async fn next_line(&mut self, within: Duration) -> Option<Vec<u8>> {
        tokio::time::timeout(within, self.lines.recv())
            .await
            .ok()
            .flatten()
    }

    /// The next message of its output, which must be a message of JSON-RPC 2.0.
    pub async fn next_message(&mut self, within: Duration) -> Option<Value> {
        let line = self.next_line(within).await?;
        let message: Value = serde_json::from_slice(&line).unwrap_or_else(|_| {
            panic!(
                "not JSON on the standard output: {:?}",
                String::from_utf8_lossy(&line)
            )
        });
        assert_eq!(message["jsonrpc"], "2.0", "{message}");
        Some(message)
    }

    /// Sends a request and returns its identifier.
    pub async fn send_request(&mut self, method: &str, params: Value) -> i64 {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        id
    }

    /// The answer to the request `id`: the other messages that come meanwhile are kept aside.
    pub async fn answer(&mut self, id: i64, within: Duration) -> Value {
        let is_it = |message: &Value| {
            message.get("id") == Some(&json!(id))
                && (message.get("result").is_some() || message.get("error").is_some())
        };
        if let Some(at) = self.aside.iter().position(is_it) {
            return self.aside.remove(at);
        }
        let until = tokio::time::Instant::now() + within;
        loop {
            let left = until.saturating_duration_since(tokio::time::Instant::now());
            let message = self.next_message(left).await.unwrap_or_else(|| {
                panic!("no answer to the request {id}; aside: {:?}", self.aside)
            });
            if is_it(&message) {
                return message;
            }
            self.aside.push(message);
        }
    }

    /// Sends a request and waits for its answer.
    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params).await;
        self.answer(id, PATIENCE).await
    }

    /// The messages kept aside, taken.
    pub fn take_aside(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.aside)
    }

    /// `initialize` and `notifications/initialized`, as a client of MCP starts. Returns the
    /// result of `initialize`.
    pub async fn initialize(&mut self) -> Value {
        let answer = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": ASKED_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "fake-hermes", "version": "0.0.0"},
                }),
            )
            .await;
        let result = answer
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("initialize failed: {answer}"));
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        result
    }

    /// The names of `tools/list`.
    pub async fn tools(&mut self) -> Vec<String> {
        let answer = self.request("tools/list", json!({})).await;
        answer["result"]["tools"]
            .as_array()
            .unwrap_or_else(|| panic!("tools/list failed: {answer}"))
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// `tools/call`, and the result.
    pub async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self
            .request("tools/call", json!({"name": tool, "arguments": arguments}))
            .await;
        answer
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("tools/call failed: {answer}"))
    }

    /// Everything it wrote to its standard output so far.
    #[must_use]
    pub fn stdout(&self) -> Vec<u8> {
        self.stdout.lock().unwrap().clone()
    }

    /// Everything it wrote to its standard error so far.
    #[must_use]
    pub fn stderr(&self) -> Vec<u8> {
        self.stderr.lock().unwrap().clone()
    }

    async fn collected(mut self) -> (Vec<u8>, Vec<u8>) {
        for reader in std::mem::take(&mut self.readers) {
            let _ = tokio::time::timeout(Duration::from_secs(10), reader).await;
        }
        (self.stdout(), self.stderr())
    }

    /// Kills it, as a process that dies, and returns what it wrote.
    pub async fn kill(mut self) -> (Vec<u8>, Vec<u8>) {
        let _ = self.child.kill().await;
        self.collected().await
    }

    /// Closes it as Hermes does: the input first; after `grace` without an end, by force.
    pub async fn close(mut self, grace: Duration) -> Closed {
        let started = tokio::time::Instant::now();
        drop(self.stdin.take());
        let status = tokio::time::timeout(grace, self.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
        let after = started.elapsed();
        let forced = status.is_none();
        if forced {
            let _ = self.child.kill().await;
        }
        let (stdout, stderr) = self.collected().await;
        Closed {
            status,
            after,
            forced,
            stdout,
            stderr,
        }
    }

    /// Waits for it to end by itself, and returns how it exited and what it wrote.
    pub async fn ended(mut self, within: Duration) -> (Option<ExitStatus>, Vec<u8>, Vec<u8>) {
        let status = tokio::time::timeout(within, self.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
        if status.is_none() {
            let _ = self.child.kill().await;
        }
        let (stdout, stderr) = self.collected().await;
        (status, stdout, stderr)
    }
}
