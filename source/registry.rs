use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{KernelError, Result};
use crate::layout::{read_json, Layout};
use crate::model::{InvalidPlugin, PluginManifest, PluginSettings, KERNEL_PLUGIN_ID};

const MANIFEST_PATH: [&str; 2] = ["manifest", "main.json"];
const CONFIG_PATH: [&str; 2] = ["config", "main.json"];

#[derive(Debug, Default)]
pub struct RegistrySnapshot {
    pub manifests: BTreeMap<String, PluginManifest>,
    pub invalid_directories: Vec<InvalidPlugin>,
}

pub fn scan(layout: &Layout) -> Result<RegistrySnapshot> {
    let mut entries = fs::read_dir(&layout.root)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut snapshot = RegistrySnapshot::default();
    for entry in entries {
        let directory = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type()?.is_dir() {
            snapshot.invalid_directories.push(InvalidPlugin {
                directory,
                error: "workspace entries must be plugin directories".to_owned(),
            });
            continue;
        }

        match load_plugin(&entry.path()) {
            Ok(manifest) if manifest.id == directory => {
                snapshot.manifests.insert(manifest.id.clone(), manifest);
            }
            Ok(manifest) => snapshot.invalid_directories.push(InvalidPlugin {
                directory: directory.clone(),
                error: format!(
                    "manifest id {:?} must match directory name {:?}",
                    manifest.id, directory
                ),
            }),
            Err(error) => snapshot.invalid_directories.push(InvalidPlugin {
                directory,
                error: error.to_string(),
            }),
        }
    }
    Ok(snapshot)
}

pub fn load_plugin(plugin_dir: &Path) -> Result<PluginManifest> {
    let manifest: PluginManifest = read_json(&manifest_path(plugin_dir))?;
    manifest.validate()?;

    if !plugin_dir.join("bin").is_dir() {
        return Err(KernelError::InvalidData(format!(
            "plugin {} is missing bin/",
            manifest.id
        )));
    }
    if !plugin_dir.join("config").is_dir() {
        return Err(KernelError::InvalidData(format!(
            "plugin {} is missing config/",
            manifest.id
        )));
    }
    let settings: PluginSettings = read_json(&config_path(plugin_dir))?;
    settings.validate()?;
    Ok(manifest)
}

pub fn install(layout: &Layout, source: &Path) -> Result<PluginManifest> {
    if !source.is_dir() {
        return Err(KernelError::InvalidData(format!(
            "plugin source is not a directory: {}",
            source.display()
        )));
    }
    let manifest = load_plugin(source)?;
    if manifest.id == KERNEL_PLUGIN_ID {
        return Err(KernelError::Protected(KERNEL_PLUGIN_ID.to_owned()));
    }
    let destination = layout.plugin_dir(&manifest.id);
    if destination.exists() {
        return Err(KernelError::AlreadyExists(manifest.id));
    }

    if let Err(error) = copy_directory(source, &destination) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    if let Err(error) = load_plugin(&destination) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    Ok(manifest)
}

pub fn uninstall(layout: &Layout, id: &str) -> Result<()> {
    if id == KERNEL_PLUGIN_ID {
        return Err(KernelError::Protected(id.to_owned()));
    }
    let destination = layout.plugin_dir(id);
    if !destination.exists() {
        return Err(KernelError::NotFound(id.to_owned()));
    }
    fs::remove_dir_all(destination)?;
    Ok(())
}

fn manifest_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(MANIFEST_PATH[0]).join(MANIFEST_PATH[1])
}

fn config_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(CONFIG_PATH[0]).join(CONFIG_PATH[1])
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(KernelError::InvalidData(format!(
                "plugin packages must not contain symbolic links: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(KernelError::InvalidData(format!(
                "plugin packages may only contain files and directories: {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}
