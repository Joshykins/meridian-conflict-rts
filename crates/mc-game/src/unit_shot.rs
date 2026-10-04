//! `--unit-shot KEY`: one unit on the test range, drawn with no HUD from several
//! angles in one run (a contact sheet), or as an animation (`--frames N`, an
//! animated PNG). A headless shot mostly pays for starting the renderer, so a
//! sheet of six views costs about what one `--screenshot` does.
//!
//! Angles are taken from the unit's own heading, so `front` faces its nose
//! whichever way it stands. `--look X,Y,Z` aims at a point in the unit's own
//! frame (metres: x forward, y left, z up from the ground under its centre) and
//! `--zoom` closes in on it, for one piece of a unit: a turret, a flank, a muzzle.
//! The range's `--scenario`, `--hurt` and `--ticks` stage what the unit is doing.
//!
//! KEY may also be a map prop's model key (`city_office`): the prop then stands
//! alone on the range pad at its authored size, its front (`front`, `front34`)
//! its street side, local +y (`mc_map::city`), and `--hurt` takes that share off
//! its health, so a city structure's windows break.

use crate::headless::{run_sim, write_png};
use crate::setup::{self, Options};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_map::{Prop, PropKind};
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK, WRECK_INNER};
use mc_sim::{RenderFrame, World};
use std::path::Path;
use std::sync::Arc;

pub(crate) const VIEWS_HELP: &str =
    "front34, front, left, right, rear34, back, top, low, or BEARING:ELEVATION in degrees";

/// What `--unit-shot` draws.
pub(crate) struct Spec {
    pub(crate) path: String,
    /// One view's size in pixels; a sheet lays its views out in rows of three.
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) views: Vec<Angle>,
    /// A point in the unit's own frame to aim at, in place of its middle.
    pub(crate) look: Option<[f32; 3]>,
    pub(crate) zoom: f32,
    /// Animated: this many frames at 20 a second (two a sim tick), the camera
    /// turning `turn` degrees about the unit over them from the first view.
    pub(crate) frames: u32,
    pub(crate) turn: f32,
}

/// Where the eye is: `bearing` degrees round from the unit's nose (positive
/// towards its left), `elevation` degrees above the horizon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Angle {
    pub(crate) bearing: f32,
    pub(crate) elevation: f32,
}

/// The default sheet: the three-quarter views a model is judged from, its
/// profile, and plan.
pub(crate) const SHEET: &str = "front34,front,left,rear34,back,top";

pub(crate) fn parse_views(list: &str) -> Result<Vec<Angle>, String> {
    list.split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| {
            let (bearing, elevation) = match v {
                "front34" => (35.0, 22.0),
                "front" => (0.0, 8.0),
                "left" => (90.0, 8.0),
                "right" => (-90.0, 8.0),
                "rear34" => (145.0, 22.0),
                "back" => (180.0, 8.0),
                "top" => (0.0, 88.0),
                "low" => (35.0, 2.0),
                custom => {
                    let (b, e) = custom
                        .split_once(':')
                        .ok_or(format!("--views: {custom:?} is not a view ({VIEWS_HELP})"))?;
                    let num = |s: &str| {
                        s.trim()
                            .parse::<f32>()
                            .map_err(|_| format!("--views: {custom:?} is not BEARING:ELEVATION"))
                    };
                    (num(b)?, num(e)?.clamp(-10.0, 89.0))
                }
            };
            Ok(Angle { bearing, elevation })
        })
        .collect::<Result<Vec<_>, String>>()
        .and_then(|v| {
            if v.is_empty() {
                Err(format!("--views needs at least one view ({VIEWS_HELP})"))
            } else {
                Ok(v)
            }
        })
}

/// Reads one of `--unit-shot`'s own options into `spec`, taking its value
/// from `value`; false when `arg` is not one of them.
pub(crate) fn flag(
    spec: &mut Spec,
    arg: &str,
    value: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<bool, String> {
    match arg {
        "--views" => spec.views = parse_views(&value(arg)?)?,
        "--look" => {
            let v: Vec<f32> = value(arg)?
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            spec.look = Some(<[f32; 3]>::try_from(v).map_err(|_| "--look takes X,Y,Z")?);
        }
        "--zoom" => {
            spec.zoom = value(arg)?
                .parse::<f32>()
                .ok()
                .filter(|z| *z > 0.0)
                .ok_or("--zoom takes a number above 0")?;
        }
        "--frames" => {
            spec.frames = value(arg)?.parse().map_err(|_| "--frames takes a number")?;
        }
        "--turn" => spec.turn = value(arg)?.parse().map_err(|_| "--turn takes degrees")?,
        _ => return Ok(false),
    }
    Ok(true)
}

impl Default for Spec {
    fn default() -> Spec {
        Spec {
            path: String::new(),
            width: 800,
            height: 600,
            views: Vec::new(),
            look: None,
            zoom: 1.0,
            frames: 0,
            turn: 0.0,
        }
    }
}

/// Runs the range for `ticks` and draws `spec`, on a renderer of its own.
pub(crate) fn run(
    opts: &Options,
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    ticks: u32,
    spec: &Spec,
) -> Result<(), String> {
    let mut studio = Studio::new(map, blueprints, pool, spec.width, spec.height)?;
    let said = studio.shoot(opts, ticks, spec)?;
    print!("{said}");
    Ok(())
}

/// A renderer kept for shot after shot (`--shot-server` holds one between
/// requests), and the frame clock the views share.
pub(crate) struct Studio {
    renderer: Renderer,
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    overlay: Overlay,
    time: f32,
    /// The staged props' wear (`staged_props`).
    hurt: i16,
}

impl Studio {
    pub(crate) fn new(
        map: Arc<MapFile>,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        width: u32,
        height: u32,
    ) -> Result<Studio, String> {
        Studio::with_hurt(map, blueprints, pool, width, height, 0)
    }

    /// A studio whose staged props (`staged_props`) have `hurt` permille of their
    /// health gone.
    fn with_hurt(
        map: Arc<MapFile>,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        width: u32,
        height: u32,
        hurt: i16,
    ) -> Result<Studio, String> {
        let mut renderer = Renderer::new_staged(
            Target::Headless { width, height },
            SceneDesc {
                map: map.clone(),
                blueprints: blueprints.clone(),
                pool: pool.clone(),
                team_colors: setup::TEAM_COLORS,
            },
            &staged_props(&map, hurt),
        )
        .map_err(|e| e.to_string())?;
        renderer.set_map_look(&setup::map_config(&map).look());
        Ok(Studio {
            renderer,
            map,
            blueprints,
            pool,
            overlay: Overlay::default(),
            time: 10.0,
            hurt,
        })
    }

    /// Stages the range for `opts` and `ticks` and draws `spec`; says what it drew.
    pub(crate) fn shoot(
        &mut self,
        opts: &Options,
        ticks: u32,
        spec: &Spec,
    ) -> Result<String, String> {
        use std::fmt::Write as _;
        let started = std::time::Instant::now();
        // A prop is staged on the range's default subject's pad, in its place.
        let prop = prop_kind(&opts.subject);
        let staged;
        let (key, opts) = match prop {
            Some(_) => {
                if spec.frames > 0 {
                    return Err(format!(
                        "{}: a prop stands still: no --frames",
                        opts.subject
                    ));
                }
                // The staged props wear the shot's `--hurt`: a studio staged with
                // another is built again.
                if self.hurt != opts.hurt {
                    *self = Studio::with_hurt(
                        self.map.clone(),
                        self.blueprints.clone(),
                        self.pool.clone(),
                        spec.width,
                        spec.height,
                        opts.hurt,
                    )?;
                }
                staged = Options {
                    subject: crate::range::DEFAULT_SUBJECT.into(),
                    hurt: 0,
                    ..opts.clone()
                };
                (opts.subject.as_str(), &staged)
            }
            None => (opts.subject.as_str(), opts),
        };
        // The range puts its subject down on the first ticks.
        let mut world = run_sim(
            opts,
            &self.map,
            &self.blueprints,
            &self.pool,
            ticks.max(5),
            false,
            None,
        )?;
        let mut frame = RenderFrame::default();
        world.write_render_frame(None, &mut frame);
        let mut subject = find_subject(&world, &frame, &opts.subject)?;
        if let Some(kind) = prop {
            subject = stage_prop(self, &mut frame, kind, key, subject)?;
        }
        self.renderer
            .resize(spec.width, spec.height)
            .map_err(|e| e.to_string())?;
        let mut camera = Camera::new(
            glam::Vec2::from(self.map.info().size_metres().to_f32()),
            glam::Vec2::new(spec.width as f32, spec.height as f32),
        );
        let mut said = format!(
            "{} (radius {:.1} m) staged in {:.1} s\n",
            key,
            subject.radius,
            started.elapsed().as_secs_f32()
        );
        if spec.frames > 0 {
            animate(self, &mut camera, &mut world, &mut frame, opts, spec)?;
        } else {
            said += &sheet(self, &mut camera, &frame, &subject, spec)?;
        }
        let _ = writeln!(
            said,
            "wrote {} in {:.1} s",
            spec.path,
            started.elapsed().as_secs_f32()
        );
        Ok(said)
    }
}

impl Studio {
    /// Draws `frames` frames of `camera`; the first hands over `sim`.
    fn draw(
        &mut self,
        camera: &Camera,
        sim: Option<&RenderFrame>,
        frames: u32,
        alpha: f32,
    ) -> Result<(), String> {
        for i in 0..frames {
            let input = FrameInput {
                camera,
                time: self.time,
                alpha,
                sim: if i == 0 { sim } else { None },
                ghosts: &[],
                marks: &[],
                ranges: &[],
                ranges_drawn: 0,
                overlay: &self.overlay,
                build_grid: false,
                icons: true,
            };
            self.renderer.render(&input).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn pixels(&mut self) -> Result<Vec<u8>, String> {
        self.renderer
            .read_pixels()
            .ok_or_else(|| "no pixels from a headless target".to_owned())
    }
}

/// The map prop kind drawn with model `key`: a prop shot. None for a unit's key.
pub(crate) fn prop_kind(key: &str) -> Option<PropKind> {
    PropKind::ALL
        .iter()
        .copied()
        .find(|k| mc_render::models::prop_model_key(k.raw()) == key)
}

/// Frames prop `kind` (model `key`), staged on the pad by the studio (`Studio::new`):
/// every unit on the range taken away, every other staged prop hidden. The prop's
/// front is its street side (local +y).
fn stage_prop(
    studio: &Studio,
    frame: &mut RenderFrame,
    kind: PropKind,
    key: &str,
    at: UnitInstance,
) -> Result<UnitInstance, String> {
    let model =
        mc_render::models::build_model(key).ok_or_else(|| format!("{key}: no model is made"))?;
    let first = studio.map.props().len();
    let subject = PropKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
    frame
        .props_dead
        .resize((first + PropKind::ALL.len()).div_ceil(32), 0);
    for i in (0..PropKind::ALL.len()).filter(|&i| i != subject) {
        let bit = first + i;
        frame.props_dead[bit / 32] |= 1 << (bit % 32);
    }
    frame.units.clear();
    let xy = pad(&studio.map);
    let pos = [xy.x, xy.y, studio.renderer.ground_height(xy)];
    Ok(UnitInstance {
        pos,
        prev_pos: pos,
        heading: std::f32::consts::FRAC_PI_2,
        prev_heading: std::f32::consts::FRAC_PI_2,
        radius: model.bounds_radius,
        ..at
    })
}

/// Where a prop shot's props stand: the first start position.
fn pad(map: &MapFile) -> glam::Vec2 {
    map.start_positions()
        .first()
        .map_or(glam::Vec2::ZERO, |p| glam::Vec2::from(p.to_f32()))
}

/// One of every map prop, for prop shots: on the pad, `hurt` (permille) of each one's
/// health gone.
fn staged_props(map: &MapFile, hurt: i16) -> Vec<Prop> {
    let at = map
        .start_positions()
        .first()
        .copied()
        .unwrap_or(mc_core::FxVec2::ZERO);
    PropKind::ALL
        .iter()
        .map(|&kind| Prop {
            kind,
            pos: at,
            heading: mc_core::Angle::ZERO,
            scale_milli: 1000,
            wear_milli: hurt.clamp(0, 1000) as u16,
        })
        .collect()
}

/// The unit to frame; once it is destroyed (`--scenario destruct`), its settled wreck,
/// every piece of it when it broke up.
fn find_subject(world: &World, frame: &RenderFrame, key: &str) -> Result<UnitInstance, String> {
    let is_key = |u: &UnitInstance| {
        world
            .blueprints
            .unit(mc_data::BlueprintId(u.blueprint as u16))
            .key
            == key
    };
    if let Some(u) = frame
        .units
        .iter()
        .find(|u| u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0 && is_key(u))
    {
        return Ok(*u);
    }
    let settled = |u: &&UnitInstance| {
        u.owner_flags & KIND_WRECK != 0 && u.packed == 0 && u.refit_modules & WRECK_INNER == 0
    };
    let first = frame
        .units
        .iter()
        .filter(settled)
        .find(|u| is_key(u))
        .copied()
        .ok_or_else(|| format!("{key} is not on the range to shoot (not built?)"))?;
    let pieces: Vec<&UnitInstance> = frame
        .units
        .iter()
        .filter(settled)
        .filter(|u| u.unit_id == first.unit_id)
        .collect();
    let mid = pieces
        .iter()
        .map(|u| glam::Vec3::from(u.pos))
        .sum::<glam::Vec3>()
        / pieces.len() as f32;
    let spread = pieces
        .iter()
        .map(|u| (glam::Vec3::from(u.pos) - mid).length())
        .fold(0.0, f32::max);
    Ok(UnitInstance {
        pos: mid.into(),
        radius: first.radius + spread,
        ..first
    })
}

/// Points `camera` at `subject` from `angle`, framing its whole body or, with
/// `--look`, a piece of it.
fn frame_on(
    camera: &mut Camera,
    renderer: &Renderer,
    subject: &UnitInstance,
    spec: &Spec,
    angle: Angle,
) {
    let pos = glam::Vec3::from(subject.pos);
    let (sin, cos) = subject.heading.sin_cos();
    let (forward, left) = (glam::Vec2::new(cos, sin), glam::Vec2::new(-sin, cos));
    let radius = subject.radius.max(1.0);
    camera.focus = match spec.look {
        Some([x, y, z]) => pos + (forward * x + left * y).extend(z),
        None => pos + glam::Vec3::Z * radius * 0.45,
    };
    // Half the frame's height holds the body with a margin, or its piece zoomed in.
    let fit = radius * 1.25 / spec.zoom.max(0.05);
    let aspect = (camera.viewport.x / camera.viewport.y.max(1.0)).min(1.0);
    camera.distance = (fit / ((camera.fov * 0.5).tan() * aspect)).max(2.0);
    let eye = glam::Vec2::from_angle(angle.bearing.to_radians()).rotate(forward);
    camera.yaw = (-eye.x).atan2(-eye.y);
    camera.pitch_free = Some(angle.elevation.to_radians());
    // A low eye stays above the ground: lift the look until it clears.
    let clear = renderer.ground_height(camera.eye().truncate()) + 0.8;
    let e = camera.eye();
    if e.z < clear {
        camera.focus.z += clear - e.z;
    }
}

/// Every view in turn, laid out three to a row in one PNG.
fn sheet(
    studio: &mut Studio,
    camera: &mut Camera,
    frame: &RenderFrame,
    subject: &UnitInstance,
    spec: &Spec,
) -> Result<String, String> {
    use std::fmt::Write as _;
    let mut said = String::new();
    let (w, h) = (spec.width as usize, spec.height as usize);
    let cols = spec.views.len().min(3);
    let rows = spec.views.len().div_ceil(3);
    let mut out = vec![0u8; cols * w * rows * h * 4];
    for (i, &angle) in spec.views.iter().enumerate() {
        frame_on(camera, &studio.renderer, subject, spec, angle);
        // The first view waits for terrain tiles, shadows and temporal passes to
        // settle; later ones only for the temporal passes.
        let settle = if i == 0 { 30 } else { 12 };
        studio.draw(camera, (i == 0).then_some(frame), settle, 1.0)?;
        let pixels = studio.pixels()?;
        let (cx, cy) = (i % 3, i / 3);
        for row in 0..h {
            let from = row * w * 4;
            let to = ((cy * h + row) * cols * w + cx * w) * 4;
            out[to..to + w * 4].copy_from_slice(&pixels[from..from + w * 4]);
        }
        let _ = writeln!(
            said,
            "view {} (row {}, column {}): bearing {}, elevation {}",
            i + 1,
            cy + 1,
            cx + 1,
            angle.bearing,
            angle.elevation
        );
    }
    write_png(
        Path::new(&spec.path),
        (cols * w) as u32,
        (rows * h) as u32,
        &out,
    )?;
    Ok(said)
}

/// The sim played on through the renderer, two frames a tick, the camera turning
/// about the unit from the first view; written as an animated PNG.
fn animate(
    studio: &mut Studio,
    camera: &mut Camera,
    world: &mut World,
    frame: &mut RenderFrame,
    opts: &Options,
    spec: &Spec,
) -> Result<(), String> {
    let first = spec.views[0];
    let file = std::fs::File::create(&spec.path).map_err(|e| format!("{}: {e}", spec.path))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), spec.width, spec.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(spec.frames, 0)
        .and_then(|()| encoder.set_frame_delay(1, 20))
        .map_err(|e| e.to_string())?;
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    let tick_s = 1.0 / mc_core::TICKS_PER_SECOND as f32;
    // The range's own clock: what it still has to do goes in on its tick.
    let mut t = world.state.tick;
    for i in 0..spec.frames {
        // A new tick every other frame; the one between is drawn half way through it.
        let half = i % 2 == 0;
        if half && i > 0 {
            let late = crate::setup::late_orders(opts, world, t);
            world.tick(&late).map_err(|e| e.to_string())?;
            t += 1;
            world.write_render_frame(None, frame);
        }
        let alpha = if half { 0.5 } else { 1.0 };
        // Framed where the unit is drawn, part way through the tick, or a moving unit
        // shakes against the camera every other frame.
        let mut subject = find_subject(world, frame, &opts.subject)?;
        subject.pos = glam::Vec3::from(subject.prev_pos)
            .lerp(glam::Vec3::from(subject.pos), alpha)
            .to_array();
        let turn = (subject.heading - subject.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        subject.heading = subject.prev_heading + turn * alpha;
        let angle = Angle {
            bearing: first.bearing + spec.turn * i as f32 / spec.frames.max(1) as f32,
            ..first
        };
        frame_on(camera, &studio.renderer, &subject, spec, angle);
        studio.time = 10.0 + (i / 2) as f32 * tick_s + alpha * tick_s;
        let settle = if i == 0 { 30 } else { 1 };
        studio.draw(camera, half.then_some(&*frame), settle, alpha)?;
        writer
            .write_image_data(&studio.pixels()?)
            .map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_parse_by_name_and_by_angle() {
        let v = parse_views("front, 90:30").unwrap();
        assert_eq!(
            v[0],
            Angle {
                bearing: 0.0,
                elevation: 8.0
            }
        );
        assert_eq!(
            v[1],
            Angle {
                bearing: 90.0,
                elevation: 30.0
            }
        );
        assert_eq!(parse_views(SHEET).unwrap().len(), 6);
        assert!(parse_views("sideways").is_err());
        assert!(parse_views("").is_err());
    }
}
