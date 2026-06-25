use std::path::Path;
use anyhow::{Result, bail};
use regex::Regex;

pub struct InstallerRewriter {
    rules: Vec<RewriteRule>,
}

struct RewriteRule {
    pattern: Regex,
    replacement: String,
    description: String,
}

impl InstallerRewriter {
    pub fn new(instance_dir: &Path) -> Self {
        let dir = instance_dir.to_string_lossy();
        let home_dir = dirs::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("$HOME"));

        let rules = vec![
            RewriteRule {
                pattern: Regex::new(r"/usr/local/bin").unwrap(),
                replacement: format!("{}/bin", dir),
                description: "/usr/local/bin → instance/bin".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"/usr/bin").unwrap(),
                replacement: format!("{}/bin", dir),
                description: "/usr/bin → instance/bin".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.local/share", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/data", dir),
                description: "~/.local/share → instance/data".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.local/state", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/state", dir),
                description: "~/.local/state → instance/state".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.local/bin", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/bin", dir),
                description: "~/.local/bin → instance/bin".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.config", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/config", dir),
                description: "~/.config → instance/config".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.cache", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/cache", dir),
                description: "~/.cache → instance/cache".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.local/share").unwrap(),
                replacement: format!("{}/data", dir),
                description: "~/.local/share → instance/data".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.local/state").unwrap(),
                replacement: format!("{}/state", dir),
                description: "~/.local/state → instance/state".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.local/bin").unwrap(),
                replacement: format!("{}/bin", dir),
                description: "~/.local/bin → instance/bin".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.config").unwrap(),
                replacement: format!("{}/config", dir),
                description: "~/.config → instance/config".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.cache").unwrap(),
                replacement: format!("{}/cache", dir),
                description: "~/.cache → instance/cache".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{HOME\}").unwrap(),
                replacement: format!("{}/home", dir),
                description: "${HOME} → instance/home".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$HOME").unwrap(),
                replacement: format!("{}/home", dir),
                description: "$HOME → instance/home".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_CONFIG_HOME\}?").unwrap(),
                replacement: format!("{}/config", dir),
                description: "$XDG_CONFIG_HOME → instance/config".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_CACHE_HOME\}?").unwrap(),
                replacement: format!("{}/cache", dir),
                description: "$XDG_CACHE_HOME → instance/cache".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_DATA_HOME\}?").unwrap(),
                replacement: format!("{}/data", dir),
                description: "$XDG_DATA_HOME → instance/data".to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_STATE_HOME\}?").unwrap(),
                replacement: format!("{}/state", dir),
                description: "$XDG_STATE_HOME → instance/state".to_string(),
            },
        ];
        Self { rules }
    }

    pub fn rewrite(&self, content: &str) -> RewriteResult {
        let mut output = content.to_string();
        let mut changes = Vec::new();
        for rule in &self.rules {
            let count = rule.pattern.find_iter(&output).count();
            if count > 0 {
                output = rule.pattern.replace_all(&output, rule.replacement.as_str()).to_string();
                changes.push(RewriteChange { description: rule.description.clone(), count });
            }
        }
        RewriteResult { content: output, changes }
    }

    pub fn validate(content: &str, _instance_dir: &Path) -> Result<()> {
        if content.contains("..") {
            let traversal = Regex::new(r"\.\.[\\/]").unwrap();
            if traversal.is_match(content) {
                bail!("installer contains path traversal sequences (../) which are not allowed");
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RewriteResult {
    pub content: String,
    pub changes: Vec<RewriteChange>,
}

impl RewriteResult {
    pub fn total_changes(&self) -> usize {
        self.changes.iter().map(|c| c.count).sum()
    }
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }
}

#[derive(Debug)]
pub struct RewriteChange {
    pub description: String,
    pub count: usize,
}

pub fn generate_diff(original: &str, rewritten: &str) -> String {
    let mut diff = String::new();
    let original_lines: Vec<&str> = original.lines().collect();
    let rewritten_lines: Vec<&str> = rewritten.lines().collect();
    diff.push_str("--- original\n");
    diff.push_str("+++ rewritten\n");
    for (i, (orig, rewr)) in original_lines.iter().zip(rewritten_lines.iter()).enumerate() {
        if orig != rewr {
            diff.push_str(&format!("@@ line {} @@\n", i + 1));
            diff.push_str(&format!("-{}\n", orig));
            diff.push_str(&format!("+{}\n", rewr));
        }
    }
    diff
}
