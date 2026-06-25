use anyhow::Result;
use chrono::Utc;

use crate::config::WarrenConfig;
use crate::instance::InstanceMetadata;
use crate::ui::theme::Theme;

pub async fn execute(config: &WarrenConfig, theme: &Theme) -> Result<()> {
    let instances_dir = &config.paths.instances_dir;
    if !instances_dir.exists() {
        theme.dim("No instances found.");
        theme.dim("Run 'warren dig <source> --as <alias>' to get started.");
        return Ok(());
    }
    let mut entries: Vec<InstanceMetadata> = Vec::new();
    for entry in std::fs::read_dir(instances_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() { continue; }
        let metadata_path = entry.path().join("metadata.toml");
        if metadata_path.exists() {
            if let Ok(meta) = InstanceMetadata::load(&metadata_path) {
                entries.push(meta);
            }
        }
    }
    if entries.is_empty() {
        theme.dim("No instances found.");
        theme.dim("Run 'warren dig <source> --as <alias>' to get started.");
        return Ok(());
    }
    entries.sort_by(|a, b| a.instance.alias.cmp(&b.instance.alias));
    theme.blank();
    eprintln!("  {:<20}{:<14}{:<12}{}",
        console::style("ALIAS").bold().dim(),
        console::style("APP").bold().dim(),
        console::style("VERSION").bold().dim(),
        console::style("CREATED").bold().dim(),
    );
    for meta in &entries {
        let age = format_relative_time(&meta.instance.created_at);
        let version = meta.instance.version.as_deref().unwrap_or("-");
        eprintln!("  {:<20}{:<14}{:<12}{}",
            console::style(&meta.instance.alias).cyan(),
            meta.instance.app_name,
            version,
            console::style(&age).dim(),
        );
    }
    theme.blank();
    theme.dim(&format!("{} instance{}  •  {}",
        entries.len(),
        if entries.len() == 1 { "" } else { "s" },
        InstanceMetadata::display_path(&instances_dir.to_string_lossy()),
    ));
    theme.blank();
    Ok(())
}

fn format_relative_time(dt: &chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    let seconds = diff.num_seconds();
    if seconds < 60 { return "just now".to_string(); }
    let minutes = diff.num_minutes();
    if minutes < 60 { return format!("{} min{} ago", minutes, if minutes == 1 { "" } else { "s" }); }
    let hours = diff.num_hours();
    if hours < 24 { return format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" }); }
    let days = diff.num_days();
    if days < 7 { return format!("{} day{} ago", days, if days == 1 { "" } else { "s" }); }
    let weeks = days / 7;
    if weeks < 5 { return format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" }); }
    let months = days / 30;
    if months < 12 { return format!("{} month{} ago", months, if months == 1 { "" } else { "s" }); }
    let years = days / 365;
    format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
}
