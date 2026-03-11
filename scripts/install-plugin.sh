#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGINS_SRC="$SCRIPT_DIR/../plugins"

# Determine default plugin directory (macOS vs Linux)
if [[ "$(uname)" == "Darwin" ]]; then
    DEFAULT_PLUGIN_DIR="$HOME/Library/Application Support/mycelium/plugins"
else
    DEFAULT_PLUGIN_DIR="$HOME/.config/mycelium/plugins"
fi

# Check config.toml for custom directory
CONFIG_FILE="$HOME/.config/mycelium/config.toml"
if [[ -f "$CONFIG_FILE" ]]; then
    CONFIGURED_DIR=$(grep 'directory' "$CONFIG_FILE" 2>/dev/null | head -1 | cut -d'"' -f2 || true)
    [[ -n "$CONFIGURED_DIR" ]] && DEFAULT_PLUGIN_DIR="$CONFIGURED_DIR"
fi

PLUGIN_DIR="${MYCELIUM_PLUGIN_DIR:-$DEFAULT_PLUGIN_DIR}"
FORCE=false

usage() {
    echo "Usage: $0 [--list | --all | <plugin-name>] [--force]"
    echo "  --list           List available plugins"
    echo "  --all            Install all plugins"
    echo "  <plugin-name>    Install a specific plugin (e.g., 'atmos')"
    echo "  --force          Overwrite existing plugins without prompting"
}

install_plugin() {
    local name="$1"
    local src="$PLUGINS_SRC/${name}.sh"
    [[ ! -f "$src" ]] && { echo "Error: plugin '$name' not found in $PLUGINS_SRC"; exit 1; }
    
    mkdir -p "$PLUGIN_DIR"
    local dest="$PLUGIN_DIR/${name}.sh"
    
    if [[ -f "$dest" ]] && [[ "$FORCE" != "true" ]]; then
        read -r -p "Plugin '$name' already exists. Overwrite? [y/N] " answer
        [[ "$answer" != "y" && "$answer" != "Y" ]] && { echo "Skipped."; return; }
    fi
    
    cp "$src" "$dest"
    chmod 755 "$dest"
    echo "✓ Installed $name → $dest"
}

# Parse args
case "${1:-}" in
    --list)
        echo "Available plugins:"
        for f in "$PLUGINS_SRC"/*.sh; do
            [[ -f "$f" ]] && echo "  $(basename "$f" .sh)"
        done
        ;;
    --all)
        [[ "${2:-}" == "--force" ]] && FORCE=true
        for f in "$PLUGINS_SRC"/*.sh; do
            [[ -f "$f" ]] && install_plugin "$(basename "$f" .sh)"
        done
        ;;
    --help|-h|"")
        usage
        ;;
    *)
        PLUGIN_NAME="$1"
        [[ "${2:-}" == "--force" ]] && FORCE=true
        install_plugin "$PLUGIN_NAME"
        ;;
esac
