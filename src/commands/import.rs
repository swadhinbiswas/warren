use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use std::path::{Component, Path};

use crate::config::WarrenConfig;
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::theme::Theme;

pub async fn execute(
    config: &WarrenConfig,
    theme: &Theme,
    path: &Path,
    alias_override: Option<&str>,
    fresh: bool,
) -> Result<()> {
    if !path.exists() {
        bail!("archive not found: {}", path.display());
    }
    theme.step("📦", &format!("Importing from {}", path.display()));

    // Inspect the archive before touching disk: determine the top-level
    // directory and reject any path traversal or mixed-root archives.
    let archive_alias = inspect_archive(path)?;

    let alias = alias_override.unwrap_or(&archive_alias);
    instance::validate_alias(alias)?;

    let target_dir = config.paths.instances_dir.join(alias);
    if target_dir.exists() {
        bail!("instance '{}' already exists", alias);
    }
    // Extraction goes to a unique temp dir and is renamed into place, so
    // the archive's internal root name can never clobber an existing
    // instance — even when `--as` renames it. No extra guard needed.

    // All entries were validated above; now extract to a temporary directory
    // first, then atomically rename to the final location. This prevents
    // partial extractions from polluting the instances directory.
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open archive {}", path.display()))?;
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    // Create a temporary extraction directory
    let temp_dir = config
        .paths
        .instances_dir
        .join(format!(".import-tmp-{}", std::process::id()));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)
            .with_context(|| format!("failed to clean up temp dir {}", temp_dir.display()))?;
    }
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("failed to create temp dir {}", temp_dir.display()))?;

    archive
        .unpack(&temp_dir)
        .with_context(|| "failed to extract archive")?;

    // Move the extracted directory to its final location
    let src = temp_dir.join(&archive_alias);
    if !src.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
        bail!(
            "archive did not contain expected root directory '{}'",
            archive_alias
        );
    }
    std::fs::rename(&src, &target_dir).with_context(|| {
        format!(
            "failed to move {} to {}",
            src.display(),
            target_dir.display()
        )
    })?;

    // Clean up temp directory
    std::fs::remove_dir_all(&temp_dir).ok();

    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if fresh {
        // Fresh import: wipe every storage directory so the imported app
        // boots like a first-time install. Program (`bin/`) and installer
        // scripts (`installers/`) are kept; logins, configs, caches,
        // sessions and logs are emptied.
        for dir in [
            layout.home_dir(),
            layout.config_dir(),
            layout.cache_dir(),
            layout.data_dir(),
            layout.state_dir(),
            layout.tmp_dir(),
            layout.runtime_dir(),
            layout.logs_dir(),
        ] {
            if dir.exists() {
                std::fs::remove_dir_all(&dir).with_context(|| {
                    format!("failed to clear {} on fresh import", dir.display())
                })?;
            }
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("failed to recreate {} on fresh import", dir.display()))?;
        }
        theme.step("✦", "fresh import — logins and data start empty");
    }
    let mut metadata = InstanceMetadata::load(&layout.metadata_path())?;
    if alias != metadata.instance.alias {
        metadata.instance.alias = alias.to_string();
        metadata.paths.root = layout.root.to_string_lossy().to_string();
        metadata.paths.bin = layout.bin_dir().to_string_lossy().to_string();
    }
    // Ensure the metadata alias matches the actual directory name
    metadata.instance.alias = alias.to_string();
    let launcher_content = if metadata.install.launch_command.is_empty() {
        Launcher::generate_file(
            alias,
            &layout,
            &metadata.instance.app_name,
            metadata.instance.gui,
        )
    } else {
        Launcher::generate_wrapped(
            alias,
            &layout,
            &crate::app::LaunchSpec {
                display_name: metadata.instance.app_name.clone(),
                command: metadata.install.launch_command.clone(),
                gui: metadata.instance.gui,
                icon: metadata.instance.icon.clone(),
                desktop_id: None,
            },
        )
    };
    Launcher::write(&layout, &launcher_content)?;
    let launcher_dest = Launcher::install(&layout, &config.paths.bin_dir, alias)?;
    metadata.paths.launcher = launcher_dest.to_string_lossy().to_string();
    // Desktop entries contain absolute Exec paths — always regenerate.
    metadata.paths.desktop_file = None;
    if metadata.instance.gui {
        let path = crate::app::desktop::install(
            alias,
            &metadata.instance.app_name,
            &launcher_dest,
            metadata.instance.icon.as_deref(),
            false,
        )?;
        metadata.paths.desktop_file = Some(path.to_string_lossy().to_string());
    }
    metadata.save(&layout.metadata_path())?;
    theme.success(&format!("Imported: {}", alias));
    theme.success(&format!(
        "Launcher: {}",
        InstanceMetadata::display_path(&launcher_dest.to_string_lossy())
    ));
    theme.blank();
    Ok(())
}

/// Validate every entry of a warren export archive and return the name of
/// its single top-level directory.
fn inspect_archive(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open archive {}", path.display()))?;
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    let mut top: Option<String> = None;
    for entry in archive
        .entries()
        .with_context(|| "failed to read archive entries")?
    {
        let entry = entry.with_context(|| "failed to read archive entry")?;
        let entry_path = entry
            .path()
            .with_context(|| "failed to read archive entry path")?;

        let mut components = entry_path.components();
        if matches!(components.clone().next(), Some(Component::CurDir)) {
            components.next();
        }
        match components.next() {
            Some(Component::Normal(name)) => {
                let name = name.to_string_lossy().to_string();
                if top.is_none() && !entry.header().entry_type().is_dir() {
                    bail!("archive root '{}' is not a directory", name);
                }
                match &top {
                    None => top = Some(name),
                    Some(t) if *t != name => {
                        bail!(
                            "archive contains multiple top-level directories ('{}' and '{}')",
                            t,
                            name
                        );
                    }
                    _ => {}
                }
            }
            Some(Component::ParentDir) => {
                bail!("archive contains path traversal ('..') and was rejected");
            }
            Some(Component::RootDir) | Some(Component::Prefix(_)) => {
                bail!("archive contains an absolute or prefixed path and was rejected");
            }
            Some(Component::CurDir) | None => continue,
        }
        for component in components {
            match component {
                Component::ParentDir => {
                    bail!("archive contains path traversal ('..') and was rejected");
                }
                Component::RootDir | Component::Prefix(_) => {
                    bail!("archive contains an absolute or prefixed path and was rejected");
                }
                Component::CurDir | Component::Normal(_) => {}
            }
        }
    }
    Ok(top.unwrap_or_default())
}
