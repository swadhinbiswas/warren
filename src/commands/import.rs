use std::path::Path;
use anyhow::{Result, bail, Context};
use flate2::read::GzDecoder;

use crate::config::WarrenConfig;
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, path: &Path, alias_override: Option<&str>) -> Result<()> {
    if !path.exists() { bail!("archive not found: {}", path.display()); }
    theme.step("📦", &format!("Importing from {}", path.display()));
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open archive {}", path.display()))?;
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive.unpack(&config.paths.instances_dir).with_context(|| "failed to extract archive")?;

    let file2 = std::fs::File::open(path)?;
    let dec2 = GzDecoder::new(file2);
    let mut archive2 = tar::Archive::new(dec2);
    let first_entry = archive2.entries()?.next();
    let archive_alias = first_entry
        .and_then(|e| e.ok())
        .and_then(|e| e.path().ok().and_then(|p| p.components().next().map(|c| c.as_os_str().to_string_lossy().to_string())))
        .unwrap_or_default();

    let alias = alias_override.unwrap_or(&archive_alias);
    instance::validate_alias(alias)?;

    if alias_override.is_some() && alias != archive_alias {
        let src = config.paths.instances_dir.join(&archive_alias);
        let dst = config.paths.instances_dir.join(alias);
        if dst.exists() { bail!("instance '{}' already exists", alias); }
        std::fs::rename(&src, &dst)
            .with_context(|| format!("failed to rename {} to {}", src.display(), dst.display()))?;
    }

    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    let mut metadata = InstanceMetadata::load(&layout.metadata_path())?;
    if alias != &metadata.instance.alias {
        metadata.instance.alias = alias.to_string();
        metadata.paths.root = layout.root.to_string_lossy().to_string();
        metadata.paths.bin = layout.bin_dir().to_string_lossy().to_string();
    }
    let launcher_content = Launcher::generate(alias, &layout, &metadata.instance.app_name);
    Launcher::write(&layout, &launcher_content)?;
    let launcher_dest = Launcher::install(&layout, &config.paths.bin_dir, alias)?;
    metadata.paths.launcher = launcher_dest.to_string_lossy().to_string();
    metadata.save(&layout.metadata_path())?;
    theme.success(&format!("Imported: {}", alias));
    theme.success(&format!("Launcher: {}", InstanceMetadata::display_path(&launcher_dest.to_string_lossy())));
    theme.blank();
    Ok(())
}
