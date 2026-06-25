use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};

use super::detect::ShellType;

pub fn install_shell_integration(shell: &ShellType, bin_dir: &Path) -> Result<()> {
    let rc_file = get_rc_file(shell)?;
    let line = get_path_line(shell, bin_dir);
    if rc_file.exists() {
        let content = std::fs::read_to_string(&rc_file)
            .with_context(|| format!("failed to read {}", rc_file.display()))?;
        if content.contains("# warren") || content.contains(&line) {
            tracing::info!(file = %rc_file.display(), "shell integration already installed");
            return Ok(());
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rc_file)
        .with_context(|| format!("failed to open {} for writing", rc_file.display()))?;
    use std::io::Write;
    writeln!(file, "\n# warren — CLI instance runtime")?;
    writeln!(file, "{}", line)?;
    tracing::info!(file = %rc_file.display(), "installed shell integration");
    Ok(())
}

fn get_rc_file(shell: &ShellType) -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    match shell {
        ShellType::Bash => {
            let bashrc = home.join(".bashrc");
            if bashrc.exists() { Ok(bashrc) } else { Ok(home.join(".bash_profile")) }
        }
        ShellType::Zsh => Ok(home.join(".zshrc")),
        ShellType::Fish => Ok(home.join(".config").join("fish").join("config.fish")),
        ShellType::Nushell => Ok(home.join(".config").join("nushell").join("env.nu")),
        ShellType::Sh => Ok(home.join(".profile")),
        ShellType::Unknown(name) => bail!("unsupported shell '{}' for shell integration", name),
    }
}

fn get_path_line(shell: &ShellType, bin_dir: &Path) -> String {
    let dir = bin_dir.to_string_lossy();
    match shell {
        ShellType::Fish => format!("fish_add_path {}", dir),
        ShellType::Nushell => format!("$env.PATH = ($env.PATH | prepend \"{}\")", dir),
        _ => format!("export PATH=\"{}:$PATH\"", dir),
    }
}
