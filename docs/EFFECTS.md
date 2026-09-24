# Unit dust and shockwaves

Each unit entry in a faction's units RON files accepts an optional `effects` block:

```ron
effects: (
    dust_color: Some((0.7, 0.45, 0.22)),
    dust_opacity: 1.3,
    dust_brightness: 1.1,
    dust_lifetime: 1.5,
    shockwave_color: Some((0.25, 0.65, 1.0)),
),
```

- `dust_color`: optional linear RGB base color, each channel 0–1. Default `None` keeps the natural dust/smoke colors. Shaded folds and rolling texture remain visible.
- `dust_opacity`: opacity multiplier, 0–4; default 1. Zero disables dust and smoke emission. The existing name `dust_visibility` is an alias; use only one spelling.
- `dust_brightness`: lighting multiplier, 0–4; default 1. Zero makes dark dust; it does not make the cloud transparent. Brightness does not change opacity or lifetime.
- `dust_lifetime`: lifetime multiplier, 0–10; default 1. Zero disables emission.
- `shockwave_color`: optional linear RGB channels, each 0–1. `None` keeps the natural weapon tint. A specified color tints the faint transparent surface and its brighter edge; ground dust uses its separate `dust_color` setting.

Settings belong to the emitting unit blueprint: its weapons, impacts, delayed Shatter bursts, destruction, and movement dust all use them. They change presentation, not damage or shield strength. Reload the game after editing definitions.

Pressure fronts lose density as they expand. Larger overlapping ground clouds roll using material coordinates attached to the moving cloud, with a terrain contact fade. Scene refraction is depth tested so objects in front of the wave remain undisturbed.

Live shield membranes clip the propagating wave, refraction, flashes and particles. Blast damage snapshots protection before applying damage, so the blast that exhausts a shield cannot leak through to later victims. Units sharing the same shield interior can still be hit by explosions originating there.
