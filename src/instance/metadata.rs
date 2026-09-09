use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetadata {
    pub instance: InstanceInfo,
    pub install: InstallInfo,
    pub paths: PathsInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub alias: String,
    pub app_name: String,
    pub version: Option<String>,
    pub shell: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Graphical app: gets a `.desktop` entry and detached `warren run`.
    #[serde(default)]
    pub gui: bool,
    /// Icon name/path for the `.desktop` entry (wrapped apps only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallInfo {
    pub source: String,
    pub source_type: SourceType,
    pub installer_hash: Option<String>,
    /// Host command for wrapped apps (flatpak / system / desktop).
    /// Empty for script installs and legacy instances.
    #[serde(default)]
    pub launch_command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    RemoteScript,
    LocalScript,
    Package,
    Flatpak,
    Snap,
    System,
    App,
    Desktop,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::RemoteScript => write!(f, "remote_script"),
            SourceType::LocalScript => write!(f, "local_script"),
            SourceType::Package => write!(f, "package"),
            SourceType::Flatpak => write!(f, "flatpak"),
            SourceType::Snap => write!(f, "snap"),
            SourceType::System => write!(f, "system"),
            SourceType::App => write!(f, "app"),
            SourceType::Desktop => write!(f, "desktop"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsInfo {
    pub root: String,
    pub bin: String,
    pub launcher: String,
    /// Absolute path of the `warren-<alias>.desktop` entry, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_file: Option<String>,
}

impl InstanceMetadata {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read metadata from {}", path.display()))?;
        let metadata: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse metadata from {}", path.display()))?;
        Ok(metadata)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize metadata")?;
        std::fs::write(path, content)
            .with_context(|| format!("failed to write metadata to {}", path.display()))?;
        Ok(())
    }

    pub fn display_path(path: &str) -> String {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy();
            if path.starts_with(home_str.as_ref()) {
                return path.replacen(home_str.as_ref(), "~", 1);
            }
        }
        path.to_string()
    }
}
