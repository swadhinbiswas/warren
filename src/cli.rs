use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// warren — install any CLI tool unlimited times, fully isolated
#[derive(Parser, Debug)]
#[command(
    name = "warren",
    version,
    about = "Install any CLI tool unlimited times. Every instance is its own world.",
    long_about = "warren is a rootless CLI runtime that lets you install and run unlimited\nisolated instances of any CLI application — each with its own identity,\nconfiguration, and data.",
    after_help = "Examples:\n  warren dig \"curl -fsSL https://example.com/install | bash\" --as myapp\n  warren ls\n  warren run myapp -- --help\n  warren inspect myapp"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install a new instance from a source
    Dig {
        /// Install source: URL piped to shell, local script, or package name
        source: String,
        /// Alias for this instance
        #[arg(long = "as", value_name = "ALIAS")]
        alias: String,
        /// Skip confirmation prompts
        #[arg(long, short)]
        yes: bool,
    },
    /// Run an installed instance
    Run {
        /// Instance alias to run
        alias: String,
        /// Arguments to pass to the application
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// List all instances
    Ls,
    /// Inspect an instance
    Inspect {
        /// Instance alias to inspect
        alias: String,
    },
    /// Remove an instance
    Rm {
        /// Instance alias to remove
        alias: String,
        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,
    },
    /// Update an instance by re-running its installer
    Update {
        /// Instance alias to update
        alias: String,
        /// Skip confirmation prompts
        #[arg(long, short)]
        yes: bool,
    },
    /// Clone an instance into a new one
    Clone {
        /// Source instance alias
        source: String,
        /// Destination alias for the clone
        dest: String,
    },
    /// Export an instance to a portable archive
    Export {
        /// Instance alias to export
        alias: String,
        /// Output path for the archive
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Import an instance from an archive
    Import {
        /// Path to the archive file
        path: PathBuf,
        /// Alias for the imported instance
        #[arg(long = "as", value_name = "ALIAS")]
        alias: Option<String>,
    },
    /// Show warren environment information
    Env,
    /// Manage shell integration
    Shell {
        #[command(subcommand)]
        action: ShellAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ShellAction {
    /// Install shell integration (PATH setup)
    Install,
    /// Show current shell detection info
    Info,
}
