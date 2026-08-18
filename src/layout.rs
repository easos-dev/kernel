use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::Result;
use crate::model::{KernelConfig, PluginManifest, KERNEL_PLUGIN_ID};

#[derive(Debug, Clone)]
pub struct Layout {
    pub root: PathBuf,
    pub plugins_dir: PathBuf,
    pub config_dir: PathBuf,
    pub kernel_config_file: PathBuf,
    pub run_dir: PathBuf,
    pub socket_file: PathBuf,
    pub pid_file: PathBuf,
    pub runtime_plugins_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let plugins_dir = root.join("plugins");
        let config_dir = root.join("config");
        let run_dir = root.join("run");
        Self {
            kernel_config_file: config_dir.join("kernel.json"),
            socket_file: run_dir.join("kernel.sock"),
            pid_file: run_dir.join("kernel.pid"),
            runtime_plugins_dir: run_dir.join("plugins"),
            logs_dir: root.join("logs"),
            root,
            plugins_dir,
            config_dir,
            run_dir,
        }
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.plugins_dir)?;
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.run_dir)?;
        fs::create_dir_all(&self.runtime_plugins_dir)?;
        fs::create_dir_all(&self.logs_dir)?;

        if !self.kernel_config_file.exists() {
            self.write_kernel_config(&KernelConfig::default())?;
        } else {
            self.read_kernel_config()?.validate()?;
        }

        let kernel_manifest = self
            .plugins_dir
            .join(KERNEL_PLUGIN_ID)
            .join("Manifest")
            .join("main.json");
        // The built-in Kernel identity is protected. Refresh only its manifest;
        // binaries and other files in the same plugin directory remain untouched.
        write_json_atomic(&kernel_manifest, &PluginManifest::kernel_builtin())?;
        Ok(())
    }

    pub fn read_kernel_config(&self) -> Result<KernelConfig> {
        read_json(&self.kernel_config_file)
    }

    pub fn write_kernel_config(&self, config: &KernelConfig) -> Result<()> {
        config.validate()?;
        write_json_atomic(&self.kernel_config_file, config)
    }

    pub fn plugin_dir(&self, id: &str) -> PathBuf {
        self.plugins_dir.join(id)
    }

    pub fn plugin_runtime_config(&self, id: &str) -> PathBuf {
        self.runtime_plugins_dir.join(id).join("config.json")
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
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
