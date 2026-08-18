use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info, warn};

use crate::error::{KernelError, Result};
use crate::layout::Layout;
use crate::model::{
    Inventory, KernelState, PluginKind, PluginManifest, PluginState, PluginView, KERNEL_PLUGIN_ID,
};
use crate::process::ProcessManager;
use crate::protocol::{
    ControlCommand, ControlData, ControlRequest, ControlResponse, CONTROL_PROTOCOL_VERSION,
};
use crate::registry::{self, RegistrySnapshot};

const MAX_CONTROL_MESSAGE_BYTES: usize = 1024 * 1024;

pub struct Kernel {
    layout: Layout,
    processes: ProcessManager,
}

impl Kernel {
    pub fn new(root: impl Into<PathBuf>, runtime_root: impl Into<PathBuf>) -> Result<Self> {
        let layout = Layout::new(root, runtime_root);
        layout.init()?;
        Ok(Self {
            layout,
            processes: ProcessManager::default(),
        })
    }

    pub async fn start_autostart_plugins(&mut self) {
        let result = self.autostart_ids();
        let ids = match result {
            Ok(ids) => ids,
            Err(error) => {
                error!(%error, "cannot read autostart configuration");
                return;
            }
        };
        for id in ids {
            if let Err(error) = self.start_plugin(&id).await {
                error!(plugin = %id, %error, "autostart failed");
            }
        }
    }

    pub async fn execute(&mut self, command: ControlCommand) -> Result<ControlData> {
        match command {
            ControlCommand::List => Ok(ControlData::Inventory(self.inventory()?)),
            ControlCommand::Status { id } => Ok(ControlData::Plugin(self.plugin_view(&id)?)),
            ControlCommand::Install { source } => {
                let manifest = registry::install(&self.layout, Path::new(&source))?;
                self.processes.forget(&manifest.id);
                Ok(ControlData::Plugin(self.plugin_view(&manifest.id)?))
            }
            ControlCommand::Uninstall { id } => {
                self.ensure_mutable(&id)?;
                let snapshot = registry::scan(&self.layout)?;
                self.require_manifest(&snapshot, &id)?;
                self.processes.refresh()?;
                if self.processes.is_running(&id) {
                    return Err(KernelError::Conflict(format!(
                        "plugin {id} is running; stop it before uninstalling"
                    )));
                }
                let dependents = installed_dependents(&snapshot, &id);
                if !dependents.is_empty() {
                    return Err(KernelError::Dependency(format!(
                        "plugin {id} is required by: {}",
                        dependents.join(", ")
                    )));
                }
                registry::uninstall(&self.layout, &id)?;
                let mut state = self.layout.read_kernel_state()?;
                state.autostart.remove(&id);
                self.layout.write_kernel_state(&state)?;
                self.processes.forget(&id);
                Ok(ControlData::Inventory(self.inventory()?))
            }
            ControlCommand::Start { id } => {
                self.start_plugin(&id).await?;
                Ok(ControlData::Plugin(self.plugin_view(&id)?))
            }
            ControlCommand::Stop { id } => {
                self.ensure_mutable(&id)?;
                let snapshot = registry::scan(&self.layout)?;
                self.require_manifest(&snapshot, &id)?;
                self.processes.refresh()?;
                let running_dependents = installed_dependents(&snapshot, &id)
                    .into_iter()
                    .filter(|dependent| self.processes.is_running(dependent))
                    .collect::<Vec<_>>();
                if !running_dependents.is_empty() {
                    return Err(KernelError::Dependency(format!(
                        "plugin {id} has running dependents: {}",
                        running_dependents.join(", ")
                    )));
                }
                self.processes.stop(&id).await?;
                Ok(ControlData::Plugin(self.plugin_view(&id)?))
            }
            ControlCommand::SetAutostart { id, enabled } => {
                self.ensure_mutable(&id)?;
                let snapshot = registry::scan(&self.layout)?;
                self.require_manifest(&snapshot, &id)?;
                let mut state = self.layout.read_kernel_state()?;
                if enabled {
                    state.autostart.insert(id.clone());
                } else {
                    state.autostart.remove(&id);
                }
                self.layout.write_kernel_state(&state)?;
                Ok(ControlData::Plugin(self.plugin_view(&id)?))
            }
            ControlCommand::GetConfig { id } => {
                let snapshot = registry::scan(&self.layout)?;
                self.require_manifest(&snapshot, &id)?;
                Ok(ControlData::Config(self.layout.read_plugin_settings(&id)?))
            }
            ControlCommand::SetConfig { id, key, value } => {
                self.ensure_mutable(&id)?;
                validate_config_key(&key)?;
                let snapshot = registry::scan(&self.layout)?;
                self.require_manifest(&snapshot, &id)?;
                let mut settings = self.layout.read_plugin_settings(&id)?;
                settings.settings.insert(key, value);
                self.layout.write_plugin_settings(&id, &settings)?;
                Ok(ControlData::Config(settings))
            }
            ControlCommand::UnsetConfig { id, key } => {
                self.ensure_mutable(&id)?;
                validate_config_key(&key)?;
                let snapshot = registry::scan(&self.layout)?;
                self.require_manifest(&snapshot, &id)?;
                let mut settings = self.layout.read_plugin_settings(&id)?;
                settings.settings.remove(&key);
                self.layout.write_plugin_settings(&id, &settings)?;
                Ok(ControlData::Config(settings))
            }
        }
    }

    async fn start_plugin(&mut self, id: &str) -> Result<()> {
        self.ensure_mutable(id)?;
        let snapshot = registry::scan(&self.layout)?;
        let order = dependency_order(&snapshot, id)?;
        for plugin_id in order {
            if plugin_id == KERNEL_PLUGIN_ID {
                continue;
            }
            let manifest = self.require_manifest(&snapshot, &plugin_id)?;
            self.processes.start(&self.layout, manifest).await?;
        }
        Ok(())
    }

    fn inventory(&mut self) -> Result<Inventory> {
        let snapshot = registry::scan(&self.layout)?;
        let state = self.layout.read_kernel_state()?;
        self.processes.refresh()?;
        let plugins = snapshot
            .manifests
            .values()
            .map(|manifest| self.view(manifest, &state))
            .collect::<Result<Vec<_>>>()?;
        Ok(Inventory {
            plugins,
            invalid_directories: snapshot.invalid_directories,
        })
    }

    fn plugin_view(&mut self, id: &str) -> Result<PluginView> {
        let snapshot = registry::scan(&self.layout)?;
        let manifest = self.require_manifest(&snapshot, id)?.clone();
        let state = self.layout.read_kernel_state()?;
        self.processes.refresh()?;
        self.view(&manifest, &state)
    }

    fn view(&self, manifest: &PluginManifest, state: &KernelState) -> Result<PluginView> {
        let settings = self.layout.read_plugin_settings(&manifest.id)?;
        let last_exit = self.processes.last_exit(&manifest.id);
        let plugin_state = match manifest.kind {
            PluginKind::Builtin => PluginState::Builtin,
            PluginKind::Process if self.processes.is_running(&manifest.id) => PluginState::Running,
            PluginKind::Process if last_exit.is_some() => PluginState::Exited,
            PluginKind::Process => PluginState::Installed,
        };
        Ok(PluginView {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            kind: manifest.kind.clone(),
            state: plugin_state,
            autostart: state.autostart.contains(&manifest.id),
            path: self.layout.plugin_dir(&manifest.id).display().to_string(),
            last_exit,
            settings: settings.settings,
        })
    }

    fn require_manifest<'a>(
        &self,
        snapshot: &'a RegistrySnapshot,
        id: &str,
    ) -> Result<&'a PluginManifest> {
        snapshot
            .manifests
            .get(id)
            .ok_or_else(|| KernelError::NotFound(id.to_owned()))
    }

    fn ensure_mutable(&self, id: &str) -> Result<()> {
        if id == KERNEL_PLUGIN_ID {
            Err(KernelError::Protected(id.to_owned()))
        } else {
            Ok(())
        }
    }

    fn autostart_ids(&self) -> Result<Vec<String>> {
        Ok(self
            .layout
            .read_kernel_state()?
            .autostart
            .into_iter()
            .collect())
    }
}

pub async fn run_daemon(root: impl Into<PathBuf>, runtime_root: impl Into<PathBuf>) -> Result<()> {
    let mut kernel = Kernel::new(root, runtime_root)?;
    prepare_socket(&kernel.layout).await?;
    let listener = UnixListener::bind(&kernel.layout.socket_file)?;
    fs::set_permissions(
        &kernel.layout.socket_file,
        fs::Permissions::from_mode(0o600),
    )?;
    fs::write(&kernel.layout.pid_file, std::process::id().to_string())?;
    info!(socket = %kernel.layout.socket_file.display(), "EasOS Kernel daemon started");

    kernel.start_autostart_plugins().await;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => handle_connection(&mut kernel, stream).await,
                    Err(error) => warn!(%error, "control connection failed"),
                }
            }
            _ = terminate.recv() => break,
            _ = interrupt.recv() => break,
        }
    }

    info!("EasOS Kernel daemon is stopping");
    kernel.processes.stop_all().await;
    let _ = fs::remove_file(&kernel.layout.socket_file);
    let _ = fs::remove_file(&kernel.layout.pid_file);
    Ok(())
}

async fn prepare_socket(layout: &Layout) -> Result<()> {
    if !layout.socket_file.exists() {
        return Ok(());
    }
    if UnixStream::connect(&layout.socket_file).await.is_ok() {
        return Err(KernelError::Unavailable(format!(
            "another daemon is already listening on {}",
            layout.socket_file.display()
        )));
    }
    fs::remove_file(&layout.socket_file)?;
    Ok(())
}

async fn handle_connection(kernel: &mut Kernel, stream: UnixStream) {
    if let Err(error) = serve_connection(kernel, stream).await {
        warn!(%error, "control request failed before a response was sent");
    }
}

async fn serve_connection(kernel: &mut Kernel, stream: UnixStream) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let count = reader.read_line(&mut line).await?;
    if count == 0 || count > MAX_CONTROL_MESSAGE_BYTES {
        return Err(KernelError::InvalidData(
            "empty or oversized control request".to_owned(),
        ));
    }
    let response = match serde_json::from_str::<ControlRequest>(&line) {
        Ok(request) if request.protocol_version == CONTROL_PROTOCOL_VERSION => {
            match kernel.execute(request.command).await {
                Ok(data) => ControlResponse::success(data),
                Err(error) => ControlResponse::failure(&error),
            }
        }
        Ok(request) => ControlResponse::failure(&KernelError::InvalidData(format!(
            "control protocol version {} is unsupported",
            request.protocol_version
        ))),
        Err(error) => ControlResponse::failure(&KernelError::InvalidData(format!(
            "invalid control request: {error}"
        ))),
    };
    let mut stream = reader.into_inner();
    stream.write_all(&serde_json::to_vec(&response)?).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;
    Ok(())
}

fn dependency_order(snapshot: &RegistrySnapshot, root: &str) -> Result<Vec<String>> {
    fn visit(
        id: &str,
        snapshot: &RegistrySnapshot,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        output: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_owned()) {
            return Err(KernelError::Dependency(format!(
                "dependency cycle detected at plugin {id}"
            )));
        }
        let manifest = snapshot
            .manifests
            .get(id)
            .ok_or_else(|| KernelError::Dependency(format!("required plugin {id} is missing")))?;
        for dependency in &manifest.requires {
            visit(dependency, snapshot, visiting, visited, output)?;
        }
        visiting.remove(id);
        visited.insert(id.to_owned());
        output.push(id.to_owned());
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut output = Vec::new();
    visit(root, snapshot, &mut visiting, &mut visited, &mut output)?;
    Ok(output)
}

fn installed_dependents(snapshot: &RegistrySnapshot, id: &str) -> Vec<String> {
    snapshot
        .manifests
        .values()
        .filter(|manifest| manifest.requires.iter().any(|required| required == id))
        .map(|manifest| manifest.id.clone())
        .collect()
}

fn validate_config_key(key: &str) -> Result<()> {
    if !key.is_empty() && key.len() <= 128 && !key.chars().any(char::is_whitespace) {
        Ok(())
    } else {
        Err(KernelError::InvalidData(
            "config key must be 1-128 non-whitespace characters".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, requires: &[&str]) -> PluginManifest {
        let mut value = PluginManifest::kernel_builtin();
        value.id = id.to_owned();
        value.requires = requires.iter().map(|item| (*item).to_owned()).collect();
        value
    }

    #[test]
    fn dependencies_are_ordered_before_the_requesting_plugin() {
        let mut snapshot = RegistrySnapshot::default();
        snapshot
            .manifests
            .insert("base".to_owned(), manifest("base", &[]));
        snapshot
            .manifests
            .insert("app".to_owned(), manifest("app", &["base"]));
        assert_eq!(dependency_order(&snapshot, "app").unwrap(), ["base", "app"]);
    }

    #[test]
    fn dependency_cycles_are_rejected() {
        let mut snapshot = RegistrySnapshot::default();
        snapshot
            .manifests
            .insert("a".to_owned(), manifest("a", &["b"]));
        snapshot
            .manifests
            .insert("b".to_owned(), manifest("b", &["a"]));
        assert!(dependency_order(&snapshot, "a").is_err());
    }
}
