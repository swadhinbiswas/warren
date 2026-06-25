use anyhow::{Context, Result, bail};
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::instance::metadata::{InstanceInfo, InstallInfo, PathsInfo, SourceType};
use crate::install::download::{parse_source, SourceInfo};
use crate::install::{InstallerRewriter, InstallerExecutor};
use crate::ui::theme::Theme;
use crate::ui::progress;

pub async fn execute(
    config: &WarrenConfig,
    theme: &Theme,
    source: &str,
    alias: &str,
    yes: bool,
) -> Result<()> {
    instance::validate_alias(alias)?;

    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if layout.exists() {
        if !yes {
            use dialoguer::Confirm;
            let overwrite = Confirm::new()
                .with_prompt(format!("Instance '{}' already exists. Overwrite?", alias))
                .default(false)
                .interact()?;
            if !overwrite {
                theme.warn("Aborted.");
                return Ok(());
            }
        }
        Launcher::uninstall(&config.paths.bin_dir, alias)?;
        layout.destroy()?;
    }

    theme.header(&format!("digging {}", alias));
    layout.create()?;

    let source_info = parse_source(source);
    let (installer_hash, source_type) = match &source_info {
        SourceInfo::RemoteScript { url, .. } => {
            let spinner = progress::spinner(&format!("Downloading installer from {}", url));
            let original_path = layout.installers_dir().join("original.sh");
            let hash = crate::install::download_installer(url, &original_path).await?;
            spinner.finish_with_message("Downloaded installer");
            (Some(hash), SourceType::RemoteScript)
        }
        SourceInfo::LocalScript { path } => {
            let src_path = std::path::Path::new(path);
            if !src_path.exists() {
                layout.destroy()?;
                bail!("local installer not found: {}", path);
            }
            let original_path = layout.installers_dir().join("original.sh");
            let hash = crate::install::download::read_local_installer(src_path, &original_path)?;
            (Some(hash), SourceType::LocalScript)
        }
        SourceInfo::Package { name } => {
            theme.step("📦", &format!("Package source: {}", name));
            if let Ok(found) = which::which(name) {
                let dest = layout.bin_dir().join(name);
                std::fs::copy(&found, &dest)
                    .with_context(|| format!("failed to copy {} to instance", name))?;
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
                (None, SourceType::Package)
            } else {
                layout.destroy()?;
                bail!("package '{}' not found on PATH", name);
            }
        }
    };

    if matches!(source_info, SourceInfo::RemoteScript { .. } | SourceInfo::LocalScript { .. }) {
        let original_path = layout.installers_dir().join("original.sh");
        let original_content = std::fs::read_to_string(&original_path)
            .context("failed to read original installer")?;
        let rewriter = InstallerRewriter::new(&layout.root);
        let result = rewriter.rewrite(&original_content);
        theme.step("✎", &format!(
            "Rewriting {} path reference{}",
            result.total_changes(),
            if result.total_changes() == 1 { "" } else { "s" }
        ));
        InstallerRewriter::validate(&result.content, &layout.root)?;
        let rewritten_path = layout.installers_dir().join("rewritten.sh");
        std::fs::write(&rewritten_path, &result.content)
            .context("failed to write rewritten installer")?;
        if !yes && (config.defaults.show_diff || result.has_changes()) {
            use dialoguer::Confirm;
            let show = Confirm::new()
                .with_prompt("Show diff?")
                .default(false)
                .interact()?;
            if show {
                let diff = crate::install::rewriter::generate_diff(&original_content, &result.content);
                eprintln!("\n{}\n", diff);
            }
        }
        let spinner = progress::spinner("Running installer...");
        InstallerExecutor::execute(&rewritten_path, &layout).await?;
        spinner.finish_with_message("Installer complete");
    }

    let binary_name = crate::install::detect::detect_binary(&layout.bin_dir())?
        .unwrap_or_else(|| alias.to_string());
    let version = crate::install::detect::detect_version(&layout.bin_dir(), &binary_name);
    let launcher_content = Launcher::generate(alias, &layout, &binary_name);
    Launcher::write(&layout, &launcher_content)?;
    let launcher_dest = Launcher::install(&layout, &config.paths.bin_dir, alias)?;

    let now = Utc::now();
    let shell = crate::shell::detect_shell();
    let metadata = InstanceMetadata {
        instance: InstanceInfo {
            alias: alias.to_string(),
            app_name: binary_name.clone(),
            version,
            shell: shell.to_string(),
            created_at: now,
            updated_at: now,
        },
        install: InstallInfo {
            source: source.to_string(),
            source_type,
            installer_hash,
        },
        paths: PathsInfo {
            root: layout.root.to_string_lossy().to_string(),
            bin: layout.bin_dir().to_string_lossy().to_string(),
            launcher: launcher_dest.to_string_lossy().to_string(),
        },
    };
    metadata.save(&layout.metadata_path())?;

    theme.blank();
    theme.success(&format!("Installed: {}", alias));
    theme.success(&format!("Launcher: {}", InstanceMetadata::display_path(&launcher_dest.to_string_lossy())));
    theme.success(&format!("Binary:   {}", InstanceMetadata::display_path(&layout.bin_dir().join(&binary_name).to_string_lossy())));
    theme.blank();
    theme.dim(&format!("Run it:  {}", alias));
    theme.dim(&format!("         warren run {}", alias));
    theme.blank();

    Ok(())
}
