#!/usr/bin/env bash
# Builds and runs natively on macOS/Linux, or on Windows from WSL.
# macOS uses MoltenVK (Vulkan over Metal); WSL uses the Windows GPU.
#
#   ./play.sh                          the front end (main menu)
#   ./play.sh --map crosswater              straight into a skirmish against the AI
#   ./play.sh --map meridian_basin --players 8
#   ./play.sh --scene battle
#   ./play.sh --range                  the test range (add --unit KEY, --scenario under-fire|targets|build)
#   ./play.sh --build-only
set -euo pipefail
cd "$(dirname "$0")"

build_only=false
args=()
for a in "$@"; do
    if [ "$a" = "--build-only" ]; then build_only=true; else args+=("$a"); fi
done

platform=$(uname -s)
if [ "$platform" = Linux ] && { [ -n "${WSL_INTEROP:-}${WSL_DISTRO_NAME:-}" ] || [[ $(uname -r) == *[Mm]icrosoft* ]]; }; then
    # Keep Windows build output off the WSL filesystem and separate from Linux.
    repo=$(wslpath -w "$PWD")
    repo=${repo//\'/\'\'}
    windows_args=()
    for a in "${args[@]+"${args[@]}"}"; do
        a=${a//\'/\'\'}
        windows_args+=("'$a'")
    done
    run='& "$env:CARGO_TARGET_DIR\release\meridian.exe" '"${windows_args[*]:-}"'; exit $LASTEXITCODE'
    $build_only && run='Write-Host "built $env:CARGO_TARGET_DIR\release\meridian.exe"'

    exec powershell.exe -NoProfile -Command "
    \$env:CARGO_TARGET_DIR = \"\$env:LOCALAPPDATA\\meridian-conflict-target\"
    Set-Location '$repo'
    cargo build --release -p mc-game
    if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
    $run
"
fi

if [ "$platform" = Darwin ]; then
    # Homebrew's rustup is keg-only. Also support the standard rustup installer.
    brew_prefix=
    if command -v brew >/dev/null 2>&1; then brew_prefix=$(brew --prefix); fi
    if ! command -v cargo >/dev/null 2>&1; then
        export PATH="${HOME}/.cargo/bin:${brew_prefix:-/opt/homebrew}/opt/rustup/bin:$PATH"
    fi
    # ash loads libvulkan dynamically. Homebrew's library directory is outside
    # dyld's default search path on Apple Silicon. Preserve SDK/user overrides.
    if [ -n "$brew_prefix" ]; then
        export DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:+$DYLD_FALLBACK_LIBRARY_PATH:}$brew_prefix/lib:/usr/local/lib:/usr/lib"
        if [ -z "${VK_DRIVER_FILES:-}${VK_ICD_FILENAMES:-}" ] && [ -f "$brew_prefix/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json" ]; then
            export VK_DRIVER_FILES="$brew_prefix/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json"
        fi
    fi
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust is required. Install rustup (on macOS: brew install rustup)." >&2
    exit 1
fi
if $build_only; then
    exec cargo build --release -p mc-game
fi
exec cargo run --release -p mc-game -- "${args[@]+"${args[@]}"}"
