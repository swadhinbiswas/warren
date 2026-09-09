use anyhow::{Context, Result, bail};
use std::fs::{File, OpenOptions};
use std::time::Duration;

/// A simple file-based lock to prevent concurrent warren operations
/// on the same instance from corrupting data.
pub struct InstanceLock {
    lock_path: std::path::PathBuf,
    _file: File,
}

impl InstanceLock {
    /// Acquire a lock for the given instance alias.
    /// Returns Err if the lock is already held by another process.
    pub fn acquire(instances_dir: &std::path::Path, alias: &str) -> Result<Self> {
        let lock_path = instances_dir.join(format!(".{}.lock", alias));

        // Try to create the lock file exclusively
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .with_context(|| format!("failed to acquire lock for instance '{}' (another warren operation may be in progress)", alias))?;

        // Write the current PID to the lock file for debugging
        let pid = std::process::id();
        use std::io::Write;
        let mut file = file;
        writeln!(file, "{}", pid).ok();

        Ok(Self {
            lock_path,
            _file: file,
        })
    }

    /// Try to acquire the lock, waiting up to the given timeout.
    #[allow(dead_code)]
    pub fn acquire_timeout(
        instances_dir: &std::path::Path,
        alias: &str,
        timeout: Duration,
    ) -> Result<Self> {
        let start = std::time::Instant::now();
        loop {
            match Self::acquire(instances_dir, alias) {
                Ok(lock) => return Ok(lock),
                Err(_) => {
                    if start.elapsed() >= timeout {
                        bail!(
                            "timed out waiting for lock on instance '{}' ({}s)",
                            alias,
                            timeout.as_secs()
                        );
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// A global warren lock for operations that modify the instances directory
/// itself (e.g., import, export).
pub struct GlobalLock {
    lock_path: std::path::PathBuf,
    _file: File,
}

impl GlobalLock {
    pub fn acquire(instances_dir: &std::path::Path) -> Result<Self> {
        let lock_path = instances_dir.join(".warren.lock");

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .with_context(|| "failed to acquire global warren lock (another warren operation may be in progress)")?;

        let pid = std::process::id();
        use std::io::Write;
        let mut file = file;
        writeln!(file, "{}", pid).ok();

        Ok(Self {
            lock_path,
            _file: file,
        })
    }
}

impl Drop for GlobalLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}
