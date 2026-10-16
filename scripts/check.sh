#!/usr/bin/env bash
# The gate every commit is measured against (CLAUDE.md, "Work lands in commits").
#
#   scripts/check.sh            fmt, clippy (warnings are errors), every test
#   scripts/check.sh --quick    fmt and clippy only
#   scripts/check.sh --head [REF]  the full check on the last commit (or REF), in a
#                               worktree of its own: what is committed, without
#                               the uncommitted edits other sessions have in the
#                               shared tree. scripts/channel.sh checks what it
#                               promotes this way.
#
# The toolchain comes from rust-toolchain.toml. Tests build in the `gate`
# profile (Cargo.toml): optimised like release, but incremental and without
# LTO, so after an edit only what changed rebuilds. Clippy runs while the tests
# build, and the test binaries run side by side, with perf_budgets alone at the
# end (its time budgets need a quiet machine). A new checkout's third-party
# crates come ready-built from scripts/target-seed.sh. The Linux build has no
# audio device, so sound is left out (`--features alsa` is opt-in).
#
# Many sessions check at once on one machine, so a check first takes one of
# MERIDIAN_CHECK_SLOTS machine-wide slots (default: one per 8 cores) and waits,
# naming the holders, while all are taken. It runs at low priority, with its
# cargo jobs and test threads sized to its slot's share of the cores, so checks
# side by side neither starve each other nor the sessions' own builds and shots.
# A test binary that fails runs once more on as quiet a machine as two minutes'
# wait for the other checks gives, so a load-sensitive test does not fail it.
#
# A test binary still running after MERIDIAN_TEST_LIMIT seconds (default 300)
# fails the check as hung; MERIDIAN_TEST_JOBS (default: half the slot's cores)
# binaries run at once.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)

mode=full
ref=HEAD
case "${1:-}" in
    --quick) mode=quick ;;
    --head) mode=head; ref=${2:-HEAD} ;;
    "") ;;
    *) echo "usage: scripts/check.sh [--quick|--head [REF]]" >&2; exit 2 ;;
esac

if [ "$mode" = head ]; then
    # One detached worktree, reused across runs, with its own target dir.
    verify="$root/../$(basename "$root")-verify"
    if [ ! -d "$verify" ]; then
        git -C "$root" worktree add --detach "$verify" HEAD >/dev/null
    fi
    commit=$(git -C "$root" rev-parse --verify "$ref^{commit}")
    git -C "$verify" checkout --quiet --detach "$commit"
    # Baked maps are not checked in; the tests that need them read the shared ones.
    for m in "$root"/maps/*.mcmap; do
        [ -e "$m" ] && ln -sf "$m" "$verify/maps/"
    done
    echo "== checking $(git -C "$root" log -1 --format='%h %s' "$commit") in $verify"
    exec "$verify/scripts/check.sh"
fi

cd "$root"
start=$SECONDS
renice -n 10 -p $$ > /dev/null 2>&1 || true

# Machine-wide slots are lock files in /tmp, held until the check exits (or is
# killed). Each one's .who file names its holder, so a wait says whom it is for.
cpus=$(nproc)
slots=${MERIDIAN_CHECK_SLOTS:-$((cpus >= 16 ? cpus / 8 : 1))}
share=$((cpus / slots > 1 ? cpus / slots : 1))
half=$((share / 2 > 2 ? share / 2 : 2))
me="$(basename "$root") (pid $$)"
declare -A held=()
lock_slot() { # SLOT WAIT(-n or empty)
    local fd
    exec {fd}> "/tmp/meridian-check-slot-$1"
    if flock $2 "$fd"; then
        held[$1]=$fd
        echo "$me" > "/tmp/meridian-check-slot-$1.who"
    else
        exec {fd}>&-
        return 1
    fi
}
take_slot() {
    local i waited=0
    while :; do
        for ((i = 0; i < slots; i++)); do
            lock_slot "$i" -n && return
        done
        [ $((waited % 60)) != 0 ] || echo "== waiting for a check slot, held by: $(cat /tmp/meridian-check-slot-*.who 2> /dev/null | paste -sd, - | sed 's/,/, /g')"
        sleep 5
        waited=$((waited + 5))
    done
}
# Every slot it can gather within SECONDS, for a run that wants a quiet
# machine. A check gives its own slot back first and only one gathers at a
# time, so two never deadlock; one that runs out of time goes on with what it
# has rather than stall its session.
take_machine() {
    local i fd quiet until=$((SECONDS + $1))
    for i in "${!held[@]}"; do
        fd=${held[$i]}
        exec {fd}>&-
    done
    held=()
    exec {quiet}> /tmp/meridian-check-quiet
    flock -w "$1" "$quiet" || true
    for ((i = 0; i < slots; i++)); do
        lock_slot "$i" "-w $((until > SECONDS ? until - SECONDS : 0))" || true
    done
}
take_slot
echo "== check slot taken ($slots on this machine, $share cores each)"
export CARGO_BUILD_JOBS=$share
export RUST_TEST_THREADS=${RUST_TEST_THREADS:-$half}

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
# Clippy only checks, so it gets half the slot's jobs beside the test build.
CARGO_BUILD_JOBS=$half cargo clippy --workspace --all-targets --quiet --color always -- -D warnings \
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

# A test binary still running after this long has a hung test: it is stopped
# and the check fails naming it (cargo prints "running for over 60 seconds" for
# each test still going), well inside a session's 10-minute command limit. The
# slowest binaries take 90 s, up to about 240 s on a loaded machine.
TEST_LIMIT=${MERIDIAN_TEST_LIMIT:-300}

# Runs one test binary; its output is kept, and printed only if it fails.
run_one() {
    local exe=$1 dir=$2 name=$3
    local log
    log="$logs/$(basename "$exe").log"
    local began=$SECONDS
    local status=0
    (cd "$dir" && CARGO_MANIFEST_DIR=$dir timeout "$TEST_LIMIT" "$exe" --quiet) > "$log" 2>&1 || status=$?
    if [ "$status" = 124 ]; then
        echo "FAIL $name ($(basename "$dir")): still running after $TEST_LIMIT s, a test hangs"
        grep -a 'running for over' "$log" | sed 's/^/     /'
        touch "$log.failed"
    elif [ "$status" = 0 ]; then
        echo "ok   $name: $(grep -Eo '[0-9]+ passed.*finished in [0-9.]+s' "$log" | tail -1)"
        echo "$((SECONDS - began)) $dir $name" >> "$logs/times"
    else
        echo "FAIL $name ($(basename "$dir"))"
        # The binary's line, so it can run again (a hung one does not).
        printf '%s\t%s\t%s\n' "$exe" "$dir" "$name" > "$log.failed"
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
grep -v -P '\tperf_budgets$' "$logs/binaries" | order | run_all "${MERIDIAN_TEST_JOBS:-$half}"
wait "$docs" || doc=$?
grep -P '\tperf_budgets$' "$logs/binaries" | run_all 1

# perf_budgets and the tests that count ticks against the clock can fail on a
# busy machine alone, so a binary that failed runs once more, one at a time,
# with as many slots as two minutes' wait gathers. One that passes then is
# named, with its first failure, as one to make load-proof.
again=$(cat "$logs"/*.log.failed 2> /dev/null || true)
if [ -n "$again" ]; then
    echo "== running the failed binaries again (load $(cut -d' ' -f1 /proc/loadavg))"
    for f in "$logs"/*.log.failed; do
        [ -s "$f" ] || continue
        mv "${f%.failed}" "${f%.log.failed}.first"
        rm "$f"
    done
    take_machine 120
    RUST_TEST_THREADS=$cpus run_all 1 <<< "$again"
    for f in "$logs"/*.first; do
        [ -e "${f%.first}.log.failed" ] && continue
        echo "   $(basename "${f%.first}") passed the second time; it failed under load with:"
        grep -a -A2 'panicked at' "$f" | sed 's/^/     /' || true
    done
fi

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
