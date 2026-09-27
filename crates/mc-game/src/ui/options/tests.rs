use super::*;
use crate::audio::Audio;
use crate::settings::{Antialiasing, Quality};
use crate::ui::{Input, Memory};
use mc_render::Overlay;

fn click(settings: &mut Settings, at: Vec2) -> OptionsOutcome {
    let mut overlay = Overlay::default();
    let mut memory = Memory::default();
    let audio = Audio::silent();
    let mut outcome = OptionsOutcome::default();
    for released in [false, true] {
        let input = Input {
            cursor: at,
            down: !released,
            pressed: !released,
            released,
            ..Default::default()
        };
        overlay.clear();
        memory.begin_frame();
        let mut ui = Ui::new(
            &mut overlay,
            &input,
            &mut memory,
            &audio,
            Vec2::new(1920.0, 1080.0),
            1.0,
            1.0,
            1.0 / 60.0,
        );
        outcome = draw(&mut ui, settings, 1.0);
        memory.end_frame(&input);
    }
    outcome
}

#[test]
fn quality_click_applies_and_requests_save_and_live_update() {
    let mut settings = Settings::default();
    settings.apply_quality(Quality::Low);
    // Quality row's right arrow, in the full Settings screen.
    let right = Vec2::new(784.0, 623.0);
    for quality in [Quality::Balanced, Quality::High, Quality::Ultra] {
        let outcome = click(&mut settings, right);
        assert!(outcome.changed && outcome.display_changed && !outcome.back);
        assert_eq!(settings.quality, quality);
        assert_eq!(settings.quality_label(), quality.label());
    }
    // A manual render-scale change must also be applied and saved.
    let outcome = click(&mut settings, Vec2::new(652.0, 673.0));
    assert!(outcome.changed && outcome.display_changed);
    assert_eq!(settings.quality_label(), "Custom");
    assert_eq!(settings.quality, Quality::Ultra);
    // Restore Ultra even though the right arrow is already at the upper end.
    assert!(click(&mut settings, right).display_changed);
    assert_eq!(settings.quality_label(), "Ultra");
    assert_eq!(settings.antialiasing, Antialiasing::Smaa);
}
