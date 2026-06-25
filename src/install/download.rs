use std::path::Path;
use anyhow::{Context, Result, bail};
use sha2::{Sha256, Digest};

pub fn parse_source(source: &str) -> SourceInfo {
    let trimmed = source.trim();
    if trimmed.contains('|') {
        if let Some(url) = extract_url_from_pipe(trimmed) {
            return SourceInfo::RemoteScript { url, original: trimmed.to_string() };
        }
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return SourceInfo::RemoteScript { url: trimmed.to_string(), original: trimmed.to_string() };
    }
    if trimmed.starts_with("./") || trimmed.starts_with('/') || trimmed.ends_with(".sh") {
        return SourceInfo::LocalScript { path: trimmed.to_string() };
    }
    SourceInfo::Package { name: trimmed.to_string() }
}

#[derive(Debug, Clone)]
pub enum SourceInfo {
    RemoteScript { url: String, original: String },
    LocalScript { path: String },
    Package { name: String },
}

fn extract_url_from_pipe(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split('|').collect();
    let fetch_cmd = parts.first()?.trim();
    let tokens: Vec<&str> = fetch_cmd.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        if token.starts_with("http://") || token.starts_with("https://") {
            return Some(token.to_string());
        }
        if *token == "-fsSL" || *token == "-sSL" || *token == "-fsL" || *token == "-sL" || *token == "-L" {
            if let Some(next) = tokens.get(i + 1) {
                if next.starts_with("http://") || next.starts_with("https://") {
                    return Some(next.to_string());
                }
            }
        }
    }
    for token in &tokens {
        if token.starts_with("http://") || token.starts_with("https://") {
            return Some(token.to_string());
        }
    }
    None
}

pub async fn download_installer(url: &str, dest: &Path) -> Result<String> {
    tracing::info!(url = %url, dest = %dest.display(), "downloading installer");
    let response = reqwest::get(url).await
        .with_context(|| format!("failed to fetch installer from {}", url))?;
    if !response.status().is_success() {
        bail!("failed to download installer: HTTP {} from {}", response.status(), url);
    }
    let bytes = response.bytes().await
        .with_context(|| format!("failed to read response body from {}", url))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("sha256:{}", hex::encode(hasher.finalize()));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    std::fs::write(dest, &bytes)
        .with_context(|| format!("failed to write installer to {}", dest.display()))?;
    tracing::debug!(hash = %hash, bytes = bytes.len(), "download complete");
    Ok(hash)
}

pub fn read_local_installer(path: &Path, dest: &Path) -> Result<String> {
    let content = std::fs::read(path)
        .with_context(|| format!("failed to read local installer {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = format!("sha256:{}", hex::encode(hasher.finalize()));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &content)
        .with_context(|| format!("failed to copy installer to {}", dest.display()))?;
    Ok(hash)
}
