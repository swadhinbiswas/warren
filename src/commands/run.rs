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
/// Every instance — CLI, native GUI, snap, *and* Flatpak — gets a private
/// `$HOME`/`$XDG_*`/`$TMPDIR`. Flatpak honors `$HOME` by relocating its
/// per-app data to `$HOME/.var/app/<id>/`, so each alias keeps its own
/// login and files (verified live). GUI instances keep the host
/// `$XDG_RUNTIME_DIR` (Wayland / DBus sockets live there); CLI instances
/// get the fully isolated runtime dir.
fn apply_sandbox_env(
    cmd: &mut std::process::Command,
    layout: &InstanceLayout,
    alias: &str,
    gui: bool,
) {
    let data_dirs = format!(
        "{}:/usr/local/share:/usr/share:/var/lib/flatpak/exports/share",
        layout.data_dir().display()
    );
    // Ensure the private storage exists even for older instances created
    // before the launcher learned to `mkdir -p` it.
    for dir in [
        layout.home_dir(),
        layout.config_dir(),
        layout.cache_dir(),
        layout.data_dir(),
        layout.state_dir(),
        layout.tmp_dir(),
        layout.runtime_dir(),
        layout.bin_dir(),
    ] {
        std::fs::create_dir_all(&dir).ok();
    }
    cmd.env("HOME", layout.home_dir())
        .env("XDG_CONFIG_HOME", layout.config_dir())
        .env("XDG_CACHE_HOME", layout.cache_dir())
        .env("XDG_DATA_HOME", layout.data_dir())
        .env("XDG_DATA_DIRS", &data_dirs)
        .env("XDG_CONFIG_DIRS", "/etc/xdg")
        .env("XDG_STATE_HOME", layout.state_dir())
        .env("TMPDIR", layout.tmp_dir())
        .env(
            "PATH",
            format!(
                "{}:{}",
                layout.bin_dir().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
    cmd.env("WARREN_INSTANCE", alias)
        .env("WARREN_INSTANCE_DIR", &layout.root);
    if gui {
        cmd.env("WARREN_RUNTIME_DIR", layout.runtime_dir());
        if let Ok(host) = std::env::var("XDG_RUNTIME_DIR") {
            cmd.env("WARREN_HOST_RUNTIME_DIR", host);
        }
    } else {
        cmd.env("XDG_RUNTIME_DIR", layout.runtime_dir());
    }
    // Only host *installation* metadata is inherited. Ephemeral per-run
    // sandbox variables (sandbox id, instance id, …) are deliberately
    // dropped so one instance can never leak its sandbox identity into
    // another; `flatpak run` sets fresh ones per launch.
    for var in &["FLATPAK_DATA_DIRS", "FLATPAK_BASEDIR"] {
        if let Ok(val) = std::env::var(var) {
            cmd.env(var, val);
        }
    }
    for var in &[
        "FLATPAK_ID",
        "FLATPAK_SANDBOX_DIR",
        "FLATPAK_INSTANCE_ID",
        "FLATPAK_DEST",
    ] {
        cmd.env_remove(var);
    }
}
