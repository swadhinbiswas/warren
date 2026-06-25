<h1 align="center">Warren</h1>

<p align="center">
  <img src="assets/logo.png" alt="Warren Mascot" width="300">
</p>

<p align="center">
  <strong>Install any CLI tool unlimited times. Every instance is its own world.</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/warren-cli"><img src="https://img.shields.io/crates/v/warren-cli.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/warren-cli"><img src="https://img.shields.io/docsrs/warren-cli" alt="Docs"></a>
  <a href="https://github.com/swadhinbiswas/warren/blob/main/LICENSE"><img src="https://img.shields.io/github/license/swadhinbiswas/warren.svg" alt="License"></a>
</p>

---

**Warren** is a production-grade, rootless, zero-daemon CLI runtime. It lets you install and run unlimited, completely isolated instances of any CLI or TUI application on your Linux machine. 

Like a rabbit warren, it creates a network of isolated tunnels—each self-contained yet sharing the same ground. Fast, rootless, and invisible infrastructure.

## Why Warren?

Ever needed to log into two different GitHub accounts using the `gh` CLI? Or test a beta version of a tool without breaking your stable setup? Or maintain separate configurations for work and personal projects?

With Warren, you can:
```bash
warren dig gh --as gh-work
warren dig gh --as gh-personal

gh-work        # Logs into your company GitHub
gh-personal    # Logs into your personal GitHub
```
Both instances behave independently. They share the host machine but nothing else.

### Non-Goals
Warren is **not** a container runtime (like Docker), a virtual machine, or a privilege escalation tool. 
It stays lightweight (single binary), rootless (no sudo ever), zero-daemon (no background processes), and incredibly fast (<50ms startup overhead).

## Features

- **Isolated Filesystems:** Each instance gets its own `home/`, `config/`, `cache/`, `data/`, and `tmp/` directories.
- **Installer Rewriting:** Automatically rewrites paths inside installation scripts on-the-fly to keep them contained.
- **Rootless by Design:** Will absolutely refuse to run as root.
- **Universal Shell Support:** Works seamlessly across Bash, Zsh, Fish, and Nushell.
- **Zero Overhead:** No daemons or containers. Just a thin, native bash launcher script.

## Installation

### Method 1: The One-Liner (Recommended)

Run the automated install script. This will download the latest binary and automatically configure your shell (`bash`, `zsh`, `fish`, or `nushell`).

```bash
curl -fsSL https://raw.githubusercontent.com/swadhinbiswas/warren/main/install.sh | bash
```

### Method 2: Via Cargo (crates.io)

If you already have Rust installed, you can build and install Warren directly from crates.io:

```bash
cargo install warren-cli
warren shell install  # Sets up your PATH
```

## Quick Start

**1. Install a new instance from a remote script:**
```bash
warren dig "curl -fsSL https://opencode.ai/install | bash" --as opencode-work
```

**2. See what you've installed:**
```bash
$ warren ls

  ALIAS            APP         VERSION    CREATED
  opencode-work    opencode    0.4.2      just now
```

**3. Run your instance:**
```bash
opencode-work --version
# or 
warren run opencode-work -- --version
```

**4. Inspect an instance's isolated footprint:**
```bash
warren inspect opencode-work
```

## How it Works

Warren achieves total isolation without kernel namespaces or OverlayFS through three simple levers:

1. **Intelligent Rewriting:** When you install a tool via a bash script, Warren intercepts it. It scans for hardcoded paths (like `/usr/local/bin` or `~/.config`) and rewrites them to point inside the instance's private directory (`~/.warren/instances/<alias>`).
2. **Environment Injection:** Warren generates a lightweight wrapper script in `~/.local/bin`. When you run your alias, this script forcibly overrides `$HOME`, `$XDG_CONFIG_HOME`, `$XDG_DATA_HOME`, and `$TMPDIR`. 
3. **Execution:** The target application boots up, entirely unaware that its "home directory" is actually a sandbox inside `~/.warren/`.

## Command Reference

| Command | Description |
| :--- | :--- |
| `warren dig <source> --as <alias>` | Install a new isolated instance |
| `warren run <alias>` | Run an instance explicitly |
| `warren ls` | List all installed instances |
| `warren inspect <alias>` | View paths, version, and disk usage |
| `warren clone <src> <dest>` | Deep copy an instance |
| `warren export <alias>` | Export an instance to a portable `.tar.gz` archive |
| `warren import <archive>` | Import a previously exported instance |
| `warren update <alias>` | Re-run the installer to update the binary |
| `warren rm <alias>` | Delete an instance and all its isolated data |
| `warren shell install` | Add Warren to your terminal's PATH |

## Shell Support

Warren automatically detects your shell and configures your `$PATH`. It natively supports:
- **Bash** (`~/.bashrc` / `~/.bash_profile`)
- **Zsh** (`~/.zshrc`)
- **Fish** (`~/.config/fish/config.fish` via `fish_add_path`)
- **Nushell** (`env.nu` via `$env.PATH`)

## License

MIT License. See [LICENSE](LICENSE) for details.
