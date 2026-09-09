use anyhow::{Result, bail};
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::install::download::{SourceInfo, parse_source};
use crate::install::{InstallerExecutor, InstallerRewriter};
use crate::instance::{InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::progress;
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, alias: &str, yes: bool) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() {
        bail!("instance '{}' not found", alias);
    }
    let mut metadata = InstanceMetadata::load(&layout.metadata_path())?;
    theme.header(&format!("updating {}", alias));
    if !yes {
        use dialoguer::Confirm;
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Re-run the installer for instance '{}'? This may overwrite its files.",
                alias
            ))
            .default(false)
            .interact()?;
        if !confirm {
            theme.warn("Aborted.");
            return Ok(());
        }
    }
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
            theme.step(
                "✎",
                &format!("Rewriting {} path references", result.total_changes()),
            );
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
            if !src.exists() {
                bail!("local installer not found: {}", path);
            }
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
            // For system packages, attempt to re-resolve the binary path.
            // The actual package update must be done by the user via
            // their package manager (warren stays rootless).
            theme.warn(&format!(
                "Package source '{}' — warren cannot update system packages. \
                 Update with your package manager (e.g., `sudo apt upgrade {}`), \
                 then run `warren update {}` to refresh the launcher.",
                name, name, alias
            ));
            return Ok(());
        }
        // Wrap mode: nothing is installed, so "update" only re-resolves
        // the host app (picks up a new binary location / desktop entry)
        // and rewrites the launcher + desktop file. Account data is never
        // touched — and the *host* app is never modified either: run
        // `flatpak update` / your package manager yourself if you want a
        // newer host binary, then `warren update` to re-resolve it.
        SourceInfo::Flatpak { .. }
        | SourceInfo::Snap { .. }
        | SourceInfo::System { .. }
        | SourceInfo::App { .. }
        | SourceInfo::Desktop { .. } => {
            let gui_override = Some(metadata.instance.gui);
            match crate::app::resolve::resolve(&source_info, gui_override) {
                Ok(spec) => {
                    metadata.instance.app_name = spec.display_name.clone();
                    metadata.instance.gui = spec.gui;
                    metadata.instance.icon = spec.icon.clone();
                    metadata.install.launch_command = spec.command.clone();
                    let content = Launcher::generate_wrapped(alias, &layout, &spec);
                    Launcher::write(&layout, &content)?;
                    theme.step("🧷", &format!("Re-resolved: {}", spec.command.join(" ")));
                }
                Err(e) => {
                    theme.warn(&format!(
                        "Host app no longer resolves ({}). Launcher left as-is.",
                        e
                    ));
                    return Ok(());
                }
            }
        }
    }
    // Regenerate the launcher the same way `dig` built it so wrapper
    // improvements apply to older instances.
    if metadata.install.launch_command.is_empty() {
        let binary_name = metadata.instance.app_name.clone();
        metadata.instance.version = crate::install::detect::detect_version(&layout, &binary_name);
        let content = Launcher::generate_file(alias, &layout, &binary_name, metadata.instance.gui);
        Launcher::write(&layout, &content)?;
    } else {
        let spec = crate::app::LaunchSpec {
            display_name: metadata.instance.app_name.clone(),
            command: metadata.install.launch_command.clone(),
            gui: metadata.instance.gui,
            icon: metadata.instance.icon.clone(),
            desktop_id: None,
        };
        let content = Launcher::generate_wrapped(alias, &layout, &spec);
        Launcher::write(&layout, &content)?;
    }
    Launcher::install(&layout, &config.paths.bin_dir, alias)?;
    if metadata.instance.gui {
        let launcher_dest = config.paths.bin_dir.join(alias);
        let path = crate::app::desktop::install(
            alias,
            &metadata.instance.app_name,
            &launcher_dest,
            metadata.instance.icon.as_deref(),
            false,
        )?;
        metadata.paths.desktop_file = Some(path.to_string_lossy().to_string());
    }
    metadata.instance.updated_at = Utc::now();
    metadata.save(&layout.metadata_path())?;
    theme.success(&format!("Updated: {}", alias));
    Ok(())
}
