#!/usr/bin/env bash
# AI tournament (docs/AI_COMMANDER.md, "Tournament"): many headless matches between
# AI sides in parallel, then win rates, Elo and the plans each side ran.
#
#   scripts/ai-tournament.sh [OUT_DIR]
#
# Environment:
#   TOURNEY_MAPS     maps:players, space separated
#                    (default "serac_divide:2 twin_shoals:2 meridian_basin:2 crosswater:2 the_axis:8 haldens_grip:8")
#   TOURNEY_SEEDS    seeds (default "3 7 11")
#   TOURNEY_SIDES    pairs A,B of doctrines (adaptive, aggressive, economic,
#                    defensive; default: adaptive against each of the others, each
#                    way round)
#   TOURNEY_MINUTES  match length (default 35)
#   TOURNEY_DIFF     easy | normal | hard (default hard)
#   TOURNEY_JOBS     matches at once (default: the number of cores less two)
#
# Each match writes OUT_DIR/<name>.log; OUT_DIR/results.tsv holds the RESULT lines and
# the summary is printed at the end. A match that crashed is listed as such.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/artifacts/tournament/$(date +%Y%m%d-%H%M%S)}
mkdir -p "$out"
maps=${TOURNEY_MAPS:-"serac_divide:2 twin_shoals:2 meridian_basin:2 crosswater:2 the_axis:8 haldens_grip:8"}
seeds=${TOURNEY_SEEDS:-"3 7 11"}
sides=${TOURNEY_SIDES:-"adaptive,aggressive aggressive,adaptive adaptive,economic economic,adaptive adaptive,defensive defensive,adaptive"}
minutes=${TOURNEY_MINUTES:-35}
diff=${TOURNEY_DIFF:-hard}
jobs=${TOURNEY_JOBS:-$(( $(nproc) > 3 ? $(nproc) - 2 : 1 ))}

cd "$root"
"$root/scripts/target-seed.sh" take >/dev/null
cargo test --profile gate -p mc-sim --test sim --no-run --message-format=json 2>/dev/null \
    | jq -r 'select(.executable != null and .target.name == "sim") | .executable' > "$out/.bin"
bin=$(cat "$out/.bin")
[ -x "$bin" ] || { echo "ai-tournament: the sim test binary did not build" >&2; exit 1; }
# A copy, so a rebuild while the tournament runs does not swap the binary under it.
cp "$bin" "$out/sim"

: > "$out/matches"
for m in $maps; do
    for s in $seeds; do
        for pair in $sides; do
            a=${pair%,*}; b=${pair#*,}
            echo "${m%:*}:${m#*:}:$minutes:$s:$a:$b:$diff" >> "$out/matches"
        done
    done
done
echo "ai-tournament: $(wc -l < "$out/matches") matches, $jobs at once, logs in $out"

export out
xargs -P "$jobs" -I{} bash -c '
    spec={}
    name=$(echo "$spec" | tr ":/" "__")
    TOURNEY=$spec "$out/sim" zz_ai_tournament:: --ignored --nocapture > "$out/$name.log" 2>&1 \
        || echo "CRASHED $spec" >> "$out/crashed"
' < "$out/matches"

grep -h "^RESULT" "$out"/*.log | sed "s/^RESULT //" > "$out/results.tsv" || true
python3 - "$out" <<'EOF'
import sys, collections, math, glob, os
out = sys.argv[1]
rows = []
for line in open(os.path.join(out, "results.tsv")):
    kv = dict(p.split("=", 1) for p in line.split())
    rows.append(kv)
crashed = open(os.path.join(out, "crashed")).read().split("\n") if os.path.exists(os.path.join(out, "crashed")) else []
wins = collections.Counter(); games = collections.Counter(); score = collections.Counter()
elo = collections.defaultdict(lambda: 1000.0)
by_map = collections.defaultdict(lambda: collections.Counter())
minutes = collections.defaultdict(list)
for r in rows:
    a, b, v = r["A"], r["B"], r["verdict"]
    s = {"A": 1.0, "a": 0.75, "-": 0.5, "b": 0.25, "B": 0.0}[v]
    games[a] += 1; games[b] += 1
    score[a] += s; score[b] += 1 - s
    if v in "Aa": wins[a] += 1
    if v in "Bb": wins[b] += 1
    by_map[r["map"]][(a if v in "Aa" else b if v in "Bb" else "draw")] += 1
    if v in "AB": minutes[a if v == "A" else b].append(int(r["minute"]))
    ea = 1 / (1 + 10 ** ((elo[b] - elo[a]) / 400))
    elo[a] += 24 * (s - ea); elo[b] -= 24 * (s - ea)
print(f"\n{len(rows)} matches, {len([c for c in crashed if c])} crashed")
for c in crashed:
    if c: print("  " + c)
print("\nside                       games  score   wins  elo   decisive wins at (min)")
for side in sorted(games, key=lambda s: -score[s]):
    m = minutes[side]
    print(f"{side:<26} {games[side]:>5} {score[side]/games[side]:>6.2f} {wins[side]:>6} {elo[side]:>5.0f}   {sorted(m)}")
print("\nby map (wins, a lead at the end counts):")
for m, c in sorted(by_map.items()):
    print(f"  {m:<16} " + ", ".join(f"{k} {n}" for k, n in c.most_common()))
EOF
