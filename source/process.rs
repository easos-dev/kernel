use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::error::{KernelError, Result};
use crate::layout::Layout;
use crate::model::PluginManifest;

struct ManagedProcess {
    child: Child,
    stop_timeout_ms: u64,
}

#[derive(Default)]
pub struct ProcessManager {
    running: BTreeMap<String, ManagedProcess>,
    last_exit: BTreeMap<String, i32>,
}

impl ProcessManager {
    pub fn refresh(&mut self) -> Result<()> {
        let mut exited = Vec::new();
        for (id, managed) in &mut self.running {
            if let Some(status) = managed.child.try_wait()? {
                exited.push((id.clone(), status.code().unwrap_or(-1)));
            }
        }
        for (id, code) in exited {
            self.running.remove(&id);
            self.last_exit.insert(id, code);
        }
        Ok(())
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.running.contains_key(id)
    }

    pub fn last_exit(&self, id: &str) -> Option<i32> {
        self.last_exit.get(id).copied()
    }

    pub fn forget(&mut self, id: &str) {
        self.last_exit.remove(id);
    }

    pub async fn start(&mut self, layout: &Layout, manifest: &PluginManifest) -> Result<()> {
        self.refresh()?;
        if self.is_running(&manifest.id) {
            return Ok(());
        }
        let runtime = manifest.runtime.as_ref().ok_or_else(|| {
            KernelError::InvalidData(format!("plugin {} has no process runtime", manifest.id))
        })?;
        let plugin_dir = layout.plugin_dir(&manifest.id);
        let executable = plugin_dir.join(&runtime.entrypoint);
        if !executable.is_file() {
            return Err(KernelError::InvalidData(format!(
                "plugin {} entrypoint does not exist: {}",
                manifest.id,
                executable.display()
            )));
        }

        let config_file = layout.plugin_config_file(&manifest.id);
        let log_file = layout.plugin_log(&manifest.id);
        if let Some(parent) = log_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;
        let stderr = stdout.try_clone()?;

        let mut command = Command::new(&executable);
        command
            .args(&runtime.args)
            .current_dir(&plugin_dir)
            .envs(&runtime.environment)
            .env("EASOS_PLUGIN_ID", &manifest.id)
            .env("EASOS_PLUGIN_HOME", &plugin_dir)
            .env("EASOS_PLUGIN_CONFIG_PATH", &config_file)
            .env("EASOS_HOME", &layout.root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);

        let child = command.spawn().map_err(|error| {
            KernelError::Io(std::io::Error::new(
                error.kind(),
                format!("failed to start plugin {}: {error}", manifest.id),
            ))
        })?;
        self.last_exit.remove(&manifest.id);
        self.running.insert(
            manifest.id.clone(),
            ManagedProcess {
                child,
                stop_timeout_ms: runtime.stop_timeout_ms,
            },
        );
        Ok(())
    }

    pub async fn stop(&mut self, id: &str) -> Result<()> {
        self.refresh()?;
        let Some(mut managed) = self.running.remove(id) else {
            return Ok(());
        };
        if let Some(pid) = managed.child.id() {
            let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(KernelError::Io(error));
                }
            }
        }

        let status = match tokio::time::timeout(
            Duration::from_millis(managed.stop_timeout_ms),
            managed.child.wait(),
        )
        .await
        {
            Ok(status) => status?,
            Err(_) => {
                managed.child.kill().await?;
                managed.child.wait().await?
            }
        };
        self.last_exit
            .insert(id.to_owned(), status.code().unwrap_or(-1));
        Ok(())
    }

    pub async fn stop_all(&mut self) {
        let ids = self.running.keys().cloned().collect::<Vec<_>>();
        for id in ids.into_iter().rev() {
            let _ = self.stop(&id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forget_clears_exit_state_for_reinstall() {
        let mut processes = ProcessManager::default();
        processes.last_exit.insert("clock".to_owned(), 0);
        processes.forget("clock");
        assert_eq!(processes.last_exit("clock"), None);
    }
}
