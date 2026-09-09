use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};

use crate::session::scan::AppKind;
use crate::session::store::{SessionSnapshot, snapshot_dir};

/// One relaunchable unit derived from a snapshot entry.
#[derive(Debug, Clone)]
pub struct RestoreEntry {
    /// Short display name (binary).
    pub name: String,
    pub kind: AppKind,
    /// Exact argv to spawn.
    pub command: Vec<String>,
    /// Directory to spawn in, if it still exists.
    pub cwd: Option<String>,
    /// Workspace files/folders that still exist (editors).
    pub workspaces: Vec<String>,
    /// Set when the entry is skipped instead of launched.
    pub skip_reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct RestoreReport {
    pub launched: Vec<String>,
    pub skipped: Vec<(String, String)>,
    pub failed: Vec<(String, String)>,
    pub log_path: Option<String>,
}

/// Pure planning step: snapshot -> relaunch commands. No side effects,
/// so `--dry-run` and tests can inspect it safely.
pub fn build_restore_plan(snapshot: &SessionSnapshot) -> Vec<RestoreEntry> {
    snapshot
        .apps
        .iter()
        .map(|app| {
            let binary_on_path = which::which(&app.binary).is_ok();
            if !binary_on_path {
                return RestoreEntry {
                    name: app.name.clone(),
                    kind: app.kind,
                    command: vec![app.binary.clone()],
                    cwd: app.cwd.clone(),
                    workspaces: app.workspaces.clone(),
                    skip_reason: Some("binary not found on PATH".to_string()),
                };
            }
            let cwd = app.cwd.clone().filter(|c| Path::new(c).exists());
            match app.kind {
                AppKind::Browser => RestoreEntry {
                    // Browsers restore their own tabs ("continue where you
                    // left off"); relaunching the binary is sufficient and
                    // avoids replaying `--type=renderer` children.
                    name: app.name.clone(),
                    kind: app.kind,
                    command: vec![app.binary.clone()],
                    cwd,
                    workspaces: Vec::new(),
                    skip_reason: None,
                },
                AppKind::Terminal => {
                    let mut command = vec![app.binary.clone()];
                    command.extend(terminal_workdir_args(&app.binary, cwd.as_deref()));
                    RestoreEntry {
                        name: app.name.clone(),
                        kind: app.kind,
                        command,
                        cwd,
                        workspaces: Vec::new(),
                        skip_reason: None,
                    }
                }
                AppKind::Editor => {
                    let mut command = vec![app.binary.clone()];
                    let present: Vec<String> = app
                        .workspaces
                        .iter()
                        .filter(|w| Path::new(w).exists())
                        .cloned()
                        .collect();
                    command.extend(present.clone());
                    RestoreEntry {
                        name: app.name.clone(),
                        kind: app.kind,
                        command,
                        cwd,
                        workspaces: present,
                        skip_reason: None,
                    }
                }
                AppKind::Files | AppKind::Other => {
                    // For commands that start with "flatpak run", preserve the
                    // full command (not just the binary name) so Flatpak apps
                    // are restored correctly.
                    let command = if app.binary == "flatpak"
                        && app.argv.len() > 2
                        && app.argv.get(1).map(|s| s.as_str()) == Some("run")
                    {
                        app.argv.clone()
                    } else {
                        vec![app.binary.clone()]
                    };
                    RestoreEntry {
                        name: app.name.clone(),
                        kind: app.kind,
                        command,
                        cwd,
                        workspaces: Vec::new(),
                        skip_reason: None,
                    }
                }
            }
        })
        .collect()
}

/// Extra CLI flags that pin a terminal's initial directory.
///
/// The spawn also sets `current_dir`, so unknown terminals still open in
/// roughly the right place; these flags make it exact for common ones.
fn terminal_workdir_args(binary: &str, cwd: Option<&str>) -> Vec<String> {
    let Some(cwd) = cwd else { return Vec::new() };
    match binary {
        "gnome-terminal"
        | "gnome-terminal-server"
        | "tilix"
        | "terminator"
        | "xfce4-terminal"
        | "mate-terminal"
        | "lxterminal"
        | "qterminal" => {
            vec![format!("--working-directory={}", cwd)]
        }
        "konsole" => vec!["--workdir".to_string(), cwd.to_string()],
        "alacritty" => vec!["--working-directory".to_string(), cwd.to_string()],
        "kitty" => vec!["--directory".to_string(), cwd.to_string()],
        "foot" | "footclient" => vec!["--working-directory".to_string(), cwd.to_string()],
        "ghostty" => vec![format!("--working-directory={}", cwd)],
        "ptyxis" => vec![format!("--working-directory={}", cwd)],
        "wezterm" => vec!["start".to_string(), "--cwd".to_string(), cwd.to_string()],
        _ => Vec::new(),
    }
}

/// Launch every entry in the plan, detached, continuing past failures.
///
/// Each app is spawned with stdio redirected to `<snapshot>/restore.log`
/// (truncated at the start of the run) so restored apps survive the
/// terminal that ran `warren session restore`.
pub fn execute_restore(
    snapshot: &SessionSnapshot,
    plan: &[RestoreEntry],
    dry_run: bool,
) -> Result<RestoreReport> {
    let mut report = RestoreReport::default();
    if dry_run {
        for entry in plan {
            if let Some(reason) = &entry.skip_reason {
                report.skipped.push((entry.name.clone(), reason.clone()));
            } else {
                report.launched.push(display_command(entry));
            }
        }
        return Ok(report);
    }

    let dir = snapshot_dir(&snapshot.session.name);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create session dir {}", dir.display()))?;
    let log_path = dir.join("restore.log");
    // Truncate (not append) so the log cannot grow across restores.
    std::fs::write(&log_path, "")
        .with_context(|| format!("failed to initialize restore log {}", log_path.display()))?;
    report.log_path = Some(log_path.to_string_lossy().to_string());

    for entry in plan {
        if let Some(reason) = &entry.skip_reason {
            report.skipped.push((entry.name.clone(), reason.clone()));
            continue;
        }
        match spawn_detached(entry, &dir.join("restore.log")) {
            Ok(()) => report.launched.push(entry.name.clone()),
            Err(e) => report.failed.push((entry.name.clone(), e.to_string())),
        }
        // Stagger spawns so the window manager / session bus is not
        // hammered with 20 launches in the same millisecond.
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    Ok(report)
}

fn spawn_detached(entry: &RestoreEntry, log_path: &Path) -> Result<()> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("failed to open restore log {}", log_path.display()))?;
    let err_log = log
        .try_clone()
        .with_context(|| "failed to clone log handle")?;
    let mut cmd = std::process::Command::new(&entry.command[0]);
    if entry.command.len() > 1 {
        cmd.args(&entry.command[1..]);
    }
    if let Some(cwd) = &entry.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err_log))
        .env("WARREN_SESSION_RESTORE", "1");
    // Forget the child: when warren exits it is reparented to init and
    // keeps running — the whole point of session restore.
    cmd.spawn()
        .with_context(|| format!("failed to launch {}", entry.command.join(" ")))?;
    Ok(())
}

fn display_command(entry: &RestoreEntry) -> String {
    if entry.command.len() > 1 {
        format!("{} {}", entry.command[0], entry.command[1..].join(" "))
    } else {
        entry.command[0].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::scan::SessionApp;
    use crate::session::store::{SessionMeta, ShellState, WarrenState};
    use chrono::Utc;

    fn snapshot_with(apps: Vec<SessionApp>) -> SessionSnapshot {
        SessionSnapshot {
            session: SessionMeta {
                name: "test".to_string(),
                created_at: Utc::now(),
                hostname: "h".to_string(),
                desktop: "gnome".to_string(),
                session_type: "wayland".to_string(),
                warren_version: "0.0.0".to_string(),
            },
            shell: ShellState {
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
            },
            warren: WarrenState { instances: vec![] },
            apps,
        }
    }

    #[test]
    fn missing_binary_is_skipped() {
        let snap = snapshot_with(vec![SessionApp {
            name: "no-such-app-xyz".to_string(),
            kind: AppKind::Other,
            binary: "no-such-app-xyz".to_string(),
            cwd: None,
            workspaces: vec![],
            argv: vec![],
        }]);
        let plan = build_restore_plan(&snap);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].skip_reason.is_some());
    }

    #[test]
    fn editor_keeps_only_existing_workspaces() {
        let snap = snapshot_with(vec![SessionApp {
            name: "code".to_string(),
            kind: AppKind::Editor,
            binary: "sh".to_string(), // present on PATH for the planner
            cwd: None,
            workspaces: vec![
                "/tmp".to_string(),
                "/definitely/not/here-xyz-123".to_string(),
            ],
            argv: vec![],
        }]);
        let plan = build_restore_plan(&snap);
        assert_eq!(plan[0].command, vec!["sh".to_string(), "/tmp".to_string()]);
    }

    #[test]
    fn dry_run_has_no_side_effects() {
        let snap = snapshot_with(vec![]);
        let report = execute_restore(&snap, &[], true).unwrap();
        assert!(report.launched.is_empty());
    }
}
