# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- App wrapping: `warren dig` now installs the same graphical app many
  times with separate accounts — `flatpak:<id>` (or a bare Flathub id
  like `com.discordapp.Discord`), `snap:<name>`, `apt:`/`dnf:`/
  `pacman:`/`apk:`/`zypper:`/`xbps:`/`eopkg:`/`nix:`/`brew:`/`pkg:`/
  `emerge:` system packages, `app:<name>` smart lookup, and
  `desktop:<id>` / bare `foo.desktop` entries. Wrapping is rootless and
  zero-copy: the host binary is reused, each alias keeps private data.
- `warren dig --gui / --no-gui` to override GUI/CLI detection.
- GUI instances get a `warren-<alias>.desktop` app-grid entry (removed
  on `rm`, regenerated on `clone`/`import`/`update`), keep host
  Wayland/DBus/audio working (host `$XDG_RUNTIME_DIR` preserved), and
  `warren run` launches them detached.
- `warren ls` shows a `KIND` (`gui`/`cli`) column; `warren inspect`
  shows the launch command, desktop entry, and wrap-mode disk note.
- `warren update` re-resolves wrapped apps without touching account data.
- `CONTRIBUTING.md`, `SECURITY.md`, and `LICENSE.md` documentation files.
- `warren env` and `warren shell info` commands.
- Color and progress output now respect `NO_COLOR`, `CLICOLOR_FORCE`,
  terminal detection, and the `ui.color` / `ui.progress` config options.
- Streaming installer downloads with a progress bar, HTTP timeouts, and a
  proper `User-Agent`.
- Confirmation prompt for `warren update` (skippable with `--yes`).
- Warning when an installer finishes without placing an executable in the
  instance's `bin/`.

### Fixed
- **Security:** `warren import` no longer extracts tar archives with path
  traversal (`..`) entries — every entry is validated before unpacking.
- **Security:** `warren import --as <alias>` no longer clobbers an existing
  instance whose name matches the archive's internal root directory.
- `warren clone` now preserves symlinks instead of following (or failing
  on) them.
- Installers now run with the instance directory as their working
  directory, so scripts using relative paths behave predictably.
- Version detection (`--version`) now runs inside the instance's sandboxed
  environment instead of the real `$HOME`.
- `warren shell install` creates missing rc-file parent directories (fixes
  fish/nushell setups without an existing config dir) and correctly
  prefers `~/.bashrc` over `~/.bash_profile`.
- Config files with a partial `[paths]` table no longer fail to parse.
- Removed dead code; `cargo build` and `cargo clippy` are clean.

## [0.1.6] - 2026-06-26

### Added
- Initial public release. Support for `dig`, `run`, `ls`, `inspect`, `rm`,
  `update`, `clone`, `export`, `import`, `env`, and `shell` commands.
