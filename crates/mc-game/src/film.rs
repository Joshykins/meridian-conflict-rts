//! `--film DIR`: one camera move over a match, drawn headless frame by frame into
//! DIR as numbered PNGs (`scripts/film.sh` cuts them into a video).
//!
//! The camera is rigged on an anchor: a fixed point (`--camera X,Y`), or the
//! middle of the units on the field (only those whose key contains `--film-on`),
//! followed smoothly so a death or a charge never jolts the frame. The rig eases
//! from `--film-from` to `--film-to` over the clip: its bearing, its angle down,
//! its distance, its lens, how high above the ground it aims and how far to the
//! side of the anchor, so one pair of rigs makes an orbit, a crane, a push in or a
//! truck past the fight. The sim plays at `--film-speed` (half speed is slow
//! motion), and no interface is drawn.

use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_render::{Camera, FrameInput, Overlay, Renderer};
use mc_sim::mirror::{KIND_GHOST, KIND_WRECK};
use mc_sim::{RenderFrame, World};
use std::path::PathBuf;
use std::sync::mpsc;

/// The lowest the eye may come over the ground or the sea, metres.
const CLEARANCE: f32 = 6.0;
/// Seconds of film time the anchor takes to close most of the way on the units.
const ANCHOR_LAG: f32 = 1.2;
/// Frames drawn at the first rig, not kept, so streamed terrain and shadows settle.
const SETTLE_FRAMES: u32 = 30;
/// PNG writers at once: encoding a 1080p frame takes longer than drawing it.
const WRITERS: usize = 6;

/// Where the camera stands relative to its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    /// Bearing the camera looks along, degrees (0 looks along +Y).
    pub yaw: f32,
    /// Degrees below the horizon.
    pub pitch: f32,
    /// Metres from the aim point to the eye.
    pub distance: f32,
    /// Vertical field of view, degrees.
    pub fov: f32,
    /// Metres the aim point sits above the ground under the anchor.
    pub lift: f32,
    /// Metres the aim point sits to the camera's right of the anchor.
    pub side: f32,
}

impl Rig {
    /// `YAW,PITCH,DIST[,FOV[,LIFT[,SIDE]]]`.
    pub fn parse(s: &str) -> Option<Rig> {
        let v: Vec<f32> = s
            .split(',')
            .map(|p| p.trim().parse().ok())
            .collect::<Option<_>>()?;
        if !(3..=6).contains(&v.len()) || v[2] <= 0.0 {
            return None;
        }
        Some(Rig {
            yaw: v[0],
            pitch: v[1],
            distance: v[2],
            fov: v.get(3).copied().unwrap_or(34.0),
            lift: v.get(4).copied().unwrap_or(4.0),
            side: v.get(5).copied().unwrap_or(0.0),
        })
    }

    /// Between `self` (t = 0) and `to` (t = 1); distance and lens on a log scale,
    /// so a push in or a zoom is as steady at its end as at its start.
    fn toward(&self, to: &Rig, t: f32) -> Rig {
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let log = |a: f32, b: f32| (a.ln() + (b.ln() - a.ln()) * t).exp();
        Rig {
            yaw: lerp(self.yaw, to.yaw),
            pitch: lerp(self.pitch, to.pitch),
            distance: log(self.distance, to.distance),
            fov: log(self.fov, to.fov),
            lift: lerp(self.lift, to.lift),
            side: lerp(self.side, to.side),
        }
    }
}

/// A clip to film (`--film` and its options).
#[derive(Clone, Debug)]
pub struct Film {
    pub dir: PathBuf,
    pub secs: f32,
    pub fps: u32,
    /// Sim seconds per film second: 0.5 is half-speed slow motion.
    pub speed: f32,
    pub from: Rig,
    pub to: Rig,
    /// Anchor on the units whose blueprint key contains this, not on them all.
    pub on: Option<String>,
}

impl Film {
    pub fn new(dir: PathBuf) -> Film {
        let rig = Rig {
            yaw: 30.0,
            pitch: 14.0,
            distance: 420.0,
            fov: 34.0,
            lift: 4.0,
            side: 0.0,
        };
        Film {
            dir,
            secs: 8.0,
            fps: 30,
            speed: 1.0,
            from: rig,
            to: Rig {
                yaw: rig.yaw + 40.0,
                ..rig
            },
            on: None,
        }
    }

    /// Frames drawn per sim tick, whole so every tick lands on a frame.
    fn per_tick(&self) -> u32 {
        (self.fps as f32 / (mc_core::TICKS_PER_SECOND as f32 * self.speed))
            .round()
            .max(1.0) as u32
    }
}

/// The middle of the units the film is on: `None` when there are none.
fn units_middle(frame: &RenderFrame, blueprints: &Blueprints, on: Option<&str>) -> Option<Vec2> {
    let (mut sum, mut n) = (Vec2::ZERO, 0.0);
    for u in &frame.units {
        if u.owner_flags & (KIND_WRECK | KIND_GHOST) != 0 {
            continue;
        }
        let bp = blueprints.unit(mc_data::BlueprintId(u.blueprint as u16));
        if on.is_some_and(|key| !bp.key.contains(key)) || (on.is_none() && !bp.is_mobile()) {
            continue;
        }
        sum += Vec2::new(u.pos[0], u.pos[1]);
        n += 1.0;
    }
    (n > 0.0).then(|| sum / n)
}

/// Puts the camera on `rig` about `anchor`, the eye kept clear of the ground.
fn place(camera: &mut Camera, renderer: &Renderer, anchor: Vec2, rig: &Rig, water: f32) {
    let ground = |p: Vec2| renderer.ground_height(p).max(water);
    let yaw = rig.yaw.to_radians();
    let right = Vec2::new(yaw.cos(), -yaw.sin());
    let at = anchor + right * rig.side;
    camera.focus = Vec3::new(at.x, at.y, ground(at) + rig.lift);
    camera.distance = rig.distance;
    camera.yaw = yaw;
    camera.fov = rig.fov.to_radians();
    let mut pitch = rig.pitch.to_radians();
    camera.pitch_free = Some(pitch);
    // Raise the angle until the eye clears whatever is under it.
    for _ in 0..40 {
        let eye = camera.eye();
        if eye.z >= ground(eye.truncate()) + CLEARANCE {
            break;
        }
        pitch += 0.02;
        camera.pitch_free = Some(pitch);
    }
}

/// Eases in and out, but never stops: half constant speed, half a cosine ease.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 * t + 0.25 * (1.0 - (std::f32::consts::PI * t).cos())
}

/// What the film draws with: the headless shot's world, mirror and renderer.
pub struct Stage<'a> {
    pub world: &'a mut World,
    pub frame: &'a mut RenderFrame,
    pub renderer: &'a mut Renderer,
    pub camera: &'a mut Camera,
    /// Whose eyes the mirror is written through.
    pub eyes: Option<u8>,
    /// A fixed anchor (`--camera X,Y`) instead of the units.
    pub fixed: Option<Vec2>,
    pub water: f32,
    pub width: u32,
    pub height: u32,
}

/// Films the clip into `film.dir`, and writes its last frame to `last` too.
pub fn roll(film: &Film, stage: Stage<'_>, last: &std::path::Path) -> Result<(), String> {
    let Stage {
        world,
        frame,
        renderer,
        camera,
        eyes,
        fixed,
        water,
        width,
        height,
    } = stage;
    std::fs::create_dir_all(&film.dir).map_err(|e| format!("{}: {e}", film.dir.display()))?;
    let blueprints = world.blueprints.clone();
    let middle = |frame: &RenderFrame| {
        fixed.or_else(|| units_middle(frame, &blueprints, film.on.as_deref()))
    };
    let mut anchor = middle(frame).ok_or(match &film.on {
        Some(key) => format!("--film-on {key}: no unit's key has that in it"),
        None => "--film: there are no units to film".to_owned(),
    })?;
    let overlay = Overlay::default();
    let draw = |renderer: &mut Renderer,
                camera: &Camera,
                frame: &RenderFrame,
                fresh: bool,
                time: f32,
                alpha: f32| {
        renderer
            .render(&FrameInput {
                camera,
                time,
                alpha,
                sim: fresh.then_some(frame),
                latest: Some(frame),
                ghosts: &[],
                marks: &[],
                ranges: &[],
                ranges_drawn: 0,
                overlay: &overlay,
                build_grid: false,
                icons: false,
            })
            .map_err(|e| e.to_string())
    };

    let mut time = 20.0;
    place(camera, renderer, anchor, &film.from, water);
    for i in 0..SETTLE_FRAMES {
        draw(renderer, camera, frame, i == 0, time, 1.0)?;
    }

    // Frames go to a few writer threads; the bounded queue keeps memory to a few frames.
    let (send, take) = mpsc::sync_channel::<(PathBuf, Vec<u8>)>(WRITERS * 2);
    let take = std::sync::Arc::new(std::sync::Mutex::new(take));
    let writers: Vec<_> = (0..WRITERS)
        .map(|_| {
            let take = take.clone();
            std::thread::spawn(move || -> Result<(), String> {
                loop {
                    let job = take.lock().map_err(|e| e.to_string())?.recv();
                    let Ok((path, rgba)) = job else {
                        return Ok(());
                    };
                    write_fast_png(&path, width, height, &rgba)?;
                }
            })
        })
        .collect();

    let total = (film.secs * film.fps as f32).round().max(1.0) as u32;
    let per = film.per_tick();
    let dt = 1.0 / film.fps as f32 * film.speed;
    let follow = 1.0 - (-dt / ANCHOR_LAG).exp();
    let mut kept = 0;
    let mut rgba = Vec::new();
    'film: loop {
        world.tick(&[]).map_err(|e| e.to_string())?;
        world.write_render_frame(eyes, frame);
        for j in 1..=per {
            if kept == total {
                break 'film;
            }
            if let Some(to) = middle(frame) {
                anchor += (to - anchor) * follow;
            }
            let rig = film
                .from
                .toward(&film.to, ease(kept as f32 / (total - 1).max(1) as f32));
            place(camera, renderer, anchor, &rig, water);
            time += dt;
            draw(renderer, camera, frame, j == 1, time, j as f32 / per as f32)?;
            rgba = renderer
                .read_pixels()
                .ok_or("no pixels from a headless target")?;
            send.send((film.dir.join(format!("{kept:05}.png")), rgba.clone()))
                .map_err(|_| "a frame writer stopped")?;
            kept += 1;
        }
    }
    drop(send);
    for w in writers {
        w.join().map_err(|_| "a frame writer panicked")??;
    }
    println!("filmed {kept} frames into {}", film.dir.display());
    crate::headless::write_png(last, width, height, &rgba)
}

/// A PNG written for speed, not size: the frames are read once, by the encoder.
fn write_fast_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigs_parse_with_defaults() {
        let r = Rig::parse("30,12,400").expect("three figures are a rig");
        assert_eq!(
            (r.yaw, r.pitch, r.distance, r.fov),
            (30.0, 12.0, 400.0, 34.0)
        );
        assert_eq!(Rig::parse("0,10,80,20,6,-50").map(|r| r.side), Some(-50.0));
        assert!(Rig::parse("30,12").is_none());
        assert!(Rig::parse("30,12,0").is_none());
        assert!(Rig::parse("30,x,400").is_none());
    }

    #[test]
    fn a_move_starts_and_ends_on_its_rigs() {
        let a = Rig::parse("0,10,100,20").unwrap();
        let b = Rig::parse("90,30,400,40").unwrap();
        let start = a.toward(&b, ease(0.0));
        assert!((start.distance - 100.0).abs() < 0.01 && (start.fov - 20.0).abs() < 0.01);
        assert_eq!((start.yaw, start.pitch), (a.yaw, a.pitch));
        let end = a.toward(&b, ease(1.0));
        assert!((end.distance - 400.0).abs() < 0.01 && (end.yaw - 90.0).abs() < 0.01);
        // Halfway in distance is the geometric middle.
        assert!((a.toward(&b, 0.5).distance - 200.0).abs() < 0.01);
    }

    #[test]
    fn slow_motion_draws_more_frames_a_tick() {
        let mut f = Film::new(PathBuf::from("x"));
        assert_eq!(f.per_tick(), 3);
        f.speed = 0.5;
        assert_eq!(f.per_tick(), 6);
    }
}
