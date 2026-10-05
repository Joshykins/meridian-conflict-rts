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
    ai.income = 3000;
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
    state.lineup.sky.time = Some(mc_data::weather::TimeOfDay::Dusk);
    state.set_mode(Mode::Skirmish);
    assert_eq!(state.lineup.map, other, "skirmish finds its map again");
    assert!(!state.lineup.fog);
    assert!(state.request().survival.is_none());
    assert_ne!(state.sky(), state.skies[1]);

    let mut settings = Settings::default();
    assert!(state.store(&mut settings, false));
    assert_eq!(settings.skirmish_map, state.catalog.maps[other].stem);
    assert!(!state.store(&mut settings, false), "nothing changed since");
}

#[test]
fn a_new_set_up_starts_on_the_default_fog_and_sky_but_the_last_map() {
    let mut state = state();
    let other = (state.lineup.map + 1) % state.catalog.maps.len();
    state
        .lineup
        .set_map(&state.catalog, other, |_| false)
        .unwrap();
    state.lineup.fog = false;
    state.lineup.sky.time = Some(mc_data::weather::TimeOfDay::Dusk);
    let mut settings = Settings::default();
    state.store(&mut settings, false);

    let again = SetupState::with_catalog(state.catalog, &settings, Mode::Skirmish, 0);
    assert_eq!(again.lineup.map, other, "the map picked last is remembered");
    assert!(again.lineup.fog, "fog starts on");
    assert_eq!(
        again.sky(),
        SkyChoice::default(),
        "the sky starts on the map's own"
    );
}

#[test]
fn the_chart_is_drawn_off_the_frame_and_kept_for_coming_back() {
    let mut state = state();
    let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
    frame(&mut state, &mut overlay, &mut memory, &Input::default());
    assert!(
        !state.lineup.chart_shown(),
        "the first frame does not wait for the map's picture"
    );
    for _ in 0..1000 {
        if state.lineup.chart_shown() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        frame(&mut state, &mut overlay, &mut memory, &Input::default());
    }
    assert!(state.lineup.chart_shown(), "the picture arrives");
    state.chart_lost();
    frame(&mut state, &mut overlay, &mut memory, &Input::default());
    assert!(
        state.lineup.chart_shown(),
        "a picture lost to another screen goes back without being drawn again"
    );
}

/// Where seat `key`'s Zone cell was drawn last frame.
fn zone_cell(state: &SetupState, key: u8) -> Vec2 {
    state
        .lineup
        .zone_cells
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, at)| *at)
        .expect("the seat's row is on screen")
}

#[test]
fn a_zone_cell_picks_a_commander_up_and_the_chart_puts_them_down() {
    let mut state = state();
    let Some(big) = state.catalog.maps.iter().position(|m| m.starts >= 4) else {
        return;
    };
    state
        .lineup
        .set_map(&state.catalog, big, |_| false)
        .unwrap();
    state.seat_teams_for_shot(2);
    let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
    frame(&mut state, &mut overlay, &mut memory, &Input::default());
    let ai = state.lineup.roster.seats[1];
    let free_zone = |state: &SetupState| -> Option<u8> {
        (0..state.lineup.markers.len() as u8).find(|&n| {
            !state
                .lineup
                .roster
                .seats
                .iter()
                .any(|s| s.open() && s.start == n)
        })
    };
    // The AI's Zone cell, then where it goes on the chart: an AI moves, not you.
    let mine = state.lineup.roster.seats[0].start;
    let cell = zone_cell(&state, ai.key);
    click(&mut state, &mut overlay, &mut memory, cell);
    assert!(state.lineup.placing(), "the cell picks the commander up");
    let target = match free_zone(&state) {
        Some(n) => n,
        None => mine,
    };
    let at = state.lineup.markers[target as usize];
    click(&mut state, &mut overlay, &mut memory, at);
    assert!(!state.lineup.placing(), "and the chart puts them down");
    assert_eq!(state.lineup.roster.seats[1].start, target);
    if target == mine {
        assert_eq!(
            state.lineup.roster.seats[0].start, ai.start,
            "whoever held the zone takes theirs"
        );
    } else {
        assert_eq!(state.lineup.roster.seats[0].start, mine, "you stay put");
    }

    // A held marker picks its commander up; Escape puts them down and stays on the screen.
    let held = state.lineup.markers[state.lineup.roster.seats[1].start as usize];
    click(&mut state, &mut overlay, &mut memory, held);
    assert!(state.lineup.placing());
    let escape = Input {
        keys: vec![crate::ui::Key::Escape],
        ..Default::default()
    };
    assert!(
        frame(&mut state, &mut overlay, &mut memory, &escape).is_none(),
        "Escape cancels the move, not the screen"
    );
    assert!(!state.lineup.placing());
    assert!(matches!(
        frame(&mut state, &mut overlay, &mut memory, &escape),
        Some(SetupAction::Back)
    ));
}
