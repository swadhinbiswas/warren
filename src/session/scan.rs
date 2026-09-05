use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One restorable application captured in a session snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionApp {
    /// Short binary name, e.g. `google-chrome`, `gnome-terminal`, `code`.
    pub name: String,
    pub kind: AppKind,
    /// Binary to relaunch (resolved to a bare name on PATH where possible).
    pub binary: String,
    /// Working directory the process was sitting in, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Workspace folders/files for editors (only paths that existed at save time).
    #[serde(default)]
    pub workspaces: Vec<String>,
    /// Full argv at capture time (informational; restore builds a clean command).
    #[serde(default)]
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    Browser,
    Terminal,
    Editor,
    Files,
    Other,
}

impl std::fmt::Display for AppKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppKind::Browser => write!(f, "browser"),
            AppKind::Terminal => write!(f, "terminal"),
            AppKind::Editor => write!(f, "editor"),
            AppKind::Files => write!(f, "files"),
            AppKind::Other => write!(f, "other"),
        }
    }
}

impl std::str::FromStr for AppKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "browser" => Ok(AppKind::Browser),
            "terminal" => Ok(AppKind::Terminal),
            "editor" => Ok(AppKind::Editor),
            "files" => Ok(AppKind::Files),
            "other" => Ok(AppKind::Other),
            _ => Err(format!("unknown app kind '{}'", s)),
        }
    }
}

/// Scan `/proc` for user-owned, restorable GUI/terminal applications.
///
/// Best-effort and rootless: anything unreadable is skipped. Multi-process
/// apps (browsers, VS Code helpers) are deduplicated so restore launches
/// each app once instead of replaying every `--type=renderer` child.
pub fn scan_running_apps() -> Vec<SessionApp> {
    let self_uid = proc_dir_uid(&std::process::id().to_string());
    let self_pid = std::process::id();
    let own_exe = std::env::current_exe().ok();

    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut apps: Vec<SessionApp> = Vec::new();

    let entries = std::fs::read_dir("/proc").map(|rd| {
        rd.filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .chars()
                    .all(|c| c.is_ascii_digit())
            })
            .collect::<Vec<_>>()
    });

    let entries = match entries {
        Ok(e) => e,
        Err(_) => return fallback_shell_app(),
    };

    for entry in entries {
        let pid: u32 = match entry.file_name().to_string_lossy().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if pid == self_pid || pid <= 1 {
            continue;
        }
        // Only capture our own user's processes.
        if let (Some(want), Some(got)) = (self_uid, proc_dir_uid(&pid.to_string()))
            && want != got
        {
            continue;
        }
        let Some(raw) = read_proc(pid) else { continue };
        if raw.argv.is_empty() {
            continue; // kernel thread
        }
        // Skip multiprocess children: they cannot be relaunched directly
        // and would spam restore with dozens of entries. The `contains`
        // arm catches collapsed Chromium-style cmdlines that were not
        // NUL-separated (`/usr/bin/chromium --type=renderer ...`).
        if raw.argv.iter().any(|a| {
            a.starts_with("--type=")
                || a.contains(" --type=")
                || a == "--renderer"
                || a.starts_with("--extension-")
        }) {
            continue;
        }
        let binary = binary_name(&raw.argv[0]);
        if binary.is_empty() {
            continue;
        }
        if is_helper_binary(&binary) {
            continue;
        }
        // Never capture the `warren session save` invocation itself.
        if binary == "warren"
            && raw.argv.iter().any(|a| a == "session")
            && raw.argv.iter().any(|a| a == "save")
        {
            continue;
        }
        // Skip our own executable when invoked under a different name
        // (e.g. tests) — compare canonical exe paths.
        if let (Some(own), Some(exe)) = (own_exe.as_ref(), raw.exe.as_ref())
            && same_file(own, exe)
        {
            continue;
        }

        let kind = classify(&binary);
        // Non-interactive helpers and one-shot commands are noise.
        if kind == AppKind::Other && is_transient(&binary) {
            continue;
        }
        // For browsers keep a single entry per binary: tabs are restored
        // by the browser itself ("continue where you left off").
        let cwd = raw.cwd.filter(|c| !c.is_empty());
        let workspaces = if matches!(kind, AppKind::Editor) {
            extract_workspaces(&binary, &raw.argv)
        } else {
            Vec::new()
        };
        let dedup_key = match kind {
            AppKind::Browser => (binary.clone(), String::new(), String::new()),
            _ => (
                binary.clone(),
                cwd.clone().unwrap_or_default(),
                workspaces.join("\n"),
            ),
        };
        if !seen.insert(dedup_key) {
            continue;
        }
        apps.push(SessionApp {
            name: binary.clone(),
            kind,
            binary,
            cwd,
            workspaces,
            argv: raw.argv,
        });
        // Hard cap: snapshots must stay tiny so retention stays cheap.
        if apps.len() >= 100 {
            break;
        }
    }

    apps.sort_by(|a, b| {
        (
            kind_rank(a.kind),
            &a.name,
            a.cwd.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                kind_rank(b.kind),
                &b.name,
                b.cwd.as_deref().unwrap_or_default(),
            ))
    });

    if apps.is_empty() {
        return fallback_shell_app();
    }
    apps
}

fn fallback_shell_app() -> Vec<SessionApp> {
    // Always record at least the current shell so restore can reopen a
    // terminal in the right place even on headless/CI machines.
    let shell = std::env::var("SHELL")
        .ok()
        .and_then(|s| {
            Path::new(&s)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_string())
        })
        .unwrap_or_else(|| "sh".to_string());
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    vec![SessionApp {
        name: shell.clone(),
        kind: AppKind::Terminal,
        binary: shell,
        cwd,
        workspaces: Vec::new(),
        argv: Vec::new(),
    }]
}

struct RawProc {
    argv: Vec<String>,
    cwd: Option<String>,
    exe: Option<std::path::PathBuf>,
}

fn read_proc(pid: u32) -> Option<RawProc> {
    let base = format!("/proc/{}", pid);
    let cmdline = std::fs::read(format!("{}/cmdline", base)).ok()?;
    let mut argv: Vec<String> = cmdline
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect();
    if argv.is_empty() {
        return None;
    }
    // Chromium (and some Electron apps) rewrite their own argv into a
    // single space-separated string instead of NUL-separated entries,
    // e.g. `/usr/lib/chromium/chromium --type=renderer ...\0`.
    // Split those collapsed segments so child-process filtering and
    // binary extraction below see real tokens.
    argv = split_collapsed_argv(argv);
    if argv.is_empty() {
        return None;
    }
    // argv[0] can be empty for some daemons; fall back to comm or exe.
    if argv[0].is_empty()
        && let Ok(comm) = std::fs::read_to_string(format!("{}/comm", base))
    {
        let comm = comm.trim().to_string();
        if !comm.is_empty() {
            argv[0] = comm;
        }
    }
    let cwd = std::fs::read_link(format!("{}/cwd", base))
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let exe = std::fs::read_link(format!("{}/exe", base)).ok();
    Some(RawProc { argv, cwd, exe })
}

fn proc_dir_uid(pid: &str) -> Option<u32> {
    std::fs::metadata(format!("/proc/{}", pid))
        .ok()
        .map(|m| m.uid())
}

fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => {
            use std::os::unix::fs::MetadataExt;
            ma.dev() == mb.dev() && ma.ino() == mb.ino()
        }
        _ => false,
    }
}

fn binary_name(argv0: &str) -> String {
    // Defense in depth: if a collapsed `argv0` like
    // `/usr/lib/chromium/chromium --type=renderer ...` slips through,
    // only the executable part is the binary name.
    let first_token = argv0.split_whitespace().next().unwrap_or(argv0);
    let base = first_token.rsplit('/').next().unwrap_or(first_token);
    // Some sandboxed processes wrap the name in parens: `(chrome)`.
    let trimmed = base.trim_matches(|c| c == '(' || c == ')').trim();
    // Flatpak child wrappers look like `bwrap` — not restorable directly.
    trimmed.to_string()
}

/// Split collapsed single-string cmdlines (`a --type=b ...`) into tokens.
///
/// Only segments containing `--type=` are split, so editor workspace paths
/// containing spaces are never broken apart.
fn split_collapsed_argv(argv: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    for arg in argv {
        if arg.contains("--type=") && arg.contains(' ') {
            out.extend(arg.split_whitespace().map(|t| t.to_string()));
        } else {
            out.push(arg);
        }
    }
    out
}

fn kind_rank(kind: AppKind) -> u8 {
    match kind {
        AppKind::Browser => 0,
        AppKind::Terminal => 1,
        AppKind::Editor => 2,
        AppKind::Files => 3,
        AppKind::Other => 4,
    }
}

pub fn classify(binary: &str) -> AppKind {
    let b = binary.to_lowercase();
    if matches!(
        b.as_str(),
        "google-chrome"
            | "google-chrome-stable"
            | "chrome"
            | "chromium"
            | "chromium-browser"
            | "firefox"
            | "firefox-esr"
            | "brave"
            | "brave-browser"
            | "microsoft-edge"
            | "microsoft-edge-stable"
            | "edge"
            | "opera"
            | "vivaldi"
            | "zen"
    ) {
        return AppKind::Browser;
    }
    if matches!(
        b.as_str(),
        "gnome-terminal"
            | "gnome-terminal-server"
            | "konsole"
            | "alacritty"
            | "kitty"
            | "wezterm"
            | "foot"
            | "footclient"
            | "xterm"
            | "uxterm"
            | "tilix"
            | "terminator"
            | "terminology"
            | "ghostty"
            | "ptyxis"
            | "kgx"
            | "lxterminal"
            | "xfce4-terminal"
            | "mate-terminal"
            | "qterminal"
    ) || b.ends_with("-terminal")
    {
        return AppKind::Terminal;
    }
    if matches!(
        b.as_str(),
        "code"
            | "code-insiders"
            | "vscodium"
            | "codium"
            | "cursor"
            | "zed"
            | "zeditor"
            | "subl"
            | "sublime_text"
            | "nvim"
            | "vim"
            | "emacs"
            | "emacsclient"
            | "gedit"
            | "kate"
            | "neovide"
            | "lapce"
            | "helix"
            | "hx"
    ) {
        return AppKind::Editor;
    }
    if matches!(
        b.as_str(),
        "nautilus" | "dolphin" | "thunar" | "nemo" | "pcmanfm" | "caja" | "ranger" | "nnn" | "yazi"
    ) {
        return AppKind::Files;
    }
    AppKind::Other
}

fn is_helper_binary(binary: &str) -> bool {
    matches!(
        binary,
        "ps" | "pgrep"
            | "pkill"
            | "pidof"
            | "pgrep-user"
            | "lsof"
            | "ss"
            | "netstat"
            | "which"
            | "which.debianutils"
            | "sleep"
            | "timeout"
            | "env"
            | "sh"
            | "-sh"
            | "dash"
    )
}

fn is_transient(binary: &str) -> bool {
    matches!(
        binary,
        "ls" | "cat"
            | "grep"
            | "rg"
            | "sed"
            | "awk"
            | "find"
            | "git"
            | "cargo"
            | "rustc"
            | "node"
            | "python3"
            | "python"
            | "curl"
            | "wget"
            | "tar"
            | "gzip"
            | "sudo"
            | "systemctl"
            | "journalctl"
            | "dmesg"
            | "id"
            | "whoami"
            | "hostname"
            | "uname"
            | "dbus-daemon"
            | "dbus-launch"
            | "at-spi-bus-launcher"
            | "dconf-service"
            | "gvfsd"
            | "gvfsd-fuse"
            | "xdg-desktop-portal"
            | "xdg-document-portal"
            | "xdg-permission-store"
            | "ibus-daemon"
            | "pulseaudio"
            | "pipewire"
            | "wireplumber"
            | "systemd"
            | "(sd-pam)"
    )
}

/// Pull workspace folders/files out of an editor's argv.
///
/// Keeps only args that look like existing paths and skips known flags
/// (and flags that consume the next arg, like `--user-data-dir <dir>`).
pub fn extract_workspaces(binary: &str, argv: &[String]) -> Vec<String> {
    let b = binary.to_lowercase();
    let editor = matches!(
        b.as_str(),
        "code"
            | "code-insiders"
            | "vscodium"
            | "codium"
            | "cursor"
            | "zed"
            | "zeditor"
            | "subl"
            | "sublime_text"
            | "nvim"
            | "vim"
            | "emacs"
            | "emacsclient"
            | "gedit"
            | "kate"
            | "neovide"
            | "lapce"
            | "helix"
            | "hx"
    );
    if !editor {
        return Vec::new();
    }
    // Flags that take a value in the next argv slot.
    const VALUE_FLAGS: &[&str] = &[
        "--user-data-dir",
        "--extensions-dir",
        "--locale",
        "--log",
        "--open-url",
        "--socket",
        "--server",
        "--remote",
    ];
    let mut out = Vec::new();
    let mut skip_next = false;
    for (i, arg) in argv.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg.starts_with('-') {
            if arg.contains('=') {
                continue;
            }
            if VALUE_FLAGS.contains(&arg.as_str()) {
                skip_next = true;
            }
            // Bare `--` separator, `--new-window`, `--reuse-window`, etc.
            continue;
        }
        // file:// URIs from desktop launches.
        let candidate = arg
            .strip_prefix("file://")
            .map(|s| s.to_string())
            .unwrap_or_else(|| arg.clone());
        if candidate.is_empty() {
            continue;
        }
        if Path::new(&candidate).exists() && !out.contains(&candidate) {
            out.push(candidate);
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_apps() {
        assert_eq!(classify("google-chrome"), AppKind::Browser);
        assert_eq!(classify("firefox"), AppKind::Browser);
        assert_eq!(classify("gnome-terminal"), AppKind::Terminal);
        assert_eq!(classify("alacritty"), AppKind::Terminal);
        assert_eq!(classify("code"), AppKind::Editor);
        assert_eq!(classify("nvim"), AppKind::Editor);
        assert_eq!(classify("nautilus"), AppKind::Files);
        assert_eq!(classify("my-server"), AppKind::Other);
    }

    #[test]
    fn extracts_editor_workspaces() {
        let dir = std::env::temp_dir();
        let dir_str = dir.to_string_lossy().to_string();
        let argv = vec![
            "code".to_string(),
            "--new-window".to_string(),
            "--user-data-dir".to_string(),
            "/tmp/does-not-exist-xyz".to_string(),
            dir_str.clone(),
            "--nonexistent-path-xyz-123".to_string(),
        ];
        let ws = extract_workspaces("code", &argv);
        assert_eq!(ws, vec![dir_str]);
    }

    #[test]
    fn scan_returns_at_least_shell_fallback() {
        // Must never be empty: worst case is the shell fallback entry.
        let apps = scan_running_apps();
        assert!(!apps.is_empty());
    }

    #[test]
    fn collapsed_chromium_argv_splits() {
        // Chromium rewrites argv into one space-separated string.
        let collapsed = vec![
            "/usr/lib/chromium/chromium --type=renderer --lang=en-US --renderer-client-id=15"
                .to_string(),
        ];
        let split = split_collapsed_argv(collapsed);
        assert_eq!(
            split,
            vec![
                "/usr/lib/chromium/chromium",
                "--type=renderer",
                "--lang=en-US",
                "--renderer-client-id=15",
            ]
        );
        // Binary extraction must not include flags.
        assert_eq!(
            binary_name("/usr/lib/chromium/chromium --type=renderer --lang=en-US"),
            "chromium"
        );
    }

    #[test]
    fn normal_argv_untouched_by_collapse_split() {
        // Paths with spaces (editors) must survive when there is no --type=.
        let argv = vec![
            "code".to_string(),
            "/home/user/my project".to_string(),
            "--new-window".to_string(),
        ];
        assert_eq!(split_collapsed_argv(argv.clone()), argv);
    }
}
