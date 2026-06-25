pub mod dig;
pub mod run;
pub mod ls;
pub mod inspect;
pub mod rm;
pub mod update;
pub mod clone;
pub mod export;
pub mod import;

use anyhow::Result;
use crate::cli::{Cli, Command, ShellAction};
use crate::config::WarrenConfig;
use crate::ui::theme::Theme;

pub async fn dispatch(cli: Cli) -> Result<()> {
    let config = WarrenConfig::load()?;
    let theme = Theme::new();

    match cli.command {
        Command::Dig { source, alias, yes } => {
            config.ensure_dirs()?;
            dig::execute(&config, &theme, &source, &alias, yes).await
        }
        Command::Run { alias, args } => {
            run::execute(&config, &alias, &args).await
        }
        Command::Ls => {
            ls::execute(&config, &theme).await
        }
        Command::Inspect { alias } => {
            inspect::execute(&config, &theme, &alias).await
        }
        Command::Rm { alias, yes } => {
            rm::execute(&config, &theme, &alias, yes).await
        }
        Command::Update { alias, yes } => {
            update::execute(&config, &theme, &alias, yes).await
        }
        Command::Clone { source, dest } => {
            config.ensure_dirs()?;
            clone::execute(&config, &theme, &source, &dest).await
        }
        Command::Export { alias, out } => {
            export::execute(&config, &theme, &alias, out.as_deref()).await
        }
        Command::Import { path, alias } => {
            config.ensure_dirs()?;
            import::execute(&config, &theme, &path, alias.as_deref()).await
        }
        Command::Env => {
            print_env(&config, &theme);
            Ok(())
        }
        Command::Shell { action } => {
            match action {
                ShellAction::Install => {
                    let shell = crate::shell::detect_shell();
                    crate::shell::integration::install_shell_integration(&shell, &config.paths.bin_dir)?;
                    theme.success(&format!("Shell integration installed for {}", shell));
                    Ok(())
                }
                ShellAction::Info => {
                    let shell = crate::shell::detect_shell();
                    theme.kv("Shell", &shell.to_string());
                    theme.kv("Bin dir", &config.paths.bin_dir.to_string_lossy());
                    Ok(())
                }
            }
        }
    }
}

fn print_env(config: &WarrenConfig, theme: &Theme) {
    theme.header("environment");
    theme.kv("Version", env!("CARGO_PKG_VERSION"));
    theme.kv("Warren dir", &WarrenConfig::warren_dir().to_string_lossy());
    theme.kv("Instances", &config.paths.instances_dir.to_string_lossy());
    theme.kv("Bin dir", &config.paths.bin_dir.to_string_lossy());
    theme.kv("Shell", &crate::shell::detect_shell().to_string());
    theme.kv("Config", &WarrenConfig::config_path().to_string_lossy());
    theme.blank();
}
