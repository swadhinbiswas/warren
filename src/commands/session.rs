use anyhow::Result;

use crate::cli::SessionAction;
use crate::config::WarrenConfig;
use crate::session::restore::{build_restore_plan, execute_restore};
use crate::session::store::{
    collect_snapshot, default_snapshot_name, latest_snapshot, list_snapshots, load_snapshot,
    prune_snapshots, remove_snapshot, save_snapshot,
};
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme, action: &SessionAction) -> Result<()> {
    match action {
        SessionAction::Save { name, keep } => save(config, theme, name.as_deref(), *keep).await,
        SessionAction::Restore { name, dry_run } => restore(theme, name.as_deref(), *dry_run).await,
        SessionAction::Ls => ls(theme).await,
        SessionAction::Rm { name, yes } => rm(theme, name, *yes).await,
        SessionAction::Prune { keep } => prune(config, theme, *keep).await,
    }
}

async fn save(
    config: &WarrenConfig,
    theme: &Theme,
    name: Option<&str>,
    keep: Option<usize>,
) -> Result<()> {
    config.ensure_dirs()?;
    let keep = keep.unwrap_or(config.sessions.keep).max(1);
    let name = name
        .map(|s| s.to_string())
        .unwrap_or_else(default_snapshot_name);
    theme.header(&format!("saving session {}", name));
    let snapshot = collect_snapshot(&name, config)?;
    let app_count = snapshot.apps.len();
    let (path, pruned) = save_snapshot(&snapshot, keep)?;
    theme.success(&format!(
        "Saved {} app{} to {}",
        app_count,
        if app_count == 1 { "" } else { "s" },
        crate::instance::InstanceMetadata::display_path(&path.to_string_lossy()),
    ));
    for app in snapshot.apps.iter().take(10) {
        let detail = match &app.cwd {
            Some(cwd) => format!(
                "{} ({}) — {}",
                app.binary,
                app.kind,
                crate::instance::InstanceMetadata::display_path(cwd)
            ),
            None => format!("{} ({})", app.binary, app.kind),
        };
        theme.dim(&detail);
    }
    if snapshot.apps.len() > 10 {
        theme.dim(&format!("… and {} more", snapshot.apps.len() - 10));
    }
    if !pruned.is_empty() {
        theme.dim(&format!(
            "Pruned {} old snapshot{} (keeping {}): {}",
            pruned.len(),
            if pruned.len() == 1 { "" } else { "s" },
            keep,
            pruned.join(", "),
        ));
    } else {
        theme.dim(&format!(
            "Keeping last {} snapshot{}.",
            keep,
            if keep == 1 { "" } else { "s" }
        ));
    }
    theme.blank();
    theme.dim("Recover with:  warren session restore");
    theme.blank();
    Ok(())
}

async fn restore(theme: &Theme, name: Option<&str>, dry_run: bool) -> Result<()> {
    let snapshot = match name {
        Some(n) => load_snapshot(n)?,
        None => latest_snapshot()?,
    };
    let plan = build_restore_plan(&snapshot);
    theme.header(&format!(
        "{} session {}{}",
        if dry_run { "planning" } else { "restoring" },
        snapshot.session.name,
        if name.is_none() { " (latest)" } else { "" },
    ));
    if plan.is_empty() {
        theme.warn("Snapshot contains no apps to restore.");
        return Ok(());
    }
    if dry_run {
        for entry in &plan {
            match &entry.skip_reason {
                Some(reason) => theme.dim(&format!("SKIP  {} — {}", entry.name, reason)),
                None => {
                    let cmd = if entry.command.len() > 1 {
                        format!("{} {}", entry.command[0], entry.command[1..].join(" "))
                    } else {
                        entry.command[0].clone()
                    };
                    let where_ = entry.cwd.as_deref().unwrap_or("~");
                    let mut detail = format!("{} [{}] (in {})", cmd, entry.kind, where_);
                    if !entry.workspaces.is_empty() {
                        detail.push_str(&format!("  workspaces: {}", entry.workspaces.join(", ")));
                    }
                    theme.kv(&entry.name, &detail);
                }
            }
        }
        theme.blank();
        return Ok(());
    }
    let report = execute_restore(&snapshot, &plan, false)?;
    for launched in &report.launched {
        theme.success(&format!("Launched: {}", launched));
    }
    for (name, reason) in &report.skipped {
        theme.warn(&format!("Skipped {}: {}", name, reason));
    }
    for (name, err) in &report.failed {
        theme.error(&format!("Failed {}: {}", name, err));
    }
    theme.blank();
    if !report.launched.is_empty() {
        theme.dim("Tip: enable your browser's “continue where you left off”");
        theme.dim("so tabs come back alongside the reopened windows.");
    }
    if let Some(log) = &report.log_path {
        theme.dim(&format!(
            "Restore log: {}",
            crate::instance::InstanceMetadata::display_path(log)
        ));
    }
    if !snapshot.warren.instances.is_empty() {
        theme.dim(&format!(
            "Warren instances at save time: {}",
            snapshot.warren.instances.join(", ")
        ));
    }
    theme.blank();
    Ok(())
}

async fn ls(theme: &Theme) -> Result<()> {
    let snapshots = list_snapshots()?;
    if snapshots.is_empty() {
        theme.dim("No saved sessions.");
        theme.dim("Run 'warren session save' to capture your current workspace.");
        return Ok(());
    }
    theme.blank();
    let header = console::Style::new().bold().dim();
    eprintln!(
        "  {:<24}{:<22}{:<8}{}",
        header.apply_to("NAME"),
        header.apply_to("SAVED"),
        header.apply_to("APPS"),
        header.apply_to("SIZE"),
    );
    for snap in &snapshots {
        let age = snap.created_at.format("%Y-%m-%d %H:%M").to_string();
        eprintln!(
            "  {:<24}{:<22}{:<8}{}",
            theme.brand.apply_to(&snap.name),
            theme.value.apply_to(&age),
            snap.app_count.to_string(),
            theme.dim.apply_to(&format!("{} B", snap.size_bytes)),
        );
    }
    theme.blank();
    theme.dim(&format!(
        "{} snapshot{}  •  {}  •  recover with 'warren session restore'",
        snapshots.len(),
        if snapshots.len() == 1 { "" } else { "s" },
        crate::instance::InstanceMetadata::display_path(
            &WarrenConfig::sessions_dir().to_string_lossy()
        ),
    ));
    theme.blank();
    Ok(())
}

async fn rm(theme: &Theme, name: &str, yes: bool) -> Result<()> {
    if !yes {
        use dialoguer::Confirm;
        let confirm = Confirm::new()
            .with_prompt(format!("Delete saved session '{}'?", name))
            .default(false)
            .interact()?;
        if !confirm {
            theme.warn("Aborted.");
            return Ok(());
        }
    }
    remove_snapshot(name)?;
    theme.success(&format!("Removed session '{}'", name));
    Ok(())
}

async fn prune(config: &WarrenConfig, theme: &Theme, keep: Option<usize>) -> Result<()> {
    let keep = keep.unwrap_or(config.sessions.keep).max(1);
    let pruned = prune_snapshots(keep)?;
    if pruned.is_empty() {
        theme.dim(&format!("Nothing to prune (keeping {}).", keep));
    } else {
        theme.success(&format!(
            "Pruned {} snapshot{}: {}",
            pruned.len(),
            if pruned.len() == 1 { "" } else { "s" },
            pruned.join(", ")
        ));
    }
    Ok(())
}
