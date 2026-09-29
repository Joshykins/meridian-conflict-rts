#!/usr/bin/env bash
# Renders a fixed set of scenes on the native GPU (or Windows GPU from WSL) and collects the
# reports, so any renderer or sim change can be compared with `mc-perf diff`.
#
#   scripts/perf-suite.sh OUT_DIR [CASE...]      (from WSL; builds first)
#   PERF_SIZE=2560x1440 PERF_FOLLOW=30 PERF_TARGET=meridian-target-x scripts/perf-suite.sh ...
#   cargo run -q -p mc-core --bin mc-perf -- diff OLD/matchup_low.frames.json NEW/matchup_low.frames.json
#
# Each case writes OUT_DIR/<case>.sim.{json,txt} (the ticks before rendering)
# and OUT_DIR/<case>.frames.{json,txt} (every followed frame: GPU scopes with
# triangle and fragment counts, CPU render time, and the tick each frame showed).
# Extra environment for every case (e.g. MERIDIAN_AA=off) is passed through
# PERF_ENV="MERIDIAN_AA=off MERIDIAN_GTAO=0".
#
# GPU times swing when something else uses the GPU (the game running, other
# builds); compare medians, rerun a case whose untouched scopes moved, and
# trust counters (triangles, fragments, queries) over times.
set -euo pipefail

out=${1:?usage: perf-suite.sh OUT_DIR [CASE...]}
shift
size=${PERF_SIZE:-5120x1440}
follow=${PERF_FOLLOW:-45}
target=${PERF_TARGET:-meridian-target-perf}

paladins='--blue aster_t3_assault_bot:100 --red aster_t5_titan:1'
# name | extra env | arguments
cases=(
  "battle_mid||--scene battle --ticks 200 --camera 8192,8192,700,25"
  "battle_low|MERIDIAN_TILT=2|--scene battle --ticks 200 --camera 8192,8192,700,25"
  "matchup_mid||--no-fog $paladins --ticks 250 --camera 8192,8192,450,0"
  "matchup_low|MERIDIAN_TILT=2|--no-fog $paladins --ticks 250 --camera 8192,8192,450,0"
  "overview_far||--scene battle --ticks 200 --camera 8192,8192,30000,0"
  "overcast_far|MERIDIAN_WEATHER=overcast|--scene battle --ticks 200 --camera 8192,8192,30000,0"
  "overcast_low|MERIDIAN_WEATHER=overcast MERIDIAN_TILT=2|--scene battle --ticks 200 --camera 8192,8192,900,25"
)
want=("$@")

# Native macOS/Linux: use play.sh for Rust/Vulkan discovery. PERF_EXE may point
# to an already built binary, so A/B runs can use exactly the same build.
platform=$(uname -s)
if [ "$platform" = Darwin ] || { [ "$platform" = Linux ] && [ -z "${WSL_INTEROP:-}${WSL_DISTRO_NAME:-}" ] && [[ $(uname -r) != *[Mm]icrosoft* ]]; }; then
  root=$(cd "$(dirname "$0")/.." && pwd)
  mkdir -p "$out"
  out=$(cd "$out" && pwd)
  cd "$root"
  run=("$root/play.sh")
  if [ -n "${PERF_EXE:-}" ]; then
    run=("$PERF_EXE")
    if [ "$platform" = Darwin ] && command -v brew >/dev/null 2>&1; then
      brew_prefix=$(brew --prefix)
      export DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:+$DYLD_FALLBACK_LIBRARY_PATH:}$brew_prefix/lib:/usr/local/lib:/usr/lib"
      if [ -z "${VK_DRIVER_FILES:-}${VK_ICD_FILENAMES:-}" ]; then
        export VK_DRIVER_FILES="$brew_prefix/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json"
      fi
    fi
  fi
  for c in "${cases[@]}"; do
    IFS='|' read -r name case_env args <<<"$c"
    if ((${#want[@]})) && [[ ! " ${want[*]} " == *" $name "* ]]; then continue; fi
    # Case arguments and PERF_ENV are whitespace-separated tokens, never shell code.
    read -r -a case_args <<<"$args"
    read -r -a env_args <<<"$case_env ${PERF_ENV:-}"
    echo "== $name ($case_env ${PERF_ENV:-})"
    if ! env ${env_args[@]+"${env_args[@]}"} "${run[@]}" "${case_args[@]}" --follow "$follow" --size "$size" \
      --screenshot "$out/$name.png" --perf "$out/$name.json" > "$out/$name.log" 2>&1; then
      cat "$out/$name.log" >&2
      exit 1
    fi
    # Fail on missing reports too: never silently benchmark an old/failed build.
    test -s "$out/$name.frames.json"
    grep -E 'frame ms|perf report' "$out/$name.log"
  done
  echo "reports in $out"
  exit 0
fi

repo_win=$(wslpath -w "$(cd "$(dirname "$0")/.." && pwd)")
temp_win='C:\Users\joshu\AppData\Local\Temp'
out_win_dir="$temp_win\\meridian-perf-$$"
exe="$temp_win\\$target\\release\\meridian.exe"

echo "building $target ..."
powershell.exe -NoProfile -Command "\$env:CARGO_TARGET_DIR='$temp_win\\$target'; Set-Location '$repo_win'; cargo build --release -p mc-game 2>&1 | Select-Object -Last 3" | tr -d '\r' || true

mkdir -p "$out"
for c in "${cases[@]}"; do
  IFS='|' read -r name env args <<<"$c"
  if ((${#want[@]})) && [[ ! " ${want[*]} " == *" $name "* ]]; then continue; fi
  sets=""
  for kv in $env ${PERF_ENV:-}; do sets+="\$env:${kv%%=*}='${kv#*=}'; "; done
  echo "== $name ($env ${PERF_ENV:-})"
  # Another process on the GPU (the game left running, another run) inflates every time.
  busy=$(nvidia-smi.exe --query-gpu=utilization.gpu --format=csv,noheader,nounits 2>/dev/null | tr -d '\r ' || true)
  if [[ -n "$busy" && "$busy" -gt 20 ]]; then
    echo "   WARNING: GPU already ${busy}% busy before this case; its times are inflated (counters are fine)"
    tasklist.exe 2>/dev/null | grep -i meridian | sed 's/^/   /' || true
  fi
  powershell.exe -NoProfile -Command "$sets Set-Location '$repo_win'; New-Item -ItemType Directory -Force '$out_win_dir' | Out-Null; & '$exe' $args --follow $follow --size $size --screenshot '$out_win_dir\\$name.png' --perf '$out_win_dir\\$name.json' 2>&1 | Select-String -Pattern 'frame ms|perf report|error|panicked' | ForEach-Object { \$_.Line }" | tr -d '\r' || true
  src=$(wslpath "$out_win_dir")
  cp "$src/$name".* "$out/" 2>/dev/null || echo "   (no report for $name)"
done
echo "reports in $out"
