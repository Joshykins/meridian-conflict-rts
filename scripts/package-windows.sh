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
# from this tree), and lands in target/dist/MeridianConflict-Build<N>-<commit>.zip,
# N being the build number the game's menu shows. The build's symbols stay beside
# it, not in it (MeridianConflict-Build<N>-<commit>.pdb): a tester's crash report
# names code as meridian.exe+0x..., and a minidump opens in Visual Studio or WinDbg,
# only with that .pdb.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
repo=$PWD
wt=$(dirname "$repo")/mc-package
commit=$(git rev-parse --short=10 HEAD)
# The build number is the commit count, as the game's menu shows it (build.rs).
number=$(git rev-list --count HEAD)
name="MeridianConflict-Build$number-$commit"

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
# Not under %TEMP%: Storage Sense deletes old files there one by one, and cargo,
# finding its fingerprints intact, then misses a build script's lost output.
powershell.exe -NoProfile -Command "
\$env:CARGO_TARGET_DIR = \"\$env:LOCALAPPDATA\\meridian-package-target\"
\$env:RUSTFLAGS = '-C target-feature=+crt-static'
Set-Location '$win_wt'
cargo build --release -p mc-game 2>&1 | ForEach-Object { \"\$_\" }
if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
Copy-Item \"\$env:CARGO_TARGET_DIR\\release\\meridian.exe\" '$win_wt\\target-meridian.exe'
Copy-Item \"\$env:CARGO_TARGET_DIR\\release\\meridian.pdb\" '$win_wt\\target-meridian.pdb'
"

stage="$repo/target/dist/$name"
rm -rf "$stage" "$stage.zip"
mkdir -p "$stage/maps"
mv "$wt/target-meridian.exe" "$stage/meridian.exe"
mv "$wt/target-meridian.pdb" "$repo/target/dist/$name.pdb"
git -C "$wt" archive HEAD data | tar -x -C "$stage"
cp "$wt"/maps/*.mcmap "$wt"/maps/*.ron "$stage/maps/"
cat >"$stage/README.txt" <<EOF
Meridian Conflict playtest, build $number ($commit)

Unzip anywhere and run meridian.exe. Keep data\ and maps\ next to it.

Needs a Vulkan-capable GPU with a current driver (NVIDIA, AMD or Intel).
If Windows SmartScreen warns about an unknown app: More info > Run anyway.

If the game crashes or cannot start, it says so in a window: press Copy details
and paste them into a message to us. Open folder shows the saved report.

Settings, replays, crash reports and the last run's log (meridian.log) live in
%APPDATA%\meridian-conflict (Win+R, then paste %APPDATA%\meridian-conflict). If the
game ever closes without a window, please send meridian.log from there.
EOF
(cd "$repo/target/dist" && 7z a -tzip -mx=7 -bso0 -bsp0 "$name.zip" "$name")
echo "built $repo/target/dist/$name.zip ($(du -h "$repo/target/dist/$name.zip" | cut -f1))"
