#!/usr/bin/env bash
set -e

# Warren Automated Installer
# https://github.com/swadhinbiswas/warren

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "  ${CYAN}warren${NC}  installer\n"

# Prevent running as root
if [ "$(id -u)" -eq 0 ]; then
    echo -e "  ${RED}✗${NC}  Warren refuses to be installed or run as root."
    echo -e "     Please run this script as your normal user."
    exit 1
fi

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "$OS" != "Linux" ]; then
    echo -e "  ${RED}✗${NC}  Warren currently only supports Linux."
    exit 1
fi

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

# Try to install via cargo if available
if command -v cargo &> /dev/null; then
    echo -e "  ${CYAN}ℹ${NC}  Cargo detected. Installing Warren from source..."
    
    # If we are in the source directory, install from path, otherwise from crates.io
    if [ -f "Cargo.toml" ] && grep -q 'name = "warren"' Cargo.toml; then
        cargo install --path . --quiet
    else
        cargo install warren --quiet
    fi
    
    echo -e "  ${GREEN}✓${NC}  Warren binary built and installed."
else
    # Fallback to pre-built binary if cargo is not available
    # NOTE: Update this URL once GitHub releases are active
    VERSION="latest"
    DOWNLOAD_URL="https://github.com/swadhinbiswas/warren/releases/latest/download/warren-linux-${ARCH}.tar.gz"
    
    echo -e "  ${CYAN}ℹ${NC}  Downloading pre-built Warren binary..."
    
    if curl --output /dev/null --silent --head --fail "$DOWNLOAD_URL"; then
        TMP_DIR=$(mktemp -d)
        curl -fsSL "$DOWNLOAD_URL" | tar -xz -C "$TMP_DIR"
        mv "$TMP_DIR/warren" "$BIN_DIR/warren"
        chmod +x "$BIN_DIR/warren"
        rm -rf "$TMP_DIR"
        echo -e "  ${GREEN}✓${NC}  Warren binary downloaded."
    else
        echo -e "  ${RED}✗${NC}  Pre-built binary not found for ${ARCH}. Please install Rust/Cargo first:"
        echo -e "     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
fi

# Run Warren's built-in shell integration
# This handles Bash, Zsh, Fish, and Nushell automatically!
export PATH="$BIN_DIR:$PATH"
if command -v warren &> /dev/null; then
    echo -e "\n  ${CYAN}warren${NC}  Setting up shell integration..."
    warren shell install
else
    echo -e "  ${RED}✗${NC}  Failed to locate warren executable after installation."
    exit 1
fi

echo -e "\n  ${GREEN}✓${NC}  Warren installation complete!"
echo -e "     You may need to restart your terminal or source your shell config."
echo -e "     Run '${CYAN}warren --help${NC}' to get started."
