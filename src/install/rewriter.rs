use anyhow::{Result, bail};
use regex::Regex;
use std::path::Path;

pub struct InstallerRewriter {
    rules: Vec<RewriteRule>,
}

struct RewriteRule {
    pattern: Regex,
    replacement: String,
}

impl InstallerRewriter {
    pub fn new(instance_dir: &Path) -> Self {
        let dir = instance_dir.to_string_lossy();
        let home_dir = dirs::home_dir()
            .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_default();

        let rules = vec![
            RewriteRule {
                pattern: Regex::new(r"/usr/local/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"/usr/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            // Absolute home paths (e.g., /home/user/.config)
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.local/share", regex::escape(&home_dir)))
                    .unwrap(),
                replacement: format!("{}/data", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.local/state", regex::escape(&home_dir)))
                    .unwrap(),
                replacement: format!("{}/state", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.local/bin", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/bin", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.config", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/config", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/.cache", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/cache", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/Documents", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/home/Documents", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/Downloads", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/home/Downloads", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/Desktop", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/home/Desktop", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/Music", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/home/Music", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/Pictures", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/home/Pictures", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(&format!(r"{}/Videos", regex::escape(&home_dir))).unwrap(),
                replacement: format!("{}/home/Videos", dir).to_string(),
            },
            // Tilde shorthand paths (e.g., ~/.config)
            RewriteRule {
                pattern: Regex::new(r"~/.local/share").unwrap(),
                replacement: format!("{}/data", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.local/state").unwrap(),
                replacement: format!("{}/state", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.local/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.config").unwrap(),
                replacement: format!("{}/config", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/.cache").unwrap(),
                replacement: format!("{}/cache", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/Documents").unwrap(),
                replacement: format!("{}/home/Documents", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/Downloads").unwrap(),
                replacement: format!("{}/home/Downloads", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/Desktop").unwrap(),
                replacement: format!("{}/home/Desktop", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/Music").unwrap(),
                replacement: format!("{}/home/Music", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/Pictures").unwrap(),
                replacement: format!("{}/home/Pictures", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/Videos").unwrap(),
                replacement: format!("{}/home/Videos", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"~/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            // Environment variable references.
            // IMPORTANT: `$HOME/bin` (and `${HOME}/bin`) must map to the
            // instance `bin/` — that is where the launcher and binary
            // detection look. These rules run *before* the generic `$HOME`
            // rewrite below so installers doing `install -m755 tool
            // $HOME/bin/` actually produce a runnable instance instead of
            // stranding the binary in `home/bin/`.
            RewriteRule {
                pattern: Regex::new(r"\$\{HOME\}/\.local/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$HOME/\.local/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{HOME\}/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$HOME/bin").unwrap(),
                replacement: format!("{}/bin", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{HOME\}").unwrap(),
                replacement: format!("{}/home", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$HOME").unwrap(),
                replacement: format!("{}/home", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_CONFIG_HOME\}?").unwrap(),
                replacement: format!("{}/config", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_CACHE_HOME\}?").unwrap(),
                replacement: format!("{}/cache", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_DATA_HOME\}?").unwrap(),
                replacement: format!("{}/data", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_STATE_HOME\}?").unwrap(),
                replacement: format!("{}/state", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?XDG_RUNTIME_DIR\}?").unwrap(),
                replacement: format!("{}/runtime", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$\{?TMPDIR\}?").unwrap(),
                replacement: format!("{}/tmp", dir).to_string(),
            },
            RewriteRule {
                pattern: Regex::new(r"\$TMPDIR").unwrap(),
                replacement: format!("{}/tmp", dir).to_string(),
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
                output = rule
                    .pattern
                    .replace_all(&output, rule.replacement.as_str())
                    .to_string();
                changes.push(RewriteChange { count });
            }
        }
        RewriteResult {
            content: output,
            changes,
        }
    }

    pub fn validate(content: &str, _instance_dir: &Path) -> Result<()> {
        static TRAVERSAL_REGEX: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
            Regex::new(r"\.\.[\\/]").expect("invalid traversal regex pattern")
        });
        if content.contains("..") && TRAVERSAL_REGEX.is_match(content) {
            bail!("installer contains path traversal sequences (../) which are not allowed");
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
    pub count: usize,
}

pub fn generate_diff(original: &str, rewritten: &str) -> String {
    let mut diff = String::new();
    let original_lines: Vec<&str> = original.lines().collect();
    let rewritten_lines: Vec<&str> = rewritten.lines().collect();
    diff.push_str("--- original\n");
    diff.push_str("+++ rewritten\n");
    for (i, (orig, rewr)) in original_lines
        .iter()
        .zip(rewritten_lines.iter())
        .enumerate()
    {
        if orig != rewr {
            diff.push_str(&format!("@@ line {} @@\n", i + 1));
            diff.push_str(&format!("-{}\n", orig));
            diff.push_str(&format!("+{}\n", rewr));
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rewriter() -> InstallerRewriter {
        InstallerRewriter::new(&PathBuf::from("/home/u/.warren/instances/demo"))
    }

    #[test]
    fn home_bin_installs_land_in_instance_bin() {
        // The #1 isolation trap for CLI tools: installers do
        // `install -m755 tool $HOME/bin`. That MUST become the instance
        // `bin/` (where the launcher points), never `home/bin/`.
        let r = rewriter();
        let out = r.rewrite("install -m755 mytool $HOME/bin/").content;
        assert!(
            out.contains("/home/u/.warren/instances/demo/bin/"),
            "got: {out}"
        );
        assert!(!out.contains("/home/bin"), "leaked home/bin: {out}");

        let out = r.rewrite("install -m755 mytool \"${HOME}/bin/\"").content;
        assert!(
            out.contains("/home/u/.warren/instances/demo/bin/"),
            "got: {out}"
        );

        let out = r.rewrite("cp mytool ~/bin/").content;
        assert!(
            out.contains("/home/u/.warren/instances/demo/bin/"),
            "got: {out}"
        );
    }

    #[test]
    fn braced_home_and_xdg_rewrite() {
        let r = rewriter();
        // `${HOME}` (previously broken: regex `$\\{HOME\\}` never matched)
        let out = r.rewrite("mkdir -p \"${HOME}/.config/app\"").content;
        assert!(
            out.contains("/home/u/.warren/instances/demo/home/"),
            "got: {out}"
        );
        assert!(!out.contains("${HOME}"), "unrewritten: {out}");

        // `$XDG_CONFIG_HOME` / `$XDG_DATA_HOME` / runtime / tmp
        let out = r
            .rewrite("echo $XDG_CONFIG_HOME $XDG_DATA_HOME $XDG_RUNTIME_DIR $TMPDIR")
            .content;
        assert!(
            out.contains("/home/u/.warren/instances/demo/config"),
            "got: {out}"
        );
        assert!(
            out.contains("/home/u/.warren/instances/demo/data"),
            "got: {out}"
        );
        assert!(
            out.contains("/home/u/.warren/instances/demo/runtime"),
            "got: {out}"
        );
        assert!(
            out.contains("/home/u/.warren/instances/demo/tmp"),
            "got: {out}"
        );
    }
}
