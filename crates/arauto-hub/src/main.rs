//! The `arauto-hub` command. It only reads the arguments and the environment, calls the
//! functions of the library and prints what they report.
#![forbid(unsafe_code)]

use std::io::{BufRead, Write};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context;
use arauto_hub::admin::render::{self, Report};
use arauto_hub::admin::{self, Session};
use arauto_hub::config::{ProcessEnv, PublicUrl, StorageConfig};
use arauto_hub::core::model::DeviceState;
use arauto_hub::db::{self, PoolOptions};
use arauto_hub::mark::Mark;
use arauto_hub::repo::alert::AlertFilter;
use arauto_hub::repo::approval::{ApproveOutcome, Recognition, Recognized};
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_protocol::message::DeclaredText;
use clap::{Parser, Subcommand, ValueEnum};
use uuid::Uuid;

/// The Arauto hub.
#[derive(Debug, Parser)]
#[command(name = "arauto-hub", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Runs the hub.
    Serve,
    #[command(flatten)]
    Stored(StoredCommand),
}

/// The commands that open the database and the data folder.
#[derive(Debug, Subcommand)]
enum StoredCommand {
    /// Creates the master key, the audit mark, the organization and the key of the hub. Does
    /// not redo what exists.
    Init,
    /// Applies pending migrations to a database that already has data.
    Migrate {
        /// Says that a backup of the database was taken before migrating.
        #[arg(long = "i-have-a-backup")]
        i_have_a_backup: bool,
    },
    /// What the panel would do, run by a person on the machine of the hub.
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    /// Development only: erases the database and the audit mark, together.
    DevReset {
        /// Confirms that everything in the database is to be erased.
        #[arg(long = "yes-erase-everything")]
        yes_erase_everything: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AdminCommand {
    /// The situation, the public key of the hub and the restored mode.
    Status,
    #[command(flatten)]
    Installed(InstalledCommand),
}

/// The admin commands that need an installed hub.
#[derive(Debug, Subcommand)]
enum InstalledCommand {
    /// The lines of Hermes.
    Hermes {
        #[command(subcommand)]
        command: HermesCommand,
    },
    /// Issues an invite and shows the block, to connect or to reconnect a Hermes.
    Invite {
        /// The name of the Hermes.
        name: String,
    },
    /// Approves the pending request of a Hermes with the code it answered.
    Approve {
        /// The name of the Hermes.
        name: String,
        /// The six digits the Hermes answered. Without it, the command asks.
        code: Option<String>,
        /// On a reconnect: the last use of the current device is recognized.
        #[arg(long, conflicts_with = "not_recognized")]
        recognized: bool,
        /// On a reconnect: the last use of the current device is NOT recognized.
        #[arg(long = "not-recognized")]
        not_recognized: bool,
    },
    /// Denies the pending request of a Hermes.
    Deny {
        /// The name of the Hermes.
        name: String,
    },
    /// Revokes a Hermes: its device, its request in progress and its issued invite.
    Revoke {
        /// The name of the Hermes.
        name: String,
    },
    /// Says whether a Hermes can be woken.
    Wake {
        /// The name of the Hermes.
        name: String,
        /// `on` or `off`.
        state: OnOff,
    },
    /// The alerts the panel would show.
    Alerts {
        /// Marks one alert as seen.
        #[arg(long, value_name = "ID")]
        ack: Option<Uuid>,
        /// Shows the ones already seen too.
        #[arg(long)]
        all: bool,
    },
    /// The audit.
    Audit {
        /// How many of the last events to show.
        #[arg(long, value_name = "N", default_value_t = 50)]
        tail: u32,
    },
}

#[derive(Debug, Subcommand)]
enum HermesCommand {
    /// Creates a Hermes line.
    Add {
        /// The name: lowercase letters, digits, hyphen and underscore, up to 32.
        name: String,
    },
    /// Lists the Hermes lines.
    List {
        /// Shows the removed ones too.
        #[arg(long)]
        all: bool,
    },
    /// Removes a Hermes line. Its name is never used again.
    Remove {
        /// The name of the Hermes.
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OnOff {
    On,
    Off,
}

/// How many alerts `admin alerts` shows.
const ALERTS_SHOWN: u32 = 200;

/// The system user that is running the command, as a declared text for the audit.
fn os_user() -> DeclaredText {
    let name = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let name = DeclaredText::lossy(name.trim());
    if name.as_str().is_empty() {
        DeclaredText::lossy("unknown")
    } else {
        name
    }
}

/// Asks for the verification code on the terminal.
fn ask_code() -> anyhow::Result<String> {
    let mut out = std::io::stdout().lock();
    write!(
        out,
        "Type the verification code the Hermes answered in the command channel: "
    )?;
    out.flush()?;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("could not read the code")?;
    Ok(line)
}

async fn run(command: Command) -> anyhow::Result<Report> {
    let command = match command {
        Command::Serve => {
            return Ok(Report {
                text: "`serve` is not in this build yet: the channel comes with the next stage \
                       of the MVP.\n"
                    .to_owned(),
                done: false,
            });
        }
        Command::Stored(command) => command,
    };
    let now = SystemClock.now_ms();
    let storage = StorageConfig::read(&ProcessEnv)?;
    let options = PoolOptions {
        size: 2,
        acquire_timeout: Duration::from_secs(5),
    };
    let pool = db::connect(&storage.database_url, options).await?;
    let mark = Mark::new(&storage.data_dir);
    match command {
        StoredCommand::Init => {
            let outcome = admin::init(
                &pool,
                &storage.data_dir,
                &os_user(),
                storage.allow_unsafe_db,
                now,
            )
            .await?;
            Ok(render::init(&outcome))
        }
        StoredCommand::Migrate { i_have_a_backup } => Ok(render::migrate(
            &admin::migrate(&pool, i_have_a_backup, now).await?,
        )),
        StoredCommand::DevReset {
            yes_erase_everything,
        } => {
            admin::dev_reset(&pool, &mark, yes_erase_everything).await?;
            Ok(Report {
                text: "The database and the audit mark were erased. Run `arauto-hub init`.\n"
                    .to_owned(),
                done: true,
            })
        }
        StoredCommand::Admin {
            command: AdminCommand::Status,
        } => Ok(render::status(&admin::status(&pool, &mark).await?)),
        StoredCommand::Admin {
            command: AdminCommand::Installed(command),
        } => {
            // A commit the server may lose would later read as a restored database.
            db::check_durability(&pool, storage.allow_unsafe_db).await?;
            let session = Session::open(pool, mark, os_user()).await?;
            let mut report = run_installed(&session, command, now).await?;
            // While the restored mode is on, every screen says so (C11).
            if let Some(notice) = session
                .installation()
                .await?
                .as_ref()
                .and_then(render::restored_notice)
            {
                report.text.insert_str(0, &notice);
            }
            Ok(report)
        }
    }
}

async fn run_installed(
    session: &Session,
    command: InstalledCommand,
    now: i64,
) -> anyhow::Result<Report> {
    Ok(match command {
        InstalledCommand::Hermes { command } => match command {
            HermesCommand::Add { name } => {
                let hermes = session.hermes_add(&name, now).await?;
                render::hermes_added(hermes.name.as_str())
            }
            HermesCommand::List { all } => render::hermes_list(&session.hermes_list(all).await?),
            HermesCommand::Remove { name } => {
                render::hermes_removed(&session.hermes_remove(&name, now).await?)
            }
        },
        InstalledCommand::Invite { name } => {
            let public_url = PublicUrl::read(&ProcessEnv)?;
            render::block(&session.invite(&name, &public_url, now).await?, now)
        }
        InstalledCommand::Approve {
            name,
            code,
            recognized,
            not_recognized,
        } => {
            let recognition = match (recognized, not_recognized) {
                (true, _) => Some(Recognition::Recognized),
                (_, true) => Some(Recognition::NotRecognized),
                _ => None,
            };
            approve(session, &name, code, recognition, now).await?
        }
        InstalledCommand::Deny { name } => render::denied(&name, session.deny(&name, now).await?),
        InstalledCommand::Revoke { name } => {
            render::revoked(&name, &session.revoke(&name, now).await?)
        }
        InstalledCommand::Wake { name, state } => {
            let on = state == OnOff::On;
            render::wake(&name, on, session.wake(&name, on, now).await?)
        }
        InstalledCommand::Alerts { ack: Some(id), .. } => {
            render::acknowledged(session.acknowledge(id, now).await?)
        }
        InstalledCommand::Alerts { ack: None, all } => {
            let filter = if all {
                AlertFilter::All
            } else {
                AlertFilter::Unseen
            };
            render::alerts(
                &session.alerts(filter, ALERTS_SHOWN).await?,
                &session.unknown_devices(ALERTS_SHOWN).await?,
            )
        }
        InstalledCommand::Audit { tail } => render::audit(&session.audit(tail).await?),
    })
}

/// `admin approve`: shows the request, asks what is missing and approves.
async fn approve(
    session: &Session,
    name: &str,
    code: Option<String>,
    recognition: Option<Recognition>,
    now: i64,
) -> anyhow::Result<Report> {
    // Only a pending request waits for a code.
    let pending = session
        .request(name)
        .await?
        .filter(|view| view.request.state == DeviceState::Pending);
    let Some(view) = pending else {
        return Ok(render::approved(name, &ApproveOutcome::NoRequest));
    };
    // The request, with the last use of the device it would replace, is always shown: an
    // answer about the last use is never accepted without the last use on the screen.
    let shown = render::request(name, &view);
    // The answer goes with what was shown: if the device is used after this reading, the
    // approval is refused and the new last use is shown.
    let recognition = recognition
        .zip(view.replaces.clone())
        .map(|(answer, shown)| Recognized { answer, shown });
    // On a reconnect the answer about the last use comes before the code.
    if let (Some(last_use), None) = (&view.replaces, &recognition) {
        let mut report =
            render::approved(name, &ApproveOutcome::NeedsRecognition(last_use.clone()));
        report.text.insert_str(0, &shown);
        return Ok(report);
    }
    let (typed, already_printed) = match code {
        Some(code) => (code, false),
        None => {
            print!("{shown}");
            (ask_code()?, true)
        }
    };
    let outcome = session.approve(name, &typed, recognition, now).await?;
    let mut report = render::approved(name, &outcome);
    if !already_printed {
        report.text.insert_str(0, &shown);
    }
    Ok(report)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("arauto-hub: could not start: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(cli.command)) {
        Ok(report) => {
            print!("{}", report.text);
            if report.done {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("arauto-hub: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn the_documented_commands_parse() {
        for line in [
            "arauto-hub init",
            "arauto-hub serve",
            "arauto-hub migrate --i-have-a-backup",
            "arauto-hub admin status",
            "arauto-hub admin hermes add vendas",
            "arauto-hub admin hermes list",
            "arauto-hub admin hermes remove vendas",
            "arauto-hub admin invite vendas",
            "arauto-hub admin approve vendas",
            "arauto-hub admin approve vendas 123-456",
            "arauto-hub admin approve vendas 123456 --recognized",
            "arauto-hub admin approve vendas 123456 --not-recognized",
            "arauto-hub admin deny vendas",
            "arauto-hub admin revoke vendas",
            "arauto-hub admin wake vendas on",
            "arauto-hub admin wake vendas off",
            "arauto-hub admin alerts",
            "arauto-hub admin alerts --ack 00000000-0000-0000-0000-000000000001",
            "arauto-hub admin audit",
            "arauto-hub admin audit --tail 20",
            "arauto-hub dev-reset --yes-erase-everything",
        ] {
            let parsed = Cli::try_parse_from(line.split(' '));
            assert!(parsed.is_ok(), "{line}: {:?}", parsed.err());
        }
        for line in [
            "arauto-hub admin approve vendas 123456 --recognized --not-recognized",
            "arauto-hub admin wake vendas maybe",
            "arauto-hub admin alerts --ack not-an-id",
            "arauto-hub admin",
        ] {
            assert!(Cli::try_parse_from(line.split(' ')).is_err(), "{line}");
        }
    }
}
