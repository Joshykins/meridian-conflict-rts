#!/usr/bin/env bash
# Build and run the multiplayer server on macOS/Linux.
#   ./server.sh
#   ./server.sh --motd "Welcome!"
#   ./server.sh --bind 0.0.0.0:7778 --data-dir /path/to/server-data
#   ./server.sh --help
set -euo pipefail
cd "$(dirname "$0")"

# Support both the standard rustup installer and Homebrew's keg-only rustup.
if ! command -v cargo >/dev/null 2>&1; then
    export PATH="${HOME}/.cargo/bin:$PATH"
fi
if ! command -v cargo >/dev/null 2>&1 && [ "$(uname -s)" = Darwin ]; then
    brew_prefix=/opt/homebrew
    if command -v brew >/dev/null 2>&1; then brew_prefix=$(brew --prefix); fi
    export PATH="$brew_prefix/opt/rustup/bin:$PATH"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust is required. Install rustup (on macOS: brew install rustup)." >&2
    exit 1
fi

# Cargo rebuilds after a pull when needed. Relative paths, including the default
# meridian-data directory, are always relative to this repository.
# Keep the Mac awake until the server exits; Ctrl+C stops it cleanly.
if [ "$(uname -s)" = Darwin ]; then
    exec caffeinate -i cargo run --locked --release -p mc-server -- "$@"
fi
exec cargo run --locked --release -p mc-server -- "$@"
