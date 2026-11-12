//! `status`: the hub, the version, the situation of the device and whether the Hermes can be
//! woken (`docs/11-conector.md`, section 1). It reads the files and asks the leader through the
//! local socket; it never opens a connection to the hub, and shows no secret.

use std::fmt::Write as _;

use arauto_protocol::tools::ToolName;

use crate::ask;
use crate::home::Home;
use crate::settings::Settings;
use crate::status::{ChannelStatus, Status};
use crate::store::{self, Current, Situation};

/// What the files say about one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderReport {
    /// The name of the folder: drawn at random, not a secret.
    pub folder: String,
    /// What the connector knows of the device, or `None` when its files are damaged.
    pub situation: Option<Situation>,
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
    /// What the leader in execution says, when one answers.
    pub leader: Option<Status>,
}

/// Reads the files and asks the leader.
pub async fn report(home: &Home, settings: &Settings) -> Report {
    let files = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || (store::scan(&home), store::read_state(&home))).await
    };
    let (layout, state) = match files {
        Ok((layout, state)) => (layout.ok(), state.ok()),
        Err(_) => (None, None),
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
        leader,
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
        report.device = device.map(folder);
        report.request = request.map(folder);
    }
    report
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
    match &report.leader {
        Some(leader) => {
            let process = leader
                .leader_process
                .map_or_else(String::new, |process| format!(" (process {process})"));
            let _ = writeln!(text, "Leader: running{process}");
            if let Some(device) = &leader.device {
                let _ = writeln!(text, "Channel: {}", channel(&device.channel));
                let woken = match device.channel {
                    ChannelStatus::Session { can_be_woken, .. } => {
                        if can_be_woken {
                            "yes"
                        } else {
                            "no"
                        }
                    }
                    _ => "unknown",
                };
                let _ = writeln!(text, "Can be woken: {woken}");
            }
            if let Some(request) = &leader.request {
                let _ = writeln!(text, "Request channel: {}", channel(&request.channel));
            }
            let tools: Vec<&str> = leader.tools.iter().map(ToolName::as_str).collect();
            let _ = writeln!(text, "Tools: {}", tools.join(", "));
        }
        None => {
            let _ = writeln!(text, "Leader: not running");
            let tools: Vec<&str> = report.tools.iter().map(ToolName::as_str).collect();
            if !tools.is_empty() {
                let _ = writeln!(text, "Tools of the last session: {}", tools.join(", "));
            }
        }
    }
    text
}
