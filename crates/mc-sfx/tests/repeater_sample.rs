//! The chosen audition must survive library loading and game synthesis unchanged.
use mc_data::{sounds::Layer, SoundLibrary};
fn library() -> SoundLibrary {
    SoundLibrary::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
        .unwrap()
}
#[test]
fn approved_repeater_is_the_selected_waveform_not_a_reconstruction() {
    let library = library();
    let sound = library.sound(library.id_of("regency_repeater").unwrap());
    let frames = mc_sfx::synth(sound, 48000);
    let wav = include_bytes!("../../../data/sounds/samples/regency-repeater-f.wav");
    let expected: Vec<[f32; 2]> = wav[44..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| {
            [
                i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
                i16::from_le_bytes([b[2], b[3]]) as f32 / 32768.0,
            ]
        })
        .collect();
    assert_eq!(frames.len(), expected.len());
    let error = frames
        .iter()
        .zip(&expected)
        .flat_map(|(a, b)| (0..2).map(move |ch| (a[ch] - b[ch]).abs()))
        .fold(0.0_f32, f32::max);
    assert!(
        error < 0.000001,
        "selected waveform changed: max error {error}"
    );
}
#[test]
fn variants_resample_cleanly_and_keep_actual_shot_cadence() {
    let library = library();
    for name in [
        "regency_repeater",
        "regency_repeater_light",
        "regency_repeater_hit",
        "regency_repeater_light_hit",
        "regency_canopy_fire",
        "regency_brazier_fire",
        "regency_stinger",
    ] {
        let sound = library.sound(library.id_of(name).unwrap());
        let starts: Vec<_> = sound
            .layers
            .iter()
            .filter_map(|l| match l {
                Layer::Sample { at, .. } => Some(*at),
                _ => None,
            })
            .collect();
        assert_eq!(
            starts,
            if name == "regency_stinger" {
                vec![0.0, 0.1, 0.2]
            } else {
                vec![0.0]
            },
            "{name}"
        );
        for rate in [24000, 44100, 48000, 96000] {
            let frames = mc_sfx::synth(sound, rate);
            assert_eq!(frames.len(), (sound.length * rate as f32) as usize);
            assert!(
                frames
                    .iter()
                    .flatten()
                    .all(|s| s.is_finite() && s.abs() <= 0.71),
                "{name}"
            );
            let tail = &frames[frames.len() - rate as usize / 100..];
            assert!(
                tail.iter().flatten().all(|s| s.abs() < 0.01),
                "{name}: truncated tail"
            );
        }
    }
}
