use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Global warren configuration from ~/.warren/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WarrenConfig {
    #[serde(default = "PathsConfig::default")]
    pub paths: PathsConfig,
    #[serde(default = "DefaultsConfig::default")]
    pub defaults: DefaultsConfig,
    #[serde(default = "UiConfig::default")]
    pub ui: UiConfig,
    #[serde(default = "SessionsConfig::default")]
    pub sessions: SessionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_instances_dir")]
    pub instances_dir: PathBuf,
    #[serde(default = "default_bin_dir")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsConfig {
    /// How many session snapshots to keep. Older ones are deleted on save/prune.
    #[serde(default = "default_session_keep")]
    pub keep: usize,
}

fn default_true() -> bool {
    true
}

fn default_session_keep() -> usize {
    5
}

fn default_instances_dir() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".warren")
        .join("instances")
}

fn default_bin_dir() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local")
        .join("bin")
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            instances_dir: default_instances_dir(),
            bin_dir: default_bin_dir(),
        }
    }
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            shell: String::new(),
            confirm: true,
            show_diff: false,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            color: true,
            progress: true,
        }
    }
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            keep: default_session_keep(),
        }
    }
}

impl WarrenConfig {
    pub fn config_path() -> PathBuf {
        Self::warren_dir_raw().join("config.toml")
    }

    pub fn sessions_dir() -> PathBuf {
        Self::warren_dir_raw().join("sessions")
    }

    /// Non-Result version for internal use where we have a reasonable fallback.
    pub(crate) fn warren_dir_raw() -> PathBuf {
        dirs::home_dir()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".warren")
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

    /// Validate that a real home directory exists. Warren needs ~/.warren
    /// to store instances, so refusing early gives a clear error.
    pub fn validate_home() -> Result<()> {
        if dirs::home_dir().is_none() && std::env::var_os("HOME").is_none() {
            bail!("could not determine home directory. Set the HOME environment variable.");
        }
        Ok(())
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.paths.instances_dir).with_context(|| {
            format!(
                "failed to create instances directory {}",
                self.paths.instances_dir.display()
            )
        })?;
        std::fs::create_dir_all(&self.paths.bin_dir).with_context(|| {
            format!(
                "failed to create bin directory {}",
                self.paths.bin_dir.display()
            )
        })?;
        std::fs::create_dir_all(Self::sessions_dir()).with_context(|| {
            format!(
                "failed to create sessions directory {}",
                Self::sessions_dir().display()
            )
        })?;
        Ok(())
    }
}
