use super::*;
use crate::audio::Audio;
use crate::settings::{Antialiasing, Quality};
use crate::ui::{Input, Memory};
use mc_render::Overlay;

fn click(settings: &mut Settings, at: Vec2) -> OptionsOutcome {
    click_on(settings, 1, 1.0, at)
}

/// Clicks `at` on tab `tab`, with the interface scaled by `scale`.
fn click_on(settings: &mut Settings, tab: usize, scale: f32, at: Vec2) -> OptionsOutcome {
    let mut overlay = Overlay::default();
    let mut memory = Memory::default();
    memory.anims.insert(id("options-open-tab", 0), tab as f32);
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
            scale,
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
    // Quality row's right arrow, on the Display tab.
    let right = Vec2::new(766.0, 363.0);
    for quality in [Quality::Medium, Quality::High, Quality::Ultra] {
        let outcome = click(&mut settings, right);
        assert!(outcome.changed && outcome.display_changed && !outcome.back);
        assert_eq!(settings.quality, quality);
        assert_eq!(settings.quality_label(), quality.label());
    }
    // A manual render-scale change must also be applied and saved.
    let outcome = click(&mut settings, Vec2::new(634.0, 413.0));
    assert!(outcome.changed && outcome.display_changed);
    assert_eq!(settings.quality_label(), "Custom");
    assert_eq!(settings.quality, Quality::Ultra);
    // Restore Ultra even though the right arrow is already at the upper end.
    assert!(click(&mut settings, right).display_changed);
    assert_eq!(settings.quality_label(), "Ultra");
    assert_eq!(settings.antialiasing, Antialiasing::Smaa);
}

#[test]
fn quality_steps_down_from_low_to_auto_and_a_manual_scale_leaves_auto() {
    let mut settings = Settings::default();
    settings.apply_quality(Quality::Low);
    let left = Vec2::new(634.0, 363.0);
    assert!(click(&mut settings, left).display_changed);
    assert!(settings.auto_quality);
    assert_eq!(settings.quality_label(), "Auto");
    // Already at the start: stays Auto.
    click(&mut settings, left);
    assert!(settings.auto_quality);
    // A render scale chosen by hand is no longer Auto.
    assert!(click(&mut settings, Vec2::new(634.0, 413.0)).display_changed);
    assert!(!settings.auto_quality);
}

#[test]
fn back_is_clear_of_the_rows_at_every_interface_scale() {
    for step in 15..=30 {
        let scale = step as f32 / 20.0;
        for tab in 0..TABS.len() {
            let mut settings = Settings::default();
            // Back's centre, in window pixels.
            let at = Vec2::new(64.0 + 100.0, 1080.0 / scale - 64.0 - 26.0) * (scale);
            let out = click_on(&mut settings, tab, scale, at);
            assert!(out.back && !out.changed, "scale {scale}, tab {tab}");
        }
    }
}

#[test]
fn rows_scroll_in_whole_steps() {
    let mut rows = Rows {
        x: 0.0,
        w: 100.0,
        top: 10.0,
        first: 2,
        fits: 2,
        next: 0,
    };
    let shown: Vec<_> = (0..5).map(|_| rows.row().map(|r| r.y)).collect();
    assert_eq!(shown, [None, None, Some(10.0), Some(10.0 + ROW), None]);
    assert_eq!(rows.next, 5);
}
