use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    Nushell,
    Sh,
    Unknown(String),
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellType::Bash => write!(f, "bash"),
            ShellType::Zsh => write!(f, "zsh"),
            ShellType::Fish => write!(f, "fish"),
            ShellType::Nushell => write!(f, "nushell"),
            ShellType::Sh => write!(f, "sh"),
            ShellType::Unknown(s) => write!(f, "{}", s),
        }
    }
}

pub fn detect_shell() -> ShellType {
    if let Ok(shell) = env::var("SHELL") {
        return parse_shell_path(&shell);
    }
    ShellType::Unknown("unknown".to_string())
}

fn parse_shell_path(path: &str) -> ShellType {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "bash" => ShellType::Bash,
        "zsh" => ShellType::Zsh,
        "fish" => ShellType::Fish,
        "nu" | "nushell" => ShellType::Nushell,
        "sh" | "dash" => ShellType::Sh,
        other => ShellType::Unknown(other.to_string()),
    }
}
