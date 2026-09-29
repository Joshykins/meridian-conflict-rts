//! The layers of the living world's sounds (`Layer::Wind`, `Chirp`, `Chorus`):
//! the ambient bed of wind, leaves and surf, and the birds, insects and frogs
//! that call over it (`ambience.rs` plays them, `data/sounds/ambience.ron` has
//! the recipes).
//!
//! Nature is never steady. Wind is noise whose level and band both wander at
//! random, never a sine's wobble; a call is a pitch that moves; a chorus is
//! many callers, none keeping another's time or pitch, each fading in and out
//! on its own, so a loop of it has no beat to give its length away.

use crate::buf::{pluck, Air, Buf, Noise};
use mc_data::sounds::Layer;
use std::f32::consts::TAU;
use std::f64::consts::TAU as TAU64;

/// A smooth random level between zero and one: a value at every whole `x`, eased
/// between them. The same `seed` always wanders the same way. It keeps the two
/// knots it is between, since it is read once a sample and moves on slowly.
struct Wander {
    seed: u32,
    knot: i64,
    ends: (f32, f32),
}

impl Wander {
    fn new(seed: u32) -> Wander {
        let mut w = Wander {
            seed,
            knot: 0,
            ends: (0.0, 0.0),
        };
        w.ends = (w.value(0), w.value(1));
        w
    }

    fn value(&self, k: i64) -> f32 {
        let mut n =
            Noise((self.seed ^ (k as u32).wrapping_mul(0x9E37_79B9)).wrapping_add(0x6D2B_79F5) | 1);
        n.next();
        n.next();
        n.next() * 0.5 + 0.5
    }

    fn at(&mut self, x: f32) -> f32 {
        let k = x.floor();
        if k as i64 != self.knot {
            self.knot = k as i64;
            self.ends = (self.value(self.knot), self.value(self.knot + 1));
        }
        let f = x - k;
        let e = f * f * (3.0 - 2.0 * f);
        self.ends.0 * (1.0 - e) + self.ends.1 * e
    }
}

/// Lays down one of this module's layers. `seed` is the layer's own when it names
/// none, `looped` the loop's length in seconds (zero for a sound played once), to
/// which a chorus tunes its rates so the loop joins without a seam.
pub(crate) fn layer(b: &mut Buf, layer: &Layer, seed: u32, looped: f32) {
    match *layer {
        Layer::Wind {
            freq,
            q,
            gain,
            swell,
            depth,
            sway,
            pan,
            seed: own,
        } => wind(
            b,
            freq,
            q,
            gain,
            swell,
            depth,
            sway,
            pan,
            own.unwrap_or(seed),
        ),
        Layer::Chirp {
            at,
            from,
            to,
            glide,
            attack,
            decay,
            gain,
            pan,
            vibrato,
            bend,
            pulse,
            ratio,
            index,
        } => {
            let rate = b.rate as f64;
            let (mut carrier, mut shaker) = (0.0f64, 0.0f64);
            b.add(at, pan, |t| {
                let k = (t / glide.max(1e-4)).min(1.0);
                let f = from + (to - from) * (1.0 - (1.0 - k) * (1.0 - k));
                let f = f * (1.0 + bend * (std::f32::consts::TAU * vibrato * t).sin());
                let env = pluck(t, attack, decay);
                if env < 1e-5 && t > attack {
                    return 0.0;
                }
                let beat = if pulse > 0.0 {
                    let s = (std::f32::consts::PI * pulse * t).sin();
                    s * s
                } else {
                    1.0
                };
                carrier = (carrier + TAU64 * f as f64 / rate) % TAU64;
                shaker = (shaker + TAU64 * (f * ratio) as f64 / rate) % TAU64;
                (carrier as f32 + index * env * (shaker as f32).sin()).sin() * env * beat * gain
            });
        }
        Layer::Chorus {
            freq,
            voices,
            spread,
            pulse,
            chirps,
            duty,
            gain,
            width,
            bend,
            ratio,
            index,
            seed: own,
        } => chorus(
            b,
            Chorus {
                freq,
                voices,
                spread,
                pulse,
                chirps,
                duty: duty.clamp(0.02, 1.0),
                gain,
                width,
                bend,
                ratio,
                index,
            },
            own.unwrap_or(seed),
            looped,
        ),
        _ => unreachable!("not a nature layer"),
    }
}

/// Noise through a band that wanders with the level: the band's centre goes up by
/// as much as `sway` octaves as a gust swells and falls back as it dies.
fn wind(
    b: &mut Buf,
    freq: f32,
    q: f32,
    gain: f32,
    swell: f32,
    depth: f32,
    sway: f32,
    pan: f32,
    seed: u32,
) {
    let (mut noise, mut air) = (Noise(seed), Air::default());
    let (mut slow, mut fast) = (
        Wander::new(seed ^ 0x5EED),
        Wander::new(seed.rotate_left(11) ^ 0xA11),
    );
    let depth = depth.clamp(0.0, 1.0);
    let rate = b.rate;
    b.add(0.0, pan, |t| {
        // Two rates of wandering, the faster lighter, raised to the fourth power so the
        // gusts stand out of the lulls (about 1 on average), as `Roll` does.
        let v = 0.62 * slow.at(t * swell) + 0.38 * fast.at(t * swell * 2.37 + 17.0);
        let level = 1.0 - depth + depth * 7.5 * (v * v) * (v * v);
        let cutoff = freq * (sway * (v - 0.5) * 2.0).exp2();
        air.step(noise.next(), cutoff, q, rate) * gain * level
    });
}

struct Chorus {
    freq: f32,
    voices: u32,
    spread: f32,
    pulse: f32,
    chirps: f32,
    duty: f32,
    gain: f32,
    width: f32,
    bend: f32,
    ratio: f32,
    index: f32,
}

fn chorus(b: &mut Buf, c: Chorus, seed: u32, looped: f32) {
    let mut dice = Noise(seed | 1);
    let mut roll = move || dice.next() * 0.5 + 0.5;
    // Rates tuned to whole cycles of a loop, never to none at all.
    let whole = |hz: f32| {
        if looped > 0.0 {
            (hz * looped).round().max(1.0) / looped
        } else {
            hz
        }
    };
    let rate = b.rate as f64;
    for _ in 0..c.voices.max(1) {
        let pitch = whole(c.freq * (1.0 + c.spread * (2.0 * roll() - 1.0))) as f64;
        let cycle = whole(c.chirps * (0.75 + 0.5 * roll())) as f64;
        let offset = roll() as f64;
        let syllables = c.pulse * (0.85 + 0.3 * roll());
        let level = 0.45 + 0.55 * roll();
        let pan = c.width * (2.0 * roll() - 1.0);
        // Each voice comes and goes on its own slow tide.
        let tide = whole(0.025 + 0.07 * roll()) as f64;
        let tide_at = roll() as f64;
        let mut n = 0u64;
        let c = &c;
        b.add(0.0, pan, move |_| {
            let t = n as f64 / rate;
            n += 1;
            let g = (t * cycle + offset).fract() as f32;
            if g >= c.duty {
                return 0.0;
            }
            let swell = 0.5 + 0.5 * (TAU * (tide * t + tide_at).fract() as f32).sin();
            let swell = swell * swell.sqrt();
            if swell < 0.01 {
                return 0.0;
            }
            let into = g / c.duty;
            let shape = (std::f32::consts::PI * into).sin().sqrt();
            // Seconds into the chirp, then into its syllable.
            let s = g as f64 / cycle;
            let (beat, since) = if c.pulse > 0.0 {
                let u = s * syllables as f64;
                let b = (std::f32::consts::PI * u.fract() as f32).sin();
                (b * b, u.fract() / syllables as f64)
            } else {
                (1.0, s)
            };
            // Each syllable slides onto the note from `bend` above it (below, if negative):
            // the phase gained on the way, so the note itself stays whole-cycle tuned.
            let settle = 0.018f32;
            let slide = pitch as f32 * c.bend * settle * (1.0 - (-(since as f32) / settle).exp());
            let x = TAU * ((pitch * t).fract() as f32 + slide.fract());
            let env = shape * beat;
            (x + c.index * env * (c.ratio * x).sin()).sin() * env * swell * level * c.gain
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::synth as from_recipe;
    use mc_data::SoundLibrary;

    /// The ambience's sounds (`data/sounds/ambience.ron`) are not held tones or
    /// steady hiss: every bed's level moves by several decibels over its loop,
    /// and every call's brightest pitch moves over the call. Also says how long
    /// they take to make, since the whole bank waits on them at start-up.
    #[test]
    fn nature_sounds_move() {
        let library = SoundLibrary::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let rate = 44_100;
        let started = std::time::Instant::now();
        let mut seen = 0;
        for sound in &library.sounds {
            // Only the ambience file: a Wind layer elsewhere (a Regency builder's
            // particle rush) is a texture under a held machine, not a bed.
            let nature = sound.file == "ambience"
                && sound.layers.iter().any(|l| {
                    matches!(
                        l,
                        mc_data::sounds::Layer::Wind { .. }
                            | mc_data::sounds::Layer::Chirp { .. }
                            | mc_data::sounds::Layer::Chorus { .. }
                    )
                });
            if !nature {
                continue;
            }
            seen += 1;
            let frames = from_recipe(sound, rate);
            let mono: Vec<f32> = frames.iter().map(|f| (f[0] + f[1]) * 0.5).collect();
            let top = frames.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                frames.iter().flatten().all(|s| s.is_finite()) && (0.1..=0.9).contains(&top),
                "{} peaks at {top}",
                sound.name
            );
            let step = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs().max((a[1] - b[1]).abs());
            if sound.looped {
                let usual = frames
                    .windows(2)
                    .map(|w| step(w[0], w[1]))
                    .fold(0.0f32, f32::max);
                let seam = step(frames[frames.len() - 1], frames[0]);
                assert!(
                    seam <= usual * 1.5 + 1e-4,
                    "{}: a jump of {seam} at the seam",
                    sound.name
                );
            } else {
                let tail = frames[frames.len() - 1200..frames.len() - 600]
                    .iter()
                    .flatten()
                    .fold(0.0f32, |m, s| m.max(s.abs()));
                assert!(
                    frames[0].iter().all(|s| s.abs() < 0.02),
                    "{} starts with a click",
                    sound.name
                );
                assert!(tail < 0.06 * top, "{} is cut off at {tail}", sound.name);
            }
            if sound.looped {
                let hop = rate as usize / 20;
                let mut levels: Vec<f32> = mono
                    .chunks_exact(hop)
                    .map(|c| {
                        (c.iter().map(|s| s * s).sum::<f32>() / hop as f32)
                            .sqrt()
                            .max(1e-6)
                    })
                    .collect();
                levels.sort_by(f32::total_cmp);
                let (low, high) = (levels[levels.len() / 10], levels[levels.len() * 9 / 10]);
                let swing = 20.0 * (high / low).log10();
                assert!(
                    swing > 4.0,
                    "{}: its level only moves {swing:.1} dB",
                    sound.name
                );
            } else {
                // Zero crossings in 10 ms windows stand in for the pitch of a call.
                let hop = rate as usize / 100;
                let peak = mono.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                let pitches: Vec<f32> = mono
                    .chunks_exact(hop)
                    .filter(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs())) > peak * 0.3)
                    .map(|c| {
                        c.windows(2)
                            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
                            .count() as f32
                    })
                    .collect();
                let (low, high) = pitches
                    .iter()
                    .fold((f32::MAX, 0.0f32), |(l, h), &p| (l.min(p), h.max(p)));
                assert!(
                    high > low * 1.08,
                    "{}: its pitch holds at {low}..{high} crossings",
                    sound.name
                );
            }
        }
        assert!(seen >= 20, "{seen} nature sounds");
        eprintln!(
            "{seen} nature sounds made in {:.0} ms",
            started.elapsed().as_secs_f32() * 1000.0
        );
    }
}
