//! Guard sample integrity and rapid-fire spacing of the actual faction recipes.
use mc_data::SoundLibrary;

#[test]
fn repeaters_keep_headroom_audible_body_and_short_tails() {
    let library =
        SoundLibrary::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    for name in [
        "regency_repeater",
        "regency_repeater_hit",
        "regency_repeater_light",
        "regency_repeater_light_hit",
        "regency_canopy_fire",
        "regency_brazier_fire",
        "regency_stinger",
    ] {
        let sound = library.sound(library.id_of(name).unwrap());
        for rate in [24000, 48000] {
            let frames = mc_sfx::synth(sound, rate);
            let mut energy = 0.0;
            for frame in &frames {
                for sample in frame {
                    assert!(sample.is_finite() && sample.abs() < 0.65, "{name}");
                    energy += sample * sample;
                }
            }
            assert!(energy / frames.len() as f32 > 0.0001, "{name}: inaudible");
            // Technical guards only: the weapon's timbre is judged by audition,
            // not by imposing a bright spectral target on a deliberately deep pulse.
            let tail = &frames[frames.len() - (rate / 50) as usize..];
            let tail_peak = tail.iter().flatten().fold(0.0_f32, |p, s| p.max(s.abs()));
            assert!(
                tail_peak < sound.peak * 0.035,
                "{name}: truncated tail {tail_peak}"
            );
            if name == "regency_canopy_fire" || name == "regency_brazier_fire" {
                let late = &frames[(rate as f32 * 0.22) as usize..];
                let peak = late.iter().flatten().fold(0.0_f32, |p, s| p.max(s.abs()));
                assert!(
                    peak < sound.peak * 0.12,
                    "{name}: phantom second round {peak}"
                );
            }
        }
    }
}
