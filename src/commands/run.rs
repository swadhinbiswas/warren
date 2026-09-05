use anyhow::{Context, Result, bail};

use crate::config::WarrenConfig;
use crate::instance::{InstanceLayout, InstanceMetadata};

pub async fn execute(config: &WarrenConfig, alias: &str, args: &[String]) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() {
        bail!("instance '{}' not found", alias);
    }
    let metadata = InstanceMetadata::load(&layout.metadata_path())
        .with_context(|| format!("failed to load metadata for instance '{}'", alias))?;

    let mut cmd = if metadata.install.launch_command.is_empty() {
        // Classic file mode: run the instance-local binary.
        let binary_path = layout.bin_dir().join(&metadata.instance.app_name);
        if !binary_path.exists() {
            bail!(
                "binary '{}' not found in instance '{}'",
                metadata.instance.app_name,
                alias
            );
        }
        std::process::Command::new(&binary_path)
    } else {
        // Wrap mode: re-execute the host command inside the sandbox.
        // (The installed launcher does the same; `warren run` mirrors it
        // so behavior is identical with or without ~/.local/bin on PATH.)
        let launch = &metadata.install.launch_command;
        let mut cmd = std::process::Command::new(&launch[0]);
        if launch.len() > 1 {
            cmd.args(&launch[1..]);
        }
        cmd
    };
    cmd.args(args);
    apply_sandbox_env(&mut cmd, &layout, alias, metadata.instance.gui);

    if metadata.instance.gui {
        // Graphical apps must not block the terminal: detach and report.
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let child = cmd
            .spawn()
            .with_context(|| format!("failed to launch instance '{}'", alias))?;
        eprintln!("  launched {} (pid {})", alias, child.id());
        Ok(())
    } else {
        let status = cmd
            .status()
            .with_context(|| format!("failed to run instance '{}'", alias))?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}

/// Shared sandbox environment. Mirrors the generated launcher script so
/// `warren run` and the `~/.local/bin` launcher behave identically.
///
/// GUI instances keep the host `$XDG_RUNTIME_DIR` (Wayland / DBus sockets
/// live there); CLI instances get the fully isolated runtime dir.
fn apply_sandbox_env(
    cmd: &mut std::process::Command,
    layout: &InstanceLayout,
    alias: &str,
    gui: bool,
) {
    cmd.env("HOME", layout.home_dir())
        .env("XDG_CONFIG_HOME", layout.config_dir())
        .env("XDG_CACHE_HOME", layout.cache_dir())
        .env("XDG_DATA_HOME", layout.data_dir())
        .env("XDG_STATE_HOME", layout.state_dir())
        .env("TMPDIR", layout.tmp_dir())
        .env("WARREN_INSTANCE", alias)
        .env("WARREN_INSTANCE_DIR", &layout.root);
    if gui {
        cmd.env("WARREN_RUNTIME_DIR", layout.runtime_dir());
        if let Ok(host) = std::env::var("XDG_RUNTIME_DIR") {
            cmd.env("WARREN_HOST_RUNTIME_DIR", host);
        }
    } else {
        cmd.env("XDG_RUNTIME_DIR", layout.runtime_dir());
    }
}
