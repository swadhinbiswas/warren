pub mod layout;
pub mod metadata;
pub mod launcher;

pub use layout::InstanceLayout;
pub use metadata::InstanceMetadata;
pub use launcher::Launcher;

use anyhow::{bail, Result};
use regex::Regex;

/// Validate an instance alias.
pub fn validate_alias(alias: &str) -> Result<()> {
    if alias.is_empty() {
        bail!("alias cannot be empty");
    }
    if alias.len() > 64 {
        bail!("alias must be 64 characters or fewer (got {})", alias.len());
    }
    let re = Regex::new(r"^[a-z0-9]([a-z0-9\-]*[a-z0-9])?$").unwrap();
    if !re.is_match(alias) {
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
