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
    /// The program of Hermes was not found, in the `PATH` nor in the usual places. Its sentence
    /// starts with AR-601, the code it has outside, and says what the catalog of the protocol
    /// does not: where to look.
    HermesNotFound,
    /// The files were deleted, and removing the registration from Hermes failed.
    Unregister,
    /// There is no device in the folder.
    NoDevice,
    /// There is no device in the folder to register in Hermes.
    NothingToRegister,
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
        Local::HermesNotFound,
        Local::Unregister,
        Local::NoDevice,
        Local::NothingToRegister,
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
            Local::HermesNotFound => {
                "AR-601: the program of Hermes was not found. The operator must set ARAUTO_HERMES_BIN to its \
                 path. The connection request is still open; repeating the same join tries \
                 again."
            }
            Local::Unregister => {
                "The files of this device were deleted, but removing the MCP server arauto from \
                 Hermes failed. The operator must remove it from Hermes."
            }
            Local::NoDevice => "There is no device in this folder. There is nothing to leave.",
            Local::NothingToRegister => {
                "There is no device in this folder. Nothing was registered in Hermes."
            }
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

/// What a command tells the operator when it succeeded but a thing is not as they may expect,
/// about the key of the local API of Hermes and about the lines kept in its memory. Fixed phrases, under the same rules as [`Local`].
/// The connector never restarts Hermes. It saves a key only in `join`, only when Hermes has none (KeyMade); these phrases say whose setting it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Told {
    /// The connector is registered, but waking needs the local API of Hermes to be on.
    WakingNeedsLocalApi,
    /// This version of Hermes refused the reference, so the entry has none.
    ReferenceRefused,
    /// A running Hermes keeps the entry it loaded.
    RunningKeepsOldEntry,
    /// The running connector did not receive the key.
    KeyNotReceived,
    /// What to expect when the setting already holds a key.
    KeyAfterRestart,
    /// `join` drew a key for the local API of a Hermes that had none and Hermes saved it.
    KeyMade,
    /// A key was made: the local API turns on only after the gateway restarts.
    RestartAfterKey,
    /// `join` did not confirm a new key.
    KeyNotMade,
    /// The entry was saved by an older connector.
    EntryOlder,
    /// The registration changed nothing in the connection.
    ConnectionUnchanged,
    /// After `leave`: the Arauto notes the Hermes kept in its memory are still there.
    NotesStayInMemory,
}

impl Told {
    /// Every phrase.
    pub const ALL: &'static [Told] = &[
        Told::WakingNeedsLocalApi,
        Told::ReferenceRefused,
        Told::RunningKeepsOldEntry,
        Told::KeyNotReceived,
        Told::KeyAfterRestart,
        Told::KeyMade,
        Told::RestartAfterKey,
        Told::KeyNotMade,
        Told::EntryOlder,
        Told::ConnectionUnchanged,
        Told::NotesStayInMemory,
    ];

    /// The fixed phrase.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Told::WakingNeedsLocalApi => {
                "This Hermes can be woken only while its local API is on, which Hermes does when \
                 its own setting API_SERVER_KEY holds a key. The status command says whether the \
                 connector received it."
            }
            Told::ReferenceRefused => {
                "This version of Hermes refused the reference to the key of its local API, so the \
                 entry was saved without it and no notice wakes this Hermes. Notes and tools \
                 still work."
            }
            Told::RunningKeepsOldEntry => {
                "A running Hermes keeps the entry it loaded: the new entry takes effect after \
                 Hermes restarts its gateway. The connector does not restart Hermes."
            }
            Told::KeyNotReceived => {
                "No notice wakes this Hermes until its local API is on. Hermes turns it on when \
                 its own setting API_SERVER_KEY, kept in the configuration of Hermes, holds a \
                 key."
            }
            Told::KeyAfterRestart => {
                "If that setting already holds a key, Hermes hands it to the connector after it \
                 restarts its gateway."
            }
            Told::KeyMade => {
                "This Hermes had no key for its local API, so the connector drew one and saved it \
                 in API_SERVER_KEY with the config command of Hermes. The connector shows it to \
                 nobody and writes it in no file of its own."
            }
            Told::RestartAfterKey => {
                "The local API of this Hermes turns on only after its gateway restarts. After the \
                 approval, the operator sends the restart command of Hermes to this Hermes in the \
                 chat, once."
            }
            Told::KeyNotMade => {
                "The connector did not confirm a new key for the local API of this Hermes. If the \
                 status command says the key is not received, the operator sets API_SERVER_KEY in \
                 the configuration of Hermes and restarts its gateway."
            }
            Told::EntryOlder => {
                "The entry of the connector in Hermes was saved by an older connector and may \
                 have no reference to the key. The register command of the connector saves it \
                 again."
            }
            Told::ConnectionUnchanged => {
                "The connection did not change: the same device and the same credential."
            }
            Told::NotesStayInMemory => {
                "If this Hermes saved the three Arauto notes in its memory, they are still there: \
                 remove them with the memory tool of Hermes. The connector never reads or changes \
                 the memory of Hermes."
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

    /// The rules of the catalog: ASCII, one line, a whole sentence, and nothing an LLM could
    /// take as a command to run.
    fn check_rules(phrases: &[&str]) {
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
        for phrase in phrases {
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
    }

    #[test]
    fn local_phrases_follow_the_rules_of_the_catalog() {
        let phrases: Vec<&str> = Local::ALL.iter().map(|local| local.phrase()).collect();
        check_rules(&phrases);
    }

    #[test]
    fn told_phrases_follow_the_rules_and_name_the_setting_of_hermes() {
        let told: Vec<&str> = Told::ALL.iter().map(|told| told.phrase()).collect();
        check_rules(&told);
        // Distinct across both lists.
        let mut all: Vec<&str> = Local::ALL.iter().map(|local| local.phrase()).collect();
        all.extend(&told);
        let distinct: std::collections::HashSet<&str> = all.iter().copied().collect();
        assert_eq!(distinct.len(), all.len());
        // The ones that tell the operator what to look for name Hermes's own setting.
        for phrase in [
            Told::WakingNeedsLocalApi,
            Told::KeyNotReceived,
            Told::KeyMade,
            Told::KeyNotMade,
        ]
        .map(Told::phrase)
        {
            assert!(phrase.contains("API_SERVER_KEY"), "{phrase}");
        }
        // The connector may set the key now (only in `join`): no phrase says it never does.
        for phrase in &told {
            assert!(!phrase.contains("never sets"), "{phrase}");
        }
    }

    /// The five phrases of the key step are the approved texts, character by character.
    #[test]
    fn the_phrases_of_the_key_step_are_the_approved_texts() {
        let approved: [(Told, &str); 5] = [
            (
                Told::WakingNeedsLocalApi,
                "This Hermes can be woken only while its local API is on, which Hermes does when its own setting API_SERVER_KEY holds a key. The status command says whether the connector received it.",
            ),
            (
                Told::KeyNotReceived,
                "No notice wakes this Hermes until its local API is on. Hermes turns it on when its own setting API_SERVER_KEY, kept in the configuration of Hermes, holds a key.",
            ),
            (
                Told::KeyMade,
                "This Hermes had no key for its local API, so the connector drew one and saved it in API_SERVER_KEY with the config command of Hermes. The connector shows it to nobody and writes it in no file of its own.",
            ),
            (
                Told::RestartAfterKey,
                "The local API of this Hermes turns on only after its gateway restarts. After the approval, the operator sends the restart command of Hermes to this Hermes in the chat, once.",
            ),
            (
                Told::KeyNotMade,
                "The connector did not confirm a new key for the local API of this Hermes. If the status command says the key is not received, the operator sets API_SERVER_KEY in the configuration of Hermes and restarts its gateway.",
            ),
        ];
        let sizes = [182, 160, 203, 173, 215];
        for ((told, text), size) in approved.into_iter().zip(sizes) {
            assert_eq!(told.phrase(), text);
            assert_eq!(text.len(), size, "{text}");
        }
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
