use anyhow::{Result, bail, Context};
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, source: &str, dest: &str) -> Result<()> {
    instance::validate_alias(dest)?;
    let source_layout = InstanceLayout::new(&config.paths.instances_dir, source);
    if !source_layout.exists() { bail!("source instance '{}' not found", source); }
    let dest_layout = InstanceLayout::new(&config.paths.instances_dir, dest);
    if dest_layout.exists() { bail!("destination instance '{}' already exists", dest); }
    theme.header(&format!("cloning {} → {}", source, dest));
    copy_dir_recursive(&source_layout.root, &dest_layout.root).context("failed to copy instance directory")?;
    let mut metadata = InstanceMetadata::load(&dest_layout.metadata_path())?;
    metadata.instance.alias = dest.to_string();
    metadata.instance.created_at = Utc::now();
    metadata.instance.updated_at = Utc::now();
    metadata.paths.root = dest_layout.root.to_string_lossy().to_string();
    metadata.paths.bin = dest_layout.bin_dir().to_string_lossy().to_string();
    let launcher_content = Launcher::generate(dest, &dest_layout, &metadata.instance.app_name);
    Launcher::write(&dest_layout, &launcher_content)?;
    let launcher_dest = Launcher::install(&dest_layout, &config.paths.bin_dir, dest)?;
    metadata.paths.launcher = launcher_dest.to_string_lossy().to_string();
    metadata.save(&dest_layout.metadata_path())?;
    theme.success(&format!("Cloned: {} → {}", source, dest));
    theme.success(&format!("Launcher: {}", InstanceMetadata::display_path(&launcher_dest.to_string_lossy())));
    theme.blank();
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() { copy_dir_recursive(&src_path, &dst_path)?; }
        else { std::fs::copy(&src_path, &dst_path)?; }
    }
    Ok(())
}
