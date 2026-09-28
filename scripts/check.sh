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
# The toolchain comes from rust-toolchain.toml. Tests build in the `gate`
# profile (Cargo.toml): optimised like release, but incremental and without
# LTO, so after an edit only what changed rebuilds. Clippy runs while the tests
# build, and the test binaries run side by side, with perf_budgets alone at the
# end (its time budgets need a quiet machine). A new checkout's third-party
# crates come ready-built from scripts/target-seed.sh. The Linux build has no
# audio device, so sound is left out (`--features alsa` is opt-in).
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
start=$SECONDS
"$root/scripts/target-seed.sh" take
echo "== rustfmt"
cargo fmt --all --check

logs=$(mktemp -d)
trap 'rm -rf "$logs"' EXIT

if [ "$mode" = quick ]; then
    echo "== clippy"
    cargo clippy --workspace --all-targets --quiet -- -D warnings
    exit 0
fi

echo "== clippy and the test build"
cargo clippy --workspace --all-targets --quiet --color always -- -D warnings \
    > "$logs/clippy" 2>&1 &
clippy=$!
build=0
cargo test --workspace --profile gate --no-run --quiet \
    --message-format json-render-diagnostics > "$logs/build.json" || build=$?
lint=0
wait "$clippy" || lint=$?
cat "$logs/clippy"
[ "$lint" = 0 ] || { echo "== clippy failed" >&2; exit 1; }
[ "$build" = 0 ] || { echo "== the tests do not build" >&2; exit 1; }
echo "   built in $((SECONDS - start)) s"

# Each test binary, with its package directory (cargo runs tests from there).
jq -r 'select(.reason == "compiler-artifact" and .profile.test and .executable != null)
       | "\(.executable)\t\(.manifest_path | rtrimstr("/Cargo.toml"))\t\(.target.name)"' \
    "$logs/build.json" | sort -u > "$logs/binaries"

# Runs one test binary; its output is kept, and printed only if it fails.
run_one() {
    local exe=$1 dir=$2 name=$3
    local log
    log="$logs/$(basename "$exe").log"
    local began=$SECONDS
    if (cd "$dir" && CARGO_MANIFEST_DIR=$dir "$exe" --quiet) > "$log" 2>&1; then
        echo "ok   $name: $(grep -Eo '[0-9]+ passed.*finished in [0-9.]+s' "$log" | tail -1)"
        echo "$((SECONDS - began)) $dir $name" >> "$logs/times"
    else
        echo "FAIL $name ($(basename "$dir"))"
        touch "$log.failed"
    fi
}

# Runs the binaries listed on stdin, up to JOBS at a time.
run_all() {
    local jobs=$1 exe dir name
    while IFS=$'\t' read -r exe dir name; do
        while [ "$(jobs -rp | wc -l)" -ge "$jobs" ]; do wait -n || true; done
        run_one "$exe" "$dir" "$name" &
    done
    wait
}

# The slowest binaries start first (by their times last run; new ones count as
# slow), so the run ends when the slowest one does, not after it.
times="$root/target/gate/check-times"
order() {
    awk -v known="$times" 'BEGIN { while ((getline l < known) > 0) { split(l, f, " "); t[f[2] " " f[3]] = f[1] } }
        { split($0, f, "\t"); k = f[2] " " f[3]; print ((k in t) ? t[k] : 999) "\t" $0 }' \
        | sort -t$'\t' -k1,1 -rn | cut -f2-
}

echo "== tests"
doc=0
cargo test --workspace --profile gate --doc --quiet > "$logs/doc.log" 2>&1 &
docs=$!
grep -v -P '\tperf_budgets$' "$logs/binaries" | order | run_all "${MERIDIAN_TEST_JOBS:-6}"
wait "$docs" || doc=$?
grep -P '\tperf_budgets$' "$logs/binaries" | run_all 1

[ -s "$logs/times" ] && cp "$logs/times" "$times"

failed=0
for f in "$logs"/*.log.failed; do
    [ -e "$f" ] || continue
    failed=1
    echo "---- ${f%.failed}" | sed "s|$logs/||"
    cat "${f%.failed}"
done
if [ "$doc" != 0 ]; then
    failed=1
    echo "---- doc tests"
    cat "$logs/doc.log"
fi
[ "$failed" = 0 ] || { echo "== tests failed" >&2; exit 1; }

"$root/scripts/target-seed.sh" give
echo "== all checks passed in $((SECONDS - start)) s"
