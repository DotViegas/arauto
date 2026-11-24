//! `status`: the hub, the version, the situation of the device and whether the Hermes can be
//! woken (`docs/11-conector.md`, section 1). It reads the files and asks the leader through the
//! local socket; it never opens a connection to the hub, and shows no secret. It changes nothing.
//! When the entry carries the reference to the key it runs one command of Hermes, a read of the
//! list of tools of the runs a notice wakes (`crate::toolsets`).
//!
//! **The key of the local API of Hermes** is what the leader holds, not what this process has:
//! the line comes from the answer of the leader and never from the environment of the process
//! that runs `status`, and it is shown whatever the state of the channel. With no leader
//! running the line says it is unknown.

use std::fmt::Write as _;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::tools::ToolName;

use crate::ask;
use crate::catalog::Told;
use crate::hermes::Hermes;
use crate::home::Home;
use crate::settings::Settings;
use crate::status::{ChannelStatus, Status};
use crate::store::{self, Current, Situation};
use crate::toolsets::RunTools;
use crate::wake::LastWake;

/// What the files say about one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderReport {
    /// The name of the folder: drawn at random, not a secret.
    pub folder: String,
    /// What the connector knows of the device, or `None` when its files are damaged.
    pub situation: Option<Situation>,
}

/// What `hub.json` says of the entry of the connector in Hermes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryReport {
    /// Whether `join` or `register` saved the entry.
    pub registered: bool,
    /// Whether it was saved with the reference to the key; `None` when it is not known (an
    /// older connector saved it).
    pub key_reference: Option<bool>,
}

/// What `status` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The address of the hub of the device in use, or of the request.
    pub hub: Option<String>,
    /// The device in use.
    pub device: Option<FolderReport>,
    /// The request followed beside it.
    pub request: Option<FolderReport>,
    /// The other folders.
    pub others: usize,
    /// Whether a file of the folder is damaged or of an unknown format.
    pub damaged: bool,
    /// The tools of the last session, from the files.
    pub tools: Vec<ToolName>,
    /// The entry in Hermes, from the record of the device in use, else of the request.
    pub entry: Option<EntryReport>,
    /// What the leader in execution says, when one answers.
    pub leader: Option<Status>,
    /// The list of tools of the runs a notice wakes, read now; only when the entry carries the
    /// reference to the key.
    pub run_tools: Option<RunTools>,
    /// What `join` recorded about that list.
    pub recorded: Option<RunTools>,
}

/// Reads the files and asks the leader.
pub async fn report(home: &Home, hermes: &Hermes, settings: &Settings) -> Report {
    let files = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || {
            (
                store::scan(&home),
                store::read_state(&home),
                store::read_run_tools(&home),
            )
        })
        .await
    };
    let (layout, state, recorded) = match files {
        Ok((layout, state, recorded)) => (layout.ok(), state.ok(), Some(recorded)),
        Err(_) => (None, None, None),
    };
    let leader = match ask::ask(home, ask::operation("status"), settings.local_answer_ms).await {
        Ok(answer) => answer.get("status").and_then(Status::from_json),
        Err(_) => None,
    };
    let folder = |folder: &store::Folder| FolderReport {
        folder: folder.id().to_string(),
        situation: folder.record().map(|record| record.situation),
    };
    let mut report = Report {
        hub: None,
        device: None,
        request: None,
        others: 0,
        damaged: state.is_none() && home.state_file().exists(),
        tools: state.map(|state| state.tools).unwrap_or_default(),
        entry: None,
        leader,
        run_tools: None,
        recorded,
    };
    if let Some(layout) = layout {
        report.damaged |= layout.current == Current::Damaged
            || layout.folders.iter().any(|folder| folder.record.is_err());
        let device = layout.current();
        let request = layout.request();
        report.hub = device
            .or(request)
            .and_then(|folder| folder.record())
            .map(|record| record.address.url());
        report.others = layout
            .folders
            .len()
            .saturating_sub(usize::from(device.is_some()) + usize::from(request.is_some()));
        report.entry = device
            .or(request)
            .and_then(|folder| folder.record())
            .map(|record| EntryReport {
                registered: record.registered,
                key_reference: record.key_reference,
            });
        report.device = device.map(folder);
        report.request = request.map(folder);
    }
    if report
        .entry
        .is_some_and(|entry| entry.registered && entry.key_reference == Some(true))
    {
        report.run_tools = crate::toolsets::read_now(hermes, settings).await;
    }
    report
}

/// The line about the tools of the runs a notice wakes, and the sentences that go with it.
fn run_tools_lines(text: &mut String, now: RunTools, recorded: Option<RunTools>) {
    let (line, told): (&str, Option<Told>) = match now {
        RunTools::Included => (
            "the list of the api_server platform includes arauto",
            (recorded == Some(RunTools::Added)).then_some(Told::RunToolsAddedByJoin),
        ),
        RunTools::Default => (
            "no list for the api_server platform (Hermes gives its default tools)",
            None,
        ),
        RunTools::Missing => (
            "the list of the api_server platform does not include arauto",
            Some(Told::RunToolsMissing),
        ),
        RunTools::Empty => (
            "the list of the api_server platform is empty (cannot tell)",
            Some(Told::RunToolsEmpty),
        ),
        RunTools::Unreadable => (
            "unknown (the list of the api_server platform could not be read)",
            Some(Told::RunToolsUnreadable),
        ),
        // The live read never yields these two.
        RunTools::Added | RunTools::NotChecked => return,
    };
    let _ = writeln!(text, "Tools of woken runs: {line}");
    if let Some(told) = told {
        let _ = writeln!(text, "{}", told.phrase());
    }
}

fn situation(situation: Option<Situation>) -> String {
    match situation {
        Some(Situation::Pending) => "waiting for the approval of the admin".to_owned(),
        Some(Situation::Active) => "active".to_owned(),
        Some(Situation::Ended(code)) => format!("ended by the hub ({code})"),
        None => "files damaged".to_owned(),
    }
}

fn channel(channel: &ChannelStatus) -> String {
    match channel {
        ChannelStatus::Connecting => "connecting".to_owned(),
        ChannelStatus::Waiting => "waiting for the approval of the admin".to_owned(),
        ChannelStatus::Session { hermes, .. } => format!("in session, as the Hermes {hermes}"),
        ChannelStatus::Down(Some(code)) => format!("down ({code}), trying again"),
        ChannelStatus::Down(None) => "down, trying again".to_owned(),
        ChannelStatus::Stopped(Some(code)) => format!("not used ({code})"),
        ChannelStatus::Stopped(None) => "not used (files damaged)".to_owned(),
    }
}

fn entry_line(entry: &EntryReport) -> &'static str {
    match (entry.registered, entry.key_reference) {
        (true, Some(true)) => "saved with a reference to API_SERVER_KEY",
        (true, Some(false)) => {
            "saved without a reference to the key (this version of Hermes refused it)"
        }
        (true, None) => "saved by an older connector, perhaps without a reference to the key",
        (false, _) => "not saved by this connector",
    }
}

fn key_line(received: Option<bool>) -> &'static str {
    match received {
        Some(true) => "received by the running connector",
        Some(false) => "not received by the running connector (no notice wakes this Hermes)",
        None => "unknown (the running connector is of an older version)",
    }
}

fn last_line(last: Option<LastWake>) -> &'static str {
    match last {
        Some(LastWake::Accepted) => "accepted by the local API of Hermes",
        Some(LastWake::Refused) => "refused by the local API of Hermes",
        Some(LastWake::NoAnswer) => "the local API of Hermes did not answer",
        None => "none since the connector started",
    }
}

/// What to tell when the running connector did not receive the key, by what the entry holds.
fn told_when_not_received(entry: Option<EntryReport>) -> Vec<Told> {
    match entry {
        Some(EntryReport {
            registered: true,
            key_reference: Some(true),
        }) => vec![Told::KeyNotReceived, Told::KeyAfterRestart],
        Some(EntryReport {
            registered: true,
            key_reference: Some(false),
        }) => vec![Told::ReferenceRefused],
        Some(EntryReport {
            registered: true,
            key_reference: None,
        }) => vec![Told::EntryOlder],
        Some(EntryReport {
            registered: false, ..
        })
        | None => vec![Told::KeyNotReceived],
    }
}

/// The text `status` prints.
#[must_use]
pub fn render(report: &Report) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "Arauto connector {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(text, "Hub: {}", report.hub.as_deref().unwrap_or("none"));
    match &report.device {
        Some(device) => {
            let _ = writeln!(text, "Device in use: {}", situation(device.situation));
        }
        None => {
            let _ = writeln!(text, "Device in use: none");
        }
    }
    if let Some(request) = &report.request {
        let _ = writeln!(text, "Connection request: {}", situation(request.situation));
    }
    if report.others > 0 {
        let _ = writeln!(
            text,
            "Older identities waiting for the hub to confirm their closing: {}",
            report.others
        );
    }
    if report.damaged {
        let _ = writeln!(
            text,
            "A file of the connector is damaged or has an unknown format."
        );
    }
    if let Some(entry) = &report.entry {
        let _ = writeln!(text, "Entry in Hermes: {}", entry_line(entry));
    }
    if let Some(now) = report.run_tools {
        run_tools_lines(&mut text, now, report.recorded);
    }
    match &report.leader {
        Some(leader) => {
            let process = leader
                .leader_process
                .map_or_else(String::new, |process| format!(" (process {process})"));
            let _ = writeln!(text, "Leader: running{process}");
            if let Some(device) = &leader.device {
                let _ = writeln!(text, "Channel: {}", channel(&device.channel));
                // What the admin allowed for WhatsApp; whether this connector has the means is
                // the line below, which does not depend on the channel.
                let allowed = match device.channel {
                    ChannelStatus::Session { can_be_woken, .. } => {
                        if can_be_woken {
                            "yes"
                        } else {
                            "no"
                        }
                    }
                    _ => "unknown",
                };
                let _ = writeln!(text, "Waking by WhatsApp allowed by the admin: {allowed}");
            }
            let _ = writeln!(
                text,
                "Key of the local API of Hermes: {}",
                key_line(leader.waking.map(|waking| waking.key_received))
            );
            if let Some(waking) = &leader.waking {
                let _ = writeln!(text, "Last wake: {}", last_line(waking.last));
                if waking.wakes_without_session > 0 {
                    let _ = writeln!(
                        text,
                        "WhatsApp wakes without a session key from the hub: {}",
                        waking.wakes_without_session
                    );
                }
                if matches!(waking.last, Some(LastWake::Refused | LastWake::NoAnswer)) {
                    let _ = writeln!(text, "{}", ErrorCode::Ar602.phrase(None));
                }
                if !waking.key_received {
                    for told in told_when_not_received(report.entry) {
                        let _ = writeln!(text, "{}", told.phrase());
                    }
                }
            }
            if let Some(request) = &leader.request {
                let _ = writeln!(text, "Request channel: {}", channel(&request.channel));
            }
            let tools: Vec<&str> = leader.tools.iter().map(ToolName::as_str).collect();
            let _ = writeln!(text, "Tools: {}", tools.join(", "));
        }
        None => {
            let _ = writeln!(text, "Leader: not running");
            let _ = writeln!(
                text,
                "Key of the local API of Hermes: unknown (no connector started by Hermes is running)"
            );
            let tools: Vec<&str> = report.tools.iter().map(ToolName::as_str).collect();
            if !tools.is_empty() {
                let _ = writeln!(text, "Tools of the last session: {}", tools.join(", "));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use crate::status::{DeviceStatus, Role};
    use crate::wake::Waking;

    use super::*;

    fn leader(waking: Option<Waking>, device: Option<ChannelStatus>) -> Status {
        Status {
            role: Role::Leader,
            leader_process: Some(7),
            version: "0.1.0".to_owned(),
            device: device.map(|channel| DeviceStatus {
                folder: crate::store::DeviceId::generate().unwrap().to_string(),
                channel,
            }),
            request: None,
            tools: Vec::new(),
            leftovers: 0,
            waking,
        }
    }

    fn report(entry: Option<EntryReport>, leader: Option<Status>) -> Report {
        Report {
            hub: Some("http://192.0.2.1:1".to_owned()),
            device: None,
            request: None,
            others: 0,
            damaged: false,
            tools: Vec::new(),
            entry,
            leader,
            run_tools: None,
            recorded: None,
        }
    }

    fn entry(registered: bool, key_reference: Option<bool>) -> Option<EntryReport> {
        Some(EntryReport {
            registered,
            key_reference,
        })
    }

    #[test]
    fn render_says_what_the_leader_holds() {
        let received = Waking {
            key_received: true,
            last: None,
            wakes_without_session: 0,
        };
        let missing = Waking {
            key_received: false,
            last: None,
            wakes_without_session: 0,
        };
        // Received: the key line, the last wake, and no sentence about missing keys.
        let text = render(&report(
            entry(true, Some(true)),
            Some(leader(Some(received), None)),
        ));
        assert!(text.contains("Entry in Hermes: saved with a reference to API_SERVER_KEY\n"));
        assert!(
            text.contains("Key of the local API of Hermes: received by the running connector\n")
        );
        assert!(text.contains("Last wake: none since the connector started\n"));
        for told in Told::ALL {
            assert!(!text.contains(told.phrase()), "{}", told.phrase());
        }
        // Not received: the words depend on what the entry holds.
        let table: [(Option<EntryReport>, &str, &[Told]); 5] = [
            (
                entry(true, Some(true)),
                "saved with a reference to API_SERVER_KEY",
                &[Told::KeyNotReceived, Told::KeyAfterRestart],
            ),
            (
                entry(true, Some(false)),
                "saved without a reference to the key (this version of Hermes refused it)",
                &[Told::ReferenceRefused],
            ),
            (
                entry(true, None),
                "saved by an older connector, perhaps without a reference to the key",
                &[Told::EntryOlder],
            ),
            (
                entry(false, None),
                "not saved by this connector",
                &[Told::KeyNotReceived],
            ),
            (None, "", &[Told::KeyNotReceived]),
        ];
        for (entry, line, told) in table {
            let text = render(&report(entry, Some(leader(Some(missing), None))));
            assert!(
                text.contains(
                    "Key of the local API of Hermes: not received by the running connector (no notice wakes this Hermes)\n"
                ),
                "{text}"
            );
            if entry.is_some() {
                assert!(
                    text.contains(&format!("Entry in Hermes: {line}\n")),
                    "{text}"
                );
            } else {
                assert!(!text.contains("Entry in Hermes"));
            }
            for phrase in Told::ALL {
                assert_eq!(
                    text.contains(phrase.phrase()),
                    told.contains(phrase),
                    "{phrase:?} in {text}"
                );
            }
            assert!(!text.contains("not configured"));
        }
        // An older leader does not say: unknown, and no claim about the entry.
        let text = render(&report(entry(true, Some(true)), Some(leader(None, None))));
        assert!(text.contains(
            "Key of the local API of Hermes: unknown (the running connector is of an older version)\n"
        ));
        assert!(!text.contains("Last wake"));
        // No leader: unknown, and it says why.
        let text = render(&report(entry(true, Some(true)), None));
        assert!(text.contains(
            "Leader: not running\nKey of the local API of Hermes: unknown (no connector started by Hermes is running)\n"
        ));
    }

    #[test]
    fn the_last_wake_is_shown_with_the_phrase_of_the_code() {
        for (last, line) in [
            (LastWake::Accepted, "accepted by the local API of Hermes"),
            (LastWake::Refused, "refused by the local API of Hermes"),
            (LastWake::NoAnswer, "the local API of Hermes did not answer"),
        ] {
            let waking = Waking {
                key_received: true,
                last: Some(last),
                wakes_without_session: 0,
            };
            let text = render(&report(None, Some(leader(Some(waking), None))));
            assert!(text.contains(&format!("Last wake: {line}\n")), "{text}");
            assert_eq!(
                text.contains(&*ErrorCode::Ar602.phrase(None)),
                last != LastWake::Accepted
            );
        }
    }

    /// The key line does not wait for a session: it is the same while the channel waits.
    #[test]
    fn the_key_line_does_not_depend_on_the_channel() {
        let waking = Waking {
            key_received: true,
            last: None,
            wakes_without_session: 0,
        };
        for channel in [
            None,
            Some(ChannelStatus::Waiting),
            Some(ChannelStatus::Connecting),
            Some(ChannelStatus::Down(None)),
        ] {
            let text = render(&report(None, Some(leader(Some(waking), channel))));
            assert!(
                text.contains(
                    "Key of the local API of Hermes: received by the running connector\n"
                ),
                "{text}"
            );
        }
    }

    /// T2: each live state has its line and its sentence; the reminder of `join` only when the
    /// list includes `arauto` and `join` was the one that added it.
    #[test]
    fn the_tools_of_woken_runs_are_said_for_each_state() {
        let table: [(RunTools, &str, &[Told]); 5] = [
            (
                RunTools::Included,
                "Tools of woken runs: the list of the api_server platform includes arauto\n",
                &[],
            ),
            (
                RunTools::Default,
                "Tools of woken runs: no list for the api_server platform (Hermes gives its default tools)\n",
                &[],
            ),
            (
                RunTools::Missing,
                "Tools of woken runs: the list of the api_server platform does not include arauto\n",
                &[Told::RunToolsMissing],
            ),
            (
                RunTools::Empty,
                "Tools of woken runs: the list of the api_server platform is empty (cannot tell)\n",
                &[Told::RunToolsEmpty],
            ),
            (
                RunTools::Unreadable,
                "Tools of woken runs: unknown (the list of the api_server platform could not be read)\n",
                &[Told::RunToolsUnreadable],
            ),
        ];
        let run_tools_told = [
            Told::RunToolsIncluded,
            Told::RunToolsDefault,
            Told::RunToolsAddedByJoin,
            Told::RunToolsMissing,
            Told::RunToolsEmpty,
            Told::RunToolsUnreadable,
        ];
        for (state, line, told) in table {
            let mut shown = report(entry(true, Some(true)), None);
            shown.run_tools = Some(state);
            let text = render(&shown);
            let entry_at = text.find("Entry in Hermes").unwrap();
            let line_at = text.find(line).unwrap_or_else(|| panic!("{text}"));
            assert!(line_at > entry_at, "{text}");
            for phrase in run_tools_told {
                assert_eq!(
                    text.contains(phrase.phrase()),
                    told.contains(&phrase),
                    "{phrase:?} {text}"
                );
            }
        }
        let mut shown = report(entry(true, Some(true)), None);
        shown.run_tools = Some(RunTools::Included);
        shown.recorded = Some(RunTools::Added);
        assert!(render(&shown).contains(Told::RunToolsAddedByJoin.phrase()));
        shown.recorded = Some(RunTools::Included);
        assert!(!render(&shown).contains(Told::RunToolsAddedByJoin.phrase()));
        assert!(!render(&report(entry(true, Some(true)), None)).contains("Tools of woken runs"));
    }
}
