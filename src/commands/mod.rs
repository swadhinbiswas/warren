pub mod clone;
pub mod dig;
pub mod export;
pub mod import;
pub mod inspect;
pub mod ls;
pub mod rm;
pub mod run;
pub mod session;
pub mod update;

use crate::cli::{Cli, Command, ShellAction};
use crate::config::WarrenConfig;
use crate::ui::theme::Theme;
use anyhow::{Context, Result};

pub async fn dispatch(cli: Cli, config: &WarrenConfig) -> Result<()> {
    let theme = Theme::new();

    match cli.command {
        Command::Dig {
            source,
            alias,
            yes,
            gui,
            no_gui,
        } => {
            config.ensure_dirs()?;
            let gui_override = if gui {
                Some(true)
            } else if no_gui {
                Some(false)
            } else {
                None
            };
            let _lock = crate::lock::InstanceLock::acquire(&config.paths.instances_dir, &alias)
                .with_context(|| format!("could not lock instance '{}'", alias))?;
            dig::execute(config, &theme, &source, &alias, yes, gui_override).await
        }
        Command::Run { alias, args } => run::execute(config, &alias, &args).await,
        Command::Ls => ls::execute(config, &theme).await,
        Command::Inspect { alias } => inspect::execute(config, &theme, &alias).await,
        Command::Rm { alias, yes } => {
            let _lock = crate::lock::InstanceLock::acquire(&config.paths.instances_dir, &alias)
                .with_context(|| format!("could not lock instance '{}'", alias))?;
            rm::execute(config, &theme, &alias, yes).await
        }
        Command::Update { alias, yes } => {
            let _lock = crate::lock::InstanceLock::acquire(&config.paths.instances_dir, &alias)
                .with_context(|| format!("could not lock instance '{}'", alias))?;
            update::execute(config, &theme, &alias, yes).await
        }
        Command::Clone {
            source,
            dest,
            copy_data,
        } => {
            config.ensure_dirs()?;
            let _lock_src =
                crate::lock::InstanceLock::acquire(&config.paths.instances_dir, &source)
                    .with_context(|| format!("could not lock source instance '{}'", source))?;
            let _lock_dst = crate::lock::InstanceLock::acquire(&config.paths.instances_dir, &dest)
                .with_context(|| format!("could not lock destination instance '{}'", dest))?;
            clone::execute(config, &theme, &source, &dest, copy_data).await
        }
        Command::Export { alias, out } => {
            let _lock = crate::lock::InstanceLock::acquire(&config.paths.instances_dir, &alias)
                .with_context(|| format!("could not lock instance '{}'", alias))?;
            export::execute(config, &theme, &alias, out.as_deref()).await
        }
        Command::Import { path, alias, fresh } => {
            config.ensure_dirs()?;
            let _lock = crate::lock::GlobalLock::acquire(&config.paths.instances_dir)
                .with_context(|| "could not acquire global warren lock")?;
            import::execute(config, &theme, &path, alias.as_deref(), fresh).await
        }
        Command::Env => {
            print_env(config, &theme);
            Ok(())
        }
        Command::Session { action } => {
            // Ensure the sessions dir exists for every session subcommand
            // so `ls`/`prune` work on fresh installs.
            config.ensure_dirs()?;
            session::execute(config, &theme, &action).await
        }
        Command::Shell { action } => match action {
            ShellAction::Install => {
                let shell = crate::shell::detect_shell();
                crate::shell::integration::install_shell_integration(
                    &shell,
                    &config.paths.bin_dir,
                )?;
                theme.success(&format!("Shell integration installed for {}", shell));
                Ok(())
            }
            ShellAction::Info => {
                let shell = crate::shell::detect_shell();
                theme.kv("Shell", &shell.to_string());
                theme.kv("Bin dir", &config.paths.bin_dir.to_string_lossy());
                Ok(())
            }
        },
    }
}

fn print_env(config: &WarrenConfig, theme: &Theme) {
    theme.header("environment");
    theme.kv("Version", env!("CARGO_PKG_VERSION"));
    theme.kv(
        "Warren dir",
        &WarrenConfig::warren_dir_raw().to_string_lossy(),
    );
    theme.kv("Instances", &config.paths.instances_dir.to_string_lossy());
    theme.kv("Bin dir", &config.paths.bin_dir.to_string_lossy());
    theme.kv("Shell", &crate::shell::detect_shell().to_string());
    theme.kv("Config", &WarrenConfig::config_path().to_string_lossy());
    theme.blank();
}
