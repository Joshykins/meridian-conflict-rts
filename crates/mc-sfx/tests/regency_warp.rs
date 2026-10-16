//! Render the actual recipes: sample continuity catches clicks and broken loop joins.
use mc_data::SoundLibrary;

#[test]
fn regency_warp_samples_have_headroom_and_continuous_joins() {
    let library = SoundLibrary::load(
        &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
    )
    .unwrap();
    for suffix in [
        "spool",
        "charge",
        "charge_arcs",
        "jump",
        "rift",
        "arrive",
        "jump_damped",
        "rift_damped",
        "arrive_damped",
        "snag",
    ] {
        let name = format!("regency_warp_{suffix}");
        let sound = library.sound(library.id_of(&name).expect("faction warp cue"));
        let frames = mc_sfx::synth(sound, 48000);
        assert!(!frames.is_empty(), "{name}");
        let mut energy = 0.0;
        for frame in &frames {
            for &s in frame {
                assert!(
                    s.is_finite() && s.abs() <= 0.81,
                    "{name}: invalid or clipping sample {s}"
                );
                energy += s * s;
            }
        }
        assert!(energy / frames.len() as f32 > 0.0001, "{name}: inaudible");
        let step = frames
            .windows(2)
            .flat_map(|w| (0..2).map(move |ch| (w[1][ch] - w[0][ch]).abs()))
            .fold(0.0_f32, f32::max);
        assert!(step < 0.08, "{name}: abrupt sample step {step}");
        if !sound.looped {
            let tail = &frames[frames.len() - 1300..frames.len() - 650];
            let level = tail.iter().flatten().fold(0.0_f32, |m, s| m.max(s.abs()));
            assert!(level < sound.peak * 0.06, "{name}: tail cut short {level}");
        }
        if sound.looped {
            for ch in 0..2 {
                let seam = (frames[0][ch] - frames.last().unwrap()[ch]).abs();
                assert!(seam < 0.02, "{name}: loop seam {seam}");
            }
        }
    }
}
