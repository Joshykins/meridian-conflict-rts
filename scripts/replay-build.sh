#!/usr/bin/env bash
# Plays a replay in the build that recorded it, rebuilt from git: for dev
# builds, which are never published (docs/RELEASES.md). Run from WSL:
#
#   scripts/replay-build.sh replays/20261004-142233.mcreplay [--at M:SS]
#
# Reads the commit from the replay's origin, checks it out in a worktree of its
# own (../mc-replay-<commit>, kept for the next replay of that commit), builds
# the Windows game there (target %LOCALAPPDATA%\meridian-replay-target, shared by
# every commit, so a build after the first is incremental) and opens the replay
# in it. The baked maps come from this checkout; a map rebaked since the
# recording has another content id, and the old build then says its map is
# missing: bake it in that worktree with that commit's README commands.
#
# A build made from uncommitted edits names a commit that does not hold them,
# and the replay may play out differently there.
set -euo pipefail
replay=${1:?"usage: scripts/replay-build.sh REPLAY [game options]"}
shift
replay=$(realpath "$replay")
cd "$(git rev-parse --show-toplevel)"
repo=$PWD
origin=$(cargo run --quiet --profile gate -p mc-builds --bin mc-release -- origin "$replay")
commit=$(sed -n 's/^commit: //p' <<<"$origin")
build=$(sed -n 's/^build: //p' <<<"$origin")
[[ -n $commit ]] || { echo "$replay does not name its commit (build: ${build:-unknown})" >&2; exit 1; }
git cat-file -e "$commit^{commit}" 2>/dev/null || { echo "commit $commit (of $build) is not in this repository" >&2; exit 1; }
short=${commit:0:10}
wt=$(dirname "$repo")/mc-replay-$short
[[ -d $wt ]] || git worktree add --quiet --detach "$wt" "$commit"
for m in maps/*.mcmap; do
    [[ -e $wt/$m ]] || cp "$m" "$wt/maps/"
done
echo "playing $(basename "$replay") in $build"

win_wt=$(wslpath -w "$wt")
win_wt=${win_wt//\'/\'\'}
win_replay=$(wslpath -w "$replay")
win_replay=${win_replay//\'/\'\'}
args=""
# Builds from before --replay-only save their settings over this build's on leaving.
if grep -q '"--replay-only"' "$wt/crates/mc-game/src/main.rs"; then
    args=" --replay-only"
else
    echo "warning: $build predates --replay-only and saves its settings when it closes" >&2
fi
for a in "$@"; do args+=" '${a//\'/\'\'}'"; done
powershell.exe -NoProfile -Command "
\$env:CARGO_TARGET_DIR = \"\$env:LOCALAPPDATA\\meridian-replay-target\"
Set-Location '$win_wt'
cargo build --release -p mc-game 2>&1 | ForEach-Object { \"\$_\" }
if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
& \"\$env:CARGO_TARGET_DIR\\release\\meridian.exe\" --replay '$win_replay'$args
"
