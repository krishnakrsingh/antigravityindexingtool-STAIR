#!/usr/bin/env bash
set -e

echo "============================================================"
echo "  Antigravity Fast-Core: Production Installer (Unix/macOS)  "
echo "============================================================"

# Check for cargo
if ! command -v cargo &> /dev/null; then
    echo "Error: Cargo not found. Please install Rust from https://rustup.rs"
    exit 1
fi

echo -e "\n[1/3] Compiling high-speed release binary..."
cargo build --release

BIN_DIR="$HOME/.antigravity/bin"
mkdir -p "$BIN_DIR"
cp target/release/antigravity-fast-core "$BIN_DIR/antigravity-fast-core"
chmod +x "$BIN_DIR/antigravity-fast-core"

echo -e "\n[2/3] Registering global binary and syncing IDE MCP schemas..."
"$BIN_DIR/antigravity-fast-core" --setup

echo -e "\n[3/3] Checking environment PATH..."
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "Tip: Add $BIN_DIR to your PATH in ~/.bashrc or ~/.zshrc:"
    echo "  export PATH=\"\$HOME/.antigravity/bin:\$PATH\""
fi

echo "============================================================"
echo "  ✅ Installation Complete! Fast-Core is ready for PROD!   "
echo "============================================================"
