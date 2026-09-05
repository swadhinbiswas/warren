# Contributing to Warren

Thanks for your interest in Warren! This document covers how to build, test,
and contribute to the project.

## Project Overview

Warren is a single Rust binary (`src/main.rs` → `warren`) that installs and
runs fully isolated instances of CLI tools. Isolation is achieved without
root, containers, or daemons:

1. **`src/install/rewriter.rs`** rewrites hardcoded paths inside installer
   scripts so everything lands inside the instance directory.
2. **`src/install/executor.rs`** runs the rewritten installer with a
   sandboxed environment (`$HOME`, `$XDG_*`, `$TMPDIR`, `$PATH`).
3. **`src/instance/launcher.rs`** generates a bash wrapper in
   `~/.local/bin/<alias>` that re-applies the sandboxed environment and
   `exec`s the instance's binary.

### Layout

| Path | Purpose |
| :--- | :--- |
| `src/cli.rs` | clap command-line definition |
| `src/config.rs` | `~/.warren/config.toml` (paths, defaults, ui) |
| `src/instance/` | instance directory layout, metadata, launcher scripts |
| `src/install/` | source parsing, downloading, rewriting, executing, detecting |
| `src/commands/` | one module per subcommand (`dig`, `run`, `ls`, ...) |
| `src/shell/` | shell detection and PATH integration |
| `src/ui/` | colors (theme) and progress bars |

## Prerequisites

- Rust stable (edition 2024), installed via [rustup](https://rustup.rs)
- Linux (Warren is Linux-only by design)

## Building

```bash
cargo build                # debug
cargo build --release      # optimized (strips symbols, LTO)
```

## Running

```bash
cargo run -- dig "curl -fsSL https://example.com/install | bash" --as myapp
cargo run -- ls
cargo run -- run myapp -- --version
```

`warren` refuses to run as root, so use a normal user account.

## Testing

There is no test suite yet — Warren is exercised via manual smoke tests:

1. `cargo run -- dig ./path/to/installer.sh --as test-instance --yes`
2. `cargo run -- ls` and `cargo run -- inspect test-instance`
3. Run the launcher directly: `test-instance`
4. `cargo run -- clone test-instance test-instance-2`
5. `cargo run -- export test-instance` then
   `cargo run -- import test-instance.warren.tar.gz --as test-instance-3`

New features should include a similar manual verification path.

## Quality Gates

Before opening a pull request, make sure both of these pass cleanly:

```bash
cargo build
cargo clippy --all-targets -- -D warnings
```

## Code Style

- Follow the existing patterns: `anyhow::Result` with
  `with_context(|| format!(...))` on I/O operations, `bail!` for early exits.
- Keep user-facing output in `src/ui/theme.rs` helpers (never raw
  `console::style` calls in commands). Everything is written to *stderr*
  so stdout stays clean for piping.
- Respect the global color switch: honor `NO_COLOR` and `CLICOLOR_FORCE`
  (handled once in `src/main.rs`), and use the `Theme` struct rather than
  hardcoding ANSI codes.
- No comments unless they explain *why*; keep them sparse.

## Alias Validation

Aliases must match `[a-z0-9][a-z0-9-]*[a-z0-9]` (no consecutive hyphens,
max 64 chars) — see `src/instance/mod.rs::validate_alias`. Any new command
that accepts an alias should call it.

## Security Notes

- `import` validates every tar entry and rejects path traversal (`..`)
  before extracting anything.
- Installer scripts are executed as the current user with a rewritten
  environment — treat them like any untrusted code you run.
- Never make Warren require root, ever.

## Commit Messages

Keep them concise and imperative: `Fix import path traversal`, `Colorize ls output`.

## License

By contributing you agree that your contributions are licensed under the
[MIT License](LICENSE.md).
