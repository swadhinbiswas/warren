use std::path::Path;
use std::os::unix::fs::PermissionsExt;
use anyhow::{Context, Result, bail};

use crate::instance::InstanceLayout;

pub struct InstallerExecutor;

impl InstallerExecutor {
    pub async fn execute(script_path: &Path, layout: &InstanceLayout) -> Result<()> {
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(script_path, perms)
            .with_context(|| format!("failed to make installer executable: {}", script_path.display()))?;
        tracing::info!(script = %script_path.display(), instance = %layout.alias, "executing installer");
        let output = tokio::process::Command::new("bash")
            .arg(script_path)
            .env("HOME", layout.home_dir())
            .env("XDG_CONFIG_HOME", layout.config_dir())
            .env("XDG_CACHE_HOME", layout.cache_dir())
            .env("XDG_DATA_HOME", layout.data_dir())
            .env("XDG_STATE_HOME", layout.state_dir())
            .env("XDG_RUNTIME_DIR", layout.runtime_dir())
            .env("TMPDIR", layout.tmp_dir())
            .env("WARREN_INSTANCE", &layout.alias)
            .env("WARREN_INSTANCE_DIR", &layout.root)
            .env("PATH", Self::build_path(layout))
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .output()
            .await
            .with_context(|| format!("failed to execute installer {}", script_path.display()))?;
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            bail!("installer exited with code {}", code);
        }
        tracing::info!(instance = %layout.alias, "installer completed successfully");
        Ok(())
    }

    fn build_path(layout: &InstanceLayout) -> String {
        let instance_bin = layout.bin_dir().to_string_lossy().to_string();
        let current_path = std::env::var("PATH").unwrap_or_default();
        format!("{}:{}", instance_bin, current_path)
    }
}
