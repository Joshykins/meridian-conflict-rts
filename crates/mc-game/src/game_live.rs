//! The live desk in the test range (`crate::live`): each request dropped in the
//! mailbox is done here, between frames, and answered.
//!
//! Laying goes through the same `prefabs::layout` and `orders::site_verdict` the
//! player's prefab placement does, so what fits for the desk fits for the player.
//! Finished structures and sites are debug spawns; `how: "build"` spawns engineers
//! and gives them `Command::Build` orders once they stand, exactly as placing the
//! prefab with them would.

use super::Game;
use crate::hud;
use crate::live::{self, At, Grow, How, KeptPrefab, Lay, Mailbox, Request, Spot};
use crate::orders::{self, Field};
use crate::prefabs::{self, Laid, Prefab, PrefabId, Turn};
use crate::range;
use glam::Vec2;
use mc_core::{Angle, Fx, FxVec2};
use mc_data::BlueprintId;
use mc_render::Renderer;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};
use mc_sim::{Command, Handle};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// A screenshot not back by then is given up.
const SHOT_WAIT: Duration = Duration::from_secs(10);
/// Builders that have not all turned up by then start with those that have.
const CREW_WAIT: Duration = Duration::from_secs(3);

/// The mailbox, once the range has opened it, and the work a request left owing.
#[derive(Default)]
pub struct Live {
    mailbox: Option<Mailbox>,
    /// The mailbox was tried: a folder that cannot be made is not tried every frame.
    tried: bool,
    crew: Option<Crew>,
    shot: Option<Shot>,
}

/// Builders spawned for a `lay` to build, and the orders they are owed.
struct Crew {
    builder: BlueprintId,
    owner: u8,
    count: usize,
    /// Units already standing when they were spawned.
    known: HashSet<u32>,
    orders: Vec<Laid>,
    since: Instant,
}

/// A screenshot asked for: the request it answers, when, and whether the renderer
/// was asked yet.
struct Shot {
    id: String,
    since: Instant,
    asked: bool,
}

fn round(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn standing(u: &UnitInstance) -> bool {
    u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) == 0
}

fn refuse(why: impl Into<String>) -> Value {
    json!({ "ok": false, "error": why.into() })
}

impl Game {
    /// Once a frame, in the test range only: answers what waits in the mailbox.
    pub(super) fn live_frame(&mut self, renderer: &mut Renderer, fresh: bool) {
        if self.view.range.is_none() {
            return;
        }
        let now = Instant::now();
        if !self.live.tried {
            self.live.tried = true;
            self.live.mailbox = Mailbox::open();
        }
        let Some(mailbox) = self.live.mailbox.as_mut() else {
            return;
        };
        mailbox.beat(
            now,
            &json!({ "map": self.map.name(), "tick": self.view.status.tick }),
        );
        if fresh {
            self.live_crew(now);
        }
        // A screenshot holds the queue: requests are answered in order.
        if self.live.shot.is_some() {
            self.live_shot(renderer, now);
            return;
        }
        let Some((id, request)) = self.live.mailbox.as_mut().and_then(|m| m.next(now)) else {
            return;
        };
        log::info!("live desk: request {id}");
        let reply = match request {
            Err(why) => Some(refuse(why)),
            Ok(request) => self.live_request(&id, request, renderer, now),
        };
        if let (Some(reply), Some(mailbox)) = (reply, &self.live.mailbox) {
            mailbox.answer(&id, &reply);
        }
    }

    /// Does `request`; its answer, or `None` when it is answered later (a screenshot).
    fn live_request(
        &mut self,
        id: &str,
        request: Request,
        renderer: &Renderer,
        now: Instant,
    ) -> Option<Value> {
        Some(match request {
            Request::State { units } => self.live_state(units, renderer),
            Request::Lay(lay) => self.live_lay(lay, renderer, now),
            Request::Capture { ids } => self.live_capture(&ids),
            Request::Keep { prefab } => self.live_keep(prefab),
            Request::Remove { ids, area } => self.live_remove(ids, area),
            Request::Clear => {
                self.live.crew = None;
                self.send(Command::DebugClear);
                json!({ "ok": true })
            }
            Request::Camera { at, zoom, yaw } => self.live_camera(at, zoom, yaw, renderer),
            Request::Grow(grow) => match self.live_grow(grow, renderer) {
                Ok(v) => v,
                Err(why) => refuse(why),
            },
            Request::Shot => {
                self.live.shot = Some(Shot {
                    id: id.to_owned(),
                    since: now,
                    asked: false,
                });
                return None;
            }
        })
    }

    fn live_field<'a>(&'a self, renderer: &'a Renderer) -> Field<'a> {
        Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer,
        }
    }

    /// The ground `at` names, inside the map.
    fn live_point(&self, at: At, renderer: &Renderer) -> Result<Vec2, String> {
        let p = match at {
            At::Point(p) => Vec2::from(p),
            At::Spot(Spot::Camera) => self.camera.focus.truncate(),
            At::Spot(Spot::Pad) => match &self.view.range {
                Some(r) => Vec2::from(r.pad.to_f32()),
                None => return Err("no range".into()),
            },
            At::Spot(Spot::Cursor) => {
                orders::surface_under(&self.live_field(renderer), self.cursor)
                    .ok_or("the pointer is not over the ground")?
                    .truncate()
            }
        };
        let size = Vec2::from(self.map.info().size_metres().to_f32());
        if !p.is_finite() || p.x < 0.0 || p.y < 0.0 || p.x > size.x || p.y > size.y {
            return Err(format!(
                "({}, {}) is off the map ({} x {} m)",
                p.x, p.y, size.x, size.y
            ));
        }
        Ok(p)
    }

    fn live_state(&self, units: bool, renderer: &Renderer) -> Value {
        let b = &self.blueprints;
        let mut structures = Vec::new();
        let mut mobile = Vec::new();
        for u in self.view.frame.units.iter().filter(|u| standing(u)) {
            let bp = b.unit(BlueprintId(u.blueprint as u16));
            let selected = self.view.selection.contains(&u.unit_id);
            if bp.is_mobile() {
                if units {
                    mobile.push(json!({
                        "id": u.unit_id, "key": bp.key, "owner": u.owner_flags & 0xFF,
                        "x": round(u.pos[0]), "y": round(u.pos[1]), "selected": selected,
                    }));
                }
                continue;
            }
            structures.push(json!({
                "id": u.unit_id, "key": bp.key, "owner": u.owner_flags & 0xFF,
                "x": round(u.pos[0]), "y": round(u.pos[1]),
                "turn": bp.lot_turn(Angle::from_radians_f32(u.heading)),
                "build": round(u.build), "selected": selected,
            }));
        }
        let cursor = orders::surface_under(&self.live_field(renderer), self.cursor)
            .map(|g| [round(g.x), round(g.y)]);
        let size = self.map.info().size_metres().to_f32();
        let pad = self.view.range.as_ref().map(|r| r.pad.to_f32());
        let library: Vec<Value> = self
            .prefabs
            .library
            .iter()
            .map(|p| json!({ "name": p.name, "members": p.members }))
            .collect();
        let players: Vec<Value> = self
            .view
            .status
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| json!({ "slot": i, "name": p.name, "team": p.team }))
            .collect();
        let mut out = json!({
            "ok": true,
            "map": self.map.name(),
            "size": size,
            "tick": self.view.status.tick,
            "pad": pad,
            "local": self.view.local,
            "players": players,
            "camera": {
                "at": [round(self.camera.focus.x), round(self.camera.focus.y)],
                "zoom": round(self.camera.distance),
                "yaw": round(self.camera.yaw.to_degrees()),
            },
            "cursor": cursor,
            "structures": structures,
            "prefabs": library,
        });
        if units {
            out["units"] = Value::Array(mobile);
        }
        out
    }

    fn live_lay(&mut self, lay: Lay, renderer: &Renderer, now: Instant) -> Value {
        match self.try_live_lay(lay, renderer, now) {
            Ok(v) => v,
            Err(why) => refuse(why),
        }
    }

    fn try_live_lay(
        &mut self,
        mut lay: Lay,
        renderer: &Renderer,
        now: Instant,
    ) -> Result<Value, String> {
        let b = self.blueprints.clone();
        let prefab = match (&lay.name, lay.structures.take()) {
            (Some(name), None) => self
                .prefabs
                .library
                .by_name(name)
                .cloned()
                .ok_or_else(|| format!("no prefab named {name:?} in the library"))?,
            (None, Some(members)) => Prefab {
                name: "live".into(),
                members,
                category: None,
                icon: None,
                id: PrefabId::default(),
            },
            _ => return Err("give a prefab's `name` or `structures`, not both".into()),
        };
        prefab.check(live::MAX_LAID).map_err(|why| {
            format!(
                "not laid: {why} (at most {} structures, each within 2048 m)",
                live::MAX_LAID
            )
        })?;
        let unknown: Vec<&str> = prefab
            .members
            .iter()
            .filter(|m| b.id_of(&m.key).is_none_or(|id| !b.unit(id).built_on_site()))
            .map(|m| m.key.as_str())
            .collect();
        if !unknown.is_empty() {
            return Err(format!("not structures this build has: {unknown:?}"));
        }
        let turn = match &lay.turn {
            Some(t) => {
                Turn::parse(t).ok_or_else(|| format!("turn {t:?}: give Q or Qm, Q 0 to 3"))?
            }
            None => Turn::default(),
        };
        if lay.owner as usize >= self.view.status.players.len() {
            return Err(format!("no player {}", lay.owner));
        }
        let anchor = self.live_point(lay.at, renderer)?;
        let anchor = FxVec2::new(Fx::from_f32(anchor.x), Fx::from_f32(anchor.y));
        let (members, _) = prefab.resolve(&b);
        let laid = prefabs::layout(&b, &members, anchor, turn);

        // Who builds it, and what they can.
        let builder = match (lay.how, &lay.builder) {
            (How::Build, Some(key)) => {
                let id = b.id_of(key).ok_or_else(|| format!("no unit {key:?}"))?;
                if !b.unit(id).is_mobile() || b.unit(id).builder.is_none() {
                    return Err(format!("{key} is not a mobile builder"));
                }
                Some(id)
            }
            (How::Build, None) => {
                let wanted: Vec<BlueprintId> = laid.iter().map(|l| l.blueprint).collect();
                Some(live::pick_builder(&b, &wanted).ok_or(
                    "no one engineer builds all of it: name a `builder`, or lay it in parts",
                )?)
            }
            _ => None,
        };
        if lay.how == How::Build && !(1..=live::MAX_BUILDERS).contains(&lay.builders) {
            return Err(format!("builders: 1 to {}", live::MAX_BUILDERS));
        }
        let builds: Option<&[BlueprintId]> = builder
            .and_then(|id| b.unit(id).builder.as_ref())
            .map(|bl| bl.builds.as_slice());

        // Each site by the player's own placement rules, the earlier ones taken.
        let field = self.live_field(renderer);
        let mut taken: Vec<Laid> = Vec::new();
        let mut sites = Vec::with_capacity(laid.len());
        for l in &laid {
            let xy = Vec2::from(l.at.to_f32());
            let ground = xy.extend(orders::surface_height(&field, xy));
            let fit = orders::site_verdict(&field, l.blueprint, ground, l.heading, None, &taken)
                .map_or(Err(mc_sim::placement::Unfit::OffMap), |(_, fit)| fit);
            let buildable = builds.is_none_or(|bs| bs.contains(&l.blueprint));
            let goes = fit.is_ok() && buildable;
            if goes {
                taken.push(*l);
            }
            let bp = b.unit(l.blueprint);
            sites.push(json!({
                "key": bp.key, "x": round(xy.x), "y": round(xy.y),
                "turn": bp.lot_turn(l.heading),
                "fits": match fit { Ok(()) => "yes".to_string(), Err(why) => why.label().to_string() },
                "buildable": buildable,
                "laid": goes && !lay.dry,
            }));
        }
        let bounds = prefabs::bounds(&b, &taken);
        let anchor = prefabs::snap_anchor(anchor).to_f32();
        let mut reply = json!({
            "ok": true,
            "anchor": anchor,
            "laid": if lay.dry { 0 } else { taken.len() },
            "fit": taken.len(),
            "of": laid.len(),
            "bounds": bounds,
            "sites": sites,
        });
        if lay.dry {
            return Ok(reply);
        }
        if let Some(builder) = builder.filter(|_| !taken.is_empty()) {
            reply["builder"] = json!(b.unit(builder).key);
        }
        self.live_put_down(&lay, builder, taken, now);
        Ok(reply)
    }

    /// What `lay` laid put down: finished, as sites, or a crew spawned to build it.
    fn live_put_down(
        &mut self,
        lay: &Lay,
        builder: Option<BlueprintId>,
        taken: Vec<Laid>,
        now: Instant,
    ) {
        if let Some(on) = lay.free_build {
            self.send(Command::DebugFreeBuild {
                player: lay.owner,
                on,
            });
        }
        match (lay.how, builder) {
            (How::Build, Some(builder)) if !taken.is_empty() => {
                // The crew stands south of the lots, clear of them.
                let (lo, hi) =
                    prefabs::bounds(&self.blueprints, &taken).expect("taken is not empty");
                let at = Vec2::new((lo[0] + hi[0]) * 0.5, lo[1] - 40.0);
                let size = Vec2::from(self.map.info().size_metres().to_f32());
                let at = at.clamp(Vec2::ZERO, size);
                self.live.crew = Some(Crew {
                    builder,
                    owner: lay.owner,
                    count: lay.builders as usize,
                    known: self.view.frame.units.iter().map(|u| u.unit_id).collect(),
                    orders: taken,
                    since: now,
                });
                self.send(Command::DebugSpawn {
                    owner: lay.owner,
                    blueprint: builder,
                    pos: FxVec2::new(Fx::from_f32(at.x), Fx::from_f32(at.y)),
                    heading: Angle::from_radians_f32(std::f32::consts::FRAC_PI_2),
                    count: lay.builders,
                    flags: 0,
                    build: 1000,
                });
            }
            (How::Build, _) => {}
            (how, _) => {
                for l in &taken {
                    self.send(Command::DebugSpawn {
                        owner: lay.owner,
                        blueprint: l.blueprint,
                        pos: l.at,
                        heading: l.heading,
                        count: 1,
                        flags: 0,
                        build: if how == How::Sites { 0 } else { 1000 },
                    });
                }
            }
        }
    }

    /// The crew a `lay` spawned, once it stands: its orders, in the order laid.
    fn live_crew(&mut self, now: Instant) {
        let Some(crew) = &self.live.crew else { return };
        let found: Vec<Handle> = self
            .view
            .frame
            .units
            .iter()
            .filter(|u| {
                standing(u)
                    && u.owner_flags & 0xFF == crew.owner as u32
                    && u.blueprint == crew.builder.0 as u32
                    && u.build >= 1.0
                    && !crew.known.contains(&u.unit_id)
            })
            .map(|u| Handle(u.unit_id))
            .collect();
        if found.len() < crew.count && (found.is_empty() || now - crew.since < CREW_WAIT) {
            return;
        }
        let crew = self.live.crew.take().expect("checked above");
        // Building is the owner's business, whichever side was being steered.
        self.send(Command::DebugControl { player: crew.owner });
        self.view.local = crew.owner;
        for (i, l) in crew.orders.iter().enumerate() {
            self.send(Command::Build {
                units: found.clone(),
                blueprint: l.blueprint,
                pos: l.at,
                heading: l.heading,
                queue: i > 0,
            });
        }
    }

    fn live_capture(&self, ids: &[u32]) -> Value {
        let structures: Vec<Laid> = if ids.is_empty() {
            hud::prefabs::selection_structures(&self.view, &self.prefabs.order)
        } else {
            self.view
                .frame
                .units
                .iter()
                .filter(|u| standing(u) && ids.contains(&u.unit_id))
                .map(|u| Laid {
                    blueprint: BlueprintId(u.blueprint as u16),
                    at: FxVec2::new(Fx::from_f32(u.pos[0]), Fx::from_f32(u.pos[1])),
                    heading: Angle::from_radians_f32(u.heading),
                })
                .collect()
        };
        match prefabs::capture(&self.blueprints, &structures) {
            Ok(p) => json!({ "ok": true, "prefab": { "name": p.name, "members": p.members } }),
            Err(why) => refuse(why.label()),
        }
    }

    fn live_keep(&mut self, kept: KeptPrefab) -> Value {
        let prefab = Prefab {
            name: kept.name,
            members: kept.members,
            category: None,
            icon: None,
            id: PrefabId::default(),
        };
        if let Err(why) = prefab.check(prefabs::MAX_MEMBERS) {
            return refuse(format!(
                "not kept: {why} (at most {} structures)",
                prefabs::MAX_MEMBERS
            ));
        }
        let (_, gone) = prefab.resolve(&self.blueprints);
        if gone > 0 {
            return refuse(format!(
                "not kept: {gone} members are not structures this build has"
            ));
        }
        let name = prefabs::clean_name(&prefab.name);
        let Some(kept) = self.prefabs.library.keep(prefab) else {
            return refuse(format!(
                "the library is full ({} prefabs)",
                prefabs::MAX_PREFABS
            ));
        };
        if let Err(e) = self.prefabs.library.save() {
            return refuse(format!("kept for this run, not saved: {e}"));
        }
        let replaced = matches!(kept, prefabs::Kept::Replaced(_));
        json!({ "ok": true, "name": name, "replaced": replaced })
    }

    fn live_remove(&mut self, ids: Vec<u32>, area: Option<[f32; 4]>) -> Value {
        let mut units: Vec<Handle> = ids.into_iter().map(Handle).collect();
        if let Some([x0, y0, x1, y1]) = area {
            let (lo, hi) = (
                Vec2::new(x0.min(x1), y0.min(y1)),
                Vec2::new(x0.max(x1), y0.max(y1)),
            );
            units.extend(
                self.view
                    .frame
                    .units
                    .iter()
                    .filter(|u| {
                        let p = Vec2::new(u.pos[0], u.pos[1]);
                        standing(u) && p.cmpge(lo).all() && p.cmple(hi).all()
                    })
                    .map(|u| Handle(u.unit_id)),
            );
        }
        let n = units.len();
        if n > 0 {
            self.send(Command::DebugRemove { units });
        }
        json!({ "ok": true, "removed": n })
    }

    fn live_camera(
        &mut self,
        at: Option<At>,
        zoom: Option<f32>,
        yaw: Option<f32>,
        renderer: &Renderer,
    ) -> Value {
        if let Some(at) = at {
            match self.live_point(at, renderer) {
                Ok(p) => {
                    self.track = None;
                    self.cancel_orbit_return();
                    self.camera.focus = p.extend(self.camera.focus.z);
                }
                Err(why) => return refuse(why),
            }
        }
        if let Some(z) = zoom.filter(|z| z.is_finite()) {
            self.zoom_target = Some(z.clamp(range::ZOOMS[0], self.camera.max_distance()));
            self.zoom_anchor = None;
        }
        if let Some(deg) = yaw.filter(|y| y.is_finite()) {
            self.camera.yaw = deg.to_radians();
        }
        json!({ "ok": true })
    }

    /// Asks for the next frame when no one else is waiting on one, and saves it when it
    /// comes back; the answer is written once the file is.
    fn live_shot(&mut self, renderer: &mut Renderer, now: Instant) {
        let Some(shot) = &mut self.live.shot else {
            return;
        };
        let Some(mailbox) = &self.live.mailbox else {
            return;
        };
        if now - shot.since > SHOT_WAIT {
            mailbox.answer(&shot.id, &refuse("no frame came back"));
            self.live.shot = None;
            return;
        }
        if !shot.asked {
            if !self.hud.issues.capturing() {
                renderer.capture_next_frame();
                shot.asked = true;
            }
            return;
        }
        let Some(result) = renderer.take_capture() else {
            return;
        };
        let shot = self.live.shot.take().expect("checked above");
        match result {
            Ok(frame) => {
                let path = mailbox.shot_path(&shot.id);
                let (w, h) = (frame.width, frame.height);
                let reply =
                    json!({ "ok": true, "shot": format!("{}.png", shot.id), "size": [w, h] });
                let id = shot.id;
                let answer = mailbox.outbox();
                std::thread::spawn(move || {
                    let reply = match crate::headless::write_png(&path, w, h, &frame.rgba) {
                        Ok(()) => reply,
                        Err(e) => refuse(format!("the screenshot was not saved: {e}")),
                    };
                    answer.answer(&id, &reply);
                });
            }
            Err(e) => mailbox.answer(&shot.id, &refuse(e)),
        }
    }

    /// Hands a side to the AI at a new start (or back to the player), and sets the pace.
    fn live_grow(&mut self, grow: Grow, renderer: &Renderer) -> Result<Value, String> {
        let side = grow.side;
        if side as usize >= self.view.status.players.len() {
            return Err(format!("there is no side {side}"));
        }
        if let Some(speed) = grow.speed {
            let step = hud::SPEEDS
                .iter()
                .copied()
                .min_by_key(|s| s.abs_diff(speed))
                .unwrap_or(100);
            self.set_speed(step);
        }
        let config = mc_sim::AiConfig {
            difficulty: mc_sim::Difficulty::Hard,
            ..Default::default()
        };
        if !grow.on {
            self.send(Command::DebugSetAi {
                player: side,
                on: false,
                config,
                commander: false,
            });
            return Ok(json!({ "ok": true, "side": side, "ai": false }));
        }
        let at = self.live_point(grow.at, renderer)?;
        let front = grow
            .front
            .map(|f| self.live_point(f, renderer))
            .transpose()?;
        if let Some(on) = grow.free_build {
            self.send(Command::DebugFreeBuild { player: side, on });
        }
        let fx = |p: Vec2| FxVec2::new(Fx::from_f32(p.x), Fx::from_f32(p.y));
        self.send(Command::DebugSetStart {
            player: side,
            pos: fx(at),
        });
        if let Some(front) = front {
            for other in 0..self.view.status.players.len() as u8 {
                if other != side {
                    self.send(Command::DebugSetStart {
                        player: other,
                        pos: fx(front),
                    });
                }
            }
        }
        self.send(Command::DebugSetAi {
            player: side,
            on: true,
            config,
            commander: grow.commander,
        });
        // The player keeps the other side: commands of theirs given as the AI's side
        // would fight it for its units.
        if self.view.local == side {
            self.take_control(if side == range::BLUE {
                range::RED
            } else {
                range::BLUE
            });
        }
        Ok(json!({
            "ok": true, "side": side, "ai": true,
            "at": [round(at.x), round(at.y)],
            "front": front.map(|f| [round(f.x), round(f.y)]),
            "speed": self.view.speed,
        }))
    }
}
