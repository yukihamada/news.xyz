#!/usr/bin/env bash
set -euo pipefail

# EnablerDAO CLI Installer
# Automated contribution agent for open source projects

VERSION="0.1.0"
INSTALL_DIR="$HOME/.enabler"
BIN_DIR="$HOME/.local/bin"

echo "🚀 EnablerDAO CLI Installer v${VERSION}"
echo ""

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux*)   OS_TYPE="linux";;
  Darwin*)  OS_TYPE="darwin";;
  *)        echo "❌ Unsupported OS: $OS"; exit 1;;
esac

case "$ARCH" in
  x86_64)   ARCH_TYPE="amd64";;
  aarch64|arm64) ARCH_TYPE="arm64";;
  *)        echo "❌ Unsupported architecture: $ARCH"; exit 1;;
esac

echo "📦 Detected: ${OS_TYPE}-${ARCH_TYPE}"
echo ""

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$BIN_DIR"

# Download enabler CLI (placeholder - update with actual release URL)
DOWNLOAD_URL="https://github.com/enablerdao/enabler-cli/releases/latest/download/enabler-${OS_TYPE}-${ARCH_TYPE}"

echo "⬇️  Downloading EnablerDAO CLI..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$INSTALL_DIR/enabler" 2>/dev/null || {
    echo "⚠️  Download URL not available yet. Creating placeholder..."
    echo '#!/bin/bash' > "$INSTALL_DIR/enabler"
    echo 'echo "EnablerDAO CLI v0.1.0 (development mode)"' >> "$INSTALL_DIR/enabler"
    echo 'echo "Usage: enabler [command]"' >> "$INSTALL_DIR/enabler"
    echo 'echo "  scan    - Scan GitHub for contribution opportunities"' >> "$INSTALL_DIR/enabler"
    echo 'echo "  improve - Analyze and improve a project"' >> "$INSTALL_DIR/enabler"
    echo 'echo "  push    - Create PR with improvements"' >> "$INSTALL_DIR/enabler"
  }
elif command -v wget >/dev/null 2>&1; then
  wget -q "$DOWNLOAD_URL" -O "$INSTALL_DIR/enabler" 2>/dev/null || {
    echo "⚠️  Download URL not available yet. Creating placeholder..."
    echo '#!/bin/bash' > "$INSTALL_DIR/enabler"
    echo 'echo "EnablerDAO CLI v0.1.0 (development mode)"' >> "$INSTALL_DIR/enabler"
  }
else
  echo "❌ Neither curl nor wget found. Please install one of them."
  exit 1
fi

chmod +x "$INSTALL_DIR/enabler"

# Create symlink
ln -sf "$INSTALL_DIR/enabler" "$BIN_DIR/enabler"

echo "✅ EnablerDAO CLI installed to: $BIN_DIR/enabler"
echo ""

# Add to PATH if needed
SHELL_RC=""
case "$SHELL" in
  */bash) SHELL_RC="$HOME/.bashrc";;
  */zsh)  SHELL_RC="$HOME/.zshrc";;
  */fish) SHELL_RC="$HOME/.config/fish/config.fish";;
esac

if [ -n "$SHELL_RC" ] && [ -f "$SHELL_RC" ]; then
  if ! grep -q "$BIN_DIR" "$SHELL_RC"; then
    echo "export PATH=\"$BIN_DIR:\$PATH\"" >> "$SHELL_RC"
    echo "📝 Added $BIN_DIR to PATH in $SHELL_RC"
    echo "   Run: source $SHELL_RC"
  fi
fi

echo ""
echo "🎉 Installation complete!"
echo ""
echo "Get started:"
echo "  enabler --help        # Show all commands"
echo "  enabler scan          # Find contribution opportunities"
echo "  enabler improve       # Analyze and improve projects"
echo ""
echo "Learn more: https://enablerdao.com"
