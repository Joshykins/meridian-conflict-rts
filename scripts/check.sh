#!/usr/bin/env bash
# The gate every commit is measured against (CLAUDE.md, "Work lands in commits").
#
#   scripts/check.sh            fmt, clippy (warnings are errors), every test
#   scripts/check.sh --quick    fmt and clippy only
#   scripts/check.sh --head     the full check on the last commit, in a worktree
#                               of its own: what is committed, without the
#                               uncommitted edits other sessions have in the
#                               shared tree
#
# The toolchain comes from rust-toolchain.toml. Tests run in a release build,
# which is what the perf budgets are written for; the Linux build has no audio
# device, so sound is left out (`--features alsa` is opt-in).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)

mode=full
case "${1:-}" in
    --quick) mode=quick ;;
    --head) mode=head ;;
    "") ;;
    *) echo "usage: scripts/check.sh [--quick|--head]" >&2; exit 2 ;;
esac

if [ "$mode" = head ]; then
    # One detached worktree, reused across runs, with its own target dir.
    verify="$root/../$(basename "$root")-verify"
    if [ ! -d "$verify" ]; then
        git -C "$root" worktree add --detach "$verify" HEAD >/dev/null
    fi
    git -C "$verify" checkout --quiet --detach "$(git -C "$root" rev-parse HEAD)"
    # Baked maps are not checked in; the tests that need them read the shared ones.
    for m in "$root"/maps/*.mcmap; do
        [ -e "$m" ] && ln -sf "$m" "$verify/maps/"
    done
    echo "== checking $(git -C "$root" log -1 --format='%h %s') in $verify"
    exec "$verify/scripts/check.sh"
fi

cd "$root"
echo "== rustfmt"
cargo fmt --all --check
echo "== clippy"
cargo clippy --workspace --all-targets --quiet -- -D warnings
[ "$mode" = quick ] && exit 0
echo "== tests"
cargo test --workspace --release --quiet --no-fail-fast
echo "== all checks passed"
