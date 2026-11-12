//! The `arauto-hub` command. It only reads the arguments and the environment, calls the
//! functions of the library and prints what they report.
#![forbid(unsafe_code)]

use std::io::{BufRead, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use arauto_hub::admin::render::{self, Report, access, link, whatsapp};
use arauto_hub::admin::whatsapp::{PairMethod, Removal};
use arauto_hub::admin::{self, Session};
use arauto_hub::channel::{self, ChannelSettings};
use arauto_hub::config::{ProcessEnv, PublicUrl, ServeConfig, StorageConfig};
use arauto_hub::core::access::{Grant, Scope, Verbs};
use arauto_hub::core::model::DeviceState;
use arauto_hub::db::{self, PoolOptions};
use arauto_hub::download;
use arauto_hub::engine::EngineSettings;
use arauto_hub::mark::Mark;
use arauto_hub::repo::alert::AlertFilter;
use arauto_hub::repo::approval::{ApproveOutcome, Recognition, Recognized};
use arauto_hub::vault::Vault;
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
    /// Asks the hub that runs on this machine whether it is up, by its internal port. Ends
    /// with 0 when it answers: what the health check of a container runs.
    Health,
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
    /// The WhatsApp engine: where it is, its key, the secret of its events.
    Engine {
        #[command(subcommand)]
        command: EngineCommand,
    },
    /// The WhatsApp numbers and their pairing.
    Number {
        #[command(subcommand)]
        command: NumberCommand,
    },
    /// The matrix of accesses: what each Hermes may do on each number and in each chat.
    Access {
        #[command(subcommand)]
        command: AccessCommand,
    },
    /// The list of peers: which Hermes may send messages to which.
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
}

#[derive(Debug, Subcommand)]
enum LinkCommand {
    /// Lets one Hermes send messages to another, or changes the wake mark of the link.
    Add {
        /// The Hermes that may send.
        from: String,
        /// The Hermes that receives.
        to: String,
        /// A message of FROM wakes TO (TO must also be allowed to be woken: `admin wake`).
        #[arg(long)]
        wake: bool,
        /// The link back too, from TO to FROM, with the same wake mark.
        #[arg(long)]
        both: bool,
    },
    /// Takes the link from one Hermes to another away.
    Remove {
        /// The Hermes that could send.
        from: String,
        /// The Hermes that received.
        to: String,
        /// The link back too.
        #[arg(long)]
        both: bool,
    },
    /// Lists the links.
    List,
}

/// The verbs given on the command line.
#[derive(Debug, Clone, Copy, clap::Args)]
struct VerbArgs {
    /// See chats and messages, download files, see groups, contacts and the state.
    #[arg(long)]
    read: bool,
    /// Write, react, mark as read, type, forward, ask whether a telephone has WhatsApp.
    #[arg(long)]
    send: bool,
    /// Edit and delete messages.
    #[arg(long)]
    modify: bool,
    /// Be woken when a message arrives. Needs --read.
    #[arg(long)]
    wake: bool,
}

impl VerbArgs {
    const fn verbs(self) -> Verbs {
        Verbs {
            read: self.read,
            send: self.send,
            modify: self.modify,
            wake: self.wake,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ScopeArg {
    /// Every chat of the number, those that come later included.
    All,
    /// Only the chats given a line with `access chat`.
    List,
}

#[derive(Debug, Subcommand)]
enum AccessCommand {
    /// Gives a Hermes an access to a number, or changes it whole: the scope and the verbs.
    Grant {
        /// The name of the Hermes.
        hermes: String,
        /// The alias of the number.
        number: String,
        /// Which chats the verbs reach.
        #[arg(long, value_enum)]
        scope: ScopeArg,
        #[command(flatten)]
        verbs: VerbArgs,
        /// May write first to someone who never talked to the number. Needs --send and
        /// --scope all.
        #[arg(long = "start-chats")]
        start_chats: bool,
    },
    /// Takes the access of a Hermes to a number away, with the lines of its chats.
    Remove {
        /// The name of the Hermes.
        hermes: String,
        /// The alias of the number.
        number: String,
    },
    /// Gives one chat its own verbs, which replace the verbs of the number there. With no
    /// verb, the chat is blocked.
    Chat {
        /// The name of the Hermes.
        hermes: String,
        /// The alias of the number.
        number: String,
        /// A telephone with the code of the country, an identifier of the engine, or an
        /// identifier shown by `access list`.
        chat: String,
        #[command(flatten)]
        verbs: VerbArgs,
    },
    /// Takes the line of one chat away.
    ChatRemove {
        /// The name of the Hermes.
        hermes: String,
        /// The alias of the number.
        number: String,
        /// The chat, as `access chat` takes it.
        chat: String,
    },
    /// Lists the accesses, with the lines of their chats.
    List {
        /// Only the accesses of this Hermes.
        hermes: Option<String>,
    },
    /// How long a chat must be quiet before its messages wake the Hermes: whoever writes
    /// usually sends a few messages in a row.
    WakeWait {
        /// The name of the Hermes.
        hermes: String,
        /// The alias of the number.
        number: String,
        /// Seconds, from 0 (at once) to 60. An access starts with 6.
        seconds: u32,
    },
}

#[derive(Debug, Subcommand)]
enum EngineCommand {
    /// Creates the engine, or changes its addresses.
    Set {
        /// Where the hub reaches the engine: http://host:port, on the private network.
        #[arg(long)]
        url: String,
        /// Where the engine reaches the internal port of the hub: http://host:port, with no
        /// underscore in the name.
        #[arg(long = "events-url")]
        events_url: String,
    },
    /// Sets the key of the API of the engine, read from the terminal (never an argument).
    SetKey,
    /// Draws a new secret for the events of the engine. It is never shown.
    NewEventsSecret,
    /// The engine: its addresses, whether its secrets are set, the version seen.
    Show,
    /// Asks the engine whether it is healthy.
    Health,
    /// Asks the engine its version, and says whether it is one the hub was checked against.
    Version,
}

#[derive(Debug, Subcommand)]
enum NumberCommand {
    /// Adds a number, unpaired.
    Add {
        /// The alias: lowercase letters, digits, hyphen and underscore, up to 32.
        alias: String,
        /// The telephone, with the code of the country.
        phone: String,
        /// Marks the number as a command channel of some Hermes.
        #[arg(long)]
        command: bool,
    },
    /// Lists the numbers.
    List {
        /// Shows the removed ones too.
        #[arg(long)]
        all: bool,
    },
    /// Pairs a number: shows a QR (or, with --code, a code to type on the telephone).
    Pair {
        /// The alias of the number.
        alias: String,
        /// A code to type on the telephone instead of a QR.
        #[arg(long)]
        code: bool,
    },
    /// Disconnects a number: its session stops, without leaving WhatsApp.
    Disconnect {
        /// The alias of the number.
        alias: String,
    },
    /// Restarts a failed session of the engine.
    Restart {
        /// The alias of the number.
        alias: String,
    },
    /// Accepts the new telephone of a swapped number.
    Confirm {
        /// The alias of the number.
        alias: String,
    },
    /// Undoes the pairing of a swapped number. Leaving WhatsApp erases the history the
    /// engine keeps.
    Unpair {
        /// The alias of the number.
        alias: String,
        /// Says that the history the engine keeps of this number may be erased.
        #[arg(long = "erase-engine-history", required = true)]
        erase_engine_history: bool,
    },
    /// Removes a number: every access to it ends.
    #[command(group = clap::ArgGroup::new("removal").required(true).args(["keep_in_engine", "erase_engine_history"]))]
    Remove {
        /// The alias of the number.
        alias: String,
        /// Stops its session and keeps it in the engine, with its history.
        #[arg(long = "keep-in-engine")]
        keep_in_engine: bool,
        /// Leaves WhatsApp and deletes its session: the engine erases its history.
        #[arg(long = "erase-engine-history")]
        erase_engine_history: bool,
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
        Command::Serve => return serve().await,
        Command::Health => return health().await,
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
            let mut report = run_installed(&session, &storage, command, now).await?;
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

/// `arauto-hub serve`: the hub, until SIGTERM (Ctrl+C on Windows).
async fn serve() -> anyhow::Result<Report> {
    // Listening starts here, so a request to stop during the start is not lost.
    let shutdown = arauto_platform::shutdown_signal()?;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init();
    let config = ServeConfig::read(&ProcessEnv)?;
    let options = channel::Options {
        settings: ChannelSettings::default(),
        downloads: downloads()?.0,
        ..channel::Options::default()
    };
    let running = channel::start(&config, options, Arc::new(SystemClock)).await?;
    shutdown.await;
    running.stop().await;
    Ok(Report {
        text: "The hub stopped.\n".to_owned(),
        done: true,
    })
}

/// The files of the folder of downloads, when the environment names one.
fn downloads() -> anyhow::Result<(channel::Downloads, Vec<download::Listed>)> {
    match std::env::var_os(download::DOWNLOAD_DIR) {
        Some(dir) if !dir.is_empty() => download::load(std::path::Path::new(&dir))
            .with_context(|| format!("could not read the folder of {}", download::DOWNLOAD_DIR)),
        _ => Ok((channel::Downloads::empty(), Vec::new())),
    }
}

/// `arauto-hub health`: one request to the internal port of the hub of this machine.
async fn health() -> anyhow::Result<Report> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listen = std::env::var(arauto_hub::config::names::LISTEN_INTERNAL)
        .context("ARAUTO_LISTEN_INTERNAL is not set")?;
    let listen: std::net::SocketAddr = listen
        .parse()
        .context("ARAUTO_LISTEN_INTERNAL is not an address with a port")?;
    // The hub may listen on every address: it is asked on this machine.
    let address = if listen.ip().is_unspecified() {
        let local: std::net::IpAddr = if listen.is_ipv4() {
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        };
        std::net::SocketAddr::new(local, listen.port())
    } else {
        listen
    };
    let asked = async {
        let mut stream = tokio::net::TcpStream::connect(address).await.ok()?;
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n",
            channel::HEALTH_PATH
        );
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut head = [0u8; 64];
        let read = stream.read(&mut head).await.ok()?;
        let line = std::str::from_utf8(head.get(..read)?).ok()?;
        Some(line.starts_with("HTTP/1.1 200"))
    };
    let up = matches!(
        tokio::time::timeout(Duration::from_secs(3), asked).await,
        Ok(Some(true))
    );
    Ok(Report {
        text: if up {
            "The hub is up.\n".to_owned()
        } else {
            "The hub does not answer on its internal port.\n".to_owned()
        },
        done: up,
    })
}

/// Opens the master key: the commands of the engine and of the numbers need it.
async fn vault(storage: &StorageConfig) -> anyhow::Result<Vault> {
    let data_dir = storage.data_dir.clone();
    Ok(tokio::task::spawn_blocking(move || Vault::open(&data_dir))
        .await
        .context("could not open the master key")??)
}

/// Reads the key of the engine from the terminal: one line, never an argument, so that it
/// stays out of the history of the shell.
fn read_key() -> anyhow::Result<zeroize::Zeroizing<String>> {
    let mut out = std::io::stdout().lock();
    write!(
        out,
        "Paste the key of the API of the engine and press Enter: "
    )?;
    out.flush()?;
    let mut line = zeroize::Zeroizing::new(String::new());
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("could not read the key")?;
    Ok(line)
}

async fn run_engine(
    session: &Session,
    storage: &StorageConfig,
    command: EngineCommand,
    now: i64,
) -> anyhow::Result<Report> {
    if let EngineCommand::Show = command {
        return Ok(whatsapp::engine_show(session.engine_show().await?.as_ref()));
    }
    let vault = vault(storage).await?;
    Ok(match command {
        EngineCommand::Set { url, events_url } => {
            whatsapp::engine_set(session.engine_set(&vault, &url, &events_url, now).await?)
        }
        EngineCommand::SetKey => {
            let typed = read_key()?;
            session.engine_set_key(&vault, &typed, now).await?;
            whatsapp::engine_secret("The key of the engine was set")
        }
        EngineCommand::NewEventsSecret => {
            session.engine_new_events_secret(&vault, now).await?;
            whatsapp::engine_secret("A new secret of the events was drawn")
        }
        EngineCommand::Health => match session.engine_health(&vault).await {
            Ok(()) => whatsapp::engine_health(Ok(())),
            Err(admin::AdminError::Engine(error)) => whatsapp::engine_health(Err(error)),
            Err(other) => return Err(other.into()),
        },
        EngineCommand::Version => {
            whatsapp::engine_version(&session.engine_version(&vault, now).await?)
        }
        EngineCommand::Show => whatsapp::engine_show(session.engine_show().await?.as_ref()),
    })
}

async fn run_number(
    session: &Session,
    storage: &StorageConfig,
    command: NumberCommand,
    now: i64,
) -> anyhow::Result<Report> {
    let vault = vault(storage).await?;
    Ok(match command {
        NumberCommand::Add {
            alias,
            phone,
            command,
        } => whatsapp::number_added(
            &session
                .number_add(&vault, &alias, &phone, command, now)
                .await?,
        ),
        NumberCommand::List { all } => {
            let mut listed = Vec::new();
            for number in session.number_list(all).await? {
                let phone = session.number_phone(&vault, &number).await.ok();
                listed.push((number, phone));
            }
            whatsapp::number_list(&listed)
        }
        NumberCommand::Pair { alias, code } => {
            let method = if code {
                PairMethod::Code
            } else {
                PairMethod::Qr
            };
            let outcome = session
                .number_pair(
                    &vault,
                    &alias,
                    method,
                    &EngineSettings::default(),
                    &SystemClock,
                    |shown| print!("{}", whatsapp::shown(&shown)),
                )
                .await?;
            whatsapp::paired(&alias, &outcome)
        }
        NumberCommand::Disconnect { alias } => whatsapp::number_changed(
            "was disconnected",
            &session.number_disconnect(&vault, &alias, now).await?,
        ),
        NumberCommand::Restart { alias } => whatsapp::number_changed(
            "was restarted",
            &session.number_restart(&vault, &alias, now).await?,
        ),
        NumberCommand::Confirm { alias } => {
            whatsapp::number_confirmed(&session.number_confirm(&alias, now).await?)
        }
        NumberCommand::Unpair { alias, .. } => whatsapp::number_changed(
            "left WhatsApp and its pairing was undone",
            &session.number_unpair(&vault, &alias, now).await?,
        ),
        NumberCommand::Remove {
            alias,
            erase_engine_history,
            ..
        } => {
            let removal = if erase_engine_history {
                Removal::EraseEngineHistory
            } else {
                Removal::KeepInEngine
            };
            whatsapp::number_changed(
                "was removed",
                &session.number_remove(&vault, &alias, removal, now).await?,
            )
        }
    })
}

async fn run_access(
    session: &Session,
    storage: &StorageConfig,
    command: AccessCommand,
    now: i64,
) -> anyhow::Result<Report> {
    Ok(match command {
        AccessCommand::Grant {
            hermes,
            number,
            scope,
            verbs,
            start_chats,
        } => {
            let grant = Grant {
                scope: match scope {
                    ScopeArg::All => Scope::All,
                    ScopeArg::List => Scope::List,
                },
                verbs: verbs.verbs(),
                can_start_chats: start_chats,
            };
            access::granted(
                &hermes,
                &number,
                &session.access_grant(&hermes, &number, grant, now).await?,
            )
        }
        AccessCommand::Remove { hermes, number } => access::removed(
            &hermes,
            &number,
            session.access_remove(&hermes, &number, now).await?,
        ),
        AccessCommand::Chat {
            hermes,
            number,
            chat,
            verbs,
        } => {
            let vault = vault(storage).await?;
            access::chat_changed(
                &hermes,
                &number,
                &session
                    .access_chat(&vault, &hermes, &number, &chat, verbs.verbs(), now)
                    .await?,
            )
        }
        AccessCommand::ChatRemove {
            hermes,
            number,
            chat,
        } => {
            let vault = vault(storage).await?;
            access::chat_changed(
                &hermes,
                &number,
                &session
                    .access_chat_remove(&vault, &hermes, &number, &chat, now)
                    .await?,
            )
        }
        AccessCommand::WakeWait {
            hermes,
            number,
            seconds,
        } => access::wake_wait(
            &hermes,
            &number,
            seconds,
            session
                .access_wake_wait(&hermes, &number, seconds, now)
                .await?,
        ),
        AccessCommand::List { hermes } => {
            let vault = vault(storage).await?;
            access::list(&session.access_list(&vault, hermes.as_deref()).await?)
        }
    })
}

async fn run_installed(
    session: &Session,
    storage: &StorageConfig,
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
            let listed = downloads()?.1;
            render::block(
                &session.invite(&name, &public_url, now).await?,
                &listed,
                now,
            )
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
        InstalledCommand::Engine { command } => run_engine(session, storage, command, now).await?,
        InstalledCommand::Number { command } => run_number(session, storage, command, now).await?,
        InstalledCommand::Access { command } => run_access(session, storage, command, now).await?,
        InstalledCommand::Link { command } => match command {
            LinkCommand::Add {
                from,
                to,
                wake,
                both,
            } => link::linked(&session.link_add(&from, &to, wake, both, now).await?),
            LinkCommand::Remove { from, to, both } => {
                link::unlinked(&session.link_remove(&from, &to, both, now).await?)
            }
            LinkCommand::List => link::list(&session.link_list().await?),
        },
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
            "arauto-hub admin engine set --url http://192.0.2.10:3000 --events-url http://hub:9000",
            "arauto-hub admin engine set-key",
            "arauto-hub admin engine new-events-secret",
            "arauto-hub admin engine show",
            "arauto-hub admin engine health",
            "arauto-hub admin engine version",
            "arauto-hub admin number add vendas +5511900000000",
            "arauto-hub admin number add vendas 5511900000000 --command",
            "arauto-hub admin number list",
            "arauto-hub admin number list --all",
            "arauto-hub admin number pair vendas",
            "arauto-hub admin number pair vendas --code",
            "arauto-hub admin number disconnect vendas",
            "arauto-hub admin number restart vendas",
            "arauto-hub admin number confirm vendas",
            "arauto-hub admin number unpair vendas --erase-engine-history",
            "arauto-hub admin number remove vendas --keep-in-engine",
            "arauto-hub admin number remove vendas --erase-engine-history",
            "arauto-hub admin access grant vendas balcao --scope all --read --send",
            "arauto-hub admin access grant vendas balcao --scope all --read --send --start-chats",
            "arauto-hub admin access grant vendas balcao --scope list",
            "arauto-hub admin access grant vendas balcao --scope list --read --wake",
            "arauto-hub admin access remove vendas balcao",
            "arauto-hub admin access chat vendas balcao +5511900000000 --read",
            "arauto-hub admin access chat vendas balcao 120363012345678901@g.us",
            "arauto-hub admin access chat-remove vendas balcao +5511900000000",
            "arauto-hub admin access list",
            "arauto-hub admin access list vendas",
            "arauto-hub admin access wake-wait vendas balcao 0",
            "arauto-hub admin access wake-wait vendas balcao 10",
            "arauto-hub admin link add vendas suporte",
            "arauto-hub admin link add vendas suporte --wake --both",
            "arauto-hub admin link remove vendas suporte",
            "arauto-hub admin link remove vendas suporte --both",
            "arauto-hub admin link list",
            "arauto-hub dev-reset --yes-erase-everything",
        ] {
            let parsed = Cli::try_parse_from(line.split(' '));
            assert!(parsed.is_ok(), "{line}: {:?}", parsed.err());
        }
        for line in [
            "arauto-hub admin approve vendas 123456 --recognized --not-recognized",
            "arauto-hub admin wake vendas maybe",
            "arauto-hub admin alerts --ack not-an-id",
            "arauto-hub admin engine set-key s3cr3t-engine-key",
            "arauto-hub admin engine set --url http://192.0.2.10:3000",
            "arauto-hub admin number unpair vendas",
            "arauto-hub admin number remove vendas",
            "arauto-hub admin number remove vendas --keep-in-engine --erase-engine-history",
            "arauto-hub admin",
            "arauto-hub admin access grant vendas balcao --read",
            "arauto-hub admin access grant vendas balcao --scope some --read",
            "arauto-hub admin access remove vendas",
            "arauto-hub admin access chat vendas balcao",
            "arauto-hub admin link add vendas",
            "arauto-hub admin link remove vendas",
            "arauto-hub admin link list vendas",
            "arauto-hub admin link",
        ] {
            assert!(Cli::try_parse_from(line.split(' ')).is_err(), "{line}");
        }
    }
}
