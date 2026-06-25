use anyhow::{Result, bail};

use crate::config::WarrenConfig;
use crate::instance::{InstanceLayout, Launcher};
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, alias: &str, yes: bool) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() { bail!("instance '{}' not found", alias); }
    if !yes {
        use dialoguer::Confirm;
        let confirm = Confirm::new()
            .with_prompt(format!("Remove instance '{}' and all its data?", alias))
            .default(false)
            .interact()?;
        if !confirm { theme.warn("Aborted."); return Ok(()); }
    }
    Launcher::uninstall(&config.paths.bin_dir, alias)?;
    layout.destroy()?;
    theme.success(&format!("Removed instance '{}'", alias));
    Ok(())
}
