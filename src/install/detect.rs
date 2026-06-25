use std::path::Path;
use anyhow::Result;

pub fn detect_binary(bin_dir: &Path) -> Result<Option<String>> {
    if !bin_dir.exists() { return Ok(None); }
    let mut binaries: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_executable(&path) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                binaries.push(name.to_string());
            }
        }
    }
    tracing::debug!(binaries = ?binaries, "detected binaries");
    if binaries.is_empty() { return Ok(None); }
    if binaries.len() == 1 { return Ok(Some(binaries.into_iter().next().unwrap())); }
    binaries.sort_by_key(|b| b.len());
    Ok(Some(binaries.into_iter().next().unwrap()))
}

pub fn detect_version(bin_dir: &Path, binary_name: &str) -> Option<String> {
    let binary_path = bin_dir.join(binary_name);
    if !binary_path.exists() { return None; }
    for flag in &["--version", "-v", "-V", "version"] {
        if let Ok(output) = std::process::Command::new(&binary_path).arg(flag).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = extract_version_string(&stdout) { return Some(version); }
                let stderr = String::from_utf8_lossy(&output.stderr);
                if let Some(version) = extract_version_string(&stderr) { return Some(version); }
            }
        }
    }
    None
}

fn extract_version_string(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"v?(\d+\.\d+(?:\.\d+)?(?:-[a-zA-Z0-9.]+)?)").ok()?;
    re.captures(text.lines().next()?)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
