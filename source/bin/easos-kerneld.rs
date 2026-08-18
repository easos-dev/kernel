use std::path::PathBuf;

use clap::Parser;
use easos_kernel::run_daemon;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "easos-kerneld", version, about = "EasOS Kernel daemon")]
struct Args {
    #[arg(long, env = "EASOS_HOME", default_value = "/var/lib/easos")]
    root: PathBuf,
    #[arg(long, env = "EASOS_RUNTIME_HOME", default_value = "/run/easos")]
    runtime_root: PathBuf,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = Args::parse();
    if let Err(error) = run_daemon(args.root, args.runtime_root).await {
        eprintln!("easos-kerneld: {error}");
        std::process::exit(1);
    }
}
