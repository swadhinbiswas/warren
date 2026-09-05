#!/usr/bin/env bash
#
# Warren — automated installer (see --help / --from-source for details).
# Raw GitHub one-liner:
#   curl -fsSL https://raw.githubusercontent.com/swadhinbiswas/warren/main/install.sh | bash

set -euo pipefail

REPO="swadhinbiswas/warren"
REPO_URL="https://github.com/${REPO}"
RAW_URL="https://raw.githubusercontent.com/${REPO}"
DEFAULT_REF="main"

SOURCE_REF="${WARREN_REF:-$DEFAULT_REF}"
BIN_DIR="${WARREN_BIN_DIR:-$HOME/.local/bin}"
FROM_SOURCE=false

usage() {
    cat <<'EOF'
  warren — automated installer

  One-liner (raw GitHub link):
    curl -fsSL https://raw.githubusercontent.com/swadhinbiswas/warren/main/install.sh | bash

  Or download it and pass options:
    curl -fsSL https://raw.githubusercontent.com/swadhinbiswas/warren/main/install.sh -o install.sh
    bash install.sh --from-source

  Options:
    --from-source    Always build from source (skips pre-built binary)
    --help           Show this help

  Environment:
    WARREN_REF       Git ref to build from (default: main; tags need the v prefix, e.g. v0.1.6)
    WARREN_BIN_DIR   Install directory (default: $HOME/.local/bin)
EOF
}

for arg in "$@"; do
    case "$arg" in
        --from-source) FROM_SOURCE=true ;;
        -h|--help) usage; exit 0 ;;
        *) echo "  ✗  Unknown option: $arg" >&2; usage; exit 1 ;;
    esac
done

# Colors (honor NO_COLOR and non-TTY stderr)
if [[ -t 2 && -z "${NO_COLOR:-}" && "${CLICOLOR:-1}" != "0" ]]; then
    CYAN='\033[0;36m'; GREEN='\033[0;32m'; RED='\033[0;31m'
    YELLOW='\033[1;33m'; BOLD='\033[1m'; NC='\033[0m'
else
    CYAN=''; GREEN=''; RED=''; YELLOW=''; BOLD=''; NC=''
fi

info()  { echo -e "  ${CYAN}ℹ${NC}  $*"; }
ok()    { echo -e "  ${GREEN}✓${NC}  $*"; }
warn()  { echo -e "  ${YELLOW}!${NC}  $*"; }
err()   { echo -e "  ${RED}✗${NC}  $*" >&2; }

banner() {
    echo -e "  ${CYAN}${BOLD}warren${NC}  installer"
    echo
}

die_root() {
    err "Warren refuses to be installed or run as root."
    err "Please run this script as your normal user."
    exit 1
}

die_os() {
    err "Warren currently only supports Linux."
    exit 1
}

die_no_cargo() {
    err "Cargo (Rust) is required to build Warren from source."
    info "Install Rust first:"
    echo "     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
}

require_cargo() {
    command -v cargo >/dev/null 2>&1 || die_no_cargo
}

require_curl() {
    command -v curl >/dev/null 2>&1 || {
        err "curl is required for the one-line installer."
        exit 1
    }
}

# Detect the release asset triple, or empty when no pre-built binary exists.
detect_triple() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *) echo "" ;;
    esac
}

# Install the pre-built release binary. Returns 0 on success, 1 when no
# release is available for this architecture.
install_prebuilt() {
    local triple url
    triple="$(detect_triple)"
    [ -n "$triple" ] || return 1
    url="${REPO_URL}/releases/latest/download/warren-linux-${triple}.tar.gz"
    info "Downloading pre-built binary (${triple})..."
    if curl --output /dev/null --silent --head --fail "$url"; then
        local tmp
        tmp="$(mktemp -d)"
        if ! curl -fsSL "$url" | tar -xz -C "$tmp" 2>/dev/null; then
            rm -rf "$tmp"
            err "Failed to download pre-built binary from ${url}"
            return 1
        fi
        mv "$tmp/warren" "$BIN_DIR/warren"
        chmod +x "$BIN_DIR/warren"
        rm -rf "$tmp"
        return 0
    fi
    warn "No pre-built release for ${triple} yet; building from source instead."
    return 1
}

# Build Warren from source: either the current directory (if we are inside
# the repo) or the GitHub source tarball for $SOURCE_REF.
install_from_source() {
    require_cargo
    if [ -f "Cargo.toml" ] && grep -qE 'name[[:space:]]*=[[:space:]]*"warren-cli"' Cargo.toml; then
        info "Installing from current source directory..."
        cargo install --path . --root "$(dirname "$BIN_DIR")" --locked
        return 0
    fi
    info "Fetching Warren source (${SOURCE_REF}) from GitHub..."
    local ref_path
    if [ "$SOURCE_REF" = "$DEFAULT_REF" ]; then
        ref_path="refs/heads/${SOURCE_REF}"
    else
        ref_path="refs/tags/${SOURCE_REF}"
    fi
    curl -fsSL "${REPO_URL}/archive/${ref_path}.tar.gz" -o "$TMP_DIR/warren.tar.gz" || {
        err "Failed to download source tarball from ${REPO_URL}/archive/${ref_path}.tar.gz"
        exit 1
    }
    tar -xzf "$TMP_DIR/warren.tar.gz" -C "$TMP_DIR"
    local srcdir
    srcdir="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'warren-*' | head -n1)"
    [ -n "$srcdir" ] || { err "Could not locate the source tree in the archive."; exit 1; }
    info "Building Warren (this can take a few minutes)..."
    cargo install --path "$srcdir" --root "$(dirname "$BIN_DIR")" --locked
}

# ---------------------------------------------------------------------------

banner

[ "$(id -u)" -ne 0 ] || die_root
[ "$(uname -s)" = "Linux" ] || die_os
require_curl

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$BIN_DIR"

if $FROM_SOURCE || ! install_prebuilt; then
    install_from_source
fi

ok "Warren binary installed at ${BIN_DIR}/warren"

# Run Warren's built-in shell integration (bash, zsh, fish, nushell)
export PATH="$BIN_DIR:$PATH"
if command -v warren >/dev/null 2>&1; then
    echo
    warren shell install
else
    err "Failed to locate warren executable after installation."
    exit 1
fi

echo
ok "Warren installation complete!"
info "You may need to restart your terminal or source your shell config."
info "Run 'warren --help' to get started."
