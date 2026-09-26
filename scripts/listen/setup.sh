#!/usr/bin/env bash
# Creates .venv-listen with the tools listen.sh uses: yt-dlp (fetching), Demucs
# (stem separation, CPU PyTorch), Basic Pitch (audio to notes), librosa (beats,
# onsets, features), music21 (keys, chords, Roman numerals, score import),
# imageio-ffmpeg (decoding: an ffmpeg binary from PyPI, no system install). About 2 GB. No sudo: the system
# Python here lacks ensurepip, so virtualenv (user install) makes the env.
set -euo pipefail
cd "$(dirname "$0")/../.."
python3 -m pip install -q --user virtualenv
python3 -m virtualenv -q .venv-listen
P=.venv-listen/bin/python
$P -m pip install -q torch torchaudio --index-url https://download.pytorch.org/whl/cpu
$P -m pip install -q yt-dlp demucs imageio-ffmpeg onnxruntime music21 mido soundfile numpy scipy librosa basic-pitch
echo "ready: ./listen.sh <url | file | score>"
