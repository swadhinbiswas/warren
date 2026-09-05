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
    // When the archive is renamed to a different alias, the unpack
    // landing spot (the archive's internal root) must not clobber an
    // existing instance either.
    if alias != archive_alias && config.paths.instances_dir.join(&archive_alias).exists() {
        bail!(
            "archive root '{}' conflicts with an existing instance; remove it first",
            archive_alias
        );
    }

    // All entries were validated above; now extract.
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open archive {}", path.display()))?;
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive
        .unpack(&config.paths.instances_dir)
        .with_context(|| "failed to extract archive")?;

    if alias != archive_alias {
        let src = config.paths.instances_dir.join(&archive_alias);
        let dst = target_dir.clone();
        std::fs::rename(&src, &dst)
            .with_context(|| format!("failed to rename {} to {}", src.display(), dst.display()))?;
    }

    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    let mut metadata = InstanceMetadata::load(&layout.metadata_path())?;
    if alias != metadata.instance.alias {
        metadata.instance.alias = alias.to_string();
        metadata.paths.root = layout.root.to_string_lossy().to_string();
        metadata.paths.bin = layout.bin_dir().to_string_lossy().to_string();
    }
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
