use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod config;
mod instance;
mod install;
mod shell;
mod commands;
mod ui;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if is_root() {
        anyhow::bail!("warren refuses to run as root. Please run as a normal user.");
    }

    let cli = cli::Cli::parse();
    commands::dispatch(cli).await
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines()
                .find(|line| line.starts_with("Uid:"))
                .and_then(|line| {
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    fields.get(2).and_then(|uid| uid.parse::<u32>().ok())
                })
        })
        .map(|euid| euid == 0)
        .unwrap_or(false)
}
