//! A network match's full-screen moments: waiting for every commander to load,
//! reconnecting after this machine's link dropped, and the report when the
//! machines fall out of step.

use super::{clock, Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::netplay::{DesyncReport, NetLink, Rejoining};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use mc_net::Link;

/// A state section's name as a player reads it.
fn section_label(name: &str) -> &'static str {
    match name {
        "clock" => "Clock",
        "players" => "Economy",
        "units" => "Units",
        "orders" => "Orders",
        "projectiles" => "Shots",
        "debris" => "Wrecks",
        "giants" => "Titans",
        "strategic" => "Strategic Weapons",
        "mines" => "Mines",
        "factories" => "Factories",
        "survival" => "Survival",
        "terrain" => "Terrain",
        "ai" => "AI",
        "navigation" => "Pathing",
        "fog" => "Vision",
        _ => "Other",
    }
}

impl Hud {
    /// Before the first tick: every seat, and whether its machine has built the match.
    pub(super) fn waiting_card(&mut self, ui: &mut Ui, s: &Scene, link: &NetLink) {
        let (w, h) = (ui.size.x, ui.size.y);
        let seats: Vec<_> = link
            .stats
            .iter()
            .filter(|p| (p.slot.index()) < s.view.status.players.len())
            .collect();
        // Past twelve seats (up to 32) they go in columns of up to sixteen, tighter.
        let columns = seats.len().div_ceil(16).max(1);
        let per_column = seats.len().div_ceil(columns).max(1);
        let row = if columns > 1 { 24.0 } else { 30.0 };
        let column_w = 420.0;
        let wide = column_w * columns as f32;
        let r = Rect::new(
            (w - wide) * 0.5,
            (h * 0.36).min(h - 120.0 - per_column as f32 * row),
            wide,
            96.0 + per_column as f32 * row,
        );
        self.claim(ui, r);
        ui.panel(r);
        ui.text_centred(
            w * 0.5,
            r.y + 34.0,
            type_scale::TITLE,
            rgb(palette::TEXT, 0.95),
            "Waiting for Commanders",
        );
        let k = (ui.time * 0.8).fract();
        ui.fill(
            Rect::new(w * 0.5 - 120.0 + 200.0 * k, r.y + 62.0, 40.0, 2.0),
            rgb(palette::ACCENT, 1.0),
        );
        ui.fill(
            Rect::new(w * 0.5 - 120.0, r.y + 62.0, 240.0, 1.0),
            rgb(palette::LINE, 0.25),
        );
        let loaded = link.loading.unwrap_or(0);
        for (i, p) in seats.iter().enumerate() {
            let x = r.x + (i / per_column) as f32 * column_w;
            let y = r.y + 88.0 + (i % per_column) as f32 * row;
            let slot = p.slot.0;
            ui.fill(Rect::new(x + 34.0, y - 4.0, 8.0, 8.0), s.team_color(slot));
            let name = &s.view.status.players[slot as usize].name;
            ui.text(
                x + 54.0,
                y,
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                name,
            );
            let (tag, tone) = if loaded & mc_core::player_bit(slot) != 0 {
                ("Ready".to_owned(), palette::TEXT)
            } else if p.link == Link::Dropped {
                ("Disconnected".to_owned(), palette::WARN)
            } else {
                let dots = ".".repeat(1 + (ui.time * 3.0) as usize % 3);
                (format!("Loading{dots}"), palette::DIM)
            };
            ui.text_right(
                x + column_w - 34.0,
                y,
                type_scale::CAPTION,
                rgb(tone, 1.0),
                &tag,
            );
        }
    }

    /// This machine's connection dropped: the match plays on, and we are trying to get back.
    pub(super) fn rejoin_band(&mut self, ui: &mut Ui, r: &Rejoining) {
        let (w, h) = (ui.size.x, ui.size.y);
        let k = ui.ease(id("rejoin-band", 0), 1.0, 9.0);
        ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.35 * k));
        let band = Rect::new(0.0, h * 0.5 - 86.0, w, 172.0);
        self.claim(ui, band);
        ui.scrim(Rect::new(0.0, band.y, w * 0.5, band.h), 0.0, 0.8 * k, true);
        ui.scrim(
            Rect::new(w * 0.5, band.y, w * 0.5, band.h),
            0.8 * k,
            0.0,
            true,
        );
        let pulse = 0.6 + 0.4 * (ui.time * 3.0).sin().abs();
        ui.text_centred(
            w * 0.5,
            band.y + 52.0,
            type_scale::TITLE,
            rgb(palette::WARN, k * pulse),
            "Connection Lost",
        );
        let waited = r.since.elapsed().as_secs_f32();
        ui.text_centred(
            w * 0.5,
            band.y + 88.0,
            type_scale::CAPTION,
            rgb(palette::DIM, k),
            &format!(
                "Reconnecting  \u{b7}  attempt {}  \u{b7}  {}  \u{b7}  the battle goes on without you until you are back",
                r.attempts.max(1),
                clock(waited)
            ),
        );
        // A sweep along a rule under the words: still trying.
        let run = 320.0;
        let t = (ui.time * 0.7).fract();
        ui.fill(
            Rect::new(w * 0.5 - run * 0.5, band.y + 108.0, run, 1.0),
            rgb(palette::LINE, 0.2 * k),
        );
        ui.fill(
            Rect::new(
                w * 0.5 - run * 0.5 + (run - 60.0) * t,
                band.y + 108.0,
                60.0,
                2.0,
            ),
            rgb(palette::WARN, k),
        );
        if ui.button(
            id("rejoin-leave", 0),
            Rect::new(w * 0.5 - 110.0, band.y + 122.0, 220.0, 38.0),
            "Leave Match",
            ButtonKind::Secondary,
            true,
        ) {
            ui.audio.play(Sfx::Back);
            self.actions.push(HudAction::Leave);
        }
    }

    /// The machines no longer agree on the battle: what differs, whose, and where
    /// this machine's state was written for a look afterwards.
    pub(super) fn desync_card(&mut self, ui: &mut Ui, s: &Scene, d: &DesyncReport) {
        let (w, h) = (ui.size.x, ui.size.y);
        ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.45));
        let pw = 640.0;
        let rows = d.hashes.len() as f32 * 26.0;
        let r = Rect::new((w - pw) * 0.5, h * 0.24, pw, 282.0 + rows);
        self.claim(ui, r);
        ui.panel(r);
        let (x, cw) = (r.x + 36.0, r.w - 72.0);
        ui.text_centred(
            w * 0.5,
            r.y + 44.0,
            type_scale::TITLE,
            rgb(palette::BAD, 1.0),
            "Out of Step",
        );
        let when = s.view.status.error.clone().unwrap_or_default();
        ui.text_centred(
            w * 0.5,
            r.y + 80.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &when,
        );
        ui.text_centred(
            w * 0.5,
            r.y + 104.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            "Every machine plays the same battle; this one's no longer matches, so the match stops.",
        );
        let apart = d.differing();
        let (label, tone) = if apart.is_empty() {
            let dots = ".".repeat(1 + (ui.time * 3.0) as usize % 3);
            (format!("Comparing reports{dots}"), palette::DIM)
        } else {
            let names: Vec<&str> = apart.iter().map(|n| section_label(n)).collect();
            (
                format!("Differs in: {}", names.join("  \u{b7}  ")),
                palette::WARN,
            )
        };
        ui.text_centred(
            w * 0.5,
            r.y + 140.0,
            type_scale::VALUE,
            rgb(tone, 1.0),
            &label,
        );

        // Every machine's hash for that tick; those in the minority are the odd ones out.
        let mut y = r.y + 176.0;
        for (slot, hash) in &d.hashes {
            let agree = d.hashes.iter().filter(|(_, h)| h == hash).count();
            let odd = agree * 2 <= d.hashes.len() && d.hashes.len() > 2;
            ui.fill(Rect::new(x, y - 4.0, 8.0, 8.0), s.team_color(*slot));
            let name = s
                .view
                .status
                .players
                .get(*slot as usize)
                .map_or("", |p| p.name.as_str());
            let you = if Some(*slot) == d.local {
                "  (you)"
            } else {
                ""
            };
            ui.text(
                x + 20.0,
                y,
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                &format!("{name}{you}"),
            );
            ui.text_right(
                x + cw,
                y,
                type_scale::CAPTION,
                rgb(if odd { palette::BAD } else { palette::DIM }, 1.0),
                &format!("{hash:016x}"),
            );
            y += 26.0;
        }
        let saved = match &d.dump {
            Some(path) => format!("This machine's state was saved to {}", path.display()),
            None => "This machine's state could not be saved.".to_owned(),
        };
        ui.text_fit(
            w * 0.5,
            y + 10.0,
            cw,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &saved,
        );
        if ui.button(
            id("desync-leave", 0),
            Rect::new(w * 0.5 - 120.0, r.bottom() - 62.0, 240.0, 42.0),
            "Leave Match",
            ButtonKind::Primary,
            true,
        ) {
            ui.audio.play(Sfx::Back);
            self.actions.push(HudAction::Leave);
        }
    }
}
