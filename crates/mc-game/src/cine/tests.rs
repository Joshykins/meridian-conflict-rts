use super::*;
use std::cell::Cell;

/// Flat ground at 10 m, and one unit that can be moved.
struct Flat {
    unit: Cell<Vec3>,
}

impl World for Flat {
    fn ground(&self, _: Vec2) -> f32 {
        10.0
    }
    fn unit(&self, id: u32) -> Option<(Vec3, f32)> {
        (id == 1).then(|| (self.unit.get(), 4.0))
    }
}

fn strategic() -> Camera {
    let mut c = Camera::new(Vec2::splat(16_384.0), Vec2::new(1920.0, 1080.0));
    c.focus = Vec3::new(4000.0, 4000.0, 10.0);
    c.distance = 300.0;
    c.yaw = 0.8;
    c.tilt = 0.4;
    c
}

fn run(cine: &mut Cine, world: &Flat, seconds: f32, c: &Controls) {
    for _ in 0..(seconds * 60.0) as usize {
        cine.update(1.0 / 60.0, c, world, Vec2::splat(16_384.0));
    }
}

#[test]
fn taking_over_keeps_the_view_exactly() {
    let before = strategic();
    let world = Flat {
        unit: Cell::new(before.focus),
    };
    let mut cine = Cine::default();
    cine.enter(&before);
    // Locked on and riding with the unit at the focus, as from an Alt-orbit.
    cine.aim = Some(Aim::Unit(1));
    cine.follow = Some((1, cine.goal.eye - before.focus));
    run(&mut cine, &world, 0.5, &Controls::default());
    let mut after = before.clone();
    cine.apply(&mut after, &world);
    assert!(
        after.eye().distance(before.eye()) < 0.01,
        "{:?} {:?}",
        after.eye(),
        before.eye()
    );
    let (fa, fb) = (
        (after.focus - after.eye()).normalize(),
        (before.focus - before.eye()).normalize(),
    );
    assert!(fa.dot(fb) > 0.99999, "the look direction moved");
}

#[test]
fn following_carries_the_eye_with_the_unit() {
    let world = Flat {
        unit: Cell::new(Vec3::new(4000.0, 4000.0, 10.0)),
    };
    let mut cine = Cine::default();
    cine.enter(&strategic());
    cine.aim = Some(Aim::Unit(1));
    cine.follow = Some((1, cine.goal.eye - world.unit.get()));
    let eye = cine.shown.eye;
    for _ in 0..240 {
        world.unit.set(world.unit.get() + Vec3::new(0.5, 0.0, 0.0));
        cine.update(
            1.0 / 60.0,
            &Controls::default(),
            &world,
            Vec2::splat(16_384.0),
        );
    }
    let moved = cine.shown.eye - eye;
    assert!(
        (moved.x - 120.0).abs() < 1.0 && moved.y.abs() < 0.5,
        "{moved:?}"
    );
}

#[test]
fn a_saved_shot_is_glided_to_and_landed_on() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    let mut cine = Cine::default();
    cine.enter(&strategic());
    cine.save(0);
    let shot = cine.goal;
    run(
        &mut cine,
        &world,
        1.0,
        &Controls {
            fly: Vec3::new(1.0, 1.0, 0.5),
            ..Default::default()
        },
    );
    run(&mut cine, &world, 1.0, &Controls::default());
    assert!(cine.shown.eye.distance(shot.eye) > 50.0);
    assert!(cine.recall(0, false));
    assert!(cine.gliding());
    run(&mut cine, &world, 8.0, &Controls::default());
    assert!(cine.settled());
    assert!(cine.shown.eye.distance(shot.eye) < 0.05);
}

#[test]
fn the_eye_stays_off_the_ground_and_locked_stays_put() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    let mut cine = Cine::default();
    cine.enter(&strategic());
    run(
        &mut cine,
        &world,
        6.0,
        &Controls {
            fly: Vec3::new(0.0, 0.0, -1.0),
            fast: true,
            ..Default::default()
        },
    );
    assert!(
        cine.shown.eye.z >= 10.0 + CLEARANCE - 0.01,
        "{}",
        cine.shown.eye.z
    );
    cine.locked = true;
    let at = cine.goal;
    run(
        &mut cine,
        &world,
        1.0,
        &Controls {
            fly: Vec3::Y,
            look: Vec2::splat(0.1),
            ..Default::default()
        },
    );
    assert_eq!(cine.goal, at);
}

#[test]
fn looking_up_past_the_horizon_is_allowed() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    let mut cine = Cine::default();
    cine.enter(&strategic());
    run(
        &mut cine,
        &world,
        2.0,
        &Controls {
            look: Vec2::new(0.0, -0.05),
            ..Default::default()
        },
    );
    assert!((cine.goal.pitch - PITCH_MIN).abs() < 1e-4);
    let mut cam = strategic();
    cine.apply(&mut cam, &world);
    assert!(cam.pitch() < 0.0 && cam.eye().is_finite());
}

/// Leaves the free camera and plays the hand-back out, the way the game's frame
/// does: lifted, the strategic camera's own frame (`between`), put back on.
fn hand_back(
    cine: &mut Cine,
    world: &Flat,
    camera: &mut Camera,
    mut between: impl FnMut(&mut Camera),
) -> (Option<u32>, Camera) {
    cine.apply(camera, world);
    let before = camera.clone();
    let track = cine.leave(camera, world);
    cine.hand_back_apply(camera, 0.0);
    assert!(
        camera.eye().distance(before.eye()) < 0.05,
        "the first frame jumped: {:?} {:?}",
        camera.eye(),
        before.eye()
    );
    let (a, b) = (
        (camera.focus - camera.eye()).normalize(),
        (before.focus - before.eye()).normalize(),
    );
    assert!(a.dot(b) > 0.9999, "the first frame turned");
    for _ in 0..60 {
        cine.hand_back_lift(camera);
        between(camera);
        cine.hand_back_apply(camera, 1.0 / 60.0);
    }
    (track, before)
}

#[test]
fn leaving_lands_on_the_nearest_strategic_view_at_once() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    let mut cine = Cine::default();
    let mut camera = strategic();
    cine.enter(&camera);
    // Somewhere else, lower, on a long lens.
    run(
        &mut cine,
        &world,
        1.5,
        &Controls {
            fly: Vec3::new(0.4, 1.0, -0.3),
            look: Vec2::new(0.0, -0.002),
            lens: 1.0,
            ..Default::default()
        },
    );
    run(&mut cine, &world, 1.0, &Controls::default());
    let (_, before) = hand_back(&mut cine, &world, &mut camera, |_| {});
    // Done inside the frames played, and a plain strategic camera.
    assert!(camera.pitch_free.is_none() && camera.fov == FOV_Y);
    // Looking at the same place from as far away, at the player's own tilt.
    assert!((camera.tilt - strategic().tilt).abs() < 1e-5);
    assert!(
        (camera.eye().distance(camera.focus) - before.eye().distance(before.focus)).abs() < 1.0
    );
    let ahead = before.focus - before.eye();
    let off = (camera.focus - before.eye())
        .normalize()
        .dot(ahead.normalize());
    assert!(off > 0.9999, "the focus left the free view's line");
    assert!((camera.focus.z - 10.0).abs() < 0.1, "on the ground");
}

#[test]
fn leaving_puts_the_players_tilt_back() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    for pitch in [-0.3, 0.05, 0.6, 1.4] {
        let mut cine = Cine::default();
        let mut camera = strategic();
        cine.enter(&camera);
        cine.cut_to(Pose { pitch, ..cine.goal });
        hand_back(&mut cine, &world, &mut camera, |_| {});
        assert!(
            (camera.tilt - strategic().tilt).abs() < 1e-5,
            "{pitch}: tilt {}",
            camera.tilt
        );
    }
}

#[test]
fn leaving_from_the_sky_tips_down_onto_the_ground() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    let mut cine = Cine::default();
    let mut camera = strategic();
    cine.enter(&camera);
    cine.cut_to(Pose {
        pitch: -0.4,
        ..cine.goal
    });
    let (_, before) = hand_back(&mut cine, &world, &mut camera, |_| {});
    assert!(camera.pitch_free.is_none() && camera.pitch() > 0.3);
    assert!((camera.focus.z - 10.0).abs() < 0.1, "on the ground");
    assert!(camera.eye().distance(before.eye()) < 200.0);
}

#[test]
fn the_player_has_the_camera_during_the_hand_back() {
    let world = Flat {
        unit: Cell::new(Vec3::ZERO),
    };
    let setup = || {
        let camera = strategic();
        let mut cine = Cine::default();
        cine.enter(&camera);
        cine.cut_to(Pose {
            pitch: 0.1,
            fov: 0.2,
            ..cine.goal
        });
        (cine, camera)
    };
    let (mut probe, mut landed) = setup();
    hand_back(&mut probe, &world, &mut landed, |_| {});
    let (mut cine, mut camera) = setup();
    // Panning and turning every frame of it: all of it counts.
    hand_back(&mut cine, &world, &mut camera, |c| {
        c.focus.x += 2.0;
        c.yaw += 0.01;
    });
    assert!((camera.focus.x - landed.focus.x - 120.0).abs() < 0.01);
    assert!((camera.yaw - landed.yaw - 0.6).abs() < 1e-4);
}

#[test]
fn leaving_while_riding_with_a_unit_tracks_it() {
    let world = Flat {
        unit: Cell::new(Vec3::new(4200.0, 4100.0, 10.0)),
    };
    let mut cine = Cine::default();
    let mut camera = strategic();
    cine.enter(&camera);
    cine.aim = Some(Aim::Unit(1));
    cine.follow = Some((1, cine.goal.eye - world.unit.get()));
    run(&mut cine, &world, 2.0, &Controls::default());
    let (track, _) = hand_back(&mut cine, &world, &mut camera, |_| {});
    assert_eq!(track, Some(1));
    assert!(camera.focus.distance(world.unit.get()) < 0.01);
    assert!(cine.aim.is_none() && cine.follow.is_none());
}
