//! The check of the installation: a few things the hub looks at by itself, with how each one
//! turned out. One function, [`facts`], reads what the check looks at, and one, [`rows`], gives
//! the rows; the window of the panel (`GET /installation`) writes them as a table and
//! `arauto-hub admin check` prints them in English, so the two never say different things.
//!
//! Nothing here changes anything: it only reads. No row names the address or the key of the
//! WhatsApp engine, and none shows a secret.

use std::net::IpAddr;

use sqlx::PgPool;

use crate::config::{PasswordRotation, StorageConfig};
use crate::db::ResolvedOrg;
use crate::db_password;
use crate::download::Listed;
use crate::panel::text::{self, Lang, k};
use crate::repo::{self, RepoError};

/// How a row turned out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// It works.
    Ok,
    /// The admin looks: the hub cannot tell by itself.
    Check,
    /// It works, and something needs care.
    Attention,
    /// Not needed yet.
    Later,
    /// It was asked for and did not happen.
    Failed,
}

impl Status {
    /// The word of the state: a sentence of the screens.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Status::Ok => k("ok"),
            Status::Check => k("check"),
            Status::Attention => k("attention"),
            Status::Later => k("later"),
            Status::Failed => k("failed"),
        }
    }
}

/// What a row is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// The address the panel answers at.
    PanelAddress,
    /// The address the connectors reach.
    ConnectorsAddress,
    /// The address the hub sees for whoever is reading the window.
    YourAddress,
    /// The password of the database role.
    DatabasePassword,
    /// The connector the hub serves for a Hermes to download.
    Connector,
    /// The folder of the master key.
    DataFolder,
    /// The WhatsApp engine.
    Engine,
}

impl Subject {
    /// The label of the row: a sentence of the screens.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Subject::PanelAddress => k("Panel address"),
            Subject::ConnectorsAddress => k("Connectors address"),
            Subject::YourAddress => k("Your address"),
            Subject::DatabasePassword => k("Database password"),
            Subject::Connector => k("Connector"),
            Subject::DataFolder => k("Data folder"),
            Subject::Engine => k("WhatsApp engine"),
        }
    }
}

/// What a row says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detail {
    /// The panel works at this origin.
    PanelWorks {
        /// `https://host[:port]`.
        origin: String,
    },
    /// The connectors address, to be opened by the admin.
    ConnectorsOpen {
        /// `https://host[:port]`.
        origin: String,
    },
    /// The address of the connectors or of the panel is not set in this environment.
    NotSet,
    /// The hub sees the reader at this address.
    Seen {
        /// What the hub sees.
        address: IpAddr,
    },
    /// A peer outside `ARAUTO_TRUSTED_PROXIES` brought a forwarded address.
    UntrustedForwarder {
        /// The peer.
        address: IpAddr,
    },
    /// The hub sees an address of an internal network and no proxy is trusted.
    InternalNetwork {
        /// What the hub sees.
        address: IpAddr,
    },
    /// Only the running hub sees the address of whoever opens the window.
    OnlyTheRunningHub,
    /// The hub replaced the password.
    PasswordReplaced,
    /// The installation sets the password itself.
    PasswordOwn,
    /// The replacement was asked for and did not happen.
    PasswordFailed,
    /// The replacement was asked for and comes with the first passkey, which is not there yet.
    PasswordAtFirstAccess,
    /// The folder of downloads serves a connector.
    ConnectorReady,
    /// The folder of downloads has none.
    ConnectorMissing,
    /// What the data folder holds.
    DataFolder,
    /// An engine is set.
    EngineConfigured,
    /// No engine yet.
    EngineLater,
}

/// One row of the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What it is about.
    pub subject: Subject,
    /// How it turned out.
    pub status: Status,
    /// What it says.
    pub detail: Detail,
}

/// What the address of the reader looks like to the hub: known only by the running hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reader {
    /// The address the hub sees.
    pub seen: IpAddr,
    /// The socket the request came from.
    pub peer: IpAddr,
    /// The peer sent the forwarding header and is not a trusted proxy.
    pub untrusted_forwarder: bool,
    /// The hub reached `seen` from the peer itself: no trusted proxy spoke for it.
    pub no_trusted_proxy: bool,
}

/// What the check reads.
#[derive(Debug, Clone)]
pub struct Facts {
    /// The origin of `ARAUTO_PANEL_URL`.
    pub panel_origin: Option<String>,
    /// The origin of `ARAUTO_PUBLIC_URL`.
    pub public_origin: Option<String>,
    /// The hub replaced the password of its database role: the data folder keeps its own.
    pub password_replaced: bool,
    /// The installation asks the hub to replace it at the first access.
    pub password_asked: bool,
    /// The first passkey of the organization was registered: the durable mark of the first
    /// access.
    pub first_access_done: bool,
    /// The folder of downloads serves a connector.
    pub connector_served: bool,
    /// An engine is set.
    pub engine_set: bool,
    /// Who is reading, when the running hub answers for it. `None` in the command.
    pub reader: Option<Reader>,
}

/// Where the check reads what it says. The window and the command fill it from the same
/// places, and [`facts`] reads the rest the same way for both.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    /// The database of the hub.
    pub pool: &'a PgPool,
    /// The organization the panel and the command serve.
    pub organization: ResolvedOrg,
    /// Where the hub keeps its data, and whether it replaces the password of its database.
    pub storage: &'a StorageConfig,
    /// The connector the hub serves, as [`crate::panel::block::connector`] picks it from the
    /// folder of downloads.
    pub connector: Option<&'a Listed>,
}

/// What the check reads: the data folder, the configuration, the connector served, the mark of
/// the first access and whether an engine is set. The addresses and the reader are what the
/// caller knows. Only reads.
///
/// # Errors
/// What the database returns.
pub async fn facts(
    sources: Sources<'_>,
    panel_origin: Option<String>,
    public_origin: Option<String>,
    reader: Option<Reader>,
) -> Result<Facts, RepoError> {
    let Sources {
        pool,
        organization,
        storage,
        connector,
    } = sources;
    Ok(Facts {
        panel_origin,
        public_origin,
        password_replaced: matches!(db_password::read_current(&storage.data_dir), Ok(Some(_))),
        password_asked: storage.password_rotation == PasswordRotation::AtFirstAccess,
        first_access_done: repo::panel::first_access_done(pool, organization).await?,
        connector_served: connector.is_some(),
        engine_set: repo::engine::get(pool, organization).await?.is_some(),
        reader,
    })
}

/// Whether an address belongs to an internal network, or to this machine.
#[must_use]
pub fn is_internal(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            // fc00::/7 (unique local) and fe80::/10 (link local) are not in std's stable API.
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

/// The rows of the check, in the order the window shows them.
#[must_use]
pub fn rows(facts: &Facts) -> Vec<Row> {
    let mut rows = Vec::with_capacity(7);
    rows.push(match &facts.panel_origin {
        Some(origin) => Row {
            subject: Subject::PanelAddress,
            status: Status::Ok,
            detail: Detail::PanelWorks {
                origin: origin.clone(),
            },
        },
        None => Row {
            subject: Subject::PanelAddress,
            status: Status::Later,
            detail: Detail::NotSet,
        },
    });
    rows.push(match &facts.public_origin {
        Some(origin) => Row {
            subject: Subject::ConnectorsAddress,
            status: Status::Check,
            detail: Detail::ConnectorsOpen {
                origin: origin.clone(),
            },
        },
        None => Row {
            subject: Subject::ConnectorsAddress,
            status: Status::Later,
            detail: Detail::NotSet,
        },
    });
    rows.push(match facts.reader {
        None => Row {
            subject: Subject::YourAddress,
            status: Status::Check,
            detail: Detail::OnlyTheRunningHub,
        },
        Some(reader) if reader.untrusted_forwarder => Row {
            subject: Subject::YourAddress,
            status: Status::Attention,
            detail: Detail::UntrustedForwarder {
                address: reader.peer,
            },
        },
        Some(reader) if reader.no_trusted_proxy && is_internal(reader.seen) => Row {
            subject: Subject::YourAddress,
            status: Status::Attention,
            detail: Detail::InternalNetwork {
                address: reader.seen,
            },
        },
        Some(reader) => Row {
            subject: Subject::YourAddress,
            status: Status::Check,
            detail: Detail::Seen {
                address: reader.seen,
            },
        },
    });
    let (status, detail) = if facts.password_replaced {
        (Status::Ok, Detail::PasswordReplaced)
    } else if facts.password_asked && !facts.first_access_done {
        // Nothing failed: the replacement comes with the first passkey.
        (Status::Later, Detail::PasswordAtFirstAccess)
    } else if facts.password_asked {
        (Status::Failed, Detail::PasswordFailed)
    } else {
        (Status::Check, Detail::PasswordOwn)
    };
    rows.push(Row {
        subject: Subject::DatabasePassword,
        status,
        detail,
    });
    rows.push(Row {
        subject: Subject::Connector,
        status: if facts.connector_served {
            Status::Ok
        } else {
            Status::Failed
        },
        detail: if facts.connector_served {
            Detail::ConnectorReady
        } else {
            Detail::ConnectorMissing
        },
    });
    rows.push(Row {
        subject: Subject::DataFolder,
        status: Status::Attention,
        detail: Detail::DataFolder,
    });
    rows.push(Row {
        subject: Subject::Engine,
        status: if facts.engine_set {
            Status::Ok
        } else {
            Status::Later
        },
        detail: if facts.engine_set {
            Detail::EngineConfigured
        } else {
            Detail::EngineLater
        },
    });
    rows
}

/// A value that goes in the place of a `{placeholder}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A value that came from outside: escaped by the page.
    Text(String),
    /// A command: shown as code.
    Code(&'static str),
}

impl Detail {
    /// The sentence of the row, as a template of the screens and what goes in it. `None` when
    /// the row has no sentence of its own beyond its value.
    #[must_use]
    pub fn sentence(&self) -> Option<(&'static str, Vec<(&'static str, Value)>)> {
        Some(match self {
            Detail::PanelWorks { .. } => (k("Your passkey works here."), vec![]),
            Detail::ConnectorsOpen { .. } => (
                k("Open it: it should say it is the address of the connectors."),
                vec![],
            ),
            Detail::NotSet | Detail::OnlyTheRunningHub | Detail::PasswordAtFirstAccess => {
                return None;
            }
            Detail::Seen { address } => (
                k(
                    "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
                ),
                vec![("address", Value::Text(address.to_string()))],
            ),
            Detail::UntrustedForwarder { address } => (
                k(
                    "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
                ),
                vec![("address", Value::Text(address.to_string()))],
            ),
            Detail::InternalNetwork { address } => (
                k(
                    "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
                ),
                vec![("address", Value::Text(address.to_string()))],
            ),
            Detail::PasswordReplaced => (k("Replaced by one nobody saw."), vec![]),
            Detail::PasswordOwn => (
                k(
                    "Not replaced by the hub: this installation sets the password of the database itself.",
                ),
                vec![],
            ),
            Detail::PasswordFailed => (
                k("The replacement failed. See the log of the hub, then run {command}."),
                vec![(
                    "command",
                    Value::Code("arauto-hub admin database rotate-password"),
                )],
            ),
            Detail::ConnectorReady => (k("Ready for your Hermes."), vec![]),
            Detail::ConnectorMissing => (
                k(
                    "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
                ),
                vec![],
            ),
            Detail::DataFolder => (
                k(
                    "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
                ),
                vec![],
            ),
            Detail::EngineConfigured => (k("Configured."), vec![]),
            Detail::EngineLater => (
                k("A Hermes can be connected without it; WhatsApp numbers need it."),
                vec![],
            ),
        })
    }

    /// The sentence as plain text, in `lang`.
    #[must_use]
    pub fn plain(&self, lang: Lang) -> String {
        let Some((key, values)) = self.sentence() else {
            return String::new();
        };
        let values: Vec<(&str, String)> = values
            .into_iter()
            .map(|(name, value)| {
                (
                    name,
                    match value {
                        Value::Text(text) => text,
                        Value::Code(code) => code.to_owned(),
                    },
                )
            })
            .collect();
        let borrowed: Vec<(&str, &str)> = values
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        text::fill(lang, key, &borrowed)
    }
}

/// What `arauto-hub admin check` prints: the rows in English, one block each.
#[must_use]
pub fn print(rows: &[Row]) -> String {
    let lang = Lang::En;
    let mut out = format!("{}\n\n", text::tr(lang, k("Check the installation")));
    for row in rows {
        let label = text::tr(lang, row.subject.label());
        let state = text::tr(lang, row.status.label());
        out.push_str(&format!("[{state}] {label}\n"));
        match &row.detail {
            Detail::PanelWorks { origin } | Detail::ConnectorsOpen { origin } => {
                out.push_str(&format!("    {origin}\n"));
            }
            Detail::NotSet => out.push_str("    Not set in this environment.\n"),
            Detail::OnlyTheRunningHub => out.push_str(
                "    Only the running hub sees it: open the panel and the check window.\n",
            ),
            Detail::PasswordAtFirstAccess => {
                out.push_str("    Replaced when the first passkey is registered.\n");
            }
            _ => {}
        }
        let sentence = row.detail.plain(lang);
        if !sentence.is_empty() {
            out.push_str(&format!("    {sentence}\n"));
        }
    }
    out.push_str(&format!(
        "\n{}\n",
        text::tr(
            lang,
            k("The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.")
        )
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            panel_origin: Some("https://panel.example.com".to_owned()),
            public_origin: Some("https://hub.example.com".to_owned()),
            password_replaced: true,
            password_asked: true,
            first_access_done: true,
            connector_served: true,
            engine_set: true,
            reader: None,
        }
    }

    fn status_of(rows: &[Row], subject: Subject) -> Status {
        rows.iter()
            .find(|row| row.subject == subject)
            .unwrap()
            .status
    }

    #[test]
    fn internal_addresses_are_told_from_public_ones() {
        for internal in [
            "10.0.0.1",
            "192.168.1.5",
            "172.16.0.9",
            "127.0.0.1",
            "::1",
            "fd00::1",
        ] {
            assert!(is_internal(internal.parse().unwrap()), "{internal}");
        }
        for public in ["203.0.113.9", "198.51.100.7", "2001:db8::1"] {
            assert!(!is_internal(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn the_rows_follow_what_the_hub_read() {
        let all = rows(&facts());
        assert_eq!(all.len(), 7);
        assert_eq!(status_of(&all, Subject::PanelAddress), Status::Ok);
        assert_eq!(status_of(&all, Subject::DatabasePassword), Status::Ok);
        assert_eq!(status_of(&all, Subject::Connector), Status::Ok);
        assert_eq!(status_of(&all, Subject::DataFolder), Status::Attention);
        assert_eq!(status_of(&all, Subject::Engine), Status::Ok);
        let worse = rows(&Facts {
            password_replaced: false,
            connector_served: false,
            engine_set: false,
            ..facts()
        });
        assert_eq!(status_of(&worse, Subject::DatabasePassword), Status::Failed);
        assert_eq!(status_of(&worse, Subject::Connector), Status::Failed);
        assert_eq!(status_of(&worse, Subject::Engine), Status::Later);
        let own = rows(&Facts {
            password_replaced: false,
            password_asked: false,
            ..facts()
        });
        assert_eq!(status_of(&own, Subject::DatabasePassword), Status::Check);
        // Asked for and not done before the first passkey: nothing failed yet.
        let early = rows(&Facts {
            password_replaced: false,
            first_access_done: false,
            ..facts()
        });
        assert_eq!(status_of(&early, Subject::DatabasePassword), Status::Later);
        let printed = print(&early);
        assert!(printed.contains("[later] Database password"), "{printed}");
        assert!(printed.contains("Replaced when the first passkey is registered."));
        assert!(!printed.contains("failed"), "{printed}");
    }

    #[test]
    fn the_address_of_the_reader_is_told_apart_by_what_the_hub_saw() {
        let reader = |seen: &str, untrusted, no_trusted_proxy| Reader {
            seen: seen.parse().unwrap(),
            peer: "10.0.0.2".parse().unwrap(),
            untrusted_forwarder: untrusted,
            no_trusted_proxy,
        };
        let with = |reader| {
            rows(&Facts {
                reader: Some(reader),
                ..facts()
            })
        };
        assert_eq!(
            status_of(
                &with(reader("203.0.113.9", false, false)),
                Subject::YourAddress
            ),
            Status::Check
        );
        assert_eq!(
            status_of(&with(reader("10.0.0.2", true, true)), Subject::YourAddress),
            Status::Attention
        );
        assert_eq!(
            status_of(&with(reader("10.0.0.2", false, true)), Subject::YourAddress),
            Status::Attention
        );
        // An internal address that a trusted proxy spoke for is a person on a network.
        assert_eq!(
            status_of(
                &with(reader("10.0.0.9", false, false)),
                Subject::YourAddress
            ),
            Status::Check
        );
        // A public address reached directly is fine.
        assert_eq!(
            status_of(
                &with(reader("203.0.113.9", false, true)),
                Subject::YourAddress
            ),
            Status::Check
        );
    }

    #[test]
    fn the_command_prints_what_the_window_shows_and_no_engine_address() {
        let printed = print(&rows(&facts()));
        assert!(printed.is_ascii());
        for needle in [
            "[ok] Panel address",
            "https://panel.example.com",
            "[check] Connectors address",
            "Only the running hub sees it",
            "Replaced by one nobody saw.",
            "[attention] Data folder",
            "Configured.",
        ] {
            assert!(printed.contains(needle), "{needle}\n{printed}");
        }
    }
}
