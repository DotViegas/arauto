//! The command line (`docs/11-conector.md`, section 1):
//!
//! ```text
//! arauto-connector join HUB KEY INVITE [--no-register]   the request to connect
//! arauto-connector mcp                                   the server of tools Hermes starts
//! arauto-connector status                                the situation, without any secret
//! arauto-connector leave [--local]                       the request to leave, and the cleaning
//! ```
//!
//! Every command takes `--home FOLDER` (or `ARAUTO_HOME`; `~/.arauto` by default).
//! `ARAUTO_HERMES_BIN` names the command of Hermes, `hermes` by default. No other variable is
//! read, and there is no other command.
//!
//! What a command says is a fixed phrase, with the code of the catalog when there is one; a
//! wrong argument is answered with the usage, never with the argument itself, which may be the
//! invite. The standard output of `mcp` is the channel of MCP, and nothing else is written to
//! it: its log goes to the standard error, which the transport of MCP leaves for it.
//!
//! **Exit codes** ([`Exit`]): 0 done; 1 unexpected failure; 2 wrong arguments; 3 the invite or
//! the request was refused (AR-101, AR-102, AR-301, AR-302, AR-303); 4 a device of another hub
//! is in the folder (AR-105); 5 the hub was not reached or ended the connection (AR-201,
//! AR-202, AR-204, AR-205, AR-206, AR-208); 6 the versions do not agree (AR-401, AR-402); 7 the
//! registration in Hermes failed (AR-601); 8 leaving is pending (AR-505); 9 a problem of this
//! machine (the folder, a damaged file, another `join` running, the copy of the program, no
//! TLS); 10 the files were deleted and the registration in Hermes was not removed.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use arauto_protocol::clock::{Clock, SystemClock};
use arauto_protocol::errors::ErrorCode;
use clap::{Parser, Subcommand};
use zeroize::Zeroizing;

use crate::catalog::{Local, Said};
use crate::hermes::Hermes;
use crate::home::Home;
use crate::join::{JoinArgs, Joined};
use crate::leave::Left;
use crate::runtime::Connector;
use crate::settings::Settings;

/// The exit code of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Exit {
    /// Done.
    Done = 0,
    /// Something unexpected failed.
    Failure = 1,
    /// The arguments are wrong.
    Usage = 2,
    /// The invite or the request was refused: AR-101, AR-102, AR-301, AR-302, AR-303.
    Refused = 3,
    /// A device of another hub is in the folder: AR-105.
    OtherHub = 4,
    /// The hub was not reached, or ended the connection.
    Hub = 5,
    /// The versions of the protocol do not agree: AR-401, AR-402.
    Version = 6,
    /// The registration in Hermes failed: AR-601.
    Registration = 7,
    /// Leaving is pending: AR-505.
    LeavePending = 8,
    /// A problem of this machine.
    Local = 9,
    /// The files were deleted and the registration in Hermes was not removed.
    Unregister = 10,
}

impl Exit {
    /// The number.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// The exit code of what a command said when it failed.
    #[must_use]
    pub const fn of(said: Said) -> Self {
        match said {
            Said::Code(code, _) => match code {
                ErrorCode::Ar101
                | ErrorCode::Ar102
                | ErrorCode::Ar301
                | ErrorCode::Ar302
                | ErrorCode::Ar303 => Exit::Refused,
                ErrorCode::Ar105 => Exit::OtherHub,
                ErrorCode::Ar201
                | ErrorCode::Ar202
                | ErrorCode::Ar204
                | ErrorCode::Ar205
                | ErrorCode::Ar206
                | ErrorCode::Ar208 => Exit::Hub,
                ErrorCode::Ar401 | ErrorCode::Ar402 => Exit::Version,
                ErrorCode::Ar601 => Exit::Registration,
                ErrorCode::Ar505 => Exit::LeavePending,
                _ => Exit::Failure,
            },
            Said::Unreachable(_) => Exit::Hub,
            Said::Local(Local::HubAddress | Local::PlainNotLocal | Local::HubKey) => Exit::Usage,
            Said::Local(Local::Unregister) => Exit::Unregister,
            Said::Local(Local::Internal) => Exit::Failure,
            Said::Local(_) => Exit::Local,
        }
    }
}

/// The connector of the Arauto: beside a Hermes, with the channel to the hub.
#[derive(Debug, Parser)]
#[command(
    name = "arauto-connector",
    version,
    about = "The Arauto connector: beside a Hermes, with the channel to the hub.",
    disable_help_subcommand = true
)]
struct Cli {
    /// The folder of the connector (default: the folder .arauto of the user).
    #[arg(long, global = true, env = "ARAUTO_HOME", value_name = "FOLDER")]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Asks the hub to connect this Hermes, with the block of the admin, and registers the
    /// connector in Hermes.
    Join {
        /// The address of the hub.
        hub: String,
        /// The key of the hub.
        key: String,
        /// The invite.
        invite: String,
        /// Does not register the connector in Hermes (for development).
        #[arg(long)]
        no_register: bool,
    },
    /// The server of tools that Hermes starts.
    Mcp,
    /// Shows the situation, without any secret.
    Status,
    /// Asks the hub to close this device, then deletes its files and the registration.
    Leave {
        /// Deletes the files and the registration without the hub. For a person, when the hub
        /// is gone or changed its key: the device still counts on the hub until it is revoked.
        #[arg(long)]
        local: bool,
    },
}

/// What a wrong command line gets: the usage, and nothing it was given.
const USAGE: &str = "Usage: arauto-connector [--home FOLDER] <join HUB KEY INVITE [--no-register] | mcp | status | leave [--local]>";

/// Runs the command line `arguments`, with `hermes` the value of `ARAUTO_HERMES_BIN`, and
/// returns the exit code.
pub async fn run(arguments: Vec<OsString>, hermes: Option<OsString>) -> Exit {
    run_waking(arguments, hermes, None, None).await
}

/// [`run`], with the values of [`crate::wake::URL_VARIABLE`] and
/// [`crate::wake::KEY_VARIABLE`]: where the local API of Hermes is, and its key. Only the
/// command `mcp` uses them.
pub async fn run_waking(
    arguments: Vec<OsString>,
    hermes: Option<OsString>,
    api_url: Option<OsString>,
    api_key: Option<OsString>,
) -> Exit {
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            return match error.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    let _ = error.print();
                    Exit::Done
                }
                // The message of the library repeats the arguments, and one may be the
                // invite.
                _ => {
                    eprintln!("{USAGE}");
                    Exit::Usage
                }
            };
        }
    };
    let home = match Home::resolve(cli.home.as_deref()) {
        Ok(home) => home,
        Err(_) => {
            eprintln!("{}", Local::Folder.phrase());
            return Exit::Local;
        }
    };
    let hermes = Hermes::new(hermes);
    let settings = Settings::default();
    match cli.command {
        Command::Join {
            hub,
            key,
            invite,
            no_register,
        } => {
            let program = std::env::current_exe().unwrap_or_default();
            let args = JoinArgs {
                hub,
                key,
                invite: Zeroizing::new(invite),
                register: !no_register,
                program,
            };
            join(&home, args, &hermes, &settings).await
        }
        Command::Mcp => {
            let url = api_url.as_deref().and_then(std::ffi::OsStr::to_str);
            let key = api_key.as_deref().and_then(std::ffi::OsStr::to_str);
            init_log();
            // A value that cannot be used is no waking, never a connector that does not
            // start: the tools of the Hermes go on. But it is said, and never the value.
            let target = match crate::wake::WakeTarget::from_environment(url, key) {
                Ok(Some(target)) => Some(target),
                Ok(None) => {
                    tracing::warn!(
                        "waking is off: ARAUTO_HERMES_API_KEY is not set, so no notice wakes this Hermes"
                    );
                    None
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "waking is off: ARAUTO_HERMES_API_URL must be http:// with an address of \
                         this machine written as numbers and a port, like \
                         http://127.0.0.1:8642/v1/runs, and ARAUTO_HERMES_API_KEY one line \
                         without spaces"
                    );
                    None
                }
            };
            mcp(home, settings.waking(target)).await
        }
        Command::Status => {
            let report = crate::report::report(&home, &settings).await;
            print!("{}", crate::report::render(&report));
            Exit::Done
        }
        Command::Leave { local } => leave(&home, local, &hermes, &settings).await,
    }
}

async fn join(home: &Home, args: JoinArgs, hermes: &Hermes, settings: &Settings) -> Exit {
    match crate::join::join(home, args, hermes, settings, SystemClock.now_ms()).await {
        Ok(Joined::Enrolled {
            code,
            minutes,
            replaces,
            registered,
        }) => {
            println!("Arauto connector {}", env!("CARGO_PKG_VERSION"));
            println!("Connection request sent to the hub.");
            if registered {
                println!(
                    "MCP server \"arauto\" registered in this Hermes (no WhatsApp tools until \
                     the approval)."
                );
            } else {
                println!("MCP server \"arauto\" not registered in Hermes, as asked.");
            }
            println!("VERIFICATION CODE: {code}");
            println!(
                "The admin types this code on the hub within {minutes} minute(s) of the first \
                 enrollment."
            );
            if replaces {
                println!("This code replaces the one shown before, which is no longer valid.");
            }
            Exit::Done
        }
        Ok(Joined::AlreadyApproved { .. }) => {
            println!("{}", ErrorCode::Ar104.phrase(None));
            Exit::Done
        }
        Err(said) => {
            eprintln!("{said}");
            Exit::of(said)
        }
    }
}

async fn leave(home: &Home, local: bool, hermes: &Hermes, settings: &Settings) -> Exit {
    match crate::leave::leave(home, local, hermes, settings).await {
        Ok(Left::Nothing) => {
            println!("{}", Local::NoDevice.phrase());
            Exit::Done
        }
        Ok(Left::Done {
            unregistered: false,
            ..
        }) => {
            eprintln!("{}", Local::Unregister.phrase());
            Exit::Unregister
        }
        Ok(Left::Done { .. }) if local => {
            println!(
                "The files of this device were deleted and the MCP server arauto was removed \
                 from Hermes. The device still counts on the hub until the admin revokes it."
            );
            Exit::Done
        }
        Ok(Left::Done { .. }) => {
            println!(
                "This device left the hub. Its files were deleted and the MCP server arauto \
                 was removed from Hermes."
            );
            Exit::Done
        }
        Err(said) => {
            eprintln!("{said}");
            Exit::of(said)
        }
    }
}

/// How long the connector has to stop in order once the server of tools ended: Hermes waits 2
/// seconds after it closes the input, and the program still has to end.
const STOP_BUDGET: Duration = Duration::from_millis(1_300);

/// The server of tools: MCP on the standard input and output (`crate::mcp`), until Hermes
/// closes the input, a stop signal comes, or a `join` asks the program to end. Then the channel
/// closes in order and the program ends.
async fn mcp(home: Home, settings: Settings) -> Exit {
    init_log();
    let stop = arauto_platform::shutdown_signal();
    let connector = match Connector::start(home, settings).await {
        Ok(connector) => connector,
        Err(_) => {
            eprintln!("{}", Local::Folder.phrase());
            return Exit::Local;
        }
    };
    let ended = {
        let signal = async {
            match stop {
                Ok(stop) => stop.await,
                Err(_) => std::future::pending().await,
            }
        };
        let stop = async {
            tokio::select! {
                () = signal => {}
                () = connector.finished() => {}
            }
        };
        let input = tokio::io::BufReader::with_capacity(64 * 1024, tokio::io::stdin());
        crate::mcp::serve(&connector, input, tokio::io::stdout(), stop).await
    };
    tracing::info!(?ended, "the server of tools ended");
    if tokio::time::timeout(STOP_BUDGET, connector.shutdown())
        .await
        .is_err()
    {
        tracing::warn!("the connector did not stop in time");
    }
    Exit::Done
}

/// The log of the server of tools: on the standard error, at a fixed level, never from the
/// environment. The standard output is the channel of MCP, and no secret ever reaches the log.
fn init_log() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_stable() {
        let codes = [
            (Exit::Done, 0),
            (Exit::Failure, 1),
            (Exit::Usage, 2),
            (Exit::Refused, 3),
            (Exit::OtherHub, 4),
            (Exit::Hub, 5),
            (Exit::Version, 6),
            (Exit::Registration, 7),
            (Exit::LeavePending, 8),
            (Exit::Local, 9),
            (Exit::Unregister, 10),
        ];
        for (exit, code) in codes {
            assert_eq!(exit.code(), code);
        }
        assert_eq!(Exit::of(Said::code(ErrorCode::Ar102)), Exit::Refused);
        assert_eq!(Exit::of(Said::code(ErrorCode::Ar105)), Exit::OtherHub);
        assert_eq!(Exit::of(Said::code(ErrorCode::Ar202)), Exit::Hub);
        assert_eq!(Exit::of(Said::code(ErrorCode::Ar601)), Exit::Registration);
        assert_eq!(Exit::of(Said::code(ErrorCode::Ar505)), Exit::LeavePending);
        assert_eq!(Exit::of(Said::Local(Local::PlainNotLocal)), Exit::Usage);
        assert_eq!(Exit::of(Said::Local(Local::Busy)), Exit::Local);
    }

    #[test]
    fn the_command_line_has_the_four_commands() {
        let parse = |line: &[&str]| Cli::try_parse_from(line.iter().map(OsString::from));
        assert!(matches!(
            parse(&["arauto-connector", "join", "h", "k", "i", "--no-register"])
                .unwrap()
                .command,
            Command::Join {
                no_register: true,
                ..
            }
        ));
        assert!(matches!(
            parse(&["arauto-connector", "--home", "x", "mcp"])
                .unwrap()
                .command,
            Command::Mcp
        ));
        assert!(matches!(
            parse(&["arauto-connector", "status", "--home", "x"])
                .unwrap()
                .command,
            Command::Status
        ));
        assert!(matches!(
            parse(&["arauto-connector", "leave", "--local"])
                .unwrap()
                .command,
            Command::Leave { local: true }
        ));
        assert!(parse(&["arauto-connector", "join", "h", "k"]).is_err());
        assert!(parse(&["arauto-connector", "update"]).is_err());
        assert!(parse(&["arauto-connector", "help"]).is_err());
    }
}
