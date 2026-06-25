use anyhow::{Context, Result, bail};

use crate::config::WarrenConfig;
use crate::instance::{InstanceLayout, InstanceMetadata};

pub async fn execute(config: &WarrenConfig, alias: &str, args: &[String]) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() { bail!("instance '{}' not found", alias); }
    let metadata = InstanceMetadata::load(&layout.metadata_path())
        .with_context(|| format!("failed to load metadata for instance '{}'", alias))?;
    let binary_path = layout.bin_dir().join(&metadata.instance.app_name);
    if !binary_path.exists() {
        bail!("binary '{}' not found in instance '{}'", metadata.instance.app_name, alias);
    }
    let status = std::process::Command::new(&binary_path)
        .args(args)
        .env("HOME", layout.home_dir())
        .env("XDG_CONFIG_HOME", layout.config_dir())
        .env("XDG_CACHE_HOME", layout.cache_dir())
        .env("XDG_DATA_HOME", layout.data_dir())
        .env("XDG_STATE_HOME", layout.state_dir())
        .env("XDG_RUNTIME_DIR", layout.runtime_dir())
        .env("TMPDIR", layout.tmp_dir())
        .env("WARREN_INSTANCE", alias)
        .env("WARREN_INSTANCE_DIR", &layout.root)
        .status()
        .with_context(|| format!("failed to run {}", binary_path.display()))?;
    if !status.success() { std::process::exit(status.code().unwrap_or(1)); }
    Ok(())
}
