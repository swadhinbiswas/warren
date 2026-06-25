use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub fn download_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(
            "     [{bar:44.cyan/dim}] {bytes}/{total_bytes} {msg}"
        )
        .unwrap()
        .progress_chars("━━╌"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan}  {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

pub fn step_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(
            "     [{bar:44.green/dim}] {percent}% {msg}"
        )
        .unwrap()
        .progress_chars("━━╌"),
    );
    pb
}
