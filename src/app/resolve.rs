//! Turn wrap sources into concrete launch commands (rootless).
//!
//! Warren never installs through system package managers — that would
//! need root. Instead it **wraps** the already-installed program: the
//! generated launcher runs the host binary (`flatpak run …`,
//! `/usr/bin/discord`, …) with a private `$HOME`/`$XDG_*`, so two
//! instances (e.g. `discord-work` + `discord-home`) keep separate
//! logins while sharing the same on-disk application.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::sources::SourceInfo;

/// What `warren dig` will actually execute inside the isolated env.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    /// Short name used for metadata / `.desktop` Name.
    pub display_name: String,
    /// Host command, e.g. `["flatpak", "run", "com.discordapp.Discord"]`.
    pub command: Vec<String>,
    /// Graphical app → `.desktop` entry + detached `warren run`.
    pub gui: bool,
    pub icon: Option<String>,
    pub desktop_id: Option<String>,
}

/// Resolve a wrap source against the current machine.
///
/// `gui_override` comes from `warren dig --gui / --no-gui` and wins over
/// auto-detection.
pub fn resolve(source: &SourceInfo, gui_override: Option<bool>) -> Result<LaunchSpec> {
    let path_dirs = path_dirs();
    let data_dirs = standard_data_dirs();
    match source {
        SourceInfo::Flatpak { app_id } => resolve_flatpak(app_id, gui_override),
        SourceInfo::Snap { name } => resolve_snap(name, gui_override),
        SourceInfo::System { manager, name } => {
            resolve_system_in_path(manager, name, &path_dirs, gui_override)
        }
        SourceInfo::App { query } => {
            resolve_app_with_dirs(query, &path_dirs, &data_dirs, true, gui_override)
        }
        SourceInfo::Desktop { id } => resolve_desktop_with_dirs(id, &data_dirs, gui_override),
        other => bail!(
            "source '{}' is not a wrappable app source",
            super::sources::source_kind(other)
        ),
    }
}

// ---------------------------------------------------------------------------
// Per-kind resolvers
// ---------------------------------------------------------------------------

fn resolve_flatpak(app_id: &str, gui_override: Option<bool>) -> Result<LaunchSpec> {
    if which_in_path("flatpak", &path_dirs()).is_none() {
        bail!(
            "flatpak is not installed, so '{app_id}' cannot be wrapped. Install flatpak first, then run: warren dig flatpak:{app_id} --as <alias>"
        );
    }
    let status = std::process::Command::new("flatpak")
        .args(["info", app_id])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(LaunchSpec {
            display_name: flatpak_display_name(app_id),
            command: vec!["flatpak".to_string(), "run".to_string(), app_id.to_string()],
            gui: gui_override.unwrap_or(true),
            icon: None,
            desktop_id: Some(format!("{app_id}.desktop")),
        }),
        _ => bail!(
            "flatpak app '{app_id}' is not installed. Install it first (e.g. `flatpak install flathub {app_id}`), then re-run your warren dig command."
        ),
    }
}

fn resolve_snap(name: &str, gui_override: Option<bool>) -> Result<LaunchSpec> {
    if which_in_path("snap", &path_dirs()).is_none() {
        bail!("snap is not installed, so '{name}' cannot be wrapped. Install snapd first.");
    }
    Ok(LaunchSpec {
        display_name: name.to_string(),
        command: vec!["snap".to_string(), "run".to_string(), name.to_string()],
        gui: gui_override.unwrap_or(true),
        icon: None,
        desktop_id: Some(format!("{name}_{name}.desktop")),
    })
}

fn resolve_system_in_path(
    manager: &str,
    name: &str,
    path_dirs: &[PathBuf],
    gui_override: Option<bool>,
) -> Result<LaunchSpec> {
    match which_in_path(name, path_dirs) {
        Some(path) => Ok(LaunchSpec {
            display_name: name.to_string(),
            command: vec![path.to_string_lossy().to_string()],
            gui: gui_override.unwrap_or(false),
            icon: None,
            desktop_id: None,
        }),
        None => bail!(
            "'{name}' is not on $PATH. Install it with your package manager first (e.g. `sudo {manager} install {name}` — warren stays rootless and only wraps what is already installed), then re-run your warren dig command."
        ),
    }
}

fn resolve_app_with_dirs(
    query: &str,
    path_dirs: &[PathBuf],
    data_dirs: &[PathBuf],
    check_flatpak_list: bool,
    gui_override: Option<bool>,
) -> Result<LaunchSpec> {
    // 1. Desktop entry first: it tells us the GUI flag + icon + real Exec.
    let desktop = find_desktop_file(query, data_dirs)
        .and_then(|p| parse_desktop_file(&p).ok().map(|e| (p, e)));
    // 2. Binary on PATH.
    let bin = which_in_path(query, path_dirs);

    match (bin, desktop) {
        (Some(path), Some((desk_path, entry))) => {
            let cmd = if entry.exec_argv.is_empty() {
                vec![path.to_string_lossy().to_string()]
            } else {
                entry.exec_argv.clone()
            };
            Ok(LaunchSpec {
                display_name: entry.name.clone().unwrap_or_else(|| query.to_string()),
                command: cmd,
                gui: gui_override.unwrap_or(!entry.terminal),
                icon: entry.icon.clone(),
                desktop_id: desk_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string()),
            })
        }
        (Some(path), None) => Ok(LaunchSpec {
            display_name: query.to_string(),
            command: vec![path.to_string_lossy().to_string()],
            gui: gui_override.unwrap_or(false),
            icon: None,
            desktop_id: None,
        }),
        (None, Some((desk_path, entry))) if !entry.exec_argv.is_empty() => Ok(LaunchSpec {
            display_name: entry.name.clone().unwrap_or_else(|| query.to_string()),
            command: entry.exec_argv.clone(),
            gui: gui_override.unwrap_or(!entry.terminal),
            icon: entry.icon.clone(),
            desktop_id: desk_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string()),
        }),
        (None, _) => {
            if check_flatpak_list && flatpak_app_ids().iter().any(|id| app_id_matches(id, query)) {
                let id = flatpak_app_ids()
                    .into_iter()
                    .find(|id| app_id_matches(id, query))
                    .unwrap_or_else(|| query.to_string());
                return Ok(LaunchSpec {
                    display_name: flatpak_display_name(&id),
                    command: vec!["flatpak".to_string(), "run".to_string(), id.clone()],
                    gui: gui_override.unwrap_or(true),
                    icon: None,
                    desktop_id: Some(format!("{id}.desktop")),
                });
            }
            bail!(
                "app '{query}' not found: no `{query}` on $PATH, no matching .desktop file, and no installed flatpak provides it. Install the app first (flatpak / snap / system package), then run: warren dig app:{query} --as <alias>"
            );
        }
    }
}

fn resolve_desktop_with_dirs(
    id: &str,
    data_dirs: &[PathBuf],
    gui_override: Option<bool>,
) -> Result<LaunchSpec> {
    let path = find_desktop_file(id, data_dirs).with_context(|| {
        format!(
            "desktop entry '{id}' not found in {}. Install the app first, then retry.",
            data_dirs
                .iter()
                .map(|d| d.join("applications").to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let entry = parse_desktop_file(&path)?;
    if entry.exec_argv.is_empty() {
        bail!(
            "desktop entry '{}' has no usable Exec= line",
            path.display()
        );
    }
    Ok(LaunchSpec {
        display_name: entry.name.clone().unwrap_or_else(|| id.to_string()),
        command: entry.exec_argv.clone(),
        gui: gui_override.unwrap_or(!entry.terminal),
        icon: entry.icon.clone(),
        desktop_id: path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string()),
    })
}

// ---------------------------------------------------------------------------
// Environment helpers
// ---------------------------------------------------------------------------

fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

fn which_in_path(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        return is_executable(&p).then_some(p);
    }
    dirs.iter().map(|d| d.join(name)).find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file()
        && p.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn standard_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        let custom = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        dirs.push(custom.unwrap_or_else(|| home.join(".local/share")));
    }
    if let Ok(list) = std::env::var("XDG_DATA_DIRS") {
        dirs.extend(std::env::split_paths(&list));
    } else {
        dirs.push(PathBuf::from("/usr/local/share"));
        dirs.push(PathBuf::from("/usr/share"));
    }
    if let Ok(flatpak_dirs) = std::env::var("FLATPAK_DATA_DIRS") {
        dirs.extend(std::env::split_paths(&flatpak_dirs));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop"));
    dirs
}

fn flatpak_app_ids() -> Vec<String> {
    let out = std::process::Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && l != "Application")
            .collect(),
        _ => Vec::new(),
    }
}

fn app_id_matches(id: &str, query: &str) -> bool {
    let q = query.to_ascii_lowercase();
    let lower = id.to_ascii_lowercase();
    lower == q || lower.ends_with(&format!(".{q}")) || lower.contains(&q)
}

fn flatpak_display_name(app_id: &str) -> String {
    app_id
        .rsplit('.')
        .next()
        .unwrap_or(app_id)
        .to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// .desktop parsing
// ---------------------------------------------------------------------------

/// Parsed `[Desktop Entry]` group (only the keys warren needs).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: Option<String>,
    pub exec_argv: Vec<String>,
    /// `Terminal=true` → CLI app; anything else → GUI.
    pub terminal: bool,
    pub icon: Option<String>,
}

/// Find `query[.desktop]` (or a `Name=` match) under `<dir>/applications`.
pub fn find_desktop_file(query: &str, data_dirs: &[PathBuf]) -> Option<PathBuf> {
    let q = query.trim();
    let bare = q.strip_suffix(".desktop").unwrap_or(q);
    let mut candidates = vec![
        format!("{bare}.desktop"),
        format!("{}.desktop", bare.to_ascii_lowercase()),
    ];
    candidates.sort();
    candidates.dedup();

    // 1. Exact filename match.
    for dir in data_dirs {
        let apps = dir.join("applications");
        for c in &candidates {
            let p = apps.join(c);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // 2. Case-insensitive filename match.
    for dir in data_dirs {
        let apps = dir.join("applications");
        let rd = std::fs::read_dir(&apps).ok()?;
        for entry in rd.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".desktop") {
                continue;
            }
            if name.eq_ignore_ascii_case(&candidates[0]) {
                return Some(entry.path());
            }
        }
    }
    // 3. `Name=` match (e.g. `desktop:Discord` → `discord.desktop`).
    for dir in data_dirs {
        let apps = dir.join("applications");
        let rd = std::fs::read_dir(&apps).ok()?;
        for entry in rd.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&p)
                && let Some(name) = desktop_name(&content)
                && (name.eq_ignore_ascii_case(bare) || name.eq_ignore_ascii_case(q))
            {
                return Some(p);
            }
        }
    }
    None
}

fn desktop_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("Name=") {
            return Some(name.trim().to_string());
        }
        if line.starts_with('[') && line != "[Desktop Entry]" {
            break;
        }
    }
    None
}

/// Parse a `.desktop` file's `[Desktop Entry]` group.
pub fn parse_desktop_file(path: &Path) -> Result<DesktopEntry> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read desktop entry {}", path.display()))?;
    let mut entry = DesktopEntry::default();
    let mut in_group = false;
    let mut saw_group = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            saw_group |= in_group;
            continue;
        }
        if !in_group || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            if entry.name.is_none() {
                entry.name = Some(value.trim().to_string());
            }
        } else if let Some(value) = line.strip_prefix("Exec=") {
            if entry.exec_argv.is_empty() {
                entry.exec_argv = split_exec(value);
            }
        } else if let Some(value) = line.strip_prefix("Terminal=") {
            entry.terminal = value.trim().eq_ignore_ascii_case("true");
        } else if let Some(value) = line.strip_prefix("Icon=")
            && entry.icon.is_none()
            && !value.trim().is_empty()
        {
            entry.icon = Some(value.trim().to_string());
        }
    }
    if !saw_group {
        bail!(
            "{} is not a desktop entry (missing [Desktop Entry])",
            path.display()
        );
    }
    Ok(entry)
}

/// Split an `Exec=` line, dropping freedesktop field codes (`%U`, `%F`, …).
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut argv = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = exec.chars().peekable();
    let flush = |cur: &mut String, argv: &mut Vec<String>| {
        if cur.starts_with('%') && cur.len() == 2 {
            // Field code → drop and reset.
            cur.clear();
        } else if !cur.is_empty() {
            argv.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' && chars.peek() == Some(&q) {
                    cur.push(chars.next().unwrap_or(q));
                } else {
                    cur.push(c);
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                ' ' | '\t' => flush(&mut cur, &mut argv),
                _ => cur.push(c),
            },
        }
    }
    flush(&mut cur, &mut argv);
    argv
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "warren-resolve-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_exe(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\necho hi\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn system_resolves_binary_on_path() {
        let bin = tmp();
        make_exe(&bin, "mybrowser");
        let spec =
            resolve_system_in_path("apt", "mybrowser", std::slice::from_ref(&bin), None).unwrap();
        assert_eq!(
            spec.command,
            vec![bin.join("mybrowser").to_string_lossy().to_string()]
        );
        assert!(!spec.gui);
        let _ = std::fs::remove_dir_all(&bin);
    }

    #[test]
    fn system_missing_binary_errors_with_manager_hint() {
        let bin = tmp();
        let err =
            resolve_system_in_path("apt", "no-such-bin-xyz", std::slice::from_ref(&bin), None)
                .unwrap_err();
        assert!(err.to_string().contains("apt"), "{}", err);
        let _ = std::fs::remove_dir_all(&bin);
    }

    #[test]
    fn gui_override_wins() {
        let bin = tmp();
        make_exe(&bin, "tool");
        assert!(
            resolve_system_in_path("dnf", "tool", std::slice::from_ref(&bin), Some(true))
                .unwrap()
                .gui
        );
        assert!(
            !resolve_system_in_path("dnf", "tool", std::slice::from_ref(&bin), Some(false))
                .unwrap()
                .gui
        );
        let _ = std::fs::remove_dir_all(&bin);
    }

    #[test]
    fn desktop_parse_and_find() {
        let data = tmp();
        let apps = data.join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join("discord.desktop"),
            "[Desktop Entry]\nName=Discord\nExec=/usr/bin/discord --start-minimized %U\nTerminal=false\nIcon=discord\n",
        )
        .unwrap();
        let found = find_desktop_file("discord", std::slice::from_ref(&data)).unwrap();
        assert!(found.ends_with("discord.desktop"));
        // Name= fallback.
        assert!(find_desktop_file("Discord", std::slice::from_ref(&data)).is_some());
        let entry = parse_desktop_file(&found).unwrap();
        assert_eq!(entry.name.as_deref(), Some("Discord"));
        assert_eq!(
            entry.exec_argv,
            vec!["/usr/bin/discord", "--start-minimized"]
        );
        assert!(!entry.terminal);
        assert_eq!(entry.icon.as_deref(), Some("discord"));

        let spec = resolve_desktop_with_dirs("discord", std::slice::from_ref(&data), None).unwrap();
        assert!(spec.gui);
        assert_eq!(spec.command[0], "/usr/bin/discord");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn split_exec_handles_quotes_and_codes() {
        assert_eq!(
            split_exec("/opt/app/run --flag %U %F"),
            vec!["/opt/app/run", "--flag"]
        );
        assert_eq!(
            split_exec("\"/opt/my app/run\" --x"),
            vec!["/opt/my app/run", "--x"]
        );
        assert_eq!(
            split_exec("flatpak run com.foo.Bar @@u %U @@"),
            vec!["flatpak", "run", "com.foo.Bar", "@@u", "@@"]
        );
    }

    #[test]
    fn app_prefers_desktop_gui_flag_but_path_binary() {
        let base = tmp();
        let bin = base.join("bin");
        let data = base.join("data");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(data.join("applications")).unwrap();
        make_exe(&bin, "notes");
        std::fs::write(
            data.join("applications/notes.desktop"),
            "[Desktop Entry]\nName=Notes\nExec=notes %U\nTerminal=false\nIcon=notes\n",
        )
        .unwrap();
        // Desktop Exec (minus field codes) wins so flags like
        // `--start-minimized` survive; GUI flag + icon come along.
        let spec = resolve_app_with_dirs(
            "notes",
            std::slice::from_ref(&bin),
            std::slice::from_ref(&data),
            false,
            None,
        )
        .unwrap();
        assert!(spec.gui);
        assert_eq!(spec.command, vec!["notes".to_string()]);
        assert_eq!(spec.icon.as_deref(), Some("notes"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn flatpak_display_name_uses_last_component() {
        assert_eq!(flatpak_display_name("com.discordapp.Discord"), "discord");
        assert_eq!(flatpak_display_name("org.mozilla.firefox"), "firefox");
    }
}
