use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use crate::ui::progress;

// Source parsing lives in `crate::app::sources` (extended with flatpak /
// snap / system / app / desktop wrap sources). Re-exported here so
// existing `install::download::{parse_source, SourceInfo}` call sites keep
// working unchanged.
pub use crate::app::sources::{SourceInfo, parse_source};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

pub async fn download_installer(url: &str, dest: &Path) -> Result<String> {
    tracing::info!(url = %url, dest = %dest.display(), "downloading installer");
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(RESPONSE_TIMEOUT)
        .user_agent(concat!("warren/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build HTTP client")?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to fetch installer from {}", url))?;
    if !response.status().is_success() {
        bail!(
            "failed to download installer: HTTP {} from {}",
            response.status(),
            url
        );
    }

    let total = response.content_length().unwrap_or(0);
    let bar = if total > 0 {
        Some(progress::download_bar(total))
    } else {
        None
    };
    let bar_ref = bar.as_ref();

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("failed to create installer file {}", dest.display()))?;

    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("failed to read response body from {}", url))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .with_context(|| format!("failed to write installer to {}", dest.display()))?;
        written += chunk.len() as u64;
        if let Some(bar) = bar_ref {
            bar.set_position(written);
        }
    }
    file.flush()
        .await
        .with_context(|| format!("failed to flush installer file {}", dest.display()))?;
    if let Some(bar) = bar_ref {
        bar.finish_with_message("Downloaded");
    }

    let hash = format!("sha256:{}", hex::encode(hasher.finalize()));
    tracing::debug!(hash = %hash, bytes = written, "download complete");
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
