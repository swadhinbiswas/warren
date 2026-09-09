use anyhow::{Context, Result, bail};
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::instance::metadata::{InstallInfo, InstanceInfo, PathsInfo};
use crate::instance::{self, InstanceLayout, InstanceMetadata, Launcher};
use crate::ui::theme::Theme;

/// Clone an instance into a new, independent app.
///
/// A clone is a **new identity, not a copy of your logins**: only the
/// installed program (`bin/`) and the installer scripts (`installers/`,
/// so `warren update` keeps working) are carried over. Every storage
/// directory — `home/`, `config/`, `cache/`, `data/`, `state/`, `tmp/`,
/// `runtime/`, `logs/` — starts empty, so the new alias boots exactly
/// like a first-time install: no old accounts, no stale sessions.
///
/// Pass `--copy-data` to opt back into a full deep copy (useful when
/// forking a logged-in profile on purpose).
pub async fn execute(
    config: &WarrenConfig,
    theme: &Theme,
    source: &str,
    dest: &str,
    copy_data: bool,
) -> Result<()> {
    instance::validate_alias(dest)?;
    let source_layout = InstanceLayout::new(&config.paths.instances_dir, source);
    if !source_layout.exists() {
        bail!("source instance '{}' not found", source);
    }
    // Load first so a corrupt source fails before anything is created.
    let source_metadata = InstanceMetadata::load(&source_layout.metadata_path())
        .with_context(|| format!("failed to load metadata for instance '{}'", source))?;
    let dest_layout = InstanceLayout::new(&config.paths.instances_dir, dest);
    if dest_layout.exists() {
        bail!("destination instance '{}' already exists", dest);
    }

    theme.header(&format!("cloning {} → {}", source, dest));
    // Fresh, empty storage for the new identity.
    dest_layout.create()?;

    if copy_data {
        copy_dir_recursive(&source_layout.root, &dest_layout.root, true)
            .context("failed to copy instance directory")?;
        theme.step("⎘", "copying program and user data (--copy-data)");
    } else {
        // Program + installer scripts only. Everything else (logins,
        // configs, caches, sessions, logs) stays behind with the source.
        for dir in ["bin", "installers"] {
            let src = source_layout.root.join(dir);
            if src.exists() {
                copy_dir_recursive(&src, &dest_layout.root.join(dir), false)
                    .with_context(|| format!("failed to copy {} directory", dir))?;
            }
        }
        theme.step("✦", "fresh storage — logins and data start empty");
    }

    let now = Utc::now();
    let shell = crate::shell::detect_shell();
    let mut metadata = InstanceMetadata {
        instance: InstanceInfo {
            alias: dest.to_string(),
            app_name: source_metadata.instance.app_name.clone(),
            version: source_metadata.instance.version.clone(),
            shell: shell.to_string(),
            created_at: now,
            updated_at: now,
            gui: source_metadata.instance.gui,
            icon: source_metadata.instance.icon.clone(),
        },
        install: InstallInfo {
            source: source_metadata.install.source.clone(),
            source_type: source_metadata.install.source_type.clone(),
            installer_hash: source_metadata.install.installer_hash.clone(),
            launch_command: source_metadata.install.launch_command.clone(),
        },
        paths: PathsInfo {
            root: dest_layout.root.to_string_lossy().to_string(),
            bin: dest_layout.bin_dir().to_string_lossy().to_string(),
            launcher: String::new(), // filled in below
            desktop_file: None,
        },
    };
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
    // A clone is a new identity: always (re)create the desktop entry for
    // the new alias when the source was graphical.
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
    if !copy_data {
        theme.dim(" Fresh clone: sign in again — nothing was carried over.");
    }
    theme.blank();
    Ok(())
}

/// Copy a directory tree, preserving symlinks, permissions and timestamps.
///
/// When `is_root` is true the destination root itself is created and every
/// entry is copied; otherwise `src` is a single subdirectory (e.g. `bin`)
/// copied into the already-created `dst`.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path, is_root: bool) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        // When cloning the whole instance root, the launcher symlink in
        // ~/.local/bin points back at the source instance — never copy it.
        // (The launcher is regenerated for the new alias below anyway.)
        if is_root && entry.file_name().to_string_lossy() == "launcher" {
            continue;
        }
        let dst_path = dst.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&src_path)
            .with_context(|| format!("failed to read metadata for {}", src_path.display()))?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            let target = std::fs::read_link(&src_path)
                .with_context(|| format!("failed to read symlink {}", src_path.display()))?;
            std::os::unix::fs::symlink(&target, &dst_path)
                .with_context(|| format!("failed to create symlink at {}", dst_path.display()))?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path, false)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
            // Preserve file permissions and timestamps.
            let perms = metadata.permissions();
            std::fs::set_permissions(&dst_path, perms)
                .with_context(|| format!("failed to set permissions on {}", dst_path.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let atime = metadata.atime();
                let mtime = metadata.mtime();
                let _ = filetime::set_file_atime(
                    &dst_path,
                    filetime::FileTime::from_unix_time(atime, 0),
                );
                let _ = filetime::set_file_mtime(
                    &dst_path,
                    filetime::FileTime::from_unix_time(mtime, 0),
                );
            }
        } else {
            bail!(
                "cannot clone: unsupported file type at {}",
                src_path.display()
            );
        }
    }
    Ok(())
}
