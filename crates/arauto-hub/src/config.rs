//! Configuration of the hub, from environment variables (`docs/15-mvp.md`, section 4).
//!
//! | Variable | For what |
//! |---|---|
//! | `ARAUTO_DATABASE_URL` | The PostgreSQL database. |
//! | `ARAUTO_DATA_DIR` | The folder of the master key and of the audit mark, outside the volume of the database. |
//! | `ARAUTO_LISTEN` | The address of the port of the connectors. |
//! | `ARAUTO_LISTEN_INTERNAL` | The address of the internal port (health, engine events), never published. |
//! | `ARAUTO_PUBLIC_URL` | The address of the hub that goes in the block. |
//! | `ARAUTO_CLIENT_IP_SOURCE` | Where the address of a connector comes from: `socket` or `forwarded`. |
//! | `ARAUTO_TRUSTED_PROXIES` | With `forwarded`: the proxies whose word is taken, as addresses or networks. |
//! | `ARAUTO_ALLOW_UNSAFE_DB` | `1` lets the hub start on a database with `fsync` off. For tests only. |
//!
//! Each command asks only for what it uses: [`StorageConfig`] for the commands that open the
//! database and the data folder, [`PublicUrl`] for the command that shows the block, and
//! [`ServeConfig`] for the hub itself. Every problem of a command is reported at once, by the
//! name of the variable, and the value of the database address is never repeated: it may
//! carry a password.

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

/// The name of each variable.
pub mod names {
    /// The PostgreSQL database.
    pub const DATABASE_URL: &str = "ARAUTO_DATABASE_URL";
    /// The folder of the master key and of the audit mark.
    pub const DATA_DIR: &str = "ARAUTO_DATA_DIR";
    /// The address of the port of the connectors.
    pub const LISTEN: &str = "ARAUTO_LISTEN";
    /// The address of the internal port.
    pub const LISTEN_INTERNAL: &str = "ARAUTO_LISTEN_INTERNAL";
    /// The address of the hub that goes in the block.
    pub const PUBLIC_URL: &str = "ARAUTO_PUBLIC_URL";
    /// Where the address of a connector comes from.
    pub const CLIENT_IP_SOURCE: &str = "ARAUTO_CLIENT_IP_SOURCE";
    /// The proxies whose word is taken.
    pub const TRUSTED_PROXIES: &str = "ARAUTO_TRUSTED_PROXIES";
    /// Lets the hub start on a database that does not force its writes to disk.
    pub const ALLOW_UNSAFE_DB: &str = "ARAUTO_ALLOW_UNSAFE_DB";
}

/// The longest public address accepted.
const PUBLIC_URL_MAX_LEN: usize = 255;

/// The most networks accepted in the list of trusted proxies.
const TRUSTED_PROXIES_MAX: usize = 64;

/// Where the variables are read from. The process environment in the programs; a closure in
/// the tests.
pub trait Lookup {
    /// The value of the variable `name`, when it is set and is valid Unicode.
    fn get(&self, name: &str) -> Option<String>;
}

impl<F: Fn(&str) -> Option<String>> Lookup for F {
    fn get(&self, name: &str) -> Option<String> {
        self(name)
    }
}

/// The environment of the process.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEnv;

impl Lookup for ProcessEnv {
    fn get(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// One variable that is missing or wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The variable.
    pub variable: &'static str,
    /// What is wrong and what is expected. Never the value of a secret.
    pub what: String,
}

/// The configuration is not usable. Lists every problem of the command at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// Every variable that is missing or wrong, in a fixed order.
    pub problems: Vec<Problem>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the configuration is not usable:")?;
        for problem in &self.problems {
            write!(f, "\n  {}: {}", problem.variable, problem.what)?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

/// The address of the database. It may carry a password: it prints `[redacted]`.
#[derive(Clone, PartialEq, Eq)]
pub struct DatabaseUrl(String);

impl DatabaseUrl {
    /// Wraps an address that was already checked elsewhere, or that a test built.
    #[must_use]
    pub fn new(url: String) -> Self {
        Self(url)
    }

    /// The address. Whoever calls this answers for where it goes: never a log or an error.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DatabaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseUrl([redacted])")
    }
}

/// What every command that touches the state of the hub needs: `init`, `migrate`, `admin`
/// and `serve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    /// The database.
    pub database_url: DatabaseUrl,
    /// The folder of the master key and of the audit mark.
    pub data_dir: PathBuf,
    /// The explicit test option: start even when the database does not force writes to disk.
    pub allow_unsafe_db: bool,
}

impl StorageConfig {
    /// Reads `ARAUTO_DATABASE_URL`, `ARAUTO_DATA_DIR` and `ARAUTO_ALLOW_UNSAFE_DB`.
    ///
    /// # Errors
    /// [`ConfigError`] with every variable that is missing or wrong.
    pub fn read(env: &impl Lookup) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let storage = read_storage(env, &mut problems);
        finish(storage, problems)
    }
}

/// The address of the hub that goes in the block: where the connectors reach the port of the
/// connectors from outside. `https://host[:port]`, or `http://` for this machine only, in
/// development. No path, no query and no user: the paths are fixed by the protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    secure: bool,
    authority: String,
}

impl PublicUrl {
    /// Reads `ARAUTO_PUBLIC_URL`. Only the command that shows the block and the hub need it.
    ///
    /// # Errors
    /// [`ConfigError`] when the variable is missing or is not such an address.
    pub fn read(env: &impl Lookup) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let url = read_public_url(env, &mut problems);
        finish(url, problems)
    }

    /// Checks an address.
    ///
    /// # Errors
    /// The rule that the text breaks, as a sentence for the person who configures the hub.
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.len() > PUBLIC_URL_MAX_LEN {
            return Err("the address is too long");
        }
        let (secure, rest) = if let Some(rest) = text.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = text.strip_prefix("http://") {
            (false, rest)
        } else {
            return Err("the address starts with https:// (or http:// for this machine only)");
        };
        let authority = rest.strip_suffix('/').unwrap_or(rest);
        if authority.is_empty() {
            return Err("the address has no host");
        }
        if authority.contains(['/', '?', '#', '@', '\\']) {
            return Err("the address is only the host and the port: no path, query or user");
        }
        let (host, port) = split_port(authority);
        if let Some(port) = port
            && !matches!(port.parse::<u16>(), Ok(1..))
        {
            return Err("the port is a number from 1 to 65535");
        }
        if !is_host(host) {
            return Err("the host is a name or an address, in ASCII");
        }
        if !secure && !is_this_machine(host) {
            return Err("http:// is only accepted for 127.0.0.1, [::1] and localhost");
        }
        Ok(Self {
            secure,
            authority: authority.to_ascii_lowercase(),
        })
    }

    /// The address of the websocket of the channel: `wss://host/v1/channel`.
    #[must_use]
    pub fn channel_url(&self) -> String {
        let scheme = if self.secure { "wss" } else { "ws" };
        format!("{scheme}://{}/v1/channel", self.authority)
    }

    /// The address a file of the download list is served at.
    #[must_use]
    pub fn download_url(&self, file: &str) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{scheme}://{}/v1/download/{file}", self.authority)
    }
}

/// A network of proxies: an address and how many of its leading bits count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network {
    address: IpAddr,
    prefix: u8,
}

impl Network {
    /// Reads `192.0.2.7`, `192.0.2.0/24`, `2001:db8::1` or `2001:db8::/32`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (address, prefix) = match text.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (text, None),
        };
        let address: IpAddr = address.parse().ok()?;
        let widest: u8 = if address.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            // No sign, no spaces and no leading zeros: one way to write each network.
            Some(text) => {
                let prefix: u8 = text.parse().ok()?;
                if prefix > widest || prefix.to_string() != text {
                    return None;
                }
                prefix
            }
            None => widest,
        };
        Some(Self { address, prefix })
    }

    /// Whether `candidate` is inside this network. An IPv4 address is never inside an IPv6
    /// network, nor the other way around.
    #[must_use]
    pub fn contains(&self, candidate: IpAddr) -> bool {
        match (self.address, candidate) {
            (IpAddr::V4(network), IpAddr::V4(candidate)) => {
                let shift = 32u32.saturating_sub(u32::from(self.prefix));
                let mask = u32::MAX.checked_shl(shift).unwrap_or(0);
                (u32::from(network) & mask) == (u32::from(candidate) & mask)
            }
            (IpAddr::V6(network), IpAddr::V6(candidate)) => {
                let shift = 128u32.saturating_sub(u32::from(self.prefix));
                let mask = u128::MAX.checked_shl(shift).unwrap_or(0);
                (u128::from(network) & mask) == (u128::from(candidate) & mask)
            }
            _ => false,
        }
    }
}

/// Where the address of a connector comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientIpSource {
    /// The address of the socket: nothing sits between the connectors and the hub.
    Socket,
    /// The forwarding header, believed only when the socket is one of these proxies.
    Forwarded {
        /// The proxies whose word is taken. Never empty.
        trusted_proxies: Vec<Network>,
    },
}

/// What `serve` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeConfig {
    /// The database and the data folder.
    pub storage: StorageConfig,
    /// The address of the port of the connectors.
    pub listen: SocketAddr,
    /// The address of the internal port.
    pub listen_internal: SocketAddr,
    /// The address that goes in the block.
    pub public_url: PublicUrl,
    /// Where the address of a connector comes from. There is no default: a wrong guess
    /// either lets anyone forge the address or shows the proxy as every connector.
    pub client_ip: ClientIpSource,
}

impl ServeConfig {
    /// Reads every variable of the hub.
    ///
    /// # Errors
    /// [`ConfigError`] with every variable that is missing or wrong.
    pub fn read(env: &impl Lookup) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let storage = read_storage(env, &mut problems);
        let listen = read_listen(
            env,
            names::LISTEN,
            "the port of the connectors",
            &mut problems,
        );
        let listen_internal = read_listen(
            env,
            names::LISTEN_INTERNAL,
            "the internal port",
            &mut problems,
        );
        if let (Some(listen), Some(internal)) = (listen, listen_internal)
            && listen == internal
        {
            problems.push(Problem {
                variable: names::LISTEN_INTERNAL,
                what: format!(
                    "is the same address as {}; the internal port is another one, and is \
                     never published",
                    names::LISTEN
                ),
            });
        }
        let public_url = read_public_url(env, &mut problems);
        let client_ip = read_client_ip(env, &mut problems);
        let config = match (storage, listen, listen_internal, public_url, client_ip) {
            (
                Some(storage),
                Some(listen),
                Some(listen_internal),
                Some(public_url),
                Some(client_ip),
            ) => Some(Self {
                storage,
                listen,
                listen_internal,
                public_url,
                client_ip,
            }),
            _ => None,
        };
        finish(config, problems)
    }
}

fn finish<T>(value: Option<T>, problems: Vec<Problem>) -> Result<T, ConfigError> {
    match value {
        Some(value) if problems.is_empty() => Ok(value),
        _ => Err(ConfigError { problems }),
    }
}

/// The value of a variable, with an empty one counted as not set.
fn value(env: &impl Lookup, variable: &str) -> Option<String> {
    env.get(variable).filter(|text| !text.trim().is_empty())
}

fn missing(variable: &'static str, what_for: &str, example: &str) -> Problem {
    Problem {
        variable,
        what: format!("is not set; it is {what_for}, like {example}"),
    }
}

fn read_storage(env: &impl Lookup, problems: &mut Vec<Problem>) -> Option<StorageConfig> {
    let database_url = match value(env, names::DATABASE_URL) {
        Some(url) if url.starts_with("postgres://") || url.starts_with("postgresql://") => {
            Some(DatabaseUrl(url))
        }
        // The value is not shown: it may carry a password.
        Some(_) => {
            problems.push(Problem {
                variable: names::DATABASE_URL,
                what: "is not a PostgreSQL address; it starts with postgres://".to_owned(),
            });
            None
        }
        None => {
            problems.push(missing(
                names::DATABASE_URL,
                "the PostgreSQL database of the hub",
                "postgres://arauto@127.0.0.1:5432/arauto",
            ));
            None
        }
    };
    let data_dir = match value(env, names::DATA_DIR) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            problems.push(missing(
                names::DATA_DIR,
                "the folder of the master key and of the audit mark, outside the volume of \
                 the database",
                "/var/lib/arauto",
            ));
            None
        }
    };
    let allow_unsafe_db = match value(env, names::ALLOW_UNSAFE_DB).as_deref() {
        None => Some(false),
        Some("1") => Some(true),
        Some(_) => {
            problems.push(Problem {
                variable: names::ALLOW_UNSAFE_DB,
                what: "is either not set or 1; it is a test option that lets the hub start on \
                       a database that does not force its writes to disk"
                    .to_owned(),
            });
            None
        }
    };
    Some(StorageConfig {
        database_url: database_url?,
        data_dir: data_dir?,
        allow_unsafe_db: allow_unsafe_db?,
    })
}

fn read_listen(
    env: &impl Lookup,
    variable: &'static str,
    what_for: &str,
    problems: &mut Vec<Problem>,
) -> Option<SocketAddr> {
    let Some(text) = value(env, variable) else {
        problems.push(missing(
            variable,
            &format!("the address {what_for} listens on"),
            "127.0.0.1:8443",
        ));
        return None;
    };
    match text.parse::<SocketAddr>() {
        Ok(address) if address.port() != 0 => Some(address),
        _ => {
            problems.push(Problem {
                variable,
                what: "is not an address with a port, like 127.0.0.1:8443 or [::]:8443".to_owned(),
            });
            None
        }
    }
}

fn read_public_url(env: &impl Lookup, problems: &mut Vec<Problem>) -> Option<PublicUrl> {
    let Some(text) = value(env, names::PUBLIC_URL) else {
        problems.push(missing(
            names::PUBLIC_URL,
            "the address of the hub that goes in the block",
            "https://hub.example.com",
        ));
        return None;
    };
    match PublicUrl::parse(&text) {
        Ok(url) => Some(url),
        Err(rule) => {
            problems.push(Problem {
                variable: names::PUBLIC_URL,
                what: format!("is not usable: {rule}"),
            });
            None
        }
    }
}

fn read_client_ip(env: &impl Lookup, problems: &mut Vec<Problem>) -> Option<ClientIpSource> {
    let proxies = value(env, names::TRUSTED_PROXIES);
    let Some(source) = value(env, names::CLIENT_IP_SOURCE) else {
        problems.push(Problem {
            variable: names::CLIENT_IP_SOURCE,
            what: "is not set; it is `socket` when the connectors reach the hub directly, or \
                   `forwarded` when a proxy sits in front of it (then ARAUTO_TRUSTED_PROXIES \
                   lists the proxies)"
                .to_owned(),
        });
        return None;
    };
    match source.as_str() {
        "socket" => {
            if proxies.is_some() {
                problems.push(Problem {
                    variable: names::TRUSTED_PROXIES,
                    what: "is set, but ARAUTO_CLIENT_IP_SOURCE is `socket`, which trusts no \
                           proxy; unset it, or use `forwarded`"
                        .to_owned(),
                });
                return None;
            }
            Some(ClientIpSource::Socket)
        }
        "forwarded" => {
            let Some(list) = proxies else {
                problems.push(Problem {
                    variable: names::TRUSTED_PROXIES,
                    what: "is not set, and ARAUTO_CLIENT_IP_SOURCE is `forwarded`; it lists \
                           the proxies in front of the hub, like 192.0.2.10,198.51.100.0/24"
                        .to_owned(),
                });
                return None;
            };
            let items: Vec<&str> = list.split(',').map(str::trim).collect();
            let networks: Option<Vec<Network>> =
                items.iter().map(|item| Network::parse(item)).collect();
            match networks {
                Some(networks) if networks.len() <= TRUSTED_PROXIES_MAX => {
                    Some(ClientIpSource::Forwarded {
                        trusted_proxies: networks,
                    })
                }
                _ => {
                    problems.push(Problem {
                        variable: names::TRUSTED_PROXIES,
                        what: format!(
                            "is not a list of up to {TRUSTED_PROXIES_MAX} addresses or \
                             networks separated by commas, like 192.0.2.10,198.51.100.0/24"
                        ),
                    });
                    None
                }
            }
        }
        _ => {
            problems.push(Problem {
                variable: names::CLIENT_IP_SOURCE,
                what: "is either `socket` or `forwarded`".to_owned(),
            });
            None
        }
    }
}

/// Splits `host:port`. An IPv6 address comes in brackets, so its colons are not the port.
fn split_port(authority: &str) -> (&str, Option<&str>) {
    if authority.starts_with('[') {
        return match authority.split_once("]:") {
            Some((host, port)) => (&authority[..=host.len()], Some(port)),
            None => (authority, None),
        };
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    }
}

/// A DNS name, an IPv4 address, or an IPv6 address in brackets.
fn is_host(host: &str) -> bool {
    if let Some(inner) = host.strip_prefix('[') {
        return inner
            .strip_suffix(']')
            .is_some_and(|address| address.parse::<std::net::Ipv6Addr>().is_ok());
    }
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn is_this_machine(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "[::1]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Lookup {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    const FULL: [(&str, &str); 7] = [
        (
            names::DATABASE_URL,
            "postgres://arauto:s3cr3t-pass@db.example.com/arauto",
        ),
        (names::DATA_DIR, "/var/lib/arauto"),
        (names::LISTEN, "0.0.0.0:8443"),
        (names::LISTEN_INTERNAL, "127.0.0.1:9000"),
        (names::PUBLIC_URL, "https://hub.example.com"),
        (names::CLIENT_IP_SOURCE, "forwarded"),
        (
            names::TRUSTED_PROXIES,
            "192.0.2.10, 198.51.100.0/24,2001:db8::/32",
        ),
    ];

    fn without(variable: &str) -> Vec<(&'static str, &'static str)> {
        FULL.iter()
            .copied()
            .filter(|(name, _)| *name != variable)
            .collect()
    }

    fn with(variable: &'static str, value: &'static str) -> Vec<(&'static str, &'static str)> {
        let mut pairs = without(variable);
        pairs.push((variable, value));
        pairs
    }

    fn variables(error: &ConfigError) -> Vec<&'static str> {
        error.problems.iter().map(|p| p.variable).collect()
    }

    #[test]
    fn the_hub_reads_every_variable() {
        let config = ServeConfig::read(&env(&FULL)).unwrap();
        assert_eq!(
            config.storage.database_url.expose(),
            "postgres://arauto:s3cr3t-pass@db.example.com/arauto"
        );
        assert_eq!(config.storage.data_dir, PathBuf::from("/var/lib/arauto"));
        assert!(!config.storage.allow_unsafe_db);
        assert_eq!(config.listen, "0.0.0.0:8443".parse().unwrap());
        assert_eq!(config.listen_internal, "127.0.0.1:9000".parse().unwrap());
        assert_eq!(
            config.public_url.channel_url(),
            "wss://hub.example.com/v1/channel"
        );
        let ClientIpSource::Forwarded { trusted_proxies } = config.client_ip else {
            panic!("forwarded was asked for");
        };
        assert_eq!(trusted_proxies.len(), 3);
    }

    #[test]
    fn each_command_asks_only_for_what_it_uses() {
        // `init`, `migrate` and `admin` run with the database and the data folder alone.
        let storage = [
            (
                names::DATABASE_URL,
                "postgres://arauto@127.0.0.1:5432/arauto",
            ),
            (names::DATA_DIR, "data"),
        ];
        let config = StorageConfig::read(&env(&storage)).unwrap();
        assert_eq!(config.data_dir, PathBuf::from("data"));

        // The block needs the public address and nothing else.
        let block = [(names::PUBLIC_URL, "https://hub.example.com")];
        assert!(PublicUrl::read(&env(&block)).is_ok());
        assert!(StorageConfig::read(&env(&block)).is_err());

        // And the hub needs all of it.
        let error = ServeConfig::read(&env(&storage)).unwrap_err();
        assert_eq!(
            variables(&error),
            [
                names::LISTEN,
                names::LISTEN_INTERNAL,
                names::PUBLIC_URL,
                names::CLIENT_IP_SOURCE
            ]
        );
    }

    #[test]
    fn everything_missing_is_said_at_once_and_by_name() {
        let error = ServeConfig::read(&env(&[])).unwrap_err();
        assert_eq!(
            variables(&error),
            [
                names::DATABASE_URL,
                names::DATA_DIR,
                names::LISTEN,
                names::LISTEN_INTERNAL,
                names::PUBLIC_URL,
                names::CLIENT_IP_SOURCE
            ]
        );
        let text = error.to_string();
        for variable in variables(&error) {
            assert!(text.contains(variable), "{variable} in {text}");
        }
        assert!(text.contains("is not set"), "{text}");

        let error = StorageConfig::read(&env(&[])).unwrap_err();
        assert_eq!(variables(&error), [names::DATABASE_URL, names::DATA_DIR]);
    }

    #[test]
    fn an_empty_variable_counts_as_not_set() {
        let error = ServeConfig::read(&env(&with(names::DATA_DIR, "  "))).unwrap_err();
        assert_eq!(variables(&error), [names::DATA_DIR]);
        assert!(error.to_string().contains("is not set"));
    }

    #[test]
    fn the_client_ip_source_has_no_default() {
        let mut pairs = without(names::CLIENT_IP_SOURCE);
        pairs.retain(|(name, _)| *name != names::TRUSTED_PROXIES);
        let error = ServeConfig::read(&env(&pairs)).unwrap_err();
        assert_eq!(variables(&error), [names::CLIENT_IP_SOURCE]);

        let error =
            ServeConfig::read(&env(&with(names::CLIENT_IP_SOURCE, "x-real-ip"))).unwrap_err();
        assert_eq!(variables(&error), [names::CLIENT_IP_SOURCE]);
    }

    #[test]
    fn forwarded_needs_the_proxies_and_socket_refuses_them() {
        let error = ServeConfig::read(&env(&without(names::TRUSTED_PROXIES))).unwrap_err();
        assert_eq!(variables(&error), [names::TRUSTED_PROXIES]);

        let error = ServeConfig::read(&env(&with(names::CLIENT_IP_SOURCE, "socket"))).unwrap_err();
        assert_eq!(variables(&error), [names::TRUSTED_PROXIES]);

        let mut socket = with(names::CLIENT_IP_SOURCE, "socket");
        socket.retain(|(name, _)| *name != names::TRUSTED_PROXIES);
        let config = ServeConfig::read(&env(&socket)).unwrap();
        assert_eq!(config.client_ip, ClientIpSource::Socket);

        for list in [
            "",
            ",",
            "192.0.2.1,",
            "proxy.example.com",
            "192.0.2.0/33",
            "*",
        ] {
            let pairs: Vec<(&str, &str)> = without(names::TRUSTED_PROXIES)
                .into_iter()
                .chain([(names::TRUSTED_PROXIES, list)])
                .collect();
            let error = ServeConfig::read(&env(&pairs)).unwrap_err();
            assert_eq!(variables(&error), [names::TRUSTED_PROXIES], "{list:?}");
        }
    }

    #[test]
    fn the_database_address_never_shows() {
        let secret = "s3cr3t-pass";
        let config = ServeConfig::read(&env(&FULL)).unwrap();
        assert!(!format!("{config:?}").contains(secret));
        assert!(!format!("{:?}", config.storage).contains(secret));
        assert_eq!(
            format!("{:?}", config.storage.database_url),
            "DatabaseUrl([redacted])"
        );

        // A wrong address is refused without being repeated.
        let wrong = with(
            names::DATABASE_URL,
            "mysql://root:s3cr3t-pass@db.example.com/x",
        );
        let error = ServeConfig::read(&env(&wrong)).unwrap_err();
        assert_eq!(variables(&error), [names::DATABASE_URL]);
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
    }

    #[test]
    fn the_unsafe_database_option_is_explicit() {
        let on = with(names::ALLOW_UNSAFE_DB, "1");
        assert!(
            ServeConfig::read(&env(&on))
                .unwrap()
                .storage
                .allow_unsafe_db
        );
        for other in ["0", "true", "yes", "on", "2", " 1 x"] {
            let pairs: Vec<(&str, &str)> = FULL
                .iter()
                .copied()
                .chain([(names::ALLOW_UNSAFE_DB, other)])
                .collect();
            let error = ServeConfig::read(&env(&pairs)).unwrap_err();
            assert_eq!(variables(&error), [names::ALLOW_UNSAFE_DB], "{other:?}");
        }
    }

    #[test]
    fn the_listen_addresses_are_addresses_with_a_port_and_differ() {
        for wrong in [
            "8443",
            "localhost:8443",
            "0.0.0.0",
            "0.0.0.0:0",
            "0.0.0.0:99999",
        ] {
            let pairs: Vec<(&str, &str)> = without(names::LISTEN)
                .into_iter()
                .chain([(names::LISTEN, wrong)])
                .collect();
            let error = ServeConfig::read(&env(&pairs)).unwrap_err();
            assert_eq!(variables(&error), [names::LISTEN], "{wrong:?}");
        }
        let same = with(names::LISTEN_INTERNAL, "0.0.0.0:8443");
        let error = ServeConfig::read(&env(&same)).unwrap_err();
        assert_eq!(variables(&error), [names::LISTEN_INTERNAL]);
        let v6 = with(names::LISTEN, "[::]:8443");
        assert!(ServeConfig::read(&env(&v6)).is_ok());
    }

    #[test]
    fn the_public_address_is_a_host_over_tls() {
        let channel = |text: &str| PublicUrl::parse(text).map(|url| url.channel_url());
        assert_eq!(
            channel("https://hub.example.com").unwrap(),
            "wss://hub.example.com/v1/channel"
        );
        assert_eq!(
            channel("https://Hub.Example.com:8443/").unwrap(),
            "wss://hub.example.com:8443/v1/channel"
        );
        assert_eq!(
            channel("https://[2001:db8::1]:8443").unwrap(),
            "wss://[2001:db8::1]:8443/v1/channel"
        );
        assert_eq!(
            channel("http://127.0.0.1:8443").unwrap(),
            "ws://127.0.0.1:8443/v1/channel"
        );
        assert_eq!(
            channel("http://localhost:8443").unwrap(),
            "ws://localhost:8443/v1/channel"
        );
        assert_eq!(
            PublicUrl::parse("https://hub.example.com")
                .unwrap()
                .download_url("arauto-connector"),
            "https://hub.example.com/v1/download/arauto-connector"
        );

        let long = format!("https://{}.example.com", "a".repeat(300));
        for wrong in [
            "",
            "hub.example.com",
            "wss://hub.example.com",
            "ftp://hub.example.com",
            "https://",
            "https:///",
            "https://hub.example.com/path",
            "https://hub.example.com/?q=1",
            "https://hub.example.com#x",
            "https://user:pass@hub.example.com",
            "https://hub.example.com:0",
            "https://hub.example.com:port",
            "https://hub.example.com:65536",
            "https://hub..example.com",
            "https://-hub.example.com",
            "https://hub example.com",
            "https://h\u{00fc}b.example.com",
            "https://[not-an-address]",
            // Without TLS, only this machine.
            "http://hub.example.com",
            "http://192.0.2.1:8443",
            "http://127.0.0.1.example.com",
            long.as_str(),
        ] {
            assert!(PublicUrl::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn a_network_contains_its_addresses_and_no_other() {
        let network = |text: &str| Network::parse(text).unwrap();
        let ip = |text: &str| text.parse::<IpAddr>().unwrap();

        assert!(network("192.0.2.7").contains(ip("192.0.2.7")));
        assert!(!network("192.0.2.7").contains(ip("192.0.2.8")));
        assert!(network("198.51.100.0/24").contains(ip("198.51.100.255")));
        assert!(!network("198.51.100.0/24").contains(ip("198.51.101.0")));
        assert!(network("198.51.100.77/24").contains(ip("198.51.100.1")));
        assert!(network("0.0.0.0/0").contains(ip("203.0.113.9")));
        assert!(!network("0.0.0.0/0").contains(ip("2001:db8::1")));
        assert!(network("2001:db8::/32").contains(ip("2001:db8:ffff::1")));
        assert!(!network("2001:db8::/32").contains(ip("2001:db9::1")));
        assert!(network("::/0").contains(ip("2001:db8::1")));
        assert!(!network("::/0").contains(ip("192.0.2.1")));
        assert!(network("2001:db8::1").contains(ip("2001:db8::1")));
        assert!(!network("2001:db8::1").contains(ip("2001:db8::2")));
        // An IPv4 address written as IPv6 is another address: no proxy is trusted by it.
        assert!(!network("192.0.2.0/24").contains(ip("::ffff:192.0.2.1")));

        for wrong in [
            "",
            "/",
            "192.0.2.0/",
            "192.0.2.0/33",
            "192.0.2.0/-1",
            "192.0.2.0/+8",
            "192.0.2.0/08",
            "192.0.2/24",
            "2001:db8::/129",
            "example.com",
            "192.0.2.0/24/8",
            " 192.0.2.1",
        ] {
            assert!(Network::parse(wrong).is_none(), "{wrong:?}");
        }
    }

    #[test]
    fn the_process_environment_is_a_source() {
        // Only reads: a variable nobody sets.
        assert_eq!(
            ProcessEnv.get("ARAUTO_TEST_VARIABLE_THAT_IS_NEVER_SET"),
            None
        );
    }
}
