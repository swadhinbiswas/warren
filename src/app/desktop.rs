//! Per-instance `.desktop` entries so wrapped GUI apps appear in the
//! app grid / dock under their warren alias (`discord-work`).
//!
//! Files live at `~/.local/share/applications/warren-<alias>.desktop`
//! (user scope — no root needed) and `Exec` the warren launcher in
//! `~/.local/bin`, so launching from the desktop runs the isolated
//! instance, not the shared host app.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Directory for user `.desktop` files.
pub fn applications_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("WARREN_APPLICATIONS_DIR").map(PathBuf::from) {
        return dir;
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
        .expect("could not determine data directory");
    base.join("applications")
}

pub fn desktop_path(alias: &str) -> PathBuf {
    applications_dir().join(format!("warren-{alias}.desktop"))
}

/// Write (or overwrite) the `.desktop` entry for a GUI instance.
pub fn install(
    alias: &str,
    display_name: &str,
    launcher_path: &Path,
    icon: Option<&str>,
    terminal: bool,
) -> Result<PathBuf> {
    let dir = applications_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create applications directory {}", dir.display()))?;
    let path = desktop_path(alias);
    let exec = format!("{} %U", launcher_path.to_string_lossy());
    let mut content = format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName={} ({})\nComment=Warren isolated instance of {}\nExec={}\n",
        display_name, alias, display_name, exec
    );
    if let Some(icon) = icon.filter(|i| !i.is_empty()) {
        content.push_str(&format!("Icon={icon}\n"));
    }
    content.push_str(&format!(
        "Terminal={}\nCategories=Warren;\nStartupNotify=true\nStartupWMClass=warren-{alias}\nX-Warren-Alias={alias}\nX-Warren-Version={}\n",
        if terminal { "true" } else { "false" },
        env!("CARGO_PKG_VERSION"),
    ));
    std::fs::write(&path, content)
        .with_context(|| format!("failed to write desktop entry {}", path.display()))?;
    refresh_database(&dir);
    Ok(path)
}

/// Remove the `.desktop` entry (best-effort: missing file is fine).
pub fn uninstall(alias: &str) -> Result<()> {
    let path = desktop_path(alias);
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove desktop entry {}", path.display()))?;
        if let Some(dir) = path.parent() {
            refresh_database(dir);
        }
    }
    Ok(())
}

fn refresh_database(dir: &Path) {
    // Optional: failures are harmless (the desktop re-scans on login).
    let _ = std::process::Command::new("update-desktop-database")
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_expected_entry() {
        let dir = std::env::temp_dir().join(format!(
            "warren-desktop-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        unsafe { std::env::set_var("WARREN_APPLICATIONS_DIR", &dir) };
        let launcher = PathBuf::from("/home/test/.local/bin/discord-work");
        let path = install("discord-work", "discord", &launcher, Some("discord"), false).unwrap();
        assert!(path.ends_with("warren-discord-work.desktop"));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Name=discord (discord-work)"));
        assert!(content.contains("Exec=/home/test/.local/bin/discord-work %U"));
        assert!(content.contains("Terminal=false"));
        assert!(content.contains("Icon=discord"));
        assert!(content.contains("X-Warren-Alias=discord-work"));
        uninstall("discord-work").unwrap();
        assert!(!path.exists());
        unsafe { std::env::remove_var("WARREN_APPLICATIONS_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
