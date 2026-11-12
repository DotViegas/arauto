//! The situation of the connector, as `status` and the local socket show it: never a secret.

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::names::HermesName;
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value, json};

use crate::catalog::Said;

/// What this copy of the connector is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// It holds the lock of the folder: it keeps the channel and the local socket.
    Leader,
    /// It sends its calls to the leader through the local socket.
    Relay,
    /// The role is being decided: at the start, or while a copy takes the place of a leader
    /// that ended.
    Starting,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::Leader => "leader",
            Role::Relay => "relay",
            Role::Starting => "starting",
        }
    }
}

/// The channel of one device, as the leader sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelStatus {
    /// An attempt is under way.
    Connecting,
    /// The device is pending: the connection waits for the admin.
    Waiting,
    /// The session is open.
    Session {
        /// The name of this Hermes.
        hermes: HermesName,
        /// Whether the admin allowed waking it.
        can_be_woken: bool,
    },
    /// The channel is down and the connector tries again; the code says why, when known.
    Down(Option<ErrorCode>),
    /// The channel is no longer used for this device: the code says why; without a code, a
    /// file of the device is damaged.
    Stopped(Option<ErrorCode>),
}

impl ChannelStatus {
    pub(crate) fn down(said: Option<Said>) -> Self {
        ChannelStatus::Down(said.and_then(code_of))
    }

    pub(crate) fn stopped(said: Said) -> Self {
        ChannelStatus::Stopped(code_of(said))
    }
}

/// The code a [`Said`] carries, if any.
pub(crate) fn code_of(said: Said) -> Option<ErrorCode> {
    match said {
        Said::Code(code, _) => Some(code),
        Said::Unreachable(_) => Some(ErrorCode::Ar201),
        Said::Local(_) => None,
    }
}

/// A device folder and its channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceStatus {
    /// The name of the folder: drawn at random, not a secret.
    pub folder: String,
    /// Its channel.
    pub channel: ChannelStatus,
}

/// What `status` shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// What the copy that answered is.
    pub role: Role,
    /// The process of the leader, when one answered.
    pub leader_process: Option<u32>,
    /// The version of the program.
    pub version: String,
    /// The device in use.
    pub device: Option<DeviceStatus>,
    /// The request followed beside it.
    pub request: Option<DeviceStatus>,
    /// The released tools.
    pub tools: Vec<ToolName>,
    /// Other folders, waiting for the hub to confirm their closing.
    pub leftovers: usize,
}

fn channel_json(channel: &ChannelStatus) -> Value {
    match channel {
        ChannelStatus::Connecting => json!({"channel": "connecting"}),
        ChannelStatus::Waiting => json!({"channel": "waiting"}),
        ChannelStatus::Session {
            hermes,
            can_be_woken,
        } => json!({"channel": "session", "hermes": hermes.as_str(), "can_be_woken": can_be_woken}),
        ChannelStatus::Down(code) => {
            json!({"channel": "down", "code": code.map(ErrorCode::as_str)})
        }
        ChannelStatus::Stopped(code) => {
            json!({"channel": "stopped", "code": code.map(ErrorCode::as_str)})
        }
    }
}

fn channel_from(object: &Map<String, Value>) -> Option<ChannelStatus> {
    let code = || {
        object
            .get("code")
            .and_then(Value::as_str)
            .and_then(ErrorCode::parse)
    };
    Some(match object.get("channel")?.as_str()? {
        "connecting" => ChannelStatus::Connecting,
        "waiting" => ChannelStatus::Waiting,
        "session" => ChannelStatus::Session {
            hermes: HermesName::parse(object.get("hermes")?.as_str()?).ok()?,
            can_be_woken: object.get("can_be_woken")?.as_bool()?,
        },
        "down" => ChannelStatus::Down(code()),
        "stopped" => ChannelStatus::Stopped(code()),
        _ => return None,
    })
}

fn device_json(device: Option<&DeviceStatus>) -> Value {
    match device {
        None => Value::Null,
        Some(device) => {
            let mut object = match channel_json(&device.channel) {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            object.insert("folder".to_owned(), Value::from(device.folder.as_str()));
            Value::Object(object)
        }
    }
}

fn device_from(value: Option<&Value>) -> Option<Option<DeviceStatus>> {
    match value {
        None | Some(Value::Null) => Some(None),
        Some(Value::Object(object)) => {
            let folder = crate::store::DeviceId::parse(object.get("folder")?.as_str()?)?;
            Some(Some(DeviceStatus {
                folder: folder.to_string(),
                channel: channel_from(object)?,
            }))
        }
        Some(_) => None,
    }
}

impl Status {
    /// As the local socket carries it.
    pub(crate) fn to_json(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| Value::from(tool.as_str()))
            .collect();
        json!({
            "role": self.role.name(),
            "leader_process": self.leader_process,
            "version": self.version,
            "device": device_json(self.device.as_ref()),
            "request": device_json(self.request.as_ref()),
            "tools": tools,
            "leftovers": self.leftovers,
        })
    }

    /// Read from the local socket. The leader may be of another version: what is out of
    /// form is not shown.
    pub(crate) fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let role = match object.get("role")?.as_str()? {
            "leader" => Role::Leader,
            "relay" => Role::Relay,
            "starting" => Role::Starting,
            _ => return None,
        };
        let version = object.get("version")?.as_str()?;
        let version = arauto_protocol::message::DeclaredText::lossy(version)
            .as_str()
            .to_owned();
        Some(Self {
            role,
            leader_process: object
                .get("leader_process")
                .and_then(Value::as_u64)
                .and_then(|process| u32::try_from(process).ok()),
            version,
            device: device_from(object.get("device"))?,
            request: device_from(object.get("request"))?,
            tools: crate::local::read_tools(object)?,
            leftovers: object
                .get("leftovers")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_goes_through_the_socket_and_back() {
        let folder = crate::store::DeviceId::generate().unwrap().to_string();
        let status = Status {
            role: Role::Leader,
            leader_process: Some(42),
            version: "0.1.0".to_owned(),
            device: Some(DeviceStatus {
                folder: folder.clone(),
                channel: ChannelStatus::Session {
                    hermes: HermesName::parse("hermes-1").unwrap(),
                    can_be_woken: false,
                },
            }),
            request: Some(DeviceStatus {
                folder,
                channel: ChannelStatus::Stopped(Some(ErrorCode::Ar301)),
            }),
            tools: vec![ToolName::parse("arauto_status").unwrap()],
            leftovers: 2,
        };
        assert_eq!(Status::from_json(&status.to_json()), Some(status));
        assert_eq!(Status::from_json(&json!({"role": "x"})), None);
    }
}
