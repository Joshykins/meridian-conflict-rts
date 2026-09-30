//! The set-up screen driven frame by frame over the test maps.

use super::*;
use crate::ui::faction::Pick;
use crate::ui::{Input, Memory};
use glam::Vec2;
use mc_render::Overlay;
use mc_sim::{Difficulty, Doctrine};

/// Set-up over the test maps (`test_maps`), not the checkout's `maps/`.
fn state() -> SetupState {
    in_mode(Mode::Skirmish)
}

fn in_mode(mode: Mode) -> SetupState {
    let catalog = Catalog::load_from(crate::ui::test_maps::paths(), true);
    let settings = Settings {
        player_name: "Tester".into(),
        ..Settings::default()
    };
    SetupState::with_catalog(catalog, &settings, mode, 0)
}

fn frame(
    state: &mut SetupState,
    overlay: &mut Overlay,
    memory: &mut Memory,
    input: &Input,
) -> Option<SetupAction> {
    let audio = crate::audio::Audio::silent();
    memory.begin_frame();
    let mut ui = Ui::new(
        overlay,
        input,
        memory,
        &audio,
        Vec2::new(1920.0, 1080.0),
        1.0,
        1.0,
        1.0 / 30.0,
    );
    let action = draw(&mut ui, state, 1.0);
    ui.popups();
    memory.end_frame(input);
    overlay.clear();
    action
}

fn click(state: &mut SetupState, overlay: &mut Overlay, memory: &mut Memory, at: Vec2) {
    for input in [
        Input {
            cursor: at,
            ..Default::default()
        },
        Input {
            cursor: at,
            down: true,
            pressed: true,
            ..Default::default()
        },
        Input {
            cursor: at,
            released: true,
            ..Default::default()
        },
    ] {
        assert!(
            frame(state, overlay, memory, &input).is_none(),
            "nothing starts while browsing"
        );
    }
}

#[test]
fn the_map_browser_picks_a_map_and_arc_is_the_default_race() {
    let mut state = state();
    assert!(
        state.catalog.maps.len() >= 2,
        "bake two skirmish maps to test the browser"
    );
    assert!(crate::ui::faction::races()
        .iter()
        .any(|r| r.abbreviation == "ARC"));
    state.catalog.browser.wait_for_thumbs();
    let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
    state.catalog.browser.open_now(state.lineup.map);
    frame(&mut state, &mut overlay, &mut memory, &Input::default());
    assert_eq!(
        state.catalog.browser.cards.len(),
        state.catalog.maps.len(),
        "every map shows with no filter"
    );
    let (other, at) = *state
        .catalog
        .browser
        .cards
        .iter()
        .find(|(i, _)| *i != state.lineup.map)
        .unwrap();
    click(&mut state, &mut overlay, &mut memory, at);
    assert!(
        state.catalog.browser.is_open(),
        "one click only picks the card"
    );
    let choose = state.catalog.browser.select_at;
    click(&mut state, &mut overlay, &mut memory, choose);
    assert!(!state.catalog.browser.is_open());
    assert_eq!(state.lineup.map, other);
    assert_eq!(
        state.lineup.roster.seats.len(),
        state.catalog.maps[other].starts.min(mc_core::MAX_PLAYERS),
        "the seats follow the new map"
    );
    assert_eq!(state.request().config.players[0].faction, "Aster");
}

#[test]
fn the_race_picker_sets_a_seat_to_random_and_launch_deals_a_race() {
    let mut state = state();
    assert!(
        !state.catalog.maps.is_empty(),
        "bake a map so skirmish set-up can be tested"
    );
    let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
    let seat = state.lineup.roster.seats[1];
    state
        .lineup
        .races
        .open_now(seat.key as usize, "ARC AI 1", seat.race);
    frame(&mut state, &mut overlay, &mut memory, &Input::default());
    let cards = state.lineup.races.cards.clone();
    assert_eq!(
        cards.len(),
        crate::ui::faction::races().len() + 1,
        "every race and Random"
    );
    let (pick, at) = *cards.last().unwrap();
    assert_eq!(pick, Pick::Random);
    click(&mut state, &mut overlay, &mut memory, at);
    assert!(
        state.lineup.races.is_open(),
        "one click only picks the card"
    );
    let choose = state.lineup.races.choose_at;
    click(&mut state, &mut overlay, &mut memory, choose);
    assert!(!state.lineup.races.is_open());
    assert_eq!(state.lineup.roster.seats[1].race, Pick::Random);
    let table = table_of(false, "Tester");
    assert!(state.lineup.ai_name(&table, 1).starts_with("Random AI"));
    let request = state.request();
    let dealt = &request.config.players[1];
    assert!(
        crate::ui::faction::race_by_key(&dealt.faction).is_some(),
        "a real race is dealt: {}",
        dealt.faction
    );
    assert_eq!(
        request.config.players[1].faction,
        state.request().config.players[1].faction,
        "the seed deals it the same way twice"
    );
}

#[test]
fn two_sides_splits_a_full_map_evenly_with_you_on_team_one() {
    let mut state = state();
    let Some(big) = state.catalog.maps.iter().position(|m| m.starts == 8) else {
        return;
    };
    state
        .lineup
        .set_map(&state.catalog, big, |_| false)
        .unwrap();
    state.seat_teams_for_shot(2);
    let seated = state.lineup.roster.seated_teams();
    assert_eq!(teams::matchup(&seated), "4 v 4");
    assert_eq!(state.lineup.roster.seats[0].team, 0);
    assert!(state.lineup.roster.allied());
    let request = state.request();
    assert_eq!(
        request
            .config
            .players
            .iter()
            .filter(|p| p.team == 0)
            .count(),
        4
    );
}

#[test]
fn observing_your_slot_starts_an_all_ai_match() {
    let mut state = state();
    assert!(
        !state.catalog.maps.is_empty(),
        "bake a map so skirmish set-up can be tested"
    );
    assert!(!state.observing());
    assert!(state.problem().is_none());
    let play = state.request();
    assert!(play
        .config
        .players
        .iter()
        .any(|p| p.controller == Controller::Human));

    let ai = &mut state.lineup.roster.seats[1].ai;
    ai.difficulty = Difficulty::Hard;
    ai.doctrine = Doctrine::Defensive;
    ai.domain_weights = [50, 150, 75];
    let preserved = *ai;
    let configured = state.request();
    assert_eq!(configured.config.players[1].ai, preserved);
    // Another map keeps the seats' AI set-up.
    let other = (state.lineup.map + 1) % state.catalog.maps.len();
    state
        .lineup
        .set_map(&state.catalog, other, |_| false)
        .unwrap();
    assert_eq!(state.lineup.roster.seats[1].ai, preserved);
    state.observe = true;
    assert!(state.observing());
    assert!(state.problem().is_none());
    let watch = state.request();
    assert!(watch
        .config
        .players
        .iter()
        .all(|p| p.controller == Controller::Ai));
    assert!(watch.config.players.len() >= 2);

    state.observe = false;
    assert!(!state.observing());
    assert!(state
        .request()
        .config
        .players
        .iter()
        .any(|p| p.controller == Controller::Human));
}

fn survival() -> SetupState {
    let state = in_mode(Mode::Survival);
    assert_eq!(
        state.mode(),
        Mode::Survival,
        "the test maps have a survival theatre"
    );
    state
}

#[test]
fn clicking_a_zone_marker_in_survival_deploys_there() {
    let mut state = survival();
    let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
    frame(&mut state, &mut overlay, &mut memory, &Input::default());
    assert!(
        state.lineup.markers.len() >= 2,
        "the theatre needs two zones"
    );
    let at = state.lineup.markers[1];
    for input in [
        Input {
            cursor: at,
            ..Default::default()
        },
        Input {
            cursor: at,
            down: true,
            pressed: true,
            ..Default::default()
        },
        Input {
            cursor: at,
            released: true,
            ..Default::default()
        },
    ] {
        frame(&mut state, &mut overlay, &mut memory, &input);
    }
    assert_eq!(state.lineup.roster.seats[0].start, 1);
    let request = state.request();
    let theatre = &state.catalog.theatres[state.lineup.map];
    assert_eq!(
        request.config.players[0].start,
        theatre.layout.spawns[1].start
    );
    assert!(request.survival.is_some());
    assert_eq!(request.colors[1], crate::survival::ENGINE_COLOR);
    assert_eq!(
        request.config.players[0].controller,
        Controller::Human,
        "you defend alone by default"
    );
}

#[test]
fn the_last_attacking_front_cannot_be_turned_off() {
    use crate::ui::survival::rules;
    use mc_data::survival::Domain;
    let mut state = survival();
    let theatre = &state.catalog.theatres[state.lineup.map];
    let counts = crate::survival::front_counts(&theatre.layout);
    let present: Vec<Domain> = Domain::ALL
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(d, _)| d)
        .collect();
    let rules = &mut state.lineup.rules;
    for d in &present[..present.len() - 1] {
        if rules.has(*d) {
            assert!(rules::toggle_front(rules, counts, *d));
        }
    }
    let last = *present.last().unwrap();
    if !rules.has(last) {
        assert!(rules::toggle_front(rules, counts, last));
    }
    assert_eq!(rules::attacking(rules, counts), vec![last]);
    assert!(
        !rules::toggle_front(rules, counts, last),
        "turning off the last front is refused"
    );
    assert!(state.problem().is_none());
}

#[test]
fn each_mode_keeps_its_own_map_fog_and_sky() {
    let mut state = state();
    let other = (state.lineup.map + 1) % state.catalog.maps.len();
    state
        .lineup
        .set_map(&state.catalog, other, |_| false)
        .unwrap();
    state.lineup.fog = false;
    state.set_mode(Mode::Survival);
    assert_eq!(state.mode(), Mode::Survival);
    assert!(state.lineup.fog, "survival keeps its own fog");
    assert!(state.request().survival.is_some());
    state.skies[1].time = Some(mc_data::weather::TimeOfDay::Dusk);
    state.set_mode(Mode::Skirmish);
    assert_eq!(state.lineup.map, other, "skirmish finds its map again");
    assert!(!state.lineup.fog);
    assert!(state.request().survival.is_none());
    assert_ne!(state.sky(), state.skies[1]);

    let mut settings = Settings::default();
    assert!(state.store(&mut settings, false));
    assert_eq!(settings.skirmish_map, state.catalog.maps[other].stem);
    assert!(!settings.skirmish_fog && settings.survival_fog);
    assert_eq!(settings.survival_sky, state.skies[1]);
    assert!(!state.store(&mut settings, false), "nothing changed since");
}

#[test]
fn a_left_lobby_goes_back_to_the_plan_it_had() {
    let mut state = survival();
    let zones = state.lineup.zones(&state.catalog);
    state
        .lineup
        .roster
        .set_control(1, Control::Ai, zones, |_| false)
        .unwrap();
    let plan = state.lineup.plan();
    let seats = plan.roster.seats.clone();
    let catalog = std::mem::replace(
        &mut state.catalog,
        Catalog::new(Vec::new(), Vec::new(), Vec::new()),
    );
    let back = SetupState::resume(
        &Settings::default(),
        0,
        crate::ui::multiplayer::Resume {
            catalog,
            lineup: plan,
        },
    );
    assert_eq!(back.mode(), Mode::Survival);
    assert_eq!(back.lineup.roster.seats, seats);
    assert_eq!(
        back.request()
            .config
            .players
            .iter()
            .filter(|p| p.controller == Controller::Ai && p.team == 0)
            .count(),
        1,
        "the AI defender is still beside you"
    );
}
