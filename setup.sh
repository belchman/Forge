#!/usr/bin/env bash
set -euo pipefail

# FlowForge Setup Script
# Installs FlowForge and initializes it for the current project.
#
# Usage:
#   ./setup.sh              # Build, install, and init for current project
#   ./setup.sh --global     # Also set up global config
#   ./setup.sh --no-init    # Only build and install, skip project init

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DO_INIT=true
DO_GLOBAL=false

for arg in "$@"; do
    case "$arg" in
        --global) DO_GLOBAL=true ;;
        --no-init) DO_INIT=false ;;
        --help|-h)
            echo "FlowForge Setup"
            echo ""
            echo "Usage: ./setup.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --global     Also set up global config (~/.flowforge/)"
            echo "  --no-init    Only build and install, skip project init"
            echo "  -h, --help   Show this help"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            exit 1
            ;;
    esac
done

echo "==> Building FlowForge (release)..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" 2>&1

echo ""
echo "==> Installing flowforge to ~/.cargo/bin..."
cargo install --path "$SCRIPT_DIR/crates/flowforge-cli" --force 2>&1

# Verify it's on PATH
if ! command -v flowforge &>/dev/null; then
    echo ""
    echo "WARNING: flowforge is not on your PATH."
    echo "Add ~/.cargo/bin to your PATH:"
    echo '  export PATH="$HOME/.cargo/bin:$PATH"'
    echo ""
    echo "Add this to your ~/.zshrc or ~/.bashrc to make it permanent."
    exit 1
fi

echo ""
echo "==> Installed: $(which flowforge)"
echo "    Version:   $(flowforge --version)"

if $DO_INIT; then
    echo ""
    echo "==> Initializing FlowForge for current project ($(pwd))..."
    flowforge init --project

    if $DO_GLOBAL; then
        echo ""
        echo "==> Setting up global config..."
        flowforge init --global
    fi
fi

echo ""
echo "============================================"
echo " FlowForge is ready!"
echo "============================================"
echo ""
echo "What was set up:"
echo "  - .flowforge/config.toml  (project config)"
echo "  - .flowforge/flowforge.db (SQLite database)"
echo "  - .claude/settings.json   (Claude Code hooks)"
echo "  - .mcp.json               (MCP server auto-registration)"
echo "  - CLAUDE.md               (agent instructions)"
echo ""
echo "Quick start:"
echo "  flowforge agent list              # See 60+ built-in agents"
echo "  flowforge route \"<task>\"           # Get agent suggestions"
echo "  flowforge work create --type task --title \"My task\"  # Create a tracked work item"
echo "  flowforge work status             # See work tracking status"
echo "  flowforge session current         # Check current session"
echo "  flowforge learn stats             # Check learning stats"
echo "  flowforge mcp serve               # Start MCP server (auto via .mcp.json)"
echo ""
echo "Start a new Claude Code session to activate hooks."
