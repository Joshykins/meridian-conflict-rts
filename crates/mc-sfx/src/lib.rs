//! The game's sounds as samples: a library sound (`mc_data::sounds`) made from
//! its recipe, and the interface set written here in code. The game renders
//! them all at start-up; the studio renders one at a time to play it, whole or
//! one layer alone. Nothing here reaches the simulation.

mod buf;
mod interface;
mod nature;

pub use buf::pan;
pub use interface::{interface, Sfx};

use buf::{Air, Buf, Noise};
use mc_data::sounds::{Layer, Sound};
use std::f32::consts::TAU;

/// A library sound from its recipe. Noise layers that name no seed get one
/// from the sound's name and their place in it, so every run sounds the same
/// and two sounds `like` each other still differ in their grain.
pub fn synth(sound: &Sound, rate: u32) -> Vec<[f32; 2]> {
    let (b, whole) = build(sound, rate, None, None);
    b.scaled(sound.peak / whole.top, !sound.looped)
}

/// Layer `layer` of `sound` alone, as loud as it is in the whole sound: the
/// same room, and any `Drive` squashing it against the whole sound's ceiling.
pub fn synth_layer(sound: &Sound, rate: u32, layer: usize) -> Vec<[f32; 2]> {
    let (_, whole) = build(sound, rate, None, None);
    let (b, _) = build(sound, rate, Some(layer), Some(&whole));
    b.scaled(sound.peak / whole.top, !sound.looped)
}

/// The levels a whole sound reached: at each `Drive`, and at the end.
struct Levels {
    drives: Vec<f32>,
    top: f32,
}

/// Lays down every layer (or only `only`), then the room, then folds a loop.
/// `heard` is the whole sound's levels when one layer is being heard alone.
fn build(sound: &Sound, rate: u32, only: Option<usize>, heard: Option<&Levels>) -> (Buf, Levels) {
    // A loop is made a little long and its end folded back over its start.
    let fold = if sound.looped { 0.25 } else { 0.0 };
    let mut b = Buf::new(rate, sound.length + fold);
    let name_seed = sound.name.bytes().fold(0x811C_9DC5u32, |h, c| {
        (h ^ c as u32).wrapping_mul(0x0100_0193)
    }) | 1;
    // A loop's steady layers are tuned to whole cycles of it, so the fold is seamless.
    let whole = |hz: f32| {
        if sound.looped {
            (hz * sound.length).round().max(0.0) / sound.length
        } else {
            hz
        }
    };
    let mut drives = Vec::new();
    for (i, layer) in sound.layers.iter().enumerate() {
        let auto = name_seed.wrapping_add(i as u32 * 7919) | 1;
        if let Layer::Drive(amount) = layer {
            // Heard alone, a layer is squashed against the whole sound's ceiling, not its own.
            let top = heard
                .map(|h| h.drives[drives.len()])
                .unwrap_or_else(|| b.top());
            drives.push(top);
            b.drive(*amount, top);
            continue;
        }
        if only.is_some_and(|o| o != i) {
            continue;
        }
        match layer {
            Layer::Tone {
                at,
                from,
                to,
                glide,
                attack,
                decay,
                gain,
                pan,
            } => b.tone(*at, *from, *to, *glide, *attack, *decay, *gain, *pan),
            Layer::Stack {
                at,
                from,
                to,
                glide,
                attack,
                decay,
                gain,
                detune,
                width,
                partials,
            } => {
                for (multiple, level) in partials {
                    b.tone(
                        *at,
                        from * multiple,
                        to * multiple,
                        *glide,
                        *attack,
                        *decay,
                        gain * level,
                        -width,
                    );
                    b.tone(
                        *at,
                        from * multiple * detune,
                        to * multiple * detune,
                        *glide,
                        *attack,
                        *decay,
                        gain * level,
                        *width,
                    );
                }
            }
            Layer::Fm {
                at,
                from,
                to,
                glide,
                ratio,
                index,
                fade,
                attack,
                decay,
                gain,
                pan,
            } => b.fm(
                *at, *from, *to, *glide, *ratio, *index, *fade, *attack, *decay, *gain, *pan,
            ),
            Layer::Burst {
                at,
                low,
                high,
                attack,
                decay,
                gain,
                pan,
                seed,
            } => b.burst(
                *at,
                *low,
                *high,
                *attack,
                *decay,
                *gain,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Hiss {
                at,
                freq,
                q,
                attack,
                decay,
                gain,
                pan,
                seed,
            } => b.hiss(
                *at,
                *freq,
                *q,
                *attack,
                *decay,
                *gain,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Sweep {
                at,
                from,
                to,
                glide,
                q,
                attack,
                decay,
                gain,
                pan,
                seed,
            } => b.sweep(
                *at,
                *from,
                *to,
                *glide,
                *q,
                *attack,
                *decay,
                *gain,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Roll {
                at,
                from,
                to,
                glide,
                attack,
                decay,
                gain,
                swell,
                depth,
                pan,
                seed,
            } => b.roll(
                *at,
                *from,
                *to,
                *glide,
                *attack,
                *decay,
                *gain,
                *swell,
                *depth,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Rumble {
                freq,
                q,
                gain,
                wobble,
                depth,
                pan,
                seed,
            } => {
                let (mut noise, mut air, wobble) =
                    (Noise(seed.unwrap_or(auto)), Air::default(), whole(*wobble));
                let r = b.rate;
                b.add(0.0, *pan, |t| {
                    air.step(noise.next(), *freq, *q, r)
                        * gain
                        * (1.0 - depth * 0.5 * (1.0 + (TAU * wobble * t).sin()))
                });
            }
            Layer::Drone {
                freq,
                gain,
                wobble,
                depth,
                pan,
            } => {
                let (freq, wobble) = (whole(*freq), whole(*wobble));
                b.add(0.0, *pan, |t| {
                    (TAU * freq * t).sin()
                        * gain
                        * (1.0 - depth * 0.5 * (1.0 + (TAU * wobble * t).sin()))
                });
            }
            Layer::Drive(_) => {}
            Layer::Wind { .. } | Layer::Chirp { .. } | Layer::Chorus { .. } => nature::layer(
                &mut b,
                layer,
                auto,
                if sound.looped { sound.length } else { 0.0 },
            ),
        }
    }
    if sound.room > 0.0 {
        b.space(sound.room);
    }
    if sound.looped {
        b.fold_loop(fold);
    }
    let top = heard.map_or_else(|| b.top(), |h| h.top);
    (b, Levels { drives, top })
}
