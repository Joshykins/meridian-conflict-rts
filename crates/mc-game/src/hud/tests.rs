use super::*;
use crate::ui::{Input, Memory};
use mc_render::Overlay;
use mc_sim::mirror::{QueuedOrder, UnitOrders, STATE_IDLE};
use mc_sim::tables::OrderKind;
use std::sync::Arc;

/// A match with one finished unit of `key` selected, and the HUD that draws it.
struct Rig {
    hud: Hud,
    view: View,
    blueprints: Arc<Blueprints>,
    map: MapFile,
    camera: Camera,
    overlay: Overlay,
    memory: Memory,
    hover: Option<u32>,
    /// Control held: the reclaim survey is up.
    reclaim: bool,
}

const VIEWPORT: Vec2 = Vec2::new(1920.0, 1080.0);

impl Rig {
    fn new(key: &str) -> Rig {
        let blueprints = Arc::new(
            Blueprints::load(&Blueprints::locate_data_dir().expect("data dir"))
                .expect("blueprints"),
        );
        let map = MapFile::open(crate::ui::test_maps::field()).expect("map opens");
        let camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), VIEWPORT);
        let mut view = View::new(0, crate::setup::TEAM_COLORS, false);
        let blueprint = blueprints.id_of(key).expect("blueprint exists");
        let pos = [4000.0, 4000.0, 0.0];
        view.frame.units.push(UnitInstance {
            prev_pos: pos,
            prev_heading: 0.0,
            pos,
            heading: 0.0,
            blueprint: blueprint.0 as u32,
            owner_flags: 0,
            health: 1.0,
            build: 1.0,
            turret_yaw: 0.0,
            radius: 4.0,
            unit_id: 7,
            packed: 0,
            gait: [0.0; 3],
            upgrade: 0.0,
            arm_pitch: [0.0; 4],
            prev_turret_yaw: 0.0,
            weld: [0.0; 3],
            recoil: 0.0,
            prev_recoil: 0.0,
            weld_first: 0,
            weld_count: 0,
            deploy: 0.0,
            prev_deploy: 0.0,
            _pad2: [0.0; 2],
            refit_modules: 0,
            status: [0; 3],
            mount: [0.0; 4],
            spin_recoil: [0.0; 4],
        });
        view.index_of.insert(7, 0);
        view.selection = vec![7];
        view.status.owns_clock = true;
        view.status.tick = 50;
        view.status.players = vec![Default::default(), Default::default()];
        Rig {
            hud: Hud::default(),
            view,
            blueprints,
            map,
            camera,
            overlay: Overlay::default(),
            memory: Memory::default(),
            hover: None,
            reclaim: false,
        }
    }

    fn frame(&mut self, input: &Input) -> Vec<HudAction> {
        let audio = crate::audio::Audio::silent();
        let stats = FrameStats::default();
        self.overlay.clear();
        self.memory.begin_frame();
        let mut ui = Ui::new(
            &mut self.overlay,
            input,
            &mut self.memory,
            &audio,
            VIEWPORT,
            1.0,
            1.0,
            0.016,
        );
        let scene = Scene {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            gpu: &stats,
            hover: self.hover,
            show_reclaim: self.reclaim,
            placing: None,
            net: None,
            net_notices: &[],
        };
        let actions = self.hud.draw(&mut ui, &scene, 0.016);
        self.memory.end_frame(input);
        assert!(!self.overlay.overflowed, "the HUD overflowed the overlay");
        actions
    }

    /// Frames enough for every panel to finish sliding in or out.
    fn settle(&mut self) {
        for _ in 0..60 {
            self.frame(&Input::default());
        }
    }

    /// Hover, press, release at `at`; returns what the release produced.
    fn click(&mut self, at: Vec2) -> Vec<HudAction> {
        self.settle();
        self.tap(at)
    }

    /// [`Rig::click`] without settling first, for a scan over a HUD that is
    /// already still: sixty frames a probe add up over a thousand probes.
    fn tap(&mut self, at: Vec2) -> Vec<HudAction> {
        self.frame(&Input {
            cursor: at,
            ..Default::default()
        });
        self.frame(&Input {
            cursor: at,
            down: true,
            pressed: true,
            ..Default::default()
        });
        self.frame(&Input {
            cursor: at,
            released: true,
            ..Default::default()
        })
    }

    fn right_click(&mut self, at: Vec2) -> Vec<HudAction> {
        self.settle();
        self.frame(&Input {
            cursor: at,
            ..Default::default()
        });
        self.frame(&Input {
            cursor: at,
            right_pressed: true,
            ..Default::default()
        })
    }
}

// Where things are at 1920x1080 and interface scale 1: see `Hud::draw`.
const DECK_Y: f32 = 1080.0 - EDGE - DECK_H;
const INFO_X: f32 = EDGE;
const ORDERS_X: f32 = INFO_X + 336.0 + GAP;
/// The construction panel's left edge, after an order card of `families` columns.
fn build_x(families: usize) -> f32 {
    ORDERS_X + selection::orders_width(families) + GAP
}
fn first_tile(families: usize) -> Vec2 {
    // Inside the first tile whether or not the strip has its end arrows.
    Vec2::new(
        build_x(families) + 14.0 + 30.0 + 40.0,
        DECK_Y + 44.0 + 32.0 + 40.0,
    )
}
/// The middle of order `row` in family column `col`.
fn order_slot(col: usize, row: usize) -> Vec2 {
    Vec2::new(
        ORDERS_X + 14.0 + col as f32 * (selection::ORDER_W + selection::ORDER_GAP) + 30.0,
        DECK_Y + 36.0 + row as f32 * (selection::ORDER_H + selection::ORDER_GAP) + 20.0,
    )
}
/// The top bar: its left edge, the speed control's parts, pause.
fn top_bar() -> f32 {
    1920.0 - EDGE - TOP_BAR_W
}
fn speed_slower() -> Vec2 {
    Vec2::new(top_bar() + 196.0 + 15.0, EDGE + 22.0)
}
fn speed_faster() -> Vec2 {
    Vec2::new(top_bar() + 196.0 + SPEED_W - 15.0, EDGE + 22.0)
}
fn speed_centre() -> Vec2 {
    Vec2::new(top_bar() + 196.0 + SPEED_W * 0.5, EDGE + 22.0)
}
/// Row `i` of the open speed list.
fn speed_pick(i: usize) -> Vec2 {
    Vec2::new(
        top_bar() + 196.0 + SPEED_W * 0.5,
        EDGE + 44.0 + 4.0 + 4.0 + i as f32 * 28.0 + 14.0,
    )
}
fn pause_button() -> Vec2 {
    Vec2::new(top_bar() + 196.0 + SPEED_W + 10.0 + 20.0, EDGE + 22.0)
}
/// Middle of the minimap's chart: under the top bar and the two-player roster.
fn chart() -> Vec2 {
    let top = EDGE + 44.0 + GAP + 2.0 * 22.0 + 12.0 + GAP;
    Vec2::new(1920.0 - EDGE - MINIMAP * 0.5, top + 11.0 + MINIMAP * 0.5)
}
/// What the first tile of tier `tech` offers: the tier's builds on shelves by purpose.
fn first_on_shelves(rig: &Rig, key: &str, tech: u8) -> BlueprintId {
    let builder = rig.blueprints.unit(rig.blueprints.id_of(key).unwrap());
    let structures = !builder.has(mc_data::cat::FACTORY);
    let mut items: Vec<&UnitBlueprint> = builder
        .builder
        .as_ref()
        .unwrap()
        .builds
        .iter()
        .map(|b| rig.blueprints.unit(*b))
        .filter(|b| b.tech == tech)
        .collect();
    items.sort_by_key(|b| style::Purpose::of(b, structures));
    items[0].id
}

/// Tank families: movement, combat, fire state, control.
const TANK_FAMILIES: usize = 4;
/// A factory only has STOP.
/// A land factory offers what its units can do: movement, combat, stance, engineering.
const FACTORY_FAMILIES: usize = 4;

#[test]
fn range_weather_is_picked_from_a_list_and_waits_for_apply() {
    use crate::range::{Range, RangeAction};
    use mc_data::weather::{TimeOfDay, WeatherPreset};
    let mut rig = Rig::new("aster_t1_tank");
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
    rig.view.range.as_mut().unwrap().sky = Some(Default::default());
    // The Sky tab; the middle of the Weather value opens its list, it does not step.
    assert!(rig
        .click(Vec2::new(253.0, 279.0 + focus::FOCUS_H))
        .is_empty());
    assert!(rig
        .click(Vec2::new(238.0, 320.0 + focus::FOCUS_H))
        .is_empty());
    let popup = rig.memory.popup.as_ref().expect("the weather list is open");
    let anchor = popup.anchor();
    let stormy = popup.row_centre(4, VIEWPORT);
    // While it is open the whole screen belongs to it.
    assert!(rig.hud.covers(Vec2::new(1800.0, 700.0)));
    // Picking a row changes the draft, not the range's weather.
    assert!(rig.click(stormy).is_empty());
    assert!(rig.memory.popup.is_none());
    rig.frame(&Input::default());
    let draft = rig.hud.range_sky.expect("a draft waits for Apply");
    assert_eq!(draft.choice.preset, Some(WeatherPreset::Stormy));
    // The arrows still step: the right one on the Time of Day row goes to Dawn.
    let time_arrow = Vec2::new(anchor.right() - 12.0, anchor.mid_y() + 32.0);
    assert!(rig.click(time_arrow).is_empty());
    assert_eq!(
        rig.hud.range_sky.unwrap().choice.time,
        Some(TimeOfDay::Dawn)
    );
    // Apply sits right of Storm Overhead, under the two rows.
    let apply = Vec2::new(anchor.right() - 40.0, anchor.mid_y() + 2.0 * 32.0);
    let asked = rig.click(apply);
    let Some(HudAction::Range(RangeAction::Sky(sky))) = asked.first() else {
        panic!("Apply asked for {asked:?}");
    };
    assert_eq!(sky.choice.preset, Some(WeatherPreset::Stormy));
    assert_eq!(sky.choice.time, Some(TimeOfDay::Dawn));
}

#[test]
fn range_browser_search_pick_cancel_and_background_capture() {
    use crate::range::{Range, RangeAction};
    use crate::ui::Key;
    let mut rig = Rig::new("aster_t1_tank");
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
    assert!(rig
        .click(Vec2::new(180.0, 152.0 + focus::FOCUS_H))
        .is_empty());
    assert!(rig.hud.unit_picker_open());
    assert!(rig.hud.covers(Vec2::new(1800.0, 700.0)));
    // A covered battlefield/reset click cannot produce a range action.
    assert!(rig
        .click(Vec2::new(184.0, 615.0 + focus::FOCUS_H))
        .is_empty());
    rig.frame(&Input {
        typed: "wArDeN".into(),
        ..Default::default()
    });
    let asked = rig.frame(&Input {
        keys: vec![Key::Enter],
        ..Default::default()
    });
    assert_eq!(
        asked,
        vec![HudAction::Range(RangeAction::PickSubject(tank))]
    );
    assert!(!rig.hud.unit_picker_open());
    assert!(rig.memory.editing.is_none());

    rig.click(Vec2::new(180.0, 152.0 + focus::FOCUS_H));
    rig.frame(&Input {
        typed: "no such unit".into(),
        ..Default::default()
    });
    assert!(rig
        .frame(&Input {
            keys: vec![Key::Enter],
            ..Default::default()
        })
        .is_empty());
    assert!(rig.hud.unit_picker_open());
    assert!(rig
        .frame(&Input {
            keys: vec![Key::Escape],
            ..Default::default()
        })
        .is_empty());
    assert!(!rig.hud.unit_picker_open());
    assert!(rig.memory.editing.is_none());
}

#[test]
fn range_browser_picks_t4_t5_and_space() {
    use crate::range::{Range, RangeAction};
    use crate::ui::Key;
    let mut rig = Rig::new("aster_t1_tank");
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
    // The first unit the browser lists under a filter: by tech, then name.
    let first = |keep: &dyn Fn(&mc_data::UnitBlueprint) -> bool| {
        let b = &rig.blueprints;
        let unit = b
            .units
            .iter()
            .filter(|u| b.is_listed(u.id) && keep(u))
            .min_by(|a, b| (a.tech, &a.name, &a.key).cmp(&(b.tech, &b.name, &b.key)))
            .unwrap();
        unit.key.clone()
    };
    let tech_4 = first(&|u| u.tech == 4);
    let tech_5 = first(&|u| u.tech == 5);
    let space = first(&|u| u.categories & mc_data::cat::SPACE != 0);
    for (filter, key) in [
        (Vec2::new(758.0, 283.0), tech_4.as_str()),
        (Vec2::new(844.0, 283.0), tech_5.as_str()),
        (Vec2::new(1128.0, 242.0), space.as_str()),
    ] {
        // The browser keeps its filters between openings; start each pick clean.
        rig.hud.unit_picker_filters = Default::default();
        rig.hud.unit_picker = Some(unit_picker::Picker::new());
        assert!(rig.click(filter).is_empty());
        let chosen = rig.blueprints.id_of(key).unwrap();
        let asked = rig.frame(&Input {
            keys: vec![Key::Enter],
            ..Default::default()
        });
        assert_eq!(
            asked,
            vec![HudAction::Range(RangeAction::PickSubject(chosen))],
            "{key}"
        );
        assert!(!rig.hud.unit_picker_open());
    }
    // Space and tech intersect; clearing an empty combination restores the catalog.
    rig.hud.unit_picker = Some(unit_picker::Picker::new());
    rig.click(Vec2::new(1128.0, 242.0));
    rig.click(Vec2::new(844.0, 283.0));
    assert!(rig
        .frame(&Input {
            keys: vec![Key::Enter],
            ..Default::default()
        })
        .is_empty());
    assert!(rig.hud.unit_picker_open());
    // Clear Filters, right of the search field.
    rig.click(Vec2::new(1248.0, 189.0));
    rig.frame(&Input {
        typed: "space".into(),
        ..Default::default()
    });
    assert_eq!(
        rig.frame(&Input {
            keys: vec![Key::Enter],
            ..Default::default()
        }),
        vec![HudAction::Range(RangeAction::PickSubject(
            rig.blueprints.id_of(&space).unwrap()
        ))]
    );
}

#[test]
fn range_browser_card_click_and_paging_reach_the_catalog() {
    use crate::range::{Range, RangeAction};
    let mut rig = Rig::new("aster_t1_tank");
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
    let mut sorted: Vec<_> = rig
        .blueprints
        .units
        .iter()
        .filter(|b| rig.blueprints.is_listed(b.id))
        .collect();
    sorted.sort_by(|a, b| (a.tech, &a.name, &a.key).cmp(&(b.tech, &b.name, &b.key)));
    let first = sorted[0].id;
    let last = sorted.last().unwrap().id;
    rig.click(Vec2::new(180.0, 152.0 + focus::FOCUS_H));
    assert_eq!(
        rig.click(Vec2::new(450.0, 355.0)),
        vec![HudAction::Range(RangeAction::PickSubject(first))]
    );
    rig.click(Vec2::new(180.0, 152.0 + focus::FOCUS_H));
    for _ in 0..rig.blueprints.units.len() {
        rig.frame(&Input {
            scroll: -1.0,
            ..Default::default()
        });
    }
    assert_eq!(
        rig.frame(&Input {
            keys: vec![crate::ui::Key::Enter],
            ..Default::default()
        }),
        vec![HudAction::Range(RangeAction::PickSubject(last))]
    );
}

#[test]
fn the_range_panel_reports_what_was_asked() {
    use crate::range::{Range, RangeAction, Scenario, RED};
    let mut rig = Rig::new("aster_t1_tank");
    assert!(
        !rig.hud.covers(Vec2::new(180.0, 400.0 + focus::FOCUS_H)),
        "a match that is not the range has no range panel"
    );
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
    // Where things are at 1920x1080 (see `range::draw`), less the economy's focus strip
    // over it: the panel's top is at 94, its tab strip at 266..292, and the open tab's
    // page starts at 306.
    let range = |a| vec![HudAction::Range(a)];
    assert_eq!(
        rig.click(Vec2::new(170.0, 206.0 + focus::FOCUS_H)),
        range(RangeAction::Side(crate::range::Side::Blue)),
        "the duplicate's team"
    );
    assert_eq!(
        rig.click(Vec2::new(320.0, 116.0 + focus::FOCUS_H)),
        range(RangeAction::Control(RED)),
        "the side commanded sits by the title"
    );
    assert_eq!(
        rig.click(Vec2::new(333.0, 152.0 + focus::FOCUS_H)),
        range(RangeAction::Subject(1))
    );
    assert_eq!(
        rig.click(Vec2::new(270.0, 238.0 + focus::FOCUS_H)),
        range(RangeAction::ArmSpawn)
    );

    // The Unit tab is open first.
    assert_eq!(
        rig.click(Vec2::new(308.0, 336.0 + focus::FOCUS_H)),
        range(RangeAction::Damage(1000)),
        "Kill"
    );
    assert_eq!(
        rig.click(Vec2::new(295.0, 368.0 + focus::FOCUS_H)),
        range(RangeAction::Flag(flag::INVULNERABLE, true))
    );
    // Holding the build track half way along asks for a half-built unit.
    let half = Vec2::new(80.0 + 216.0 * 0.5, 404.0 + focus::FOCUS_H);
    rig.frame(&Input {
        cursor: half,
        ..Default::default()
    });
    assert_eq!(
        rig.frame(&Input {
            cursor: half,
            down: true,
            pressed: true,
            ..Default::default()
        }),
        range(RangeAction::Build(500))
    );
    rig.frame(&Input::default());
    // Reset is always under the page.
    assert_eq!(
        rig.click(Vec2::new(184.0, 449.0 + focus::FOCUS_H)),
        range(RangeAction::Reset)
    );
    assert!(
        rig.hud.covers(Vec2::new(180.0, 400.0 + focus::FOCUS_H)),
        "a click on the panel must not reach the battlefield"
    );

    // With nothing selected it acts on every unit of the subject's type; with none of those, on nothing.
    rig.view.selection.clear();
    assert_eq!(
        rig.click(Vec2::new(308.0, 336.0 + focus::FOCUS_H)),
        range(RangeAction::Damage(1000))
    );
    let units = std::mem::take(&mut rig.view.frame.units);
    let index = std::mem::take(&mut rig.view.index_of);
    assert_eq!(rig.click(Vec2::new(308.0, 336.0 + focus::FOCUS_H)), vec![]);
    (rig.view.frame.units, rig.view.index_of) = (units, index);

    // Stage: scenarios around the subject, then what it does itself.
    assert!(
        rig.click(Vec2::new(127.0, 279.0 + focus::FOCUS_H))
            .is_empty(),
        "a tab is not an order"
    );
    assert_eq!(
        rig.click(Vec2::new(230.0, 322.0 + focus::FOCUS_H)),
        range(RangeAction::Scenario(Scenario::Targets))
    );
    assert_eq!(
        rig.click(Vec2::new(230.0, 358.0 + focus::FOCUS_H)),
        range(RangeAction::Scenario(Scenario::March)),
        "the second row: what the subject does itself"
    );
    // The panel is shorter on this tab, and Reset came up with it.
    assert_eq!(
        rig.click(Vec2::new(184.0, 405.0 + focus::FOCUS_H)),
        range(RangeAction::Reset)
    );

    // Range: the camera presets.
    assert!(rig
        .click(Vec2::new(316.0, 279.0 + focus::FOCUS_H))
        .is_empty());
    assert_eq!(
        rig.click(Vec2::new(300.0, 336.0 + focus::FOCUS_H)),
        range(RangeAction::Zoom(2))
    );
}

#[test]
fn the_range_economy_tab_fills_starves_and_scatters_wrecks() {
    use crate::range::{Range, RangeAction, BLUE, INCOME_NORMAL, RED};
    let mut rig = Rig::new("aster_t1_tank");
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
    for p in &mut rig.view.status.players {
        (p.mass_capacity, p.energy_capacity) = (1000.0, 5000.0);
    }
    let range = |a| vec![HudAction::Range(a)];
    assert!(
        rig.click(Vec2::new(190.0, 279.0 + focus::FOCUS_H))
            .is_empty(),
        "the Economy tab"
    );
    assert_eq!(
        rig.click(Vec2::new(160.0, 368.0 + focus::FOCUS_H)),
        range(RangeAction::Stock {
            player: BLUE,
            mass: Some(1000),
            energy: None
        }),
        "fill the materials"
    );
    assert_eq!(
        rig.click(Vec2::new(270.0, 368.0 + focus::FOCUS_H)),
        range(RangeAction::Income {
            player: BLUE,
            resource: 0,
            step: 1
        })
    );
    // Red's economy is its own.
    assert!(rig
        .click(Vec2::new(122.0, 318.0 + focus::FOCUS_H))
        .is_empty());
    assert_eq!(
        rig.click(Vec2::new(57.0, 428.0 + focus::FOCUS_H)),
        range(RangeAction::Stock {
            player: RED,
            mass: None,
            energy: Some(0)
        }),
        "empty red's energy"
    );
    // A power shortage in one click: free build off, a quarter of the income, the store dry.
    assert_eq!(
        rig.click(Vec2::new(150.0, 466.0 + focus::FOCUS_H)),
        vec![
            HudAction::Range(RangeAction::FreeBuild(false)),
            HudAction::Range(RangeAction::SetIncome {
                player: RED,
                resource: 1,
                index: 2
            }),
            HudAction::Range(RangeAction::Stock {
                player: RED,
                mass: None,
                energy: Some(0)
            }),
        ]
    );
    assert_eq!(
        rig.click(Vec2::new(71.0, 466.0 + focus::FOCUS_H)),
        range(RangeAction::FreeBuild(false))
    );
    let normal = rig.click(Vec2::new(308.0, 466.0 + focus::FOCUS_H));
    assert!(normal.contains(&HudAction::Range(RangeAction::SetIncome {
        player: RED,
        resource: 1,
        index: INCOME_NORMAL
    })));
    assert_eq!(
        rig.click(Vec2::new(100.0, 500.0 + focus::FOCUS_H)),
        range(RangeAction::Wrecks)
    );
}

#[test]
fn formation_controls_emit_actions_and_capture_their_clicks() {
    let mut rig = Rig::new("aster_t1_tank");
    // Formation is for more than one unit.
    let mut second = rig.view.frame.units[0];
    second.unit_id = 9;
    rig.view.frame.units.push(second);
    rig.view.index_of.insert(9, 1);
    rig.view.selection.push(9);
    assert_eq!(rig.click(order_slot(0, 2)), vec![HudAction::FormationPanel]);
    rig.view.formation_panel = true;
    let w = selection::orders_width(TANK_FAMILIES);
    let (px, py) = (ORDERS_X, DECK_Y - 154.0);
    let cw = w - 24.0;
    assert_eq!(
        rig.click(Vec2::new(px + 12.0 + cw * 0.75, py + 42.0)),
        vec![HudAction::FormationTogether(false)]
    );
    assert_eq!(
        rig.click(Vec2::new(px + 12.0 + cw * 0.25, py + 42.0)),
        vec![HudAction::FormationTogether(true)]
    );
    assert_eq!(
        rig.click(Vec2::new(px + 12.0 + cw * 0.85, py + 74.0)),
        vec![HudAction::FormationSpacing(2)]
    );
    let up = Vec2::new(px + w * 0.5, py + 107.0);
    assert_eq!(rig.click(up), vec![HudAction::FormUp]);
    assert!(
        rig.hud.covers(up),
        "formation control click leaked onto the battlefield"
    );
}

#[test]
fn a_construction_tile_builds_and_the_hud_keeps_the_click() {
    let mut rig = Rig::new("aster_t1_land_factory");
    let first = first_on_shelves(&rig, "aster_t1_land_factory", 1);
    let tile = first_tile(FACTORY_FAMILIES);
    assert_eq!(rig.click(tile), vec![HudAction::Build(first)]);
    assert!(
        rig.hud.covers(tile),
        "a click on a tile must not reach the battlefield"
    );
    assert!(
        !rig.hud.covers(Vec2::new(960.0, 400.0)),
        "the middle of the screen is the battlefield's"
    );
    assert_eq!(rig.right_click(tile), vec![HudAction::Cancel(first)]);
}

#[test]
fn tech_tabs_switch_what_is_offered() {
    let mut rig = Rig::new("aster_t2_engineer");
    let builds = rig
        .blueprints
        .unit(rig.blueprints.id_of("aster_t2_engineer").unwrap())
        .builder
        .as_ref()
        .unwrap()
        .builds
        .clone();
    let _ = builds;
    let tier = |rig: &Rig, t: u8| first_on_shelves(rig, "aster_t2_engineer", t);
    // Movement, and work (assist, reclaim, stop).
    let families = 2;
    let tile = first_tile(families);
    // A tech 2 engineer opens on its own tier; the T1 tab brings the basics back.
    assert_eq!(rig.click(tile), vec![HudAction::Build(tier(&rig, 2))]);
    assert_eq!(
        rig.click(Vec2::new(build_x(families) + 14.0 + 29.0, DECK_Y + 23.0)),
        vec![]
    );
    assert_eq!(rig.click(tile), vec![HudAction::Build(tier(&rig, 1))]);
}

#[test]
fn the_queue_strip_lists_production_and_cancels_from_it() {
    let mut rig = Rig::new("aster_t1_land_factory");
    let builds = rig
        .blueprints
        .unit(rig.blueprints.id_of("aster_t1_land_factory").unwrap())
        .builder
        .as_ref()
        .unwrap()
        .builds
        .clone();
    let order = |b| QueuedOrder {
        formation: 0,
        offset: [0.0; 2],
        moving_slot: None,
        formation_phase: 0,
        kind: OrderKind::Produce,
        pos: [0.0, 0.0],
        at: mc_core::FxVec2::ZERO,
        blueprint: b,
        radius: 0.0,
    };
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        orders: vec![order(builds[0]), order(builds[0]), order(builds[1])],
        progress: 0.4,
        ..Default::default()
    }];
    rig.frame(&Input::default());
    let strip = Vec2::new(build_x(FACTORY_FAMILIES) + 200.0, DECK_Y - GAP - 30.0);
    assert!(
        rig.hud.covers(strip),
        "the queue strip appears above the construction panel"
    );
    // Two stacks: 2 of the first product, then 1 of the second. The second stack starts one tile along.
    let second = Vec2::new(build_x(FACTORY_FAMILIES) + 14.0, DECK_Y - GAP - 31.0);
    let hits: Vec<HudAction> = (0..40)
        .flat_map(|i| rig.right_click(second + Vec2::X * (150.0 + i as f32 * 6.0)))
        .collect();
    assert!(
        hits.contains(&HudAction::Cancel(builds[0]))
            && hits.contains(&HudAction::Cancel(builds[1])),
        "{hits:?}"
    );
}

#[test]
fn the_free_camera_folds_the_panels_away_and_gives_them_back() {
    let mut rig = Rig::new("aster_t1_tank");
    rig.settle();
    assert!(
        rig.hud.covers(speed_faster()),
        "the top bar keeps the pointer"
    );
    rig.hud.free.set(true);
    rig.settle();
    // Folded: nothing it held takes a click, and the battlefield gets the pointer.
    assert_eq!(rig.click(speed_faster()), vec![]);
    assert!(!rig.hud.covers(speed_faster()));
    assert!(!rig.hud.covers(chart()));
    // The unit panel, bottom left.
    assert!(!rig.hud.covers(Vec2::new(100.0, VIEWPORT.y - 100.0)));
    // Only the guide at the foot of the screen is left, and it keeps its own clicks.
    assert!(rig
        .hud
        .covers(Vec2::new(VIEWPORT.x * 0.5, VIEWPORT.y - 40.0)));
    rig.hud.free.set(false);
    rig.settle();
    assert_eq!(rig.click(speed_faster()), vec![HudAction::SetSpeed(150)]);
}

#[test]
fn the_top_bar_and_the_minimap_report_what_was_asked() {
    let mut rig = Rig::new("aster_t1_tank");
    // Arrows step; the middle opens every speed, and a pick closes it.
    assert_eq!(rig.click(speed_faster()), vec![HudAction::SetSpeed(150)]);
    assert_eq!(rig.click(speed_slower()), vec![HudAction::SetSpeed(50)]);
    assert_eq!(rig.click(speed_centre()), vec![]);
    assert!(rig.hud.speed_open);
    assert_eq!(rig.click(speed_pick(8)), vec![HudAction::SetSpeed(1200)]);
    assert!(!rig.hud.speed_open);
    rig.click(speed_centre());
    assert_eq!(
        rig.click(speed_pick(3)),
        vec![],
        "the speed in force is no change"
    );
    rig.click(speed_centre());
    rig.click(Vec2::new(900.0, 500.0));
    assert!(!rig.hud.speed_open, "a click elsewhere closes the list");
    assert_eq!(rig.click(pause_button()), vec![HudAction::Pause]);
    assert_eq!(
        rig.click(pause_button() + Vec2::new(74.0, 0.0)),
        vec![HudAction::Menu]
    );
    // Holding the button on the chart looks there; the right button orders there.
    let chart = chart();
    rig.frame(&Input {
        cursor: chart,
        ..Default::default()
    });
    let held = rig.frame(&Input {
        cursor: chart,
        down: true,
        pressed: true,
        ..Default::default()
    });
    assert!(matches!(held[..], [HudAction::LookAt(_)]), "{held:?}");
    rig.frame(&Input {
        cursor: chart,
        released: true,
        ..Default::default()
    });
    assert!(matches!(
        rig.right_click(chart)[..],
        [HudAction::OrderAt(_)]
    ));
    // The map folds away, and comes back from its tab.
    let fold = Vec2::new(1920.0 - EDGE - 16.0, chart.y - MINIMAP * 0.5 - 11.0 + 12.0);
    rig.click(fold);
    assert!(rig.hud.minimap_hidden);
    rig.settle();
    assert!(
        !rig.hud.covers(chart),
        "a folded map leaves the battlefield clear"
    );
    rig.click(Vec2::new(1920.0 - EDGE - 48.0, fold.y));
    assert!(!rig.hud.minimap_hidden);
    // A network match owns no clock: the speed and pause controls are dead.
    rig.view.status.owns_clock = false;
    assert_eq!(rig.click(pause_button()), vec![]);
    assert_eq!(rig.click(speed_faster()), vec![]);
}

#[test]
fn the_pause_strip_leaves_the_battlefield_clear() {
    let mut rig = Rig::new("aster_t1_tank");
    rig.view.paused = true;
    rig.settle();
    for p in [Vec2::new(960.0, 540.0), Vec2::new(960.0, 300.0)] {
        assert!(!rig.hud.covers(p), "the pause card covers {p}");
    }
    // Its Resume button sits at the top, between the economy and the clock.
    let resume = (700..1400)
        .step_by(8)
        .map(|x| Vec2::new(x as f32, EDGE + 22.0))
        .find(|&p| rig.click(p) == [HudAction::Pause]);
    assert!(resume.is_some(), "no Resume on the pause strip");
}

#[test]
fn the_order_card_offers_what_the_selection_can_do_by_family() {
    let mut rig = Rig::new("aster_t1_tank");
    assert_eq!(
        rig.click(order_slot(0, 0)),
        vec![HudAction::Target(Targeting::Move)]
    );
    assert_eq!(
        rig.click(order_slot(0, 1)),
        vec![HudAction::Target(Targeting::Patrol)]
    );
    assert_eq!(
        rig.click(order_slot(1, 0)),
        vec![HudAction::Target(Targeting::Attack)]
    );
    assert_eq!(
        rig.click(order_slot(1, 1)),
        vec![HudAction::Target(Targeting::AttackMove)]
    );
    assert_eq!(
        rig.click(order_slot(1, 2)),
        vec![HudAction::Target(Targeting::AttackGround)]
    );
    assert_eq!(
        rig.click(order_slot(1, 3)),
        vec![HudAction::Target(Targeting::Bombard)]
    );
    assert_eq!(
        rig.click(order_slot(2, 0)),
        vec![HudAction::FireState(mc_sim::FireState::FireAtWill)]
    );
    assert_eq!(
        rig.click(order_slot(2, 1)),
        vec![HudAction::FireState(mc_sim::FireState::HoldPosition)]
    );
    assert_eq!(
        rig.click(order_slot(2, 2)),
        vec![HudAction::FireState(mc_sim::FireState::HoldFire)]
    );
    assert_eq!(rig.click(order_slot(3, 0)), vec![HudAction::Stop]);
    // One tank has no formation, and a tank cannot reclaim: nothing more on the card.
    assert_eq!(rig.click(order_slot(0, 2)), vec![]);
    assert_eq!(rig.click(order_slot(4, 0)), vec![]);
}

#[test]
fn a_factory_card_orders_its_units_and_its_queue_holds_repeat() {
    let mut rig = Rig::new("aster_t1_land_factory");
    // What it makes takes these: moves, patrols, attacks, an engineer's assist.
    assert_eq!(
        rig.click(order_slot(0, 0)),
        vec![HudAction::Target(Targeting::Move)]
    );
    assert_eq!(
        rig.click(order_slot(0, 1)),
        vec![HudAction::Target(Targeting::Patrol)]
    );
    assert_eq!(
        rig.click(order_slot(3, 0)),
        vec![HudAction::Target(Targeting::Assist)]
    );
    assert_eq!(
        rig.click(order_slot(3, 1)),
        vec![HudAction::PauseWork(true)]
    );
    assert_eq!(rig.click(order_slot(3, 2)), vec![HudAction::Stop]);
    let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
    assert_eq!(rig.click(repeat), vec![HudAction::Repeat(true)]);
    // Pause sits beside it on the strip.
    let pause = Vec2::new(repeat.x - 48.0 - 10.0 - 48.0, repeat.y);
    assert_eq!(rig.click(pause), vec![HudAction::PauseWork(true)]);
}

#[test]
fn the_economy_priorities_set_each_kind_last_even_or_first() {
    use mc_sim::focus::{Focus, Priority};
    let mut rig = Rig::new("aster_t1_tank");
    // One row under the figures: Mines under materials, Power under energy, each
    // with Last / Even / First at its right end.
    let y = EDGE + ECONOMY_H - focus::FOCUS_H + 11.0;
    let end = |i: f32| EDGE + 16.0 + i * 306.0 + 280.0;
    let (mines_last, power_first) = (
        Vec2::new(end(0.0) - 110.0, y),
        Vec2::new(end(1.0) - 22.0, y),
    );
    let power = Focus {
        mines: Priority::Even,
        power: Priority::First,
    };
    assert_eq!(rig.click(power_first), vec![HudAction::Focus(power)]);
    // Once the sim has it, the other kind's click keeps it.
    rig.view.status.players[0].focus = power;
    rig.frame(&Input::default());
    let both = Focus {
        mines: Priority::Last,
        ..power
    };
    assert_eq!(rig.click(mines_last), vec![HudAction::Focus(both)]);
    rig.view.status.players[0].focus = both;
    rig.frame(&Input::default());
    // The one on goes back to Even.
    assert_eq!(rig.click(mines_last), vec![HudAction::Focus(power)]);
    assert!(rig.hud.covers(power_first));
}

#[test]
fn paused_work_offers_resume_on_the_card_and_the_strip() {
    let mut rig = Rig::new("aster_t1_land_factory");
    rig.view.frame.units[0].status[0] |= mc_sim::mirror::UNIT_PAUSED;
    assert_eq!(
        rig.click(order_slot(3, 1)),
        vec![HudAction::PauseWork(false)]
    );
    let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
    let resume = Vec2::new(repeat.x - 48.0 - 10.0 - 48.0, repeat.y);
    assert_eq!(rig.click(resume), vec![HudAction::PauseWork(false)]);
    // A tank has no work to pause: its card has no such order.
    let mut rig = Rig::new("aster_t1_tank");
    rig.view.frame.units[0].status[0] |= mc_sim::mirror::UNIT_PAUSED;
    assert_eq!(rig.click(order_slot(3, 0)), vec![HudAction::Stop]);
}

#[test]
fn an_engineers_queue_takes_a_right_click() {
    let mut rig = Rig::new("aster_t1_engineer");
    let build = rig.blueprints.id_of("aster_t1_power").unwrap_or_else(|| {
        rig.blueprints
            .unit(rig.blueprints.id_of("aster_t1_engineer").unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds[0]
    });
    let at = mc_core::FxVec2::new(mc_core::Fx::from_int(100), mc_core::Fx::from_int(200));
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        orders: vec![QueuedOrder {
            formation: 0,
            offset: [0.0; 2],
            moving_slot: None,
            formation_phase: 0,
            kind: OrderKind::Build,
            pos: [100.0, 200.0],
            at,
            blueprint: build,
            radius: 0.0,
        }],
        progress: 0.0,
        ..Default::default()
    }];
    let families = 2;
    rig.frame(&Input::default());
    let hits: Vec<HudAction> = (0..40)
        .flat_map(|i| {
            rig.right_click(Vec2::new(
                build_x(families) + 150.0 + i as f32 * 6.0,
                DECK_Y - GAP - 31.0,
            ))
        })
        .collect();
    assert!(
        hits.contains(&HudAction::CancelOrder {
            kind: OrderKind::Build,
            pos: at
        }),
        "{hits:?}"
    );
}

#[test]
fn the_commander_card_is_always_there_and_selects_it() {
    let mut rig = Rig::new("aster_commander");
    rig.view.selection.clear();
    let card = Vec2::new(
        EDGE + COMMANDER_W * 0.5,
        EDGE + ECONOMY_H + GAP + COMMANDER_H * 0.5,
    );
    assert_eq!(
        rig.click(card),
        vec![HudAction::Select {
            units: vec![7],
            focus: true
        }]
    );
}

#[test]
fn a_late_match_reclaim_survey_leaves_the_panels_room() {
    // Control over a survival match's seventeen hundred wrecks: the survey drew
    // before the top bar and the right column and used up the overlay, so they
    // flickered in and out as its size moved about the limit.
    let mut rig = Rig::new("aster_t3_reclaimer");
    let tank = rig.blueprints.id_of("aster_t1_tank").expect("tank");
    let template = rig.view.frame.units[0];
    for k in 0..1720u32 {
        // Heaps round a few dozen fights, scattered through each.
        let fight = Vec2::new((k % 43) as f32 * 97.0, ((k % 43) * 7 % 43) as f32 * 61.0);
        let scatter = Vec2::new(
            (k.wrapping_mul(2654435761) % 997) as f32 / 997.0,
            (k.wrapping_mul(40503) % 991) as f32 / 991.0,
        );
        let at = Vec2::splat(2400.0) + fight + scatter * 160.0;
        let pos = [at.x, at.y, 0.0];
        let id = 100 + k;
        rig.view.index_of.insert(id, rig.view.frame.units.len());
        rig.view.frame.units.push(UnitInstance {
            pos,
            prev_pos: pos,
            unit_id: id,
            blueprint: tank.0 as u32,
            owner_flags: mc_sim::mirror::KIND_WRECK,
            ..template
        });
    }
    rig.reclaim = true;
    rig.camera.focus = glam::Vec3::new(4500.0, 3800.0, 0.0);
    for distance in [250.0, 600.0, 1500.0, 3000.0, 6000.0] {
        rig.camera.distance = distance;
        rig.settle();
        let used = rig.overlay.vertices.len();
        assert!(
            used < mc_render::overlay::MAX_OVERLAY_VERTICES / 4,
            "{used} overlay vertices at {distance} m"
        );
    }
}

#[test]
fn a_late_match_mine_survey_leaves_the_panels_room() {
    // Zoomed out over dozens of mines with one selected: the survey drew
    // first and filled the overlay, and every panel after it vanished.
    let mut rig = Rig::new("aster_core_mine");
    let size = Vec2::from(rig.map.info().size_metres().to_f32());
    let template = rig.view.frame.units[0];
    for k in 0..48u32 {
        let at = size
            * Vec2::new(
                0.1 + 0.8 * (k % 8) as f32 / 7.0,
                0.1 + 0.8 * (k / 8) as f32 / 5.0,
            );
        let pos = [at.x, at.y, 0.0];
        let id = 100 + k;
        rig.view.index_of.insert(id, rig.view.frame.units.len());
        rig.view.frame.units.push(UnitInstance {
            pos,
            prev_pos: pos,
            unit_id: id,
            ..template
        });
    }
    rig.settle();
    assert!(rig.overlay.vertices.len() < mc_render::overlay::MAX_OVERLAY_VERTICES);
    let commander = rig.blueprints.id_of("aster_commander").expect("commander");
    let pos = [4100.0, 4000.0, 0.0];
    rig.view.index_of.insert(8, rig.view.frame.units.len());
    rig.view.frame.units.push(UnitInstance {
        pos,
        prev_pos: pos,
        unit_id: 8,
        blueprint: commander.0 as u32,
        ..template
    });
    let card = Vec2::new(
        EDGE + COMMANDER_W * 0.5,
        EDGE + ECONOMY_H + GAP + COMMANDER_H * 0.5,
    );
    assert_eq!(
        rig.click(card),
        vec![HudAction::Select {
            units: vec![8],
            focus: true
        }]
    );
}

#[test]
fn a_builders_queued_mines_bring_up_their_territories_once_each() {
    let mut rig = Rig::new("aster_t1_engineer");
    rig.camera.focus = glam::Vec3::new(4000.0, 4000.0, 0.0);
    rig.camera.distance = 3000.0;
    let mine = rig.blueprints.id_of("aster_core_mine").expect("core mine");
    let order = |x: i32| QueuedOrder {
        formation: 0,
        offset: [0.0; 2],
        moving_slot: None,
        formation_phase: 0,
        kind: OrderKind::Build,
        pos: [x as f32, 4000.0],
        at: mc_core::FxVec2::from_ints(x, 4000),
        blueprint: mine,
        radius: 0.0,
    };
    let drawn = |rig: &mut Rig, queues: Vec<UnitOrders>| {
        rig.view.status.queues = queues;
        rig.settle();
        rig.overlay.vertices.len()
    };
    let none = drawn(&mut rig, Vec::new());
    let queued = drawn(
        &mut rig,
        vec![UnitOrders {
            unit_id: 7,
            orders: vec![order(4000), order(4300)],
            ..Default::default()
        }],
    );
    assert!(queued > none, "{queued} vertices, {none} with none queued");
    // A second builder carrying the same order draws nothing more.
    let shared = drawn(
        &mut rig,
        vec![
            UnitOrders {
                unit_id: 7,
                orders: vec![order(4000), order(4300)],
                ..Default::default()
            },
            UnitOrders {
                unit_id: 9,
                orders: vec![order(4300)],
                ..Default::default()
            },
        ],
    );
    assert_eq!(shared, queued);
}

#[test]
fn an_idle_engineer_tile_steps_through_them_and_shift_takes_them_all() {
    let mut rig = Rig::new("aster_t1_engineer");
    rig.view.selection.clear();
    rig.view.frame.units[0].owner_flags |= STATE_IDLE;
    let mut second = rig.view.frame.units[0];
    second.unit_id = 9;
    rig.view.frame.units.push(second);
    rig.view.index_of.insert(9, 1);
    // No commander: the card sits right under the economy, its first tile right
    // of the title block.
    let card_y = EDGE + ECONOMY_H + GAP;
    let tile = Vec2::new(EDGE + 8.0 + 66.0 + 8.0 + 18.0, card_y + 8.0 + 18.0);
    let one = |id| {
        vec![HudAction::Select {
            units: vec![id],
            focus: true,
        }]
    };
    assert_eq!(rig.click(tile), one(7));
    assert_eq!(rig.click(tile), one(9));
    assert_eq!(rig.click(tile), one(7), "and round again");
    rig.view.shift = true;
    assert_eq!(
        rig.click(tile),
        vec![HudAction::Select {
            units: vec![7, 9],
            focus: false
        }]
    );
    rig.view.selection = vec![9, 7];
    assert_eq!(
        rig.click(tile),
        vec![HudAction::Select {
            units: vec![7, 9],
            focus: true
        }],
        "a second shift-click finds them"
    );
    rig.view.shift = false;
}

#[test]
fn a_structure_that_upgrades_offers_it_on_its_next_tier() {
    let mut rig = Rig::new("aster_t1_radar");
    rig.frame(&Input::default());
    let radar = rig
        .blueprints
        .unit(rig.blueprints.id_of("aster_t1_radar").unwrap());
    assert!(
        radar.upgrades_to.is_some(),
        "the test needs an upgradable structure"
    );
    // No builds, so the panel opens on T2, whose first tile is the upgrade.
    // Its order card is Stop alone: one family.
    assert_eq!(rig.click(first_tile(1)), vec![HudAction::Upgrade]);
}

#[test]
fn a_factory_queues_its_upgrade_and_the_units_it_opens_on_the_tier_tab() {
    let mut rig = Rig::new("aster_t1_land_factory");
    rig.frame(&Input::default());
    let (t2, tank) = (
        rig.blueprints.id_of("aster_t2_land_factory").unwrap(),
        rig.blueprints.id_of("aster_t2_tank").unwrap(),
    );
    // The factory's card has four order families; its T2 tab is the second.
    let families = 4;
    let t2_tab = Vec2::new(build_x(families) + 14.0 + 62.0 + 29.0, DECK_Y + 23.0);
    let hits = rig.click(t2_tab);
    assert!(hits.is_empty(), "{hits:?}");
    let upgrade = first_tile(families);
    // The upgrade tile, then the gap before the first shelf.
    let unit = upgrade + Vec2::new(96.0 + 22.0, 0.0);
    assert_eq!(rig.click(upgrade), vec![HudAction::Upgrade]);
    // A T2 unit is locked until the upgrade is queued...
    assert_eq!(rig.click(unit), vec![]);
    let queued = |kind, blueprint| QueuedOrder {
        formation: 0,
        offset: [0.0; 2],
        moving_slot: None,
        formation_phase: 0,
        kind,
        pos: [0.0, 0.0],
        at: mc_core::FxVec2::ZERO,
        blueprint,
        radius: 0.0,
    };
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        orders: vec![queued(OrderKind::Upgrade, t2)],
        progress: 0.0,
        ..Default::default()
    }];
    // ...then takes orders, and the upgrade's right-click takes it back out.
    let first_t2 = rig.click(unit);
    assert!(
        matches!(first_t2.as_slice(), [HudAction::Build(b)] if rig.blueprints.unit(*b).tech == 2),
        "{first_t2:?} (a T2 tank is {tank:?})"
    );
    assert_eq!(rig.click(upgrade), vec![], "already queued");
    assert_eq!(rig.right_click(upgrade), vec![HudAction::CancelRefit(t2)]);
}

/// A commander's refit tab: row `row` (slot), tile `n` along it counting `or`
/// gaps after `ors` alternatives and `arrows` tier arrows.
fn refit_tile(row: usize, n: usize, ors: usize, arrows: usize) -> Vec2 {
    let row_h = ((DECK_H - 52.0 - 12.0) / 4.0).clamp(26.0, 44.0);
    Vec2::new(
        build_x(4)
            + 14.0
            + 104.0
            + n as f32 * 172.0
            + ors as f32 * 34.0
            + arrows as f32 * 26.0
            + 86.0,
        DECK_Y + 44.0 + row as f32 * (row_h + 4.0) + row_h * 0.5,
    )
}

#[test]
fn refits_queue_their_earlier_tiers_and_ask_before_replacing() {
    let mut rig = Rig::new("aster_commander+mfe");
    let bps = rig.blueprints.clone();
    let set = bps
        .refit_set(bps.id_of("aster_commander").unwrap())
        .unwrap();
    let kit = |slot: &str, key: &str| {
        set.slots
            .iter()
            .find(|s| s.key == slot)
            .unwrap()
            .modules
            .iter()
            .find(|m| m.key == key)
            .unwrap()
            .kit
    };
    rig.hud.open_refit_tab();
    // The rail cannon goes over the cannon: one click queues both, in order.
    assert_eq!(
        rig.click(refit_tile(1, 1, 0, 1)),
        vec![HudAction::Refit(vec![
            kit("gun", "cannon"),
            kit("gun", "railgun")
        ])]
    );
    // The shield would take the formation engine off: nothing is sent until the player says so.
    let shield = refit_tile(2, 1, 1, 0);
    assert_eq!(rig.click(shield), vec![]);
    assert!(rig.hud.refit_prompt.is_some(), "a replacement asks first");
    // The card stands over the panel: its buttons along its foot.
    let yes = Vec2::new(
        (shield.x - 210.0).clamp(14.0, 1920.0 - 420.0 - 14.0) + 93.0,
        DECK_Y - GAP - 27.0,
    );
    assert_eq!(
        rig.click(yes),
        vec![HudAction::Refit(vec![kit("back", "shield")])]
    );
    assert!(rig.hud.refit_prompt.is_none());
    // Asked again and kept: nothing happens.
    rig.click(shield);
    let keep = Vec2::new(yes.x + 160.0, yes.y);
    assert_eq!(rig.click(keep), vec![]);
    assert!(rig.hud.refit_prompt.is_none());
}

fn observer_card_y(i: usize) -> f32 {
    EDGE + observer::HEADER_H + GAP + i as f32 * (observer::CARD_FULL + 6.0)
}

#[test]
fn an_observer_cannot_order_the_selection() {
    let mut rig = Rig::new("aster_t1_tank");
    rig.view.observing = true;
    assert_eq!(
        rig.click(order_slot(0, 0)),
        vec![],
        "watching has no order card"
    );
    assert_eq!(
        rig.click(first_tile(TANK_FAMILIES)),
        vec![],
        "watching has no construction panel"
    );
    // A commander's card looks through their eyes; its Find button goes to them.
    let first_card = observer_card_y(0);
    assert_eq!(
        rig.click(Vec2::new(EDGE + 120.0, first_card + 60.0)),
        vec![HudAction::Vision(Some(1 - 1))]
    );
    rig.view.perspective = Some(0);
    assert_eq!(
        rig.click(Vec2::new(EDGE + 120.0, first_card + 60.0)),
        vec![HudAction::Vision(None)],
        "the card being looked through goes back to everyone's eyes"
    );
    assert_eq!(
        rig.click(Vec2::new(EDGE + observer::WIDTH - 37.0, first_card + 16.0)),
        vec![HudAction::FocusPlayer(0)]
    );
    assert_eq!(
        rig.click(Vec2::new(
            EDGE + 18.0 + 52.0 + 6.0 + 34.0 + 4.0 + 17.0,
            EDGE + 47.0
        )),
        vec![HudAction::Vision(Some(1))],
        "the vision chips pick a side"
    );
}

#[test]
fn hovering_a_unit_fills_the_info_panel_when_nothing_is_selected() {
    let mut rig = Rig::new("aster_t1_tank");
    rig.view.selection.clear();
    let info = Vec2::new(INFO_X + 24.0, DECK_Y + 40.0);
    rig.settle();
    assert!(
        !rig.hud.covers(info),
        "an empty selection leaves the info panel off"
    );
    rig.hover = Some(7);
    rig.settle();
    assert!(
        rig.hud.covers(info),
        "hovering a unit should open its dossier"
    );
    assert_eq!(
        rig.click(order_slot(0, 0)),
        vec![],
        "a hover inspect has no order card"
    );
}

#[test]
fn an_enemy_selection_shows_details_without_orders() {
    let mut rig = Rig::new("aster_t1_tank");
    rig.view.frame.units[0].owner_flags = 1;
    let info = Vec2::new(INFO_X + 24.0, DECK_Y + 40.0);
    rig.frame(&Input::default());
    assert!(
        rig.hud.covers(info),
        "inspecting an enemy still has a dossier"
    );
    assert_eq!(
        rig.click(order_slot(0, 0)),
        vec![],
        "an enemy cannot be ordered"
    );
}

#[test]
fn hovering_an_enemy_while_selected_opens_a_hover_card() {
    let mut rig = Rig::new("aster_t1_tank");
    let mut enemy = rig.view.frame.units[0];
    enemy.unit_id = 8;
    enemy.owner_flags = 1;
    enemy.blueprint = rig.blueprints.id_of("aster_t1_scout").unwrap().0 as u32;
    rig.view.frame.units.push(enemy);
    rig.view.index_of.insert(8, 1);
    rig.hover = Some(8);
    rig.frame(&Input::default());
    assert!(
        rig.hud.covers(Vec2::new(INFO_X + 24.0, DECK_Y - 80.0)),
        "the hover card sits above the deck"
    );
    assert_eq!(
        rig.click(order_slot(0, 0)),
        vec![HudAction::Target(crate::game::Targeting::Move)],
        "the army's order card stays"
    );
}

#[test]
fn construction_keys_pick_a_tier_a_shelf_and_an_item() {
    let mut rig = Rig::new("aster_t1_engineer");
    rig.hud.build_keys = true;
    rig.frame(&Input::default());
    let blueprints = rig.blueprints.clone();
    let items: Vec<&UnitBlueprint> = blueprints
        .unit(blueprints.id_of("aster_t1_engineer").unwrap())
        .builder
        .as_ref()
        .unwrap()
        .builds
        .iter()
        .map(|b| blueprints.unit(*b))
        .filter(|b| b.tech == 1 && style::Purpose::of(b, true) == style::Purpose::Economy)
        .collect();
    rig.hud.build_key = Some('1');
    rig.frame(&Input::default());
    rig.hud.build_key = Some('W');
    rig.frame(&Input::default());
    rig.hud.build_key = Some('S');
    assert_eq!(
        rig.frame(&Input::default()),
        vec![HudAction::Build(items[1].id)]
    );
}

#[test]
fn details_opens_a_card_of_lore_and_weapons_over_the_panel() {
    let mut rig = Rig::new("aster_t1_tank");
    let details = Vec2::new(INFO_X + 336.0 - 16.0 - 13.0, DECK_Y + 26.0);
    rig.click(details);
    assert!(rig.hud.details_open);
    rig.frame(&Input::default());
    assert!(
        rig.hud.covers(Vec2::new(INFO_X + 200.0, DECK_Y - 40.0)),
        "the card sits over the deck"
    );
    rig.click(details);
    assert!(!rig.hud.details_open);
}

#[test]
fn details_close_when_the_selection_changes_or_comes_back() {
    let mut rig = Rig::new("aster_t1_tank");
    let details = Vec2::new(INFO_X + 336.0 - 16.0 - 13.0, DECK_Y + 26.0);
    rig.click(details);
    assert!(rig.hud.details_open);
    rig.view.selection.clear();
    rig.frame(&Input::default());
    assert!(!rig.hud.details_open, "deselecting closes the card");

    rig.view.selection = vec![7];
    rig.settle();
    rig.click(details);
    assert!(rig.hud.details_open);
    rig.view.selection.clear();
    rig.frame(&Input::default());
    rig.view.selection = vec![7];
    rig.frame(&Input::default());
    assert!(
        !rig.hud.details_open,
        "coming back to the unit finds the card closed"
    );
}

#[test]
fn the_construction_strip_scrolls_sideways() {
    // A commander's order card leaves its strip too short for all of tier 1.
    let mut rig = Rig::new("aster_commander");
    let tile = first_tile(4);
    let first = rig.click(tile);
    assert_eq!(rig.hud.build_scroll, 0.0);
    rig.frame(&Input {
        cursor: tile,
        scroll: -1.0,
        ..Default::default()
    });
    assert!(rig.hud.build_scroll > 0.0, "the wheel runs the strip along");
    let later = rig.click(tile);
    assert!(
        !later.is_empty() && later != first,
        "{first:?} then {later:?}"
    );
    // A shelf's key runs it to that shelf; the left arrow pages back to the start.
    rig.hud.build_keys = true;
    rig.hud.build_key = Some('Q');
    rig.frame(&Input::default());
    assert_eq!(rig.hud.build_scroll, 0.0, "factories are the first shelf");
    rig.hud.build_scroll = 400.0;
    rig.click(Vec2::new(build_x(4) + 14.0 + 12.0, tile.y));
    assert_eq!(rig.hud.build_scroll, 0.0);
}

#[test]
fn a_lift_ship_hold_lets_out_what_is_clicked_and_its_card_lands_and_takes_off() {
    use mc_sim::mirror::{CargoUnit, CargoView, LiftPhase};
    let mut rig = Rig::new("aster_t2_lift_ship");
    let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
    let bot = rig.blueprints.id_of("aster_t1_engineer").unwrap();
    let rider = |unit_id, blueprint| CargoUnit {
        unit_id,
        blueprint,
        health: 1.0,
        room: 2,
    };
    let cargo = |phase| CargoView {
        capacity: 96,
        used: 6,
        stored: vec![rider(21, tank), rider(22, bot), rider(23, tank)],
        boarding: 0,
        ramp_down: phase == LiftPhase::Ready,
        unloading: false,
        phase,
        to_unload: 0,
    };
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        cargo: Some(cargo(LiftPhase::Ready)),
        ..Default::default()
    }];
    rig.settle();
    // The hold is a panel right of the order card, a card per kind aboard: the
    // first card is the tanks, and a click lets one of them out.
    let mut first = None;
    // Settled once; a probe that set anything off settles it again.
    rig.settle();
    let mut still = true;
    'scan: for y in (0..12).map(|i| DECK_Y + 50.0 + i as f32 * 4.0) {
        for x in (0..140).map(|i| ORDERS_X + 100.0 + i as f32 * 8.0) {
            if !still {
                rig.settle();
            }
            let got = rig.tap(Vec2::new(x, y));
            still = got.is_empty();
            if got.iter().any(|a| matches!(a, HudAction::UnloadUnits(_))) {
                assert_eq!(
                    got,
                    vec![HudAction::UnloadUnits(vec![21])],
                    "one click lets one unit out"
                );
                first = Some(Vec2::new(x, y));
                break 'scan;
            }
        }
    }
    let first = first.expect("no hold card to click right of the order card");
    // Shift-click: every unit of that kind.
    rig.view.shift = true;
    assert_eq!(rig.click(first), vec![HudAction::UnloadUnits(vec![21, 23])]);
    rig.view.shift = false;
    // Ctrl-click: pick that kind alongside the ship instead.
    rig.view.ctrl = true;
    assert_eq!(
        rig.click(first),
        vec![HudAction::Select {
            units: vec![7, 21, 23],
            focus: false
        }]
    );
    rig.view.ctrl = false;
    // The order card has a Transport column: down, it offers Take Off; aloft, Land Here.
    let card = |rig: &mut Rig, row: usize| -> Vec<HudAction> {
        (0..6)
            .flat_map(|col| {
                let x = ORDERS_X
                    + 14.0
                    + col as f32 * (selection::ORDER_W + selection::ORDER_GAP)
                    + 50.0;
                let y =
                    DECK_Y + 36.0 + row as f32 * (selection::ORDER_H + selection::ORDER_GAP) + 20.0;
                rig.click(Vec2::new(x, y))
            })
            .collect()
    };
    assert!(
        card(&mut rig, 3).contains(&HudAction::TakeOff),
        "no Take Off on the card"
    );
    assert!(
        card(&mut rig, 2).contains(&HudAction::UnloadHere),
        "no Unload Here on the card"
    );
    assert!(card(&mut rig, 0).contains(&HudAction::Target(crate::game::Targeting::Land)));
    assert!(card(&mut rig, 1).contains(&HudAction::Target(crate::game::Targeting::Unload)));
    rig.view.status.queues[0].cargo = Some(cargo(LiftPhase::InFlight));
    assert!(
        card(&mut rig, 3).contains(&HudAction::LandHere),
        "no Land Here while aloft"
    );
    assert_eq!(
        super::cargo::status(&cargo(LiftPhase::RampOpening)).0,
        "Ramp opening"
    );
    let mut out = cargo(LiftPhase::Unloading);
    out.to_unload = 2;
    assert_eq!(super::cargo::status(&out).0, "Unloading \u{b7} 2 left");
}
