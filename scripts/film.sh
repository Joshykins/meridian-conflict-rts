#!/usr/bin/env bash
# Film the game: each cut of a reel is a headless match the game films itself
# (meridian --film: a camera move over the fight, no interface), and the cuts
# are crossfaded, graded and encoded into one looping video.
#
#   scripts/film.sh [options] REEL
#
# REEL is a file of cuts, one a line (a trailing \ continues a line, # starts a
# comment): a name, then the game's arguments for that cut: the map and armies
# (--blue/--red KEY:N, the matchup scene), --ticks to play before rolling, and
# the camera move (--film-secs, --film-speed, --film-from, --film-to, --film-on;
# see meridian --help). scripts/film/hero.reel is the website's background.
#
# Options:
#   --size WxH     the frames' size (default 1920x1080)
#   --height H     the video's height, scaled from the frames (default 1080)
#   --fps N        frames a second (default 30)
#   --budget MB    the most each encoded file may weigh (default 4.6: the
#                  pre-commit hook refuses files over 5 MB)
#   --only NAME    film only that cut (repeatable), the others keep their last
#                  frames: for trying a move
#   --reuse        encode the frames already filmed; film nothing
#   --site         also copy the result into site/public/hero/battle.{webm,mp4,jpg}
#   --no-build     use the last build as it is
#
# Writes artifacts/films/REEL.{webm,mp4,jpg} (VP9, H.264 and a poster frame).
# Needs ffmpeg in WSL: a static build in ~/.local/bin is enough
# (https://johnvansickle.com/ffmpeg/). A cut takes a minute or two: the game
# plays its --ticks, then draws and writes each frame (~20 a second at 1080p).
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
size=1920x1080 height=1080 fps=30 budget=4.6 reuse=0 site=0
only=() shot_opts=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --size) size="$2"; shift 2 ;;
        --height) height="$2"; shift 2 ;;
        --fps) fps="$2"; shift 2 ;;
        --budget) budget="$2"; shift 2 ;;
        --only) only+=("$2"); shift 2 ;;
        --reuse) reuse=1; shift ;;
        --site) site=1; shift ;;
        --no-build) shot_opts+=(--no-build); shift ;;
        -h|--help) sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "film.sh: unknown option $1" >&2; exit 2 ;;
        *) break ;;
    esac
done
[[ $# -eq 1 && -f $1 ]] || { echo "usage: scripts/film.sh [options] REEL (scripts/film.sh --help)" >&2; exit 2; }
reel="$1"
command -v ffmpeg > /dev/null || { echo "film.sh: no ffmpeg on PATH (see --help)" >&2; exit 1; }
stem=$(basename "$reel" .reel)

# Frames go to Windows' temp dir, where the game writes fast; ffmpeg reads them from here.
temp_cache="/tmp/meridian-shot-wintemp"
[[ -s $temp_cache ]] || wslpath "$(powershell.exe -NoProfile -Command '[IO.Path]::GetTempPath()' | tr -d '\r')" > "$temp_cache"
frames_root="$(cat "$temp_cache")/meridian-film/$(basename "$repo")/$stem"
out="$repo/artifacts/films"
mkdir -p "$frames_root" "$out"

# The reel's cuts: names, and each one's arguments.
names=() cut_args=()
line=""
while IFS= read -r raw || [[ -n $raw ]]; do
    raw="${raw%%#*}"
    if [[ $raw =~ \\[[:space:]]*$ ]]; then
        line+="${raw%\\*} "
        continue
    fi
    line+="$raw"
    read -ra words <<< "$line"
    line=""
    [[ ${#words[@]} -gt 0 ]] || continue
    names+=("${words[0]}")
    cut_args+=("${words[*]:1}")
done < "$reel"
[[ ${#names[@]} -gt 0 ]] || { echo "film.sh: no cuts in $reel" >&2; exit 1; }

# Film each cut. The first builds the game (if it needs it); the rest reuse that build.
for i in "${!names[@]}"; do
    name=${names[$i]}
    dir="$frames_root/$name"
    if [[ $reuse == 1 || (${#only[@]} -gt 0 && " ${only[*]} " != *" $name "*) ]]; then
        [[ -f $dir/00000.png ]] || { echo "film.sh: $name has no frames yet: film it" >&2; exit 1; }
        continue
    fi
    rm -rf "$dir"
    echo "film.sh: filming $name" >&2
    read -ra args <<< "${cut_args[$i]}"
    "$repo/scripts/shot.sh" "${shot_opts[@]}" --timeout 1200 -o "film-$name" run \
        "${args[@]}" --size "$size" --film "$(wslpath -w "$dir")" --film-fps "$fps" \
        | grep -E '^(filmed|error|thread)|panicked' || true
    [[ -f $dir/00000.png ]] || { echo "film.sh: $name wrote no frames" >&2; exit 1; }
    shot_opts=(--no-build)
done

# Crossfade the cuts in reel order, fade the whole in and out of black so the
# loop's seam is soft, and grade it a little: deeper blacks, a touch more colour
# and a vignette.
fade=0.6 edge=0.5
inputs=() chain="" offset=0 last="[0:v]"
for i in "${!names[@]}"; do
    dir="$frames_root/${names[$i]}"
    count=$(find "$dir" -maxdepth 1 -name '[0-9]*.png' | wc -l)
    secs=$(awk -v n="$count" -v f="$fps" 'BEGIN { printf "%.4f", n / f }')
    inputs+=(-framerate "$fps" -i "$dir/%05d.png")
    if (( i > 0 )); then
        chain+="${last}[$i:v]xfade=transition=fade:duration=$fade:offset=$offset[x$i];"
        last="[x$i]"
    fi
    # Where the next cut starts fading in: this one's end, less the fade.
    offset=$(awk -v o="$offset" -v s="$secs" -v f="$fade" 'BEGIN { printf "%.4f", o + s - f }')
done
total=$(awk -v o="$offset" -v f="$fade" 'BEGIN { printf "%.4f", o + f }')
end=$(awk -v t="$total" -v e="$edge" 'BEGIN { printf "%.4f", t - e }')
graph="${chain}${last}scale=-2:$height:flags=lanczos,eq=contrast=1.06:saturation=1.1:gamma=0.97,vignette=angle=PI/5,fade=t=in:st=0:d=$edge,fade=t=out:st=$end:d=$edge,format=yuv420p[v]"
# Bits a second that fit the budget, less a little for the container.
kbps=$(awk -v mb="$budget" -v t="$total" 'BEGIN { k = mb * 8000 * 0.97 / t; if (k > 6000) k = 6000; printf "%d", k }')
echo "film.sh: ${#names[@]} cuts, $total s, $kbps kbit/s" >&2

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
ffmpeg -hide_banner -loglevel error -y "${inputs[@]}" -filter_complex "$graph" -map '[v]' \
    -c:v ffv1 -level 3 "$work/master.mkv"
(
    cd "$work"
    ffmpeg -hide_banner -loglevel error -y -i master.mkv -c:v libvpx-vp9 -b:v "${kbps}k" \
        -row-mt 1 -tile-columns 2 -deadline good -cpu-used 2 -pass 1 -an -f null /dev/null
    ffmpeg -hide_banner -loglevel error -y -i master.mkv -c:v libvpx-vp9 -b:v "${kbps}k" \
        -row-mt 1 -tile-columns 2 -deadline good -cpu-used 2 -pass 2 -an "$out/$stem.webm"
    ffmpeg -hide_banner -loglevel error -y -i master.mkv -c:v libx264 -preset slow -b:v "${kbps}k" \
        -pass 1 -an -f null /dev/null
    ffmpeg -hide_banner -loglevel error -y -i master.mkv -c:v libx264 -preset slow -b:v "${kbps}k" \
        -pass 2 -profile:v high -movflags +faststart -an "$out/$stem.mp4"
)
# The poster: a frame a little way into the first cut, after its fade.
ffmpeg -hide_banner -loglevel error -y -ss 1.5 -i "$work/master.mkv" -frames:v 1 -q:v 3 "$out/$stem.jpg"

for f in "$out/$stem".{webm,mp4,jpg}; do
    echo "$f ($(du -h "$f" | cut -f1))"
done
if [[ $site == 1 ]]; then
    mkdir -p "$repo/site/public/hero"
    for ext in webm mp4 jpg; do
        cp "$out/$stem.$ext" "$repo/site/public/hero/battle.$ext"
    done
    echo "copied into site/public/hero/" >&2
fi
