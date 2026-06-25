use std::path::PathBuf;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Global warren configuration from ~/.warren/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarrenConfig {
    #[serde(default = "PathsConfig::default")]
    pub paths: PathsConfig,
    #[serde(default = "DefaultsConfig::default")]
    pub defaults: DefaultsConfig,
    #[serde(default = "UiConfig::default")]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub instances_dir: PathBuf,
    pub bin_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub shell: String,
    #[serde(default = "default_true")]
    pub confirm: bool,
    #[serde(default)]
    pub show_diff: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_true")]
    pub color: bool,
    #[serde(default = "default_true")]
    pub progress: bool,
}

fn default_true() -> bool { true }

impl Default for PathsConfig {
    fn default() -> Self {
        let home = dirs::home_dir().expect("could not determine home directory");
        Self {
            instances_dir: home.join(".warren").join("instances"),
            bin_dir: home.join(".local").join("bin"),
        }
    }
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self { shell: String::new(), confirm: true, show_diff: false }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { color: true, progress: true }
    }
}

impl Default for WarrenConfig {
    fn default() -> Self {
        Self {
            paths: PathsConfig::default(),
            defaults: DefaultsConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl WarrenConfig {
    pub fn warren_dir() -> PathBuf {
        dirs::home_dir()
            .expect("could not determine home directory")
            .join(".warren")
    }

    pub fn config_path() -> PathBuf {
        Self::warren_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read config from {}", path.display()))?;
            let config: WarrenConfig = toml::from_str(&content)
                .with_context(|| format!("failed to parse config from {}", path.display()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config directory {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        std::fs::write(&path, content)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.paths.instances_dir)
            .with_context(|| format!("failed to create instances directory {}", self.paths.instances_dir.display()))?;
        std::fs::create_dir_all(&self.paths.bin_dir)
            .with_context(|| format!("failed to create bin directory {}", self.paths.bin_dir.display()))?;
        Ok(())
    }
}
