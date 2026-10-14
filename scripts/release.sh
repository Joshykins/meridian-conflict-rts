#!/usr/bin/env bash
# Publishes the tip of a channel's branch (playtest: playtest, release: master;
# scripts/channel.sh moves them) as that channel's newest build
# (docs/RELEASES.md). Run from WSL:
#
#   scripts/release.sh playtest|release [--no-upload] [--from REF]
#
# 1. Builds meridian.exe and the launcher (MeridianConflict.exe) for Windows in a
#    worktree of that commit (../mc-package), so uncommitted edits stay out and
#    the build stamp names a real commit, with MERIDIAN_CHANNEL, the store
#    URL and the channel's server from release/config.sh. The C runtime is linked
#    statically: a player needs nothing installed but a Vulkan GPU driver.
# 2. Stages the build: meridian.exe, data/ (tracked files only), maps/ (the baked
#    maps; a release leaves out maps marked playtest), launcher/MeridianConflict.exe.
# 3. Packs it into the local copy of the store (~/.local/share/meridian-release/store),
#    signed with ~/.config/meridian-release/signing.key, as the channel's newest.
# 4. Uploads what the published store lacks to R2: blobs, then the build's
#    manifest, then the channel's pointer last.
# 5. Writes the install zip, target/dist/MeridianConflict-<Channel>-Build<N>.zip:
#    the launcher and this build, ready to unzip and run. Its symbols go to the
#    main checkout's target/dist (<name>-<commit>.pdb): a crash report names code
#    as meridian.exe+0x..., which scripts/symbolize.sh turns into names with it,
#    and a minidump opens in Visual Studio or WinDbg only with it.
#
# --no-upload does 1-3 and 5 only: a zip for hand delivery, and a store copy to
# upload later by running again. With it, --from REF builds REF instead of the
# channel's branch (to try the pipeline on a commit of your own).
set -euo pipefail
channel=${1:-}
case $channel in
    playtest) ref=playtest ;;
    release) ref=master ;;
    *) echo "usage: scripts/release.sh playtest|release [--no-upload] [--from REF]" >&2; exit 2 ;;
esac
shift
upload=1
while (( $# )); do
    case $1 in
        --no-upload) upload=0 ;;
        --from) ref=${2:?"--from takes a commit"}; shift; from=1 ;;
        *) echo "release.sh: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done
if [[ -n ${from:-} ]] && (( upload )); then
    echo "release.sh: --from goes with --no-upload: a channel publishes its branch (scripts/channel.sh)" >&2
    exit 2
fi

cd "$(git rev-parse --show-toplevel)"
repo=$PWD
# shellcheck source=../release/config.sh
source release/config.sh
key=$HOME/.config/meridian-release/signing.key
store=${MERIDIAN_RELEASE_STORE:-$HOME/.local/share/meridian-release/store}
[[ -f $key ]] || { echo "no release key at $key (docs/RELEASES.md)" >&2; exit 1; }
if (( upload )) && [[ -z $STORE_URL || -z $R2_BUCKET || -z $R2_ENDPOINT ]]; then
    echo "fill in STORE_URL, R2_BUCKET and R2_ENDPOINT in release/config.sh first, or pass --no-upload" >&2
    exit 1
fi
ls maps/*.mcmap >/dev/null 2>&1 || { echo "no baked maps in maps/" >&2; exit 1; }

wt=$(dirname "$repo")/mc-package
git rev-parse -q --verify "$ref^{commit}" >/dev/null || { echo "release.sh: no $ref branch: scripts/channel.sh promote $channel makes it" >&2; exit 1; }
commit=$(git rev-parse --short=10 "$ref")
number=$(git rev-list --count "$ref")
Channel=${channel^}
name="MeridianConflict-$Channel-Build$number"
if [ -d "$wt" ]; then
    git -C "$wt" checkout --quiet --detach "$commit"
else
    git worktree add --quiet --detach "$wt" "$commit"
fi
# Windows does not follow WSL symlinks, so the baked maps are copied in.
cp -u maps/*.mcmap "$wt/maps/"

win_wt=$(wslpath -w "$wt")
win_wt=${win_wt//\'/\'\'}
q() { printf "%s" "${1//\'/\'\'}"; }
# Not under %TEMP%: Storage Sense deletes old files there one by one, and cargo,
# finding its fingerprints intact, then misses a build script's lost output.
powershell.exe -NoProfile -Command "
\$env:CARGO_TARGET_DIR = \"\$env:LOCALAPPDATA\\meridian-package-target\"
\$env:RUSTFLAGS = '-C target-feature=+crt-static'
\$env:MERIDIAN_CHANNEL = '$channel'
\$env:MERIDIAN_STORE = '$(q "$STORE_URL")'
\$env:MERIDIAN_SERVER_PLAYTEST = '$(q "$SERVER_PLAYTEST")'
\$env:MERIDIAN_SERVER_RELEASE = '$(q "$SERVER_RELEASE")'
Set-Location '$win_wt'
cargo build --release -p mc-game -p mc-launcher 2>&1 | ForEach-Object { \"\$_\" }
if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
foreach (\$f in 'meridian.exe', 'meridian.pdb', 'MeridianConflict.exe') {
    Copy-Item \"\$env:CARGO_TARGET_DIR\\release\\\$f\" \"$win_wt\\target-\$f\"
}
"

out=$repo/target/dist/$name
rm -rf "$out" "$out.zip"
stage=$out/stage
mkdir -p "$stage/maps" "$stage/launcher"
mv "$wt/target-meridian.exe" "$stage/meridian.exe"
chmod +x "$stage/meridian.exe"
mv "$wt/target-MeridianConflict.exe" "$stage/launcher/MeridianConflict.exe"
# Every published build's symbols, for crash reports from it for as long as it
# is played: in the main checkout, where scripts/symbolize.sh finds them by commit.
main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
mkdir -p "$main/target/dist"
mv "$wt/target-meridian.pdb" "$main/target/dist/$name-$commit.pdb"
git -C "$wt" archive HEAD data | tar -x -C "$stage"
for ron in "$wt"/maps/*.ron; do
    stem=$(basename "$ron" .ron)
    if [[ $channel == release ]] && grep -Eq '^\s*playtest:\s*true' "$ron"; then
        echo "leaving out playtest map $stem"
        continue
    fi
    cp "$ron" "$stage/maps/"
    [[ -f $wt/maps/$stem.mcmap ]] && cp "$wt/maps/$stem.mcmap" "$stage/maps/"
done

# Who the build says it is: the Windows executable runs from WSL.
{ (cd "$stage" && ./meridian.exe --version) | tr -d '\r'; echo "platform: windows-x64"; } >"$out/version.txt"
build=$(sed -n 's/^build: //p' "$out/version.txt")
key_name=$(printf "%s" "$build" | sed 's/[^A-Za-z0-9.-]/_/g')
cargo run --quiet --profile gate -p mc-builds --bin mc-release -- pack \
    --stage "$stage" --store "$store" --key "$key" --version-of "$out/version.txt" --newest

if (( upload )); then
    s3() { aws s3 "$@" --endpoint-url "$R2_ENDPOINT" --profile "$R2_PROFILE" --only-show-errors; }
    # Blobs are named by their bytes: one already there is the same.
    s3 sync "$store/blobs" "s3://$R2_BUCKET/blobs" --size-only
    manifest=builds/windows-x64/$key_name.json
    s3 cp "$store/$manifest" "s3://$R2_BUCKET/$manifest"
    s3 cp "$store/$manifest.sig" "s3://$R2_BUCKET/$manifest.sig"
    # Last: a player who reads the pointer between the two copies finds a
    # signature that does not verify and tries again on the next start.
    pointer=channels/$channel/windows-x64.json
    s3 cp "$store/$pointer" "s3://$R2_BUCKET/$pointer"
    s3 cp "$store/$pointer.sig" "s3://$R2_BUCKET/$pointer.sig"
    echo "published $build to $STORE_URL"
fi

# The install: the launcher, and this build as its current one.
install=$out/MeridianConflict
mkdir -p "$install/versions"
cp "$stage/launcher/MeridianConflict.exe" "$install/"
cp -r "$stage" "$install/versions/$key_name"
cp "$store/builds/windows-x64/$key_name.json" "$install/versions/$key_name/manifest.json"
printf "%s" "$key_name" >"$install/versions/current"
cat >"$install/README.txt" <<README
Meridian Conflict, $channel build $number ($commit)

Unzip anywhere and run MeridianConflict.exe. It keeps the game up to date, and
fetches the build an old replay was recorded in when you watch it.

Needs a Vulkan-capable GPU with a current driver (NVIDIA, AMD or Intel).
If Windows SmartScreen warns about an unknown app: More info > Run anyway.

If the game crashes or cannot start, it says so in a window: press Copy details
and paste them into a message to us. Open folder shows the saved report.

Settings, crash reports and the last run's log (meridian.log) live in
%APPDATA%\\meridian-conflict (Win+R, then paste %APPDATA%\\meridian-conflict).
Replays are in the replays folder beside MeridianConflict.exe.
README
rm -rf "$stage"
(cd "$out" && 7z a -tzip -mx=7 -bso0 -bsp0 "$out.zip" MeridianConflict)
echo "built $out.zip ($(du -h "$out.zip" | cut -f1))"
