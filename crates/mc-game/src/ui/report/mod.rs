//! The battle report: the whole match told back when it is decided. A verdict
//! over the battlefield, then five pages: the scoreboard and honours, the
//! economy, the fighting, the battlefield replayed from above, and the moments
//! of the match in order. Opened by the match's end, and from the in-match
//! menu once the match is over (or at any time by an observer).

pub mod analysis;
mod battlefield;
mod chart;
mod economy;
mod military;
mod overview;
#[cfg(test)]
mod tests;
mod timeline;

use super::{id, ink, palette, rgb, style, type_scale, ButtonKind, Color, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::chronicle::Chronicle;
use crate::hud::thumbs::Thumbs;
use analysis::{clock, short, Analysis, Metric};
use glam::Vec2;
use mc_data::Blueprints;
use mc_render::Face;

/// What the report needs from the match around it.
pub struct Ctx<'a> {
    pub blueprints: &'a Blueprints,
    /// Player colours by player index, linear RGB.
    pub colors: &'a [[f32; 3]],
    /// The side this machine plays; `None` for an observer or a replay.
    pub local: Option<u8>,
    pub map_name: &'a str,
    pub thumbs: &'a Thumbs,
    /// The overlay image slot holding the map's chart.
    pub chart: usize,
    /// Leaving gives up a network match this side is still in.
    pub surrender: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReportAction {
    /// Back to the battlefield.
    Close,
    Leave,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Overview,
    Economy,
    Military,
    Battlefield,
    Timeline,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Overview,
        Tab::Economy,
        Tab::Military,
        Tab::Battlefield,
        Tab::Timeline,
    ];

    fn name(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Economy => "Economy",
            Tab::Military => "Military",
            Tab::Battlefield => "Battlefield",
            Tab::Timeline => "Timeline",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Tab::Overview => "Scoreboard and honours",
            Tab::Economy => "Income, spending, stores",
            Tab::Military => "Armies, kills, losses",
            Tab::Battlefield => "The match replayed",
            Tab::Timeline => "Every turning point",
        }
    }

    fn parse(s: &str) -> Option<Tab> {
        Tab::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(s))
    }
}

/// The verdict at the head of the report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    Victory,
    Defeat,
    Complete,
    /// Not decided yet: an observer looking in part way through.
    Running,
}

pub struct Report {
    a: Analysis,
    verdict: Verdict,
    tab: Tab,
    /// Seconds since the report opened, and since the page was turned.
    age: f32,
    tab_age: f32,
    economy: Metric,
    military: Metric,
    /// A side picked out on the charts (the legend under the pointer).
    focus: Option<usize>,
    field: battlefield::Playback,
    timeline: timeline::State,
}

/// Eases 0..1 in fast and settles.
fn ease(k: f32) -> f32 {
    1.0 - (1.0 - k.clamp(0.0, 1.0)).powi(3)
}

impl Report {
    pub fn new(chronicle: &Chronicle, blueprints: &Blueprints, local: Option<u8>) -> Report {
        let a = Analysis::of(chronicle, blueprints);
        let verdict = match (a.winner, local.and_then(|l| a.sides.get(l as usize))) {
            (None, _) => Verdict::Running,
            (Some(team), Some(me)) if me.team == team => Verdict::Victory,
            (Some(_), Some(_)) => Verdict::Defeat,
            (Some(_), None) => Verdict::Complete,
        };
        let field = battlefield::Playback::new(&a);
        Report {
            a,
            verdict,
            tab: Tab::Overview,
            age: 0.0,
            tab_age: 0.0,
            economy: Metric::MassIncome,
            military: Metric::ArmyValue,
            focus: None,
            field,
            timeline: timeline::State::default(),
        }
    }

    /// A report as a headless shot shows it (`--report PAGE[@M:SS]`): open on PAGE,
    /// with everything drawn in, and the battlefield replay stopped at M:SS (or the end).
    pub fn staged(
        chronicle: &Chronicle,
        blueprints: &Blueprints,
        local: Option<u8>,
        spec: &str,
    ) -> Result<Report, String> {
        let (page, at) = spec.split_once('@').unwrap_or((spec, ""));
        let mut report = Report::new(chronicle, blueprints, local);
        report.tab = Tab::parse(page).ok_or_else(|| {
            let pages: Vec<String> = Tab::ALL.iter().map(|t| t.name().to_lowercase()).collect();
            format!("--report takes one of {}", pages.join(", "))
        })?;
        (report.age, report.tab_age) = (10.0, 10.0);
        let tick = match at.split_once(':') {
            Some((m, s)) => {
                let (m, s): (u32, u32) = m
                    .parse()
                    .ok()
                    .zip(s.parse().ok())
                    .ok_or("--report PAGE@M:SS takes minutes and seconds")?;
                (m * 60 + s) * mc_core::TICKS_PER_SECOND
            }
            None => report.a.length,
        };
        report.field.seek(tick);
        Ok(report)
    }

    fn turn_to(&mut self, ui: &mut Ui, tab: Tab) {
        if tab != self.tab {
            ui.audio.play(Sfx::Select);
            self.tab = tab;
            self.tab_age = 0.0;
            self.focus = None;
        }
    }

    /// Draws the report over the whole screen; `enter` is its fade, 0 to 1.
    pub fn draw(&mut self, ui: &mut Ui, ctx: &Ctx, enter: f32) -> Option<ReportAction> {
        self.age += ui.dt;
        self.tab_age += ui.dt;
        let (w, h) = (ui.size.x, ui.size.y);
        let mut out = None;
        ui.fade = enter;
        ui.frost(Rect::new(0.0, 0.0, w, h), 0.9);
        let tone = self.tone();
        // A wash of the verdict's colour down from the top, and darker corners.
        ui.gradient_v(
            Rect::new(0.0, 0.0, w, 300.0),
            rgb(tone, 0.07),
            rgb(tone, 0.0),
        );
        ui.scrim(Rect::new(0.0, h - 220.0, w, 220.0), 0.0, 0.45, false);
        ui.shift.y = 16.0 * (1.0 - enter);

        let cw = (w - 96.0).min(2200.0);
        let x0 = (w - cw) * 0.5;
        self.header(ui, ctx, Rect::new(x0, 34.0, cw, 128.0));
        let tabs = Rect::new(x0, 176.0, cw, 52.0);
        self.tabs(ui, tabs);
        let body = Rect::new(
            x0,
            tabs.bottom() + 26.0,
            cw,
            h - tabs.bottom() - 26.0 - 96.0,
        );

        // The page slides up into place as it is turned to.
        let k = ease(self.tab_age / 0.35);
        ui.fade = enter * k;
        ui.shift.y = 16.0 * (1.0 - enter) + 10.0 * (1.0 - k);
        match self.tab {
            Tab::Overview => overview::draw(self, ui, ctx, body),
            Tab::Economy => economy::draw(self, ui, ctx, body),
            Tab::Military => military::draw(self, ui, ctx, body),
            Tab::Battlefield => battlefield::draw(self, ui, ctx, body),
            Tab::Timeline => {
                if let Some(tick) = timeline::draw(self, ui, ctx, body) {
                    // A moment picked: the battlefield at that point of the match.
                    self.field.seek(tick);
                    self.turn_to(ui, Tab::Battlefield);
                }
            }
        }
        ui.fade = enter;
        ui.shift.y = 16.0 * (1.0 - enter);
        if let Some(a) = self.footer(ui, ctx, Rect::new(x0, h - 76.0, cw, 50.0)) {
            out = Some(a);
        }
        if ui.interactive {
            let i = Tab::ALL.iter().position(|&t| t == self.tab).unwrap_or(0);
            let typed = ui
                .input
                .typed
                .chars()
                .filter_map(|c| c.to_digit(10))
                .next_back();
            if let Some(n) = typed.filter(|n| (1..=Tab::ALL.len() as u32).contains(n)) {
                self.turn_to(ui, Tab::ALL[n as usize - 1]);
            } else if ui.input.key(Key::Right) || ui.input.key(Key::Tab) {
                self.turn_to(ui, Tab::ALL[(i + 1) % Tab::ALL.len()]);
            } else if ui.input.key(Key::Left) {
                self.turn_to(ui, Tab::ALL[(i + Tab::ALL.len() - 1) % Tab::ALL.len()]);
            }
            if ui.input.key(Key::Escape) {
                ui.audio.play(Sfx::Back);
                out = Some(ReportAction::Close);
            }
        }
        ui.fade = 1.0;
        ui.shift = Vec2::ZERO;
        out
    }

    fn tone(&self) -> u32 {
        match self.verdict {
            Verdict::Defeat => palette::BAD,
            _ => palette::ACCENT,
        }
    }

    /// A figure counting up from nothing as the report opens; `delay` staggers them.
    fn count(&self, v: f32, delay: f32) -> f32 {
        v * ease((self.age - delay) / 0.9)
    }

    /// How far in a block is drawn, for blocks that come in one after another.
    fn reveal(&self, delay: f32) -> f32 {
        ease((self.tab_age - delay) / 0.8)
    }

    fn header(&self, ui: &mut Ui, ctx: &Ctx, r: Rect) {
        let tone = self.tone();
        let (word, line) = match self.verdict {
            Verdict::Victory => ("Victory", "Every enemy commander is destroyed".to_string()),
            Verdict::Defeat => ("Defeat", "Your commander has been destroyed".to_string()),
            Verdict::Complete => (
                "Engagement Complete",
                format!(
                    "{} holds the field",
                    self.a.winner.map_or(String::new(), |t| self.a.team_name(t))
                ),
            ),
            Verdict::Running => ("Battle Report", "The match so far".to_string()),
        };
        // The overline, its bar running out as the report opens.
        let bar = 34.0 * ease(self.age / 0.5);
        ui.fill(Rect::new(r.x, r.y + 4.0, bar, 2.0), rgb(tone, 1.0));
        ui.text(
            r.x + 46.0,
            r.y + 5.0,
            type_scale::OVERLINE,
            rgb(tone, 1.0),
            if self.verdict == Verdict::Running {
                "In Progress"
            } else {
                "Battle Report"
            },
        );
        // The verdict, spaced wide as it lands and closing up.
        let settle = ease(self.age / 0.9);
        let big = style(Face::Light, 64.0, 2.0 + 10.0 * (1.0 - settle));
        ui.text(r.x - 3.0, r.y + 52.0, big, rgb(palette::TEXT, settle), word);
        let rule = ui.text_width(big, word);
        ui.gradient_h(
            Rect::new(r.x, r.y + 90.0, (rule + 120.0) * settle, 1.0),
            rgb(tone, 0.9),
            rgb(tone, 0.0),
        );
        let sides = self.a.sides.len();
        let facts = format!(
            "{}   \u{b7}   {}   \u{b7}   {} sides   \u{b7}   {}",
            ctx.map_name,
            clock(self.a.length),
            sides,
            line
        );
        ui.text(
            r.x,
            r.y + 112.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            &facts,
        );

        // Headline figures on the right, counting up.
        let figures = [
            (
                "Duration",
                clock((self.count(self.a.length as f32, 0.2)) as u32),
            ),
            (
                "Materials Destroyed",
                short(self.count(self.a.total_destroyed, 0.3)),
            ),
            (
                "Units Lost",
                format!("{:.0}", self.count(self.a.total_deaths as f32, 0.4)),
            ),
            ("Battles", format!("{}", self.a.battles.len())),
        ];
        let fw = 170.0;
        let mut x = r.right() - fw * figures.len() as f32;
        for (i, (label, value)) in figures.iter().enumerate() {
            let k = ease((self.age - 0.15 * i as f32) / 0.5);
            let y = r.y + 40.0 + 8.0 * (1.0 - k);
            ui.vline(x, r.y + 22.0, 70.0, rgb(palette::LINE, 0.14 * k));
            ui.text(x + 16.0, y, type_scale::MICRO, rgb(palette::DIM, k), label);
            ui.text(
                x + 16.0,
                y + 34.0,
                style(Face::Light, 34.0, 0.5),
                rgb(palette::TEXT, k),
                value,
            );
            x += fw;
        }
    }

    fn tabs(&mut self, ui: &mut Ui, r: Rect) {
        let tw = (r.w / Tab::ALL.len() as f32).min(260.0);
        for (i, &tab) in Tab::ALL.iter().enumerate() {
            let k = ease((self.age - 0.25 - 0.06 * i as f32) / 0.4);
            let t = Rect::new(r.x + i as f32 * (tw + 8.0), r.y + 6.0 * (1.0 - k), tw, r.h);
            let lit = tab == self.tab;
            let res = ui.tile(id("report-tab", i), t, lit, true);
            let a = k * (0.7 + 0.3 * res.glow);
            ui.text(
                t.x + 16.0,
                t.y + 18.0,
                type_scale::ITEM,
                rgb(if lit { palette::TEXT } else { palette::DIM }, a),
                tab.name(),
            );
            ui.text(
                t.x + 16.0,
                t.y + 37.0,
                type_scale::MICRO,
                rgb(
                    if lit { palette::TEXT } else { palette::FAINT },
                    if lit { 0.75 * k } else { k },
                ),
                tab.blurb(),
            );
            if lit {
                ui.fill(
                    Rect::new(t.x, t.y + 8.0, 3.0, t.h - 16.0),
                    rgb(palette::ACCENT, k),
                );
            }
            ui.text_right(
                t.right() - 12.0,
                t.y + 18.0,
                type_scale::MICRO,
                rgb(palette::FAINT, 0.8 * k),
                &format!("{}", i + 1),
            );
            if res.clicked {
                self.turn_to(ui, tab);
            }
        }
    }

    fn footer(&self, ui: &mut Ui, ctx: &Ctx, r: Rect) -> Option<ReportAction> {
        let mut out = None;
        // The keys, as caps.
        let mut x = r.x;
        for (key, what) in [("Esc", "Back to the battlefield"), ("1-5", "Turn the page")] {
            let w = ui.text_width(type_scale::MICRO, key) + 12.0;
            let cap = Rect::new(x, r.mid_y() - 9.0, w, 18.0);
            ui.frame(cap, rgb(palette::LINE, 0.4));
            ui.text_centred(
                cap.x + w * 0.5,
                cap.mid_y(),
                type_scale::MICRO,
                rgb(palette::TEXT, 0.8),
                key,
            );
            x = ui.text(
                cap.right() + 8.0,
                r.mid_y(),
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                what,
            ) + 22.0;
        }
        let bw = 220.0;
        let mut x = r.right() - bw;
        if ui.button(
            id("report-quit", 0),
            Rect::new(x, r.y, bw, r.h),
            "Exit to Desktop",
            ButtonKind::Secondary,
            true,
        ) {
            ui.audio.play(Sfx::Back);
            out = Some(ReportAction::Quit);
        }
        x -= bw + 12.0;
        let leave = if ctx.surrender {
            "Surrender and Leave"
        } else {
            "Leave Match"
        };
        let primary = self.verdict != Verdict::Running;
        if ui.button(
            id("report-leave", 0),
            Rect::new(x, r.y, bw, r.h),
            leave,
            if primary {
                ButtonKind::Primary
            } else {
                ButtonKind::Secondary
            },
            true,
        ) {
            ui.audio.play(Sfx::Select);
            out = Some(ReportAction::Leave);
        }
        x -= bw + 12.0;
        let back = if primary {
            "Keep Watching"
        } else {
            "Back to the Match"
        };
        if ui.button(
            id("report-close", 0),
            Rect::new(x, r.y, bw, r.h),
            back,
            ButtonKind::Secondary,
            true,
        ) {
            ui.audio.play(Sfx::Back);
            out = Some(ReportAction::Close);
        }
        out
    }

    /// A side's colour, lifted where it would be lost on the black glass.
    fn color(&self, ctx: &Ctx, side: usize) -> Color {
        side_color(ctx.colors, side)
    }
}

pub(super) fn side_color(colors: &[[f32; 3]], side: usize) -> Color {
    let c = colors
        .get(side % colors.len().max(1))
        .copied()
        .unwrap_or([0.8, 0.8, 0.8]);
    let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let lift = ((0.10 - lum) / 0.10).clamp(0.0, 1.0) * 0.45;
    [
        c[0] + (1.0 - c[0]) * lift,
        c[1] + (1.0 - c[1]) * lift,
        c[2] + (1.0 - c[2]) * lift,
        1.0,
    ]
}

/// A labelled block on a page: a section heading over it. Returns the space under it.
fn block(ui: &mut Ui, r: Rect, title: &str) -> Rect {
    ui.section(r.x, r.y + 6.0, r.w, title);
    Rect::new(r.x, r.y + 24.0, r.w, r.h - 24.0)
}

/// A row of chips to pick one of `options`; returns the one clicked.
fn chips(ui: &mut Ui, key: &str, r: Rect, options: &[&str], chosen: usize) -> Option<usize> {
    let mut x = r.x;
    let mut out = None;
    for (i, o) in options.iter().enumerate() {
        let w = ui.text_width(type_scale::CAPTION, o) + 28.0;
        if x + w > r.right() {
            break;
        }
        let c = Rect::new(x, r.y, w, r.h);
        let res = ui.tile(id(key, i), c, i == chosen, true);
        ui.text_centred(
            c.x + c.w * 0.5,
            c.mid_y() - 1.0,
            type_scale::CAPTION,
            rgb(
                if i == chosen {
                    palette::TEXT
                } else {
                    palette::DIM
                },
                0.8 + 0.2 * res.glow,
            ),
            o,
        );
        if res.clicked && i != chosen {
            ui.audio.play(Sfx::Tick);
            out = Some(i);
        }
        x += w + 6.0;
    }
    out
}

/// The sides as a legend in a row; hovering one picks it out on the charts.
fn legend(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let mut x = r.x;
    let mut hovered = None;
    for i in 0..report.a.sides.len() {
        let name = report.a.sides[i].name.clone();
        let w = (ui.text_width(type_scale::CAPTION, &name) + 34.0).min(220.0);
        if x + w > r.right() {
            break;
        }
        let row = Rect::new(x, r.y, w, r.h);
        let res = ui.interact_with(id("report-legend", i), row, true, false);
        let picked = report.focus.is_none_or(|f| f == i);
        let c = report.color(ctx, i);
        ui.fill(
            Rect::new(x, r.mid_y() - 1.5, 14.0, 3.0),
            [c[0], c[1], c[2], if picked { 1.0 } else { 0.3 }],
        );
        let (st, name) = ui.fitted(type_scale::CAPTION, &name, w - 30.0);
        ui.text(
            x + 20.0,
            r.mid_y(),
            st,
            rgb(palette::TEXT, if picked { 0.9 } else { 0.4 }),
            &name,
        );
        if res.hovered {
            hovered = Some(i);
        }
        x += w + 10.0;
    }
    report.focus = hovered;
}

/// A dark well for a block's contents.
fn well(ui: &mut Ui, r: Rect) {
    ui.fill_cut(r, 6.0, ink(0.4));
    ui.outline_cut(r, 6.0, rgb(palette::LINE, 0.08), rgb(palette::LINE, 0.2));
}
