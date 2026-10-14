#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --profile gate -p mc-map --bin mc-bake -- --layout vela --size-km 12 --seed 5 --name "Vela" -o maps/vela_m1/terrain.mcmap
echo "Vela terrain is ready. Open The Contract from the main menu, or --editor vela_m1."
