use anyhow::{Result, bail};

use crate::config::WarrenConfig;
use crate::instance::layout::format_bytes;
use crate::instance::{InstanceLayout, InstanceMetadata};
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, alias: &str) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() {
        bail!("instance '{}' not found", alias);
    }
    let metadata = InstanceMetadata::load(&layout.metadata_path())?;
    let usage = layout.disk_usage()?;
    theme.blank();
    eprintln!("  {}", theme.brand.apply_to(alias));
    eprintln!("  {}", "─".repeat(45));
    let version_str = metadata.instance.version.as_deref().unwrap_or("unknown");
    let kind = if metadata.instance.gui { "gui" } else { "cli" };
    theme.kv(
        "App",
        &format!("{} {} [{}]", metadata.instance.app_name, version_str, kind),
    );
    theme.kv("Shell", &metadata.instance.shell);
    theme.kv(
        "Installed",
        &metadata.instance.created_at.format("%Y-%m-%d").to_string(),
    );
    theme.kv("Source", &metadata.install.source);
    theme.kv("Type", &metadata.install.source_type.to_string());
    if !metadata.install.launch_command.is_empty() {
        theme.kv("Launch", &metadata.install.launch_command.join(" "));
    }
    theme.blank();
    theme.section("Paths");
    theme.kv(
        "  Root",
        &InstanceMetadata::display_path(&metadata.paths.root),
    );
    theme.kv(
        "  Launcher",
        &InstanceMetadata::display_path(&metadata.paths.launcher),
    );
    if metadata.install.launch_command.is_empty() {
        theme.kv(
            "  Binary",
            &InstanceMetadata::display_path(&format!(
                "{}/{}",
                metadata.paths.bin, metadata.instance.app_name
            )),
        );
    }
    if let Some(desktop) = metadata.paths.desktop_file.as_deref() {
        theme.kv("  Desktop", &InstanceMetadata::display_path(desktop));
    }
    theme.blank();
    theme.section("Disk usage");
    theme.kv("  bin/", &format_bytes(usage.bin));
    theme.kv("  config/", &format_bytes(usage.config));
    theme.kv("  cache/", &format_bytes(usage.cache));
    theme.kv("  data/", &format_bytes(usage.data));
    theme.kv("  home/", &format_bytes(usage.home));
    theme.kv("  total", &format_bytes(usage.total()));
    if !metadata.install.launch_command.is_empty() {
        theme.blank();
        theme.dim("Wrap mode: the host app is reused, so bin/ stays empty and");
        theme.dim("disk usage is just this instance's private data.");
    }
    theme.blank();
    Ok(())
}
