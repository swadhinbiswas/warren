use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::path::Path;

use crate::config::WarrenConfig;
use crate::instance::InstanceLayout;
use crate::ui::theme::Theme;

pub async fn execute(
    config: &WarrenConfig,
    theme: &Theme,
    alias: &str,
    out: Option<&Path>,
) -> Result<()> {
    let layout = InstanceLayout::new(&config.paths.instances_dir, alias);
    if !layout.exists() {
        bail!("instance '{}' not found", alias);
    }
    let output_path = out.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        let dir = std::env::current_dir()
            .context("failed to determine current directory (has it been deleted?)")
            .unwrap_or_else(|_| std::env::temp_dir());
        dir.join(format!("{}.warren.tar.gz", alias))
    });
    theme.step(
        "📦",
        &format!("Exporting {} to {}", alias, output_path.display()),
    );
    let file = std::fs::File::create(&output_path)
        .with_context(|| format!("failed to create archive {}", output_path.display()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(alias, &layout.root)
        .with_context(|| format!("failed to create archive for instance '{}'", alias))?;
    tar.finish()?;
    theme.success(&format!("Exported: {}", output_path.display()));
    Ok(())
}
