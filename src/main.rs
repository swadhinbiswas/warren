use clap::Parser;
use tracing_subscriber::EnvFilter;

mod app;
mod cli;
mod commands;
mod config;
mod install;
mod instance;
mod lock;
mod session;
mod shell;
mod ui;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if is_root() {
        let theme = ui::theme::Theme::new();
        theme.error("warren refuses to run as root. Please run as a normal user.");
        std::process::exit(1);
    }

    if let Err(err) = config::WarrenConfig::validate_home() {
        let theme = ui::theme::Theme::new();
        theme.error(&err.to_string());
        std::process::exit(1);
    }

    let config = match config::WarrenConfig::load() {
        Ok(config) => config,
        Err(err) => {
            let theme = ui::theme::Theme::new();
            theme.error(&err.to_string());
            std::process::exit(1);
        }
    };
    configure_ui(&config);

    let cli = cli::Cli::parse();
    if let Err(err) = commands::dispatch(cli, &config).await {
        let theme = ui::theme::Theme::new();
        let mut msg = err.to_string();
        let mut source = err.source();
        while let Some(cause) = source {
            msg.push_str(&format!("\n     caused by: {}", cause));
            source = cause.source();
        }
        theme.error(&msg);
        std::process::exit(1);
    }
}

/// Configure color and progress rendering once, globally.
///
/// Color precedence: `NO_COLOR` (any non-empty value) always wins,
/// then the `ui.color` config flag, then TTY detection. `CLICOLOR_FORCE`
/// re-enables color even when stderr is not a terminal.
fn configure_ui(config: &config::WarrenConfig) {
    let stderr_is_terminal = console::Term::stderr().is_term();
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let force_color = std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| !v.is_empty());

    let color = !no_color && config.ui.color && (stderr_is_terminal || force_color);
    console::set_colors_enabled(color);

    ui::progress::set_enabled(config.ui.progress && stderr_is_terminal);
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("Uid:"))
                .and_then(|line| {
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    fields.get(2).and_then(|uid| uid.parse::<u32>().ok())
                })
        })
        .map(|euid| euid == 0)
        .unwrap_or(false)
}
