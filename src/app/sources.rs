//! Extended install sources for desktop / GUI / system apps.
//!
//! Warren's original sources (remote script, local script, bare PATH
//! package) only cover CLI tools. To run the *same* graphical app many
//! times — e.g. two Discords from Flathub with two different accounts —
//! warren doesn't reinstall anything. It **wraps** the host-provided
//! program (`flatpak run …`, `/usr/bin/…`, …) in an isolated launcher
//! with a private `$HOME` / `$XDG_*`. This module parses those wrap
//! sources; see `super::resolve` for turning them into launch commands.

/// A parsed `warren dig <source>` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceInfo {
    RemoteScript {
        url: String,
    },
    LocalScript {
        path: String,
    },
    Package {
        name: String,
    },
    /// `flatpak:<app-id>` or a bare Flathub-style id (`com.discordapp.Discord`).
    Flatpak {
        app_id: String,
    },
    /// `snap:<name>`.
    Snap {
        name: String,
    },
    /// `apt:<pkg>`, `dnf:<pkg>`, `pacman:<pkg>`, … — wrap the installed
    /// package's binary without reinstalling it (stays rootless).
    System {
        manager: String,
        name: String,
    },
    /// `app:<name>` — smart lookup: `$PATH` → `.desktop` → flatpak.
    App {
        query: String,
    },
    /// `desktop:<id|name>` or a bare `foo.desktop` — resolve via
    /// `*.desktop` files and reuse their `Exec=` line.
    Desktop {
        id: String,
    },
}

/// System package managers warren can *wrap* (never installs through —
/// wrapping the already-installed binary keeps warren rootless).
pub const SYSTEM_MANAGERS: &[&str] = &[
    "apt", "dnf", "yum", "pacman", "apk", "zypper", "xbps", "eopkg", "nix", "brew", "pkg", "emerge",
];

/// Parse a `warren dig` source string.
///
/// Precedence: piped script / URL → explicit `prefix:` → local script →
/// bare Flatpak id → bare package name. Explicit prefixes always win, so
/// `flatpak:foo` never falls through to script detection.
pub fn parse_source(source: &str) -> SourceInfo {
    let trimmed = source.trim();

    // 1. Piped installer / plain URL (original behaviour, unchanged).
    if trimmed.contains('|')
        && let Some(url) = extract_url_from_pipe(trimmed)
    {
        return SourceInfo::RemoteScript { url };
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return SourceInfo::RemoteScript {
            url: trimmed.to_string(),
        };
    }

    // 2. Explicit `prefix:value` wrap sources.
    if let Some((prefix, value)) = trimmed.split_once(':')
        && !value.is_empty()
    {
        let manager = normalize_manager(prefix);
        if let Some(m) = manager {
            return SourceInfo::System {
                manager: m.to_string(),
                name: value.trim().to_string(),
            };
        }
        match prefix.to_ascii_lowercase().as_str() {
            "flatpak" => {
                return SourceInfo::Flatpak {
                    app_id: value.trim().to_string(),
                };
            }
            "snap" => {
                return SourceInfo::Snap {
                    name: value.trim().to_string(),
                };
            }
            "app" => {
                return SourceInfo::App {
                    query: value.trim().to_string(),
                };
            }
            "desktop" => {
                return SourceInfo::Desktop {
                    id: value.trim().to_string(),
                };
            }
            _ => {}
        }
    }

    // 3. Local script (original behaviour, unchanged).
    if trimmed.starts_with("./") || trimmed.starts_with('/') || trimmed.ends_with(".sh") {
        return SourceInfo::LocalScript {
            path: trimmed.to_string(),
        };
    }

    // 4. Bare `foo.desktop` → desktop source.
    if trimmed.ends_with(".desktop") && !trimmed.contains(' ') && !trimmed.contains('/') {
        return SourceInfo::Desktop {
            id: trimmed.to_string(),
        };
    }

    // 5. Bare reverse-DNS id (`com.discordapp.Discord`) → flatpak source.
    if looks_like_flatpak_id(trimmed) {
        return SourceInfo::Flatpak {
            app_id: trimmed.to_string(),
        };
    }

    // 6. Fallback: bare package/binary name (original behaviour).
    SourceInfo::Package {
        name: trimmed.to_string(),
    }
}

/// `true` for wrap sources (no installer is downloaded or executed).
pub fn is_wrap_source(info: &SourceInfo) -> bool {
    matches!(
        info,
        SourceInfo::Flatpak { .. }
            | SourceInfo::Snap { .. }
            | SourceInfo::System { .. }
            | SourceInfo::App { .. }
            | SourceInfo::Desktop { .. }
    )
}

/// Human-readable source kind for status lines, e.g. `flatpak`.
pub fn source_kind(info: &SourceInfo) -> &'static str {
    match info {
        SourceInfo::RemoteScript { .. } => "remote script",
        SourceInfo::LocalScript { .. } => "local script",
        SourceInfo::Package { .. } => "package",
        SourceInfo::Flatpak { .. } => "flatpak",
        SourceInfo::Snap { .. } => "snap",
        SourceInfo::System { .. } => "system",
        SourceInfo::App { .. } => "app",
        SourceInfo::Desktop { .. } => "desktop",
    }
}

fn normalize_manager(prefix: &str) -> Option<&'static str> {
    let lower = prefix.to_ascii_lowercase();
    let canonical = match lower.as_str() {
        "apt-get" => "apt",
        "yum" => "dnf",
        "xbps-install" => "xbps",
        "nix-env" => "nix",
        _ => lower.as_str(),
    };
    SYSTEM_MANAGERS.iter().find(|m| **m == canonical).copied()
}

fn looks_like_flatpak_id(s: &str) -> bool {
    if s.contains(' ') || s.contains('/') || s.contains(':') {
        return false;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    if !matches!(
        parts[0].to_ascii_lowercase().as_str(),
        "com" | "org" | "io" | "net" | "dev" | "app" | "page" | "info" | "me" | "one"
    ) {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

fn extract_url_from_pipe(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split('|').collect();
    let fetch_cmd = parts.first()?.trim();
    let tokens: Vec<&str> = fetch_cmd.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        if token.starts_with("http://") || token.starts_with("https://") {
            return Some(token.to_string());
        }
        if (*token == "-fsSL"
            || *token == "-sSL"
            || *token == "-fsL"
            || *token == "-sL"
            || *token == "-L")
            && let Some(next) = tokens.get(i + 1)
            && (next.starts_with("http://") || next.starts_with("https://"))
        {
            return Some(next.to_string());
        }
    }
    for token in &tokens {
        if token.starts_with("http://") || token.starts_with("https://") {
            return Some(token.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wrap_prefixes() {
        assert_eq!(
            parse_source("flatpak:com.discordapp.Discord"),
            SourceInfo::Flatpak {
                app_id: "com.discordapp.Discord".to_string()
            }
        );
        assert_eq!(
            parse_source("snap:discord"),
            SourceInfo::Snap {
                name: "discord".to_string()
            }
        );
        assert_eq!(
            parse_source("apt:firefox"),
            SourceInfo::System {
                manager: "apt".to_string(),
                name: "firefox".to_string()
            }
        );
        assert_eq!(
            parse_source("pacman:firefox"),
            SourceInfo::System {
                manager: "pacman".to_string(),
                name: "firefox".to_string()
            }
        );
        assert_eq!(
            parse_source("apt-get:vim"),
            SourceInfo::System {
                manager: "apt".to_string(),
                name: "vim".to_string()
            }
        );
        assert_eq!(
            parse_source("app:discord"),
            SourceInfo::App {
                query: "discord".to_string()
            }
        );
        assert_eq!(
            parse_source("desktop:discord"),
            SourceInfo::Desktop {
                id: "discord".to_string()
            }
        );
        assert_eq!(
            parse_source("pkg:gh"),
            SourceInfo::System {
                manager: "pkg".to_string(),
                name: "gh".to_string()
            }
        );
    }

    #[test]
    fn detects_bare_flatpak_ids_and_desktop_files() {
        assert_eq!(
            parse_source("com.discordapp.Discord"),
            SourceInfo::Flatpak {
                app_id: "com.discordapp.Discord".to_string()
            }
        );
        assert_eq!(
            parse_source("org.mozilla.firefox"),
            SourceInfo::Flatpak {
                app_id: "org.mozilla.firefox".to_string()
            }
        );
        assert_eq!(
            parse_source("discord.desktop"),
            SourceInfo::Desktop {
                id: "discord.desktop".to_string()
            }
        );
        // Too few dots → plain package, not flatpak.
        assert_eq!(
            parse_source("discord.app"),
            SourceInfo::Package {
                name: "discord.app".to_string()
            }
        );
    }

    #[test]
    fn keeps_legacy_sources_unchanged() {
        assert!(matches!(
            parse_source("curl -fsSL https://example.com/install | bash"),
            SourceInfo::RemoteScript { .. }
        ));
        assert!(matches!(
            parse_source("https://example.com/install.sh"),
            SourceInfo::RemoteScript { .. }
        ));
        assert!(matches!(
            parse_source("./install.sh"),
            SourceInfo::LocalScript { .. }
        ));
        assert!(matches!(
            parse_source("/tmp/install.sh"),
            SourceInfo::LocalScript { .. }
        ));
        assert_eq!(
            parse_source("gh"),
            SourceInfo::Package {
                name: "gh".to_string()
            }
        );
        // Unknown `prefix:` with empty value is not a wrap source.
        assert_eq!(
            parse_source("weird:"),
            SourceInfo::Package {
                name: "weird:".to_string()
            }
        );
    }

    #[test]
    fn wrap_predicate_and_kind() {
        assert!(is_wrap_source(&SourceInfo::Flatpak {
            app_id: "com.x.Y".to_string()
        }));
        assert!(is_wrap_source(&SourceInfo::App {
            query: "x".to_string()
        }));
        assert!(!is_wrap_source(&SourceInfo::Package {
            name: "x".to_string()
        }));
        assert_eq!(
            source_kind(&SourceInfo::Flatpak {
                app_id: "x".to_string()
            }),
            "flatpak"
        );
    }
}
