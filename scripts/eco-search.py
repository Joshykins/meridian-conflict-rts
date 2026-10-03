#!/usr/bin/env python3
"""Searches the economy race's dials (crates/mc-sim/tests/zz_eco_race_probe.rs,
docs/ECONOMY.md) one at a time.

  scripts/eco-search.py [DIALS_JSON|-] [ROUNDS]

Each round plays the current plan and every other value of every dial (ALTS) on
MAPS, adopts every change that beat it by more than 0.15 minutes, checks the
combination against the single best change, and keeps the better. The start is the
probe's default plan (`-`) with DIALS_JSON laid over it. Env: MAPS (the probe's ECO
form), CAP (minutes a game may run, 35), DIALS (only these dials, comma separated),
OUT (where plan files and raw results go, a temp dir). Build the probe first:
cargo test --profile gate -p mc-sim --test sim --no-run. A round of 8 games a plan
takes about 12 minutes on 16 cores."""
import os, subprocess, sys, json, statistics, glob, tempfile
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCR = os.environ.get("OUT") or tempfile.mkdtemp(prefix="eco-search-")
BIN = [b for b in glob.glob(ROOT + "/target/gate/deps/sim-*") if os.access(b, os.X_OK) and not b.endswith(".d")]
BIN = max(BIN, key=os.path.getmtime)
MAPS = os.environ.get("MAPS", "serac_divide:0+1,twin_shoals:0+1,haldens_grip:0+1,vermilion_gorge:0+1")
CAP = float(os.environ.get("CAP", "35"))
ALTS = {
    "mines": [3, 4, 6, 8, 10],
    "range": [2000, 3000, 4500],
    "eff": [0.4, 0.55, 0.7],
    "engineers": [0, 2, 4, 8],
    "factory_after": [0, 1, 2, 4],
    "ratio": [5, 6, 7, 9],
    "reclaim": [0, 1500, 2500, 4000],
    "towers": [0, 1, 2],
    "tech2_at": [6, 12, 20, 30],
    "tech3_at": [15, 25, 40],
    "tech": [0, 1],
    "eng_up": [1, 2, 3],
    "engineers_max": [4, 8, 16],
    "reclaim_below": [0.2, 0.5, 0.9],
    "mfe_at": [-1, 10, 25],
    "drones_at": [-1, 10],
    "horizon": [0, 200, 400, 800],
    "lanes": [1, 2, 4],
    "build_dc": [0, 1],
    "early_line": [0, 1],
    "focus": [0, 1, 2],
    "assist": [0, 1],
    "reclaim_first": [0, 1],
    "t2_power_at": [3, 6, 15],
    "t3_power_at": [20, 40, 80],
}
# The probe's Plan::default, which the search starts from.
DEFAULT = {"mines": 6, "range": 4500, "eff": 0.7, "engineers": 8, "factory_after": 1, "ratio": 9,
    "reclaim": 4000, "towers": 2, "tech2_at": 20, "tech3_at": 25, "tech": 0, "eng_up": 1,
    "engineers_max": 8, "reclaim_below": 0.5, "mfe_at": 10, "drones_at": -1, "horizon": 0,
    "lanes": 1, "build_dc": 0, "early_line": 1, "focus": 2, "assist": 1, "reclaim_first": 0,
    "t2_power_at": 3, "t3_power_at": 80}

def spec(name, d):
    return name + ":" + ",".join(f"{k}={v}" for k, v in d.items())

def run(plans, tag):
    """plans: {name: dials}. Returns {name: [minutes per game]} and raw lines."""
    f = os.path.join(SCR, f"plans_{tag}.txt")
    open(f, "w").write("\n".join(spec(n, d) for n, d in plans.items()))
    env = dict(os.environ, ECO=MAPS, ECO_PLANS="@" + f, ECO_MIN=str(int(CAP)))
    out = subprocess.run([BIN, "zz_eco_race_probe::", "--ignored", "--nocapture", "--test-threads=1"],
                         env=env, capture_output=True, text=True, cwd=ROOT + "/crates/mc-sim").stdout
    open(os.path.join(SCR, f"out_{tag}.txt"), "w").write(out)
    res = {}
    for line in out.splitlines():
        if not line.startswith("RESULT"):
            continue
        kv = dict(x.split("=", 1) for x in line.split()[1:] if "=" in x)
        t = float(kv["dc"]) if kv["dc"].replace(".", "").isdigit() else CAP + 5
        res.setdefault(kv["plan"], []).append((kv["map"] + ":" + kv["start"], t))
    return res

def score(games):
    return statistics.mean(t for _, t in games)

if __name__ == "__main__":
    base = dict(DEFAULT)
    if len(sys.argv) > 1 and sys.argv[1] not in ("", "-"):
        base.update(json.loads(sys.argv[1]))
    rounds = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    only = os.environ.get("DIALS")
    for rnd in range(rounds):
        plans = {"best": base}
        for k, vals in ALTS.items():
            if only and k not in only.split(","):
                continue
            for v in vals:
                if v != base[k]:
                    plans[f"{k}={v}"] = dict(base, **{k: v})
        res = run(plans, f"r{rnd}")
        b = score(res["best"])
        rows = sorted(((score(g), n) for n, g in res.items()), key=lambda x: x[0])
        print(f"round {rnd}: best {b:.2f} min  games {sorted(res['best'])}", flush=True)
        for s, n in rows:
            print(f"   {s:6.2f} {s - b:+6.2f}  {n}", flush=True)
        # adopt the best value per dial that beat the base by > 0.15 min
        change = {}
        for s, n in rows:
            if n == "best" or s > b - 0.15:
                continue
            k, v = n.split("=")
            if k not in change:
                change[k] = type(base[k])(float(v)) if isinstance(base[k], int) else float(v)
        if not change:
            print("no improvement; done", flush=True)
            break
        combo = dict(base, **change)
        greedy = dict(base, **dict([next(iter(change.items()))]))
        chk = run({"combo": combo, "greedy": greedy, "best": base}, f"r{rnd}c")
        cs, gs = score(chk["combo"]), score(chk["greedy"])
        print(f"   combo {change} -> {cs:.2f}; top single -> {gs:.2f}", flush=True)
        base = combo if cs <= gs else greedy
        print("BASE " + json.dumps(base), flush=True)
