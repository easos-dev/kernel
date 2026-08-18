use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{KernelError, Result};

pub const DATA_SCHEMA_VERSION: u32 = 1;
pub const KERNEL_PLUGIN_ID: &str = "kernel";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    Builtin,
    Process,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: Version,
    pub kind: PluginKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeSpec>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSpec {
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default = "default_stop_timeout_ms")]
    pub stop_timeout_ms: u64,
}

fn default_stop_timeout_ms() -> u64 {
    5_000
}

impl PluginManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DATA_SCHEMA_VERSION {
            return Err(KernelError::InvalidData(format!(
                "plugin {} uses schema_version {}, expected {}",
                self.id, self.schema_version, DATA_SCHEMA_VERSION
            )));
        }
        validate_identifier(&self.id, "plugin id")?;
        if self.name.trim().is_empty() {
            return Err(KernelError::InvalidData(format!(
                "plugin {} has an empty name",
                self.id
            )));
        }
        for service in &self.provides {
            validate_symbol(service, "provided service")?;
        }
        for dependency in &self.requires {
            validate_identifier(dependency, "plugin dependency")?;
            if dependency == &self.id {
                return Err(KernelError::InvalidData(format!(
                    "plugin {} depends on itself",
                    self.id
                )));
            }
        }

        match (&self.kind, &self.runtime) {
            (PluginKind::Builtin, None) if self.id == KERNEL_PLUGIN_ID => {}
            (PluginKind::Builtin, None) => {
                return Err(KernelError::InvalidData(format!(
                    "plugin {} cannot use reserved kind builtin",
                    self.id
                )));
            }
            (PluginKind::Process, Some(runtime)) => runtime.validate()?,
            (PluginKind::Builtin, Some(_)) => {
                return Err(KernelError::InvalidData(format!(
                    "builtin plugin {} must not define runtime",
                    self.id
                )));
            }
            (PluginKind::Process, None) => {
                return Err(KernelError::InvalidData(format!(
                    "process plugin {} must define runtime",
                    self.id
                )));
            }
        }
        Ok(())
    }

    pub fn kernel_builtin() -> Self {
        Self {
            schema_version: DATA_SCHEMA_VERSION,
            id: KERNEL_PLUGIN_ID.to_owned(),
            name: "EasOS Kernel".to_owned(),
            version: Version::new(0, 1, 0),
            kind: PluginKind::Builtin,
            runtime: None,
            provides: vec!["kernel.lifecycle.v1".to_owned()],
            requires: Vec::new(),
        }
    }
}

impl RuntimeSpec {
    fn validate(&self) -> Result<()> {
        let path = Path::new(&self.entrypoint);
        if self.entrypoint.trim().is_empty() || path.is_absolute() {
            return Err(KernelError::InvalidData(
                "runtime.entrypoint must be a non-empty relative path".to_owned(),
            ));
        }
        if path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(KernelError::InvalidData(
                "runtime.entrypoint must stay inside the plugin directory".to_owned(),
            ));
        }
        if !matches!(
            path.components().next(),
            Some(Component::Normal(directory)) if directory == "bin"
        ) {
            return Err(KernelError::InvalidData(
                "runtime.entrypoint must be located under bin/".to_owned(),
            ));
        }
        if self.stop_timeout_ms == 0 || self.stop_timeout_ms > 60_000 {
            return Err(KernelError::InvalidData(
                "runtime.stop_timeout_ms must be between 1 and 60000".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub settings: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelState {
    pub schema_version: u32,
    #[serde(default)]
    pub autostart: BTreeSet<String>,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            schema_version: DATA_SCHEMA_VERSION,
            settings: BTreeMap::new(),
        }
    }
}

impl PluginSettings {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DATA_SCHEMA_VERSION {
            return Err(KernelError::InvalidData(format!(
                "plugin config uses schema_version {}, expected {}",
                self.schema_version, DATA_SCHEMA_VERSION
            )));
        }
        Ok(())
    }
}

impl Default for KernelState {
    fn default() -> Self {
        Self {
            schema_version: DATA_SCHEMA_VERSION,
            autostart: BTreeSet::new(),
        }
    }
}

impl KernelState {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DATA_SCHEMA_VERSION {
            return Err(KernelError::InvalidData(format!(
                "kernel state uses schema_version {}, expected {}",
                self.schema_version, DATA_SCHEMA_VERSION
            )));
        }
        for id in &self.autostart {
            validate_identifier(id, "autostart plugin id")?;
            if id == KERNEL_PLUGIN_ID {
                return Err(KernelError::InvalidData(
                    "kernel must not appear in the autostart list".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    Builtin,
    Installed,
    Running,
    Exited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub kind: PluginKind,
    pub state: PluginState,
    pub autostart: bool,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_exit: Option<i32>,
    pub settings: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidPlugin {
    pub directory: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub plugins: Vec<PluginView>,
    pub invalid_directories: Vec<InvalidPlugin>,
}

pub fn validate_identifier(value: &str, label: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric());
    if valid && !value.contains("..") {
        Ok(())
    } else {
        Err(KernelError::InvalidData(format!(
            "{label} {value:?} must be 1-64 ASCII letters, digits, '.', '-' or '_' and must not contain '..'"
        )))
    }
}

fn validate_symbol(value: &str, label: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'));
    if valid {
        Ok(())
    } else {
        Err(KernelError::InvalidData(format!(
            "{label} {value:?} contains unsupported characters"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_plugins_require_runtime() {
        let mut manifest = PluginManifest::kernel_builtin();
        manifest.id = "demo".to_owned();
        manifest.kind = PluginKind::Process;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn entrypoint_cannot_escape_plugin_directory() {
        let runtime = RuntimeSpec {
            entrypoint: "../service".to_owned(),
            args: Vec::new(),
            environment: BTreeMap::new(),
            stop_timeout_ms: 1_000,
        };
        assert!(runtime.validate().is_err());
    }

    #[test]
    fn entrypoint_must_be_inside_bin_directory() {
        let runtime = RuntimeSpec {
            entrypoint: "source/service".to_owned(),
            args: Vec::new(),
            environment: BTreeMap::new(),
            stop_timeout_ms: 1_000,
        };
        assert!(runtime.validate().is_err());
    }

    #[test]
    fn manifest_rejects_unknown_fields() {
        let json = r#"{
            "schema_version": 1,
            "id": "demo",
            "name": "Demo",
            "version": "0.1.0",
            "kind": "process",
            "runtime": {"entrypoint": "bin/demo", "unexpected": true},
            "unexpected": true
        }"#;
        assert!(serde_json::from_str::<PluginManifest>(json).is_err());
    }
}
