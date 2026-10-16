//! Map pings: a mark a player puts on the ground (or the minimap) for their allies,
//! with a call each kind is known by. A ping is the interface's, not the match's:
//! it goes to the allies through the relay (`Message::MapPing`) and never reaches
//! the simulation or a replay.
//!
//! On the ground a ping lands as rings rushing out from the point, then stands as a
//! spinning dashed ring under a badge on a stalk with who placed it and why; off the
//! screen it is an arrow at the edge pointing to it. The minimap ripples where it is
//! (`hud/pings.rs`), and Space looks at the newest.

use crate::audio::Audio;
use crate::hud::icons::{arrow_head, glyph as order_glyph, Glyph};
use crate::nuke_marks::{ground_ring, project, surface, tag};
use crate::orders::Field;
use crate::ui::{ink, rgb, Color, Ui};
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

/// What a ping says. The numbers go over the wire: never renumber one, never reuse
/// a retired one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PingKind {
    /// Look here.
    Look = 0,
    Attack = 1,
    Defend = 2,
    Danger = 3,
    OnMyWay = 4,
    /// A ping with words: the player types them after placing it.
    Message = 5,
}

impl PingKind {
    /// In the order of their buttons and keys (1 to 6 with a ping in hand).
    pub const ALL: [PingKind; 6] = [
        PingKind::Look,
        PingKind::Attack,
        PingKind::Defend,
        PingKind::Danger,
        PingKind::OnMyWay,
        PingKind::Message,
    ];

    pub fn from_wire(n: u8) -> Option<PingKind> {
        PingKind::ALL.into_iter().find(|k| *k as u8 == n)
    }

    pub fn label(self) -> &'static str {
        match self {
            PingKind::Look => "Look Here",
            PingKind::Attack => "Attack",
            PingKind::Defend => "Defend",
            PingKind::Danger => "Danger",
            PingKind::OnMyWay => "On My Way",
            PingKind::Message => "Message",
        }
    }

    pub fn tone(self) -> u32 {
        match self {
            PingKind::Look => 0x5CD6FF,
            PingKind::Attack => 0xFF4646,
            PingKind::Defend => 0x6E96FF,
            PingKind::Danger => 0xFFB43C,
            PingKind::OnMyWay => 0x62F08C,
            PingKind::Message => 0xD8C4FF,
        }
    }

    /// Its call in the sound library (`data/sounds/pings.ron`).
    pub fn sound(self) -> &'static str {
        match self {
            PingKind::Look => "ping_look",
            PingKind::Attack => "ping_attack",
            PingKind::Defend => "ping_defend",
            PingKind::Danger => "ping_danger",
            PingKind::OnMyWay => "ping_on_my_way",
            PingKind::Message => "ping_message",
        }
    }

    /// The digit that picks it with a ping in hand.
    pub fn key(self) -> usize {
        self as usize + 1
    }
}

/// How long a ping stands, and one with words.
const LIFE: f32 = 7.0;
const LIFE_TEXT: f32 = 12.0;
/// The last of its life, fading.
const FADE: f32 = 1.2;
/// Pings this machine may place in a burst, and seconds for each one more: under the
/// relay's chat allowance (five, one a second), which pings share.
const BURST: f32 = 4.0;
const REFILL: f32 = 1.25;
/// Pings standing at once. A cosmetic cap: past it the oldest goes, so a team pinging
/// hard keeps its newest marks.
const MAX_LIVE: usize = 24;

pub struct Ping {
    pub kind: PingKind,
    pub at: Vec2,
    /// The slot that placed it, for its colour and name.
    pub from: u8,
    pub name: String,
    pub text: String,
    pub age: f32,
}

impl Ping {
    fn life(&self) -> f32 {
        if self.text.is_empty() {
            LIFE
        } else {
            LIFE_TEXT
        }
    }

    /// Its opacity: whole, then fading at the end of its life.
    pub fn fade(&self) -> f32 {
        ((self.life() - self.age) / FADE).clamp(0.0, 1.0)
    }
}

/// A message ping placed and waiting for its words.
pub struct Draft {
    pub at: Vec2,
    pub text: String,
}

pub struct Pings {
    live: Vec<Ping>,
    /// Pings that may be placed now (`BURST` at most).
    allowance: f32,
    pub draft: Option<Draft>,
    /// Where the newest ping is, for Space.
    pub newest: Option<Vec2>,
}

impl Default for Pings {
    fn default() -> Pings {
        Pings {
            live: Vec::new(),
            allowance: BURST,
            draft: None,
            newest: None,
        }
    }
}

impl Pings {
    pub fn live(&self) -> &[Ping] {
        &self.live
    }

    /// A message ping's words are being typed: the match's keys must leave them alone.
    pub fn typing(&self) -> bool {
        self.draft.is_some()
    }

    /// Ages every ping, lets the old go and refills the allowance.
    pub fn tick(&mut self, dt: f32) {
        for p in &mut self.live {
            p.age += dt;
        }
        self.live.retain(|p| p.age < p.life());
        self.allowance = (self.allowance + dt / REFILL).min(BURST);
    }

    /// Takes one from the allowance: false when this machine is pinging too fast.
    pub fn spend(&mut self) -> bool {
        if self.allowance < 1.0 {
            return false;
        }
        self.allowance -= 1.0;
        true
    }

    /// Puts a ping up, this machine's or an ally's, and sounds its call.
    pub fn add(&mut self, ping: Ping, audio: &Audio) {
        let (library, _) = audio.library();
        if let Some(id) = library.id_of(ping.kind.sound()) {
            audio.play_response(id, 0.8);
        }
        self.newest = Some(ping.at);
        if self.live.len() >= MAX_LIVE {
            self.live.remove(0);
        }
        self.live.push(ping);
    }

    /// A screenshot's pings (`MERIDIAN_PINGS`): one of each kind `spread` metres round
    /// `focus`, landed at different moments, and one far off, off the screen.
    pub fn stage(&mut self, focus: Vec2, spread: f32, name: &str) {
        for (i, kind) in PingKind::ALL.into_iter().enumerate() {
            let a = i as f32 / PingKind::ALL.len() as f32 * TAU;
            let text = if kind == PingKind::Message {
                "AA is up on this ridge".to_owned()
            } else {
                String::new()
            };
            self.live.push(Ping {
                kind,
                at: focus + Vec2::from_angle(a) * spread,
                from: (i % 2) as u8,
                name: name.to_owned(),
                text,
                age: 0.15 + i as f32 * 0.5,
            });
        }
        self.live.push(Ping {
            kind: PingKind::Danger,
            at: focus + Vec2::new(-spread * 12.0, spread),
            from: 1,
            name: name.to_owned(),
            text: String::new(),
            age: 1.0,
        });
    }
}

/// Every ping on the ground, or at the screen's edge pointing to it.
pub fn draw(ui: &mut Ui, field: &Field, pings: &Pings) {
    for p in &pings.live {
        let fade = ui.fade;
        ui.fade *= p.fade();
        mark(ui, field, p);
        ui.fade = fade;
    }
}

/// Room kept clear at the screen's edges for the panels, in points: left, top,
/// right, bottom.
const EDGES: [f32; 4] = [48.0, 96.0, 48.0, 210.0];

fn mark(ui: &mut Ui, field: &Field, p: &Ping) {
    let tone = p.kind.tone();
    let ground = p.at.extend(surface(field, p.at) + 1.0);
    let Some(c) = project(ui, field, ground) else {
        return;
    };
    let inside = c.x > EDGES[0]
        && c.y > EDGES[1]
        && c.x < ui.size.x - EDGES[2]
        && c.y < ui.size.y - EDGES[3];
    if !inside {
        return edge_arrow(ui, c, p);
    }
    // Points on the screen to a metre on the ground there: the rings keep their size
    // on the screen however far the camera is.
    let per_metre = project(ui, field, ground + Vec3::X * 10.0)
        .map_or(1.0, |q| q.distance(c) / 10.0)
        .max(1e-3);
    let ring = |ui: &mut Ui, points: f32, width: f32, color: Color, dashed: bool, spin: f32| {
        ground_ring(
            ui,
            field,
            p.at,
            points / per_metre,
            width,
            color,
            dashed,
            spin,
        );
    };
    // Landing: three rings rushing out from the point.
    for i in 0..3 {
        let t = (p.age - i as f32 * 0.2) / 1.1;
        if (0.0..1.0).contains(&t) {
            let out = 1.0 - (1.0 - t).powi(3);
            ring(
                ui,
                10.0 + 80.0 * out,
                3.0 - 2.0 * t,
                rgb(tone, 0.9 * (1.0 - t)),
                false,
                0.0,
            );
        }
    }
    // Standing: a slow ripple every two seconds, under a spinning dashed ring.
    let beat = (p.age - 1.0).max(0.0) % 2.0 / 2.0;
    if p.age > 1.0 {
        ring(
            ui,
            20.0 + 30.0 * beat,
            1.5,
            rgb(tone, 0.5 * (1.0 - beat)),
            false,
            0.0,
        );
    }
    ring(ui, 20.0, 2.0, rgb(tone, 0.85), true, ui.time * 1.4);
    ui.disc(c, 3.5, rgb(tone, 1.0));

    // The badge drops in on its stalk and pops.
    let drop = (1.0 - (p.age / 0.25).min(1.0)).powi(2) * 36.0;
    let pop = 1.0 + 0.3 * (1.0 - p.age / 0.35).max(0.0);
    let top = c - Vec2::new(0.0, 54.0 + drop);
    let r = 15.0 * pop;
    // The stalk in the colour of whoever placed it.
    let who = field.view.colors[p.from as usize % mc_core::MAX_PLAYERS];
    ui.stroke(
        c,
        top + Vec2::new(0.0, r),
        2.5,
        [who[0], who[1], who[2], 0.9],
    );
    badge(ui, p.kind, top, r);
    tag(ui, top - Vec2::new(0.0, r + 14.0), &label(p), tone);
}

/// What a ping's tag says: who, and why or their words.
fn label(p: &Ping) -> String {
    if p.text.is_empty() {
        format!("{}  \u{b7}  {}", p.name, p.kind.label())
    } else {
        format!("{}: {}", p.name, p.text)
    }
}

/// A ping off the screen: its badge at the edge, an arrow pointing out to it.
fn edge_arrow(ui: &mut Ui, c: Vec2, p: &Ping) {
    let lo = Vec2::new(EDGES[0], EDGES[1]);
    let hi = ui.size - Vec2::new(EDGES[2], EDGES[3]);
    let centre = (lo + hi) * 0.5;
    let dir = (c - centre).normalize_or_zero();
    if dir == Vec2::ZERO {
        return;
    }
    let half = (hi - lo) * 0.5;
    let reach = (half.x / dir.x.abs().max(1e-4)).min(half.y / dir.y.abs().max(1e-4));
    let at = centre + dir * reach;
    // It calls for a look while it is new.
    let pulse = if p.age < 3.0 {
        0.7 + 0.3 * (p.age * 9.0).sin().abs()
    } else {
        0.8
    };
    let tone = p.kind.tone();
    badge(ui, p.kind, at, 13.0);
    arrow_head(ui, at + dir * 25.0, dir, 8.0, 3.0, rgb(tone, pulse));
}

/// A ping's badge: its glyph on a dark disc in a ring of its colour.
pub fn badge(ui: &mut Ui, kind: PingKind, c: Vec2, r: f32) {
    let color = rgb(kind.tone(), 1.0);
    ui.disc(c, r, ink(0.82));
    ui.arc(c, r, 0.0, TAU, 2.0, color);
    glyph(ui, kind, c, r * 0.6, color);
}

/// A ping kind's glyph, `r` round `c`.
pub fn glyph(ui: &mut Ui, kind: PingKind, c: Vec2, r: f32, color: Color) {
    let t = (r * 0.18).max(1.4);
    let at = |x: f32, y: f32| c + Vec2::new(x, y) * r;
    match kind {
        // A map pin.
        PingKind::Look => {
            let head = at(0.0, -0.3);
            ui.arc(head, r * 0.5, 0.0, TAU, t, color);
            ui.disc(head, t * 0.9, color);
            ui.stroke(at(-0.42, -0.04), at(0.0, 0.95), t, color);
            ui.stroke(at(0.42, -0.04), at(0.0, 0.95), t, color);
        }
        PingKind::Attack => order_glyph(ui, Glyph::Attack, c, r, color),
        // A shield.
        PingKind::Defend => {
            let rim = [
                at(-0.75, -0.8),
                at(0.75, -0.8),
                at(0.75, 0.05),
                at(0.0, 0.95),
                at(-0.75, 0.05),
            ];
            ui.polyline(&rim, t, color, true);
            ui.stroke(at(0.0, -0.5), at(0.0, 0.55), t, color);
        }
        // A warning sign.
        PingKind::Danger => {
            ui.polyline(
                &[at(0.0, -0.9), at(0.95, 0.75), at(-0.95, 0.75)],
                t,
                color,
                true,
            );
            ui.stroke(at(0.0, -0.35), at(0.0, 0.2), t, color);
            ui.disc(at(0.0, 0.48), t * 0.8, color);
        }
        // Two chevrons on the move.
        PingKind::OnMyWay => {
            arrow_head(ui, at(0.15, 0.0), Vec2::X, r * 0.6, t, color);
            arrow_head(ui, at(0.85, 0.0), Vec2::X, r * 0.6, t, color);
        }
        // A speech bubble with two lines in it.
        PingKind::Message => {
            let bubble = [
                at(-0.85, -0.7),
                at(0.85, -0.7),
                at(0.85, 0.35),
                at(-0.1, 0.35),
                at(-0.5, 0.8),
                at(-0.45, 0.35),
                at(-0.85, 0.35),
            ];
            ui.polyline(&bubble, t, color, true);
            ui.stroke(at(-0.5, -0.32), at(0.5, -0.32), t * 0.8, color);
            ui.stroke(at(-0.5, 0.0), at(0.25, 0.0), t * 0.8, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Bank;
    use mc_data::sounds::SoundLibrary;

    #[test]
    fn kinds_go_over_the_wire_and_back() {
        for kind in PingKind::ALL {
            assert_eq!(PingKind::from_wire(kind as u8), Some(kind));
        }
        assert_eq!(PingKind::from_wire(PingKind::ALL.len() as u8), None);
    }

    #[test]
    fn a_burst_is_allowed_then_pings_wait() {
        let mut pings = Pings::default();
        let placed = (0..10).filter(|_| pings.spend()).count();
        assert_eq!(placed, BURST as usize);
        pings.tick(REFILL);
        assert!(pings.spend());
        assert!(!pings.spend());
    }

    /// Every kind's call (`pings.ron`) is in the library, starts and ends cleanly and
    /// sits at a sane level: the checks `audio.rs` gives the whole library.
    #[test]
    fn ping_calls_are_clean() {
        let library = SoundLibrary::load(
            &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../../data"),
        )
        .unwrap();
        let bank = Bank::synthesise(44_100, &library);
        for kind in PingKind::ALL {
            let name = kind.sound();
            let id = library
                .id_of(name)
                .unwrap_or_else(|| panic!("{name} is in the library"));
            let frames = bank.world(id);
            assert!(
                frames.iter().flatten().all(|s| s.is_finite()),
                "{name} has a non-finite sample"
            );
            let peak = frames.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((0.1..=0.9).contains(&peak), "{name} peaks at {peak}");
            assert!(
                frames[0].iter().all(|s| s.abs() < 0.02),
                "{name} starts at {:?}",
                frames[0]
            );
            assert!(
                frames[frames.len() - 1].iter().all(|s| s.abs() < 1e-4),
                "{name} ends at {:?}",
                frames[frames.len() - 1]
            );
        }
    }
}
