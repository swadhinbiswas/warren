pub mod launcher;
pub mod layout;
pub mod metadata;

pub use launcher::Launcher;
pub use layout::InstanceLayout;
pub use metadata::InstanceMetadata;

use anyhow::{Result, bail};
use regex::Regex;
use std::sync::LazyLock;

static ALIAS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]([a-z0-9\-]*[a-z0-9])?$").expect("invalid alias regex pattern")
});

/// Validate an instance alias.
pub fn validate_alias(alias: &str) -> Result<()> {
    if alias.is_empty() {
        bail!("alias cannot be empty");
    }
    if alias.len() > 64 {
        bail!("alias must be 64 characters or fewer (got {})", alias.len());
    }
    if !ALIAS_REGEX.is_match(alias) {
        bail!(
            "alias '{}' is invalid. Must match [a-z0-9][a-z0-9-]*[a-z0-9], \
             start and end with alphanumeric, contain only lowercase letters, digits, and hyphens.",
            alias
        );
    }
    if alias.contains("--") {
        bail!("alias cannot contain consecutive hyphens");
    }
    Ok(())
}
