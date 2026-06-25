use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct InstanceLayout {
    pub root: PathBuf,
    pub alias: String,
}

impl InstanceLayout {
    pub fn new(instances_dir: &Path, alias: &str) -> Self {
        let root = instances_dir.join(alias);
        Self { root, alias: alias.to_string() }
    }

    pub fn bin_dir(&self) -> PathBuf { self.root.join("bin") }
    pub fn home_dir(&self) -> PathBuf { self.root.join("home") }
    pub fn config_dir(&self) -> PathBuf { self.root.join("config") }
    pub fn cache_dir(&self) -> PathBuf { self.root.join("cache") }
    pub fn data_dir(&self) -> PathBuf { self.root.join("data") }
    pub fn state_dir(&self) -> PathBuf { self.root.join("state") }
    pub fn runtime_dir(&self) -> PathBuf { self.root.join("runtime") }
    pub fn tmp_dir(&self) -> PathBuf { self.root.join("tmp") }
    pub fn logs_dir(&self) -> PathBuf { self.root.join("logs") }
    pub fn installers_dir(&self) -> PathBuf { self.root.join("installers") }
    pub fn metadata_path(&self) -> PathBuf { self.root.join("metadata.toml") }
    pub fn launcher_path(&self) -> PathBuf { self.root.join("launcher") }

    pub fn exists(&self) -> bool { self.root.exists() }

    pub fn create(&self) -> Result<()> {
        let dirs = [
            self.bin_dir(), self.home_dir(), self.config_dir(),
            self.cache_dir(), self.data_dir(), self.state_dir(),
            self.runtime_dir(), self.tmp_dir(), self.logs_dir(),
            self.installers_dir(),
        ];
        for dir in &dirs {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("failed to create directory {}", dir.display()))?;
        }
        tracing::debug!(alias = %self.alias, root = %self.root.display(), "created instance layout");
        Ok(())
    }

    pub fn destroy(&self) -> Result<()> {
        if self.root.exists() {
            std::fs::remove_dir_all(&self.root)
                .with_context(|| format!("failed to remove instance directory {}", self.root.display()))?;
        }
        Ok(())
    }

    pub fn validate_path_containment(&self, path: &Path) -> Result<()> {
        let canonical_root = self.root.canonicalize()
            .with_context(|| format!("failed to canonicalize root {}", self.root.display()))?;
        let canonical_path = path.canonicalize()
            .with_context(|| format!("failed to canonicalize path {}", path.display()))?;
        if !canonical_path.starts_with(&canonical_root) {
            bail!("path {} is outside instance root {}", canonical_path.display(), canonical_root.display());
        }
        Ok(())
    }

    pub fn disk_usage(&self) -> Result<DiskUsage> {
        Ok(DiskUsage {
            bin: dir_size(&self.bin_dir()),
            config: dir_size(&self.config_dir()),
            cache: dir_size(&self.cache_dir()),
            data: dir_size(&self.data_dir()),
            state: dir_size(&self.state_dir()),
            home: dir_size(&self.home_dir()),
            tmp: dir_size(&self.tmp_dir()),
            logs: dir_size(&self.logs_dir()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub bin: u64,
    pub config: u64,
    pub cache: u64,
    pub data: u64,
    pub state: u64,
    pub home: u64,
    pub tmp: u64,
    pub logs: u64,
}

impl DiskUsage {
    pub fn total(&self) -> u64 {
        self.bin + self.config + self.cache + self.data + self.state + self.home + self.tmp + self.logs
    }
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() { return 0; }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
