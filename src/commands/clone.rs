use anyhow::{Context, Result, bail};
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, source: &str, dest: &str) -> Result<()> {
    instance::validate_alias(dest)?;
    let source_layout = InstanceLayout::new(&config.paths.instances_dir, source);
    if !source_layout.exists() {
        bail!("source instance '{}' not found", source);
    }
    let dest_layout = InstanceLayout::new(&config.paths.instances_dir, dest);
    if dest_layout.exists() {
        bail!("destination instance '{}' already exists", dest);
    }
    theme.header(&format!("cloning {} → {}", source, dest));
    copy_dir_recursive(&source_layout.root, &dest_layout.root)
        .context("failed to copy instance directory")?;
    let mut metadata = InstanceMetadata::load(&dest_layout.metadata_path())?;
    metadata.instance.alias = dest.to_string();
    metadata.instance.created_at = Utc::now();
    metadata.instance.updated_at = Utc::now();
    metadata.paths.root = dest_layout.root.to_string_lossy().to_string();
    metadata.paths.bin = dest_layout.bin_dir().to_string_lossy().to_string();
    let launcher_content = if metadata.install.launch_command.is_empty() {
        Launcher::generate_file(
            dest,
            &dest_layout,
            &metadata.instance.app_name,
            metadata.instance.gui,
        )
    } else {
        Launcher::generate_wrapped(
            dest,
            &dest_layout,
            &crate::app::LaunchSpec {
                display_name: metadata.instance.app_name.clone(),
                command: metadata.install.launch_command.clone(),
                gui: metadata.instance.gui,
                icon: metadata.instance.icon.clone(),
                desktop_id: None,
            },
        )
    };
    Launcher::write(&dest_layout, &launcher_content)?;
    let launcher_dest = Launcher::install(&dest_layout, &config.paths.bin_dir, dest)?;
    metadata.paths.launcher = launcher_dest.to_string_lossy().to_string();
    // A clone is a new identity: drop the old desktop entry path and
    // re-create it for the new alias when the source was graphical.
    metadata.paths.desktop_file = None;
    if metadata.instance.gui {
        let path = crate::app::desktop::install(
            dest,
            &metadata.instance.app_name,
            &launcher_dest,
            metadata.instance.icon.as_deref(),
            false,
        )?;
        metadata.paths.desktop_file = Some(path.to_string_lossy().to_string());
    }
    metadata.save(&dest_layout.metadata_path())?;
    theme.success(&format!("Cloned: {} → {}", source, dest));
    theme.success(&format!(
        "Launcher: {}",
        InstanceMetadata::display_path(&launcher_dest.to_string_lossy())
    ));
    theme.blank();
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = std::fs::symlink_metadata(&src_path)
            .with_context(|| format!("failed to read metadata for {}", src_path.display()))?
            .file_type();
        if file_type.is_symlink() {
            let target = std::fs::read_link(&src_path)
                .with_context(|| format!("failed to read symlink {}", src_path.display()))?;
            std::os::unix::fs::symlink(&target, &dst_path)
                .with_context(|| format!("failed to create symlink at {}", dst_path.display()))?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        } else {
            bail!(
                "cannot clone: unsupported file type at {}",
                src_path.display()
            );
        }
    }
    Ok(())
}
