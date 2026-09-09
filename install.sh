#!/usr/bin/env bash
#
# Warren — beautiful automated installer.
#
# Install immediately with the one-liner:
#   curl -fsSL https://warren.run/install.sh | bash
#
# Or download first, then install (downloading alone installs nothing):
#   curl -fsSL https://warren.run/install.sh -o install.sh
#   bash install.sh                 # install
#   bash install.sh --from-source   # always build from source
#   bash install.sh --version v0.1.6  # install a specific version

set -euo pipefail

REPO="swadhinbiswas/warren"
REPO_URL="https://github.com/${REPO}"
DEFAULT_REF="main"

SOURCE_REF="${WARREN_REF:-$DEFAULT_REF}"
BIN_DIR="${WARREN_BIN_DIR:-${HOME}/.local/bin}"
FROM_SOURCE=false
VERSION_TAG=""

for arg in "$@"; do
    case "$arg" in
        --from-source) FROM_SOURCE=true ;;
        --version)
            shift
            VERSION_TAG="${1:-}"
            [ -n "$VERSION_TAG" ] || { echo "  ✗  --version requires a value (e.g., v0.1.6)" >&2; exit 1; }
            SOURCE_REF="$VERSION_TAG"
            ;;
        -v|--version-flag)
            echo "warren-installer"
            exit 0
            ;;
        -h|--help) ;;
        --uninstall)
            echo "  To uninstall Warren:"
            echo "    rm -f ${BIN_DIR}/warren"
            echo "    rm -rf ~/.warren"
            echo "    # Also remove the 'warren' line from your shell rc file"
            exit 0
            ;;
        *) echo "  ✗  Unknown option: $arg" >&2; echo "     Run 'bash install.sh --help' for usage." >&2; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Pretty output (colors auto-disable on non-TTY or NO_COLOR=1)
# ---------------------------------------------------------------------------

if [[ -t 2 && -z "${NO_COLOR:-}" && "${CLICOLOR:-1}" != "0" ]]; then
    CYAN='\033[0;36m'; MAGENTA='\033[0;35m'; GREEN='\033[0;32m'
    RED='\033[0;31m'; YELLOW='\033[1;33m'; DIM='\033[2m'
    BOLD='\033[1m'; NC='\033[0m'
else
    CYAN=''; MAGENTA=''; GREEN=''; RED=''; YELLOW=''; DIM=''; BOLD=''; NC=''
fi

if [[ -t 2 ]]; then
    CURL_FLAGS=(-fSL --progress-bar)
else
    CURL_FLAGS=(-fsSL)
fi

rule() {
    local width="${1:-54}" line
    printf -v line '%*s' "$width" ''
    echo -e "  ${DIM}${line// /─}${NC}"
}

banner() {
    echo
    echo -e "  ${MAGENTA}${BOLD} __      __   ___   ___  ___  ___  _  _ ${NC}"
    echo -e "  ${MAGENTA}${BOLD} \ \    / /  / _ \ | _ \| _ \/ __|| \| |${NC}"
    echo -e "  ${MAGENTA}${BOLD}  \ \/\/ /  | (_) ||   /|   /\__ \| .\` |${NC}"
    echo -e "  ${MAGENTA}${BOLD}   \_/\_/    \__,_||_|_\|_|_\|___/|_|\_|${NC}"
    echo -e "  ${DIM}Install any CLI tool unlimited times. Every instance is its own world.${NC}"
    rule 54
}

step() {
    echo
    echo -e "  ${CYAN}${BOLD}▸ Step $1/${TOTAL_STEPS}:${NC} ${BOLD}$2${NC}"
}

info()  { echo -e "  ${CYAN}ℹ${NC}  $*"; }
ok()    { echo -e "  ${GREEN}✓${NC}  $*"; }
warn()  { echo -e "  ${YELLOW}!${NC}  $*"; }
err()   { echo -e "  ${RED}✗${NC}  $*" >&2; }
dim()   { echo -e "  ${DIM}$*${NC}"; }

usage() {
    banner
    cat <<'EOF'
  Usage:
    curl -fsSL https://warren.run/install.sh | bash
    curl -fsSL https://warren.run/install.sh -o install.sh
    bash install.sh [OPTIONS]

  NOTE: downloading with -o only saves the file — run `bash install.sh`
  afterwards to actually install Warren.

  Options:
    --from-source    Always build from source (skips pre-built binary)
    --version TAG    Install a specific version (e.g., v0.1.6)
    --uninstall      Show uninstall instructions
    --help           Show this help

  Environment:
    WARREN_REF       Git ref to build from (default: main; tags need the v prefix, e.g. v0.1.6)
    WARREN_BIN_DIR   Install directory (default: $HOME/.local/bin)
EOF
}

# ---------------------------------------------------------------------------
# Spinner (animated on TTY, plain lines otherwise)
# ---------------------------------------------------------------------------

_SPINNER_PID=""
_SPIN_MSG=""
_SPIN_START=0

spinner_start() {
    _SPIN_MSG="$1"
    _SPIN_START="$(date +%s)"
    if [[ -t 2 ]]; then
        (
            while :; do
                for frame in '⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏'; do
                    printf '\r  %s  %s' "$frame" "$_SPIN_MSG" >&2
                    sleep 0.08
                done
            done
        ) &
        _SPINNER_PID=$!
        disown 2>/dev/null || true
    else
        echo "  …  ${_SPIN_MSG}" >&2
    fi
}

spinner_stop() {
    local status="$1" msg="$2" elapsed=0
    elapsed=$(( $(date +%s) - _SPIN_START ))
    if [[ -n "$_SPINNER_PID" ]]; then
        kill "$_SPINNER_PID" 2>/dev/null || true
        wait "$_SPINNER_PID" 2>/dev/null || true
        _SPINNER_PID=""
        if [[ "$status" == ok ]]; then
            printf '\r  %b✓%b  %s %b(%ds)%b\n' "$GREEN" "$NC" "$msg" "$DIM" "$elapsed" "$NC" >&2
        else
            printf '\r  %b✗%b  %s\n' "$RED" "$NC" "$msg" >&2
        fi
    elif [[ "$status" == ok ]]; then
        echo -e "  ${GREEN}✓${NC}  $msg (${elapsed}s)" >&2
    else
        echo -e "  ${RED}✗${NC}  $msg" >&2
    fi
}

spinner_cleanup() {
    if [[ -n "${_SPINNER_PID:-}" ]]; then
        kill "$_SPINNER_PID" 2>/dev/null || true
        wait "$_SPINNER_PID" 2>/dev/null || true
        _SPINNER_PID=""
        printf '\n' >&2
    fi
}

# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

die_root() {
    err "Warren refuses to be installed or run as root."
    err "Please run this script as your normal user."
    exit 1
}

die_os() {
    err "Warren currently only supports Linux (found: $(uname -s))."
    exit 1
}

die_no_cargo() {
    err "Cargo (Rust) is required to build Warren from source."
    info "Install Rust first:"
    echo "     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
}

die_no_home() {
    err "Could not determine your home directory."
    err "Set the HOME environment variable and try again."
    exit 1
}

require_cargo() {
    command -v cargo >/dev/null 2>&1 || die_no_cargo
}

require_curl() {
    command -v curl >/dev/null 2>&1 || {
        err "curl is required for the installer."
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

latest_tag() {
    curl -fsSL --max-time 8 -o /dev/null -w '%{url_effective}' "${REPO_URL}/releases/latest" 2>/dev/null \
        | grep -o 'tag/[^/]*$' | cut -d/ -f2 || true
}

short_home() {
    case "$1" in
        "${HOME}"*) echo "~${1#$HOME}" ;;
        *) echo "$1" ;;
    esac
}

# ---------------------------------------------------------------------------
# Install paths
# ---------------------------------------------------------------------------

# Install the pre-built release binary. Returns 0 on success, 1 when no
# release is available (caller falls back to a source build).
install_prebuilt() {
    local triple url
    triple="$(detect_triple)"
    [ -n "$triple" ] || return 1

    # If a specific version was requested, use that instead of latest
    local release_path="latest"
    if [ -n "$VERSION_TAG" ]; then
        release_path="tags/${VERSION_TAG}"
    fi

    url="${REPO_URL}/releases/${release_path}/download/warren-linux-${triple}.tar.gz"
    info "Downloading pre-built binary (${triple})…"
    if curl --output /dev/null --silent --head --fail "$url" 2>/dev/null; then
        local tmp
        tmp="$(mktemp -d)"
        spinner_start "Fetching warren-linux-${triple}.tar.gz"
        if ! curl "${CURL_FLAGS[@]}" "$url" 2>/dev/null | tar -xz -C "$tmp" 2>/dev/null; then
            spinner_stop fail "Download failed"
            rm -rf "$tmp"
            err "Failed to download pre-built binary from ${url}"
            return 1
        fi
        spinner_stop ok "Download complete"
        [ -f "$tmp/warren" ] || { err "Archive did not contain a 'warren' binary"; rm -rf "$tmp"; return 1; }
        mv "$tmp/warren" "$BIN_DIR/warren"
        chmod +x "$BIN_DIR/warren"
        rm -rf "$tmp"
        # Never install a binary that cannot run (wrong arch, truncated
        # download): fall back to a source build instead.
        if ! "$BIN_DIR/warren" --version >/dev/null 2>&1; then
            rm -f "$BIN_DIR/warren"
            err "Pre-built binary failed its smoke test; building from source instead."
            return 1
        fi
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
        info "Installing from current source directory…"
        spinner_start "Building Warren with cargo (this can take a few minutes)"
        if ! cargo install --path . --root "$(dirname "$BIN_DIR")" --locked --quiet; then
            spinner_stop fail "Cargo build failed"
            return 1
        fi
        spinner_stop ok "Build complete"
        return 0
    fi
    info "Fetching Warren source (${SOURCE_REF}) from GitHub…"
    local ref_path
    if [ "$SOURCE_REF" = "$DEFAULT_REF" ]; then
        ref_path="refs/heads/${SOURCE_REF}"
    else
        ref_path="refs/tags/${SOURCE_REF}"
    fi
    spinner_start "Downloading source tarball"
    if ! curl "${CURL_FLAGS[@]}" "${REPO_URL}/archive/${ref_path}.tar.gz" -o "$TMP_DIR/warren.tar.gz" 2>/dev/null; then
        spinner_stop fail "Source download failed"
        err "Failed to download source tarball from ${REPO_URL}/archive/${ref_path}.tar.gz"
        exit 1
    fi
    spinner_stop ok "Source downloaded"
    tar -xzf "$TMP_DIR/warren.tar.gz" -C "$TMP_DIR"
    local srcdir
    srcdir="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'warren-*' | head -n1)"
    [ -n "$srcdir" ] || { err "Could not locate the source tree in the archive."; exit 1; }
    spinner_start "Building Warren with cargo (this can take a few minutes)"
    if ! cargo install --path "$srcdir" --root "$(dirname "$BIN_DIR")" --locked --quiet; then
        spinner_stop fail "Cargo build failed"
        exit 1
    fi
    spinner_stop ok "Build complete"
}

summary() {
    local version bin_short
    version="$("$BIN_DIR/warren" --version 2>/dev/null || echo "warren")"
    bin_short="$(short_home "$BIN_DIR/warren")"
    echo
    rule 54
    echo -e "  ${GREEN}${BOLD}✓ Warren installed successfully!${NC}  ${DIM}${version}${NC}"
    rule 54
    echo -e "  ${BOLD}Binary${NC}   ${bin_short}"
    echo -e "  ${BOLD}Docs${NC}     ${REPO_URL}"
    echo
    echo -e "  ${BOLD}Quick start${NC}"
    echo -e "  ${DIM}\$${NC} warren dig gh --as gh-work"
    echo -e "  ${DIM}\$${NC} warren dig flatpak:com.discordapp.Discord --as discord-work"
    echo -e "  ${DIM}\$${NC} warren session save && warren session restore"
    echo -e "  ${DIM}\$${NC} warren --help"
    echo
    dim "You may need to restart your terminal or source your shell config."
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

banner

[ "$(id -u)" -ne 0 ] || die_root
[ "$(uname -s)" = "Linux" ] || die_os
require_curl

# Check HOME is set
[ -n "${HOME:-}" ] || die_no_home

TAG="$(latest_tag)"
if [ -n "$VERSION_TAG" ]; then
    dim "Version: ${VERSION_TAG}   ·   Arch: $(uname -m)   ·   Mode: $([ "$FROM_SOURCE" = true ] && echo "from source" || echo "pre-built preferred")"
elif [ -n "$TAG" ]; then
    dim "Latest release: ${TAG}   ·   Arch: $(uname -m)   ·   Mode: $([ "$FROM_SOURCE" = true ] && echo "from source" || echo "pre-built preferred")"
else
    dim "Arch: $(uname -m)   ·   Mode: $([ "$FROM_SOURCE" = true ] && echo "from source" || echo "pre-built preferred")"
fi

TMP_DIR="$(mktemp -d)"
trap 'spinner_cleanup; rm -rf "$TMP_DIR"' EXIT

mkdir -p "$BIN_DIR"

TOTAL_STEPS=4

step 1 "Checking environment"
ok "Linux user space ready ($(uname -m))"

step 2 "Getting Warren"
if $FROM_SOURCE || ! install_prebuilt; then
    install_from_source
fi

step 3 "Verifying installation"
INSTALLED_VERSION="$("$BIN_DIR/warren" --version 2>/dev/null || true)"
if [ -z "$INSTALLED_VERSION" ]; then
    err "Warren binary is missing or not executable at ${BIN_DIR}/warren"
    exit 1
fi
ok "Binary responds: ${INSTALLED_VERSION}"

step 4 "Setting up your shell"
export PATH="$BIN_DIR:$PATH"
if command -v warren >/dev/null 2>&1; then
    warren shell install
else
    err "Failed to locate warren executable after installation."
    exit 1
fi

summary
