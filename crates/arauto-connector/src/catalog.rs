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
/// The connector never restarts Hermes. It saves a key only in `join`, only when Hermes has none (KeyMade); these phrases say whose setting it is. It also appends `arauto` to `platform_toolsets.api_server` only in `join`, only when that list is explicit and lacks it (the `RunTools` phrases).
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
    /// The list of tools of woken runs includes arauto.
    RunToolsIncluded,
    /// No list: woken runs get the default tools.
    RunToolsDefault,
    /// `join` appended arauto to the list of tools of woken runs.
    RunToolsAdded,
    /// The new list may need a restart of the gateway.
    RunToolsRestart,
    /// The config command of Hermes refused the append.
    RunToolsNotAdded,
    /// The list of tools of woken runs lacks arauto.
    RunToolsMissing,
    /// The list of tools of woken runs is empty: cannot tell.
    RunToolsEmpty,
    /// The list of tools of woken runs could not be read.
    RunToolsUnreadable,
    /// The append was asked and not confirmed.
    RunToolsNotConfirmed,
    /// No record: the list was not checked.
    RunToolsNotChecked,
    /// Reminder after `join` appended arauto.
    RunToolsAddedByJoin,
    /// `join` replaced the program; the older one keeps running (decision 0053).
    ProgramReplaced,
    /// `join` replaced the program; no running connector answered.
    ProgramReplacedNoneRunning,
    /// `join` replaced the program and ended the older one, which cannot read the folders.
    ProgramReplacedEnded,
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
        Told::RunToolsIncluded,
        Told::RunToolsDefault,
        Told::RunToolsAdded,
        Told::RunToolsRestart,
        Told::RunToolsNotAdded,
        Told::RunToolsMissing,
        Told::RunToolsEmpty,
        Told::RunToolsUnreadable,
        Told::RunToolsNotConfirmed,
        Told::RunToolsNotChecked,
        Told::RunToolsAddedByJoin,
        Told::ProgramReplaced,
        Told::ProgramReplacedNoneRunning,
        Told::ProgramReplacedEnded,
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
                 approval, the operator restarts the gateway of this Hermes, once: with the \
                 restart command of Hermes in the chat or, where the chat refuses it, with the \
                 Restart Gateway button of the Hermes dashboard."
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
            Told::RunToolsIncluded => {
                "The list platform_toolsets.api_server in the configuration of Hermes, the tools of the runs a notice wakes, includes arauto."
            }
            Told::RunToolsDefault => {
                "The configuration of Hermes has no list platform_toolsets.api_server, so the runs a notice wakes get the default tools of Hermes."
            }
            Told::RunToolsAdded => {
                "The list platform_toolsets.api_server in the configuration of Hermes, the tools of the runs a notice wakes, did not include arauto, so the connector added arauto at its end with the config command of Hermes. The other tools stay."
            }
            Told::RunToolsRestart => {
                "Hermes may apply the new list only after its gateway restarts. After the approval, the operator restarts the gateway of this Hermes once; the same restart also serves a key the connector made."
            }
            Told::RunToolsNotAdded => {
                "The config command of Hermes did not add arauto to the list platform_toolsets.api_server, so the connector could not do it."
            }
            Told::RunToolsMissing => {
                "The list platform_toolsets.api_server in the configuration of Hermes, the tools of the runs a notice wakes, does not include arauto. The operator adds arauto to that list, keeping the other tools, and restarts the gateway."
            }
            Told::RunToolsEmpty => {
                "The list platform_toolsets.api_server in the configuration of Hermes is empty, and the connector cannot tell whether the runs a notice wakes have the Arauto tools. The connector changed nothing; the operator checks it."
            }
            Told::RunToolsUnreadable => {
                "The connector could not read the list platform_toolsets.api_server in the configuration of Hermes, so it cannot tell whether the runs a notice wakes have the Arauto tools. That list must be absent or include arauto."
            }
            Told::RunToolsNotConfirmed => {
                "The connector asked Hermes to add arauto to the list platform_toolsets.api_server and could not confirm the result. The operator checks that list in the configuration of Hermes: it keeps the other tools and includes arauto."
            }
            Told::RunToolsNotChecked => {
                "This connector has not checked the list platform_toolsets.api_server, which sets the tools of the runs a notice wakes. The status command of the connector reads it; that list must be absent or include arauto."
            }
            Told::RunToolsAddedByJoin => {
                "The join of this connector added arauto to the list platform_toolsets.api_server. Hermes may apply it only after its gateway restarts: if the gateway has not restarted since, the operator restarts it once."
            }
            Told::ProgramReplaced => {
                "This connector replaced the program of an older one. The older one keeps running, and with it the Arauto tools of this Hermes, until the gateway of this Hermes restarts; from that restart on, Hermes runs the new one. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard."
            }
            Told::ProgramReplacedNoneRunning => {
                "This connector replaced the program of an older one. No running connector of this Hermes answered, so the Arauto tools of this Hermes may be off until its gateway restarts, and the approval is confirmed at that restart. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard."
            }
            Told::ProgramReplacedEnded => {
                "This connector replaced the program of an older one that cannot read its files, and ended it: the Arauto tools of this Hermes are off until its gateway restarts, and the approval is confirmed at that restart. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard."
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
    fn check_rules(phrases: &[&str], ceiling: usize) {
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
            assert!(phrase.len() >= 30 && phrase.len() <= ceiling, "{phrase}");
            assert!(!phrase.contains("  "), "{phrase}");
            for piece in forbidden {
                assert!(!phrase.contains(piece), "{piece:?} in: {phrase}");
            }
        }
    }

    #[test]
    fn local_phrases_follow_the_rules_of_the_catalog() {
        let phrases: Vec<&str> = Local::ALL.iter().map(|local| local.phrase()).collect();
        check_rules(&phrases, 300);
    }

    #[test]
    fn told_phrases_follow_the_rules_and_name_the_setting_of_hermes() {
        let told: Vec<&str> = Told::ALL.iter().map(|told| told.phrase()).collect();
        // The three phrases of a replaced program are longer than 300 bytes (approved texts);
        // every other rule holds for them, and the ceiling for them is 440.
        let long = [
            Told::ProgramReplaced,
            Told::ProgramReplacedNoneRunning,
            Told::ProgramReplacedEnded,
        ];
        let (long, short): (Vec<&str>, Vec<&str>) = told
            .iter()
            .copied()
            .partition(|phrase| long.iter().any(|told| told.phrase() == *phrase));
        check_rules(&short, 300);
        check_rules(&long, 440);
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
                "The local API of this Hermes turns on only after its gateway restarts. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard.",
            ),
            (
                Told::KeyNotMade,
                "The connector did not confirm a new key for the local API of this Hermes. If the status command says the key is not received, the operator sets API_SERVER_KEY in the configuration of Hermes and restarts its gateway.",
            ),
        ];
        let sizes = [182, 160, 203, 281, 215];
        for ((told, text), size) in approved.into_iter().zip(sizes) {
            assert_eq!(told.phrase(), text);
            assert_eq!(text.len(), size, "{text}");
        }
    }

    /// The phrases of the tools of a woken run are the approved texts, character by character.
    #[test]
    fn the_phrases_of_the_woken_run_tools_are_the_approved_texts() {
        let approved: [(Told, &str); 11] = [
            (
                Told::RunToolsIncluded,
                "The list platform_toolsets.api_server in the configuration of Hermes, the tools of the runs a notice wakes, includes arauto.",
            ),
            (
                Told::RunToolsDefault,
                "The configuration of Hermes has no list platform_toolsets.api_server, so the runs a notice wakes get the default tools of Hermes.",
            ),
            (
                Told::RunToolsAdded,
                "The list platform_toolsets.api_server in the configuration of Hermes, the tools of the runs a notice wakes, did not include arauto, so the connector added arauto at its end with the config command of Hermes. The other tools stay.",
            ),
            (
                Told::RunToolsRestart,
                "Hermes may apply the new list only after its gateway restarts. After the approval, the operator restarts the gateway of this Hermes once; the same restart also serves a key the connector made.",
            ),
            (
                Told::RunToolsNotAdded,
                "The config command of Hermes did not add arauto to the list platform_toolsets.api_server, so the connector could not do it.",
            ),
            (
                Told::RunToolsMissing,
                "The list platform_toolsets.api_server in the configuration of Hermes, the tools of the runs a notice wakes, does not include arauto. The operator adds arauto to that list, keeping the other tools, and restarts the gateway.",
            ),
            (
                Told::RunToolsEmpty,
                "The list platform_toolsets.api_server in the configuration of Hermes is empty, and the connector cannot tell whether the runs a notice wakes have the Arauto tools. The connector changed nothing; the operator checks it.",
            ),
            (
                Told::RunToolsUnreadable,
                "The connector could not read the list platform_toolsets.api_server in the configuration of Hermes, so it cannot tell whether the runs a notice wakes have the Arauto tools. That list must be absent or include arauto.",
            ),
            (
                Told::RunToolsNotConfirmed,
                "The connector asked Hermes to add arauto to the list platform_toolsets.api_server and could not confirm the result. The operator checks that list in the configuration of Hermes: it keeps the other tools and includes arauto.",
            ),
            (
                Told::RunToolsNotChecked,
                "This connector has not checked the list platform_toolsets.api_server, which sets the tools of the runs a notice wakes. The status command of the connector reads it; that list must be absent or include arauto.",
            ),
            (
                Told::RunToolsAddedByJoin,
                "The join of this connector added arauto to the list platform_toolsets.api_server. Hermes may apply it only after its gateway restarts: if the gateway has not restarted since, the operator restarts it once.",
            ),
        ];
        let sizes = [124, 129, 229, 192, 123, 222, 218, 215, 223, 208, 205];
        for ((told, text), size) in approved.into_iter().zip(sizes) {
            assert_eq!(told.phrase(), text);
            assert_eq!(text.len(), size, "{text}");
            if told != Told::RunToolsRestart {
                assert!(text.contains("platform_toolsets.api_server"), "{text}");
            }
        }
    }

    /// The three phrases of a replaced program are the approved texts, character by character.
    #[test]
    fn the_phrases_of_a_replaced_program_are_the_approved_texts() {
        let approved: [(Told, &str); 3] = [
            (
                Told::ProgramReplaced,
                "This connector replaced the program of an older one. The older one keeps running, and with it the Arauto tools of this Hermes, until the gateway of this Hermes restarts; from that restart on, Hermes runs the new one. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard.",
            ),
            (
                Told::ProgramReplacedNoneRunning,
                "This connector replaced the program of an older one. No running connector of this Hermes answered, so the Arauto tools of this Hermes may be off until its gateway restarts, and the approval is confirmed at that restart. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard.",
            ),
            (
                Told::ProgramReplacedEnded,
                "This connector replaced the program of an older one that cannot read its files, and ended it: the Arauto tools of this Hermes are off until its gateway restarts, and the approval is confirmed at that restart. After the approval, the operator restarts the gateway of this Hermes, once: with the restart command of Hermes in the chat or, where the chat refuses it, with the Restart Gateway button of the Hermes dashboard.",
            ),
        ];
        let sizes = [427, 430, 419];
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
