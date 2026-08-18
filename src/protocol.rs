use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::KernelError;
use crate::model::{Inventory, PluginConfig, PluginView};

pub const CONTROL_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    pub protocol_version: u32,
    #[serde(flatten)]
    pub command: ControlCommand,
}

impl ControlRequest {
    pub fn new(command: ControlCommand) -> Self {
        Self {
            protocol_version: CONTROL_PROTOCOL_VERSION,
            command,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlCommand {
    List,
    Status {
        id: String,
    },
    Install {
        source: String,
    },
    Uninstall {
        id: String,
    },
    Start {
        id: String,
    },
    Stop {
        id: String,
    },
    SetAutostart {
        id: String,
        enabled: bool,
    },
    GetConfig {
        id: String,
    },
    SetConfig {
        id: String,
        key: String,
        value: Value,
    },
    UnsetConfig {
        id: String,
        key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlResponse {
    pub protocol_version: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ControlData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

impl ControlResponse {
    pub fn success(data: ControlData) -> Self {
        Self {
            protocol_version: CONTROL_PROTOCOL_VERSION,
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(error: &KernelError) -> Self {
        Self {
            protocol_version: CONTROL_PROTOCOL_VERSION,
            ok: false,
            data: None,
            error: Some(ControlError {
                code: error.code().to_owned(),
                message: error.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ControlData {
    Inventory(Inventory),
    Plugin(PluginView),
    Config(PluginConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlError {
    pub code: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattened_request_round_trips() {
        let request = ControlRequest::new(ControlCommand::Start {
            id: "clock".to_owned(),
        });
        let encoded = serde_json::to_string(&request).unwrap();
        assert_eq!(
            encoded,
            r#"{"protocol_version":1,"command":"start","id":"clock"}"#
        );
        let decoded: ControlRequest = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(
            decoded.command,
            ControlCommand::Start { id } if id == "clock"
        ));
    }
}
