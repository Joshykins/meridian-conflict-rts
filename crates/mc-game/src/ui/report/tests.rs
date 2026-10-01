use super::analysis::{region, Analysis, MomentKind};
use super::*;
use crate::chronicle::{
    Built, Death, Frame, Kill, Salvage, Sample, SideFrame, SideInfo, SideSample,
};
use crate::ui::{Input, Memory};
use mc_render::Overlay;

fn blueprints() -> Blueprints {
    Blueprints::load(&Blueprints::locate_data_dir().expect("data dir")).expect("blueprints")
}

/// A ten-minute match on a 10 km map: two sides build up, fight one big battle in
/// the north-east at six minutes, side 0 reclaims the field after it (a materials a
/// tick from 6:40), and side 1's commander falls at nine.
/// Side 0's reclaim by `tick`: one material a tick from 6:40.
fn reclaimed(side: u32, tick: u32) -> f32 {
    if side == 0 {
        tick.saturating_sub(4000) as f32
    } else {
        0.0
    }
}

fn chronicle(bp: &Blueprints) -> Chronicle {
    let id = |k: &str| bp.id_of(k).expect("blueprint exists");
    let (tank, factory, commander) = (
        id("aster_t1_tank"),
        id("aster_t2_land_factory"),
        id("aster_commander"),
    );
    let mut c = Chronicle::new(Vec2::splat(10_000.0));
    c.sides = (0..2)
        .map(|i| SideInfo {
            name: format!("Side {i}"),
            team: i,
            faction: 0,
            ai: i == 1,
        })
        .collect();
    let end = 6000;
    for tick in (0..=end).step_by(crate::chronicle::SAMPLE_TICKS as usize) {
        let k = tick as f32 / end as f32;
        c.samples.push(Sample {
            tick,
            sides: (0..2)
                .map(|i| SideSample {
                    mass_income: 20.0 + 60.0 * k * (1.0 + i as f32 * 0.2),
                    energy_income: 200.0 * k,
                    mass_spent: 18.0 + 50.0 * k,
                    efficiency: 0.95,
                    army: (40.0 * k) as u32,
                    army_value: 4000.0 * k * (1.0 - i as f32 * 0.3),
                    reclaim_income: if i == 0 && tick > 4000 { 10.0 } else { 0.0 },
                    reclaimed: reclaimed(i, tick),
                    ..Default::default()
                })
                .collect(),
        });
    }
    for tick in (0..=end).step_by(crate::chronicle::FRAME_TICKS as usize) {
        let k = tick as f32 / end as f32;
        c.frames.push(Frame {
            tick,
            sides: (0..2)
                .map(|i| SideFrame {
                    army: vec![((10 + i * 40) as u16 + (k * 20.0) as u16, 300.0)],
                    bases: vec![(i as u16 * 4000, 3)],
                    commander: Some(Vec2::new(1000.0 + 8000.0 * i as f32, 1000.0 + 500.0 * k)),
                    // On the battlefield: cell (51, 51) is (8000, 8000) on a 10 km map.
                    salvage: if reclaimed(i, tick) > 0.0 {
                        vec![(51 * 64 + 51, 50.0)]
                    } else {
                        Vec::new()
                    },
                    reclaimed: reclaimed(i, tick),
                })
                .collect(),
        });
    }
    // The battle: forty tanks of side 1 and ten of side 0 between 6:00 and 6:40.
    for n in 0..50u32 {
        let (owner, by) = if n < 40 { (1, 0) } else { (0, 1) };
        let tick = 3600 + n * 8;
        let pos = Vec2::new(
            8000.0 + (n % 7) as f32 * 30.0,
            8000.0 + (n % 5) as f32 * 30.0,
        );
        c.kills.push(Kill {
            tick,
            by,
            victim: owner,
            blueprint: tank,
            weapon_of: Some(tank),
            complete: true,
        });
        c.deaths.push(Death {
            tick,
            pos,
            owner,
            blueprint: tank,
            complete: true,
        });
    }
    c.built.push(Built {
        tick: 2400,
        owner: 0,
        blueprint: factory,
        upgrade: false,
    });
    c.built.push(Built {
        tick: 2500,
        owner: 0,
        blueprint: id("aster_t1_power"),
        upgrade: false,
    });
    c.deaths.push(Death {
        tick: 5400,
        pos: Vec2::new(9000.0, 1500.0),
        owner: 1,
        blueprint: commander,
        complete: true,
    });
    c.salvages.push(Salvage {
        tick: 4500,
        pos: Vec2::splat(8010.0),
        blueprint: tank,
        by: Some(0),
    });
    c.salvages.push(Salvage {
        tick: 4600,
        pos: Vec2::splat(8020.0),
        blueprint: commander,
        by: None,
    });
    c.defeats.push((5400, 1));
    c.ended = Some((5400, 0));
    c.tick = end;
    c
}

#[test]
fn analysis_tells_the_match() {
    let bp = blueprints();
    let a = Analysis::of(&chronicle(&bp), &bp);
    assert_eq!(a.length, 5400, "the match ends where it was decided");
    assert!(a.sides[0].victor && !a.sides[1].victor);
    assert_eq!(a.sides[1].defeated_at, Some(5400));
    assert_eq!((a.sides[0].kills, a.sides[1].kills), (40, 10));
    let tank = bp
        .unit(bp.id_of("aster_t1_tank").unwrap())
        .cost_mass
        .to_f32();
    assert!((a.sides[0].destroyed - 40.0 * tank).abs() < 1.0);
    assert!((a.matrix[0][1] - 40.0 * tank).abs() < 1.0);
    assert_eq!(
        a.sides[0].tier_at[1],
        Some(2400),
        "the tier 2 factory brings tier 2"
    );
    // One battle, in the north-east (high x, high y), named for it.
    assert_eq!(a.battles.len(), 1);
    let b = &a.battles[0];
    assert!(b.name.contains("north-east"), "{}", b.name);
    assert!(b.losses[1] > b.losses[0]);
    let kinds: Vec<MomentKind> = a.moments.iter().map(|m| m.kind).collect();
    for k in [
        MomentKind::Start,
        MomentKind::FirstBlood,
        MomentKind::Tier,
        MomentKind::Battle,
        MomentKind::Defeat,
        MomentKind::End,
    ] {
        assert!(kinds.contains(&k), "{k:?} missing from {kinds:?}");
    }
    assert!(a.moments.windows(2).all(|w| w[0].tick <= w[1].tick));
    assert!(a.awards.iter().any(|w| w.title == "Warlord" && w.side == 0));
    // The curves run over every sample; the totals stop at the side's defeat.
    assert_eq!(a.curve(Metric::ArmyValue, 0).len(), a.times.len());
    assert!(a.curve(Metric::ArmyValue, 1).last() == Some(&0.0));
}

#[test]
fn reclaim_is_told() {
    let bp = blueprints();
    let a = Analysis::of(&chronicle(&bp), &bp);
    let s = &a.sides[0];
    assert_eq!(
        s.reclaimed, 1400.0,
        "the record's running total, to the decision"
    );
    assert!((s.collected - (s.mined + s.reclaimed)).abs() < 0.01);
    assert_eq!(a.sides[1].reclaimed, 0.0);
    assert_eq!(a.total_reclaimed, 1400.0);
    // One a tick is ten a second, over any half minute of it.
    assert!((s.peak_reclaim - 10.0).abs() < 0.01, "{}", s.peak_reclaim);
    assert_eq!(
        s.wrecks_cleared, 1,
        "the wreck no one's beam finished is no one's"
    );
    // Salvaged on the battlefield, the richest wreck first.
    let (at, _) = a.salvage_points(0).next().expect("salvage on the map");
    assert!(at.distance(Vec2::splat(8000.0)) < 200.0, "{at}");
    assert_eq!(a.hauls.len(), 2);
    assert!(a.hauls[0].value >= a.hauls[1].value);
    assert!(a.moments.iter().any(|m| m.kind == MomentKind::Salvage
        && m.side == Some(0)
        && m.title == "1.0k reclaimed"));
    assert!(a
        .awards
        .iter()
        .any(|w| w.title == "Salvager" && w.side == 0));
    let rate = a.curve(Metric::ReclaimRate, 0);
    assert!(rate.iter().any(|v| (v - 10.0).abs() < 0.01));
}

#[test]
fn regions_follow_the_chart() {
    let size = Vec2::splat(1000.0);
    assert_eq!(region(Vec2::splat(500.0), size), "the centre");
    // The map's y runs north (up the chart).
    assert_eq!(region(Vec2::new(500.0, 950.0), size), "the north");
    assert_eq!(region(Vec2::new(50.0, 50.0), size), "the south-west");
}

#[test]
fn every_page_draws_at_every_size() {
    let bp = blueprints();
    let c = chronicle(&bp);
    let thumbs = Thumbs::default();
    let audio = crate::audio::Audio::silent();
    for viewport in [
        Vec2::new(1280.0, 720.0),
        Vec2::new(1920.0, 1080.0),
        Vec2::new(5120.0, 1440.0),
    ] {
        let mut report = Report::new(&c, &bp, Some(0));
        assert_eq!(report.verdict, Verdict::Victory);
        let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
        for (frame, tab) in Tab::ALL.iter().cycle().take(40).enumerate() {
            report.tab = *tab;
            // The pointer wanders over the page, so the hover cards draw too.
            let cursor = Vec2::new(
                viewport.x * (0.15 + 0.1 * (frame % 8) as f32),
                viewport.y * (0.35 + 0.08 * (frame % 6) as f32),
            );
            let input = Input {
                cursor,
                ..Default::default()
            };
            overlay.clear();
            memory.begin_frame();
            let mut ui = Ui::new(
                &mut overlay,
                &input,
                &mut memory,
                &audio,
                viewport,
                1.0,
                frame as f32 * 0.1,
                0.1,
            );
            let ctx = Ctx {
                blueprints: &bp,
                colors: &crate::setup::TEAM_COLORS,
                local: Some(0),
                map_name: "Test Field",
                thumbs: &thumbs,
                chart: crate::hud::MINIMAP_SLOT,
                surrender: false,
            };
            assert_eq!(report.draw(&mut ui, &ctx, 1.0), None);
            memory.end_frame(&input);
            assert!(!overlay.overflowed, "{tab:?} at {viewport} overflowed");
        }
    }
}

#[test]
fn verdicts() {
    let bp = blueprints();
    let mut c = chronicle(&bp);
    assert_eq!(Report::new(&c, &bp, Some(1)).verdict, Verdict::Defeat);
    assert_eq!(Report::new(&c, &bp, None).verdict, Verdict::Complete);
    c.ended = None;
    assert_eq!(Report::new(&c, &bp, None).verdict, Verdict::Running);
}
