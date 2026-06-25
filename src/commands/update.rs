use anyhow::{Result, bail};
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::instance::{InstanceLayout, InstanceMetadata, Launcher};
use crate::install::download::{parse_source, SourceInfo};
use crate::install::{InstallerRewriter, InstallerExecutor};
use crate::ui::theme::Theme;
use crate::ui::progress;

pub async fn execute(config: &WarrenConfig, theme: &Theme, alias: &str, _yes: bool) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() { bail!("instance '{}' not found", alias); }
    let mut metadata = InstanceMetadata::load(&layout.metadata_path())?;
    theme.header(&format!("updating {}", alias));
    let source_info = parse_source(&metadata.install.source);
    match &source_info {
        SourceInfo::RemoteScript { url, .. } => {
            let spinner = progress::spinner(&format!("Downloading installer from {}", url));
            let original_path = layout.installers_dir().join("original.sh");
            let hash = crate::install::download_installer(url, &original_path).await?;
            spinner.finish_with_message("Downloaded installer");
            let original_content = std::fs::read_to_string(&original_path)?;
            let rewriter = InstallerRewriter::new(&layout.root);
            let result = rewriter.rewrite(&original_content);
            theme.step("✎", &format!("Rewriting {} path references", result.total_changes()));
            InstallerRewriter::validate(&result.content, &layout.root)?;
            let rewritten_path = layout.installers_dir().join("rewritten.sh");
            std::fs::write(&rewritten_path, &result.content)?;
            let spinner = progress::spinner("Running installer...");
            InstallerExecutor::execute(&rewritten_path, &layout).await?;
            spinner.finish_with_message("Update complete");
            metadata.install.installer_hash = Some(hash);
        }
        SourceInfo::LocalScript { path } => {
            let src = std::path::Path::new(path);
            if !src.exists() { bail!("local installer not found: {}", path); }
            let original_path = layout.installers_dir().join("original.sh");
            let hash = crate::install::download::read_local_installer(src, &original_path)?;
            let original_content = std::fs::read_to_string(&original_path)?;
            let rewriter = InstallerRewriter::new(&layout.root);
            let result = rewriter.rewrite(&original_content);
            let rewritten_path = layout.installers_dir().join("rewritten.sh");
            std::fs::write(&rewritten_path, &result.content)?;
            let spinner = progress::spinner("Running installer...");
            InstallerExecutor::execute(&rewritten_path, &layout).await?;
            spinner.finish_with_message("Update complete");
            metadata.install.installer_hash = Some(hash);
        }
        SourceInfo::Package { name } => {
            theme.warn(&format!("Package source '{}' — manual update not supported yet.", name));
            return Ok(());
        }
    }
    let binary_name = &metadata.instance.app_name;
    metadata.instance.version = crate::install::detect::detect_version(&layout.bin_dir(), binary_name);
    metadata.instance.updated_at = Utc::now();
    let launcher_content = Launcher::generate(alias, &layout, binary_name);
    Launcher::write(&layout, &launcher_content)?;
    Launcher::install(&layout, &config.paths.bin_dir, alias)?;
    metadata.save(&layout.metadata_path())?;
    theme.success(&format!("Updated: {}", alias));
    Ok(())
}
