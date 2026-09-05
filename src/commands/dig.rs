use anyhow::{Context, Result, bail};
use chrono::Utc;

use crate::app::sources::{SourceInfo, is_wrap_source, parse_source, source_kind};
use crate::config::WarrenConfig;
use crate::install::{InstallerExecutor, InstallerRewriter};
use crate::instance::metadata::{InstallInfo, InstanceInfo, PathsInfo, SourceType};
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::progress;
use crate::ui::theme::Theme;

pub async fn execute(
    config: &WarrenConfig,
    theme: &Theme,
    source: &str,
    alias: &str,
    yes: bool,
    gui_override: Option<bool>,
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
        crate::app::desktop::uninstall(alias).ok();
        layout.destroy()?;
    }

    theme.header(&format!("digging {}", alias));
    layout.create()?;

    let source_info = parse_source(source);
    if is_wrap_source(&source_info) {
        return execute_wrap(
            config,
            theme,
            &layout,
            source,
            &source_info,
            alias,
            gui_override,
        )
        .await;
    }

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
        // Wrap sources are handled above; this arm is unreachable.
        _ => {
            layout.destroy()?;
            bail!("unsupported source '{}'", source);
        }
    };

    if matches!(
        source_info,
        SourceInfo::RemoteScript { .. } | SourceInfo::LocalScript { .. }
    ) {
        let original_path = layout.installers_dir().join("original.sh");
        let original_content =
            std::fs::read_to_string(&original_path).context("failed to read original installer")?;
        let rewriter = InstallerRewriter::new(&layout.root);
        let result = rewriter.rewrite(&original_content);
        theme.step(
            "✎",
            &format!(
                "Rewriting {} path reference{}",
                result.total_changes(),
                if result.total_changes() == 1 { "" } else { "s" }
            ),
        );
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
                let diff =
                    crate::install::rewriter::generate_diff(&original_content, &result.content);
                eprintln!("\n{}\n", diff);
            }
        }
        let spinner = progress::spinner("Running installer...");
        InstallerExecutor::execute(&rewritten_path, &layout).await?;
        spinner.finish_with_message("Installer complete");
    }

    let binary_name = crate::install::detect::detect_binary(&layout.bin_dir())?
        .unwrap_or_else(|| {
            theme.warn("No executable found in instance bin/ — the installer may have failed to place one.");
            theme.warn(&format!(
                "Launcher will point to '{}', which may not exist yet. Inspect with: warren inspect {}",
                alias, alias
            ));
            alias.to_string()
        });
    let version = crate::install::detect::detect_version(&layout, &binary_name);
    let gui = gui_override.unwrap_or(false);
    let launcher_content = Launcher::generate_file(alias, &layout, &binary_name, gui);
    Launcher::write(&layout, &launcher_content)?;
    let launcher_dest = Launcher::install(&layout, &config.paths.bin_dir, alias)?;

    let desktop_file = if gui {
        let path = crate::app::desktop::install(alias, &binary_name, &launcher_dest, None, false)?;
        theme.success(&format!(
            "Desktop entry: {}",
            InstanceMetadata::display_path(&path.to_string_lossy())
        ));
        Some(path.to_string_lossy().to_string())
    } else {
        None
    };

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
            gui,
            icon: None,
        },
        install: InstallInfo {
            source: source.to_string(),
            source_type,
            installer_hash,
            launch_command: Vec::new(),
        },
        paths: PathsInfo {
            root: layout.root.to_string_lossy().to_string(),
            bin: layout.bin_dir().to_string_lossy().to_string(),
            launcher: launcher_dest.to_string_lossy().to_string(),
            desktop_file,
        },
    };
    metadata.save(&layout.metadata_path())?;

    theme.blank();
    theme.success(&format!("Installed: {}", alias));
    theme.success(&format!(
        "Launcher: {}",
        InstanceMetadata::display_path(&launcher_dest.to_string_lossy())
    ));
    theme.success(&format!(
        "Binary:   {}",
        InstanceMetadata::display_path(&layout.bin_dir().join(&binary_name).to_string_lossy())
    ));
    theme.blank();
    theme.dim(&format!("Run it:  {}", alias));
    theme.dim(&format!("         warren run {}", alias));
    theme.blank();

    Ok(())
}

/// Wrap mode: resolve the host app and generate an isolated launcher.
/// Nothing is downloaded, copied, or installed — the host application is
/// reused, so snapshots stay tiny and `dig` works for any package manager.
async fn execute_wrap(
    config: &WarrenConfig,
    theme: &Theme,
    layout: &InstanceLayout,
    source: &str,
    source_info: &SourceInfo,
    alias: &str,
    gui_override: Option<bool>,
) -> Result<()> {
    theme.step(
        "🔗",
        &format!("Wrapping {} source: {}", source_kind(source_info), source),
    );
    let spec = crate::app::resolve::resolve(source_info, gui_override).inspect_err(|_| {
        layout.destroy().ok();
    })?;
    theme.step(
        "🧷",
        &format!("Found: {} → {}", spec.display_name, spec.command.join(" ")),
    );

    let source_type = match source_info {
        SourceInfo::Flatpak { .. } => SourceType::Flatpak,
        SourceInfo::Snap { .. } => SourceType::Snap,
        SourceInfo::System { .. } => SourceType::System,
        SourceInfo::App { .. } => SourceType::App,
        SourceInfo::Desktop { .. } => SourceType::Desktop,
        _ => SourceType::Package,
    };

    let launcher_content = Launcher::generate_wrapped(alias, layout, &spec);
    Launcher::write(layout, &launcher_content)?;
    let launcher_dest = Launcher::install(layout, &config.paths.bin_dir, alias)?;

    let desktop_file = if spec.gui {
        let path = crate::app::desktop::install(
            alias,
            &spec.display_name,
            &launcher_dest,
            spec.icon.as_deref(),
            false,
        )?;
        theme.success(&format!(
            "Desktop entry: {}",
            InstanceMetadata::display_path(&path.to_string_lossy())
        ));
        Some(path.to_string_lossy().to_string())
    } else {
        None
    };

    let now = Utc::now();
    let shell = crate::shell::detect_shell();
    let metadata = InstanceMetadata {
        instance: InstanceInfo {
            alias: alias.to_string(),
            app_name: spec.display_name.clone(),
            version: None,
            shell: shell.to_string(),
            created_at: now,
            updated_at: now,
            gui: spec.gui,
            icon: spec.icon.clone(),
        },
        install: InstallInfo {
            source: source.to_string(),
            source_type,
            installer_hash: None,
            launch_command: spec.command.clone(),
        },
        paths: PathsInfo {
            root: layout.root.to_string_lossy().to_string(),
            bin: layout.bin_dir().to_string_lossy().to_string(),
            launcher: launcher_dest.to_string_lossy().to_string(),
            desktop_file,
        },
    };
    metadata.save(&layout.metadata_path())?;

    theme.blank();
    theme.success(&format!(
        "Wrapped: {} ({} account #1 of many)",
        alias, spec.display_name
    ));
    theme.success(&format!(
        "Launcher: {}",
        InstanceMetadata::display_path(&launcher_dest.to_string_lossy())
    ));
    theme.blank();
    if spec.gui {
        theme.dim(&format!(
            "Run it from your app grid, or with:  warren run {}",
            alias
        ));
        theme.dim("Tip: each alias keeps its own login — dig again with a");
        theme.dim(&format!(
            "different --as name for account #2:  warren dig {} --as <other>",
            source
        ));
    } else {
        theme.dim(&format!("Run it:  {}", alias));
        theme.dim(&format!("         warren run {}", alias));
    }
    theme.blank();
    Ok(())
}
