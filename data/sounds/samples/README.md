# Regency repeater F

`regency-repeater-f.wav` is the exact stereo, 48 kHz, 16-bit PCM single shot
from audition **F - Accelerator**, selected by the user on 2026-10-07.
It was generated for Meridian Conflict, not sampled from a third-party game.
The single-shot source is 0.62 seconds; the audition's silences and repeated
shots are not part of this asset.

The source recording stays intact. Playback is reduced by about 3 dB from the
audition level to balance the rapid-fire repeater against the Warden cannon,
and uses no extra room. Light
weapons and impacts use the recipe's normal `like`/`size` scaling. The commander
places three copies 0.1 seconds apart; AA fires one copy per simulation event.

The regression test compares the game synthesizer's 48 kHz result to this PCM
waveform after the configured gain adjustment. Sound-quality approval came from the user's audition choice.
