//! The interface set: written here in code, not in the sound library.

use crate::buf::{Air, Buf, Noise};

/// Interface and notification sounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// The pointer or keyboard focus moved onto a control.
    Hover,
    /// A control was activated.
    Select,
    /// Leaving a screen, cancelling.
    Back,
    ToggleOn,
    ToggleOff,
    /// One step of a slider or a cycled value.
    Tick,
    /// The control is unavailable or the action was refused.
    Deny,
    /// The match is starting.
    Launch,
    /// Units acknowledged an order.
    Order,
    Victory,
    Defeat,
}

impl Sfx {
    pub const ALL: [Sfx; 11] = [
        Sfx::Hover,
        Sfx::Select,
        Sfx::Back,
        Sfx::ToggleOn,
        Sfx::ToggleOff,
        Sfx::Tick,
        Sfx::Deny,
        Sfx::Launch,
        Sfx::Order,
        Sfx::Victory,
        Sfx::Defeat,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Sfx::Hover => "hover",
            Sfx::Select => "select",
            Sfx::Back => "back",
            Sfx::ToggleOn => "toggle_on",
            Sfx::ToggleOff => "toggle_off",
            Sfx::Tick => "tick",
            Sfx::Deny => "deny",
            Sfx::Launch => "launch",
            Sfx::Order => "order",
            Sfx::Victory => "victory",
            Sfx::Defeat => "defeat",
        }
    }
}

/// The interface set shares one voice: soft sine blips around A, a little air
/// from filtered noise, and the same short room on everything.
pub fn interface(sfx: Sfx, rate: u32) -> Vec<[f32; 2]> {
    match sfx {
        Sfx::Hover => {
            // Heard constantly, so it is the quietest sound in the set: a soft,
            // low tap with a slow attack, nothing bright in it and no noise.
            let mut b = Buf::new(rate, 0.30);
            b.tone(0.0, 622.3, 587.3, 0.04, 0.006, 0.022, 0.8, 0.0);
            b.tone(0.0, 311.1, 293.7, 0.04, 0.008, 0.030, 0.4, 0.0);
            b.space(0.35);
            b.finish(0.12)
        }
        // Into and out of a screen are the hover's tap given a body: up into D
        // going in, down off it coming out. Low and short, with nothing ringing on.
        Sfx::Select => {
            let mut b = Buf::new(rate, 0.45);
            b.tone(0.0, 493.9, 587.3, 0.03, 0.003, 0.035, 0.9, 0.0);
            b.tone(0.0, 246.9, 293.7, 0.03, 0.004, 0.045, 0.5, 0.0);
            b.tone(0.0, 140.0, 85.0, 0.05, 0.002, 0.035, 0.6, 0.0);
            b.hiss(0.0, 1500.0, 1.0, 0.0005, 0.004, 0.35, 0.0, 23);
            b.space(0.45);
            b.finish(0.30)
        }
        Sfx::Back => {
            let mut b = Buf::new(rate, 0.45);
            b.tone(0.0, 587.3, 440.0, 0.05, 0.004, 0.040, 0.9, 0.0);
            b.tone(0.0, 293.7, 220.0, 0.05, 0.005, 0.050, 0.5, 0.0);
            b.tone(0.0, 120.0, 70.0, 0.06, 0.002, 0.040, 0.5, 0.0);
            b.space(0.45);
            b.finish(0.26)
        }
        Sfx::ToggleOn | Sfx::ToggleOff => {
            let mut b = Buf::new(rate, 0.45);
            let (first, second) = if sfx == Sfx::ToggleOn {
                (1174.7, 1760.0)
            } else {
                (1760.0, 1174.7)
            };
            b.tone(0.0, first, first, 0.01, 0.002, 0.030, 0.9, -0.2);
            b.tone(0.055, second, second, 0.01, 0.002, 0.050, 1.0, 0.2);
            b.hiss(0.0, 2600.0, 1.0, 0.0005, 0.005, 0.6, 0.0, 41);
            b.space(0.6);
            b.finish(0.32)
        }
        Sfx::Tick => {
            let mut b = Buf::new(rate, 0.12);
            b.tone(0.0, 2637.0, 2349.3, 0.02, 0.001, 0.009, 0.9, 0.0);
            b.hiss(0.0, 3000.0, 1.5, 0.0005, 0.004, 0.7, 0.0, 53);
            b.space(0.3);
            b.finish(0.16)
        }
        Sfx::Deny => {
            let mut b = Buf::new(rate, 0.55);
            for start in [0.0, 0.11] {
                // A reedy buzz: a few odd harmonics, slightly detuned between the ears.
                for (k, gain) in [(1.0, 1.0), (3.0, 0.45), (5.0, 0.22), (7.0, 0.1)] {
                    b.tone(start, 155.6 * k, 146.8 * k, 0.09, 0.004, 0.045, gain, -0.3);
                    b.tone(start, 156.9 * k, 148.0 * k, 0.09, 0.004, 0.045, gain, 0.3);
                }
            }
            b.space(0.5);
            b.finish(0.34)
        }
        Sfx::Launch => {
            let mut b = Buf::new(rate, 3.2);
            let hit = 0.62;
            // Riser: noise through a band that climbs into the hit.
            {
                let (mut noise, mut air) = (Noise(79), Air::default());
                let r = b.rate;
                b.add(0.0, 0.0, |t| {
                    let k = (t / hit).min(1.0);
                    let gate = if t < hit {
                        k * k
                    } else {
                        (-(t - hit) / 0.03).exp()
                    };
                    air.step(noise.next(), 200.0 * (12.0f32).powf(k), 2.5, r) * gate * 0.8
                });
            }
            b.tone(0.0, 110.0, 220.0, hit, hit * 0.95, 0.05, 0.35, 0.0);
            // The hit: a falling sub, a crack, and an open fifth-and-ninth chord left ringing.
            b.tone(hit, 120.0, 41.2, 0.35, 0.004, 0.55, 1.6, 0.0);
            b.tone(hit, 240.0, 82.4, 0.25, 0.004, 0.22, 0.6, 0.0);
            b.hiss(hit, 1200.0, 0.7, 0.001, 0.09, 1.8, 0.0, 83);
            b.hiss(hit, 3200.0, 0.8, 0.001, 0.30, 0.4, 0.0, 89);
            for (i, (f, gain)) in [
                (220.0, 0.5),
                (329.6, 0.42),
                (440.0, 0.36),
                (493.9, 0.26),
                (659.3, 0.2),
                (880.0, 0.12),
            ]
            .into_iter()
            .enumerate()
            {
                let side = if i % 2 == 0 { -0.45 } else { 0.45 };
                b.tone(hit, f * 0.996, f * 0.996, 0.1, 0.012, 0.85, gain, side);
                b.tone(hit, f * 1.004, f * 1.004, 0.1, 0.012, 0.85, gain, -side);
            }
            b.space(1.0);
            b.finish(0.80)
        }
        Sfx::Order => {
            let mut b = Buf::new(rate, 0.35);
            b.tone(0.0, 987.8, 987.8, 0.01, 0.002, 0.028, 0.8, -0.15);
            b.tone(0.04, 1318.5, 1318.5, 0.01, 0.002, 0.040, 0.9, 0.15);
            b.space(0.5);
            b.finish(0.17)
        }
        Sfx::Victory | Sfx::Defeat => {
            let mut b = Buf::new(rate, 3.4);
            // The same four notes, climbing in the major or sinking in the minor.
            let notes: [f32; 4] = if sfx == Sfx::Victory {
                [293.7, 370.0, 440.0, 587.3]
            } else {
                [392.0, 349.2, 311.1, 233.1]
            };
            for (i, f) in notes.into_iter().enumerate() {
                let (start, last) = (i as f32 * 0.19, i == 3);
                let ring = if last { 1.1 } else { 0.32 };
                for (detune, side) in [(0.997, -0.4), (1.003, 0.4)] {
                    b.tone(start, f * detune, f * detune, 0.1, 0.006, ring, 0.6, side);
                    b.tone(
                        start,
                        f * 2.0 * detune,
                        f * 2.0 * detune,
                        0.1,
                        0.004,
                        ring * 0.5,
                        0.2,
                        -side,
                    );
                    b.tone(
                        start,
                        f * 0.5 * detune,
                        f * 0.5 * detune,
                        0.1,
                        0.010,
                        ring,
                        0.35,
                        0.0,
                    );
                }
            }
            b.space(1.0);
            b.finish(0.55)
        }
    }
}
