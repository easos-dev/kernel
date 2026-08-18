use std::path::PathBuf;

use clap::{Parser, Subcommand};
use easos_kernel::{
    ControlCommand, ControlRequest, ControlResponse, KernelError, Layout, Result,
    CONTROL_PROTOCOL_VERSION,
};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, Parser)]
#[command(name = "easos", version, about = "Manage EasOS plugins")]
struct Args {
    #[arg(
        long,
        env = "EASOS_HOME",
        default_value = "/var/lib/easos",
        global = true
    )]
    root: PathBuf,
    #[arg(
        long,
        env = "EASOS_RUNTIME_HOME",
        default_value = "/run/easos",
        global = true
    )]
    runtime_root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    List,
    Status {
        id: String,
    },
    Install {
        source: PathBuf,
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
    Autostart {
        id: String,
        #[command(subcommand)]
        action: AutostartAction,
    },
    Config {
        id: String,
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
enum AutostartAction {
    Enable,
    Disable,
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    Get,
    Set { key: String, value: String },
    Unset { key: String },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("easos: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let command = match args.command {
        Command::List => ControlCommand::List,
        Command::Status { id } => ControlCommand::Status { id },
        Command::Install { source } => ControlCommand::Install {
            source: source.display().to_string(),
        },
        Command::Uninstall { id } => ControlCommand::Uninstall { id },
        Command::Start { id } => ControlCommand::Start { id },
        Command::Stop { id } => ControlCommand::Stop { id },
        Command::Autostart { id, action } => ControlCommand::SetAutostart {
            id,
            enabled: matches!(action, AutostartAction::Enable),
        },
        Command::Config { id, action } => match action {
            ConfigAction::Get => ControlCommand::GetConfig { id },
            ConfigAction::Set { key, value } => ControlCommand::SetConfig {
                id,
                key,
                value: parse_value(&value),
            },
            ConfigAction::Unset { key } => ControlCommand::UnsetConfig { id, key },
        },
    };

    let layout = Layout::new(args.root, args.runtime_root);
    let mut stream = UnixStream::connect(&layout.socket_file)
        .await
        .map_err(|error| {
            KernelError::Unavailable(format!(
                "cannot connect to {}: {error}",
                layout.socket_file.display()
            ))
        })?;
    let request = ControlRequest::new(command);
    stream.write_all(&serde_json::to_vec(&request)?).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let response: ControlResponse = serde_json::from_str(&line)?;
    if response.protocol_version != CONTROL_PROTOCOL_VERSION {
        return Err(KernelError::InvalidData(format!(
            "daemon returned control protocol version {}",
            response.protocol_version
        )));
    }
    if let Some(error) = response.error {
        return Err(KernelError::Unavailable(format!(
            "{}: {}",
            error.code, error.message
        )));
    }
    let data = response
        .data
        .ok_or_else(|| KernelError::Internal("daemon response has no data".to_owned()))?;
    println!("{}", serde_json::to_string_pretty(&data)?);
    Ok(())
}

fn parse_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()))
}
