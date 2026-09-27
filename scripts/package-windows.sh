#!/usr/bin/env bash
# Builds a Windows playtest zip from the last commit (run from WSL):
#
#   scripts/package-windows.sh
#
# The build is made in a worktree of HEAD (../mc-package), so other sessions'
# uncommitted edits stay out of it and the build stamp names a real commit.
# The exe links the C runtime statically, so a tester needs nothing installed
# but a Vulkan GPU driver. The zip holds meridian.exe, data/ (tracked files
# only, so no music reference recordings) and maps/ (the baked .mcmap files
# from this tree), and lands in target/dist/.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
repo=$PWD
wt=$(dirname "$repo")/mc-package
commit=$(git rev-parse --short=10 HEAD)
name="MeridianConflict-$commit"

ls maps/*.mcmap >/dev/null 2>&1 || { echo "no baked maps in maps/" >&2; exit 1; }

if [ -d "$wt" ]; then
    git -C "$wt" checkout --quiet --detach "$commit"
else
    git worktree add --quiet --detach "$wt" "$commit"
fi
# Windows does not follow WSL symlinks, so the baked maps are copied in.
cp -u maps/*.mcmap "$wt/maps/"

win_wt=$(wslpath -w "$wt")
win_wt=${win_wt//\'/\'\'}
powershell.exe -NoProfile -Command "
\$env:CARGO_TARGET_DIR = \"\$env:TEMP\\meridian-package-target\"
\$env:RUSTFLAGS = '-C target-feature=+crt-static'
Set-Location '$win_wt'
cargo build --release -p mc-game 2>&1 | ForEach-Object { \"\$_\" }
if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
Copy-Item \"\$env:CARGO_TARGET_DIR\\release\\meridian.exe\" '$win_wt\\target-meridian.exe'
"

stage="$repo/target/dist/$name"
rm -rf "$stage" "$stage.zip"
mkdir -p "$stage/maps"
mv "$wt/target-meridian.exe" "$stage/meridian.exe"
git -C "$wt" archive HEAD data | tar -x -C "$stage"
cp "$wt"/maps/*.mcmap "$wt"/maps/*.ron "$stage/maps/"
cat >"$stage/README.txt" <<EOF
Meridian Conflict playtest build ($commit)

Unzip anywhere and run meridian.exe. Keep data\ and maps\ next to it.

Needs a Vulkan-capable GPU with a current driver (NVIDIA, AMD or Intel).
If Windows SmartScreen warns about an unknown app: More info > Run anyway.

Settings, replays and crash logs live in %APPDATA%\meridian-conflict.
If the game crashes, please send the newest crash-*.log from there.
EOF
(cd "$repo/target/dist" && 7z a -tzip -mx=7 -bso0 -bsp0 "$name.zip" "$name")
echo "built $repo/target/dist/$name.zip ($(du -h "$repo/target/dist/$name.zip" | cut -f1))"
