//! What the connector says: the code of the catalog and its fixed phrase, or a fixed phrase of
//! a local problem that has no code (`docs/11-conector.md`, section 8).
//!
//! Every text here is fixed, in English and in ASCII, compiled into the connector. It says what
//! happened and who solves it, and never brings a command to run, a path, or anything that came
//! from the hub or from a file. The phrases of the codes are the ones of
//! [`arauto_protocol::errors`]; the local ones follow the same rules and have the same test.

use core::fmt;

use arauto_protocol::errors::{ConnectFailure, ErrorCode, RetryAfter};

/// A problem of this machine that the catalog of codes does not cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Local {
    /// The address of the hub in the block is not valid.
    HubAddress,
    /// The address asks for a channel without TLS to another machine.
    PlainNotLocal,
    /// This build cannot reach a hub over TLS.
    NoTls,
    /// The key of the hub in the block is not valid.
    HubKey,
    /// The folder of the connector could not be used.
    Folder,
    /// A file of the connector is damaged or of an unknown format.
    Damaged,
    /// Another `join` or `leave` is running on the same folder.
    Busy,
    /// The connector could not copy itself to its fixed place.
    Program,
    /// The files were deleted, and removing the registration from Hermes failed.
    Unregister,
    /// There is no device in the folder.
    NoDevice,
    /// The generator of the system failed.
    Random,
    /// Anything else that went wrong on this machine.
    Internal,
}

impl Local {
    /// Every local problem.
    pub const ALL: &'static [Local] = &[
        Local::HubAddress,
        Local::PlainNotLocal,
        Local::NoTls,
        Local::HubKey,
        Local::Folder,
        Local::Damaged,
        Local::Busy,
        Local::Program,
        Local::Unregister,
        Local::NoDevice,
        Local::Random,
        Local::Internal,
    ];

    /// The fixed phrase.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Local::HubAddress => {
                "The hub address in the block is not valid. The admin must issue a new block."
            }
            Local::PlainNotLocal => {
                "The hub address asks for a channel without encryption to another machine, \
                 which is refused. The admin must issue a new block."
            }
            Local::NoTls => {
                "This build of the connector cannot reach a hub over encryption. Whoever installs \
                 must use the published build."
            }
            Local::HubKey => {
                "The key of the hub in the block is not valid. The admin must issue a new block."
            }
            Local::Folder => {
                "The folder of the connector could not be used: it must belong to this user and \
                 be closed to others. The operator must fix it."
            }
            Local::Damaged => {
                "A file of the connector is damaged or has an unknown format. Nothing was \
                 deleted. The operator must check it; a person may do the local leave."
            }
            Local::Busy => {
                "Another join or leave is running on this folder. Nothing was changed. Try again \
                 when it ends."
            }
            Local::Program => {
                "The connector could not copy itself to its fixed place. The connection request \
                 is still open; repeating the same join tries again."
            }
            Local::Unregister => {
                "The files of this device were deleted, but removing the MCP server arauto from \
                 Hermes failed. The operator must remove it from Hermes."
            }
            Local::NoDevice => "There is no device in this folder. There is nothing to leave.",
            Local::Random => {
                "The random generator of the system failed. Nothing was changed. The operator \
                 must check this machine."
            }
            Local::Internal => {
                "The connector failed unexpectedly on this machine. The operator must check the \
                 log of the connector."
            }
        }
    }
}

/// What a command reports when it does not succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// A code of the catalog.
    Code(ErrorCode, Option<RetryAfter>),
    /// AR-201, with its cause.
    Unreachable(ConnectFailure),
    /// A local problem.
    Local(Local),
}

impl Said {
    /// A code of the catalog, without a wait.
    #[must_use]
    pub const fn code(code: ErrorCode) -> Self {
        Said::Code(code, None)
    }
}

impl fmt::Display for Said {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Said::Code(code, wait) => f.write_str(&code.phrase(*wait)),
            Said::Unreachable(cause) => f.write_str(cause.phrase()),
            Said::Local(local) => f.write_str(local.phrase()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The local phrases follow the rules of the catalog: ASCII, one line, a whole sentence,
    /// and nothing an LLM could take as a command to run.
    #[test]
    fn local_phrases_follow_the_rules_of_the_catalog() {
        let forbidden = [
            "`",
            "$",
            "--",
            "sudo",
            "curl",
            "wget",
            "arauto-connector",
            "arauto-hub",
            "~",
            "http",
            "ws:",
            "|",
            "&&",
            "<",
            ">",
            "\\",
            "/",
        ];
        for local in Local::ALL {
            let phrase = local.phrase();
            assert!(phrase.is_ascii(), "{phrase}");
            assert!(
                phrase.bytes().all(|b| b == b' ' || b.is_ascii_graphic()),
                "{phrase}"
            );
            assert!(phrase.ends_with('.'), "{phrase}");
            assert!(phrase.len() >= 30 && phrase.len() <= 240, "{phrase}");
            assert!(!phrase.contains("  "), "{phrase}");
            for piece in forbidden {
                assert!(!phrase.contains(piece), "{piece:?} in: {phrase}");
            }
        }
        let distinct: std::collections::HashSet<&str> =
            Local::ALL.iter().map(|local| local.phrase()).collect();
        assert_eq!(distinct.len(), Local::ALL.len());
    }

    #[test]
    fn a_code_says_its_phrase() {
        assert_eq!(
            Said::code(ErrorCode::Ar505).to_string(),
            ErrorCode::Ar505.phrase(None)
        );
        assert!(
            Said::Unreachable(ConnectFailure::Refused)
                .to_string()
                .starts_with("AR-201: ")
        );
        assert_eq!(
            Said::Local(Local::Busy).to_string(),
            Local::Busy.phrase().to_owned()
        );
    }
}
