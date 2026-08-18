use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{KernelError, Result};
use crate::model::{KernelState, PluginManifest, PluginSettings, KERNEL_PLUGIN_ID};

#[derive(Debug, Clone)]
pub struct Layout {
    pub root: PathBuf,
    pub runtime_root: PathBuf,
    pub kernel_dir: PathBuf,
    pub kernel_state_file: PathBuf,
    pub socket_file: PathBuf,
    pub pid_file: PathBuf,
    pub logs_dir: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>, runtime_root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let runtime_root = runtime_root.into();
        let kernel_dir = root.join(KERNEL_PLUGIN_ID);
        Self {
            kernel_state_file: kernel_dir.join("config").join("state.json"),
            socket_file: runtime_root.join("kernel.sock"),
            pid_file: runtime_root.join("kernel.pid"),
            logs_dir: runtime_root.join("logs"),
            root,
            runtime_root,
            kernel_dir,
        }
    }

    pub fn init(&self) -> Result<()> {
        if !self.root.is_dir() {
            return Err(KernelError::InvalidData(format!(
                "EasOS workspace does not exist: {}",
                self.root.display()
            )));
        }
        fs::create_dir_all(&self.runtime_root)?;
        fs::create_dir_all(&self.logs_dir)?;

        let kernel_manifest_file = self.kernel_dir.join("manifest").join("main.json");
        let kernel_manifest: PluginManifest = read_json(&kernel_manifest_file)?;
        kernel_manifest.validate()?;
        if kernel_manifest.id != KERNEL_PLUGIN_ID {
            return Err(KernelError::InvalidData(format!(
                "kernel manifest id must be {KERNEL_PLUGIN_ID:?}"
            )));
        }
        if !self.kernel_dir.join("bin").is_dir() {
            return Err(KernelError::InvalidData(
                "kernel plugin is missing bin/".to_owned(),
            ));
        }
        self.read_plugin_settings(KERNEL_PLUGIN_ID)?.validate()?;

        if self.kernel_state_file.exists() {
            self.read_kernel_state()?.validate()?;
        } else {
            self.write_kernel_state(&KernelState::default())?;
        }
        Ok(())
    }

    pub fn read_kernel_state(&self) -> Result<KernelState> {
        read_json(&self.kernel_state_file)
    }

    pub fn write_kernel_state(&self, state: &KernelState) -> Result<()> {
        state.validate()?;
        write_json_atomic(&self.kernel_state_file, state)
    }

    pub fn plugin_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    pub fn plugin_config_file(&self, id: &str) -> PathBuf {
        self.plugin_dir(id).join("config").join("main.json")
    }

    pub fn read_plugin_settings(&self, id: &str) -> Result<PluginSettings> {
        let settings: PluginSettings = read_json(&self.plugin_config_file(id))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn write_plugin_settings(&self, id: &str, settings: &PluginSettings) -> Result<()> {
        settings.validate()?;
        write_json_atomic(&self.plugin_config_file(id), settings)
    }

    pub fn plugin_log(&self, id: &str) -> PathBuf {
        self.logs_dir.join(format!("{id}.log"))
    }
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
